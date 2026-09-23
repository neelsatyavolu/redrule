//! Shared folders: creating, joining and leaving them. Syncing is in `folder_sync.rs`.
use std::collections::{BTreeSet, HashSet};
use std::path::PathBuf;
use std::sync::Mutex;

use serde::Serialize;
use tauri_plugin_clipboard_manager::ClipboardExt;
use tokio::sync::Notify;

use super::state::App;
use crate::core::folders::{
    FolderCache, FolderRegistry, JoinedFolder, MAX_DISPLAY_NAME, MAX_NAME, RemoteMeeting, clean_name, link, parse_link, valid_key,
};
use crate::core::models::Meeting;
use crate::core::{Error, Result};
use crate::providers::folder_client::FolderClient;
use crate::providers::share_client::BASE_URL;

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FolderInfo {
    pub id: String,
    pub name: String,
    pub owner: bool,
    /// The link stopped working: the folder was reset or deleted by its owner.
    pub unavailable: bool,
}

/// Someone else's meeting, listed read-only.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FolderMeeting {
    #[serde(flatten)]
    pub meeting: Meeting,
    pub recorded_by: String,
    pub remote: bool,
    /// Compared so an edited note refreshes open views; not sent.
    #[serde(skip)]
    pub updated_at: String,
}

/// Folder bookkeeping that lives beside `State`.
pub struct Folders {
    registry_path: PathBuf,
    pub(super) device_key_path: PathBuf,
    pub(super) cache: FolderCache,
    registry: Mutex<FolderRegistry>,
    pub(super) unavailable: Mutex<BTreeSet<String>>,
    /// Upload failures already shown, so a retry every minute doesn't repeat them.
    pub(super) reported: Mutex<HashSet<String>>,
    pub(super) nudge: Notify,
    /// Held by each sync and each change to the folder list, so they never interleave.
    pub(super) busy: tokio::sync::Mutex<()>,
}

impl Folders {
    pub fn new(support: PathBuf) -> Self {
        let registry_path = support.join("folders.json");
        let registry = FolderRegistry::load(&registry_path).unwrap_or_else(|error| {
            log::warn!("Shared folders could not be read. {error}");
            FolderRegistry::default()
        });
        Self {
            device_key_path: support.join("device.key"),
            cache: FolderCache::new(support.join("folders")),
            registry_path,
            registry: Mutex::new(registry),
            unavailable: Mutex::new(BTreeSet::new()),
            reported: Mutex::new(HashSet::new()),
            nudge: Notify::new(),
            busy: tokio::sync::Mutex::new(()),
        }
    }

    pub fn registry(&self) -> FolderRegistry {
        self.registry.lock().unwrap_or_else(|p| p.into_inner()).clone()
    }

    /// A joined folder whose link still works, so meetings can be recorded into it.
    pub fn accepts(&self, id: &str) -> bool {
        self.registry().folder(id).is_some() && !self.is_unavailable(id)
    }

    /// Asks for a sync soon, after local meetings change.
    pub fn nudge(&self) {
        self.nudge.notify_one();
    }

    pub(super) fn save(&self, next: FolderRegistry) -> Result<()> {
        next.save(&self.registry_path).map_err(|e| e.context("The shared folder list could not be saved."))?;
        *self.registry.lock().unwrap_or_else(|p| p.into_inner()) = next;
        Ok(())
    }

    pub(super) fn is_unavailable(&self, id: &str) -> bool {
        self.unavailable.lock().unwrap_or_else(|p| p.into_inner()).contains(id)
    }

    pub(super) fn set_unavailable(&self, id: &str, unavailable: bool) {
        let mut set = self.unavailable.lock().unwrap_or_else(|p| p.into_inner());
        if unavailable { set.insert(id.to_string()) } else { set.remove(id) };
    }

    fn joined(&self, id: &str) -> Result<JoinedFolder> {
        self.registry().folder(id).cloned().ok_or_else(|| Error::message("That folder isn't on this Mac any more."))
    }

    fn owned(&self, id: &str) -> Result<(JoinedFolder, String)> {
        let folder = self.joined(id)?;
        let key = folder.owner_key.clone().ok_or_else(|| Error::message("Only the person who created this folder can do that."))?;
        Ok((folder, key))
    }
}

