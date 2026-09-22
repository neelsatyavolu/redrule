//! Summary prompts, response parsing, and the summarizer that drives a provider.
use std::future::Future;

use chrono::{DateTime, Local, Utc};
use serde::Deserialize;
use serde_json::{Value, json};

use super::error::{Error, Result};
use super::models::{ActionItem, Meeting, MeetingApp, MeetingNote, NoteSection, TranscriptSegment};
use super::transcript;

/// Transcripts longer than this are digested chunk by chunk before the final note is written.
pub const CHUNK_BUDGET: usize = 60_000;

pub const SYSTEM: &str = r#"You write meeting notes from a transcript, in the style of a sharp chief of staff.
"Me" is the person who recorded the meeting. Numbered speaker labels are estimated voice identities, not real names. "Them" is unassigned call audio, possibly several people. Named labels were supplied by the user. Do not infer real names from numbered labels.
Rules:
- Report only what was said. Never invent names, numbers, dates or commitments.
- The transcript comes from speech recognition: silently fix obvious mis-hearings, ignore filler and small talk.
- title: 3-7 words naming the meeting's subject, no date.
- tldr: two or three sentences a colleague who missed the meeting could act on.
- sections: 2-6 topic sections in the order discussed, each with concise, specific bullets.
- decisions: things actually agreed. Empty list if none.
- action_items: concrete follow-ups. owner is a name if one was stated, "Me" for the recorder, otherwise an empty string.
Reply with a single JSON object and nothing else, with exactly these keys:
{"title": string, "tldr": string, "sections": [{"heading": string, "bullets": [string]}], "decisions": [string], "action_items": [{"owner": string, "task": string}]}"#;

pub const DIGEST_SYSTEM: &str = "You are condensing one part of a long meeting transcript so that notes can be written later from your digest.
Keep every decision, number, date, name, commitment and open question. Drop filler. Keep the order of discussion.
Reply with plain text bullets only.";

fn started(started_at: DateTime<Utc>) -> String {
    started_at.with_timezone(&Local).format("%b %-d, %Y at %-I:%M %p").to_string()
}

pub fn user_prompt(transcript: &str, app: MeetingApp, started_at: DateTime<Utc>) -> String {
    format!("Meeting on {}, started {}.\n\nTranscript:\n{transcript}", app.display_name(), started(started_at))
}

pub fn user_prompt_from_digests(digests: &[String], app: MeetingApp, started_at: DateTime<Utc>) -> String {
    let body = digests.iter().enumerate().map(|(i, d)| format!("Part {}:\n{d}", i + 1)).collect::<Vec<_>>().join("\n\n");
    format!(
        "Meeting on {}, started {}.\n\nDigests of the transcript, in order:\n{body}",
        app.display_name(),
        started(started_at)
    )
}

/// Strict JSON schema for providers that support structured output.
pub fn schema() -> Value {
    let string = json!({"type": "string"});
    let strings = json!({"type": "array", "items": string});
    let object = |properties: Value| {
        let mut required: Vec<String> = properties.as_object().unwrap().keys().cloned().collect();
        required.sort();
        json!({"type": "object", "properties": properties, "required": required, "additionalProperties": false})
    };
    object(json!({
        "title": string,
        "tldr": string,
        "sections": {"type": "array", "items": object(json!({"heading": string, "bullets": strings}))},
        "decisions": strings,
        "action_items": {"type": "array", "items": object(json!({"owner": string, "task": string}))},
    }))
}

#[derive(Deserialize)]
struct Payload {
    title: String,
    tldr: String,
    sections: Option<Vec<NoteSection>>,
    decisions: Option<Vec<String>>,
    action_items: Option<Vec<PayloadItem>>,
}

#[derive(Deserialize)]
struct PayloadItem {
    owner: Option<String>,
    task: String,
}

