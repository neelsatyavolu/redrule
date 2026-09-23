//! Providers reached with the person's own API key, and the id every note model is stored under.
use serde::{Deserialize, Serialize, Serializer};
use url::Url;

use super::error::{Error, Result};
use super::oauth::ProviderId;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, PartialOrd, Ord)]
#[serde(rename_all = "lowercase")]
pub enum ApiProvider {
    OpenAI,
    Anthropic,
    Gemini,
    /// Any server that speaks the Chat Completions API: xAI, OpenRouter, Groq, Ollama, LM Studio.
    Compatible,
}

impl ApiProvider {
    pub const ALL: [ApiProvider; 4] = [ApiProvider::OpenAI, ApiProvider::Anthropic, ApiProvider::Gemini, ApiProvider::Compatible];

    pub fn raw(self) -> &'static str {
        match self {
            ApiProvider::OpenAI => "openai",
            ApiProvider::Anthropic => "anthropic",
            ApiProvider::Gemini => "gemini",
            ApiProvider::Compatible => "compatible",
        }
    }

    pub fn display_name(self) -> &'static str {
        match self {
            ApiProvider::OpenAI => "OpenAI",
            ApiProvider::Anthropic => "Anthropic",
            ApiProvider::Gemini => "Gemini",
            ApiProvider::Compatible => "The custom server",
        }
    }

    pub fn parse(raw: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|p| p.raw() == raw)
    }

    /// The Keychain account that holds this provider's key.
    pub fn key_account(self) -> String {
        format!("apikey:{}", self.raw())
    }
}

/// Where a note model runs: a signed-in account or an API key. Serialized as its raw id ("codex", "openai").
/// Accounts sort first, so they stay the fallback when several are set up.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Provider {
    Account(ProviderId),
    Api(ApiProvider),
}

impl Provider {
    pub fn raw(self) -> &'static str {
        match self {
            Provider::Account(p) => p.raw(),
            Provider::Api(p) => p.raw(),
        }
    }

    pub fn display_name(self) -> &'static str {
        match self {
            Provider::Account(p) => p.display_name(),
            Provider::Api(p) => p.display_name(),
        }
    }

    pub fn parse(raw: &str) -> Option<Self> {
        ProviderId::parse(raw).map(Provider::Account).or_else(|| ApiProvider::parse(raw).map(Provider::Api))
    }
}

impl Serialize for Provider {
    fn serialize<S: Serializer>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error> {
        serializer.serialize_str(self.raw())
    }
}

/// Splits a stored choice id ("anthropic:claude-opus-5") into its provider and model.
pub fn split_choice(id: &str) -> (Option<Provider>, &str) {
    let (provider, model) = id.split_once(':').unwrap_or((id, ""));
    (Provider::parse(provider), model.trim())
}

/// A model offered for an API provider. `effort` is sent only for these listed models.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ApiModel {
    pub id: &'static str,
    pub label: &'static str,
    pub effort: &'static str,
}

const fn model(id: &'static str, label: &'static str, effort: &'static str) -> ApiModel {
    ApiModel { id, label, effort }
}

// Checked against each provider's model list in September 2026. The first entry is the default.
const OPENAI: &[ApiModel] = &[
    model("gpt-6-astra", "GPT-6 Astra", "low"),
    model("gpt-6-sol", "GPT-6 Sol", "low"),
    model("gpt-6-luna", "GPT-6 Luna", "medium"),
];
const ANTHROPIC: &[ApiModel] = &[
    model("claude-opus-5", "Claude Opus 5", "low"),
    model("claude-sonnet-5", "Claude Sonnet 5", "low"),
    // Haiku 4.5 does not take an effort setting.
    model("claude-haiku-4-5", "Claude Haiku 4.5", ""),
];
const GEMINI: &[ApiModel] = &[
    model("gemini-3.8-flash", "Gemini 3.8 Flash", "low"),
    model("gemini-3.1-pro-preview", "Gemini 3.1 Pro (preview)", "low"),
    model("gemini-3.5-flash-lite", "Gemini 3.5 Flash-Lite", ""),
];

