//! A read-only Model Context Protocol server over the meeting store, so local agents
//! (Claude Code, Codex, Grok Build) can list, search and read notes. Pure: one JSON-RPC
//! message in, at most one out; the caller owns stdin and stdout.
use serde_json::{Value, json};

use super::export::{self, ExportFormat};
use super::models::Meeting;
use super::search::{self, SearchDocument};
use super::store::MeetingStore;
use super::transcript;

pub const PROTOCOL_VERSION: &str = "2025-06-18";
const DEFAULT_LIMIT: usize = 50;
const MAX_LIMIT: usize = 500;

/// Answers one JSON-RPC message. Notifications get no reply.
pub fn handle(store: &MeetingStore, message: &Value) -> Option<Value> {
    let id = message.get("id")?.clone();
    let method = message.get("method").and_then(Value::as_str).unwrap_or_default();
    let params = message.get("params").cloned().unwrap_or(Value::Null);
    let outcome = match method {
        "initialize" => Ok(initialize(&params)),
        "ping" => Ok(json!({})),
        "tools/list" => Ok(json!({ "tools": tools() })),
        "tools/call" => Ok(call(store, &params)),
        _ => Err(json!({ "code": -32601, "message": format!("Unknown method: {method}") })),
    };
    Some(match outcome {
        Ok(result) => json!({ "jsonrpc": "2.0", "id": id, "result": result }),
        Err(error) => json!({ "jsonrpc": "2.0", "id": id, "error": error }),
    })
}

/// The reply for a line that isn't JSON.
pub fn parse_error() -> Value {
    json!({ "jsonrpc": "2.0", "id": null, "error": { "code": -32700, "message": "Parse error" } })
}

fn initialize(params: &Value) -> Value {
    let version = params.get("protocolVersion").and_then(Value::as_str).unwrap_or(PROTOCOL_VERSION);
    json!({
        "protocolVersion": version,
        "capabilities": { "tools": {} },
        "serverInfo": { "name": "redrule", "title": "Redrule", "version": env!("CARGO_PKG_VERSION") },
        "instructions": "Meeting notes and transcripts recorded with Redrule on this Mac. Use search_meetings or list_meetings to find a meeting id, then get_meeting to read it."
    })
}

fn tools() -> Value {
    let read_only = json!({ "readOnlyHint": true, "openWorldHint": false });
    json!([
        {
            "name": "list_meetings",
            "description": "Lists Redrule meetings, newest first: id, date, title, app and tags.",
            "inputSchema": { "type": "object", "properties": {
                "limit": { "type": "integer", "minimum": 1, "maximum": MAX_LIMIT, "description": "How many to return (default 50)." },
                "include_archived": { "type": "boolean", "description": "Include archived meetings (default false)." }
            } },
            "annotations": read_only
        },
        {
            "name": "search_meetings",
            "description": "Finds meetings whose title, tags, attendees, notes or transcript contain every word of the query. Returns ids with a matching snippet, newest first.",
            "inputSchema": { "type": "object", "required": ["query"], "properties": {
                "query": { "type": "string", "description": "Words to look for." },
                "limit": { "type": "integer", "minimum": 1, "maximum": MAX_LIMIT, "description": "How many to return (default 50)." }
            } },
            "annotations": read_only
        },
        {
            "name": "get_meeting",
            "description": "Returns one meeting as Markdown: summary, sections, decisions, action items and, by default, the full transcript.",
            "inputSchema": { "type": "object", "required": ["id"], "properties": {
                "id": { "type": "string", "description": "Meeting id from list_meetings or search_meetings." },
                "include_transcript": { "type": "boolean", "description": "Include the transcript (default true)." }
            } },
            "annotations": read_only
        }
    ])
}

