use std::collections::HashSet;

use super::models::{Speaker, TranscriptSegment};

const JOIN_GAP: f64 = 2.0;
const ECHO_SIMILARITY: f64 = 0.6;

/// Orders segments by time, drops mic segments that merely echo the system audio,
/// and joins neighbouring segments from the same speaker.
pub fn merge(segments: &[TranscriptSegment]) -> Vec<TranscriptSegment> {
    let cleaned: Vec<TranscriptSegment> = segments
        .iter()
        .map(|s| TranscriptSegment { text: s.text.trim().to_string(), ..s.clone() })
        .filter(|s| !s.text.is_empty())
        .collect();
    let remote: Vec<&TranscriptSegment> = cleaned.iter().filter(|s| s.speaker == Speaker::Them).collect();
    let mut ordered: Vec<TranscriptSegment> = cleaned
        .iter()
        .filter(|s| s.speaker == Speaker::Them || !remote.iter().any(|r| is_echo(s, r)))
        .cloned()
        .collect();
    ordered.sort_by(|a, b| a.start.total_cmp(&b.start).then(a.speaker.raw().cmp(b.speaker.raw())));

    let mut result: Vec<TranscriptSegment> = Vec::with_capacity(ordered.len());
    for segment in ordered {
        match result.last_mut() {
            Some(last) if last.speaker_key() == segment.speaker_key() && segment.start - last.end <= JOIN_GAP => {
                *last = TranscriptSegment {
                    end: last.end.max(segment.end),
                    text: format!("{} {}", last.text, segment.text),
                    ..last.clone()
                };
            }
            _ => result.push(segment),
        }
    }
    result
}

pub fn render(segments: &[TranscriptSegment]) -> String {
    segments
        .iter()
        .map(|s| format!("[{}] {}: {}", timestamp(s.start), s.speaker_label(), s.text))
        .collect::<Vec<_>>()
        .join("\n")
}

pub fn timestamp(seconds: f64) -> String {
    let total = seconds.max(0.0) as u64;
    let (h, m, s) = (total / 3600, (total % 3600) / 60, total % 60);
    if h > 0 { format!("{h}:{m:02}:{s:02}") } else { format!("{m:02}:{s:02}") }
}

fn is_echo(mic: &TranscriptSegment, system: &TranscriptSegment) -> bool {
    if !(mic.start < system.end && system.start < mic.end) {
        return false;
    }
    let (a, b) = (words(&mic.text), words(&system.text));
    if a.is_empty() || b.is_empty() {
        return false;
    }
    a.intersection(&b).count() as f64 / a.union(&b).count() as f64 >= ECHO_SIMILARITY
}

fn words(text: &str) -> HashSet<String> {
    text.to_lowercase()
        .split(|c: char| !c.is_alphanumeric())
        .filter(|w| !w.is_empty())
        .map(str::to_string)
        .collect()
}

/// Splits text on line boundaries into chunks of at most `budget` characters.
/// A single line longer than the budget becomes its own chunk.
pub fn chunks(text: &str, budget: usize) -> Vec<String> {
    let mut groups: Vec<(Vec<&str>, usize)> = Vec::new();
    for line in text.split('\n') {
        let length = line.chars().count();
        match groups.last_mut() {
            Some((lines, size)) if *size + length <= budget => {
                lines.push(line);
                *size += length + 1;
            }
            _ => groups.push((vec![line], length + 1)),
        }
    }
    groups.into_iter().map(|(lines, _)| lines.join("\n")).collect()
}

