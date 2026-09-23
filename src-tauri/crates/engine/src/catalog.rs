//! The on-device models the user can choose between, and which suit this Mac.

use std::path::Path;

use crate::model_download::{Model, ModelProgress};

const NEMO_TRANSDUCER_FILES: &[&str] = &["encoder.int8.onnx", "decoder.int8.onnx", "joiner.int8.onnx", "tokens.txt"];

pub(crate) const PARAKEET_V3: Model = Model {
    name: "sherpa-onnx-nemo-parakeet-tdt-0.6b-v3-int8",
    url_path: "asr-models/sherpa-onnx-nemo-parakeet-tdt-0.6b-v3-int8.tar.bz2",
    files: NEMO_TRANSDUCER_FILES,
    noun: "speech model",
};

pub(crate) const PARAKEET_V2: Model = Model {
    name: "sherpa-onnx-nemo-parakeet-tdt-0.6b-v2-int8",
    url_path: "asr-models/sherpa-onnx-nemo-parakeet-tdt-0.6b-v2-int8.tar.bz2",
    files: NEMO_TRANSDUCER_FILES,
    noun: "speech model",
};

pub(crate) const PARAKEET_110M: Model = Model {
    name: "sherpa-onnx-nemo-parakeet_tdt_transducer_110m-en-36000-int8",
    url_path: "asr-models/sherpa-onnx-nemo-parakeet_tdt_transducer_110m-en-36000-int8.tar.bz2",
    files: NEMO_TRANSDUCER_FILES,
    noun: "speech model",
};

pub(crate) const PYANNOTE: Model = Model {
    name: "sherpa-onnx-pyannote-segmentation-3-0",
    url_path: "speaker-segmentation-models/sherpa-onnx-pyannote-segmentation-3-0.tar.bz2",
    files: &["model.onnx"],
    noun: "speaker model",
};

/// The release tag really is spelled "recongition".
pub(crate) const RESNET34: Model = Model {
    name: "wespeaker_en_voxceleb_resnet34_LM.onnx",
    url_path: "speaker-recongition-models/wespeaker_en_voxceleb_resnet34_LM.onnx",
    files: &[],
    noun: "speaker model",
};

pub(crate) const TITANET_LARGE: Model = Model {
    name: "nemo_en_titanet_large.onnx",
    url_path: "speaker-recongition-models/nemo_en_titanet_large.onnx",
    files: &[],
    noun: "speaker model",
};

pub(crate) const QWEN35_4B: Model = Model {
    name: "Qwen3.5-4B-Q4_K_M.gguf",
    url_path: "https://huggingface.co/unsloth/Qwen3.5-4B-GGUF/resolve/main/Qwen3.5-4B-Q4_K_M.gguf",
    files: &[],
    noun: "notes model",
};

pub(crate) const QWEN35_9B: Model = Model {
    name: "Qwen3.5-9B-Q4_K_M.gguf",
    url_path: "https://huggingface.co/unsloth/Qwen3.5-9B-GGUF/resolve/main/Qwen3.5-9B-Q4_K_M.gguf",
    files: &[],
    noun: "notes model",
};

/// Similarity cut-offs, which differ per voice model because each spreads voices differently.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct SpeakerTuning {
    /// Cosine distance below which segments within one window merge into one speaker.
    pub cluster_threshold: f32,
    /// Minimum cosine similarity for a window's speaker to count as an already known voice.
    pub same_speaker: f32,
    /// Minimum similarity between two voices' averages for the end-of-meeting pass to merge them.
    pub merge: f32,
}

/// The models that find who spoke when (segmentation) and tell voices apart (embedding).
#[derive(Clone, Copy)]
pub(crate) struct SpeakerSetup {
    pub segmentation: &'static Model,
    pub embedding: &'static Model,
    pub tuning: SpeakerTuning,
}

/// A speech recognition model the user can pick.
pub struct SpeechOption {
    pub id: &'static str,
    pub name: &'static str,
    pub description: &'static str,
    pub languages: &'static str,
    /// Download size.
    pub size_mb: u32,
    pub(crate) model: &'static Model,
}

/// A pair of speaker recognition models the user can pick.
pub struct SpeakerOption {
    pub id: &'static str,
    pub name: &'static str,
    pub description: &'static str,
    /// Download size of both models.
    pub size_mb: u32,
    pub(crate) setup: SpeakerSetup,
}

pub const SPEECH_OPTIONS: &[SpeechOption] = &[
    SpeechOption {
        id: "parakeet-v3",
        name: "Parakeet v3",
        description: "Accurate in English and 24 other European languages, and detects the language by itself.",
        languages: "25 European languages",
        size_mb: 464,
        model: &PARAKEET_V3,
    },
    SpeechOption {
        id: "parakeet-v2",
        name: "Parakeet v2 English",
        description: "English only. Slightly ahead of v3 on English benchmarks, about the same in meetings. Same speed.",
        languages: "English",
        size_mb: 460,
        model: &PARAKEET_V2,
    },
    SpeechOption {
        id: "parakeet-lite",
        name: "Parakeet Lite English",
        description: "English only. A quarter of the processing and a fifth of the download, with a few more mistakes.",
        languages: "English",
        size_mb: 103,
        model: &PARAKEET_110M,
    },
];

