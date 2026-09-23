//! Benchmarks the speaker models on labelled recordings, the way a meeting runs them: window by
//! window, then the end-of-meeting pass. Writes one RTTM per recording and setting for scoring.
//!
//! EVAL_MODELS_DIR=<models> EVAL_OUT=<dir> EVAL_WAVS=<a.wav,b.wav> \
//!   cargo test -p minutes-engine --release speaker_eval -- --ignored --nocapture
//! Optional: EVAL_SETUPS=segmentation/embedding,... (file names under the models folder).

use std::fmt::Write as _;
use std::path::{Path, PathBuf};
use std::time::Instant;

use minutes_core::windower::{AudioWindower, SAMPLE_RATE};

use crate::catalog::{SpeakerSetup, SpeakerTuning};
use crate::diarization::SpeakerModels;
use crate::model_download::Model;
use crate::speakers::{LocalSpeaker, SpeakerRegistry};

const CLUSTER_THRESHOLDS: &[f32] = &[0.5, 0.6, 0.7, 0.8];
const SAME_SPEAKER: &[f32] = &[0.3, 0.4, 0.5, 0.6, 0.7];
const MERGE: &[f32] = &[0.3, 0.4, 0.5, 0.6, 0.7, 0.8, 1.1];

#[test]
#[ignore = "needs models and recordings; run by hand"]
fn speaker_eval() {
    let var = |name: &str| std::env::var(name).unwrap_or_else(|_| panic!("set {name}"));
    let models_dir = PathBuf::from(var("EVAL_MODELS_DIR"));
    let out = PathBuf::from(var("EVAL_OUT"));
    std::fs::create_dir_all(&out).unwrap();
    let recordings: Vec<(String, Vec<f32>)> =
        var("EVAL_WAVS").split(',').map(|path| (stem(path), read_wav(Path::new(path)))).collect();
    let setups = std::env::var("EVAL_SETUPS").unwrap_or_else(|_| {
        "sherpa-onnx-pyannote-segmentation-3-0/wespeaker_en_voxceleb_resnet34_LM.onnx".into()
    });

    for setup in setups.split(',') {
        let (segmentation, embedding) = setup.split_once('/').expect("segmentation/embedding");
        let segmentation = leak_model(segmentation);
        let embedding = leak_model(embedding);
        for &cluster_threshold in CLUSTER_THRESHOLDS {
            let tuning = SpeakerTuning { cluster_threshold, same_speaker: 0.0, merge: 0.0 };
            let models = SpeakerModels::load(&models_dir, &SpeakerSetup { segmentation, embedding, tuning }).unwrap();
            for (name, samples) in &recordings {
                let clock = Instant::now();
                let windows = analyze(&models, samples);
                let rtf = clock.elapsed().as_secs_f64() / (samples.len() as f64 / SAMPLE_RATE);
                println!("{name} {} {} ct={cluster_threshold} rtf={rtf:.3}", segmentation.name, embedding.name);
                for &same_speaker in SAME_SPEAKER {
                    for &merge in MERGE {
                        let tuning = SpeakerTuning { cluster_threshold, same_speaker, merge };
                        let file = format!(
                            "{name}__{}__{}__ct{cluster_threshold}__ss{same_speaker}__mg{merge}.rttm",
                            short(segmentation.name),
                            short(embedding.name)
                        );
                        std::fs::write(out.join(file), rttm(name, &windows, tuning)).unwrap();
                    }
                }
            }
        }
    }
}

/// Each window's speakers, with the window's start time.
fn analyze(models: &SpeakerModels, samples: &[f32]) -> Vec<(f64, Vec<LocalSpeaker>)> {
    let seconds = samples.len() as f64 / SAMPLE_RATE;
    let mut windower = AudioWindower::new();
    let mut windows = windower.append(samples, seconds);
    windows.extend(windower.flush());
    windows.iter().map(|window| (window.start, models.analyze(&window.samples).unwrap())).collect()
}

/// The meeting's final speaker turns, as RTTM. Turns whose voice was dropped are left out.
fn rttm(name: &str, windows: &[(f64, Vec<LocalSpeaker>)], tuning: SpeakerTuning) -> String {
    let mut registry = SpeakerRegistry::new(tuning);
    let turns: Vec<_> = windows
        .iter()
        .flat_map(|(start, speakers)| {
            registry.turns(speakers).into_iter().map(move |turn| (start + turn.start, start + turn.end, turn.id))
        })
        .collect();
    let map = registry.finish();
    let mut text = String::new();
    for (start, end, id) in turns {
        if let Some(id) = map.final_id(&id) {
            let _ = writeln!(text, "SPEAKER {name} 1 {start:.3} {:.3} <NA> <NA> {id} <NA> <NA>", end - start);
        }
    }
    text
}

fn leak_model(name: &str) -> &'static Model {
    let name: &'static str = Box::leak(name.to_string().into_boxed_str());
    let files: &'static [&'static str] = if name.ends_with(".onnx") { &[] } else { &["model.onnx"] };
    Box::leak(Box::new(Model { name, url_path: "", files, noun: "model" }))
}

fn short(name: &str) -> String {
    name.trim_end_matches(".onnx").replace("sherpa-onnx-", "").chars().take(40).collect()
}

pub(crate) fn stem(path: &str) -> String {
    Path::new(path).file_stem().unwrap().to_string_lossy().into_owned()
}

pub(crate) fn read_wav(path: &Path) -> Vec<f32> {
    let mut reader = hound::WavReader::open(path).unwrap();
    let spec = reader.spec();
    assert_eq!((spec.sample_rate, spec.channels), (SAMPLE_RATE as u32, 1), "{path:?} must be 16 kHz mono");
    match spec.sample_format {
        hound::SampleFormat::Float => reader.samples::<f32>().map(Result::unwrap).collect(),
        hound::SampleFormat::Int => {
            let scale = (1u64 << (spec.bits_per_sample - 1)) as f32;
            reader.samples::<i32>().map(|s| s.unwrap() as f32 / scale).collect()
        }
    }
}
