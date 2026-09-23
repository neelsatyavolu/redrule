//! Request bodies, reply parsing and error wording for the API key providers. Pure; the app sends them.
use serde_json::{Value, json};

use super::api_providers::{ApiProvider, listed_model};
use super::error::{Error, Result};

const OPENAI: &str = "https://api.openai.com/v1";
const ANTHROPIC: &str = "https://api.anthropic.com/v1";
const GEMINI: &str = "https://generativelanguage.googleapis.com/v1beta";
const ANTHROPIC_VERSION: &str = "2023-06-01";
/// Room for the notes plus the model's thinking.
const ANTHROPIC_MAX_TOKENS: u32 = 16_000;

/// An HTTP request to send: GET when `body` is `None`. Headers carry the key, so never log them.
#[derive(Debug, Clone, PartialEq)]
pub struct ApiRequest {
    pub url: String,
    pub headers: Vec<(&'static str, String)>,
    pub body: Option<Value>,
}

/// Where and how to reach one provider.
#[derive(Debug, Clone, Copy)]
pub struct Target<'a> {
    pub provider: ApiProvider,
    /// Used only by the custom server.
    pub base_url: &'a str,
    pub key: Option<&'a str>,
}

impl Target<'_> {
    fn base(&self) -> &str {
        match self.provider {
            ApiProvider::OpenAI => OPENAI,
            ApiProvider::Anthropic => ANTHROPIC,
            ApiProvider::Gemini => GEMINI,
            ApiProvider::Compatible => self.base_url,
        }
    }

    fn headers(&self) -> Vec<(&'static str, String)> {
        let Some(key) = self.key.filter(|key| !key.is_empty()) else { return vec![] };
        match self.provider {
            ApiProvider::Anthropic => vec![("x-api-key", key.into()), ("anthropic-version", ANTHROPIC_VERSION.into())],
            ApiProvider::Gemini => vec![("x-goog-api-key", key.into())],
            ApiProvider::OpenAI | ApiProvider::Compatible => vec![("authorization", format!("Bearer {key}"))],
        }
    }

    /// A cheap request that checks the key: listing the models.
    pub fn models_request(&self) -> ApiRequest {
        ApiRequest { url: format!("{}/models", self.base()), headers: self.headers(), body: None }
    }

    /// One prompt. Listed models get the strict `schema` and their effort. Custom model names rely on the
    /// prompt, which asks for JSON, and on the tolerant note parser; Gemini also gets its JSON mode.
    pub fn completion_request(&self, model: &str, system: &str, user: &str, schema: Option<&Value>) -> ApiRequest {
        let listed = listed_model(self.provider, model);
        let wants_json = schema.is_some();
        let schema = schema.filter(|_| listed.is_some());
        let effort = listed.map(|m| m.effort).filter(|e| !e.is_empty());
        let mut headers = self.headers();
        let (url, body) = match self.provider {
            ApiProvider::OpenAI => {
                let mut body = json!({"model": model, "instructions": system, "input": user, "store": false});
                if let Some(schema) = schema {
                    body["text"] = json!({"format": {"type": "json_schema", "name": "meeting_note", "strict": true, "schema": schema}});
                }
                if let Some(effort) = effort {
                    body["reasoning"] = json!({"effort": effort});
                }
                (format!("{OPENAI}/responses"), body)
            }
            ApiProvider::Anthropic => {
                let mut body = json!({
                    "model": model,
                    "max_tokens": ANTHROPIC_MAX_TOKENS,
                    "system": system,
                    "messages": [{"role": "user", "content": user}],
                });
                let mut output = serde_json::Map::new();
                if let Some(schema) = schema {
                    output.insert("format".into(), json!({"type": "json_schema", "schema": schema}));
                }
                if let Some(effort) = effort {
                    output.insert("effort".into(), json!(effort));
                }
                if !output.is_empty() {
                    body["output_config"] = Value::Object(output);
                }
                // Claude Opus 5 hands a declined request to another model instead of stopping.
                if model == "claude-opus-5" {
                    body["fallbacks"] = json!("default");
                    headers.push(("anthropic-beta", "server-side-fallback-2026-07-01".into()));
                }
                (format!("{ANTHROPIC}/messages"), body)
            }
            ApiProvider::Gemini => {
                let mut config = json!({});
                if wants_json {
                    config["responseMimeType"] = json!("application/json");
                }
                if let Some(schema) = schema {
                    config["responseJsonSchema"] = schema.clone();
                }
                if let Some(effort) = effort {
                    config["thinkingConfig"] = json!({"thinkingLevel": effort});
                }
                let body = json!({
                    "systemInstruction": {"parts": [{"text": system}]},
                    "contents": [{"role": "user", "parts": [{"text": user}]}],
                    "generationConfig": config,
                });
                (format!("{GEMINI}/models/{model}:generateContent"), body)
            }
            ApiProvider::Compatible => {
                let body = json!({
                    "model": model,
                    "messages": [{"role": "system", "content": system}, {"role": "user", "content": user}],
                });
                (format!("{}/chat/completions", self.base_url), body)
            }
        };
        ApiRequest { url, headers, body: Some(body) }
    }
}

