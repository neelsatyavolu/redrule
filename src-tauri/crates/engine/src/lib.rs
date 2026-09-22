//! Audio capture, on-device transcription and speaker recognition.

mod asr;
mod diarization;
mod mic_capture;
mod microphones;
mod model_download;
mod pipeline;
mod resampler;
mod speakers;
mod system_capture;
mod transcriber;
mod wav;
mod words;
mod worker;

pub use asr::Transcription;
pub use diarization::SpeakerRecognizer;
pub use microphones::{Microphone, microphones};
pub use model_download::ModelProgress;
pub use pipeline::RecordingPipeline;
pub use transcriber::Transcriber;
