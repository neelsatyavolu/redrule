//! Answering questions about one meeting from its notes and transcript.
use std::collections::HashSet;

use serde::Deserialize;

use super::error::{Error, Result};
use super::models::{Meeting, MeetingNote, TranscriptSegment};
use super::summary::SummaryProvider;
use super::transcript;

/// How much meeting text (notes, earlier answers and transcript) an account model is given.
pub const ACCOUNT_BUDGET: usize = 240_000;
/// Earlier questions sent along, so a follow-up like "why?" makes sense.
const HISTORY: usize = 6;
/// Earlier questions and answers are cut to this many characters each.
const EXCHANGE_CHARS: usize = 4_000;
/// Longest question accepted.
pub const MAX_QUESTION: usize = 2_000;
/// A long transcript is cut into passages of about this size and the most relevant are kept.
const PASSAGE: usize = 3_000;

pub const SYSTEM: &str = r#"You answer questions about one meeting, using only its notes and transcript.
"Me" is the person who recorded the meeting and is asking. Numbered speaker labels are estimated voice identities, not real names. "Them" is unassigned call audio, possibly several people.
Rules:
- Answer from the meeting only. If it does not say, answer that it was not discussed. Never invent names, numbers, dates or commitments.
- The transcript comes from speech recognition: silently fix obvious mis-hearings.
- Be brief: a sentence or two, or a short list. Start with the answer, not a restatement of the question.
- Plain text only. For a list, put each item on its own line starting with "- ". No headings, bold or tables.
- When a quote or moment matters, give its time as it appears in the transcript, like [12:34]."#;

/// An earlier question and its answer in the same conversation.
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct Exchange {
    pub question: String,
    pub answer: String,
}

/// The prompt for `question`: the notes, the transcript (or its most relevant passages when it is
/// longer than `budget` allows), then the conversation so far.
pub fn prompt(
    meeting: &Meeting,
    note: Option<&MeetingNote>,
    segments: &[TranscriptSegment],
    history: &[Exchange],
    question: &str,
    budget: usize,
) -> Result<String> {
    let rendered = transcript::render(&transcript::merge(segments));
    if rendered.is_empty() {
        return Err(Error::EmptyTranscript);
    }
    let notes = note.map(|note| format!("Notes written after the meeting:\n{}\n\n", note.markdown())).unwrap_or_default();
    let earlier = &history[history.len().saturating_sub(HISTORY)..];
    let conversation: String = earlier
        .iter()
        .map(|exchange| format!("Question: {}\nAnswer: {}\n\n", clip(&exchange.question), clip(&exchange.answer)))
        .collect();
    let heading = format!("Meeting \"{}\" on {}.\n\n", meeting.title, meeting.app.display_name());
    let used = [&heading, &notes, &conversation, question].iter().map(|part| part.chars().count()).sum::<usize>() + 64;
    let (label, body) = excerpt(&rendered, question, budget.saturating_sub(used));
    Ok(format!("{heading}{notes}{label}:\n{body}\n\n{conversation}Question: {}", question.trim()))
}

/// The whole transcript when it fits in `budget` characters. Otherwise the passages that share
/// the most words with the question, kept in meeting order with gaps marked.
fn excerpt(rendered: &str, question: &str, budget: usize) -> (&'static str, String) {
    if rendered.chars().count() <= budget {
        return ("Transcript", rendered.to_string());
    }
    // Small budgets get small passages, so one passage never crowds out the rest.
    let passages = transcript::chunks(rendered, PASSAGE.min(budget / 8).max(1));
    let wanted = words(question);
    let mut ranked: Vec<(usize, usize)> =
        passages.iter().enumerate().map(|(index, passage)| (index, words(passage).intersection(&wanted).count())).collect();
    // Most relevant first; ties go to the start of the meeting, where context is usually set.
    ranked.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));

    let mut kept = Vec::new();
    let mut used = 0;
    for (index, _) in ranked {
        let size = passages[index].chars().count() + 5;
        if used + size > budget {
            continue;
        }
        used += size;
        kept.push(index);
    }
    kept.sort_unstable();
    let mut body = String::new();
    for (position, index) in kept.iter().enumerate() {
        if position > 0 {
            body.push_str(if kept[position - 1] + 1 == *index { "\n" } else { "\n[…]\n" });
        }
        body.push_str(&passages[*index]);
    }
    ("Transcript excerpts most related to the question, in order", body)
}

