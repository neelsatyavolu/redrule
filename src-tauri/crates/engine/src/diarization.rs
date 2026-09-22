//! Speaker recognition for call audio: sherpa-onnx pyannote segmentation + clustering finds the
//! speakers inside one window, WeSpeaker embeddings tie them to the recording's known voices.

use std::path::Path;
use std::sync::Arc;

use minutes_core::transcript::SpeakerTurn;
use minutes_core::windower::{AudioWindow, SAMPLE_RATE};
use minutes_core::{Error, Result};
use sherpa_onnx::{
    FastClusteringConfig, OfflineSpeakerDiarization, OfflineSpeakerDiarizationConfig, OfflineSpeakerDiarizationSegment,
    SpeakerEmbeddingExtractor, SpeakerEmbeddingExtractorConfig,
};

use crate::model_download::{EMBEDDING, SEGMENTATION};
use crate::speakers::{LocalSpeaker, SpeakerRegistry};
use crate::transcriber::Transcriber;
use crate::worker::Diarize;

/// Below this much speech an embedding is too noisy to define a new voice.
const RELIABLE_SECONDS: f64 = 1.0;
/// Cosine distance below which segments in one window merge into one speaker. Sherpa's default
/// (0.5) merged two distinct voices (similarity 0.59) in testing; same-voice similarity was ~0.94.
const CLUSTER_THRESHOLD: f32 = 0.4;
const THREADS: i32 = 2;

pub(crate) struct SpeakerModels {
    diarization: OfflineSpeakerDiarization,
    embedder: SpeakerEmbeddingExtractor,
}

impl SpeakerModels {
    pub(crate) fn load(models_dir: &Path) -> Result<Self> {
        let embedding = SpeakerEmbeddingExtractorConfig {
            model: Some(EMBEDDING.path(models_dir).to_string_lossy().into_owned()),
            num_threads: THREADS,
            provider: Some("cpu".into()),
            ..Default::default()
        };
        let mut config = OfflineSpeakerDiarizationConfig {
            embedding: embedding.clone(),
            clustering: FastClusteringConfig { num_clusters: -1, threshold: CLUSTER_THRESHOLD, ..Default::default() },
            ..Default::default()
        };
        config.segmentation.pyannote.model =
            Some(SEGMENTATION.path(models_dir).join("model.onnx").to_string_lossy().into_owned());
        config.segmentation.num_threads = THREADS;

        let failed = || Error::message("The speaker model could not be loaded.");
        Ok(Self {
            diarization: OfflineSpeakerDiarization::create(&config).ok_or_else(failed)?,
            embedder: SpeakerEmbeddingExtractor::create(&embedding).ok_or_else(failed)?,
        })
    }

    /// Finds the speakers in one window, each with an embedding of all its speech. Blocking.
    pub(crate) fn analyze(&self, samples: &[f32]) -> Result<Vec<LocalSpeaker>> {
        let result =
            self.diarization.process(samples).ok_or_else(|| Error::message("Diarization returned no result."))?;
        let segments = result.sort_by_start_time();
        Ok(order_of_appearance(&segments)
            .into_iter()
            .map(|speaker| {
                let turns: Vec<(f64, f64)> =
                    segments.iter().filter(|s| s.speaker == speaker).map(|s| (s.start as f64, s.end as f64)).collect();
                let audio: Vec<f32> =
                    turns.iter().flat_map(|&(start, end)| slice(samples, start, end)).copied().collect();
                LocalSpeaker {
                    embedding: self.embed(&audio),
                    reliable: audio.len() as f64 / SAMPLE_RATE >= RELIABLE_SECONDS,
                    turns,
                }
            })
            .collect())
    }

    fn embed(&self, audio: &[f32]) -> Option<Vec<f32>> {
        let stream = self.embedder.create_stream()?;
        stream.accept_waveform(SAMPLE_RATE as i32, audio);
        stream.input_finished();
        if !self.embedder.is_ready(&stream) {
            return None;
        }
        self.embedder.compute(&stream)
    }
}

fn order_of_appearance(segments: &[OfflineSpeakerDiarizationSegment]) -> Vec<i32> {
    segments.iter().fold(Vec::new(), |mut seen, segment| {
        if !seen.contains(&segment.speaker) {
            seen.push(segment.speaker);
        }
        seen
    })
}

fn slice(samples: &[f32], start: f64, end: f64) -> &[f32] {
    let index = |seconds: f64| ((seconds.max(0.0) * SAMPLE_RATE) as usize).min(samples.len());
    let (from, to) = (index(start), index(end));
    &samples[from..to.max(from)]
}

/// Labels who spoke when in call audio, with ids ("1", "2", …) that stay stable across windows.
/// Use one per recording, so voice identities never leak between meetings.
pub struct SpeakerRecognizer {
    transcriber: Arc<Transcriber>,
    registry: SpeakerRegistry,
}

impl SpeakerRecognizer {
    pub fn new(transcriber: Arc<Transcriber>) -> Self {
        Self { transcriber, registry: SpeakerRegistry::default() }
    }

    /// Speaker turns in `window`, relative to its start. Waits for the models like `Transcriber::transcribe`.
    pub async fn turns(&mut self, window: &AudioWindow) -> Result<Vec<SpeakerTurn>> {
        let models = self.transcriber.models().await?;
        let samples = window.samples.clone();
        let speakers = tokio::task::spawn_blocking(move || models.speakers.analyze(&samples))
            .await
            .map_err(|error| Error::message(error.to_string()))??;
        Ok(self.registry.turns(&speakers))
    }
}

impl Diarize for SpeakerRecognizer {
    async fn turns(&mut self, window: &AudioWindow) -> Result<Vec<SpeakerTurn>> {
        SpeakerRecognizer::turns(self, window).await
    }
}
