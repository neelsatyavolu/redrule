//! Keeping shared folders in sync: uploading this Mac's meetings and caching everyone else's.
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use std::time::Duration;

use super::folders::{FolderInfo, FolderMeeting};
use super::state::App;
use crate::core::folders::{JoinedFolder, MAX_NAME, RemoteMeeting, SharedMeeting, clean_name, device_secret, edit_key, valid_meeting_id};
use crate::core::models::{Meeting, MeetingStatus};
use crate::core::transcript;
use crate::core::{Error, Result};
use crate::providers::folder_client::{Auth, Listing};

const SYNC_INTERVAL: Duration = Duration::from_secs(60);
/// Edits often come in bursts, like ticking off several action items.
const NUDGE_DELAY: Duration = Duration::from_secs(2);

impl App {
    /// Shows the folders and cached meetings, then syncs every minute and soon after local changes.
    pub fn start_folder_sync(self: &Arc<Self>) {
        self.publish_folders();
        let app = Arc::clone(self);
        tauri::async_runtime::spawn(async move {
            loop {
                app.sync_folders().await;
                tokio::select! {
                    _ = tokio::time::sleep(SYNC_INTERVAL) => {}
                    _ = app.folders.nudge.notified() => tokio::time::sleep(NUDGE_DELAY).await,
                }
            }
        });
    }

    async fn sync_folders(&self) {
        // Joining, leaving, resetting and deleting wait for this, so the folder list can't change underneath.
        let _one_at_a_time = self.folders.busy.lock().await;
        let registry = self.folders.registry();
        if registry.folders.is_empty() {
            return;
        }
        let secret = match device_secret(&self.folders.device_key_path) {
            Ok(secret) => secret,
            Err(error) => return log::warn!("Shared folders can't sync without this Mac's key. {error}"),
        };
        // Read fresh from disk: if the list can't be read, nothing is taken for deleted.
        let stored = self.store().and_then(|store| store.list()).map_err(|e| log::warn!("Meetings could not be listed. {e}")).ok();
        for folder in &registry.folders {
            match self.sync_folder(folder, &secret, &registry.display_name, stored.as_deref()).await {
                Ok(available) => self.folders.set_unavailable(&folder.id, !available),
                // Offline or the service is down: keep showing what was cached and try again later.
                Err(error) => log::warn!("A shared folder did not sync. {error}"),
            }
        }
        self.publish_folders();
    }

    /// Returns false when the folder's link no longer works.
    async fn sync_folder(&self, folder: &JoinedFolder, secret: &str, recorded_by: &str, stored: Option<&[Meeting]>) -> Result<bool> {
        let Some(listing) = self.client().listing(&folder.id, &folder.member_key).await? else { return Ok(false) };
        if listing.name != folder.name {
            let renamed = JoinedFolder { name: clean_name(&listing.name, MAX_NAME), ..folder.clone() };
            self.folders.save(self.folders.registry().with_folder(renamed))?;
        }
        if let Some(stored) = stored {
            self.upload_changes(folder, secret, recorded_by, stored).await?;
        }
        let local: HashSet<String> = match stored {
            Some(stored) => stored.iter().map(|m| m.id.clone()).collect(),
            None => self.read(|state| state.meetings.iter().map(|m| m.id.clone()).collect()),
        };
        self.download_changes(folder, &listing, &local).await?;
        Ok(true)
    }

    /// Uploads this Mac's finished meetings that changed, and removes the ones that left the folder.
    /// Each success is recorded at once, so an interruption never loses track of what is online.
    async fn upload_changes(&self, folder: &JoinedFolder, secret: &str, recorded_by: &str, stored: &[Meeting]) -> Result<()> {
        let cache = &self.folders.cache;
        let mut uploads = cache.uploads(&folder.id)?;
        let mine: Vec<&Meeting> = stored.iter().filter(|m| m.folder_id.as_deref() == Some(folder.id.as_str())).collect();
        for meeting in mine.iter().filter(|m| m.status == MeetingStatus::Done) {
            let shared = match self.shared_copy(meeting, recorded_by) {
                Ok(Some(shared)) => shared,
                Ok(None) => continue,
                Err(error) => {
                    self.report_upload_failure(&meeting.id, &meeting.title, &error);
                    continue;
                }
            };
            let hash = shared.content_hash()?;
            if uploads.get(&meeting.id) == Some(&hash) {
                continue;
            }
            let key = edit_key(secret, &meeting.id);
            let auth = Auth::Editor { member_key: &folder.member_key, edit_key: &key };
            match self.client().upload(&folder.id, &meeting.id, auth, &shared).await {
                Ok(()) => {
                    uploads.insert(meeting.id.clone(), hash);
                    cache.save_uploads(&folder.id, &uploads)?;
                }
                Err(error) => self.report_upload_failure(&format!("{}:{hash}", meeting.id), &meeting.title, &error),
            }
        }
        let leaving: Vec<String> = uploads.keys().filter(|id| self.left_folder(id, &folder.id, stored)).cloned().collect();
        for id in leaving {
            let key = edit_key(secret, &id);
            let auth = Auth::Editor { member_key: &folder.member_key, edit_key: &key };
            match self.client().remove(&folder.id, &id, auth).await {
                Ok(()) => {
                    uploads.remove(&id);
                    cache.save_uploads(&folder.id, &uploads)?;
                }
                Err(error) => log::warn!("A meeting could not be removed from a shared folder. {error}"),
            }
        }
        Ok(())
    }