/// The model's text from a successful reply body.
pub fn reply_text(provider: ApiProvider, body: &str) -> Result<String> {
    let reply: Value = serde_json::from_str(body).map_err(|_| unreadable(provider))?;
    if let Some(message) = reply.pointer("/error/message").and_then(Value::as_str) {
        return Err(Error::message(format!("{} returned an error. {message}", provider.display_name())));
    }
    let text = match provider {
        ApiProvider::OpenAI => openai_text(&reply)?,
        ApiProvider::Anthropic => anthropic_text(&reply)?,
        ApiProvider::Gemini => gemini_text(&reply)?,
        ApiProvider::Compatible => reply.pointer("/choices/0/message/content").and_then(Value::as_str).unwrap_or_default().to_string(),
    };
    if text.trim().is_empty() {
        return Err(Error::message(format!("{} returned a reply with no text.", provider.display_name())));
    }
    Ok(text)
}

fn unreadable(provider: ApiProvider) -> Error {
    Error::message(format!("{} sent a reply Redrule could not read.", provider.display_name()))
}

fn items<'a>(value: &'a Value, pointer: &str) -> impl Iterator<Item = &'a Value> {
    value.pointer(pointer).and_then(Value::as_array).into_iter().flatten()
}

fn openai_text(reply: &Value) -> Result<String> {
    let mut text = String::new();
    for part in items(reply, "/output").filter(|item| item["type"] == "message").flat_map(|item| items(item, "/content")) {
        match part["type"].as_str() {
            Some("output_text") => text.push_str(part["text"].as_str().unwrap_or_default()),
            Some("refusal") => return Err(Error::message("OpenAI declined to answer this request.")),
            _ => {}
        }
    }
    Ok(text)
}

fn anthropic_text(reply: &Value) -> Result<String> {
    if reply["stop_reason"] == "refusal" {
        return Err(Error::message("Claude declined to answer this request."));
    }
    Ok(items(reply, "/content").filter(|block| block["type"] == "text").filter_map(|block| block["text"].as_str()).collect())
}

fn gemini_text(reply: &Value) -> Result<String> {
    if let Some(reason) = reply.pointer("/promptFeedback/blockReason").and_then(Value::as_str) {
        return Err(Error::message(format!("Gemini blocked this request ({reason}).")));
    }
    Ok(items(reply, "/candidates/0/content/parts")
        .filter(|part| part["thought"] != true)
        .filter_map(|part| part["text"].as_str())
        .collect())
}

