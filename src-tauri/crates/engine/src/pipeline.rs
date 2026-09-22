//! Captures both sides of a meeting and turns them into transcript segments while it runs.

use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Instant;

use minutes_core::models::{Speaker, TranscriptSegment};
use minutes_core::{Error, Result};
use tokio::sync::mpsc::{UnboundedSender, unbounded_channel};
use tokio::task::JoinHandle;

use crate::diarization::SpeakerRecognizer;
use crate::mic_capture::{MicCapture, SampleSink};
use crate::system_capture::SystemCapture;
use crate::transcriber::Transcriber;
use crate::worker::{Chunk, ErrorSink, Worker};

pub struct RecordingPipeline {
    feed: Arc<Feed>,
    system: SystemCapture,
    mic: MicCapture,
    worker: JoinHandle<Vec<TranscriptSegment>>,
}

impl RecordingPipeline {
    /// Starts system audio then the microphone (stopping system capture if the mic fails), then the worker.
    /// `audio_folder`: where to keep me.wav/them.wav, or None. `microphone_id`: "" = system default.
    pub async fn start(
        transcriber: Arc<Transcriber>,
        audio_folder: Option<PathBuf>,
        microphone_id: String,
        on_segment: Arc<dyn Fn(TranscriptSegment) + Send + Sync>,
        on_error: Arc<dyn Fn(String) + Send + Sync>,
    ) -> Result<Self> {
        let (sender, input) = unbounded_channel();
        let feed = Arc::new(Feed {
            sender: Mutex::new(Some(sender)),
            recording: AtomicBool::new(true),
            began: Instant::now(),
        });
        let capture_error = feed.gate(on_error.clone());

        let system = {
            let (samples, stopped) = (feed.sink(Speaker::Them), capture_error.clone());
            blocking(move || SystemCapture::start(samples, stopped)).await?
        };
        let mic = {
            let (samples, failed) = (feed.sink(Speaker::Me), capture_error);
            blocking(move || MicCapture::start(microphone_id, samples, failed)).await
        };
        let mic = match mic {
            Ok(mic) => mic,
            Err(error) => {
                feed.recording.store(false, Ordering::SeqCst);
                if let Err(join) = tokio::task::spawn_blocking(move || system.stop()).await {
                    log::error!("Stopping system audio capture: {join}");
                }
                return Err(error);
            }
        };

        let worker = Worker {
            transcriber: transcriber.clone(),
            diarizer: SpeakerRecognizer::new(transcriber),
            on_segment,
            on_error,
        };
        let worker = tokio::spawn(async move { worker.run(input, audio_folder.as_deref()).await });
        Ok(Self { feed, system, mic, worker })
    }

    /// Stops capture, transcribes remaining audio, returns every segment of the meeting (raw, unmerged, in the order produced).
    pub async fn stop(self) -> Vec<TranscriptSegment> {
        // A late "stopped" callback must not be reported as an error for a finished recording.
        self.feed.recording.store(false, Ordering::SeqCst);
        let (mic, system) = (self.mic, self.system);
        if let Err(error) = tokio::task::spawn_blocking(move || {
            mic.stop();
            system.stop();
        })
        .await
        {
            log::error!("Stopping capture: {error}");
        }
        self.feed.close();
        self.worker.await.unwrap_or_else(|error| {
            log::error!("The transcription worker failed: {error}");
            Vec::new()
        })
    }
}

/// Where both captures deliver audio. Closing it ends the worker's input, whatever the capture
/// callbacks still hold.
struct Feed {
    sender: Mutex<Option<UnboundedSender<Chunk>>>,
    /// Cleared when stopping begins, before the captures are torn down.
    recording: AtomicBool,
    began: Instant,
}

impl Feed {
    fn sink(self: &Arc<Self>, speaker: Speaker) -> SampleSink {
        let feed = Arc::clone(self);
        Arc::new(move |samples| {
            let time = feed.began.elapsed().as_secs_f64();
            if let Ok(sender) = feed.sender.lock()
                && let Some(sender) = sender.as_ref()
            {
                // Fails only once the worker is gone, when the audio has nowhere to go anyway.
                let _ = sender.send(Chunk { speaker, samples, time });
            }
        })
    }

    /// Passes capture errors on only while recording.
    fn gate(self: &Arc<Self>, on_error: ErrorSink) -> ErrorSink {
        let feed = Arc::clone(self);
        Arc::new(move |error| {
            if feed.recording.load(Ordering::SeqCst) {
                on_error(error);
            }
        })
    }

    fn close(&self) {
        if let Ok(mut sender) = self.sender.lock() {
            sender.take();
        }
    }
}

async fn blocking<T: Send + 'static>(work: impl FnOnce() -> Result<T> + Send + 'static) -> Result<T> {
    tokio::task::spawn_blocking(work).await.map_err(|error| Error::message(error.to_string()))?
}
