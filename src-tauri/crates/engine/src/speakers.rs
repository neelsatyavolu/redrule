//! Keeps voice identities consistent across windows. Each window is diarized on its own, so its
//! speakers are only locally numbered; this matches them to the recording's known voices by
//! embedding similarity. One registry per recording: voices are never carried across meetings.

use minutes_core::transcript::SpeakerTurn;

/// Minimum cosine similarity for a local speaker to count as a known voice. In testing the same
/// voice scored ~0.94 across sentences and distinct voices 0.5–0.59.
const SAME_SPEAKER: f32 = 0.6;

/// A speaker found in one window, in order of first appearance within it.
pub(crate) struct LocalSpeaker {
    /// (start, end) seconds relative to the window start.
    pub turns: Vec<(f64, f64)>,
    pub embedding: Option<Vec<f32>>,
    /// False when the speaker talked too briefly for the embedding to define a new voice.
    pub reliable: bool,
}

#[derive(Default)]
pub(crate) struct SpeakerRegistry {
    /// Running sums of unit-length embeddings: the same direction as their mean, which is all
    /// cosine similarity needs. Index + 1 is the speaker id.
    centroids: Vec<Vec<f32>>,
}

impl SpeakerRegistry {
    /// Labels a window's speakers with global ids, ordered by start time. The diarizer already judged
    /// the window's speakers to be different people, so no two of them share an id. Speakers that
    /// cannot be identified are left out, so their words stay unassigned.
    pub(crate) fn turns(&mut self, speakers: &[LocalSpeaker]) -> Vec<SpeakerTurn> {
        let mut taken = Vec::new();
        let mut turns = Vec::new();
        for speaker in speakers {
            let Some(index) = self.identify(speaker, &taken) else { continue };
            taken.push(index);
            turns.extend(speaker.turns.iter().map(|&(start, end)| SpeakerTurn { id: id(index), start, end }));
        }
        turns.sort_by(|a, b| a.start.total_cmp(&b.start));
        turns
    }

    /// Returns the centroid index for a local speaker, never one in `taken`, or None when it cannot
    /// be identified. Reliable speakers join the closest known voice above the threshold or become a
    /// new voice. Brief speakers are only mapped to the closest known voice, never create or move one.
    fn identify(&mut self, speaker: &LocalSpeaker, taken: &[usize]) -> Option<usize> {
        let embedding = normalized(speaker.embedding.as_deref()?)?;
        let best = self
            .centroids
            .iter()
            .enumerate()
            .filter(|(index, sum)| sum.len() == embedding.len() && !taken.contains(index))
            .map(|(index, sum)| (index, cosine(sum, &embedding)))
            .max_by(|a, b| a.1.total_cmp(&b.1));

        let index = match best {
            Some((index, _)) if !speaker.reliable => return Some(index),
            None if !speaker.reliable => return None,
            Some((index, similarity)) if similarity >= SAME_SPEAKER => index,
            _ => {
                self.centroids.push(vec![0.0; embedding.len()]);
                self.centroids.len() - 1
            }
        };
        self.centroids[index].iter_mut().zip(&embedding).for_each(|(total, value)| *total += value);
        Some(index)
    }
}

fn id(index: usize) -> String {
    (index + 1).to_string()
}

fn normalized(vector: &[f32]) -> Option<Vec<f32>> {
    let norm = vector.iter().map(|v| v * v).sum::<f32>().sqrt();
    (norm > f32::EPSILON && norm.is_finite()).then(|| vector.iter().map(|v| v / norm).collect())
}

