//! API keys in the Keychain, the request that checks one, and prompts sent with one.
use std::collections::HashMap;
use std::sync::Mutex;
use std::time::Duration;

use serde_json::Value;

use super::{NetResult, keychain};
use crate::core::api_providers::ApiProvider;
use crate::core::api_requests::{ApiRequest, Target, error_message, reply_text};
use crate::core::{Error, Result};

const CHECK_TIMEOUT: Duration = Duration::from_secs(20);

/// What a prompt to an API provider needs besides the model. The key is optional only for a custom server.
#[derive(Debug, Clone, Default)]
pub struct ApiAccess {
    pub key: Option<String>,
    pub base_url: String,
}

/// Saved keys, read from the Keychain once and then cached.
#[derive(Default)]
pub struct ApiKeys {
    cache: Mutex<HashMap<ApiProvider, Option<String>>>,
}

impl ApiKeys {
    pub fn get(&self, provider: ApiProvider) -> Result<Option<String>> {
        if let Some(cached) = self.cache.lock().unwrap().get(&provider) {
            return Ok(cached.clone());
        }
        let key = keychain::load_api_key(provider)?;
        self.cache.lock().unwrap().insert(provider, key.clone());
        Ok(key)
    }

    /// An unreadable item counts as no key, as an unreadable sign-in counts as signed out.
    pub fn has(&self, provider: ApiProvider) -> bool {
        matches!(self.get(provider), Ok(Some(_)))
    }

    pub fn save(&self, provider: ApiProvider, key: &str) -> Result<()> {
        keychain::save_api_key(provider, key)?;
        self.cache.lock().unwrap().insert(provider, Some(key.to_string()));
        Ok(())
    }

    pub fn delete(&self, provider: ApiProvider) -> Result<()> {
        keychain::delete_api_key(provider)?;
        self.cache.lock().unwrap().insert(provider, None);
        Ok(())
    }
}

async fn send(http: &reqwest::Client, target: Target<'_>, request: &ApiRequest, timeout: Duration, model: Option<&str>) -> Result<String> {
    let mut builder = match &request.body {
        Some(body) => http.post(&request.url).json(body),
        None => http.get(&request.url),
    };
    for (name, value) in &request.headers {
        builder = builder.header(*name, value);
    }
    let response = match builder.timeout(timeout).send().await {
        Err(error) if target.provider == ApiProvider::Compatible && !error.is_timeout() => {
            return Err(Error::message(format!(
                "Redrule could not reach {}. Check the address and that the server is running.",
                target.base_url
            )));
        }
        result => result.net()?,
    };
    let status = response.status().as_u16();
    let body = response.text().await.net()?;
    if !(200..300).contains(&status) {
        return Err(Error::message(error_message(target.provider, status, &body, model)));
    }
    Ok(body)
}

/// Checks a key, and a custom server's address, by listing the models.
pub async fn check(http: &reqwest::Client, target: Target<'_>) -> Result<()> {
    send(http, target, &target.models_request(), CHECK_TIMEOUT, None).await.map(|_| ())
}

pub async fn complete(
    http: &reqwest::Client,
    target: Target<'_>,
    model: &str,
    system: &str,
    user: &str,
    schema: Option<&Value>,
    timeout: Duration,
) -> Result<String> {
    let request = target.completion_request(model, system, user, schema);
    reply_text(target.provider, &send(http, target, &request, timeout, Some(model)).await?)
}
