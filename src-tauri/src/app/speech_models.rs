//! Choosing, downloading and removing the on-device speech and speaker models.
use std::sync::Arc;

use minutes_engine::Transcriber;
use minutes_engine::catalog::{self, Hardware};
use objc2_foundation::NSLocale;
use serde::{Deserialize, Serialize};

use super::state::App;
use crate::core::{Error, Result};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ModelKind {
    Speech,
    Speaker,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LocalModel {
    pub id: &'static str,
    pub name: &'static str,
    pub description: &'static str,
    /// Languages it understands; speaker models work with any.
    pub languages: Option<&'static str>,
    pub size_mb: u32,
    pub installed: bool,
    pub recommended: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LocalModels {
    pub speech: Vec<LocalModel>,
    pub speaker: Vec<LocalModel>,
    pub hardware: Hardware,
}

/// The (speech, speaker) option ids that suit this Mac and its language.
pub fn recommended() -> (&'static str, &'static str) {
    catalog::recommend(Hardware::detect(), prefers_english())
}

fn prefers_english() -> bool {
    NSLocale::preferredLanguages().firstObject().is_none_or(|language| language.to_string().starts_with("en"))
}

impl App {
    pub fn local_models(&self) -> LocalModels {
        let hardware = Hardware::detect();
        let (speech, speaker) = catalog::recommend(hardware, prefers_english());
        let dir = &self.models_dir;
        LocalModels {
            speech: catalog::SPEECH_OPTIONS
                .iter()
                .map(|option| LocalModel {
                    id: option.id,
                    name: option.name,
                    description: option.description,
                    languages: Some(option.languages),
                    size_mb: option.size_mb,
                    installed: option.is_installed(dir),
                    recommended: option.id == speech,
                })
                .collect(),
            speaker: catalog::SPEAKER_OPTIONS
                .iter()
                .map(|option| LocalModel {
                    id: option.id,
                    name: option.name,
                    description: option.description,
                    languages: None,
                    size_mb: option.size_mb,
                    installed: option.is_installed(dir),
                    recommended: option.id == speaker,
                })
                .collect(),
            hardware,
        }
    }

    /// Switches to the models in the settings, downloading them if needed. A recording in progress
    /// keeps the models it started with.
    pub fn use_chosen_models(self: &Arc<Self>) {
        let (speech, speaker) = self.read(|state| (state.settings.speech_model_id.clone(), state.settings.speaker_model_id.clone()));
        let current = self.transcriber();
        if current.choice() == (catalog::speech_option(&speech).id, catalog::speaker_option(&speaker).id) {
            return;
        }
        *self.transcriber.lock().unwrap_or_else(|poisoned| poisoned.into_inner()) =
            Arc::new(Transcriber::new(self.models_dir.clone(), &speech, &speaker));
        self.prepare_speech_model();
    }

    /// Deletes a downloaded model to free disk space. The chosen models cannot be removed.
    pub fn remove_local_model(&self, kind: ModelKind, id: &str) -> Result<()> {
        let (speech, speaker) = self.transcriber().choice();
        let dir = &self.models_dir;
        let result = match kind {
            ModelKind::Speech if id == speech => return Err(in_use()),
            ModelKind::Speaker if id == speaker => return Err(in_use()),
            ModelKind::Speech => catalog::speech_option(id).remove(dir),
            ModelKind::Speaker => catalog::speaker_option(id).remove(dir),
        };
        result.map_err(|error| Error::message(format!("The model could not be removed. {error}")))
    }
}

fn in_use() -> Error {
    Error::message("This model is in use. Choose a different one first.")
}
