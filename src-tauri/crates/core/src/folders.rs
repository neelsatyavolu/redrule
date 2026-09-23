//! Shared folders: links, keys, the list of joined folders, and the cache of other people's meetings.
//! The service contract is in docs/superpowers/specs/2026-09-22-shared-folders-design.md.
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use chrono::{DateTime, Utc};
use rand::RngCore;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use super::error::{Error, Result};
use super::models::{Meeting, MeetingApp, MeetingNote, MeetingStatus, TranscriptSegment, iso8601};
use super::store::{read, read_optional, write};

pub const MAX_NAME: usize = 80;
pub const MAX_DISPLAY_NAME: usize = 60;

/// Folder ids and keys are 64 lowercase hex characters.
pub fn valid_key(text: &str) -> bool {
    text.len() == 64 && text.bytes().all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}

/// Meeting ids are uppercase UUIDs, as the store names its folders.
pub fn valid_meeting_id(text: &str) -> bool {
    uuid::Uuid::parse_str(text).is_ok() && text.len() == 36 && !text.bytes().any(|b| b.is_ascii_lowercase())
}

pub fn link(base: &str, id: &str, member_key: &str) -> String {
    format!("{base}/f/{id}#{member_key}")
}

/// Reads `…/f/<id>#<memberKey>` from pasted text. The host is ignored: Redrule only talks to its own service.
pub fn parse_link(text: &str) -> Result<(String, String)> {
    let invalid = || Error::message("That isn't a Redrule folder link. Copy the whole link, including the part after #.");
    let text = text.trim();
    let (path, key) = text.split_once('#').ok_or_else(invalid)?;
    let id = path.trim_end_matches('/').rsplit_once("/f/").map(|(_, id)| id).ok_or_else(invalid)?;
    if !valid_key(id) || !valid_key(key) {
        return Err(invalid());
    }
    Ok((id.to_string(), key.to_string()))
}

pub(crate) fn sha256_hex(bytes: &[u8]) -> String {
    Sha256::digest(bytes).iter().map(|b| format!("{b:02x}")).collect()
}

/// 32 random bytes from the thread's CSPRNG, as 64 lowercase hex characters.
pub(crate) fn random_key() -> String {
    let mut bytes = [0u8; 32];
    rand::rng().fill_bytes(&mut bytes);
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

/// Proves this Mac uploaded a meeting. Derived rather than stored, and unchanged when a folder's link is reset.
pub fn edit_key(device_secret: &str, meeting_id: &str) -> String {
    sha256_hex(format!("{device_secret}:{meeting_id}").as_bytes())
}

/// A random secret kept in `path`, created on first use. Never replaced: uploads made with it could not be changed again.
pub fn device_secret(path: &Path) -> Result<String> {
    match fs::read_to_string(path) {
        Ok(existing) if valid_key(existing.trim()) => return Ok(existing.trim().to_string()),
        Ok(_) => return Err(Error::message("This Mac's shared-folder key is damaged.")),
        Err(error) if error.kind() != std::io::ErrorKind::NotFound => return Err(error.into()),
        Err(_) => {}
    }
    let secret = random_key();
    write_private(path, secret.as_bytes())?;
    Ok(secret)
}

/// Writes a file only this user can read, from the moment it exists: it holds keys.
fn write_private(path: &Path, bytes: &[u8]) -> Result<()> {
    use std::io::Write;
    use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let temporary = path.with_extension("tmp");
    let mut file = fs::OpenOptions::new().write(true).create(true).truncate(true).mode(0o600).open(&temporary)?;
    // A leftover temporary file keeps its old mode; tighten it too.
    file.set_permissions(fs::Permissions::from_mode(0o600))?;
    file.write_all(bytes)?;
    fs::rename(&temporary, path)?;
    Ok(())
}

/// Trims a folder or display name to one line of at most `max` characters.
pub fn clean_name(name: &str, max: usize) -> String {
    name.split_whitespace().collect::<Vec<_>>().join(" ").chars().take(max).collect()
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct JoinedFolder {
    pub id: String,
    pub name: String,
    pub member_key: String,
    /// Only on the Mac that created the folder.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub owner_key: Option<String>,
}

/// `folders.json`: the folders this Mac belongs to, and the name shown on its uploads.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FolderRegistry {
    #[serde(default)]
    pub display_name: String,
    #[serde(default)]
    pub folders: Vec<JoinedFolder>,
}

impl FolderRegistry {
    pub fn load(path: &Path) -> Result<Self> {
        Ok(read_optional::<Self>(path)?.unwrap_or_default())
    }

    pub fn save(&self, path: &Path) -> Result<()> {
        write_private(path, &serde_json::to_vec_pretty(self)?)
    }

    pub fn folder(&self, id: &str) -> Option<&JoinedFolder> {
        self.folders.iter().find(|f| f.id == id)
    }

    /// A copy with `folder` added, or replacing the one with its id.
    pub fn with_folder(&self, folder: JoinedFolder) -> Self {
        let mut folders: Vec<JoinedFolder> = self.folders.iter().filter(|f| f.id != folder.id).cloned().collect();
        folders.push(folder);
        Self { folders, ..self.clone() }
    }

    pub fn without(&self, id: &str) -> Self {
        Self { folders: self.folders.iter().filter(|f| f.id != id).cloned().collect(), ..self.clone() }
    }
}

/// A meeting as the folder service stores it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SharedMeeting {
    pub title: String,
    pub app: MeetingApp,
    #[serde(with = "iso8601")]
    pub started_at: DateTime<Utc>,
    #[serde(with = "iso8601::option", default)]
    pub ended_at: Option<DateTime<Utc>>,
    pub recorded_by: String,
    pub note: MeetingNote,
    pub transcript: Vec<TranscriptSegment>,
}

