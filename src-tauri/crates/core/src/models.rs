//! Data model. Field names and encodings match the Swift app's files, so existing meetings load unchanged.
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Speaker {
    Me,
    Them,
}

impl Speaker {
    pub fn raw(self) -> &'static str {
        match self {
            Speaker::Me => "me",
            Speaker::Them => "them",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Speaker::Me => "Me",
            Speaker::Them => "Them",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TranscriptSegment {
    pub speaker: Speaker,
    pub start: f64,
    pub end: f64,
    pub text: String,
    #[serde(rename = "speakerID", default, skip_serializing_if = "Option::is_none")]
    pub speaker_id: Option<String>,
    #[serde(rename = "speakerName", default, skip_serializing_if = "Option::is_none")]
    pub speaker_name: Option<String>,
}

impl TranscriptSegment {
    pub fn new(speaker: Speaker, start: f64, end: f64, text: impl Into<String>) -> Self {
        Self { speaker, start, end, text: text.into(), speaker_id: None, speaker_name: None }
    }

    pub fn speaker_key(&self) -> String {
        format!("{}:{}", self.speaker.raw(), self.speaker_id.as_deref().unwrap_or("source"))
    }

    pub fn speaker_label(&self) -> String {
        if let Some(name) = &self.speaker_name {
            return name.clone();
        }
        match &self.speaker_id {
            Some(id) => format!("Speaker {id}"),
            None => self.speaker.label().to_string(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum MeetingApp {
    Zoom,
    GoogleMeet,
    Manual,
}

impl MeetingApp {
    pub fn display_name(self) -> &'static str {
        match self {
            MeetingApp::Zoom => "Zoom",
            MeetingApp::GoogleMeet => "Google Meet",
            MeetingApp::Manual => "Recording",
        }
    }

    /// The title a meeting has until its calendar event or its notes name it.
    pub fn default_title(self) -> String {
        match self {
            MeetingApp::Manual => "New meeting".to_string(),
            app => format!("{} meeting", app.display_name()),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum MeetingStatus {
    Recording,
    Transcribing,
    Summarizing,
    Done,
    Failed,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Meeting {
    /// Uppercase UUID, also the name of the meeting's folder.
    pub id: String,
    pub title: String,
    pub app: MeetingApp,
    #[serde(with = "iso8601")]
    pub started_at: DateTime<Utc>,
    #[serde(with = "iso8601::option", default, skip_serializing_if = "Option::is_none")]
    pub ended_at: Option<DateTime<Utc>>,
    pub status: MeetingStatus,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error_message: Option<String>,
    #[serde(with = "iso8601::option", default, skip_serializing_if = "Option::is_none")]
    pub archived_at: Option<DateTime<Utc>>,
    /// Labels for organizing meetings. Omitted when empty, so untagged meetings read and write unchanged.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tags: Vec<String>,
    /// The shared folder this meeting belongs to, if any.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub folder_id: Option<String>,
    /// Names from the calendar event, without the person recording. Omitted when empty, like `tags`.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub attendees: Vec<String>,
}

impl Meeting {
    /// Still named generically, so a calendar event or the written notes may name it.
    pub fn has_default_title(&self) -> bool {
        self.title == self.app.default_title()
    }

    pub fn is_archived(&self) -> bool {
        self.archived_at.is_some()
    }

    pub fn duration_seconds(&self) -> Option<f64> {
        self.ended_at.map(|end| (end - self.started_at).num_milliseconds() as f64 / 1000.0)
    }

    /// A copy with a new status. Clears the error, as the Swift `with(...)` did.
    pub fn with_status(&self, status: MeetingStatus) -> Self {
        Self { status, error_message: None, ..self.clone() }
    }

    pub fn failed(&self, message: impl Into<String>) -> Self {
        Self { status: MeetingStatus::Failed, error_message: Some(message.into()), ..self.clone() }
    }

    pub fn ended(&self, at: DateTime<Utc>, status: MeetingStatus) -> Self {
        Self { ended_at: Some(at), status, error_message: None, ..self.clone() }
    }

    pub fn titled(&self, title: impl Into<String>) -> Self {
        Self { title: title.into(), ..self.clone() }
    }

    pub fn tagged(&self, tags: Vec<String>) -> Self {
        Self { tags, ..self.clone() }
    }

    pub fn with_attendees(&self, attendees: Vec<String>) -> Self {
        Self { attendees, ..self.clone() }
    }

    pub fn in_folder(&self, folder_id: Option<String>) -> Self {
        Self { folder_id, ..self.clone() }
    }

    pub fn archived(&self, archived: bool, now: DateTime<Utc>) -> Self {
        let archived_at = if archived { self.archived_at.or(Some(now)) } else { None };
        Self { archived_at, ..self.clone() }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ActionItem {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub owner: Option<String>,
    pub task: String,
    /// Checked off in the notes. Omitted while open, so older notes read unchanged.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub done: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct NoteSection {
    pub heading: String,
    pub bullets: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MeetingNote {
    pub title: String,
    pub tldr: String,
    pub sections: Vec<NoteSection>,
    pub decisions: Vec<String>,
    pub action_items: Vec<ActionItem>,
}

impl MeetingNote {
    pub fn markdown(&self) -> String {
        let bulleted = |heading: &str, items: Vec<String>| {
            std::iter::once(format!("## {heading}")).chain(items).collect::<Vec<_>>().join("\n")
        };
        let mut blocks = vec![format!("# {}", self.title), self.tldr.clone()];
        for section in &self.sections {
            blocks.push(bulleted(&section.heading, section.bullets.iter().map(|b| format!("- {b}")).collect()));
        }
        if !self.decisions.is_empty() {
            blocks.push(bulleted("Decisions", self.decisions.iter().map(|d| format!("- {d}")).collect()));
        }
        if !self.action_items.is_empty() {
            let lines = self
                .action_items
                .iter()
                .map(|item| {
                    let mark = if item.done { "x" } else { " " };
                    match &item.owner {
                        Some(owner) => format!("- [{mark}] **{owner}** — {}", item.task),
                        None => format!("- [{mark}] {}", item.task),
                    }
                })
                .collect();
            blocks.push(bulleted("Action items", lines));
        }
        blocks.join("\n\n") + "\n"
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MeetingShare {
    pub id: String,
    pub includes_transcript: bool,
    pub url: String,
}

/// ISO 8601 dates at whole-second precision, as Swift's `.iso8601` strategy writes them.
pub mod iso8601 {
    use chrono::{DateTime, SecondsFormat, Utc};
    use serde::{Deserialize, Deserializer, Serializer};

    pub fn serialize<S: Serializer>(date: &DateTime<Utc>, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(&date.to_rfc3339_opts(SecondsFormat::Secs, true))
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(d: D) -> Result<DateTime<Utc>, D::Error> {
        let text = String::deserialize(d)?;
        DateTime::parse_from_rfc3339(&text).map(|date| date.with_timezone(&Utc)).map_err(serde::de::Error::custom)
    }

    pub mod option {
        use super::*;

        pub fn serialize<S: Serializer>(date: &Option<DateTime<Utc>>, s: S) -> Result<S::Ok, S::Error> {
            match date {
                Some(date) => super::serialize(date, s),
                None => s.serialize_none(),
            }
        }

        pub fn deserialize<'de, D: Deserializer<'de>>(d: D) -> Result<Option<DateTime<Utc>>, D::Error> {
            #[derive(Deserialize)]
            struct Wrapper(#[serde(with = "super")] DateTime<Utc>);
            Ok(Option::<Wrapper>::deserialize(d)?.map(|w| w.0))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_a_meeting_written_by_the_swift_app() {
        let json = r#"{
          "app" : "googleMeet",
          "id" : "0B5F1C9E-6C8B-4A1E-9F1D-2C3B4A5D6E7F",
          "startedAt" : "2026-09-21T14:05:00Z",
          "status" : "done",
          "title" : "Roadmap review"
        }"#;
        let meeting: Meeting = serde_json::from_str(json).unwrap();
        assert_eq!(meeting.app, MeetingApp::GoogleMeet);
        assert_eq!(meeting.ended_at, None);
        let written = serde_json::to_string(&meeting).unwrap();
        assert!(written.contains(r#""startedAt":"2026-09-21T14:05:00Z""#));
        assert!(!written.contains("endedAt"));
        assert!(meeting.tags.is_empty());
        assert!(!written.contains("tags"));
        assert!(meeting.attendees.is_empty());
        assert!(!written.contains("attendees"));
    }

    #[test]
    fn tags_round_trip() {
        let meeting: Meeting =
            serde_json::from_str(r#"{"app":"manual","id":"A","startedAt":"2026-09-21T14:05:00Z","status":"done","title":"T","tags":["Acme"]}"#)
                .unwrap();
        let retagged = meeting.tagged(vec!["Acme".into(), "Hiring".into()]);
        assert!(serde_json::to_string(&retagged).unwrap().contains(r#""tags":["Acme","Hiring"]"#));
    }

    #[test]
    fn attendees_round_trip() {
        let meeting: Meeting =
            serde_json::from_str(r#"{"app":"zoom","id":"A","startedAt":"2026-09-21T14:05:00Z","status":"done","title":"T","attendees":["Ada"]}"#)
                .unwrap();
        assert_eq!(meeting.attendees, vec!["Ada"]);
        let invited = meeting.with_attendees(vec!["Ada".into(), "Grace".into()]);
        assert!(serde_json::to_string(&invited).unwrap().contains(r#""attendees":["Ada","Grace"]"#));
    }

    #[test]
    fn default_titles_name_the_app() {
        assert_eq!(MeetingApp::Manual.default_title(), "New meeting");
        assert_eq!(MeetingApp::GoogleMeet.default_title(), "Google Meet meeting");
    }

    #[test]
    fn speaker_labels_prefer_names_then_numbers() {
        let mut segment = TranscriptSegment::new(Speaker::Them, 0.0, 1.0, "hi");
        assert_eq!(segment.speaker_label(), "Them");
        assert_eq!(segment.speaker_key(), "them:source");
        segment.speaker_id = Some("2".into());
        assert_eq!(segment.speaker_label(), "Speaker 2");
        segment.speaker_name = Some("Ada".into());
        assert_eq!(segment.speaker_label(), "Ada");
        assert_eq!(segment.speaker_key(), "them:2");
    }

    #[test]
    fn markdown_lists_decisions_and_owned_actions() {
        let note = MeetingNote {
            title: "Launch".into(),
            tldr: "Ship Friday.".into(),
            sections: vec![NoteSection { heading: "Plan".into(), bullets: vec!["Freeze Thursday".into()] }],
            decisions: vec!["Ship Friday".into()],
            action_items: vec![
                ActionItem { owner: Some("Me".into()), task: "Tell sales".into(), done: true },
                ActionItem { owner: None, task: "Write notes".into(), done: false },
            ],
        };
        assert_eq!(
            note.markdown(),
            "# Launch\n\nShip Friday.\n\n## Plan\n- Freeze Thursday\n\n## Decisions\n- Ship Friday\n\n## Action items\n- [x] **Me** — Tell sales\n- [ ] Write notes\n"
        );
    }

    #[test]
    fn action_items_are_open_unless_marked_done() {
        let open: ActionItem = serde_json::from_str(r#"{"task":"t"}"#).unwrap();
        assert!(!open.done);
        assert_eq!(serde_json::to_string(&open).unwrap(), r#"{"task":"t"}"#);
        let done = ActionItem { done: true, ..open };
        assert_eq!(serde_json::to_string(&done).unwrap(), r#"{"task":"t","done":true}"#);
    }

    #[test]
    fn archiving_keeps_the_first_archive_date() {
        let meeting: Meeting = serde_json::from_str(
            r#"{"app":"manual","id":"A","startedAt":"2026-09-21T14:05:00Z","status":"done","title":"T","archivedAt":"2026-09-21T15:00:00Z"}"#,
        )
        .unwrap();
        let again = meeting.archived(true, Utc::now());
        assert_eq!(again.archived_at, meeting.archived_at);
        assert!(!meeting.archived(false, Utc::now()).is_archived());
    }
}