fn display_name(name: &str) -> Result<String> {
    let name = clean_name(name, MAX_DISPLAY_NAME);
    if name.is_empty() {
        return Err(Error::message("Enter your name, so others know who recorded each meeting."));
    }
    Ok(name)
}

fn folder_name(name: &str) -> Result<String> {
    let name = clean_name(name, MAX_NAME);
    if name.is_empty() {
        return Err(Error::message("Give the folder a name."));
    }
    Ok(name)
}

/// Ids and keys from the service become file names and credentials, so they are checked first.
fn checked(values: &[&str]) -> Result<()> {
    if values.iter().all(|v| valid_key(v)) {
        return Ok(());
    }
    Err(Error::message("The shared folder service sent an unexpected reply. Try again."))
}

impl App {
    pub(super) fn client(&self) -> FolderClient<'_> {
        FolderClient { http: &self.http }
    }

    /// Creates a folder, copies its link and returns its id.
    pub async fn create_folder(&self, name: &str, your_name: &str) -> Result<String> {
        let (name, your_name) = (folder_name(name)?, display_name(your_name)?);
        let _one_at_a_time = self.folders.busy.lock().await;
        let created = self.client().create(&name).await?;
        checked(&[&created.id, &created.member_key, &created.owner_key])?;
        let folder = JoinedFolder { id: created.id, name, member_key: created.member_key, owner_key: Some(created.owner_key) };
        let (id, url) = (folder.id.clone(), link(BASE_URL, &folder.id, &folder.member_key));
        self.folders.save(FolderRegistry { display_name: your_name, ..self.folders.registry() }.with_folder(folder))?;
        self.publish_folders();
        let _ = self.handle.clipboard().write_text(url);
        Ok(id)
    }

    /// Joins the folder behind a pasted link and returns its id.
    pub async fn join_folder(&self, text: &str, your_name: &str) -> Result<String> {
        let (id, member_key) = parse_link(text)?;
        let your_name = display_name(your_name)?;
        let _one_at_a_time = self.folders.busy.lock().await;
        let listing = self
            .client()
            .listing(&id, &member_key)
            .await?
            .ok_or_else(|| Error::message("This folder link doesn't work. It may have been reset or deleted; ask for a new one."))?;
        if !listing.member {
            return Err(Error::message("This link can't add meetings. Ask for the full link, including the part after #."));
        }
        // Rejoining keeps the owner key of a folder this Mac created.
        let owner_key = self.folders.registry().folder(&id).and_then(|f| f.owner_key.clone());
        let folder = JoinedFolder { id: id.clone(), name: clean_name(&listing.name, MAX_NAME), member_key, owner_key };
        self.folders.save(FolderRegistry { display_name: your_name, ..self.folders.registry() }.with_folder(folder))?;
        self.folders.set_unavailable(&id, false);
        self.adopt_meetings(&id, listing.meetings.iter().map(|e| e.id.as_str()).collect())?;
        self.publish_folders();
        self.folders.nudge();
        Ok(id)
    }

    /// This Mac's meetings already in a folder it joins, such as after the owner reset the link,
    /// point at it again unless they now belong to another working folder.
    fn adopt_meetings(&self, id: &str, listed: HashSet<&str>) -> Result<()> {
        let store = self.store()?;
        let adopting: Vec<Meeting> = self
            .read(|state| state.meetings.clone())
            .into_iter()
            .filter(|m| listed.contains(m.id.as_str()))
            .filter(|m| m.folder_id.as_deref().is_none_or(|other| other != id && !self.folders.accepts(other)))
            .collect();
        for meeting in &adopting {
            store.save(&meeting.in_folder(Some(id.to_string())))?;
        }
        if !adopting.is_empty() {
            self.reload_meetings();
        }
        Ok(())
    }

    pub async fn rename_folder(&self, id: &str, name: &str) -> Result<()> {
        let name = folder_name(name)?;
        let _one_at_a_time = self.folders.busy.lock().await;
        let (folder, owner_key) = self.folders.owned(id)?;
        self.client().rename(id, &owner_key, &name).await?;
        self.folders.save(self.folders.registry().with_folder(JoinedFolder { name, ..folder }))?;
        self.publish_folders();
        Ok(())
    }

    /// Moves the folder to a new link, so the old one stops working for everyone. Copies the link and returns the new id.
    pub async fn reset_folder_link(&self, id: &str) -> Result<String> {
        let _one_at_a_time = self.folders.busy.lock().await;
        let (folder, owner_key) = self.folders.owned(id)?;
        self.ensure_settled(id)?;
        let reset = self.client().reset(id, &owner_key).await?;
        checked(&[&reset.id, &reset.member_key])?;
        let moved = JoinedFolder { id: reset.id.clone(), member_key: reset.member_key, ..folder };
        let url = link(BASE_URL, &moved.id, &moved.member_key);
        self.folders.save(self.folders.registry().without(id).with_folder(moved))?;
        self.folders.cache.rename_folder(id, &reset.id)?;
        self.move_meetings(id, Some(reset.id.clone()))?;
        self.publish_folders();
        let _ = self.handle.clipboard().write_text(url);
        Ok(reset.id)
    }

    /// Deletes the folder for everyone. Meetings recorded on this Mac stay here.
    pub async fn delete_folder(&self, id: &str) -> Result<()> {
        let _one_at_a_time = self.folders.busy.lock().await;
        let (_, owner_key) = self.folders.owned(id)?;
        self.ensure_settled(id)?;
        self.client().delete(id, &owner_key).await?;
        self.forget_folder(id)
    }

    /// Stops following a folder. Meetings already added from this Mac stay in it for the others.
    pub async fn leave_folder(&self, id: &str) -> Result<()> {
        let _one_at_a_time = self.folders.busy.lock().await;
        self.folders.joined(id)?;
        self.ensure_settled(id)?;
        self.forget_folder(id)
    }

    /// A meeting still being recorded or written saves its own copy later, which would undo a move.
    fn ensure_settled(&self, id: &str) -> Result<()> {
        let meetings = self.read(|state| state.meetings.clone());
        let unsettled = meetings.iter().any(|m| m.folder_id.as_deref() == Some(id) && !self.can_edit(m));
        if unsettled {
            return Err(Error::message("A meeting in this folder is still being recorded or written up. Try again when it's done."));
        }
        Ok(())
    }

    fn forget_folder(&self, id: &str) -> Result<()> {
        self.folders.save(self.folders.registry().without(id))?;
        self.folders.set_unavailable(id, false);
        self.folders.cache.remove_folder(id)?;
        self.move_meetings(id, None)?;
        self.publish_folders();
        Ok(())
    }

    /// Points this Mac's meetings in folder `from` at `to`.
    fn move_meetings(&self, from: &str, to: Option<String>) -> Result<()> {
        let store = self.store()?;
        let moving = self.read(|state| state.meetings.iter().filter(|m| m.folder_id.as_deref() == Some(from)).cloned().collect::<Vec<_>>());
        for meeting in moving {
            store.save(&meeting.in_folder(to.clone()))?;
        }
        self.reload_meetings();
        Ok(())
    }

    pub fn copy_folder_link(&self, id: &str) -> Result<()> {
        let folder = self.folders.joined(id)?;
        self.handle
            .clipboard()
            .write_text(link(BASE_URL, &folder.id, &folder.member_key))
            .map_err(|e| Error::message(format!("The link could not be copied. {e}")))
    }

    /// Puts one of this Mac's meetings in a folder, or takes it out with None.
    pub fn set_meeting_folder(&self, id: &str, folder_id: Option<String>) -> Result<()> {
        let meeting = self.editable(id)?;
        if let Some(folder_id) = &folder_id
            && !self.folders.accepts(folder_id)
        {
            return Err(Error::message("That folder's link no longer works, so meetings can't be added to it."));
        }
        self.persist(&meeting.in_folder(folder_id));
        Ok(())
    }

    /// Someone else's meeting, from the folder cache.
    pub fn folder_meeting(&self, id: &str) -> Option<RemoteMeeting> {
        let registry = self.folders.registry();
        registry.folders.iter().find_map(|folder| self.folders.cache.meeting(&folder.id, id).ok().flatten())
    }
}
