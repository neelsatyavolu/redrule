//! Connecting and disconnecting the ChatGPT and Grok accounts that write the notes.
use std::sync::Arc;

use tauri_plugin_opener::OpenerExt;

use super::settings::SettingsPatch;
use super::state::App;
use crate::core::api_providers::Provider;
use crate::core::oauth::{ProviderId, parse_callback};
use crate::core::{Error, Result};
use crate::providers::oauth_service::OAuthService;

impl App {
    /// Opens the provider's sign-in page and waits for the browser to redirect back.
    pub fn connect(self: &Arc<Self>, provider: ProviderId) -> Result<()> {
        self.abort_connecting();
        let url = self.oauth.begin(provider);
        self.update(|state| {
            state.connecting = Some(provider);
            state.connection_error = None;
        });
        self.handle.opener().open_url(url, None::<&str>).map_err(|e| Error::message(format!("The browser could not be opened. {e}")))?;
        let app = Arc::clone(self);
        let task = tauri::async_runtime::spawn(async move {
            let result = match OAuthService::await_redirect(provider).await {
                Ok(callback) => app.oauth.complete(provider, callback).await,
                Err(error) => Err(error),
            };
            app.finish_connecting(provider, result.err());
        });
        *self.connect_task.lock().unwrap() = Some(task);
        Ok(())
    }

    /// Fallback when the browser cannot reach the loopback port: the person pastes the code or redirect URL.
    pub fn submit_pasted_code(self: &Arc<Self>, text: String) {
        let Some(provider) = self.read(|state| state.connecting) else { return };
        self.abort_connecting();
        let app = Arc::clone(self);
        let task = tauri::async_runtime::spawn(async move {
            let result = match parse_callback(&text) {
                Ok(callback) => app.oauth.complete(provider, callback).await,
                Err(error) => Err(error),
            };
            match result {
                Ok(()) => app.finish_connecting(provider, None),
                Err(error) => app.update(|state| state.connection_error = Some(error.to_string())),
            }
        });
        *self.connect_task.lock().unwrap() = Some(task);
    }

    pub fn cancel_connecting(&self) {
        self.abort_connecting();
        self.update(|state| {
            state.connecting = None;
            state.connection_error = None;
        });
    }

    pub fn disconnect(&self, provider: ProviderId) {
        if let Err(error) = self.oauth.disconnect(provider) {
            self.update(|state| state.connection_error = Some(error.to_string()));
        }
        self.refresh_connections();
    }

    fn abort_connecting(&self) {
        if let Some(task) = self.connect_task.lock().unwrap().take() {
            task.abort();
        }
    }

    fn finish_connecting(&self, provider: ProviderId, error: Option<Error>) {
        self.update(|state| {
            state.connecting = None;
            state.connection_error = error.as_ref().map(ToString::to_string);
        });
        self.refresh_connections();
        if error.is_none() {
            self.write_notes_with(Provider::Account(provider));
        }
    }

    /// Switches the note writer to `provider` when the chosen model's provider is not set up.
    /// Notes written on this Mac stay that way.
    pub(super) fn write_notes_with(&self, provider: Provider) {
        let (settings, connected) = self.read(|state| (state.settings.clone(), state.connected.clone()));
        if settings.local_note_model().is_some() || connected.contains(&settings.model_choice().provider) {
            return;
        }
        if let Some(choice) = settings.default_choice(provider) {
            self.apply_settings(SettingsPatch { model_choice_id: Some(choice.id()), ..Default::default() });
        }
    }

    pub fn apply_settings(&self, patch: SettingsPatch) {
        self.update(|state| state.settings = state.settings.apply(patch));
    }
}