    /// The note and transcript as uploaded, or None while the meeting has no notes.
    fn shared_copy(&self, meeting: &Meeting, recorded_by: &str) -> Result<Option<SharedMeeting>> {
        let store = self.store()?;
        let Some(note) = store.note(&meeting.id)? else { return Ok(None) };
        let segments = transcript::merge(&store.transcript(&meeting.id)?);
        Ok(Some(SharedMeeting::new(meeting, note, segments, recorded_by)))
    }

    /// An uploaded meeting leaves the folder only when it is deleted from this Mac or moved elsewhere.
    /// One whose files can't be read right now stays.
    fn left_folder(&self, id: &str, folder_id: &str, stored: &[Meeting]) -> bool {
        match stored.iter().find(|m| m.id == id) {
            Some(meeting) => meeting.folder_id.as_deref() != Some(folder_id),
            None => self.store().and_then(|store| store.folder(id)).map(|dir| !dir.exists()).unwrap_or(true),
        }
    }

    fn report_upload_failure(&self, attempt: &str, title: &str, error: &Error) {
        if self.folders.reported.lock().unwrap_or_else(|p| p.into_inner()).insert(attempt.to_string()) {
            self.report(format!("“{title}” could not be added to its shared folder. {error}"));
        }
    }

    /// Downloads other people's new or changed meetings and drops the ones that were removed.
    /// A meeting that fails to download is skipped, so one bad upload can't stall the folder.
    async fn download_changes(&self, folder: &JoinedFolder, listing: &Listing, local: &HashSet<String>) -> Result<()> {
        let cache = &self.folders.cache;
        let cached: HashMap<String, String> = cache.meetings(&folder.id)?.into_iter().map(|m| (m.id, m.updated_at)).collect();
        let theirs: Vec<_> = listing.meetings.iter().filter(|e| valid_meeting_id(&e.id) && !local.contains(&e.id)).collect();
        for entry in &theirs {
            if cached.get(&entry.id) == Some(&entry.updated_at) {
                continue;
            }
            match self.client().meeting(&folder.id, &entry.id).await {
                // The listing's timestamp is what the next sync compares against.
                Ok(Some(meeting)) => {
                    cache.save_meeting(&folder.id, &RemoteMeeting { id: entry.id.clone(), updated_at: entry.updated_at.clone(), ..meeting })?
                }
                Ok(None) => {}
                Err(error) => log::warn!("A shared meeting could not be downloaded. {error}"),
            }
        }
        for id in cached.keys().filter(|id| !theirs.iter().any(|e| &e.id == *id)) {
            cache.remove_meeting(&folder.id, id)?;
        }
        Ok(())
    }

    /// Publishes the folder list and other people's cached meetings.
    pub(super) fn publish_folders(&self) {
        let registry = self.folders.registry();
        let local: HashSet<String> = self.read(|state| state.meetings.iter().map(|m| m.id.clone()).collect());
        let folders: Vec<FolderInfo> = registry
            .folders
            .iter()
            .map(|f| FolderInfo {
                id: f.id.clone(),
                name: f.name.clone(),
                owner: f.owner_key.is_some(),
                unavailable: self.folders.is_unavailable(&f.id),
            })
            .collect();
        let meetings: Vec<FolderMeeting> = registry
            .folders
            .iter()
            .flat_map(|f| {
                let cached = self.folders.cache.meetings(&f.id).unwrap_or_default();
                cached.into_iter().filter(|m| !local.contains(&m.id)).map(|m| FolderMeeting {
                    meeting: m.listed(&f.id),
                    recorded_by: m.meeting.recorded_by.clone(),
                    remote: true,
                    updated_at: m.updated_at,
                })
            })
            .collect();
        let changed = self.read(|state| {
            state.folders != folders || state.folder_meetings != meetings || state.display_name != registry.display_name
        });
        if changed {
            self.update(|state| {
                state.folders = folders;
                state.folder_meetings = meetings;
                state.display_name = registry.display_name;
                state.revision += 1;
            });
        }
    }
}
