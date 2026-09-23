//! Keeps voice identities consistent across windows. Each window is diarized on its own, so its
//! speakers are only locally numbered; this matches them to the recording's known voices by
//! embedding similarity. One registry per recording: voices are never carried across meetings.
//!
//! Matching during the meeting is greedy, so one voice can end up split in two (a cough, a codec
//! change) or a few seconds of noise can become a "speaker". `finish` looks at the whole meeting
//! once it is over and merges or drops those, and the saved transcript uses its answer.

use std::collections::HashMap;

use minutes_core::models::{Speaker, TranscriptSegment};
use minutes_core::transcript::SpeakerTurn;

use crate::catalog::SpeakerTuning;

/// A voice with less speech than this, over the whole meeting, is treated as noise rather than a
/// participant: its words stay unassigned.
const MIN_VOICE_SECONDS: f64 = 10.0;
/// ...unless the meeting is short, where a real participant may speak less than that.
const MIN_VOICE_SHARE: f64 = 0.05;

/// A speaker found in one window, in order of first appearance within it.
pub(crate) struct LocalSpeaker {
    /// (start, end) seconds relative to the window start.
    pub turns: Vec<(f64, f64)>,
    pub embedding: Option<Vec<f32>>,
    /// False when the speaker talked too briefly for the embedding to define a new voice.
    pub reliable: bool,
}

impl LocalSpeaker {
    fn seconds(&self) -> f64 {
        self.turns.iter().map(|(start, end)| end - start).sum()
    }
}

struct Voice {
    /// Running sum of unit-length embeddings: the same direction as their mean, which is all
    /// cosine similarity needs.
    sum: Vec<f32>,
    seconds: f64,
}

pub(crate) struct SpeakerRegistry {
    tuning: SpeakerTuning,
    /// Index + 1 is the speaker id handed out during the meeting.
    voices: Vec<Voice>,
}

impl SpeakerRegistry {
    pub(crate) fn new(tuning: SpeakerTuning) -> Self {
        Self { tuning, voices: Vec::new() }
    }

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

    /// Returns the voice index for a local speaker, never one in `taken`, or None when it cannot
    /// be identified. Reliable speakers join the closest known voice above the threshold or become a
    /// new voice. Brief speakers are only mapped to the closest known voice, never create or move one.
    fn identify(&mut self, speaker: &LocalSpeaker, taken: &[usize]) -> Option<usize> {
        let embedding = normalized(speaker.embedding.as_deref()?)?;
        let best = self
            .voices
            .iter()
            .enumerate()
            .filter(|(index, voice)| voice.sum.len() == embedding.len() && !taken.contains(index))
            .map(|(index, voice)| (index, cosine(&voice.sum, &embedding)))
            .max_by(|a, b| a.1.total_cmp(&b.1));

        let index = match best {
            Some((index, _)) if !speaker.reliable => {
                self.voices[index].seconds += speaker.seconds();
                return Some(index);
            }
            None if !speaker.reliable => return None,
            Some((index, similarity)) if similarity >= self.tuning.same_speaker => index,
            _ => {
                self.voices.push(Voice { sum: vec![0.0; embedding.len()], seconds: 0.0 });
                self.voices.len() - 1
            }
        };
        let voice = &mut self.voices[index];
        voice.sum.iter_mut().zip(&embedding).for_each(|(total, value)| *total += value);
        voice.seconds += speaker.seconds();
        Some(index)
    }

    /// The final speakers, judged over the whole meeting: voices that turn out to be the same person
    /// are merged, voices with barely any speech are dropped, and the rest are renumbered in order
    /// of first appearance.
    pub(crate) fn finish(&self) -> SpeakerMap {
        let mut groups: Vec<Group> = self
            .voices
            .iter()
            .enumerate()
            .map(|(index, voice)| Group { members: vec![index], sum: voice.sum.clone(), seconds: voice.seconds })
            .collect();
        while let Some((a, b)) = closest_pair(&groups).filter(|&(_, _, similarity)| similarity >= self.tuning.merge).map(|(a, b, _)| (a, b)) {
            let absorbed = groups.remove(b);
            groups[a].absorb(absorbed);
        }

        let total: f64 = groups.iter().map(|group| group.seconds).sum();
        let min_seconds = MIN_VOICE_SECONDS.min(total * MIN_VOICE_SHARE);
        groups.retain(|group| group.seconds >= min_seconds);
        groups.sort_by_key(|group| group.members.iter().min().copied());

        let ids: HashMap<String, String> = groups
            .iter()
            .enumerate()
            .flat_map(|(final_index, group)| group.members.iter().map(move |&member| (id(member), id(final_index))))
            .collect();
        let sole = (groups.len() == 1).then(|| id(0));
        SpeakerMap { ids: Some(ids), sole }
    }
}

/// Voices believed to be one person, during the end-of-meeting pass.
struct Group {
    members: Vec<usize>,
    sum: Vec<f32>,
    seconds: f64,
}

