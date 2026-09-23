//! File-backed storage: one folder per meeting holding `meeting.json`, `transcript.json`,
//! `note.json`, `notes.md` and optionally `share.json`. Same layout as the Swift app.
use std::fs;
use std::path::{Path, PathBuf};

use serde::Serialize;
use serde::de::DeserializeOwned;

use super::error::{Error, Result};
use super::models::{Meeting, MeetingNote, MeetingShare, MeetingStatus, TranscriptSegment};

#[derive(Debug, Clone)]
pub struct MeetingStore {
    root: PathBuf,
}

impl MeetingStore {
    pub fn new(root: PathBuf) -> Result<Self> {
        fs::create_dir_all(&root)?;
        Ok(Self { root })
    }

    /// `~/Library/Application Support/Redrule/meetings`
    pub fn default_root() -> Result<PathBuf> {
        Ok(support_folder()?.join("meetings"))
    }

    /// Meeting ids come from the webview, so only UUIDs may name a folder.
    pub fn folder(&self, id: &str) -> Result<PathBuf> {
        uuid::Uuid::parse_str(id).map_err(|_| Error::message("That meeting does not exist."))?;
        Ok(self.root.join(id))
    }

    /// All readable meetings, newest first. Folders that fail to decode are skipped.
    pub fn list(&self) -> Result<Vec<Meeting>> {
        let mut meetings: Vec<Meeting> = fs::read_dir(&self.root)?
            .filter_map(|entry| read::<Meeting>(&entry.ok()?.path().join("meeting.json")).ok())
            .collect();
        meetings.sort_by_key(|m| std::cmp::Reverse(m.started_at));
        Ok(meetings)
    }

    pub fn meeting(&self, id: &str) -> Result<Meeting> {
        read(&self.folder(id)?.join("meeting.json"))
    }

    pub fn save(&self, meeting: &Meeting) -> Result<()> {
        let folder = self.folder(&meeting.id)?;
        fs::create_dir_all(&folder)?;
        write(&folder.join("meeting.json"), meeting)
    }

    pub fn transcript(&self, id: &str) -> Result<Vec<TranscriptSegment>> {
        read_optional(&self.folder(id)?.join("transcript.json")).map(Option::unwrap_or_default)
    }

    pub fn save_transcript(&self, segments: &[TranscriptSegment], id: &str) -> Result<()> {
        write(&self.folder(id)?.join("transcript.json"), &segments)
    }

    pub fn note(&self, id: &str) -> Result<Option<MeetingNote>> {
        read_optional(&self.folder(id)?.join("note.json"))
    }

    pub fn save_note(&self, note: &MeetingNote, id: &str) -> Result<()> {
        let folder = self.folder(id)?;
        write(&folder.join("note.json"), note)?;
        write_bytes(&folder.join("notes.md"), note.markdown().as_bytes())
    }

    pub fn delete(&self, id: &str) -> Result<()> {
        Ok(fs::remove_dir_all(self.folder(id)?)?)
    }

    pub fn rename(&self, meeting: &Meeting, title: &str) -> Result<()> {
        let title = title.trim();
        if title.is_empty() {
            return Ok(());
        }
        if let Some(note) = self.note(&meeting.id)? {
            self.save_note(&MeetingNote { title: title.to_string(), ..note }, &meeting.id)?;
        }
        self.save(&meeting.titled(title))
    }

    /// Meetings left mid-flight by a crash or quit can never finish; mark them so the UI offers a retry.
    pub fn recover_interrupted(&self) -> Result<()> {
        for meeting in self.list()? {
            if meeting.status != MeetingStatus::Done && meeting.status != MeetingStatus::Failed {
                let ended = Meeting { ended_at: meeting.ended_at.or(Some(meeting.started_at)), ..meeting };
                self.save(&ended.failed("Redrule quit before this meeting was finished."))?;
            }
        }
        Ok(())
    }