/// Parses the model reply, tolerating code fences or prose around the JSON object.
pub fn parse_note(reply: &str) -> Result<MeetingNote> {
    let (Some(start), Some(end)) = (reply.find('{'), reply.rfind('}')) else { return Err(Error::NotJson) };
    if start >= end {
        return Err(Error::NotJson);
    }
    let payload: Payload = serde_json::from_str(&reply[start..=end]).map_err(|_| Error::NotJson)?;
    let action_items = payload
        .action_items
        .unwrap_or_default()
        .into_iter()
        .map(|item| ActionItem {
            owner: item.owner.map(|o| o.trim().to_string()).filter(|o| !o.is_empty()),
            task: item.task,
        })
        .collect();
    Ok(MeetingNote {
        title: payload.title,
        tldr: payload.tldr,
        sections: payload.sections.unwrap_or_default(),
        decisions: payload.decisions.unwrap_or_default(),
        action_items,
    })
}

/// Extracts the output text from a buffered Responses API server-sent-event body.
pub fn codex_output_text(body: &str) -> Result<String> {
    let mut text = String::new();
    for line in body.lines() {
        let Some(data) = line.strip_prefix("data: ").map(str::trim) else { continue };
        if data == "[DONE]" {
            continue;
        }
        let Ok(event) = serde_json::from_str::<Value>(data) else { continue };
        let error = event.get("error").or_else(|| event.pointer("/response/error"));
        if let Some(message) = error.and_then(|e| e.get("message")).and_then(Value::as_str) {
            return Err(Error::message(message));
        }
        if let Some(delta) = event.get("delta").and_then(Value::as_str) {
            text.push_str(delta);
        }
        if event.get("type").and_then(Value::as_str) == Some("response.output_text.done")
            && let Some(done) = event.get("text").and_then(Value::as_str)
        {
            text = done.to_string();
        }
    }
    Ok(text)
}

pub trait SummaryProvider: Send + Sync {
    /// Sends one prompt and returns the model's text. `schema` is a hint for providers with structured output.
    fn complete(&self, system: &str, user: &str, schema: Option<&Value>) -> impl Future<Output = Result<String>> + Send;
}

pub async fn summarize<P: SummaryProvider>(
    provider: &P,
    meeting: &Meeting,
    segments: &[TranscriptSegment],
    chunk_budget: usize,
) -> Result<MeetingNote> {
    let rendered = transcript::render(&transcript::merge(segments));
    if rendered.is_empty() {
        return Err(Error::EmptyTranscript);
    }

    let pieces = transcript::chunks(&rendered, chunk_budget);
    let user = if pieces.len() == 1 {
        user_prompt(&rendered, meeting.app, meeting.started_at)
    } else {
        let mut digests = Vec::with_capacity(pieces.len());
        for piece in &pieces {
            digests.push(provider.complete(DIGEST_SYSTEM, piece, None).await?);
        }
        user_prompt_from_digests(&digests, meeting.app, meeting.started_at)
    };

    let schema = schema();
    let first = provider.complete(SYSTEM, &user, Some(&schema)).await?;
    if let Ok(note) = parse_note(&first) {
        return Ok(note);
    }
    let second = provider.complete(SYSTEM, &user, Some(&schema)).await?;
    if let Ok(note) = parse_note(&second) {
        return Ok(note);
    }
    // Keep whatever the model wrote rather than losing the meeting.
    Ok(MeetingNote {
        title: meeting.title.clone(),
        tldr: second.trim().to_string(),
        sections: vec![],
        decisions: vec![],
        action_items: vec![],
    })
}

#[cfg(test)]
mod tests {
    use std::sync::Mutex;

    use super::*;
    use crate::models::{MeetingStatus, Speaker};

    const NOTE: &str = r#"{"title":"Launch","tldr":"Ship.","sections":[{"heading":"Plan","bullets":["a"]}],"decisions":[],"action_items":[{"owner":" ","task":"t"},{"owner":"Me","task":"u"}]}"#;

    struct Scripted {
        replies: Mutex<Vec<String>>,
        calls: Mutex<Vec<(String, bool)>>,
    }