fn cosine(a: &[f32], b: &[f32]) -> f32 {
    let dot: f32 = a.iter().zip(b).map(|(x, y)| x * y).sum();
    let norms = a.iter().map(|v| v * v).sum::<f32>().sqrt() * b.iter().map(|v| v * v).sum::<f32>().sqrt();
    if norms > f32::EPSILON { dot / norms } else { 0.0 }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The id a speaker gets in a window of its own.
    fn identify(registry: &mut SpeakerRegistry, speaker: &LocalSpeaker) -> Option<String> {
        registry.identify(speaker, &[]).map(id)
    }

    fn speaker(embedding: &[f32], reliable: bool) -> LocalSpeaker {
        LocalSpeaker { turns: vec![(0.0, 1.0)], embedding: Some(embedding.to_vec()), reliable }
    }

    #[test]
    fn numbers_new_voices_in_order_of_appearance() {
        let mut registry = SpeakerRegistry::default();
        assert_eq!(identify(&mut registry, &speaker(&[1.0, 0.0, 0.0], true)).as_deref(), Some("1"));
        assert_eq!(identify(&mut registry, &speaker(&[0.0, 1.0, 0.0], true)).as_deref(), Some("2"));
        assert_eq!(identify(&mut registry, &speaker(&[0.0, 0.0, 1.0], true)).as_deref(), Some("3"));
    }

    #[test]
    fn recognises_a_known_voice_in_a_later_window() {
        let mut registry = SpeakerRegistry::default();
        identify(&mut registry, &speaker(&[1.0, 0.1, 0.0], true));
        identify(&mut registry, &speaker(&[0.0, 1.0, 0.1], true));
        // Different scale and a little noise: still the first voice.
        assert_eq!(identify(&mut registry, &speaker(&[3.0, 0.6, 0.3], true)).as_deref(), Some("1"));
        assert_eq!(identify(&mut registry, &speaker(&[0.1, 0.9, 0.0], true)).as_deref(), Some("2"));
    }

    #[test]
    fn centroids_follow_the_running_mean() {
        let mut registry = SpeakerRegistry::default();
        identify(&mut registry, &speaker(&[1.0, 0.0], true));
        identify(&mut registry, &speaker(&[0.8, 0.6], true)); // cos 0.8: same voice, centroid drifts towards it
        // cos to the first sample alone would be 0.45 (new voice); against the mean it is 0.71.
        assert_eq!(identify(&mut registry, &speaker(&[0.45, 0.893], true)).as_deref(), Some("1"));
    }

    #[test]
    fn brief_speakers_map_to_the_closest_voice_without_creating_one() {
        let mut registry = SpeakerRegistry::default();
        assert_eq!(identify(&mut registry, &speaker(&[1.0, 0.0], false)), None);
        identify(&mut registry, &speaker(&[1.0, 0.0], true));
        identify(&mut registry, &speaker(&[0.0, 1.0], true));
        assert_eq!(identify(&mut registry, &speaker(&[-0.2, 0.3], false)).as_deref(), Some("2"));
        assert_eq!(identify(&mut registry, &speaker(&[0.5, -1.0], false)).as_deref(), Some("1"));
        assert_eq!(registry.centroids.len(), 2);
        assert_eq!(registry.centroids[0], [1.0, 0.0]);
    }

    #[test]
    fn labels_window_turns_with_global_ids() {
        let mut registry = SpeakerRegistry::default();
        registry.turns(&[speaker(&[1.0, 0.0], true)]);
        let window = [
            LocalSpeaker { turns: vec![(0.0, 2.0), (5.0, 6.0)], embedding: Some(vec![0.0, 1.0]), reliable: true },
            LocalSpeaker { turns: vec![(2.5, 4.0)], embedding: Some(vec![0.9, 0.1]), reliable: true },
            LocalSpeaker { turns: vec![(4.0, 4.5)], embedding: None, reliable: false },
        ];
        let turns = registry.turns(&window);
        let summary: Vec<_> = turns.iter().map(|t| (t.id.as_str(), t.start)).collect();
        assert_eq!(summary, [("2", 0.0), ("1", 2.5), ("2", 5.0)]);
    }

    #[test]
    fn speakers_in_one_window_never_share_an_id() {
        let mut registry = SpeakerRegistry::default();
        registry.turns(&[speaker(&[1.0, 0.0], true)]);
        // Both resemble voice 1, but the diarizer said they are different people.
        let turns = registry.turns(&[speaker(&[0.9, 0.1], true), speaker(&[0.95, 0.05], true)]);
        let ids: Vec<_> = turns.iter().map(|t| t.id.as_str()).collect();
        assert_eq!(ids, ["1", "2"]);
    }

    #[test]
    fn speakers_without_a_usable_embedding_stay_unassigned() {
        let mut registry = SpeakerRegistry::default();
        identify(&mut registry, &speaker(&[1.0, 0.0], true));
        assert_eq!(identify(&mut registry, &LocalSpeaker { turns: vec![], embedding: None, reliable: true }), None);
        assert_eq!(identify(&mut registry, &speaker(&[0.0, 0.0], true)), None);
    }
}
