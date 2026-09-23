//! Choosing, downloading and removing the on-device speech, speaker and note models.
use std::sync::{Arc, Mutex};
use std::time::Instant;

use minutes_engine::Transcriber;
use minutes_engine::catalog::{self, Hardware};
use objc2_foundation::NSLocale;
use serde::{Deserialize, Serialize};

use super::background::PROGRESS_INTERVAL;
use super::state::{App, SpeechModel};
use crate::core::{Error, Result};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ModelKind {
    Speech,
    Speaker,
    Notes,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LocalModel {
    pub id: &'static str,
    pub name: &'static str,
    pub description: &'static str,
    /// Languages it understands; speaker and note models work with any.
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
    pub notes: Vec<LocalModel>,
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
            notes: catalog::NOTE_OPTIONS
                .iter()
                .map(|option| LocalModel {
                    id: option.id,
                    name: option.name,
                    description: option.description,
                    languages: None,
                    size_mb: option.size_mb,
                    installed: option.is_installed(dir),
                    recommended: option.id == catalog::DEFAULT_NOTES,
                })
                .collect(),
            hardware,
        }
    }

    /// Switches to the models in the settings, downloading them if needed. A recording in progress
    /// keeps the models it started with.
    pub fn use_chosen_models(self: &Arc<Self>) {
        self.prepare_note_model();
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
        let notes = self.read(|state| state.settings.local_note_model()).map(|option| option.id);
        let dir = &self.models_dir;
        let result = match kind {
            ModelKind::Speech if id == speech => return Err(in_use()),
            ModelKind::Speaker if id == speaker => return Err(in_use()),
            ModelKind::Notes if Some(id) == notes => return Err(in_use()),
            ModelKind::Speech => catalog::speech_option(id).remove(dir),
            ModelKind::Speaker => catalog::speaker_option(id).remove(dir),
            ModelKind::Notes => catalog::note_option(id).remove(dir),
        };
        result.map_err(|error| Error::message(format!("The model could not be removed. {error}")))
    }
}

impl App {
    /// Downloads the chosen on-device note model in the background. Switching to another model or
    /// to an account stops a download that is no longer needed.
    pub fn prepare_note_model(self: &Arc<Self>) {
        let option = self.read(|state| state.settings.local_note_model());
        let mut download = self.note_download.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
        if let Some((id, task)) = download.as_ref() {
            if Some(*id) == option.map(|option| option.id) && !task.inner().is_finished() {
                return;
            }
            task.abort();
        }
        *download = None;
        let Some(option) = option else {
            self.update(|state| state.note_model = None);
            return;
        };
        if option.is_installed(&self.models_dir) {
            self.update(|state| state.note_model = Some(SpeechModel::Ready));
            return;
        }
        self.update(|state| state.note_model = Some(SpeechModel::Loading { progress: None }));
        let (app, dir) = (Arc::clone(self), self.models_dir.clone());
        let task = tauri::async_runtime::spawn(async move {
            let reporter = Arc::clone(&app);
            let last = Mutex::new(Instant::now() - PROGRESS_INTERVAL);
            let result = option
                .download(&dir, move |progress| {
                    let mut last = last.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
                    if last.elapsed() >= PROGRESS_INTERVAL && reporter.wants_note_model(option) {
                        *last = Instant::now();
                        reporter.update(|state| state.note_model = Some(SpeechModel::Loading { progress: Some(progress) }));
                    }
                })
                .await;
            if !app.wants_note_model(option) {
                return;
            }
            app.update(|state| {
                state.note_model = Some(match result {
                    Ok(()) => SpeechModel::Ready,
                    Err(error) => SpeechModel::Failed { message: error.to_string() },
                })
            });
        });
        *download = Some((option.id, task));
    }

    fn wants_note_model(&self, option: &catalog::NoteOption) -> bool {
        self.read(|state| state.settings.local_note_model().map(|chosen| chosen.id)) == Some(option.id)
    }
}

fn in_use() -> Error {
    Error::message("This model is in use. Choose a different one first.")
}
