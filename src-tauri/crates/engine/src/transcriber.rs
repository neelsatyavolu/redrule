//! The app-wide speech engine: downloads the models on first use, loads them once, and transcribes
//! windows off the async runtime.

use std::path::PathBuf;
use std::sync::Arc;

use minutes_core::windower::AudioWindow;
use minutes_core::{Error, Result};
use sherpa_onnx::OfflineRecognizer;
use tokio::sync::OnceCell;

use crate::asr::{self, Transcription};
use crate::catalog::{SpeakerOption, SpeechOption};
use crate::diarization::SpeakerModels;
use crate::model_download::{self, ModelProgress, Progress};
use crate::worker::Transcribe;

pub(crate) struct Models {
    recognizer: OfflineRecognizer,
    pub speakers: SpeakerModels,
}

pub struct Transcriber {
    models_dir: PathBuf,
    speech: &'static SpeechOption,
    speaker: &'static SpeakerOption,
    /// Concurrent callers share one load; a failed load leaves it empty so the next call retries.
    models: OnceCell<Arc<Models>>,
}

impl Transcriber {
    /// Uses the models chosen by id (see `catalog`); unknown ids fall back to the defaults.
    pub fn new(models_dir: PathBuf, speech_id: &str, speaker_id: &str) -> Self {
        Self {
            models_dir,
            speech: crate::catalog::speech_option(speech_id),
            speaker: crate::catalog::speaker_option(speaker_id),
            models: OnceCell::new(),
        }
    }

    /// The models this transcriber uses, as (speech id, speaker id).
    pub fn choice(&self) -> (&'static str, &'static str) {
        (self.speech.id, self.speaker.id)
    }

    /// Downloads (first run) and loads the ASR and diarization models. Safe to call repeatedly and
    /// concurrently; after a failure, a later call retries.
    pub async fn prepare(&self, progress: impl Fn(ModelProgress) + Send + Sync + 'static) -> Result<()> {
        self.load(&progress).await.map(|_| ())
    }

    pub fn is_ready(&self) -> bool {
        self.models.initialized()
    }

    /// Waits for an in-flight prepare (or starts one). Each window is independent speech (fresh decoder state).
    pub async fn transcribe(&self, window: &AudioWindow) -> Result<Transcription> {
        let models = self.models().await?;
        let samples = window.samples.clone();
        tokio::task::spawn_blocking(move || asr::transcribe(&models.recognizer, &samples))
            .await
            .map_err(|error| Error::message(error.to_string()))?
    }

    pub(crate) async fn models(&self) -> Result<Arc<Models>> {
        self.load(&|_| {}).await
    }

    async fn load(&self, progress: &Progress) -> Result<Arc<Models>> {
        let models = self
            .models
            .get_or_try_init(|| async {
                for model in self.speech.models().into_iter().chain(self.speaker.models()) {
                    model_download::ensure(model, &self.models_dir, progress).await?;
                }
                progress(ModelProgress { downloaded_bytes: 0, total_bytes: None, stage: "Loading".into() });
                let (dir, speech, speaker) = (self.models_dir.clone(), self.speech, self.speaker);
                let loaded = tokio::task::spawn_blocking(move || -> Result<Models> {
                    Ok(Models {
                        recognizer: asr::load(&dir, speech.model)?,
                        speakers: SpeakerModels::load(&dir, &speaker.setup)?,
                    })
                })
                .await
                .map_err(|error| Error::message(error.to_string()))??;
                Ok::<_, Error>(Arc::new(loaded))
            })
            .await?;
        Ok(models.clone())
    }
}

impl Transcribe for Arc<Transcriber> {
    async fn transcribe(&self, window: &AudioWindow) -> Result<Transcription> {
        Transcriber::transcribe(self, window).await
    }
}
