//! App-wide state. The backend owns it and pushes a full snapshot to every window after each change.
use std::sync::{Arc, Mutex, MutexGuard};

use minutes_engine::{ModelProgress, RecordingPipeline, Transcriber};
use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager};
use tauri::async_runtime::JoinHandle;

use super::folders::{FolderInfo, FolderMeeting, Folders};
use super::settings::Settings;
use crate::core::models::{Meeting, MeetingApp, MeetingStatus, TranscriptSegment};
use crate::core::oauth::ProviderId;
use crate::core::store::MeetingStore;
use crate::core::transcript;
use crate::platform::permissions::Permissions;
use crate::providers::oauth_service::OAuthService;
use crate::shell::RecordItems;

pub const STATE_EVENT: &str = "state";
pub const ERROR_EVENT: &str = "app-error";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum Banner {
    Detected { app: MeetingApp },
    Ended,
}

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "state", rename_all = "camelCase")]
pub enum SpeechModel {
    Loading { progress: Option<ModelProgress> },
    Ready,
    Failed { message: String },
}

/// Notes being written on this Mac for one meeting: loading the model, then writing.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NotesProgress {
    pub meeting_id: String,
    pub stage: NotesStage,
    /// 0-99; an estimate while writing.
    pub percent: u8,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum NotesStage {
    Loading,
    Writing,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct State {
    pub meetings: Vec<Meeting>,
    pub recording_id: Option<String>,
    /// Raw segments of the running recording, saved as it goes so a crash keeps what was said.
    #[serde(skip)]
    pub live_raw: Vec<TranscriptSegment>,
    /// Merged for display.
    pub live_segments: Vec<TranscriptSegment>,
    pub banner: Option<Banner>,
    pub speech_model: SpeechModel,
    /// The on-device note model's download; absent while notes are written with an account.
    pub note_model: Option<SpeechModel>,
    /// The on-device model that answers questions; absent while an account answers them.
    pub ask_model: Option<SpeechModel>,
    pub notes_progress: Option<NotesProgress>,
    pub connected: Vec<ProviderId>,
    pub connecting: Option<ProviderId>,
    pub connection_error: Option<String>,
    pub permissions: Permissions,
    pub settings: Settings,
    pub sharing_busy: bool,
    /// Bumped whenever a meeting's stored content changes, so views reload it.
    pub revision: u64,
    pub storage_error: Option<String>,
    /// Shared folders this Mac has joined.
    pub folders: Vec<FolderInfo>,
    /// Other people's meetings in those folders.
    pub folder_meetings: Vec<FolderMeeting>,
    /// Shown on meetings this Mac adds to folders.
    pub display_name: String,
}

pub struct ActiveRecording {
    pub meeting: Meeting,
    pub pipeline: RecordingPipeline,
}

pub struct App {
    pub handle: AppHandle,
    pub store: Option<MeetingStore>,
    pub http: reqwest::Client,
    pub oauth: Arc<OAuthService>,
    pub models_dir: std::path::PathBuf,
    /// Replaced when the user picks other models; a running recording keeps the one it started with.
    pub transcriber: Mutex<Arc<Transcriber>>,
    state: Mutex<State>,
    /// Held while capture starts, so stopping waits for a start that is still in progress.
    pub recording: tokio::sync::Mutex<Option<ActiveRecording>>,
    pub connect_task: Mutex<Option<JoinHandle<()>>>,
    /// On-device note models being downloaded, by id.
    pub note_downloads: Mutex<Vec<(&'static str, JoinHandle<()>)>>,
    /// One on-device note model in memory at a time, for notes and questions alike.
    pub local_notes: tokio::sync::Mutex<()>,
    /// Identifies the current banner, so a stale auto-dismiss timer does nothing.
    pub banner_generation: Mutex<u64>,
    pub folders: Folders,
}

impl App {
    pub fn new(handle: AppHandle, models_dir: std::path::PathBuf) -> Self {
        let (store, storage_error) = match MeetingStore::default_root().and_then(MeetingStore::new) {
            Ok(store) => match store.recover_interrupted() {
                Ok(()) => (Some(store), None),
                Err(error) => (Some(store), Some(format!("Some meetings could not be checked. {error}"))),
            },
            Err(error) => (None, Some(format!("Redrule cannot open its storage folder. {error}"))),
        };
        let http = reqwest::Client::new();
        let settings = Settings::load();
        let transcriber = Transcriber::new(models_dir.clone(), &settings.speech_model_id, &settings.speaker_model_id);
        let state = State {
            meetings: store.as_ref().and_then(|s| s.list().ok()).unwrap_or_default(),
            recording_id: None,
            live_raw: Vec::new(),
            live_segments: Vec::new(),
            banner: None,
            speech_model: SpeechModel::Loading { progress: None },
            note_model: None,
            ask_model: None,
            notes_progress: None,
            connected: Vec::new(),
            connecting: None,
            connection_error: None,
            permissions: Permissions::current(),
            settings,
            sharing_busy: false,
            revision: 0,
            storage_error,
            folders: Vec::new(),
            folder_meetings: Vec::new(),
            display_name: String::new(),
        };
        // Models live in the support folder, beside the shared folder files.
        let support = models_dir.parent().map(std::path::Path::to_path_buf).unwrap_or_default();
        Self {
            handle,
            store,
            oauth: Arc::new(OAuthService::new(http.clone())),
            http,
            models_dir,
            transcriber: Mutex::new(Arc::new(transcriber)),
            state: Mutex::new(state),
            recording: tokio::sync::Mutex::new(None),
            connect_task: Mutex::new(None),
            note_downloads: Mutex::new(Vec::new()),
            local_notes: tokio::sync::Mutex::new(()),
            banner_generation: Mutex::new(0),
            folders: Folders::new(support),
        }
    }

    fn lock(&self) -> MutexGuard<'_, State> {
        // A panic while holding the lock leaves plain data behind; keep serving it.
        self.state.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    pub fn transcriber(&self) -> Arc<Transcriber> {
        Arc::clone(&self.transcriber.lock().unwrap_or_else(|poisoned| poisoned.into_inner()))
    }

    pub fn snapshot(&self) -> State {
        self.lock().clone()
    }

    /// Reads a value without publishing anything.
    pub fn read<T>(&self, f: impl FnOnce(&State) -> T) -> T {
        f(&self.lock())
    }

    /// Changes the state and publishes the result to every window.
    pub fn update<T>(&self, f: impl FnOnce(&mut State) -> T) -> T {
        let (result, snapshot) = {
            let mut state = self.lock();
            let result = f(&mut state);
            (result, state.clone())
        };
        let _ = self.handle.emit(STATE_EVENT, &snapshot);
        if let Some(items) = self.handle.try_state::<RecordItems>() {
            items.sync(snapshot.recording_id.is_some());
        }
        result
    }

    pub fn report(&self, message: impl Into<String>) {
        let message = message.into();
        log::warn!("{message}");
        let _ = self.handle.emit(ERROR_EVENT, message);
    }

    pub fn store(&self) -> crate::core::Result<&MeetingStore> {
        self.store.as_ref().ok_or_else(|| crate::core::Error::message("Redrule cannot open its storage folder."))
    }

    /// Saves a meeting and refreshes the list. Failures are reported, not returned.
    pub fn persist(&self, meeting: &Meeting) {
        if let Err(error) = self.store().and_then(|store| store.save(meeting)) {
            self.report(format!("The meeting could not be saved. {error}"));
        }
        self.reload_meetings();
    }

    pub fn reload_meetings(&self) {
        let Some(store) = &self.store else { return };
        match store.list() {
            Ok(meetings) => {
                self.update(|state| {
                    state.meetings = meetings;
                    state.revision += 1;
                });
                self.folders.nudge();
            }
            Err(error) => self.report(format!("Meetings could not be loaded. {error}")),
        }
    }

    pub fn meeting(&self, id: &str) -> Option<Meeting> {
        self.read(|state| state.meetings.iter().find(|m| m.id == id).cloned())
    }

    /// Notes can be changed once a meeting has finished, successfully or not.
    pub fn can_edit(&self, meeting: &Meeting) -> bool {
        let recording = self.read(|state| state.recording_id.clone());
        recording.as_deref() != Some(meeting.id.as_str())
            && matches!(meeting.status, MeetingStatus::Done | MeetingStatus::Failed)
    }

    pub fn editable(&self, id: &str) -> crate::core::Result<Meeting> {
        self.meeting(id)
            .filter(|meeting| self.can_edit(meeting))
            .ok_or_else(|| crate::core::Error::message("This meeting can't be changed while it is being recorded or processed."))
    }

    pub fn set_live(&self, raw: Vec<TranscriptSegment>) {
        let merged = transcript::merge(&raw);
        self.update(|state| {
            state.live_raw = raw;
            state.live_segments = merged;
        });
    }

    pub fn refresh_permissions(&self) {
        let permissions = Permissions::current();
        if self.read(|state| state.permissions != permissions) {
            self.update(|state| state.permissions = permissions);
        }
    }

    pub fn refresh_connections(&self) {
        let connected: Vec<ProviderId> = ProviderId::ALL.into_iter().filter(|p| self.oauth.is_connected(*p)).collect();
        self.update(|state| state.connected = connected);
    }
}