    impl Scripted {
        fn new(replies: &[&str]) -> Self {
            Self { replies: Mutex::new(replies.iter().rev().map(|s| s.to_string()).collect()), calls: Mutex::default() }
        }
    }

    impl SummaryProvider for Scripted {
        async fn complete(&self, system: &str, _user: &str, schema: Option<&Value>) -> Result<String> {
            self.calls.lock().unwrap().push((system.to_string(), schema.is_some()));
            Ok(self.replies.lock().unwrap().pop().unwrap_or_default())
        }
    }

    fn meeting() -> Meeting {
        Meeting {
            id: "A".into(),
            title: "New meeting".into(),
            app: MeetingApp::Manual,
            started_at: Utc::now(),
            ended_at: None,
            status: MeetingStatus::Summarizing,
            error_message: None,
            archived_at: None,
        }
    }

    fn segments() -> Vec<TranscriptSegment> {
        vec![TranscriptSegment::new(Speaker::Me, 0.0, 1.0, "line one"), TranscriptSegment::new(Speaker::Them, 5.0, 6.0, "line two")]
    }

    #[test]
    fn parses_fenced_json_and_drops_blank_owners() {
        let note = parse_note(&format!("Here you go:\n```json\n{NOTE}\n```")).unwrap();
        assert_eq!(note.title, "Launch");
        assert_eq!(note.action_items[0].owner, None);
        assert_eq!(note.action_items[1].owner.as_deref(), Some("Me"));
        assert!(matches!(parse_note("no json"), Err(Error::NotJson)));
    }

    #[test]
    fn reads_codex_stream_deltas_and_errors() {
        let body = "data: {\"type\":\"response.output_text.delta\",\"delta\":\"He\"}\n\ndata: {\"delta\":\"llo\"}\ndata: [DONE]\n";
        assert_eq!(codex_output_text(body).unwrap(), "Hello");
        let done = "data: {\"delta\":\"x\"}\ndata: {\"type\":\"response.output_text.done\",\"text\":\"Final\"}\n";
        assert_eq!(codex_output_text(done).unwrap(), "Final");
        let failed = "data: {\"type\":\"response.failed\",\"response\":{\"error\":{\"message\":\"Quota\"}}}\n";
        assert_eq!(codex_output_text(failed).unwrap_err().to_string(), "Quota");
    }

    #[test]
    fn schema_requires_every_key() {
        let schema = schema();
        assert_eq!(schema["required"], json!(["action_items", "decisions", "sections", "title", "tldr"]));
        assert_eq!(schema["properties"]["sections"]["items"]["additionalProperties"], json!(false));
    }

    #[tokio::test]
    async fn short_transcripts_are_summarised_in_one_call() {
        let provider = Scripted::new(&[NOTE]);
        let note = summarize(&provider, &meeting(), &segments(), CHUNK_BUDGET).await.unwrap();
        assert_eq!(note.title, "Launch");
        assert_eq!(provider.calls.lock().unwrap().len(), 1);
    }

    #[tokio::test]
    async fn long_transcripts_are_digested_first() {
        let provider = Scripted::new(&["d1", "d2", NOTE]);
        summarize(&provider, &meeting(), &segments(), 20).await.unwrap();
        let calls = provider.calls.lock().unwrap();
        assert_eq!(calls.len(), 3);
        assert_eq!(calls[0], (DIGEST_SYSTEM.to_string(), false));
        assert!(calls[2].1);
    }

    #[tokio::test]
    async fn retries_once_then_keeps_the_raw_reply() {
        let provider = Scripted::new(&["nope", " still nope "]);
        let note = summarize(&provider, &meeting(), &segments(), CHUNK_BUDGET).await.unwrap();
        assert_eq!(note.title, "New meeting");
        assert_eq!(note.tldr, "still nope");
    }

    #[tokio::test]
    async fn empty_transcripts_are_rejected() {
        let provider = Scripted::new(&[]);
        let result = summarize(&provider, &meeting(), &[], CHUNK_BUDGET).await;
        assert!(matches!(result, Err(Error::EmptyTranscript)));
    }
}
