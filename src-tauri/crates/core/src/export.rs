//! A meeting as a file: Markdown with the notes and transcript, or the transcript as plain text.
use chrono::{DateTime, Local, Utc};
use serde::Deserialize;

use super::models::{Meeting, MeetingNote, TranscriptSegment};
use super::transcript;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ExportFormat {
    /// Notes, then the transcript.
    Markdown,
    /// The transcript alone.
    Text,
}

impl ExportFormat {
    pub fn extension(self) -> &'static str {
        match self {
            ExportFormat::Markdown => "md",
            ExportFormat::Text => "txt",
        }
    }

    pub fn filter_name(self) -> &'static str {
        match self {
            ExportFormat::Markdown => "Markdown",
            ExportFormat::Text => "Plain text",
        }
    }
}

pub fn render(format: ExportFormat, meeting: &Meeting, note: Option<&MeetingNote>, segments: &[TranscriptSegment]) -> String {
    match format {
        ExportFormat::Markdown => markdown(meeting, note, segments),
        ExportFormat::Text => {
            let heading = format!("{}\n{}\n\n", meeting.title, about(meeting));
            heading + &transcript::render(segments) + "\n"
        }
    }
}

fn markdown(meeting: &Meeting, note: Option<&MeetingNote>, segments: &[TranscriptSegment]) -> String {
    let mut blocks = match note {
        // The note's own heading is the title; the details go right under it.
        Some(note) => {
            let body = note.markdown();
            let (heading, rest) = body.split_once("\n\n").unwrap_or((&body, ""));
            vec![heading.to_string(), format!("*{}*", about(meeting)), rest.trim_end().to_string()]
        }
        None => vec![format!("# {}", meeting.title), format!("*{}*", about(meeting))],
    };
    blocks.retain(|block| !block.is_empty());
    if !segments.is_empty() {
        let lines: Vec<String> = segments
            .iter()
            .map(|s| format!("**[{}] {}:** {}", transcript::timestamp(s.start), s.speaker_label(), s.text))
            .collect();
        blocks.push(format!("## Transcript\n\n{}", lines.join("\n\n")));
    }
    blocks.join("\n\n") + "\n"
}

/// "Zoom · Sep 21, 2026 at 2:05 PM · 42 min"
fn about(meeting: &Meeting) -> String {
    let mut parts = vec![meeting.app.display_name().to_string(), local(meeting.started_at)];
    if let Some(seconds) = meeting.duration_seconds() {
        parts.push(format!("{} min", (seconds / 60.0).round().max(1.0)));
    }
    parts.join(" · ")
}

fn local(date: DateTime<Utc>) -> String {
    date.with_timezone(&Local).format("%b %-d, %Y at %-I:%M %p").to_string()
}

/// The meeting title, without characters Finder or other systems reject.
pub fn file_name(meeting: &Meeting, format: ExportFormat) -> String {
    let cleaned: String = meeting
        .title
        .chars()
        .map(|c| if matches!(c, '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|') || c.is_control() { ' ' } else { c })
        .collect();
    let cleaned: String = cleaned.split_whitespace().collect::<Vec<_>>().join(" ").chars().take(120).collect();
    let stem = cleaned.trim_start_matches('.').trim();
    let stem = if stem.is_empty() { "Meeting" } else { stem };
    format!("{stem}.{}", format.extension())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{ActionItem, Speaker};

    fn meeting(title: &str) -> Meeting {
        let json = format!(
            r#"{{"app":"zoom","id":"A","startedAt":"2026-09-21T14:05:00Z","endedAt":"2026-09-21T14:47:00Z","status":"done","title":{}}}"#,
            serde_json::to_string(title).unwrap()
        );
        serde_json::from_str(&json).unwrap()
    }

    fn segments() -> Vec<TranscriptSegment> {
        vec![TranscriptSegment::new(Speaker::Me, 1.0, 3.0, "Let's ship Friday."), TranscriptSegment::new(Speaker::Them, 65.0, 66.0, "Agreed.")]
    }

    #[test]
    fn markdown_has_notes_details_and_transcript() {
        let note = MeetingNote {
            title: "Launch".into(),
            tldr: "Ship Friday.".into(),
            sections: vec![],
            decisions: vec![],
            action_items: vec![ActionItem { owner: None, task: "Tell sales".into(), done: false }],
        };
        let text = render(ExportFormat::Markdown, &meeting("Launch"), Some(&note), &segments());
        assert!(text.starts_with("# Launch\n\n*Zoom · "), "{text}");
        assert!(text.contains("· 42 min*\n\nShip Friday.\n\n## Action items\n- [ ] Tell sales\n\n## Transcript\n\n**[00:01] Me:** Let's ship Friday.\n\n**[01:05] Them:** Agreed.\n"));
    }

    #[test]
    fn markdown_without_notes_or_transcript_still_has_a_heading() {
        let text = render(ExportFormat::Markdown, &meeting("Standup"), None, &[]);
        assert!(text.starts_with("# Standup\n\n*Zoom · "));
        assert!(!text.contains("Transcript"));
    }

    #[test]
    fn text_is_the_transcript() {
        let text = render(ExportFormat::Text, &meeting("Standup"), None, &segments());
        assert!(text.starts_with("Standup\nZoom · "));
        assert!(text.ends_with("\n\n[00:01] Me: Let's ship Friday.\n[01:05] Them: Agreed.\n"));
    }

    #[test]
    fn file_names_are_safe() {
        assert_eq!(file_name(&meeting("Q4: plan / review?"), ExportFormat::Markdown), "Q4 plan review.md");
        assert_eq!(file_name(&meeting(" ..."), ExportFormat::Text), "Meeting.txt");
        assert_eq!(file_name(&meeting(&"x".repeat(200)), ExportFormat::Text).len(), 124);
    }
}
