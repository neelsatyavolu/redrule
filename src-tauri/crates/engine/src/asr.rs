//! Parakeet TDT v3 speech recognition through sherpa-onnx. Blocking: call from a blocking thread.

use std::path::Path;

use minutes_core::windower::SAMPLE_RATE;
use minutes_core::{Error, Result};
use sherpa_onnx::{OfflineRecognizer, OfflineRecognizerConfig, OfflineTransducerModelConfig};

use crate::model_download::SPEECH;
use crate::words::words;

/// Leaves cores free for capture and the rest of the app.
const THREADS: i32 = 4;

pub struct Transcription {
    pub text: String,
    pub words: Vec<minutes_core::transcript::TranscriptWord>,
}

pub(crate) fn load(models_dir: &Path) -> Result<OfflineRecognizer> {
    let dir = SPEECH.path(models_dir);
    let file = |name: &str| Some(dir.join(name).to_string_lossy().into_owned());
    let mut config = OfflineRecognizerConfig::default();
    config.model_config.transducer = OfflineTransducerModelConfig {
        encoder: file("encoder.int8.onnx"),
        decoder: file("decoder.int8.onnx"),
        joiner: file("joiner.int8.onnx"),
    };
    config.model_config.tokens = file("tokens.txt");
    config.model_config.model_type = Some("nemo_transducer".into());
    config.model_config.num_threads = THREADS;
    config.model_config.provider = Some("cpu".into());
    config.decoding_method = Some("greedy_search".into());
    OfflineRecognizer::create(&config).ok_or_else(|| Error::message("The speech model could not be loaded."))
}

/// Decodes one window as independent speech: every call gets a fresh stream and decoder state.
pub(crate) fn transcribe(recognizer: &OfflineRecognizer, samples: &[f32]) -> Result<Transcription> {
    let stream = recognizer.create_stream();
    stream.accept_waveform(SAMPLE_RATE as i32, samples);
    recognizer.decode(&stream);
    let result = stream.get_result().ok_or_else(|| Error::message("The speech model returned no result."))?;
    let timestamps = result.timestamps.unwrap_or_default();
    Ok(Transcription {
        text: result.text.trim().to_string(),
        words: words(&result.tokens, &timestamps, result.durations.as_deref()),
    })
}
