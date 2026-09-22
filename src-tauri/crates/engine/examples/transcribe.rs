//! Transcribes a 16 kHz mono WAV with real models and prints words and speakers.
//! Usage: cargo run -p minutes-engine --release --example transcribe -- <file.wav>
//! Models go to ~/Library/Application Support/Minutes/models (downloaded on first run).

use std::error::Error;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Instant;

use minutes_core::transcript::align_speakers;
use minutes_core::windower::{AudioWindower, SAMPLE_RATE};
use minutes_engine::{SpeakerRecognizer, Transcriber};

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let path = std::env::args().nth(1).ok_or("usage: transcribe <file.wav>")?;
    let samples = read_wav(&path)?;
    let seconds = samples.len() as f64 / SAMPLE_RATE;
    println!("{path}: {seconds:.1} s");

    let models_dir = PathBuf::from(std::env::var("HOME")?).join("Library/Application Support/Minutes/models");
    let transcriber = Arc::new(Transcriber::new(models_dir));
    let started = Instant::now();
    transcriber
        .prepare(|p| {
            let total = p.total_bytes.map_or("?".into(), |t| format!("{:.0}", t as f64 / 1e6));
            eprint!("\r{}: {:.0} / {total} MB      ", p.stage, p.downloaded_bytes as f64 / 1e6);
        })
        .await?;
    eprintln!();
    println!("prepare (download + load): {:.2} s, ready = {}", started.elapsed().as_secs_f64(), transcriber.is_ready());

    let mut windower = AudioWindower::new();
    let mut windows = windower.append(&samples, seconds);
    windows.extend(windower.flush());
    let mut speakers = SpeakerRecognizer::new(transcriber.clone());
    let (mut asr_time, mut diarization_time) = (0.0, 0.0);
    for window in &windows {
        println!("\n== window {:.2}–{:.2} s", window.start, window.end());
        let clock = Instant::now();
        let transcription = transcriber.transcribe(window).await?;
        asr_time += clock.elapsed().as_secs_f64();
        println!("text: {}", transcription.text);
        for word in &transcription.words {
            println!("  {:6.2}–{:6.2}  {}", word.start, word.end, word.text);
        }
        let clock = Instant::now();
        let turns = speakers.turns(window).await?;
        diarization_time += clock.elapsed().as_secs_f64();
        for turn in &turns {
            println!("  speaker {} {:6.2}–{:6.2}", turn.id, turn.start, turn.end);
        }
        for segment in align_speakers(&transcription.words, &turns, window.start) {
            println!(
                "  [{:6.2}] Speaker {}: {}",
                segment.start,
                segment.speaker_id.as_deref().unwrap_or("?"),
                segment.text
            );
        }
    }
    println!(
        "\nspeech: {asr_time:.2} s (RTF {:.3}), speakers: {diarization_time:.2} s (RTF {:.3})",
        asr_time / seconds,
        diarization_time / seconds
    );
    Ok(())
}

fn read_wav(path: &str) -> Result<Vec<f32>, Box<dyn Error>> {
    let mut reader = hound::WavReader::open(path)?;
    let spec = reader.spec();
    if spec.sample_rate != SAMPLE_RATE as u32 || spec.channels != 1 {
        return Err(format!("expected 16 kHz mono, got {} Hz × {}", spec.sample_rate, spec.channels).into());
    }
    Ok(match spec.sample_format {
        hound::SampleFormat::Float => reader.samples::<f32>().collect::<Result<_, _>>()?,
        hound::SampleFormat::Int => {
            let scale = (1u64 << (spec.bits_per_sample - 1)) as f32;
            reader.samples::<i32>().map(|s| s.map(|s| s as f32 / scale)).collect::<Result<_, _>>()?
        }
    })
}
