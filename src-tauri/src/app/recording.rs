//! Recording a meeting, finishing its transcript, and writing the notes.
use std::sync::Arc;

use chrono::Utc;
use minutes_engine::{LocalNoteWriter, NoteProgress, NoteProgressSink, RecordingPipeline};

use super::state::{ActiveRecording, App, NotesProgress, NotesStage};
use crate::core::api_providers::Provider;
use crate::core::models::{Meeting, MeetingApp, MeetingStatus, TranscriptSegment};
use crate::core::summary::{CHUNK_BUDGET, summarize};
use crate::core::{Error, Result};
use crate::providers::api_keys::ApiAccess;
use crate::providers::clients::{ModelChoice, SummaryClient};

impl App {
    /// Records into `folder` when given, one of the shared folders this Mac has joined.
    pub async fn start_recording(self: &Arc<Self>, app: MeetingApp, folder: Option<String>) {
        if self.read(|state| state.recording_id.is_some()) || self.store.is_none() {
            return;
        }
        self.set_banner(None);
        self.refresh_permissions();
        if !self.read(|state| state.permissions.all_granted()) {
            self.report("Redrule needs Microphone and Screen & System Audio Recording access before it can record. Grant them in Settings, under Permissions.");
            return;
        }

        // Runs alongside capture starting up, so it never holds the recording back.
        let event = self.look_up_event();
        // Hold the slot while capture starts, so a quick stop waits for it instead of leaving capture running.
        let mut slot = self.recording.lock().await;
        let meeting = Meeting {
            id: uuid::Uuid::new_v4().to_string().to_uppercase(),
            title: app.default_title(),
            app,
            started_at: Utc::now(),
            ended_at: None,
            status: MeetingStatus::Recording,
            error_message: None,
            archived_at: None,
            tags: vec![],
            folder_id: folder.filter(|id| self.folders.accepts(id)),
            attendees: vec![],
        };
        self.persist(&meeting);
        self.update(|state| state.recording_id = Some(meeting.id.clone()));
        self.set_live(Vec::new());

        let settings = self.read(|state| state.settings.clone());
        let audio_folder = if settings.keep_audio { self.store().and_then(|s| s.folder(&meeting.id)).ok() } else { None };
        let on_segment = {
            let (app, id) = (Arc::clone(self), meeting.id.clone());
            Arc::new(move |segment: TranscriptSegment| app.append_live(segment, &id))
        };
        let on_error = {
            let app = Arc::clone(self);
            Arc::new(move |message: String| app.report(format!("Part of the audio could not be processed. {message}")))
        };
        match RecordingPipeline::start(self.transcriber(), audio_folder, settings.microphone_id, on_segment, on_error).await {
            Ok(pipeline) => {
                let meeting = self.name_after_event(meeting, event).await;
                *slot = Some(ActiveRecording { meeting, pipeline });
            }
            Err(error) => {
                self.update(|state| state.recording_id = None);
                self.persist(&Meeting { ended_at: Some(Utc::now()), ..meeting }.failed(format!("Recording could not start. {error}")));
            }
        }
    }

    pub async fn stop_recording(self: &Arc<Self>) {
        if self.read(|state| state.recording_id.is_none()) {
            return;
        }
        self.set_banner(None);
        self.update(|state| state.recording_id = None);
        let Some(active) = self.recording.lock().await.take() else { return };
        let ended = active.meeting.ended(Utc::now(), MeetingStatus::Transcribing);
        self.persist(&ended);
        // The pipeline's own list is authoritative: live updates can arrive after this point.
        let segments = active.pipeline.stop().await;
        if let Err(error) = self.store().and_then(|store| store.save_transcript(&segments, &ended.id)) {
            self.report(format!("The transcript could not be saved. {error}"));
        }
        self.set_live(Vec::new());
        self.generate_notes(&ended).await;
    }

    /// Writes notes from the stored transcript. Also used to retry after a failure.
    pub async fn generate_notes(self: &Arc<Self>, meeting: &Meeting) {
        let working = meeting.with_status(MeetingStatus::Summarizing);
        self.persist(&working);
        match self.write_notes(&working).await {
            Ok(title) => self.persist(&working.titled(title).with_status(MeetingStatus::Done)),
            Err(error) => self.persist(&working.failed(error.to_string())),
        }
    }

    async fn write_notes(self: &Arc<Self>, meeting: &Meeting) -> Result<String> {
        let store = self.store()?;
        let segments = store.transcript(&meeting.id)?;
        let note = match self.read(|state| state.settings.local_note_model()) {
            Some(option) => {
                let _one_at_a_time = self.local_notes.lock().await;
                let (dir, sink) = (self.models_dir.clone(), self.notes_progress(&meeting.id));
                let written = async {
                    let writer = tauri::async_runtime::spawn_blocking(move || LocalNoteWriter::load_reporting(option, &dir, sink))
                        .await
                        .map_err(|error| Error::message(error.to_string()))??;
                    summarize(&writer, meeting, &segments, option.chunk_tokens).await
                }
                .await;
                self.update(|state| state.notes_progress = None);
                written?
            }
            None => {
                let preferred = self.read(|state| state.settings.model_choice());
                summarize(&self.account_client(preferred)?, meeting, &segments, CHUNK_BUDGET).await?
            }
        };
        store.save_note(&note, &meeting.id)?;
        Ok(note.title)
    }

    /// Publishes how far along the notes on this Mac are, only when the shown percentage changes.
    fn notes_progress(self: &Arc<Self>, meeting_id: &str) -> NoteProgressSink {
        let (app, meeting_id) = (Arc::clone(self), meeting_id.to_string());
        Arc::new(move |progress| {
            let (stage, done) = match progress {
                NoteProgress::Loading(done) => (NotesStage::Loading, done),
                NoteProgress::Writing(done) => (NotesStage::Writing, done),
            };
            let next = NotesProgress { meeting_id: meeting_id.clone(), stage, percent: (done * 100.0).clamp(0.0, 99.0) as u8 };
            if app.read(|state| state.notes_progress.as_ref() != Some(&next)) {
                app.update(|state| state.notes_progress = Some(next));
            }
        })
    }

    /// A client for `preferred`, or for another connected account or key when that one is not set up.
    pub(super) fn account_client(&self, preferred: ModelChoice) -> Result<SummaryClient> {
        self.refresh_connections();
        let (connected, settings) = self.read(|state| (state.connected.clone(), state.settings.clone()));
        let choice = if connected.contains(&preferred.provider) {
            preferred
        } else {
            connected.iter().find_map(|provider| settings.default_choice(*provider)).ok_or(Error::NoProviderConnected)?
        };
        let api = match choice.provider {
            Provider::Api(provider) => ApiAccess { key: self.api_keys.get(provider)?, base_url: settings.compatible_url },
            Provider::Account(_) => ApiAccess::default(),
        };
        Ok(SummaryClient { http: self.http.clone(), oauth: Arc::clone(&self.oauth), choice, api })
    }

    fn append_live(&self, segment: TranscriptSegment, id: &str) {
        // After stop, the pipeline's returned list is saved instead; a late update must not overwrite it.
        let raw = self.read(|state| {
            (state.recording_id.as_deref() == Some(id)).then(|| {
                let mut raw = state.live_raw.clone();
                raw.push(segment);
                raw
            })
        });
        let Some(raw) = raw else { return };
        if let Err(error) = self.store().and_then(|store| store.save_transcript(&raw, id)) {
            self.report(format!("The transcript could not be saved. {error}"));
        }
        self.set_live(raw);
    }
}