/// A user-facing message for a failed request. `model` is named when a model was asked for.
pub fn error_message(provider: ApiProvider, status: u16, body: &str, model: Option<&str>) -> String {
    let name = provider.display_name();
    let reply: Value = serde_json::from_str(body).unwrap_or_default();
    let error = &reply["error"];
    let detail = error["message"].as_str().map(str::to_string).unwrap_or_else(|| body.chars().take(200).collect());
    let code = [&error["code"], &error["type"], &error["status"]].iter().filter_map(|v| v.as_str()).collect::<Vec<_>>().join(" ");
    let lower = detail.to_lowercase();
    let out_of_credit = status == 402
        || code.contains("insufficient_quota")
        || code.contains("billing")
        || lower.contains("credit balance")
        || lower.contains("exceeded your current quota");
    let bad_key = matches!(status, 401 | 403) || lower.contains("api key not valid") || lower.contains("invalid api key");
    if out_of_credit {
        format!("{name} says this account is out of credit or quota. Add credit or check billing with {name}, then try again.")
    } else if bad_key {
        format!("{name} did not accept the API key. Check it in Settings, under Accounts.")
    } else if status == 429 {
        format!("{name} is limiting how fast requests can be made. Wait a minute, then try again.")
    } else if status == 404 && model.is_some() {
        format!("{name} does not offer the model \"{}\" to this key. Choose another model in Settings.", model.unwrap_or_default())
    } else if status == 404 {
        format!("{name} did not recognize that address. Check the base URL.")
    } else if status >= 500 {
        format!("{name} is having trouble right now ({status}). Try again in a moment.")
    } else {
        format!("{name} returned an error ({status}). {detail}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::summary;

    fn target(provider: ApiProvider) -> Target<'static> {
        Target { provider, base_url: "http://localhost:11434/v1", key: Some("k") }
    }

    fn header<'a>(request: &'a ApiRequest, name: &str) -> Option<&'a str> {
        request.headers.iter().find(|(n, _)| *n == name).map(|(_, v)| v.as_str())
    }

    #[test]
    fn openai_uses_responses_with_a_strict_schema_for_listed_models() {
        let schema = summary::schema();
        let request = target(ApiProvider::OpenAI).completion_request("gpt-6-luna", "sys", "user", Some(&schema));
        let body = request.body.clone().unwrap();
        assert_eq!(request.url, "https://api.openai.com/v1/responses");
        assert_eq!(header(&request, "authorization"), Some("Bearer k"));
        assert_eq!((body["instructions"].as_str(), body["input"].as_str()), (Some("sys"), Some("user")));
        assert_eq!(body["text"]["format"]["strict"], true);
        assert_eq!(body["reasoning"]["effort"], "medium");
        let custom = target(ApiProvider::OpenAI).completion_request("my-fine-tune", "sys", "user", Some(&schema)).body.unwrap();
        assert!(custom.get("text").is_none() && custom.get("reasoning").is_none());
    }

    #[test]
    fn anthropic_sends_headers_schema_and_effort() {
        let schema = summary::schema();
        let request = target(ApiProvider::Anthropic).completion_request("claude-opus-5", "sys", "user", Some(&schema));
        let body = request.body.as_ref().unwrap();
        assert_eq!(request.url, "https://api.anthropic.com/v1/messages");
        assert_eq!(header(&request, "x-api-key"), Some("k"));
        assert_eq!(header(&request, "anthropic-version"), Some("2023-06-01"));
        assert_eq!(header(&request, "anthropic-beta"), Some("server-side-fallback-2026-07-01"));
        assert_eq!(body["system"], "sys");
        assert_eq!(body["messages"][0]["content"], "user");
        assert_eq!(body["output_config"]["format"]["type"], "json_schema");
        assert_eq!(body["output_config"]["effort"], "low");
        assert!(body["max_tokens"].as_u64().is_some());
        let haiku = target(ApiProvider::Anthropic).completion_request("claude-haiku-4-5", "s", "u", None);
        assert!(haiku.body.as_ref().unwrap().get("output_config").is_none());
        assert_eq!(header(&haiku, "anthropic-beta"), None);
    }

    #[test]
    fn gemini_uses_a_json_schema_or_json_mode() {
        let schema = summary::schema();
        let request = target(ApiProvider::Gemini).completion_request("gemini-3.8-flash", summary::SYSTEM, "user", Some(&schema));
        let body = request.body.clone().unwrap();
        assert_eq!(request.url, "https://generativelanguage.googleapis.com/v1beta/models/gemini-3.8-flash:generateContent");
        assert_eq!(header(&request, "x-goog-api-key"), Some("k"));
        assert_eq!(body["systemInstruction"]["parts"][0]["text"], summary::SYSTEM);
        assert_eq!(body["contents"][0]["parts"][0]["text"], "user");
        assert_eq!(body["generationConfig"]["responseJsonSchema"], schema);
        assert_eq!(body["generationConfig"]["thinkingConfig"]["thinkingLevel"], "low");
        let custom = target(ApiProvider::Gemini).completion_request("gemini-2.5-pro", summary::SYSTEM, "u", Some(&schema)).body.unwrap();
        assert_eq!(custom["generationConfig"], json!({"responseMimeType": "application/json"}));
        let answer = target(ApiProvider::Gemini).completion_request("gemini-3.8-flash", "Answer plainly.", "u", None).body.unwrap();
        assert_eq!(answer["generationConfig"], json!({"thinkingConfig": {"thinkingLevel": "low"}}));
    }

    #[test]
    fn custom_servers_use_chat_completions_and_an_optional_key() {
        let keyless = Target { provider: ApiProvider::Compatible, base_url: "http://localhost:11434/v1", key: None };
        let request = keyless.completion_request("llama3.2", "sys", "user", Some(&summary::schema()));
        let body = request.body.clone().unwrap();
        assert_eq!(request.url, "http://localhost:11434/v1/chat/completions");
        assert!(request.headers.is_empty());
        assert_eq!(body["messages"][0], json!({"role": "system", "content": "sys"}));
        assert!(body.get("response_format").is_none());
        assert_eq!(keyless.models_request(), ApiRequest { url: "http://localhost:11434/v1/models".into(), headers: vec![], body: None });
        assert_eq!(header(&target(ApiProvider::Compatible).models_request(), "authorization"), Some("Bearer k"));
    }

    #[test]
    fn key_checks_list_models() {
        assert_eq!(target(ApiProvider::OpenAI).models_request().url, "https://api.openai.com/v1/models");
        assert_eq!(target(ApiProvider::Anthropic).models_request().url, "https://api.anthropic.com/v1/models");
        assert_eq!(target(ApiProvider::Gemini).models_request().url, "https://generativelanguage.googleapis.com/v1beta/models");
    }

    #[test]
    fn reads_the_text_of_each_reply_shape() {
        let openai = r#"{"output":[{"type":"reasoning","summary":[]},{"type":"message","content":[{"type":"output_text","text":"{\"a\":"},{"type":"output_text","text":"1}"}]}]}"#;
        assert_eq!(reply_text(ApiProvider::OpenAI, openai).unwrap(), "{\"a\":1}");
        let anthropic = r#"{"content":[{"type":"thinking","thinking":""},{"type":"text","text":"Friday."}],"stop_reason":"end_turn"}"#;
        assert_eq!(reply_text(ApiProvider::Anthropic, anthropic).unwrap(), "Friday.");
        let gemini = r#"{"candidates":[{"content":{"parts":[{"text":"plan","thought":true},{"text":"Done."}]},"finishReason":"STOP"}]}"#;
        assert_eq!(reply_text(ApiProvider::Gemini, gemini).unwrap(), "Done.");
        let chat = r#"{"choices":[{"message":{"role":"assistant","content":"```json\n{}\n```"}}]}"#;
        assert_eq!(reply_text(ApiProvider::Compatible, chat).unwrap(), "```json\n{}\n```");
    }

    #[test]
    fn fenced_replies_still_parse_as_notes() {
        let note = r#"{"title":"T","tldr":"S","sections":[],"decisions":[],"action_items":[]}"#;
        let chat = json!({"choices": [{"message": {"content": format!("```json\n{note}\n```")}}]}).to_string();
        assert_eq!(summary::parse_note(&reply_text(ApiProvider::Compatible, &chat).unwrap()).unwrap().title, "T");
    }

    #[test]
    fn refusals_blocks_and_empty_replies_are_errors() {
        let refusal = r#"{"output":[{"type":"message","content":[{"type":"refusal","refusal":"no"}]}]}"#;
        assert!(reply_text(ApiProvider::OpenAI, refusal).is_err());
        assert!(reply_text(ApiProvider::Anthropic, r#"{"content":[],"stop_reason":"refusal"}"#).is_err());
        let blocked = r#"{"promptFeedback":{"blockReason":"SAFETY"}}"#;
        assert!(reply_text(ApiProvider::Gemini, blocked).unwrap_err().to_string().contains("SAFETY"));
        assert!(reply_text(ApiProvider::Compatible, r#"{"choices":[{"message":{"content":""}}]}"#).is_err());
        assert!(reply_text(ApiProvider::Compatible, r#"{"error":{"message":"No endpoints found"}}"#).is_err());
        assert!(reply_text(ApiProvider::OpenAI, "<html>").is_err());
    }

    #[test]
    fn errors_say_what_to_do() {
        let wrong = error_message(ApiProvider::OpenAI, 401, r#"{"error":{"message":"Incorrect API key provided: sk-...","code":"invalid_api_key"}}"#, None);
        assert!(wrong.contains("did not accept the API key"));
        let gemini_key = error_message(ApiProvider::Gemini, 400, r#"{"error":{"code":400,"message":"API key not valid. Please pass a valid API key.","status":"INVALID_ARGUMENT"}}"#, None);
        assert!(gemini_key.contains("did not accept the API key"));
        let quota = error_message(ApiProvider::OpenAI, 429, r#"{"error":{"message":"You exceeded your current quota","type":"insufficient_quota","code":"insufficient_quota"}}"#, None);
        assert!(quota.contains("out of credit"));
        let credit = error_message(ApiProvider::Anthropic, 400, r#"{"type":"error","error":{"type":"invalid_request_error","message":"Your credit balance is too low to access the Anthropic API."}}"#, None);
        assert!(credit.contains("out of credit"));
        let limited = error_message(ApiProvider::Anthropic, 429, r#"{"type":"error","error":{"type":"rate_limit_error","message":"Number of request tokens has exceeded your per-minute rate limit"}}"#, None);
        assert!(limited.contains("Wait a minute"));
        let missing = error_message(ApiProvider::Gemini, 404, r#"{"error":{"code":404,"message":"models/x is not found","status":"NOT_FOUND"}}"#, Some("x"));
        assert!(missing.contains("\"x\""));
        assert!(error_message(ApiProvider::Anthropic, 529, r#"{"error":{"type":"overloaded_error","message":"Overloaded"}}"#, None).contains("Try again in a moment"));
        assert!(error_message(ApiProvider::Compatible, 400, "bad model", None).ends_with("bad model"));
    }
}