impl SharedMeeting {
    pub fn new(meeting: &Meeting, note: MeetingNote, transcript: Vec<TranscriptSegment>, recorded_by: &str) -> Self {
        Self {
            title: meeting.title.clone(),
            app: meeting.app,
            started_at: meeting.started_at,
            ended_at: meeting.ended_at,
            recorded_by: recorded_by.to_string(),
            note,
            transcript,
        }
    }

    /// Changes whenever the upload would differ, so unchanged meetings are not sent again.
    pub fn content_hash(&self) -> Result<String> {
        Ok(sha256_hex(&serde_json::to_vec(self)?))
    }
}

/// Someone else's meeting, downloaded from a folder.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RemoteMeeting {
    pub id: String,
    pub updated_at: String,
    #[serde(flatten)]
    pub meeting: SharedMeeting,
}

impl RemoteMeeting {
    /// The read-only meeting the sidebar lists.
    pub fn listed(&self, folder_id: &str) -> Meeting {
        Meeting {
            id: self.id.clone(),
            title: self.meeting.title.clone(),
            app: self.meeting.app,
            started_at: self.meeting.started_at,
            ended_at: self.meeting.ended_at,
            status: MeetingStatus::Done,
            error_message: None,
            archived_at: None,
            tags: vec![],
            folder_id: Some(folder_id.to_string()),
        }
    }
}

/// `folders/<id>/`: downloaded meetings, and the content hash of each meeting this Mac uploaded.
#[derive(Debug, Clone)]
pub struct FolderCache {
    root: PathBuf,
}

impl FolderCache {
    pub fn new(root: PathBuf) -> Self {
        Self { root }
    }

    fn folder(&self, id: &str) -> Result<PathBuf> {
        if !valid_key(id) {
            return Err(Error::message("That folder does not exist."));
        }
        Ok(self.root.join(id))
    }

    fn meeting_file(&self, folder_id: &str, meeting_id: &str) -> Result<PathBuf> {
        if !valid_meeting_id(meeting_id) {
            return Err(Error::message("That meeting does not exist."));
        }
        Ok(self.folder(folder_id)?.join("meetings").join(format!("{meeting_id}.json")))
    }

