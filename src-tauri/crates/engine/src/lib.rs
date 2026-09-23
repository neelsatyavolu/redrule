//! Audio capture, on-device transcription and speaker recognition.

mod asr;
pub mod catalog;
mod diarization;
mod mic_capture;
mod microphones;
mod model_download;
#[cfg(test)]
mod note_eval;
mod note_writer;
mod pipeline;
mod resampler;
mod speakers;
#[cfg(test)]
mod asr_eval;
#[cfg(test)]
mod speaker_eval;
mod system_capture;
mod transcriber;
mod wav;
mod words;
mod worker;

pub use asr::Transcription;
pub use diarization::SpeakerRecognizer;
pub use microphones::{Microphone, microphones};
pub use model_download::ModelProgress;
pub use note_writer::{LocalNoteWriter, NoteProgress, NoteProgressSink};
pub use pipeline::RecordingPipeline;
pub use transcriber::Transcriber;