    pub fn share(&self, id: &str) -> Result<Option<MeetingShare>> {
        read_optional(&self.folder(id)?.join("share.json"))
    }

    pub fn save_share(&self, share: &MeetingShare, id: &str) -> Result<()> {
        write(&self.folder(id)?.join("share.json"), share)
    }

    pub fn remove_share(&self, id: &str) -> Result<()> {
        let file = self.folder(id)?.join("share.json");
        if file.exists() {
            fs::remove_file(file)?;
        }
        Ok(())
    }

    /// Names one speaker throughout a meeting. A blank name restores the original label.
    pub fn rename_speaker(&self, key: &str, name: &str, id: &str) -> Result<()> {
        let name: String = name.trim().chars().take(100).collect();
        let segments: Vec<TranscriptSegment> = self
            .transcript(id)?
            .into_iter()
            .map(|segment| {
                if segment.speaker_key() != key {
                    return segment;
                }
                TranscriptSegment { speaker_name: (!name.is_empty()).then(|| name.clone()), ..segment }
            })
            .collect();
        self.save_transcript(&segments, id)
    }
}

/// `~/Library/Application Support/Redrule`, holding meetings and the speech models.
pub fn support_folder() -> Result<PathBuf> {
    let support = dirs::data_dir().ok_or_else(|| Error::message("The Application Support folder was not found."))?;
    adopt_legacy_folder(&support.join("Minutes"), &support.join("Redrule"))
}

/// The app was called Minutes. Its folder is renamed once, so meetings and downloaded models carry over.
fn adopt_legacy_folder(legacy: &Path, current: &Path) -> Result<PathBuf> {
    if !current.exists() && legacy.is_dir() {
        fs::rename(legacy, current)?;
    }
    Ok(current.to_path_buf())
}

fn read<T: DeserializeOwned>(path: &Path) -> Result<T> {
    Ok(serde_json::from_slice(&fs::read(path)?)?)
}

fn read_optional<T: DeserializeOwned>(path: &Path) -> Result<Option<T>> {
    if !path.exists() {
        return Ok(None);
    }
    read(path).map(Some)
}

fn write<T: Serialize + ?Sized>(path: &Path, value: &T) -> Result<()> {
    write_bytes(path, &serde_json::to_vec_pretty(value)?)
}

