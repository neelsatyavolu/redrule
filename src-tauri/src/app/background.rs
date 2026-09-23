//! Work that runs for the app's lifetime: call detection and the speech model.
use std::sync::Arc;
use std::time::{Duration, Instant};

use minutes_engine::Transcriber;

use super::state::{App, Banner, SpeechModel};
use crate::core::detector_logic::{DetectionEvent, DetectorLogic};
use crate::platform::detection;
use crate::shell;

const POLL_INTERVAL: Duration = Duration::from_secs(3);
/// An unanswered "meeting detected" prompt should not sit on screen for the whole call.
const BANNER_AUTO_DISMISS: Duration = Duration::from_secs(45);
pub(super) const PROGRESS_INTERVAL: Duration = Duration::from_millis(250);

impl App {
    /// Starts everything that should run once the app has launched.
    pub fn start(self: &Arc<Self>) {
        self.refresh_connections();
        self.prepare_speech_model();
        self.prepare_note_model();
        self.start_folder_sync();
        let app = Arc::clone(self);
        tauri::async_runtime::spawn(async move { app.watch_for_calls().await });
    }

    async fn watch_for_calls(self: Arc<Self>) {
        let mut logic = DetectorLogic::default();
        loop {
            if let Ok(snapshot) = tauri::async_runtime::spawn_blocking(detection::snapshot).await {
                let (next, event) = logic.ingest(&snapshot);
                logic = next;
                match event {
                    Some(DetectionEvent::Detected(app)) if !self.is_recording() => {
                        self.set_banner(Some(Banner::Detected { app }))
                    }
                    Some(DetectionEvent::Ended) => {
                        self.set_banner(self.is_recording().then_some(Banner::Ended))
                    }
                    _ => {}
                }
            }
            tokio::time::sleep(POLL_INTERVAL).await;
        }
    }

    pub fn is_recording(&self) -> bool {
        self.read(|state| state.recording_id.is_some())
    }

    pub fn set_banner(self: &Arc<Self>, banner: Option<Banner>) {
        let generation = {
            let mut generation = self.banner_generation.lock().unwrap();
            *generation += 1;
            *generation
        };
        self.update(|state| state.banner = banner);
        shell::show_banner(&self.handle, banner.is_some());
        if matches!(banner, Some(Banner::Detected { .. })) {
            let app = Arc::clone(self);
            tauri::async_runtime::spawn(async move {
                tokio::time::sleep(BANNER_AUTO_DISMISS).await;
                if *app.banner_generation.lock().unwrap() == generation {
                    app.set_banner(None);
                }
            });
        }
    }

    pub fn prepare_speech_model(self: &Arc<Self>) {
        self.update(|state| state.speech_model = SpeechModel::Loading { progress: None });
        let app = Arc::clone(self);
        let transcriber = self.transcriber();
        tauri::async_runtime::spawn(async move {
            let reporter = Arc::clone(&app);
            let preparing = Arc::clone(&transcriber);
            let last = std::sync::Mutex::new(Instant::now() - PROGRESS_INTERVAL);
            let result = transcriber
                .prepare(move |progress| {
                    // Downloads report thousands of times; the UI needs a few updates a second.
                    let mut last = last.lock().unwrap();
                    if last.elapsed() >= PROGRESS_INTERVAL && reporter.is_current(&preparing) {
                        *last = Instant::now();
                        reporter.update(|state| state.speech_model = SpeechModel::Loading { progress: Some(progress) });
                    }
                })
                .await;
            // The user switched models meanwhile: the newer preparation reports instead.
            if !app.is_current(&transcriber) {
                return;
            }
            app.update(|state| {
                state.speech_model = match result {
                    Ok(()) => SpeechModel::Ready,
                    Err(error) => SpeechModel::Failed { message: error.to_string() },
                }
            });
        });
    }

    fn is_current(&self, transcriber: &Arc<Transcriber>) -> bool {
        Arc::ptr_eq(&self.transcriber(), transcriber)
    }
}