pub const SPEAKER_OPTIONS: &[SpeakerOption] = &[
    SpeakerOption {
        id: "standard",
        name: "Standard",
        description: "The smallest download. Fine for one-to-one calls; can merge similar voices in group calls.",
        size_mb: 32,
        setup: SpeakerSetup {
            segmentation: &PYANNOTE,
            embedding: &RESNET34,
            // Tuned on AMI meetings (see speaker_eval.rs).
            tuning: SpeakerTuning { cluster_threshold: 0.6, same_speaker: 0.5, merge: 0.5 },
        },
    },
    SpeakerOption {
        id: "accurate",
        name: "Accurate",
        description: "Keeps similar voices apart in group calls, where Standard can merge them. Just as fast.",
        size_mb: 108,
        setup: SpeakerSetup {
            segmentation: &PYANNOTE,
            embedding: &TITANET_LARGE,
            // Tuned on AMI meetings (see speaker_eval.rs): about a third fewer attribution errors than Standard.
            tuning: SpeakerTuning { cluster_threshold: 0.5, same_speaker: 0.6, merge: 0.7 },
        },
    },
];

/// A language model that writes the notes on this Mac instead of with a connected account.
pub struct NoteOption {
    pub id: &'static str,
    pub name: &'static str,
    pub description: &'static str,
    pub size_mb: u32,
    /// Most tokens (prompt and reply) one request may use. Sized so a long meeting fits whole.
    pub context_tokens: u32,
    /// Transcripts longer than this many characters are digested in parts first.
    pub chunk_chars: usize,
    /// Notes: transcripts longer than this many of the model's tokens are digested in parts first.
    pub chunk_tokens: usize,
    pub(crate) model: &'static Model,
}

/// Both are Qwen3.5: only a quarter of their layers keep a full memory of the text, so a whole
/// hour-long meeting fits in well under a gigabyte of working memory.
pub const NOTE_OPTIONS: &[NoteOption] = &[
    NoteOption {
        id: "qwen3.5-4b",
        name: "Qwen3.5 4B",
        description: "Writes notes in a minute or two and stays light on memory and battery.",
        size_mb: 2741,
        context_tokens: 24_576,
        chunk_chars: 64_000,
        chunk_tokens: 20_000,
        model: &QWEN35_4B,
    },
    NoteOption {
        id: "qwen3.5-9b",
        name: "Qwen3.5 9B",
        description: "Sharper notes for long or technical meetings. About twice as slow and needs 16 GB of memory or more.",
        size_mb: 5681,
        context_tokens: 24_576,
        chunk_chars: 64_000,
        chunk_tokens: 20_000,
        model: &QWEN35_9B,
    },
];

pub const DEFAULT_NOTES: &str = "qwen3.5-4b";

pub const DEFAULT_SPEECH: &str = "parakeet-v3";
pub const DEFAULT_SPEAKER: &str = "accurate";

/// The option with `id`, or the default when it is unknown (for example, removed in an update).
pub fn speech_option(id: &str) -> &'static SpeechOption {
    SPEECH_OPTIONS.iter().find(|option| option.id == id).unwrap_or_else(|| speech_option(DEFAULT_SPEECH))
}

/// The option with `id`, or the default when it is unknown.
pub fn speaker_option(id: &str) -> &'static SpeakerOption {
    SPEAKER_OPTIONS.iter().find(|option| option.id == id).unwrap_or_else(|| speaker_option(DEFAULT_SPEAKER))
}

/// The option with `id`, or the default when it is unknown.
pub fn note_option(id: &str) -> &'static NoteOption {
    NOTE_OPTIONS.iter().find(|option| option.id == id).unwrap_or_else(|| note_option(DEFAULT_NOTES))
}

impl NoteOption {
    pub fn is_installed(&self, models_dir: &Path) -> bool {
        self.model.is_present(models_dir)
    }

    /// Downloads the model unless it is already there.
    pub async fn download(&self, models_dir: &Path, progress: impl Fn(ModelProgress) + Send + Sync + 'static) -> minutes_core::Result<()> {
        crate::model_download::ensure(self.model, models_dir, &progress).await
    }

    /// Deletes the downloaded file. The caller makes sure the option is not in use.
    pub fn remove(&self, models_dir: &Path) -> std::io::Result<()> {
        remove(models_dir, &[self.model])
    }
}

impl SpeechOption {
    pub(crate) fn models(&self) -> [&'static Model; 1] {
        [self.model]
    }

    pub fn is_installed(&self, models_dir: &Path) -> bool {
        self.models().iter().all(|model| model.is_present(models_dir))
    }