/// Writes through a temporary file so a crash never leaves a half-written file behind.
fn write_bytes(path: &Path, bytes: &[u8]) -> Result<()> {
    let temporary = path.with_extension("tmp");
    fs::write(&temporary, bytes)?;
    fs::rename(&temporary, path)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use chrono::Utc;

    use super::*;
    use crate::models::{MeetingApp, Speaker};

    const ID: &str = "0B5F1C9E-6C8B-4A1E-9F1D-2C3B4A5D6E7F";

    fn store() -> (tempfile::TempDir, MeetingStore) {
        let dir = tempfile::tempdir().unwrap();
        let store = MeetingStore::new(dir.path().join("meetings")).unwrap();
        (dir, store)
    }

    fn meeting(id: &str, status: MeetingStatus) -> Meeting {
        Meeting {
            id: id.into(),
            title: "Standup".into(),
            app: MeetingApp::Zoom,
            started_at: Utc::now(),
            ended_at: None,
            status,
            error_message: None,
            archived_at: None,
        }
    }

    #[test]
    fn saves_and_lists_newest_first() {
        let (_dir, store) = store();
        let older = Meeting { started_at: Utc::now() - chrono::Duration::hours(1), ..meeting(ID, MeetingStatus::Done) };
        let newer = meeting("1B5F1C9E-6C8B-4A1E-9F1D-2C3B4A5D6E7F", MeetingStatus::Done);
        store.save(&older).unwrap();
        store.save(&newer).unwrap();
        let ids: Vec<_> = store.list().unwrap().into_iter().map(|m| m.id).collect();
        assert_eq!(ids, [newer.id, older.id]);
    }

    #[test]
    fn adopts_the_minutes_folder_once() {
        let dir = tempfile::tempdir().unwrap();
        let (legacy, current) = (dir.path().join("Minutes"), dir.path().join("Redrule"));
        fs::create_dir_all(legacy.join("meetings")).unwrap();
        assert_eq!(adopt_legacy_folder(&legacy, &current).unwrap(), current);
        assert!(current.join("meetings").is_dir());
        assert!(!legacy.exists());

        // A later Minutes folder never replaces the adopted one.
        fs::create_dir_all(legacy.join("other")).unwrap();
        adopt_legacy_folder(&legacy, &current).unwrap();
        assert!(legacy.join("other").is_dir());
        assert!(!current.join("other").exists());
    }

    #[test]
    fn rejects_ids_that_are_not_uuids() {
        let (_dir, store) = store();
        assert!(store.folder("../../etc").is_err());
        assert!(store.delete("..").is_err());
    }

    #[test]
    fn notes_are_written_with_markdown() {
        let (_dir, store) = store();
        store.save(&meeting(ID, MeetingStatus::Done)).unwrap();
        let note = MeetingNote { title: "T".into(), tldr: "S".into(), sections: vec![], decisions: vec![], action_items: vec![] };
        store.save_note(&note, ID).unwrap();
        assert_eq!(store.note(ID).unwrap(), Some(note));
        assert_eq!(fs::read_to_string(store.folder(ID).unwrap().join("notes.md")).unwrap(), "# T\n\nS\n");
    }

    #[test]
    fn rename_updates_meeting_and_note_titles() {
        let (_dir, store) = store();
        let saved = meeting(ID, MeetingStatus::Done);
        store.save(&saved).unwrap();
        let note = MeetingNote { title: "Old".into(), tldr: "".into(), sections: vec![], decisions: vec![], action_items: vec![] };
        store.save_note(&note, ID).unwrap();
        store.rename(&saved, "  New title ").unwrap();
        assert_eq!(store.meeting(ID).unwrap().title, "New title");
        assert_eq!(store.note(ID).unwrap().unwrap().title, "New title");
    }

    #[test]
    fn interrupted_meetings_are_marked_failed() {
        let (_dir, store) = store();
        store.save(&meeting(ID, MeetingStatus::Recording)).unwrap();
        store.recover_interrupted().unwrap();
        let recovered = store.meeting(ID).unwrap();
        assert_eq!(recovered.status, MeetingStatus::Failed);
        assert!(recovered.ended_at.is_some());
    }

    #[test]
    fn renames_and_restores_a_speaker() {
        let (_dir, store) = store();
        store.save(&meeting(ID, MeetingStatus::Done)).unwrap();
        let segment = TranscriptSegment { speaker_id: Some("1".into()), ..TranscriptSegment::new(Speaker::Them, 0.0, 1.0, "hi") };
        store.save_transcript(&[segment.clone(), TranscriptSegment::new(Speaker::Me, 2.0, 3.0, "yo")], ID).unwrap();
        store.rename_speaker("them:1", " Ada ", ID).unwrap();
        let named = store.transcript(ID).unwrap();
        assert_eq!(named[0].speaker_name.as_deref(), Some("Ada"));
        assert_eq!(named[1].speaker_name, None);
        store.rename_speaker("them:1", "", ID).unwrap();
        assert_eq!(store.transcript(ID).unwrap()[0].speaker_name, None);
    }

    #[test]
    fn shares_round_trip_and_can_be_removed() {
        let (_dir, store) = store();
        store.save(&meeting(ID, MeetingStatus::Done)).unwrap();
        let share = MeetingShare { id: "abc".into(), includes_transcript: true, url: "https://x/s/abc".into() };
        store.save_share(&share, ID).unwrap();
        assert_eq!(store.share(ID).unwrap(), Some(share));
        store.remove_share(ID).unwrap();
        assert_eq!(store.share(ID).unwrap(), None);
    }
}
