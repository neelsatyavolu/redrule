//! Records system audio and the microphone for N seconds, printing segments as they arrive.
//! Usage: cargo run -p minutes-engine --release --example record -- [seconds] [microphone-uid] [audio-folder]
//! System audio needs Screen Recording permission for the terminal running this.

use std::error::Error;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use minutes_core::transcript::timestamp;
use minutes_engine::{RecordingPipeline, Transcriber, microphones};

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let seconds: u64 = args.first().map(|s| s.parse()).transpose()?.unwrap_or(20);
    let microphone = args.get(1).cloned().unwrap_or_default();
    let audio_folder = args.get(2).map(PathBuf::from);

    println!("Microphones:");
    for mic in microphones() {
        println!("  {} ({})", mic.name, mic.id);
    }

    let models_dir = PathBuf::from(std::env::var("HOME")?).join("Library/Application Support/Minutes/models");
    let transcriber = Arc::new(Transcriber::new(models_dir));
    transcriber.prepare(|p| eprintln!("{}: {} bytes", p.stage, p.downloaded_bytes)).await?;

    let pipeline = RecordingPipeline::start(
        transcriber,
        audio_folder,
        microphone,
        Arc::new(|s| {
            println!(
                "[{}] {} {}: {}",
                timestamp(s.start),
                s.speaker.label(),
                s.speaker_id.as_deref().unwrap_or(""),
                s.text
            )
        }),
        Arc::new(|error| eprintln!("error: {error}")),
    )
    .await?;
    println!("Recording for {seconds} s…");
    tokio::time::sleep(Duration::from_secs(seconds)).await;
    let segments = pipeline.stop().await;
    println!("Stopped: {} segments", segments.len());
    Ok(())
}
