//! OAuth PKCE helpers for the ChatGPT (Codex) and Grok sign-ins. Pure and unit tested.
use base64::Engine;
use base64::engine::general_purpose::{URL_SAFE, URL_SAFE_NO_PAD};
use chrono::{DateTime, Duration, TimeZone, Utc};
use percent_encoding::{AsciiSet, NON_ALPHANUMERIC, utf8_percent_encode};
use rand::RngCore;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use serde_json::Value;
use sha2::{Digest, Sha256};

use super::error::{Error, Result};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, PartialOrd, Ord)]
#[serde(rename_all = "lowercase")]
pub enum ProviderId {
    Codex,
    Grok,
}

impl ProviderId {
    pub const ALL: [ProviderId; 2] = [ProviderId::Codex, ProviderId::Grok];

    pub fn raw(self) -> &'static str {
        match self {
            ProviderId::Codex => "codex",
            ProviderId::Grok => "grok",
        }
    }

    pub fn display_name(self) -> &'static str {
        match self {
            ProviderId::Codex => "ChatGPT (Codex)",
            ProviderId::Grok => "Grok",
        }
    }

    pub fn parse(raw: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|p| p.raw() == raw)
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Pkce {
    pub verifier: String,
    pub challenge: String,
    pub state: String,
}

impl Pkce {
    pub fn generate() -> Self {
        let verifier = random_base64url(64);
        Self { challenge: challenge_for(&verifier), verifier, state: random_base64url(32) }
    }
}

pub fn challenge_for(verifier: &str) -> String {
    URL_SAFE_NO_PAD.encode(Sha256::digest(verifier.as_bytes()))
}

fn random_base64url(byte_count: usize) -> String {
    let mut bytes = vec![0u8; byte_count];
    rand::rng().fill_bytes(&mut bytes);
    URL_SAFE_NO_PAD.encode(bytes)
}

pub struct OAuthConfig {
    pub provider: ProviderId,
    pub client_id: &'static str,
    pub authorize_endpoint: &'static str,
    pub token_endpoint: &'static str,
    pub redirect_uri: &'static str,
    pub callback_port: u16,
    pub callback_path: &'static str,
    pub scope: &'static str,
    /// Sorted by key.
    pub extra_authorize_parameters: &'static [(&'static str, &'static str)],
    /// Seconds subtracted from `expires_in` so tokens are treated as expired early.
    pub expiry_skew: i64,
}

pub const CODEX: OAuthConfig = OAuthConfig {
    provider: ProviderId::Codex,
    client_id: "app_EMoamEEZ73f0CkXaXp7hrann",
    authorize_endpoint: "https://auth.openai.com/oauth/authorize",
    token_endpoint: "https://auth.openai.com/oauth/token",
    redirect_uri: "http://localhost:1455/auth/callback",
    callback_port: 1455,
    callback_path: "/auth/callback",
    scope: "openid profile email offline_access",
    extra_authorize_parameters: &[
        ("codex_cli_simplified_flow", "true"),
        ("id_token_add_organizations", "true"),
        ("originator", "codex_cli_rs"),
    ],
    expiry_skew: 60,
};

pub const GROK: OAuthConfig = OAuthConfig {
    provider: ProviderId::Grok,
    client_id: "b1a00492-073a-47ea-816f-4c329264a828",
    authorize_endpoint: "https://auth.x.ai/oauth2/authorize",
    token_endpoint: "https://auth.x.ai/oauth2/token",
    redirect_uri: "http://127.0.0.1:56121/callback",
    callback_port: 56121,
    callback_path: "/callback",
    scope: "openid profile email offline_access grok-cli:access api:access",
    extra_authorize_parameters: &[],
    expiry_skew: 120,
};

/// Characters left unescaped in query values: RFC 3986 unreserved.
const QUERY_VALUE: &AsciiSet = &NON_ALPHANUMERIC.remove(b'-').remove(b'.').remove(b'_').remove(b'~');