/// The listed models for `provider`; none for a custom server, whose model name is free text.
pub fn api_models(provider: ApiProvider) -> &'static [ApiModel] {
    match provider {
        ApiProvider::OpenAI => OPENAI,
        ApiProvider::Anthropic => ANTHROPIC,
        ApiProvider::Gemini => GEMINI,
        ApiProvider::Compatible => &[],
    }
}

pub fn listed_model(provider: ApiProvider, id: &str) -> Option<ApiModel> {
    api_models(provider).iter().copied().find(|m| m.id == id)
}

/// A custom server's base URL, without a trailing slash or a pasted "/chat/completions".
/// Plain http is allowed only for a server on this Mac.
pub fn validate_base_url(input: &str) -> Result<String> {
    let trimmed = input.trim().trim_end_matches('/');
    let trimmed = trimmed.strip_suffix("/chat/completions").unwrap_or(trimmed);
    let url = Url::parse(trimmed).map_err(|_| Error::message("Enter the server's address, like https://openrouter.ai/api/v1."))?;
    let local = matches!(url.host_str(), Some("localhost" | "127.0.0.1" | "[::1]"));
    match url.scheme() {
        "https" if url.host_str().is_some() => {}
        "http" if local => {}
        "http" => return Err(Error::message("Use an https:// address. Plain http:// works only for a server on this Mac (localhost).")),
        _ => return Err(Error::message("Enter an address that starts with https://.")),
    }
    if url.query().is_some() || url.fragment().is_some() {
        return Err(Error::message("Enter the server's base address without ? or # parts."));
    }
    Ok(trimmed.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ids_round_trip_for_accounts_and_keys() {
        assert_eq!(split_choice("codex:gpt-6-astra"), (Some(Provider::Account(ProviderId::Codex)), "gpt-6-astra"));
        assert_eq!(split_choice("anthropic:claude-opus-5"), (Some(Provider::Api(ApiProvider::Anthropic)), "claude-opus-5"));
        assert_eq!(split_choice("compatible:org/model:free"), (Some(Provider::Api(ApiProvider::Compatible)), "org/model:free"));
        assert_eq!(split_choice("local:qwen3.5-9b").0, None);
        assert_eq!(split_choice("nonsense"), (None, ""));
        assert_eq!(serde_json::to_string(&Provider::Api(ApiProvider::OpenAI)).unwrap(), "\"openai\"");
        assert_eq!(serde_json::to_string(&Provider::Account(ProviderId::Grok)).unwrap(), "\"grok\"");
        assert!(Provider::Account(ProviderId::Grok) < Provider::Api(ApiProvider::OpenAI));
        assert_eq!(ApiProvider::Gemini.key_account(), "apikey:gemini");
    }

    #[test]
    fn every_listed_provider_has_a_default() {
        for provider in [ApiProvider::OpenAI, ApiProvider::Anthropic, ApiProvider::Gemini] {
            assert!(!api_models(provider).is_empty());
        }
        assert!(listed_model(ApiProvider::Anthropic, "claude-haiku-4-5").is_some_and(|m| m.effort.is_empty()));
        assert!(listed_model(ApiProvider::OpenAI, "gpt-2").is_none());
    }

    #[test]
    fn base_urls_need_https_unless_local() {
        assert_eq!(validate_base_url(" https://openrouter.ai/api/v1/ ").unwrap(), "https://openrouter.ai/api/v1");
        assert_eq!(validate_base_url("https://api.groq.com/openai/v1/chat/completions").unwrap(), "https://api.groq.com/openai/v1");
        assert_eq!(validate_base_url("http://localhost:11434/v1").unwrap(), "http://localhost:11434/v1");
        assert_eq!(validate_base_url("http://127.0.0.1:1234/v1").unwrap(), "http://127.0.0.1:1234/v1");
        assert_eq!(validate_base_url("http://[::1]:1234/v1").unwrap(), "http://[::1]:1234/v1");
        assert!(validate_base_url("http://example.com/v1").is_err());
        assert!(validate_base_url("http://localhost.example.com/v1").is_err());
        assert!(validate_base_url("ftp://example.com").is_err());
        assert!(validate_base_url("openrouter.ai/api/v1").is_err());
        assert!(validate_base_url("https://example.com/v1?key=x").is_err());
    }
}
