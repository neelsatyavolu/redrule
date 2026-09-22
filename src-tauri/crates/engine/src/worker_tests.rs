use std::sync::Mutex;
use std::sync::atomic::{AtomicUsize, Ordering};

use minutes_core::Error;
use minutes_core::transcript::TranscriptWord;
use minutes_core::windower::SAMPLE_RATE;
use tokio::sync::mpsc::unbounded_channel;

use super::*;

const ME_LEVEL: f32 = 0.1;
const THEM_LEVEL: f32 = 0.2;

/// Names the stream by its level and says where the window starts; two timed words per window.
struct FakeTranscriber {
    fail: bool,
    silent: bool,
}

impl Transcribe for FakeTranscriber {
    async fn transcribe(&self, window: &AudioWindow) -> Result<Transcription> {
        if self.fail {
            return Err(Error::message("model broke"));
        }
        if self.silent {
            return Ok(Transcription { text: String::new(), words: vec![] });
        }
        let who = if window.samples[0] > 0.15 { "them" } else { "me" };
        Ok(Transcription {
            text: format!("{who} at {:.2}", window.start),
            words: vec![
                TranscriptWord { text: "hello".into(), start: 0.0, end: 1.0 },
                TranscriptWord { text: "there".into(), start: 1.0, end: 2.0 },
            ],
        })
    }
}

struct FakeDiarizer {
    calls: Arc<AtomicUsize>,
    result: fn() -> Result<Vec<SpeakerTurn>>,
}

impl Diarize for FakeDiarizer {
    async fn turns(&mut self, _window: &AudioWindow) -> Result<Vec<SpeakerTurn>> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        (self.result)()
    }
}

fn two_speakers() -> Result<Vec<SpeakerTurn>> {
    Ok(vec![SpeakerTurn { id: "1".into(), start: 0.0, end: 1.0 }, SpeakerTurn { id: "2".into(), start: 1.0, end: 2.0 }])
}

fn no_turns() -> Result<Vec<SpeakerTurn>> {
    Ok(vec![])
}

fn broken() -> Result<Vec<SpeakerTurn>> {
    Err(Error::message("no voices"))
}

struct Run {
    segments: Vec<TranscriptSegment>,
    emitted: Vec<TranscriptSegment>,
    errors: Vec<String>,
    diarizer_calls: usize,
}

fn chunk(speaker: Speaker, seconds: f64, ending_at: f64) -> Chunk {
    let level = if speaker == Speaker::Me { ME_LEVEL } else { THEM_LEVEL };
    Chunk { speaker, samples: vec![level; (seconds * SAMPLE_RATE) as usize], time: ending_at }
}

async fn run(
    transcriber: FakeTranscriber,
    turns: fn() -> Result<Vec<SpeakerTurn>>,
    chunks: Vec<Chunk>,
    folder: Option<&Path>,
) -> Run {
    let emitted = Arc::new(Mutex::new(Vec::new()));
    let errors = Arc::new(Mutex::new(Vec::new()));
    let calls = Arc::new(AtomicUsize::new(0));
    let worker = Worker {
        transcriber,
        diarizer: FakeDiarizer { calls: calls.clone(), result: turns },
        on_segment: {
            let emitted = emitted.clone();
            Arc::new(move |segment| emitted.lock().unwrap().push(segment))
        },
        on_error: {
            let errors = errors.clone();
            Arc::new(move |error| errors.lock().unwrap().push(error))
        },
    };
    let (sender, receiver) = unbounded_channel();
    for chunk in chunks {
        sender.send(chunk).unwrap();
    }
    drop(sender);
    let segments = worker.run(receiver, folder).await;
    let emitted = emitted.lock().unwrap().clone();
    let errors = errors.lock().unwrap().clone();
    Run { segments, emitted, errors, diarizer_calls: calls.load(Ordering::SeqCst) }
}

fn working() -> FakeTranscriber {
    FakeTranscriber { fail: false, silent: false }
}