fn call(store: &MeetingStore, params: &Value) -> Value {
    let arguments = params.get("arguments").cloned().unwrap_or_else(|| json!({}));
    let name = params.get("name").and_then(Value::as_str).unwrap_or_default();
    let text = match name {
        "list_meetings" => list_meetings(store, &arguments),
        "search_meetings" => search_meetings(store, &arguments),
        "get_meeting" => get_meeting(store, &arguments),
        _ => Err(format!("Unknown tool: {name}")),
    };
    match text {
        Ok(text) => json!({ "content": [{ "type": "text", "text": text }] }),
        Err(message) => json!({ "content": [{ "type": "text", "text": message }], "isError": true }),
    }
}

fn limit(arguments: &Value) -> usize {
    arguments.get("limit").and_then(Value::as_u64).map_or(DEFAULT_LIMIT, |n| (n as usize).clamp(1, MAX_LIMIT))
}

fn meetings(store: &MeetingStore) -> Result<Vec<Meeting>, String> {
    store.list().map_err(|e| format!("Redrule's meetings could not be read. {e}"))
}

fn summary_line(meeting: &Meeting) -> String {
    let mut parts = vec![meeting.id.clone(), meeting.started_at.format("%Y-%m-%d").to_string(), meeting.title.clone(), meeting.app.display_name().to_string()];
    if !meeting.tags.is_empty() {
        parts.push(format!("tags: {}", meeting.tags.join(", ")));
    }
    if meeting.archived_at.is_some() {
        parts.push("archived".into());
    }
    format!("- {}", parts.join(" · "))
}

fn list_meetings(store: &MeetingStore, arguments: &Value) -> Result<String, String> {
    let archived = arguments.get("include_archived").and_then(Value::as_bool).unwrap_or(false);
    let found: Vec<String> = meetings(store)?
        .iter()
        .filter(|m| archived || m.archived_at.is_none())
        .take(limit(arguments))
        .map(summary_line)
        .collect();
    Ok(if found.is_empty() { "No meetings yet.".into() } else { found.join("\n") })
}

fn search_meetings(store: &MeetingStore, arguments: &Value) -> Result<String, String> {
    let terms = search::terms(arguments.get("query").and_then(Value::as_str).unwrap_or_default());
    if terms.is_empty() {
        return Err("Give a query to search for.".into());
    }
    let found: Vec<String> = meetings(store)?
        .iter()
        .filter_map(|meeting| {
            let note = store.note(&meeting.id).ok().flatten();
            let segments = store.transcript(&meeting.id).unwrap_or_default();
            let hit = SearchDocument::new(meeting, note.as_ref(), &segments).search(&terms)?;
            Some(match hit.snippet {
                Some(snippet) => format!("{}\n  {snippet}", summary_line(meeting)),
                None => summary_line(meeting),
            })
        })
        .take(limit(arguments))
        .collect();
    Ok(if found.is_empty() { "No meetings match.".into() } else { found.join("\n") })
}

