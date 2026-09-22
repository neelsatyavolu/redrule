//! Appends 16 kHz mono samples to a WAV file. Failures are only logged: kept audio is a convenience
//! and must never interrupt the transcript.

use std::fs::File;
use std::io::BufWriter;
use std::path::Path;

use minutes_core::windower::SAMPLE_RATE;

pub(crate) struct WavWriter {
    writer: Option<hound::WavWriter<BufWriter<File>>>,
}

impl WavWriter {
    pub(crate) fn create(path: &Path) -> Self {
        let spec = hound::WavSpec {
            channels: 1,
            sample_rate: SAMPLE_RATE as u32,
            bits_per_sample: 32,
            sample_format: hound::SampleFormat::Float,
        };
        let writer = hound::WavWriter::create(path, spec)
            .inspect_err(|error| log::warn!("Cannot keep audio in {}: {error}", path.display()))
            .ok();
        Self { writer }
    }

    pub(crate) fn write(&mut self, samples: &[f32]) {
        let Some(writer) = self.writer.as_mut() else { return };
        if let Err(error) = samples.iter().try_for_each(|sample| writer.write_sample(*sample)) {
            log::warn!("Stopped keeping audio: {error}");
            self.writer = None;
        }
    }

    /// Writes the final header so the file is playable.
    pub(crate) fn finalize(self) {
        if let Some(Err(error)) = self.writer.map(hound::WavWriter::finalize) {
            log::warn!("Kept audio may be incomplete: {error}");
        }
    }
}