    /// Deletes the downloaded files. The caller makes sure the option is not in use.
    pub fn remove(&self, models_dir: &Path) -> std::io::Result<()> {
        remove(models_dir, &self.models())
    }
}

impl SpeakerOption {
    pub(crate) fn models(&self) -> [&'static Model; 2] {
        [self.setup.segmentation, self.setup.embedding]
    }

    pub fn is_installed(&self, models_dir: &Path) -> bool {
        self.models().iter().all(|model| model.is_present(models_dir))
    }

    /// Deletes the downloaded files. The caller makes sure the option is not in use.
    pub fn remove(&self, models_dir: &Path) -> std::io::Result<()> {
        remove(models_dir, &self.models())
    }
}

fn remove(models_dir: &Path, models: &[&Model]) -> std::io::Result<()> {
    for model in models {
        let path = model.path(models_dir);
        let result = if path.is_dir() { std::fs::remove_dir_all(&path) } else { std::fs::remove_file(&path) };
        match result {
            Err(error) if error.kind() != std::io::ErrorKind::NotFound => return Err(error),
            _ => {}
        }
    }
    Ok(())
}

/// What matters about this Mac when choosing models.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Hardware {
    pub memory_gb: u32,
    pub apple_silicon: bool,
    pub cores: u32,
}

impl Hardware {
    pub fn detect() -> Self {
        let sysctl = |name: &str| -> Option<u64> {
            let output = std::process::Command::new("/usr/sbin/sysctl").args(["-n", name]).output().ok()?;
            String::from_utf8(output.stdout).ok()?.trim().parse().ok()
        };
        Self {
            memory_gb: sysctl("hw.memsize").map_or(8, |bytes| (bytes >> 30) as u32),
            apple_silicon: cfg!(target_arch = "aarch64") || sysctl("sysctl.proc_translated") == Some(1),
            cores: std::thread::available_parallelism().map_or(4, |cores| cores.get() as u32),
        }
    }
}

/// Below this, even the larger speaker model's extra ~100 MB is worth saving.
const MIN_MEMORY_GB: u32 = 8;

/// The best (speech, speaker) option ids for this Mac. All run at a small fraction of real time;
/// the lighter ones are for Macs where every bit of memory and battery counts. `english` is whether
/// the Mac's preferred language is English, as the light speech model understands only English.
pub fn recommend(hardware: Hardware, english: bool) -> (&'static str, &'static str) {
    let roomy = hardware.memory_gb >= MIN_MEMORY_GB;
    let speech = if english && !(hardware.apple_silicon && roomy) { "parakeet-lite" } else { "parakeet-v3" };
    let speaker = if roomy { "accurate" } else { "standard" };
    (speech, speaker)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn mac(memory_gb: u32, apple_silicon: bool) -> Hardware {
        Hardware { memory_gb, apple_silicon, cores: 8 }
    }

    #[test]
    fn recommends_the_most_accurate_models_a_mac_can_run_comfortably() {
        assert_eq!(recommend(mac(16, true), true), ("parakeet-v3", "accurate"));
        assert_eq!(recommend(mac(8, true), false), ("parakeet-v3", "accurate"));
        assert_eq!(recommend(mac(16, false), true), ("parakeet-lite", "accurate"));
        assert_eq!(recommend(mac(4, true), true), ("parakeet-lite", "standard"));
        // The light model is English only, so other languages keep the full one.
        assert_eq!(recommend(mac(4, false), false), ("parakeet-v3", "standard"));
    }

    #[test]
    fn every_recommendation_and_default_is_in_the_catalog() {
        for hardware in [mac(4, false), mac(8, true), mac(64, true)] {
            for english in [true, false] {
                let (speech, speaker) = recommend(hardware, english);
                assert_eq!(speech_option(speech).id, speech);
                assert_eq!(speaker_option(speaker).id, speaker);
            }
        }
        assert_eq!(speech_option(DEFAULT_SPEECH).id, DEFAULT_SPEECH);
        assert_eq!(speaker_option(DEFAULT_SPEAKER).id, DEFAULT_SPEAKER);
        assert_eq!(speech_option("gone").id, DEFAULT_SPEECH);
        assert_eq!(note_option(DEFAULT_NOTES).id, DEFAULT_NOTES);
        assert_eq!(note_option("gone").id, DEFAULT_NOTES);
    }

    #[test]
    fn removing_an_option_deletes_only_its_files() {
        let dir = tempfile::tempdir().unwrap();
        let lite = speech_option("parakeet-lite");
        let v3 = speech_option("parakeet-v3");
        for option in [lite, v3] {
            let folder = option.model.path(dir.path());
            std::fs::create_dir_all(&folder).unwrap();
            for file in option.model.files {
                std::fs::write(folder.join(file), b"x").unwrap();
            }
        }
        assert!(lite.is_installed(dir.path()));
        lite.remove(dir.path()).unwrap();
        assert!(!lite.is_installed(dir.path()));
        assert!(v3.is_installed(dir.path()));
        lite.remove(dir.path()).unwrap(); // already gone is fine
    }
}
