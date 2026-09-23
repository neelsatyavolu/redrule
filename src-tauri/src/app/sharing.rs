//! Publishing and revoking read-only links to a meeting's notes.
use std::sync::Arc;

use tauri_plugin_clipboard_manager::ClipboardExt;

use super::state::App;
use crate::core::models::MeetingShare;
use crate::core::{sharing, transcript};
use crate::core::{Error, Result};
use crate::providers::{keychain, share_client};

impl App {
    /// Creates or updates the link, copies it, and returns it.
    pub async fn publish_share(self: &Arc<Self>, id: &str, include_transcript: bool) -> Result<String> {
        let _busy = self.begin_sharing()?;
        let store = self.store()?;
        let note = store.note(id)?.ok_or_else(|| Error::message("This meeting has no notes to share yet."))?;
        let (share_id, key) = match store.share(id)? {
            Some(share) => {
                let key = owner_key(&share.id)?;
                (share.id, key)
            }
            None => {
                let (share_id, key) = sharing::new_share();
                keychain::save_share_key(&share_id, &key)?;
                (share_id, key)
            }
        };
        let share = MeetingShare { url: share_client::share_url(&share_id), id: share_id, includes_transcript: include_transcript };
        // Keep the handle and key even if the connection drops after a successful upload, so it can still be revoked.
        store.save_share(&share, id)?;
        let segments = transcript::merge(&store.transcript(id)?);
        share_client::publish(&self.http, &share, &key, &note, &segments).await?;
        let _ = self.handle.clipboard().write_text(share.url.clone());
        self.update(|state| state.revision += 1);
        Ok(share.url)
    }

    pub async fn revoke_share(self: &Arc<Self>, id: &str) -> Result<()> {
        let _busy = self.begin_sharing()?;
        let store = self.store()?;
        if let Some(share) = store.share(id)? {
            share_client::revoke(&self.http, &share, &owner_key(&share.id)?).await?;
            // A key left behind only controls a link that no longer exists.
            if let Err(error) = keychain::delete_share_key(&share.id) {
                log::warn!("The key of a removed link could not be deleted. {error}");
            }
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

/// The key that controls a link.
fn owner_key(share_id: &str) -> Result<String> {
    choose_key(keychain::share_key(share_id)?, keychain::sharing_key)
}

/// A link's own owner key or, for a link made before owner keys, the legacy service key.
fn choose_key(own: Option<String>, legacy: impl FnOnce() -> Result<Option<String>>) -> Result<String> {
    if let Some(key) = own {
        return Ok(key);
    }
    legacy()?.ok_or_else(|| {
        Error::message("This link was made by an earlier version of Redrule with a sharing key this Mac doesn't have, so it can't be changed from here.")
    })
}

/// Clears the busy flag however the sharing call ends.
struct SharingGuard(Arc<App>);

impl Drop for SharingGuard {
    fn drop(&mut self) {
        self.0.update(|state| state.sharing_busy = false);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_link_uses_its_own_key_before_the_legacy_one() {
        let never = || -> Result<Option<String>> { panic!("the legacy key is read only for old links") };
        assert_eq!(choose_key(Some("own".into()), never).unwrap(), "own");
        assert_eq!(choose_key(None, || Ok(Some("legacy".into()))).unwrap(), "legacy");
        assert!(choose_key(None, || Ok(None)).unwrap_err().to_string().contains("earlier version"));
    }
}
