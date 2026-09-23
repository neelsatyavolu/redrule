//! Model choices and the summary client for accounts (ChatGPT, Grok) and API keys.
use std::sync::{Arc, OnceLock, RwLock};
use std::time::Duration;

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use super::NetResult;
use super::api_keys::{self, ApiAccess};
use super::oauth_service::OAuthService;
use crate::core::api_providers::{ApiProvider, Provider, api_models, split_choice};
use crate::core::api_requests::Target;
use crate::core::oauth::ProviderId;
use crate::core::summary::{SummaryProvider, codex_output_text};
use crate::core::{Error, Result};

const CODEX_ENDPOINT: &str = "https://chatgpt.com/backend-api/codex/responses";
const GROK_ENDPOINT: &str = "https://api.x.ai/v1/chat/completions";
const TIMEOUT: Duration = Duration::from_secs(240);

/// A provider and model, stored as "codex:gpt-6-astra" or "anthropic:claude-opus-5".
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelChoice {
    pub provider: Provider,
    pub model: String,
    pub label: String,
    /// Reasoning effort sent with every request for this model.
    pub effort: String,
}

fn choice(provider: ProviderId, model: &str, label: &str, effort: &str) -> ModelChoice {
    ModelChoice { provider: Provider::Account(provider), model: model.into(), label: label.into(), effort: effort.into() }
}

/// Newest first within each provider; the first entry is that provider's default.
fn fallback_choices() -> Vec<ModelChoice> { vec![
    choice(ProviderId::Codex, "gpt-6-astra", "GPT-6 Astra", "low"),
    choice(ProviderId::Codex, "gpt-6-sol", "GPT-6 Sol", "low"),
    choice(ProviderId::Codex, "gpt-6-luna", "GPT-6 Luna", "medium"),
    choice(ProviderId::Codex, "gpt-5.6-sol", "GPT-5.6 Sol", "low"),
    choice(ProviderId::Codex, "gpt-5.6-terra", "GPT-5.6 Terra", "low"),
    choice(ProviderId::Codex, "gpt-5.6-luna", "GPT-5.6 Luna", "medium"),
    choice(ProviderId::Grok, "grok-4.7", "Grok 4.7", "low"),
    choice(ProviderId::Grok, "grok-4.6", "Grok 4.6", "low"),
    choice(ProviderId::Grok, "grok-4.5", "Grok 4.5", "low"),
] }

static MODEL_CHOICES: OnceLock<RwLock<Vec<ModelChoice>>> = OnceLock::new();
fn choices() -> &'static RwLock<Vec<ModelChoice>> {
    MODEL_CHOICES.get_or_init(|| RwLock::new(fallback_choices()))
}

/// Account models from the catalog, then the listed models of each API provider.
pub fn model_choices() -> Vec<ModelChoice> {
    let mut all = choices().read().expect("model catalog lock").clone();
    all.extend(ApiProvider::ALL.into_iter().flat_map(|provider| {
        api_models(provider).iter().map(move |m| ModelChoice {
            provider: Provider::Api(provider),
            model: m.id.into(),
            label: m.label.into(),
            effort: m.effort.into(),
        })
    }));
    all
}

#[derive(Deserialize)]
struct CatalogEntry { id: String, label: String, effort: Option<String> }
#[derive(Deserialize)]
struct Catalog { version: u32, codex: Vec<CatalogEntry>, grok: Vec<CatalogEntry> }

// Add IDs here only when Redrule should hide them. New IDs show by default.
const HIDDEN_MODELS: &[&str] = &[];

pub async fn refresh_model_choices() {
    let response = reqwest::Client::new()
        .get("https://raw.githubusercontent.com/neelsatyavolu/shared-ai-auth/main/models.json")
        .timeout(Duration::from_secs(5)).send().await;
    let Ok(response) = response else { return };
    if !response.status().is_success() { return; }
    let Ok(catalog) = response.json::<Catalog>().await else { return };
    if catalog.version != 1 || catalog.codex.is_empty() || catalog.grok.is_empty() { return; }
    let map = |provider, entries: Vec<CatalogEntry>| entries.into_iter().filter_map(|entry| {
        if entry.id.is_empty() || entry.label.is_empty() { return None; }
        if HIDDEN_MODELS.contains(&entry.id.as_str()) { return None; }
        Some(choice(provider, &entry.id, &entry.label, entry.effort.as_deref().unwrap_or("low")))
    }).collect::<Vec<_>>();
    let mut updated = map(ProviderId::Codex, catalog.codex);
    updated.extend(map(ProviderId::Grok, catalog.grok));
    if ProviderId::ALL.iter().any(|provider| !updated.iter().any(|entry| entry.provider == Provider::Account(*provider))) { return; }
    *choices().write().expect("model catalog lock") = updated;
}

impl ModelChoice {
    pub fn id(&self) -> String {
        format!("{}:{}", self.provider.raw(), self.model)
    }

    /// The provider's first listed model. A custom server has none: its model is whatever was typed.
    pub fn default_for(provider: Provider) -> Option<ModelChoice> {
        model_choices().into_iter().find(|c| c.provider == provider)
    }

    fn fallback() -> ModelChoice {
        Self::default_for(Provider::Account(ProviderId::Codex)).expect("the catalog lists Codex models")
    }

    /// A model typed in for an API provider, kept as typed.
    pub fn custom(provider: ApiProvider, model: &str) -> ModelChoice {
        ModelChoice { provider: Provider::Api(provider), model: model.into(), label: model.into(), effort: String::new() }
    }