impl Group {
    fn absorb(&mut self, other: Group) {
        self.members.extend(other.members);
        if self.sum.len() == other.sum.len() {
            self.sum.iter_mut().zip(&other.sum).for_each(|(total, value)| *total += value);
        }
        self.seconds += other.seconds;
    }
}

/// The two most similar groups, as (lower index, higher index, similarity).
fn closest_pair(groups: &[Group]) -> Option<(usize, usize, f32)> {
    (0..groups.len())
        .flat_map(|a| (a + 1..groups.len()).map(move |b| (a, b)))
        .filter(|&(a, b)| groups[a].sum.len() == groups[b].sum.len())
        .map(|(a, b)| (a, b, cosine(&groups[a].sum, &groups[b].sum)))
        .max_by(|x, y| x.2.total_cmp(&y.2))
}

/// Rewrites the speaker ids handed out during a meeting into the final ones. The default keeps them.
#[derive(Debug, Default, Clone, PartialEq)]
pub(crate) struct SpeakerMap {
    /// Meeting-time id to final id. Ids missing here were dropped as noise.
    ids: Option<HashMap<String, String>>,
    /// Set when the call audio held exactly one voice: every word of it is theirs.
    sole: Option<String>,
}

impl SpeakerMap {
    pub(crate) fn apply(&self, segment: TranscriptSegment) -> TranscriptSegment {
        if segment.speaker != Speaker::Them {
            return segment;
        }
        let speaker_id = match &self.sole {
            Some(sole) => Some(sole.clone()),
            None => segment.speaker_id.as_deref().and_then(|id| self.final_id(id)),
        };
        TranscriptSegment { speaker_id, ..segment }
    }

    #[cfg(test)]
    pub(crate) fn from_ids(pairs: &[(&str, &str)]) -> Self {
        Self { ids: Some(pairs.iter().map(|(from, to)| (from.to_string(), to.to_string())).collect()), sole: None }
    }