impl OAuthConfig {
    pub fn for_provider(provider: ProviderId) -> &'static OAuthConfig {
        match provider {
            ProviderId::Codex => &CODEX,
            ProviderId::Grok => &GROK,
        }
    }

    pub fn authorize_url(&self, pkce: &Pkce) -> String {
        let base = [
            ("response_type", "code"),
            ("client_id", self.client_id),
            ("redirect_uri", self.redirect_uri),
            ("scope", self.scope),
            ("code_challenge", pkce.challenge.as_str()),
            ("code_challenge_method", "S256"),
            ("state", pkce.state.as_str()),
        ];
        let query = base
            .iter()
            .chain(self.extra_authorize_parameters)
            .map(|(k, v)| format!("{k}={}", utf8_percent_encode(v, QUERY_VALUE)))
            .collect::<Vec<_>>()
            .join("&");
        format!("{}?{query}", self.authorize_endpoint)
    }

    pub fn exchange_parameters(&self, code: &str, verifier: &str) -> Vec<(&'static str, String)> {
        vec![
            ("grant_type", "authorization_code".into()),
            ("code", code.into()),
            ("redirect_uri", self.redirect_uri.into()),
            ("client_id", self.client_id.into()),
            ("code_verifier", verifier.into()),
        ]
    }

    pub fn refresh_parameters(&self, refresh_token: &str) -> Vec<(&'static str, String)> {
        vec![
            ("grant_type", "refresh_token".into()),
            ("refresh_token", refresh_token.into()),
            ("client_id", self.client_id.into()),
            ("scope", self.scope.into()),
        ]
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct OAuthCallback {
    pub code: String,
    pub state: Option<String>,
}

/// Accepts a redirect URL, an HTTP request line, `code#state`, or a bare code.
pub fn parse_callback(input: &str) -> Result<OAuthCallback> {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return Err(Error::EmptyInput);
    }

    if let Some(query_start) = trimmed.find('?') {
        let query: String =
            trimmed[query_start + 1..].chars().take_while(|&c| c != ' ' && c != '#').collect();
        let items: Vec<(String, String)> = url::form_urlencoded::parse(query.as_bytes()).into_owned().collect();
        let value = |name: &str| items.iter().find(|(k, _)| k == name).map(|(_, v)| v.clone());
        if let Some(error) = value("error") {
            return Err(Error::OAuthProvider(value("error_description").unwrap_or(error)));
        }
        let code = value("code").filter(|c| !c.is_empty()).ok_or(Error::MissingCode)?;
        return Ok(OAuthCallback { code, state: value("state") });
    }

    let mut parts = trimmed.splitn(2, '#');
    let code = parts.next().unwrap_or_default();
    if code.is_empty() || code.contains(' ') {
        return Err(Error::MissingCode);
    }
    Ok(OAuthCallback { code: code.to_string(), state: parts.next().map(str::to_string) })
}

const REFRESH_WINDOW_SECONDS: i64 = 30;

/// Stored in the Keychain as JSON, in the Swift app's encoding (dates as seconds since 2001).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TokenBundle {
    pub access_token: String,
    pub refresh_token: String,
    #[serde(with = "reference_date")]
    pub expires_at: DateTime<Utc>,
    #[serde(rename = "accountID", default, skip_serializing_if = "Option::is_none")]
    pub account_id: Option<String>,
}

impl TokenBundle {
    pub fn needs_refresh(&self, now: DateTime<Utc>) -> bool {
        (self.expires_at - now).num_seconds() < REFRESH_WINDOW_SECONDS
    }

    /// Builds a bundle from a token endpoint response. On refresh, missing fields fall back to `previous`.
    pub fn from_token_response(
        data: &[u8],
        previous: Option<&TokenBundle>,
        now: DateTime<Utc>,
        expiry_skew: i64,
    ) -> Result<Self> {
        let json: Value = serde_json::from_slice(data).map_err(|_| Error::BadTokenResponse)?;
        let text = |key: &str| json.get(key).and_then(Value::as_str).map(str::to_string);
        if let Some(error) = text("error") {
            return Err(Error::OAuthProvider(text("error_description").unwrap_or(error)));
        }
        let access = text("access_token").ok_or(Error::BadTokenResponse)?;
        let refresh = text("refresh_token")
            .or_else(|| previous.map(|p| p.refresh_token.clone()))
            .ok_or(Error::BadTokenResponse)?;
        let expires_in = json.get("expires_in").and_then(Value::as_f64).unwrap_or(3600.0);
        let lifetime = (expires_in as i64 - expiry_skew).max(30);
        let account = text("id_token")
            .and_then(|token| codex_account_id(&token))
            .or_else(|| previous.and_then(|p| p.account_id.clone()));
        Ok(Self {
            access_token: access,
            refresh_token: refresh,
            expires_at: now + Duration::seconds(lifetime),
            account_id: account,
        })
    }
}

/// The ChatGPT account id lives in the id token: the default organization, else `sub`.
pub fn codex_account_id(id_token: &str) -> Option<String> {
    let payload = id_token.split('.').nth(1)?;
    let padded = format!("{payload}{}", "=".repeat((4 - payload.len() % 4) % 4));
    let claims: Value = serde_json::from_slice(&URL_SAFE.decode(padded).ok()?).ok()?;
    let orgs = claims.pointer("/https:~1~1api.openai.com~1auth/organizations").and_then(Value::as_array);
    if let Some(orgs) = orgs.filter(|o| !o.is_empty()) {
        let chosen = orgs.iter().find(|o| o.get("is_default").and_then(Value::as_bool) == Some(true)).unwrap_or(&orgs[0]);
        return chosen.get("id").and_then(Value::as_str).map(str::to_string);
    }
    claims.get("sub").and_then(Value::as_str).map(str::to_string)
}

/// Foundation's default date encoding: seconds since 2001-01-01T00:00:00Z.
mod reference_date {
    use super::*;

    const OFFSET: f64 = 978_307_200.0;

