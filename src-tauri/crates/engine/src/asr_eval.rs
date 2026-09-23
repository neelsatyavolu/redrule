//! Benchmarks the speech models on recordings, window by window as in a meeting. Writes each
//! model's transcript of each recording as `<recording>__<model id>.txt` for scoring.
//!
//! EVAL_MODELS_DIR=<models> EVAL_OUT=<dir> EVAL_WAVS=<a.wav,b.wav> \
//!   cargo test -p minutes-engine --release asr_eval -- --ignored --nocapture

use std::path::{Path, PathBuf};
use std::time::Instant;

use minutes_core::windower::{AudioWindower, SAMPLE_RATE};

use crate::catalog::SPEECH_OPTIONS;
use crate::speaker_eval::{read_wav, stem};

#[test]
#[ignore = "needs models and recordings; run by hand"]
fn asr_eval() {
    let var = |name: &str| std::env::var(name).unwrap_or_else(|_| panic!("set {name}"));
    let models_dir = PathBuf::from(var("EVAL_MODELS_DIR"));
    let out = PathBuf::from(var("EVAL_OUT"));
    std::fs::create_dir_all(&out).unwrap();
    for option in SPEECH_OPTIONS {
        let recognizer = crate::asr::load(&models_dir, option.model).unwrap();
        for path in var("EVAL_WAVS").split(',') {
            let samples = read_wav(Path::new(path));
            let seconds = samples.len() as f64 / SAMPLE_RATE;
            let mut windower = AudioWindower::new();
            let mut windows = windower.append(&samples, seconds);
            windows.extend(windower.flush());
            let clock = Instant::now();
            let text: Vec<String> = windows
                .iter()
                .map(|window| crate::asr::transcribe(&recognizer, &window.samples).unwrap().text)
                .collect();
            let rtf = clock.elapsed().as_secs_f64() / seconds;
            println!("{} {} rtf={rtf:.3}", stem(path), option.id);
            std::fs::write(out.join(format!("{}__{}.txt", stem(path), option.id)), text.join(" ")).unwrap();
        }
    }
}