#[derive(Debug, Clone, PartialEq)]
pub struct TranscriptWord {
    pub text: String,
    pub start: f64,
    pub end: f64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct SpeakerTurn {
    pub id: String,
    pub start: f64,
    pub end: f64,
}

/// Assigns each word to a speaker by its midpoint. Overlapping and uncovered speech stays unassigned.
pub fn align_speakers(words: &[TranscriptWord], turns: &[SpeakerTurn], offset: f64) -> Vec<TranscriptSegment> {
    let segments: Vec<TranscriptSegment> = words
        .iter()
        .map(|word| {
            let midpoint = (word.start + word.end) / 2.0;
            let speakers: HashSet<&str> =
                turns.iter().filter(|t| t.start <= midpoint && midpoint < t.end).map(|t| t.id.as_str()).collect();
            TranscriptSegment {
                speaker_id: if speakers.len() == 1 { speakers.into_iter().next().map(str::to_string) } else { None },
                ..TranscriptSegment::new(Speaker::Them, offset + word.start, offset + word.end, word.text.clone())
            }
        })
        .collect();
    merge(&segments)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn seg(speaker: Speaker, start: f64, end: f64, text: &str) -> TranscriptSegment {
        TranscriptSegment::new(speaker, start, end, text)
    }

    #[test]
    fn joins_close_segments_from_the_same_speaker() {
        let merged = merge(&[seg(Speaker::Me, 0.0, 2.0, "hello"), seg(Speaker::Me, 3.0, 4.0, "there")]);
        assert_eq!(merged.len(), 1);
        assert_eq!(merged[0].text, "hello there");
        assert_eq!(merged[0].end, 4.0);
    }

    #[test]
    fn keeps_far_apart_segments_separate_and_ordered() {
        let merged = merge(&[
            seg(Speaker::Them, 10.0, 12.0, "later"),
            seg(Speaker::Me, 0.0, 2.0, "first"),
            seg(Speaker::Me, 5.0, 6.0, "second"),
        ]);
        let texts: Vec<_> = merged.iter().map(|s| s.text.as_str()).collect();
        assert_eq!(texts, ["first", "second", "later"]);
    }

    #[test]
    fn drops_microphone_echo_of_call_audio() {
        let merged = merge(&[
            seg(Speaker::Them, 0.0, 4.0, "Can everyone see my screen now?"),
            seg(Speaker::Me, 0.5, 4.2, "can everyone see my screen"),
        ]);
        assert_eq!(merged.len(), 1);
        assert_eq!(merged[0].speaker, Speaker::Them);
    }

    #[test]
    fn drops_blank_segments() {
        assert!(merge(&[seg(Speaker::Me, 0.0, 1.0, "   ")]).is_empty());
    }

    #[test]
    fn formats_timestamps() {
        assert_eq!(timestamp(65.4), "01:05");
        assert_eq!(timestamp(3725.0), "1:02:05");
    }

    #[test]
    fn renders_speaker_lines() {
        let rendered = render(&[seg(Speaker::Me, 61.0, 62.0, "Hi")]);
        assert_eq!(rendered, "[01:01] Me: Hi");
    }

    #[test]
    fn chunks_on_line_boundaries() {
        assert_eq!(chunks("aaa\nbbb\nccc", 7), ["aaa\nbbb", "ccc"]);
        assert_eq!(chunks("a-very-long-line\nb", 4), ["a-very-long-line", "b"]);
    }

    #[test]
    fn aligns_words_to_the_single_covering_speaker() {
        let words = vec![
            TranscriptWord { text: "hi".into(), start: 0.0, end: 0.4 },
            TranscriptWord { text: "there".into(), start: 0.5, end: 0.9 },
            TranscriptWord { text: "yes".into(), start: 5.0, end: 5.4 },
        ];
        let turns = vec![
            SpeakerTurn { id: "1".into(), start: 0.0, end: 1.0 },
            SpeakerTurn { id: "2".into(), start: 4.5, end: 6.0 },
        ];
        let segments = align_speakers(&words, &turns, 10.0);
        assert_eq!(segments.len(), 2);
        assert_eq!(segments[0].text, "hi there");
        assert_eq!(segments[0].speaker_id.as_deref(), Some("1"));
        assert_eq!(segments[0].start, 10.0);
        assert_eq!(segments[1].speaker_id.as_deref(), Some("2"));
    }

    #[test]
    fn leaves_overlapping_speech_unassigned() {
        let words = vec![TranscriptWord { text: "both".into(), start: 0.0, end: 1.0 }];
        let turns = vec![
            SpeakerTurn { id: "1".into(), start: 0.0, end: 2.0 },
            SpeakerTurn { id: "2".into(), start: 0.0, end: 2.0 },
        ];
        assert_eq!(align_speakers(&words, &turns, 0.0)[0].speaker_id, None);
    }
}