    pub fn meetings(&self, folder_id: &str) -> Result<Vec<RemoteMeeting>> {
        let dir = self.folder(folder_id)?.join("meetings");
        let Ok(entries) = fs::read_dir(dir) else { return Ok(Vec::new()) };
        Ok(entries.filter_map(|entry| read::<RemoteMeeting>(&entry.ok()?.path()).ok()).collect())
    }

    pub fn meeting(&self, folder_id: &str, meeting_id: &str) -> Result<Option<RemoteMeeting>> {
        read_optional(&self.meeting_file(folder_id, meeting_id)?)
    }

    pub fn save_meeting(&self, folder_id: &str, meeting: &RemoteMeeting) -> Result<()> {
        let file = self.meeting_file(folder_id, &meeting.id)?;
        fs::create_dir_all(file.parent().expect("meeting files sit in a folder"))?;
        write(&file, meeting)
    }

    pub fn remove_meeting(&self, folder_id: &str, meeting_id: &str) -> Result<()> {
        let file = self.meeting_file(folder_id, meeting_id)?;
        if file.exists() {
            fs::remove_file(file)?;
        }
        Ok(())
    }

    pub fn uploads(&self, folder_id: &str) -> Result<BTreeMap<String, String>> {
        Ok(read_optional(&self.folder(folder_id)?.join("uploads.json"))?.unwrap_or_default())
    }

    pub fn save_uploads(&self, folder_id: &str, uploads: &BTreeMap<String, String>) -> Result<()> {
        let folder = self.folder(folder_id)?;
        fs::create_dir_all(&folder)?;
        write(&folder.join("uploads.json"), uploads)
    }

    pub fn remove_folder(&self, id: &str) -> Result<()> {
        let folder = self.folder(id)?;
        if folder.exists() {
            fs::remove_dir_all(folder)?;
        }
        Ok(())
    }

