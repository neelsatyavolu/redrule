//! OAuth PKCE sign-in, token storage and refresh for Codex and Grok.
use std::collections::HashMap;
use std::sync::Mutex;
use std::time::Duration;

use chrono::Utc;

use super::{NetResult, keychain};
use super::loopback;
use crate::core::oauth::{OAuthCallback, OAuthConfig, Pkce, ProviderId, TokenBundle};
use crate::core::{Error, Result};

const REDIRECT_TIMEOUT: Duration = Duration::from_secs(300);
const REQUEST_TIMEOUT: Duration = Duration::from_secs(30);

pub struct OAuthService {
    http: reqwest::Client,
    pending: Mutex<HashMap<ProviderId, Pkce>>,
    cache: Mutex<HashMap<ProviderId, TokenBundle>>,
    /// One refresh at a time, so parallel requests do not spend the same refresh token twice.
    refreshing: tokio::sync::Mutex<()>,
}

impl OAuthService {
    pub fn new(http: reqwest::Client) -> Self {
        Self { http, pending: Mutex::default(), cache: Mutex::default(), refreshing: tokio::sync::Mutex::new(()) }
    }

    pub fn is_connected(&self, provider: ProviderId) -> bool {
        matches!(self.stored_tokens(provider), Ok(Some(_)))
    }

    /// Starts a sign-in and returns the URL to open in the browser.
    pub fn begin(&self, provider: ProviderId) -> String {
        let pkce = Pkce::generate();
        let url = OAuthConfig::for_provider(provider).authorize_url(&pkce);
        self.pending.lock().unwrap().insert(provider, pkce);
        url
    }

    /// Waits for the browser redirect on the provider's loopback port.
    pub async fn await_redirect(provider: ProviderId) -> Result<OAuthCallback> {
        let config = OAuthConfig::for_provider(provider);
        tokio::time::timeout(REDIRECT_TIMEOUT, loopback::wait_for_callback(config.callback_port, config.callback_path))
            .await
            .map_err(|_| Error::OAuthProvider("Timed out waiting for the browser.".into()))?
    }

    /// Exchanges the authorization code for tokens and stores them.
    pub async fn complete(&self, provider: ProviderId, callback: OAuthCallback) -> Result<()> {
        let pkce = self
            .pending
            .lock()
            .unwrap()
            .get(&provider)
            .cloned()
            .ok_or_else(|| Error::OAuthProvider("Start connecting first.".into()))?;
        if callback.state.as_ref().is_some_and(|state| *state != pkce.state) {
            return Err(Error::StateMismatch);
        }
        let config = OAuthConfig::for_provider(provider);
        let (body, _) = self.post_form(config.token_endpoint, &config.exchange_parameters(&callback.code, &pkce.verifier)).await?;
        let bundle = TokenBundle::from_token_response(&body, None, Utc::now(), config.expiry_skew)?;
        self.store(&bundle, provider)?;
        self.pending.lock().unwrap().remove(&provider);
        Ok(())
    }

    pub fn disconnect(&self, provider: ProviderId) -> Result<()> {
        self.cache.lock().unwrap().remove(&provider);
        self.pending.lock().unwrap().remove(&provider);
        keychain::delete_tokens(provider)
    }

    /// A valid token bundle, refreshed if it is about to expire.
    pub async fn active_tokens(&self, provider: ProviderId) -> Result<TokenBundle> {
        let _guard = self.refreshing.lock().await;
        let tokens = self.stored_tokens(provider)?.ok_or(Error::NoProviderConnected)?;
        if !tokens.needs_refresh(Utc::now()) {
            return Ok(tokens);
        }
        let config = OAuthConfig::for_provider(provider);
        let (body, status) = self.post_form(config.token_endpoint, &config.refresh_parameters(&tokens.refresh_token)).await?;
        match TokenBundle::from_token_response(&body, Some(&tokens), Utc::now(), config.expiry_skew) {
            Ok(refreshed) => {
                self.store(&refreshed, provider)?;
                Ok(refreshed)
            }
            // Only a rejected grant means signing in again. An outage must not wipe the saved sign-in.
            Err(error) if status == 400 || status == 401 => {
                let _ = self.disconnect(provider);
                Err(Error::message(format!("{} needs to be reconnected in Settings. {error}", provider.display_name())))
            }
            Err(_) => Err(Error::message(format!(
                "{} sign-in could not be refreshed ({status}). Try again in a moment.",
                provider.display_name()
            ))),
        }
    }

    fn stored_tokens(&self, provider: ProviderId) -> Result<Option<TokenBundle>> {
        if let Some(cached) = self.cache.lock().unwrap().get(&provider) {
            return Ok(Some(cached.clone()));
        }
        let loaded = keychain::load_tokens(provider)?;
        if let Some(tokens) = &loaded {
            self.cache.lock().unwrap().insert(provider, tokens.clone());
        }
        Ok(loaded)
    }

    fn store(&self, bundle: &TokenBundle, provider: ProviderId) -> Result<()> {
        keychain::save_tokens(bundle, provider)?;
        self.cache.lock().unwrap().insert(provider, bundle.clone());
        Ok(())
    }

    async fn post_form(&self, url: &str, parameters: &[(&str, String)]) -> Result<(Vec<u8>, u16)> {
        let response = self
            .http
            .post(url)
            .timeout(REQUEST_TIMEOUT)
            .header("Accept", "application/json")
            .form(parameters)
            .send()
            .await
            .net()?;
        let status = response.status().as_u16();
        Ok((response.bytes().await.net()?.to_vec(), status))
    }
}
