//! Saving and removing the API keys that write notes and answer questions.
use super::settings::SettingsPatch;
use super::state::App;
use crate::core::api_providers::{ApiProvider, Provider, validate_base_url};
use crate::core::api_requests::Target;
use crate::core::{Error, Result};
use crate::providers::api_keys;

impl App {
    /// Checks the key with the provider, then keeps it in the Keychain. A custom server also needs its
    /// address and model name; its key is optional.
    pub async fn save_api_key(&self, provider: ApiProvider, key: &str, base_url: &str, model: &str) -> Result<()> {
        let (key, model) = (key.trim(), model.trim());
        let server = provider == ApiProvider::Compatible;
        if key.is_empty() && !server {
            return Err(Error::message("Paste the API key first."));
        }
        let base_url = if server { validate_base_url(base_url)? } else { String::new() };
        if server && model.is_empty() {
            return Err(Error::message("Enter the name of the model the server should run."));
        }
        let target = Target { provider, base_url: &base_url, key: Some(key).filter(|key| !key.is_empty()) };
        api_keys::check(&self.http, target).await?;

        if key.is_empty() {
            self.api_keys.delete(provider)?;
        } else {
            self.api_keys.save(provider, key)?;
        }
        if server {
            self.apply_settings(SettingsPatch {
                compatible_url: Some(base_url),
                compatible_model: Some(model.to_string()),
                ..Default::default()
            });
        }
        self.refresh_connections();
        self.write_notes_with(Provider::Api(provider));
        Ok(())
    }

    pub fn remove_api_key(&self, provider: ApiProvider) -> Result<()> {
        self.api_keys.delete(provider)?;
        if provider == ApiProvider::Compatible {
            self.apply_settings(SettingsPatch {
                compatible_url: Some(String::new()),
                compatible_model: Some(String::new()),
                ..Default::default()
            });
        }
        self.refresh_connections();
        Ok(())
    }
}
