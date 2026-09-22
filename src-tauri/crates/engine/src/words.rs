//! Turns the recognizer's sub-word tokens into timed words, so speaker turns can be aligned per word.

use minutes_core::transcript::TranscriptWord;

/// Used for a word's last token when the model gives neither a duration nor a following token.
const LAST_TOKEN_SECONDS: f64 = 0.08;
/// SentencePiece marks the start of a word with this character; sherpa may already have turned it into a space.
const WORD_MARK: char = '\u{2581}';

/// Groups tokens into words. A token starting with a space or `▁` begins a new word; any other token
/// continues the current one. `timestamps` are token start times in seconds; `durations` (TDT models)
/// give each token's length.
pub(crate) fn words(tokens: &[String], timestamps: &[f32], durations: Option<&[f32]>) -> Vec<TranscriptWord> {
    let count = tokens.len().min(timestamps.len());
    let end_of = |i: usize| -> f64 {
        let start = timestamps[i] as f64;
        match durations.and_then(|d| d.get(i)).filter(|d| **d > 0.0) {
            Some(duration) => start + *duration as f64,
            None => timestamps.get(i + 1).map_or(start + LAST_TOKEN_SECONDS, |next| *next as f64),
        }
    };

    let mut result: Vec<TranscriptWord> = Vec::new();
    for i in 0..count {
        let token = &tokens[i];
        let starts_word = token.starts_with([' ', WORD_MARK]);
        let text = token.trim_start_matches([' ', WORD_MARK]);
        match result.last_mut() {
            Some(word) if !starts_word => {
                word.text.push_str(text);
                word.end = end_of(i);
            }
            _ => result.push(TranscriptWord { text: text.to_string(), start: timestamps[i] as f64, end: end_of(i) }),
        }
    }
    result.retain(|word| !word.text.trim().is_empty());
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tokens(list: &[&str]) -> Vec<String> {
        list.iter().map(|t| t.to_string()).collect()
    }

    #[test]
    fn merges_tokens_at_space_boundaries() {
        let result = words(&tokens(&[" Hel", "lo", " world", "."]), &[0.0, 0.2, 0.5, 0.9], Some(&[0.2, 0.2, 0.3, 0.1]));
        assert_eq!(result.len(), 2);
        assert_eq!(result[0].text, "Hello");
        assert_eq!((result[0].start, result[0].end), (0.0, 0.4000000059604645));
        assert_eq!(result[1].text, "world.");
        assert!((result[1].end - 1.0).abs() < 1e-6);
    }

    #[test]
    fn understands_sentencepiece_marks() {
        let result = words(&tokens(&["\u{2581}good", "\u{2581}morn", "ing"]), &[0.0, 0.4, 0.6], None);
        let texts: Vec<_> = result.iter().map(|w| w.text.as_str()).collect();
        assert_eq!(texts, ["good", "morning"]);
    }

    #[test]
    fn falls_back_to_the_next_token_start_without_durations() {
        let result = words(&tokens(&[" a", " b"]), &[1.0, 1.5], None);
        assert_eq!(result[0].end, 1.5);
        assert!((result[1].end - 1.58).abs() < 1e-9);
    }

    #[test]
    fn a_leading_token_without_a_space_still_starts_a_word() {
        let result = words(&tokens(&["Yes", ","]), &[0.0, 0.3], Some(&[0.3, 0.0]));
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].text, "Yes,");
        assert_eq!(result[0].end, 0.30000001192092896 + LAST_TOKEN_SECONDS);
    }

    #[test]
    fn ignores_blank_tokens_and_missing_timestamps() {
        assert!(words(&tokens(&[" ", "\u{2581}"]), &[0.0, 0.1], None).is_empty());
        assert_eq!(words(&tokens(&[" a", " b"]), &[0.0], None).len(), 1);
    }
}
