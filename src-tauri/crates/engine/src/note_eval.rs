//! Writes notes from saved transcripts with each on-device note model, downloading it if needed.
//! Prints the timing and writes each note as `<meeting>__<model id>.md` for reading side by side.
//!
//! EVAL_MODELS_DIR=<models> EVAL_OUT=<dir> EVAL_TRANSCRIPTS=<a/transcript.json,b/transcript.json> \
//! [EVAL_NOTES=qwen3.5-4b] cargo test -p minutes-engine --release note_eval -- --ignored --nocapture

use std::path::{Path, PathBuf};
use std::time::Instant;

use chrono::Utc;
use minutes_core::models::{Meeting, MeetingApp, MeetingStatus, TranscriptSegment};
use minutes_core::summary::summarize;

use crate::LocalNoteWriter;
use crate::catalog::NOTE_OPTIONS;

#[tokio::test(flavor = "multi_thread")]
#[ignore = "downloads gigabytes of models; run by hand"]
async fn note_eval() {
    let var = |name: &str| std::env::var(name).unwrap_or_else(|_| panic!("set {name}"));
    let models_dir = PathBuf::from(var("EVAL_MODELS_DIR"));
    let out = PathBuf::from(var("EVAL_OUT"));
    std::fs::create_dir_all(&out).unwrap();
    let chosen = std::env::var("EVAL_NOTES").ok();
    let options = NOTE_OPTIONS.iter().filter(|option| chosen.as_deref().is_none_or(|ids| ids.split(',').any(|id| id == option.id)));
    for option in options {
        option.download(&models_dir, |_| {}).await.unwrap();
        let clock = Instant::now();
        let writer = LocalNoteWriter::load(option, &models_dir).unwrap();
        println!("{} loaded in {:.1}s", option.id, clock.elapsed().as_secs_f32());
        for path in var("EVAL_TRANSCRIPTS").split(',') {
            let segments: Vec<TranscriptSegment> = serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap();
            let clock = Instant::now();
            let note = summarize(&writer, &meeting(), &segments, option.chunk_chars).await.unwrap();
            let name = Path::new(path).parent().and_then(Path::file_name).unwrap().to_string_lossy().into_owned();
            println!("{name} {} {:.1}s: {}", option.id, clock.elapsed().as_secs_f32(), note.title);
            std::fs::write(out.join(format!("{name}__{}.md", option.id)), note.markdown()).unwrap();
        }
    }
}

fn meeting() -> Meeting {
    Meeting {
        id: "EVAL".into(),
        title: "New meeting".into(),
        app: MeetingApp::Zoom,
        started_at: Utc::now(),
        ended_at: None,
        status: MeetingStatus::Summarizing,
        error_message: None,
        archived_at: None,
    }
}
