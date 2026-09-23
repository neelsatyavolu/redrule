//! The recording's transcription loop, kept free of devices and models so it can be tested with fakes.
//! Audio arrives per speaker, is windowed per speaker, transcribed, and (for the call audio only)
//! split between the remote participants.

use std::future::Future;
use std::path::Path;
use std::sync::Arc;

use minutes_core::Result;
use minutes_core::models::{Speaker, TranscriptSegment};
use minutes_core::transcript::{SpeakerTurn, align_speakers};
use minutes_core::windower::{AudioWindow, AudioWindower};
use tokio::sync::mpsc::UnboundedReceiver;

use crate::asr::Transcription;
use crate::speakers::SpeakerMap;
use crate::wav::WavWriter;

pub(crate) type SegmentSink = Arc<dyn Fn(TranscriptSegment) + Send + Sync>;
pub(crate) type ErrorSink = Arc<dyn Fn(String) + Send + Sync>;

pub(crate) const SPEAKER_DETECTION_FAILED: &str =
    "Speaker detection is unavailable for this recording; the transcript is still being saved.";

pub(crate) trait Transcribe: Send + Sync + 'static {
    fn transcribe(&self, window: &AudioWindow) -> impl Future<Output = Result<Transcription>> + Send;
}

/// Finds who spoke when in a window of call audio. Turn times are relative to the window start.
pub(crate) trait Diarize: Send + 'static {
    fn turns(&mut self, window: &AudioWindow) -> impl Future<Output = Result<Vec<SpeakerTurn>>> + Send;
    /// The final speaker ids, decided once the whole recording has been heard.
    fn finish(&self) -> SpeakerMap;
}

/// Captured 16 kHz mono audio whose last sample was heard `time` seconds into the recording.
pub(crate) struct Chunk {
    pub speaker: Speaker,
    pub samples: Vec<f32>,
    pub time: f64,
}

pub(crate) struct Worker<T, D> {
    pub transcriber: T,
    pub diarizer: D,
    pub on_segment: SegmentSink,
    pub on_error: ErrorSink,
}

/// Per-recording state of the loop.
struct Session {
    me: AudioWindower,
    them: AudioWindower,
    reported_error: bool,
    diarization_failed: bool,
    segments: Vec<TranscriptSegment>,
}

impl<T: Transcribe, D: Diarize> Worker<T, D> {
    /// Runs until `input` closes, then transcribes what is left (call audio first, as the Swift app did)
    /// and returns every segment in the order produced, with the final speaker ids. Segments sent
    /// live carry provisional ids. With `audio_folder`, keeps `me.wav` and `them.wav`.
    pub(crate) async fn run(
        mut self,
        mut input: UnboundedReceiver<Chunk>,
        audio_folder: Option<&Path>,
    ) -> Vec<TranscriptSegment> {
        let mut writers = audio_folder
            .map(|folder| (WavWriter::create(&folder.join("me.wav")), WavWriter::create(&folder.join("them.wav"))));
        let mut session = Session {
            me: AudioWindower::new(),
            them: AudioWindower::new(),
            reported_error: false,
            diarization_failed: false,
            segments: Vec::new(),
        };

        while let Some(chunk) = input.recv().await {
            if let Some((me, them)) = writers.as_mut() {
                let writer = if chunk.speaker == Speaker::Me { me } else { them };
                writer.write(&chunk.samples);
            }
            let windower = if chunk.speaker == Speaker::Me { &mut session.me } else { &mut session.them };
            for window in windower.append(&chunk.samples, chunk.time) {
                self.transcribe(&mut session, &window, chunk.speaker).await;
            }
        }
        if let Some((me, them)) = writers {
            me.finalize();
            them.finalize();
        }
        for speaker in [Speaker::Them, Speaker::Me] {
            let windower = if speaker == Speaker::Me { &mut session.me } else { &mut session.them };
            if let Some(window) = windower.flush() {
                self.transcribe(&mut session, &window, speaker).await;
            }
        }
        let speakers = self.diarizer.finish();
        session.segments.into_iter().map(|segment| speakers.apply(segment)).collect()
    }

    async fn transcribe(&mut self, session: &mut Session, window: &AudioWindow, speaker: Speaker) {
        let result = match self.transcriber.transcribe(window).await {
            Ok(result) => result,
            Err(error) => {
                if !session.reported_error {
                    (self.on_error)(error.to_string());
                }
                session.reported_error = true;
                return;
            }
        };
        if result.text.is_empty() {
            return;
        }
        let whole = || vec![TranscriptSegment::new(speaker, window.start, window.end(), result.text.clone())];
        let identified = if speaker == Speaker::Them && !result.words.is_empty() && !session.diarization_failed {
            match self.diarizer.turns(window).await {
                Ok(turns) if !turns.is_empty() => align_speakers(&result.words, &turns, window.start),
                Ok(_) => whole(),
                Err(error) => {
                    // One failure disables speaker detection for the rest of the recording.
                    session.diarization_failed = true;
                    (self.on_error)(format!("{SPEAKER_DETECTION_FAILED} {error}"));
                    whole()
                }
            }
        } else {
            whole()
        };
        for segment in &identified {
            (self.on_segment)(segment.clone());
        }
        session.segments.extend(identified);
    }
}

#[cfg(test)]
#[path = "worker_tests.rs"]
mod tests;