fn get_meeting(store: &MeetingStore, arguments: &Value) -> Result<String, String> {
    let id = arguments.get("id").and_then(Value::as_str).unwrap_or_default().trim().to_uppercase();
    let meeting = store.meeting(&id).map_err(|_| format!("No meeting has the id {id}. Use list_meetings or search_meetings to find one."))?;
    let note = store.note(&id).ok().flatten();
    let segments = match arguments.get("include_transcript").and_then(Value::as_bool).unwrap_or(true) {
        true => transcript::merge(&store.transcript(&id).unwrap_or_default()),
        false => Vec::new(),
    };
    Ok(format!("Meeting id: {id}\n\n{}", export::render(ExportFormat::Markdown, &meeting, note.as_ref(), &segments)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::TranscriptSegment;

    const ID: &str = "0F3C1B2A-1111-4222-8333-444455556666";

    fn store_with_meeting() -> (tempfile::TempDir, MeetingStore) {
        let dir = tempfile::tempdir().unwrap();
        let store = MeetingStore::new(dir.path().to_path_buf()).unwrap();
        let meeting: Meeting = serde_json::from_value(json!({
            "id": ID, "title": "Roadmap review", "app": "zoom",
            "startedAt": "2026-09-01T17:00:00Z", "endedAt": "2026-09-01T17:30:00Z", "status": "done", "tags": ["planning"]
        }))
        .unwrap();
        store.save(&meeting).unwrap();
        let segment: TranscriptSegment = serde_json::from_value(json!({ "speaker": "them", "start": 1.0, "end": 4.0, "text": "Ship the beta on Friday." })).unwrap();
        store.save_transcript(&[segment], ID).unwrap();
        (dir, store)
    }

    fn call_tool(store: &MeetingStore, name: &str, arguments: Value) -> Value {
        let request = json!({ "jsonrpc": "2.0", "id": 7, "method": "tools/call", "params": { "name": name, "arguments": arguments } });
        handle(store, &request).unwrap()["result"].clone()
    }

    fn text(result: &Value) -> &str {
        result["content"][0]["text"].as_str().unwrap()
    }

    #[test]
    fn initialize_echoes_the_protocol_version_and_names_the_server() {
        let (_dir, store) = store_with_meeting();
        let reply = handle(&store, &json!({ "jsonrpc": "2.0", "id": 1, "method": "initialize", "params": { "protocolVersion": "2025-03-26" } })).unwrap();
        assert_eq!(reply["id"], 1);
        assert_eq!(reply["result"]["protocolVersion"], "2025-03-26");
        assert_eq!(reply["result"]["serverInfo"]["name"], "redrule");
    }

    #[test]
    fn notifications_get_no_reply_and_unknown_methods_an_error() {
        let (_dir, store) = store_with_meeting();
        assert!(handle(&store, &json!({ "jsonrpc": "2.0", "method": "notifications/initialized" })).is_none());
        let reply = handle(&store, &json!({ "jsonrpc": "2.0", "id": 2, "method": "resources/list" })).unwrap();
        assert_eq!(reply["error"]["code"], -32601);
    }

    #[test]
    fn lists_three_read_only_tools() {
        let (_dir, store) = store_with_meeting();
        let reply = handle(&store, &json!({ "jsonrpc": "2.0", "id": 3, "method": "tools/list" })).unwrap();
        let names: Vec<&str> = reply["result"]["tools"].as_array().unwrap().iter().map(|t| t["name"].as_str().unwrap()).collect();
        assert_eq!(names, ["list_meetings", "search_meetings", "get_meeting"]);
    }

    #[test]
    fn list_meetings_shows_id_date_title_and_tags() {
        let (_dir, store) = store_with_meeting();
        let result = call_tool(&store, "list_meetings", json!({}));
        assert_eq!(text(&result), format!("- {ID} · 2026-09-01 · Roadmap review · Zoom · tags: planning"));
    }

    #[test]
    fn search_finds_words_in_the_transcript() {
        let (_dir, store) = store_with_meeting();
        assert!(text(&call_tool(&store, "search_meetings", json!({ "query": "beta friday" }))).contains("Ship the beta on Friday."));
        assert_eq!(text(&call_tool(&store, "search_meetings", json!({ "query": "budget" }))), "No meetings match.");
        assert_eq!(call_tool(&store, "search_meetings", json!({}))["isError"], true);
    }

    #[test]
    fn get_meeting_returns_markdown_with_an_optional_transcript() {
        let (_dir, store) = store_with_meeting();
        let full = call_tool(&store, "get_meeting", json!({ "id": ID.to_lowercase() }));
        assert!(text(&full).contains("# Roadmap review"));
        assert!(text(&full).contains("## Transcript"));
        let short = call_tool(&store, "get_meeting", json!({ "id": ID, "include_transcript": false }));
        assert!(!text(&short).contains("## Transcript"));
    }

    #[test]
    fn get_meeting_rejects_unknown_and_malformed_ids() {
        let (_dir, store) = store_with_meeting();
        assert_eq!(call_tool(&store, "get_meeting", json!({ "id": "../../etc" }))["isError"], true);
        assert_eq!(call_tool(&store, "get_meeting", json!({ "id": "9F3C1B2A-1111-4222-8333-444455556666" }))["isError"], true);
    }
}