#[tokio::test]
async fn windows_each_stream_separately_with_recording_time_offsets() {
    let chunks = vec![chunk(Speaker::Me, 20.0, 25.0), chunk(Speaker::Them, 20.0, 30.0)];
    let result = run(working(), no_turns, chunks, None).await;
    let summary: Vec<_> = result.segments.iter().map(|s| (s.speaker, s.text.as_str())).collect();
    // Each stream: one full window when 18 s is reached, cut at 12.05 s, then its tail on stop.
    assert_eq!(
        summary,
        [
            (Speaker::Me, "me at 5.00"),
            (Speaker::Them, "them at 10.00"),
            (Speaker::Them, "them at 22.05"),
            (Speaker::Me, "me at 17.05"),
        ]
    );
    assert!((result.segments[0].end - 17.05).abs() < 1e-9);
    assert_eq!(result.emitted, result.segments);
}

#[tokio::test]
async fn flushes_call_audio_before_the_microphone() {
    let chunks = vec![chunk(Speaker::Me, 3.0, 3.0), chunk(Speaker::Them, 3.0, 4.0)];
    let result = run(working(), no_turns, chunks, None).await;
    let speakers: Vec<_> = result.segments.iter().map(|s| s.speaker).collect();
    assert_eq!(speakers, [Speaker::Them, Speaker::Me]);
}

#[tokio::test]
async fn splits_only_call_audio_between_speakers() {
    let chunks = vec![chunk(Speaker::Me, 3.0, 3.0), chunk(Speaker::Them, 3.0, 13.0)];
    let result = run(working(), two_speakers, chunks, None).await;
    assert_eq!(result.diarizer_calls, 1);
    let summary: Vec<_> =
        result.segments.iter().map(|s| (s.speaker, s.speaker_id.as_deref(), s.text.as_str(), s.start)).collect();
    assert_eq!(
        summary,
        [
            (Speaker::Them, Some("1"), "hello", 10.0),
            (Speaker::Them, Some("2"), "there", 11.0),
            (Speaker::Me, None, "me at 0.00", 0.0),
        ]
    );
}

#[tokio::test]
async fn keeps_the_whole_window_when_no_turns_are_found() {
    let result = run(working(), no_turns, vec![chunk(Speaker::Them, 3.0, 3.0)], None).await;
    assert_eq!(result.segments.len(), 1);
    assert_eq!(result.segments[0].speaker_id, None);
    assert_eq!(result.segments[0].text, "them at 0.00");
}

#[tokio::test]
async fn a_diarization_failure_is_reported_once_and_disables_it() {
    let chunks = vec![chunk(Speaker::Them, 20.0, 20.0), chunk(Speaker::Them, 20.0, 40.0)];
    let result = run(working(), broken, chunks, None).await;
    assert_eq!(result.diarizer_calls, 1);
    assert_eq!(
        result.errors,
        ["Speaker detection is unavailable for this recording; the transcript is still being saved. no voices"]
    );
    assert_eq!(result.segments.len(), 3);
    assert!(result.segments.iter().all(|s| s.speaker_id.is_none()));
}

#[tokio::test]
async fn a_transcription_error_is_reported_once() {
    let chunks = vec![chunk(Speaker::Me, 20.0, 20.0), chunk(Speaker::Them, 20.0, 20.0)];
    let result = run(FakeTranscriber { fail: true, silent: false }, two_speakers, chunks, None).await;
    assert_eq!(result.errors, ["model broke"]);
    assert!(result.segments.is_empty());
    assert_eq!(result.diarizer_calls, 0);
}

#[tokio::test]
async fn skips_windows_without_text() {
    let result =
        run(FakeTranscriber { fail: false, silent: true }, two_speakers, vec![chunk(Speaker::Them, 3.0, 3.0)], None)
            .await;
    assert!(result.segments.is_empty());
    assert_eq!(result.diarizer_calls, 0);
}

#[tokio::test]
async fn keeps_both_streams_as_wav_files() {
    let folder = tempfile::tempdir().unwrap();
    let chunks = vec![chunk(Speaker::Me, 1.0, 1.0), chunk(Speaker::Them, 2.0, 2.0), chunk(Speaker::Me, 0.5, 1.5)];
    run(working(), no_turns, chunks, Some(folder.path())).await;
    let me = hound::WavReader::open(folder.path().join("me.wav")).unwrap();
    assert_eq!((me.spec().sample_rate, me.spec().channels, me.len()), (16_000, 1, 24_000));
    assert_eq!(hound::WavReader::open(folder.path().join("them.wav")).unwrap().len(), 32_000);
}
