//! Converts captured audio to the 16 kHz mono f32 the models expect.
//! Stateful by design: filter history carries across callbacks so chunk boundaries do not click.

use minutes_core::windower::SAMPLE_RATE;
use rubato::audioadapter_buffers::direct::InterleavedSlice;
use rubato::{Fft, FixedSync, Resampler as _};

const TARGET_RATE: usize = SAMPLE_RATE as usize;
/// Input frames per FFT pass; about 21 ms at 48 kHz, so latency stays negligible.
const CHUNK: usize = 1_024;

/// Averages interleaved channels into one.
pub(crate) fn downmix(interleaved: &[f32], channels: usize) -> Vec<f32> {
    if channels <= 1 {
        return interleaved.to_vec();
    }
    interleaved.chunks_exact(channels).map(|frame| frame.iter().sum::<f32>() / channels as f32).collect()
}

/// Streams mono audio at any rate to 16 kHz. Rebuilds itself when the input rate changes
/// (e.g. AirPods switching to the 24 kHz headset profile mid-call).
#[derive(Default)]
pub(crate) struct Resampler {
    stage: Option<Stage>,
}

struct Stage {
    rate: u32,
    /// None when the input is already 16 kHz.
    fft: Option<Fft<f32>>,
    pending: Vec<f32>,
    output: Vec<f32>,
    /// Leading output frames that are only the filter's start-up delay.
    delay_left: usize,
}

impl Resampler {
    /// Returns the 16 kHz samples that `mono` (at `rate` Hz) completes. A few milliseconds of
    /// input stay buffered until the next call.
    pub(crate) fn process(&mut self, rate: u32, mono: &[f32]) -> Vec<f32> {
        if self.stage.as_ref().is_none_or(|stage| stage.rate != rate) {
            self.stage = Stage::new(rate);
        }
        self.stage.as_mut().map(|stage| stage.process(mono)).unwrap_or_default()
    }
}

impl Stage {
    fn new(rate: u32) -> Option<Self> {
        let fft = if rate as usize == TARGET_RATE {
            None
        } else {
            match Fft::<f32>::new(rate as usize, TARGET_RATE, CHUNK, 1, FixedSync::Input) {
                Ok(fft) => Some(fft),
                Err(error) => {
                    log::error!("Cannot resample {rate} Hz audio: {error}");
                    return None;
                }
            }
        };
        let (output, delay_left) =
            fft.as_ref().map(|f| (vec![0.0; f.output_frames_max()], f.output_delay())).unwrap_or_default();
        Some(Self { rate, fft, pending: Vec::new(), output, delay_left })
    }

    fn process(&mut self, mono: &[f32]) -> Vec<f32> {
        let Some(fft) = self.fft.as_mut() else {
            return mono.to_vec();
        };
        self.pending.extend_from_slice(mono);
        let mut result = Vec::with_capacity(mono.len() * TARGET_RATE / self.rate as usize + 1);
        let mut consumed = 0;
        while self.pending.len() - consumed >= fft.input_frames_next() {
            let needed = fft.input_frames_next();
            let written = run_chunk(fft, &self.pending[consumed..consumed + needed], &mut self.output);
            consumed += needed;
            let skip = self.delay_left.min(written);
            self.delay_left -= skip;
            result.extend_from_slice(&self.output[skip..written]);
        }
        self.pending.drain(..consumed);
        result
    }
}

/// Resamples exactly one input chunk and returns how many output frames were written.
fn run_chunk(fft: &mut Fft<f32>, input: &[f32], output: &mut [f32]) -> usize {
    let frames_out = output.len();
    let (Ok(input), Ok(mut output)) =
        (InterleavedSlice::new(input, 1, input.len()), InterleavedSlice::new_mut(output, 1, frames_out))
    else {
        return 0;
    };
    match fft.process_into_buffer(&input, &mut output, None) {
        Ok((_, written)) => written,
        Err(error) => {
            log::error!("Resampling failed: {error}");
            0
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sine(rate: u32, frequency: f32, seconds: f32) -> Vec<f32> {
        let count = (rate as f32 * seconds) as usize;
        (0..count).map(|i| (2.0 * std::f32::consts::PI * frequency * i as f32 / rate as f32).sin() * 0.5).collect()
    }

    /// Streams `input` through the resampler in uneven callback-sized pieces.
    fn stream(rate: u32, input: &[f32]) -> Vec<f32> {
        let mut resampler = Resampler::default();
        input.chunks(437).flat_map(|piece| resampler.process(rate, piece)).collect()
    }

    /// Amplitude of `frequency` in 16 kHz `samples` (single-bin DFT).
    fn amplitude(samples: &[f32], frequency: f32) -> f32 {
        let (mut re, mut im) = (0.0f64, 0.0f64);
        for (i, s) in samples.iter().enumerate() {
            let phase = 2.0 * std::f64::consts::PI * frequency as f64 * i as f64 / TARGET_RATE as f64;
            re += *s as f64 * phase.cos();
            im += *s as f64 * phase.sin();
        }
        (2.0 * (re * re + im * im).sqrt() / samples.len() as f64) as f32
    }

    fn zero_crossings(samples: &[f32]) -> usize {
        samples.windows(2).filter(|pair| (pair[0] < 0.0) != (pair[1] < 0.0)).count()
    }

    #[test]
    fn keeps_a_1khz_tone_when_going_from_48k_to_16k() {
        let output = stream(48_000, &sine(48_000, 1_000.0, 1.0));
        // All but the buffered tail (under one chunk) comes out, with the start-up delay trimmed.
        assert!((15_500..=16_000).contains(&output.len()), "got {} samples", output.len());
        let middle = &output[2_000..14_000];
        // 1 kHz crosses zero 2 000 times a second: 1 500 times in these 0.75 s.
        let crossings = zero_crossings(middle);
        assert!((1_498..=1_502).contains(&crossings), "got {crossings} crossings");
        assert!((amplitude(middle, 1_000.0) - 0.5).abs() < 0.02);
    }

    #[test]
    fn handles_44_1k_and_24k_input() {
        for rate in [44_100, 24_000] {
            let output = stream(rate, &sine(rate, 440.0, 2.0));
            assert!((31_000..=32_000).contains(&output.len()), "{rate}: got {} samples", output.len());
            assert!((amplitude(&output[4_000..28_000], 440.0) - 0.5).abs() < 0.02, "{rate}");
        }
    }

    #[test]
    fn filters_out_frequencies_above_the_new_nyquist() {
        let output = stream(48_000, &sine(48_000, 12_000.0, 1.0));
        // Without a low-pass, 12 kHz would alias to 4 kHz at full strength.
        assert!(amplitude(&output[2_000..14_000], 4_000.0) < 0.005);
    }

    #[test]
    fn passes_16k_through_unchanged() {
        let input = sine(16_000, 300.0, 0.1);
        assert_eq!(stream(16_000, &input), input);
    }

    #[test]
    fn rebuilds_when_the_rate_changes() {
        let mut resampler = Resampler::default();
        resampler.process(48_000, &sine(48_000, 500.0, 0.5));
        let input = sine(16_000, 500.0, 0.1);
        assert_eq!(resampler.process(16_000, &input), input);
    }

    #[test]
    fn downmixes_interleaved_frames() {
        assert_eq!(downmix(&[1.0, 0.0, 0.5, 0.5], 2), [0.5, 0.5]);
        assert_eq!(downmix(&[0.25], 1), [0.25]);
    }
}