    /// A saved choice that is no longer offered (a retired account model) falls back to the same provider's
    /// default. API providers keep any model name, since people can type their own.
    pub fn resolve(id: Option<&str>) -> ModelChoice {
        if let Some(found) = model_choices().into_iter().find(|c| Some(c.id().as_str()) == id) {
            return found;
        }
        match id.map(split_choice).unwrap_or((None, "")) {
            (Some(Provider::Api(provider)), model) if !model.is_empty() => Self::custom(provider, model),
            (Some(provider), _) => Self::default_for(provider).unwrap_or_else(Self::fallback),
            (None, _) => Self::fallback(),
        }
    }
}

async fn send(request: reqwest::RequestBuilder, provider: ProviderId) -> Result<String> {
    let response = request.timeout(TIMEOUT).send().await.net()?;
    let status = response.status();
    let body = response.text().await.net()?;
    if !status.is_success() {
        let detail: String = body.chars().take(300).collect();
        return Err(Error::message(format!("{} returned an error ({}). {detail}", provider.display_name(), status.as_u16())));
    }
    Ok(body)
}

pub struct SummaryClient {
    pub http: reqwest::Client,
    pub oauth: Arc<OAuthService>,
    pub choice: ModelChoice,
    /// The key and address for an API provider; unused by accounts.
    pub api: ApiAccess,
}

impl SummaryClient {
    async fn codex(&self, system: &str, user: &str, schema: Option<&Value>) -> Result<String> {
        let tokens = self.oauth.active_tokens(ProviderId::Codex).await?;
        let mut body = json!({
            "model": self.choice.model,
            "instructions": system,
            "input": [{"role": "user", "content": [{"type": "input_text", "text": user}]}],
            "reasoning": {"effort": self.choice.effort},
            "store": false,
            "stream": true,
        });
        if let Some(schema) = schema {
            body["text"] = json!({"format": {"type": "json_schema", "name": "meeting_note", "strict": true, "schema": schema}});
        }
        let mut request = self
            .http
            .post(CODEX_ENDPOINT)
            .json(&body)
            .header("Accept", "text/event-stream, application/json")
            .bearer_auth(&tokens.access_token)
            .header("originator", "codex_cli_rs")
            .header("OpenAI-Beta", "responses=v1");
        if let Some(account) = &tokens.account_id {
            request = request.header("chatgpt-account-id", account);
        }
        codex_output_text(&send(request, ProviderId::Codex).await?)
    }

    async fn grok(&self, system: &str, user: &str) -> Result<String> {
        let tokens = self.oauth.active_tokens(ProviderId::Grok).await?;
        let body = json!({
            "model": self.choice.model,
            "messages": [{"role": "system", "content": system}, {"role": "user", "content": user}],
            "temperature": 0.3,
            "reasoning": {"effort": self.choice.effort},
        });
        let request = self.http.post(GROK_ENDPOINT).json(&body).bearer_auth(&tokens.access_token);
        let reply: Value = serde_json::from_str(&send(request, ProviderId::Grok).await?)?;
        reply
            .pointer("/choices/0/message/content")
            .and_then(Value::as_str)
            .map(str::to_string)
            .ok_or_else(|| Error::message("Grok returned a reply with no text."))
    }
}

impl SummaryProvider for SummaryClient {
    async fn complete(&self, system: &str, user: &str, schema: Option<&Value>) -> Result<String> {
        match self.choice.provider {
            Provider::Account(ProviderId::Codex) => self.codex(system, user, schema).await,
            Provider::Account(ProviderId::Grok) => self.grok(system, user).await,
            Provider::Api(provider) => {
                let target = Target { provider, base_url: &self.api.base_url, key: self.api.key.as_deref() };
                api_keys::complete(&self.http, target, &self.choice.model, system, user, schema, TIMEOUT).await
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolves_saved_and_retired_choices() {
        assert_eq!(ModelChoice::resolve(Some("grok:grok-4.6")).model, "grok-4.6");
        assert_eq!(ModelChoice::resolve(Some("grok:grok-2")).model, "grok-4.7");
        assert_eq!(ModelChoice::resolve(None).model, "gpt-6-astra");
        assert_eq!(ModelChoice::resolve(Some("nonsense")).provider, Provider::Account(ProviderId::Codex));
    }

    #[test]
    fn api_choices_keep_listed_and_typed_models() {
        let listed = ModelChoice::resolve(Some("anthropic:claude-sonnet-5"));
        assert_eq!((listed.label.as_str(), listed.effort.as_str()), ("Claude Sonnet 5", "low"));
        let typed = ModelChoice::resolve(Some("openai:my-fine-tune"));
        assert_eq!((typed.provider, typed.model.as_str(), typed.effort.as_str()), (Provider::Api(ApiProvider::OpenAI), "my-fine-tune", ""));
        assert_eq!(ModelChoice::resolve(Some("compatible:org/model:free")).id(), "compatible:org/model:free");
        assert_eq!(ModelChoice::resolve(Some("gemini:")).model, "gemini-3.8-flash");
        // A custom server with no model name has no default, so notes fall back to an account model.
        assert_eq!(ModelChoice::resolve(Some("compatible:")).provider, Provider::Account(ProviderId::Codex));
        assert_eq!(ModelChoice::default_for(Provider::Api(ApiProvider::Compatible)), None);
    }
}