    /// Keeps the cache when a link reset gives the folder a new id.
    pub fn rename_folder(&self, from: &str, to: &str) -> Result<()> {
        let (from, to) = (self.folder(from)?, self.folder(to)?);
        if from.exists() && !to.exists() {
            fs::rename(from, to)?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const ID: &str = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";
    const KEY: &str = "fedcba9876543210fedcba9876543210fedcba9876543210fedcba9876543210";
    const MEETING: &str = "0B5F1C9E-6C8B-4A1E-9F1D-2C3B4A5D6E7F";

    fn note() -> MeetingNote {
        MeetingNote { title: "T".into(), tldr: "S".into(), sections: vec![], decisions: vec![], action_items: vec![] }
    }

    fn remote(id: &str) -> RemoteMeeting {
        let meeting: Meeting = serde_json::from_str(r#"{"app":"zoom","id":"X","startedAt":"2026-09-21T14:05:00Z","status":"done","title":"Kickoff"}"#).unwrap();
        RemoteMeeting { id: id.into(), updated_at: "2026-09-22T10:00:00.000Z".into(), meeting: SharedMeeting::new(&meeting, note(), vec![], "Dana") }
    }

    #[test]
    fn links_round_trip_and_reject_anything_else() {
        let url = link("https://redrule.vercel.app", ID, KEY);
        assert_eq!(parse_link(&format!("  {url}\n")).unwrap(), (ID.to_string(), KEY.to_string()));
        assert!(parse_link(&format!("https://redrule.vercel.app/f/{ID}")).is_err());
        assert!(parse_link(&format!("https://redrule.vercel.app/s/{ID}#{KEY}")).is_err());
        assert!(parse_link(&format!("https://x/f/../../etc#{KEY}")).is_err());
        assert!(parse_link(&format!("https://x/f/{}#{KEY}", ID.to_uppercase())).is_err());
    }

    #[test]
    fn edit_keys_depend_on_the_device_and_meeting() {
        let key = edit_key("a", MEETING);
        assert!(valid_key(&key));
        assert_eq!(key, edit_key("a", MEETING));
        assert_ne!(key, edit_key("b", MEETING));
    }

    #[test]
    fn device_secret_is_created_once_and_private() {
        use std::os::unix::fs::PermissionsExt;
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("device.key");
        let secret = device_secret(&path).unwrap();
        assert!(valid_key(&secret));
        assert_eq!(device_secret(&path).unwrap(), secret);
        assert_eq!(fs::metadata(&path).unwrap().permissions().mode() & 0o777, 0o600);
        // A damaged key is reported, never silently replaced.
        fs::write(&path, "garbage").unwrap();
        assert!(device_secret(&path).is_err());
        assert_eq!(fs::read_to_string(&path).unwrap(), "garbage");
    }

    #[test]
    fn registry_adds_replaces_and_removes_folders() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("folders.json");
        assert_eq!(FolderRegistry::load(&path).unwrap(), FolderRegistry::default());
        let folder = JoinedFolder { id: ID.into(), name: "Acme".into(), member_key: KEY.into(), owner_key: None };
        let registry = FolderRegistry { display_name: "Neel".into(), ..Default::default() }
            .with_folder(folder.clone())
            .with_folder(JoinedFolder { name: "Acme team".into(), ..folder });
        registry.save(&path).unwrap();
        let loaded = FolderRegistry::load(&path).unwrap();
        assert_eq!(loaded.folders.len(), 1);
        assert_eq!(loaded.folder(ID).unwrap().name, "Acme team");
        assert!(loaded.without(ID).folders.is_empty());
    }

    #[test]
    fn remote_meetings_read_the_service_format() {
        let json = format!(
            r#"{{"id":"{MEETING}","updatedAt":"2026-09-22T10:00:00.000Z","title":"Kickoff","app":"zoom","startedAt":"2026-09-21T14:05:00Z","endedAt":null,"recordedBy":"Dana","note":{{"title":"T","tldr":"S","sections":[],"decisions":[],"actionItems":[]}},"transcript":[{{"speaker":"them","start":1,"end":2,"text":"hi"}}]}}"#
        );
        let meeting: RemoteMeeting = serde_json::from_str(&json).unwrap();
        assert_eq!(meeting.meeting.recorded_by, "Dana");
        assert_eq!(meeting.meeting.ended_at, None);
        let listed = meeting.listed(ID);
        assert_eq!((listed.status, listed.folder_id.as_deref()), (MeetingStatus::Done, Some(ID)));
    }

    #[test]
    fn content_hash_changes_with_the_content() {
        let a = remote(MEETING).meeting;
        let b = SharedMeeting { recorded_by: "Ada".into(), ..a.clone() };
        assert_eq!(a.content_hash().unwrap(), a.content_hash().unwrap());
        assert_ne!(a.content_hash().unwrap(), b.content_hash().unwrap());
    }

    #[test]
    fn cache_stores_meetings_and_uploads_per_folder() {
        let dir = tempfile::tempdir().unwrap();
        let cache = FolderCache::new(dir.path().join("folders"));
        assert!(cache.meetings(ID).unwrap().is_empty());
        cache.save_meeting(ID, &remote(MEETING)).unwrap();
        assert_eq!(cache.meeting(ID, MEETING).unwrap(), Some(remote(MEETING)));
        assert!(cache.save_meeting(ID, &remote("../../x")).is_err());
        assert!(cache.meetings("../x").is_err());

        let uploads = BTreeMap::from([(MEETING.to_string(), "hash".to_string())]);
        cache.save_uploads(ID, &uploads).unwrap();
        cache.rename_folder(ID, KEY).unwrap();
        assert_eq!(cache.uploads(KEY).unwrap(), uploads);
        assert_eq!(cache.meetings(KEY).unwrap().len(), 1);
        cache.remove_meeting(KEY, MEETING).unwrap();
        assert!(cache.meetings(KEY).unwrap().is_empty());
        cache.remove_folder(KEY).unwrap();
        assert!(cache.uploads(KEY).unwrap().is_empty());
    }

    #[test]
    fn names_are_one_trimmed_line() {
        assert_eq!(clean_name("  Acme \n team ", MAX_NAME), "Acme team");
        assert_eq!(clean_name(&"x".repeat(100), MAX_NAME).len(), MAX_NAME);
    }
}