    pub fn serialize<S: Serializer>(date: &DateTime<Utc>, s: S) -> std::result::Result<S::Ok, S::Error> {
        s.serialize_f64(date.timestamp_millis() as f64 / 1000.0 - OFFSET)
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(d: D) -> std::result::Result<DateTime<Utc>, D::Error> {
        let seconds = f64::deserialize(d)? + OFFSET;
        Utc.timestamp_millis_opt((seconds * 1000.0) as i64)
            .single()
            .ok_or_else(|| serde::de::Error::custom("invalid date"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn challenge_matches_the_rfc_7636_example() {
        assert_eq!(
            challenge_for("dBjftJeZ4CVP-mB92K27uhbUJU1p1r_wW1gFWFOEjXk"),
            "E9Melhoa2OwvFrEMTJguCHaoeK1t8URWbuGJSstw-cM"
        );
    }

    #[test]
    fn authorize_url_carries_pkce_and_extras() {
        let pkce = Pkce { verifier: "v".into(), challenge: "c".into(), state: "s".into() };
        let url = CODEX.authorize_url(&pkce);
        assert!(url.starts_with("https://auth.openai.com/oauth/authorize?response_type=code&client_id="));
        assert!(url.contains("redirect_uri=http%3A%2F%2Flocalhost%3A1455%2Fauth%2Fcallback"));
        assert!(url.contains("scope=openid%20profile%20email%20offline_access"));
        assert!(url.contains("code_challenge=c&code_challenge_method=S256&state=s"));
        assert!(url.ends_with("&originator=codex_cli_rs"));
    }

    #[test]
    fn parses_redirects_request_lines_and_bare_codes() {
        let redirect = parse_callback("http://localhost:1455/auth/callback?code=abc&state=xyz").unwrap();
        assert_eq!(redirect, OAuthCallback { code: "abc".into(), state: Some("xyz".into()) });
        let line = parse_callback("GET /callback?code=abc&state=xyz HTTP/1.1").unwrap();
        assert_eq!(line.state.as_deref(), Some("xyz"));
        assert_eq!(parse_callback("abc#xyz").unwrap().state.as_deref(), Some("xyz"));
        assert_eq!(parse_callback("  abc ").unwrap().code, "abc");
    }

    #[test]
    fn reports_callback_errors() {
        assert!(matches!(parse_callback(" "), Err(Error::EmptyInput)));
        assert!(matches!(parse_callback("http://x/cb?state=1"), Err(Error::MissingCode)));
        assert!(matches!(parse_callback("two words"), Err(Error::MissingCode)));
        let denied = parse_callback("http://x/cb?error=access_denied&error_description=No+thanks");
        assert_eq!(denied.unwrap_err().to_string(), "Sign-in failed: No thanks");
    }

    #[test]
    fn builds_tokens_and_keeps_refresh_token_on_refresh() {
        let now = Utc.with_ymd_and_hms(2026, 9, 22, 0, 0, 0).unwrap();
        let first = TokenBundle::from_token_response(
            br#"{"access_token":"a","refresh_token":"r","expires_in":3600}"#,
            None,
            now,
            60,
        )
        .unwrap();
        assert_eq!(first.expires_at, now + Duration::seconds(3540));
        let refreshed =
            TokenBundle::from_token_response(br#"{"access_token":"b"}"#, Some(&first), now, 60).unwrap();
        assert_eq!(refreshed.refresh_token, "r");
        assert!(!refreshed.needs_refresh(now));
        assert!(refreshed.needs_refresh(now + Duration::seconds(3520)));
    }

    #[test]
    fn token_errors_are_reported() {
        let now = Utc::now();
        let error = TokenBundle::from_token_response(br#"{"error":"invalid_grant"}"#, None, now, 60).unwrap_err();
        assert_eq!(error.to_string(), "Sign-in failed: invalid_grant");
        assert!(matches!(TokenBundle::from_token_response(b"nope", None, now, 60), Err(Error::BadTokenResponse)));
    }

    #[test]
    fn reads_the_account_from_the_default_organization() {
        let claims = r#"{"sub":"user-1","https://api.openai.com/auth":{"organizations":[{"id":"org-a"},{"id":"org-b","is_default":true}]}}"#;
        let token = format!("h.{}.s", URL_SAFE_NO_PAD.encode(claims));
        assert_eq!(codex_account_id(&token).as_deref(), Some("org-b"));
        let plain = format!("h.{}.s", URL_SAFE_NO_PAD.encode(r#"{"sub":"user-1"}"#));
        assert_eq!(codex_account_id(&plain).as_deref(), Some("user-1"));
    }

    #[test]
    fn keychain_json_round_trips_the_swift_date_encoding() {
        let json = r#"{"accessToken":"a","refreshToken":"r","expiresAt":780192000,"accountID":"acct"}"#;
        let bundle: TokenBundle = serde_json::from_str(json).unwrap();
        assert_eq!(bundle.expires_at, Utc.with_ymd_and_hms(2025, 9, 22, 0, 0, 0).unwrap());
        let again: TokenBundle = serde_json::from_str(&serde_json::to_string(&bundle).unwrap()).unwrap();
        assert_eq!(again, bundle);
    }
}
