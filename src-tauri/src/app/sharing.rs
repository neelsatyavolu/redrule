//! Publishing and revoking read-only links to a meeting's notes.
use std::sync::Arc;

use tauri_plugin_clipboard_manager::ClipboardExt;

use super::state::App;
use crate::core::models::MeetingShare;
use crate::core::transcript;
use crate::core::{Error, Result};
use crate::providers::share_client;

impl App {
    /// Creates or updates the link, copies it, and returns it.
    pub async fn publish_share(self: &Arc<Self>, id: &str, include_transcript: bool) -> Result<String> {
        let _busy = self.begin_sharing()?;
        let store = self.store()?;
        let note = store.note(id)?.ok_or_else(|| Error::message("This meeting has no notes to share yet."))?;
        let existing = store.share(id)?;
        let share_id = existing.map(|share| share.id).unwrap_or_else(share_client::new_share_id);
        let share = MeetingShare { url: share_client::share_url(&share_id), id: share_id, includes_transcript: include_transcript };
        // Keep the handle even if the connection drops after a successful upload, so it can still be revoked.
        store.save_share(&share, id)?;
        let segments = transcript::merge(&store.transcript(id)?);
        share_client::publish(&self.http, &share, &note, &segments).await?;
        let _ = self.handle.clipboard().write_text(share.url.clone());
        self.update(|state| state.revision += 1);
        Ok(share.url)
    }

    pub async fn revoke_share(self: &Arc<Self>, id: &str) -> Result<()> {
        let _busy = self.begin_sharing()?;
        let store = self.store()?;
        if let Some(share) = store.share(id)? {
            share_client::revoke(&self.http, &share).await?;
        }
        store.remove_share(id)?;
        self.update(|state| state.revision += 1);
        Ok(())
    }

    fn begin_sharing(self: &Arc<Self>) -> Result<SharingGuard> {
        let started = self.update(|state| !std::mem::replace(&mut state.sharing_busy, true));
        if !started {
            return Err(Error::message("Sharing is already in progress."));
        }
        Ok(SharingGuard(Arc::clone(self)))
    }
}

/// Clears the busy flag however the sharing call ends.
struct SharingGuard(Arc<App>);

impl Drop for SharingGuard {
    fn drop(&mut self) {
        self.0.update(|state| state.sharing_busy = false);
    }
}