    /// The final id for a meeting-time id, or None when that voice was dropped as noise.
    pub(crate) fn final_id(&self, id: &str) -> Option<String> {
        match (&self.sole, &self.ids) {
            (Some(sole), _) => Some(sole.clone()),
            (None, Some(ids)) => ids.get(id).cloned(),
            (None, None) => Some(id.to_string()),
        }
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

    const TUNING: SpeakerTuning = SpeakerTuning { cluster_threshold: 0.4, same_speaker: 0.6, merge: 0.7 };

    /// The id a speaker gets in a window of its own.
    fn identify(registry: &mut SpeakerRegistry, speaker: &LocalSpeaker) -> Option<String> {
        registry.identify(speaker, &[]).map(id)
    }

    fn speaker(embedding: &[f32], reliable: bool) -> LocalSpeaker {
        LocalSpeaker { turns: vec![(0.0, 1.0)], embedding: Some(embedding.to_vec()), reliable }
    }

    #[test]
    fn numbers_new_voices_in_order_of_appearance() {
        let mut registry = SpeakerRegistry::new(TUNING);
        assert_eq!(identify(&mut registry, &speaker(&[1.0, 0.0, 0.0], true)).as_deref(), Some("1"));
        assert_eq!(identify(&mut registry, &speaker(&[0.0, 1.0, 0.0], true)).as_deref(), Some("2"));
        assert_eq!(identify(&mut registry, &speaker(&[0.0, 0.0, 1.0], true)).as_deref(), Some("3"));
    }

    #[test]
    fn recognises_a_known_voice_in_a_later_window() {
        let mut registry = SpeakerRegistry::new(TUNING);
        identify(&mut registry, &speaker(&[1.0, 0.1, 0.0], true));
        identify(&mut registry, &speaker(&[0.0, 1.0, 0.1], true));
        // Different scale and a little noise: still the first voice.
        assert_eq!(identify(&mut registry, &speaker(&[3.0, 0.6, 0.3], true)).as_deref(), Some("1"));
        assert_eq!(identify(&mut registry, &speaker(&[0.1, 0.9, 0.0], true)).as_deref(), Some("2"));
    }

    #[test]
    fn centroids_follow_the_running_mean() {
        let mut registry = SpeakerRegistry::new(TUNING);
        identify(&mut registry, &speaker(&[1.0, 0.0], true));
        identify(&mut registry, &speaker(&[0.8, 0.6], true)); // cos 0.8: same voice, centroid drifts towards it
        // cos to the first sample alone would be 0.45 (new voice); against the mean it is 0.71.
        assert_eq!(identify(&mut registry, &speaker(&[0.45, 0.893], true)).as_deref(), Some("1"));
    }

    #[test]
    fn brief_speakers_map_to_the_closest_voice_without_creating_one() {
        let mut registry = SpeakerRegistry::new(TUNING);
        assert_eq!(identify(&mut registry, &speaker(&[1.0, 0.0], false)), None);
        identify(&mut registry, &speaker(&[1.0, 0.0], true));
        identify(&mut registry, &speaker(&[0.0, 1.0], true));
        assert_eq!(identify(&mut registry, &speaker(&[-0.2, 0.3], false)).as_deref(), Some("2"));
        assert_eq!(identify(&mut registry, &speaker(&[0.5, -1.0], false)).as_deref(), Some("1"));
        assert_eq!(registry.voices.len(), 2);
        assert_eq!(registry.voices[0].sum, [1.0, 0.0]);
    }

    #[test]
    fn labels_window_turns_with_global_ids() {
        let mut registry = SpeakerRegistry::new(TUNING);
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
        let mut registry = SpeakerRegistry::new(TUNING);
        registry.turns(&[speaker(&[1.0, 0.0], true)]);
        // Both resemble voice 1, but the diarizer said they are different people.
        let turns = registry.turns(&[speaker(&[0.9, 0.1], true), speaker(&[0.95, 0.05], true)]);
        let ids: Vec<_> = turns.iter().map(|t| t.id.as_str()).collect();
        assert_eq!(ids, ["1", "2"]);
    }

    #[test]
    fn speakers_without_a_usable_embedding_stay_unassigned() {
        let mut registry = SpeakerRegistry::new(TUNING);
        identify(&mut registry, &speaker(&[1.0, 0.0], true));
        assert_eq!(identify(&mut registry, &LocalSpeaker { turns: vec![], embedding: None, reliable: true }), None);
        assert_eq!(identify(&mut registry, &speaker(&[0.0, 0.0], true)), None);
    }

    /// A reliable speaker talking for `seconds` in a window of its own.
    fn talking(embedding: &[f32], seconds: f64) -> LocalSpeaker {
        LocalSpeaker { turns: vec![(0.0, seconds)], embedding: Some(embedding.to_vec()), reliable: true }
    }

    fn them(speaker_id: Option<&str>) -> TranscriptSegment {
        TranscriptSegment { speaker_id: speaker_id.map(str::to_string), ..TranscriptSegment::new(Speaker::Them, 0.0, 1.0, "hi") }
    }

    fn final_id(map: &SpeakerMap, speaker_id: Option<&str>) -> Option<String> {
        map.apply(them(speaker_id)).speaker_id
    }

    #[test]
    fn one_voice_split_during_the_meeting_is_merged_at_the_end() {
        let mut registry = SpeakerRegistry::new(TUNING);
        registry.turns(&[talking(&[1.0, 0.0, 0.0], 60.0)]);
        // The window's diarizer split the one voice in two, so the second half had to become voice 2.
        registry.turns(&[talking(&[0.95, 0.3, 0.0], 30.0), talking(&[0.9, 0.1, 0.4], 30.0)]);
        assert_eq!(registry.voices.len(), 2);
        let map = registry.finish();
        assert_eq!(final_id(&map, Some("1")).as_deref(), Some("1"));
        assert_eq!(final_id(&map, Some("2")).as_deref(), Some("1"));
    }

    #[test]
    fn distinct_voices_stay_apart_and_are_renumbered_after_noise_is_dropped() {
        let mut registry = SpeakerRegistry::new(TUNING);
        registry.turns(&[talking(&[1.0, 0.0, 0.0], 3.0)]); // a few seconds of noise
        registry.turns(&[talking(&[0.0, 1.0, 0.0], 120.0)]);
        registry.turns(&[talking(&[0.0, 0.0, 1.0], 90.0)]);
        let map = registry.finish();
        assert_eq!(final_id(&map, Some("1")), None);
        assert_eq!(final_id(&map, Some("2")).as_deref(), Some("1"));
        assert_eq!(final_id(&map, Some("3")).as_deref(), Some("2"));
        assert_eq!(final_id(&map, None), None);
    }

    #[test]
    fn a_single_remote_voice_gets_every_word_of_the_call_audio() {
        let mut registry = SpeakerRegistry::new(TUNING);
        registry.turns(&[talking(&[1.0, 0.0], 300.0)]);
        registry.turns(&[talking(&[0.0, 1.0], 8.0)]); // a laugh that became a "speaker"
        let map = registry.finish();
        assert_eq!(final_id(&map, Some("2")).as_deref(), Some("1"));
        assert_eq!(final_id(&map, None).as_deref(), Some("1"));
        let mine = TranscriptSegment::new(Speaker::Me, 0.0, 1.0, "hi");
        assert_eq!(map.apply(mine.clone()), mine);
    }

    #[test]
    fn short_meetings_keep_briefly_heard_participants() {
        let mut registry = SpeakerRegistry::new(TUNING);
        registry.turns(&[talking(&[1.0, 0.0], 40.0)]);
        registry.turns(&[talking(&[0.0, 1.0], 6.0)]);
        let map = registry.finish();
        assert_eq!(final_id(&map, Some("2")).as_deref(), Some("2"));
        assert_eq!(final_id(&map, None), None);
    }
}