fn clip(text: &str) -> String {
    text.trim().chars().take(EXCHANGE_CHARS).collect()
}

/// Lowercase words of four or more letters, which skips most filler.
fn words(text: &str) -> HashSet<String> {
    text.split(|c: char| !c.is_alphanumeric())
        .filter(|word| word.chars().count() >= 4)
        .map(str::to_lowercase)
        .collect()
}

/// Asks `provider` one question about the meeting and returns its plain-text answer.
pub async fn answer<P: SummaryProvider>(provider: &P, prompt: &str) -> Result<String> {
    let reply = provider.complete(SYSTEM, prompt, None).await?;
    let reply = reply.trim();
    if reply.is_empty() {
        return Err(Error::message("The model returned an empty answer. Try asking again."));
    }
    Ok(reply.to_string())
}

#[cfg(test)]
mod tests {
    use chrono::Utc;

    use super::*;
    use crate::models::{MeetingApp, MeetingStatus, Speaker};

    fn meeting() -> Meeting {
        Meeting {
            id: "A".into(),
            title: "Roadmap".into(),
            app: MeetingApp::Zoom,
            started_at: Utc::now(),
            ended_at: None,
            status: MeetingStatus::Done,
            error_message: None,
            archived_at: None,
            tags: vec![],
            folder_id: None,
            attendees: vec![],
        }
    }

    fn line(start: f64, text: &str) -> TranscriptSegment {
        TranscriptSegment::new(Speaker::Me, start, start + 1.0, text)
    }

    fn note() -> MeetingNote {
        MeetingNote {
            title: "Roadmap".into(),
            tldr: "Ship CSV first.".into(),
            sections: vec![],
            decisions: vec![],
            action_items: vec![],
        }
    }

    #[test]
    fn short_meetings_are_sent_whole_with_notes_and_recent_history() {
        let history: Vec<Exchange> =
            (0..8).map(|i| Exchange { question: format!("q{i}"), answer: format!("a{i}") }).collect();
        let text = prompt(&meeting(), Some(&note()), &[line(0.0, "hello there")], &history, " What next? ", ACCOUNT_BUDGET).unwrap();
        assert!(text.contains("Ship CSV first."));
        assert!(text.contains("Transcript:\n[00:00] Me: hello there"));
        assert!(!text.contains("q1\n") && text.contains("Question: q2\nAnswer: a2"));
        assert!(text.ends_with("Question: What next?"));
    }

    #[test]
    fn empty_transcripts_are_rejected() {
        let result = prompt(&meeting(), None, &[], &[], "What?", ACCOUNT_BUDGET);
        assert!(matches!(result, Err(Error::EmptyTranscript)));
    }

    #[test]
    fn long_transcripts_keep_the_passages_that_match_the_question_in_order() {
        let filler = "we talked about nothing much at all today".repeat(20);
        let rendered = [
            filler.clone(),
            "the budget for hiring is two engineers".into(),
            filler.clone(),
            "hiring starts in january with the budget approved".into(),
            filler,
        ]
        .join("\n");
        let (label, body) = excerpt(&rendered, "What is the hiring budget?", 200);
        assert!(label.starts_with("Transcript excerpts"));
        assert_eq!(body, "the budget for hiring is two engineers\n[…]\nhiring starts in january with the budget approved");
    }

    #[test]
    fn transcripts_within_budget_are_not_cut() {
        let (label, body) = excerpt("a\nb", "anything", 100);
        assert_eq!((label, body.as_str()), ("Transcript", "a\nb"));
    }

    #[tokio::test]
    async fn answers_are_trimmed_and_empty_ones_rejected() {
        struct Reply(&'static str);
        impl SummaryProvider for Reply {
            async fn complete(&self, system: &str, _user: &str, schema: Option<&serde_json::Value>) -> Result<String> {
                assert_eq!(system, SYSTEM);
                assert!(schema.is_none());
                Ok(self.0.to_string())
            }
        }
        assert_eq!(answer(&Reply("  Friday.\n"), "p").await.unwrap(), "Friday.");
        assert!(answer(&Reply(" "), "p").await.is_err());
    }
}
