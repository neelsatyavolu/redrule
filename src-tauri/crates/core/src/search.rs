//! Full-text search over a meeting's title, tags, attendees, notes and transcript. Pure: callers supply the text.
use serde::Serialize;

use super::models::{Meeting, MeetingNote, TranscriptSegment};

/// Characters of context kept on each side of the first match in a snippet.
const CONTEXT: usize = 48;

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SearchHit {
    pub id: String,
    /// The passage that matched, shortened around the match. None when only the title matched.
    pub snippet: Option<String>,
}

/// A meeting's searchable passages, lowercased once so each keystroke only scans.
#[derive(Debug, Clone, PartialEq)]
pub struct SearchDocument {
    id: String,
    /// The title comes first; it matches but never becomes the snippet, since the list already shows it.
    passages: Vec<String>,
    lowered: Vec<String>,
}

impl SearchDocument {
    pub fn new(meeting: &Meeting, note: Option<&MeetingNote>, transcript: &[TranscriptSegment]) -> Self {
        let mut passages = vec![meeting.title.clone()];
        passages.extend(meeting.tags.iter().cloned());
        passages.extend(meeting.attendees.iter().cloned());
        if let Some(note) = note {
            passages.push(note.tldr.clone());
            for section in &note.sections {
                passages.push(section.heading.clone());
                passages.extend(section.bullets.iter().cloned());
            }
            passages.extend(note.decisions.iter().cloned());
            passages.extend(note.action_items.iter().map(|item| match &item.owner {
                Some(owner) => format!("{owner}: {}", item.task),
                None => item.task.clone(),
            }));
        }
        passages.extend(transcript.iter().map(|segment| format!("{}: {}", segment.speaker_label(), segment.text)));
        passages.retain(|passage| !passage.trim().is_empty());
        let lowered = passages.iter().map(|passage| lower(passage)).collect();
        Self { id: meeting.id.clone(), passages, lowered }
    }

    /// Every term must appear somewhere in the meeting. The snippet comes from the passage holding the most terms.
    pub fn search(&self, terms: &[String]) -> Option<SearchHit> {
        if terms.is_empty() || !terms.iter().all(|term| self.lowered.iter().any(|passage| passage.contains(term.as_str()))) {
            return None;
        }
        let best = self
            .lowered
            .iter()
            .enumerate()
            .skip(1)
            .map(|(index, passage)| (index, terms.iter().filter(|term| passage.contains(term.as_str())).count()))
            .filter(|&(_, count)| count > 0)
            .max_by_key(|&(index, count)| (count, std::cmp::Reverse(index)));
        let snippet = best.map(|(index, _)| snippet(&self.passages[index], &self.lowered[index], terms));
        Some(SearchHit { id: self.id.clone(), snippet })
    }
}

/// Lowercase words of the query, without duplicates.
pub fn terms(query: &str) -> Vec<String> {
    let mut terms: Vec<String> = Vec::new();
    for term in lower(query).split_whitespace() {
        if !terms.iter().any(|kept| kept == term) {
            terms.push(term.to_string());
        }
    }
    terms
}

/// One lowercase char per char, so char positions in the lowered text match the original.
fn lower(text: &str) -> String {
    text.chars().map(|c| c.to_lowercase().next().unwrap_or(c)).collect()
}

fn snippet(passage: &str, lowered: &str, terms: &[String]) -> String {
    let byte = terms.iter().filter_map(|term| lowered.find(term.as_str())).min().unwrap_or(0);
    let at = lowered[..byte].chars().count();
    let chars: Vec<char> = passage.chars().collect();
    let start = at.saturating_sub(CONTEXT);
    let end = (at + CONTEXT * 2).min(chars.len());
    let body: String = chars[start..end].iter().collect();
    let body = body.split_whitespace().collect::<Vec<_>>().join(" ");
    let prefix = if start > 0 { "…" } else { "" };
    let suffix = if end < chars.len() { "…" } else { "" };
    format!("{prefix}{body}{suffix}")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{ActionItem, NoteSection, Speaker};

    fn meeting() -> Meeting {
        serde_json::from_str(r#"{"app":"zoom","id":"A","startedAt":"2026-09-21T14:05:00Z","status":"done","title":"Roadmap review","tags":["Acme"]}"#)
            .unwrap()
    }

    fn note() -> MeetingNote {
        MeetingNote {
            title: "Roadmap review".into(),
            tldr: "Agreed the launch plan.".into(),
            sections: vec![NoteSection { heading: "Pricing".into(), bullets: vec!["Keep the free tier at three seats.".into()] }],
            decisions: vec!["Ship on Friday".into()],
            action_items: vec![ActionItem { owner: Some("Ada".into()), task: "Email the design partners".into(), done: false }],
        }
    }

    fn search(document: &SearchDocument, query: &str) -> Option<SearchHit> {
        document.search(&terms(query))
    }

    #[test]
    fn finds_words_in_notes_and_transcripts() {
        let transcript = vec![TranscriptSegment::new(Speaker::Them, 0.0, 2.0, "What did we decide about pricing tiers?")];
        let document = SearchDocument::new(&meeting(), Some(&note()), &transcript);
        assert_eq!(search(&document, "free tier").unwrap().snippet.as_deref(), Some("Keep the free tier at three seats."));
        assert_eq!(search(&document, "DECIDE").unwrap().snippet.as_deref(), Some("Them: What did we decide about pricing tiers?"));
        assert_eq!(search(&document, "ada design").unwrap().snippet.as_deref(), Some("Ada: Email the design partners"));
        assert!(search(&document, "pricing budget").is_none());
    }

    #[test]
    fn finds_attendees() {
        let document = SearchDocument::new(&meeting().with_attendees(vec!["Grace Hopper".into()]), None, &[]);
        assert_eq!(search(&document, "grace").unwrap().snippet.as_deref(), Some("Grace Hopper"));
    }

    #[test]
    fn a_title_match_has_no_snippet() {
        let document = SearchDocument::new(&meeting(), None, &[]);
        assert_eq!(search(&document, "roadmap"), Some(SearchHit { id: "A".into(), snippet: None }));
        assert!(search(&document, "acme").unwrap().snippet.is_some());
        assert!(search(&document, "   ").is_none());
    }

    #[test]
    fn long_passages_are_shortened_around_the_match() {
        let text = format!("{} the budget moved to next quarter {}", "word ".repeat(40), "tail ".repeat(40));
        let transcript = vec![TranscriptSegment::new(Speaker::Me, 0.0, 2.0, text)];
        let snippet = search(&SearchDocument::new(&meeting(), None, &transcript), "budget").unwrap().snippet.unwrap();
        assert!(snippet.starts_with('…') && snippet.ends_with('…'));
        assert!(snippet.contains("the budget moved"));
        assert!(snippet.chars().count() <= CONTEXT * 3 + 2);
    }

    #[test]
    fn lowercasing_keeps_char_positions_for_any_script() {
        let transcript = vec![TranscriptSegment::new(Speaker::Me, 0.0, 2.0, "İstanbul office: Straße plans")];
        let document = SearchDocument::new(&meeting(), None, &transcript);
        assert_eq!(search(&document, "straße").unwrap().snippet.as_deref(), Some("Me: İstanbul office: Straße plans"));
    }

    #[test]
    fn terms_are_lowercase_and_unique() {
        assert_eq!(terms("  Pricing pricing  Q4 "), vec!["pricing".to_string(), "q4".into()]);
    }
}
