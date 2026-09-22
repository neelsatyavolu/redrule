//! Cuts a continuous 16 kHz stream into windows for transcription, preferring to cut in a pause.
//! Mutable by design: it sits on the audio path and must not copy its buffer on every callback.

pub const SAMPLE_RATE: f64 = 16_000.0;
const MIN_WINDOW: f64 = 12.0;
const MAX_WINDOW: f64 = 18.0;
const MIN_TAIL: f64 = 0.5;
const FRAME: usize = 1_600; // 100 ms
const SILENCE_RMS: f32 = 0.003;

#[derive(Debug, Clone, PartialEq)]
pub struct AudioWindow {
    /// Seconds since the recording started.
    pub start: f64,
    pub samples: Vec<f32>,
}

impl AudioWindow {
    pub fn end(&self) -> f64 {
        self.start + self.samples.len() as f64 / SAMPLE_RATE
    }
}

#[derive(Debug, Default)]
pub struct AudioWindower {
    buffer: Vec<f32>,
    buffer_start: f64,
}

impl AudioWindower {
    pub fn new() -> Self {
        Self::default()
    }

    /// Adds samples whose last sample was captured `ending_at` seconds into the recording.
    pub fn append(&mut self, samples: &[f32], ending_at: f64) -> Vec<AudioWindow> {
        if self.buffer.is_empty() {
            self.buffer_start = (ending_at - samples.len() as f64 / SAMPLE_RATE).max(0.0);
        }
        self.buffer.extend_from_slice(samples);

        let mut windows = Vec::new();
        while self.buffer.len() as f64 / SAMPLE_RATE >= MAX_WINDOW {
            let cut = quietest_cut(&self.buffer);
            if let Some(window) = window(self.buffer_start, &self.buffer[..cut]) {
                windows.push(window);
            }
            self.buffer.drain(..cut);
            self.buffer_start += cut as f64 / SAMPLE_RATE;
        }
        windows
    }

    /// Returns whatever audio is left, or None if it is silent or too short to be speech.
    pub fn flush(&mut self) -> Option<AudioWindow> {
        let buffer = std::mem::take(&mut self.buffer);
        if (buffer.len() as f64 / SAMPLE_RATE) < MIN_TAIL {
            return None;
        }
        window(self.buffer_start, &buffer)
    }
}

fn window(start: f64, samples: &[f32]) -> Option<AudioWindow> {
    (rms(samples) >= SILENCE_RMS).then(|| AudioWindow { start, samples: samples.to_vec() })
}

/// End index of the quietest 100 ms frame between the minimum and maximum window length.
fn quietest_cut(samples: &[f32]) -> usize {
    let lower = (MIN_WINDOW * SAMPLE_RATE) as usize;
    let upper = samples.len().min((MAX_WINDOW * SAMPLE_RATE) as usize);
    let best = (lower..upper.saturating_sub(FRAME) + 1)
        .step_by(FRAME)
        .min_by(|&a, &b| rms(&samples[a..a + FRAME]).total_cmp(&rms(&samples[b..b + FRAME])))
        .unwrap_or(upper - FRAME);
    best + FRAME / 2
}

pub fn rms(samples: &[f32]) -> f32 {
    if samples.is_empty() {
        return 0.0;
    }
    (samples.iter().map(|s| s * s).sum::<f32>() / samples.len() as f32).sqrt()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tone(seconds: f64) -> Vec<f32> {
        (0..(seconds * SAMPLE_RATE) as usize).map(|i| (i as f32 * 0.05).sin() * 0.2).collect()
    }

    #[test]
    fn holds_audio_until_the_maximum_window() {
        let mut windower = AudioWindower::new();
        assert!(windower.append(&tone(10.0), 10.0).is_empty());
    }

    #[test]
    fn cuts_in_the_quietest_frame() {
        let mut audio = tone(19.0);
        let silence_start = (14.0 * SAMPLE_RATE) as usize;
        audio[silence_start..silence_start + FRAME].fill(0.0);
        let mut windower = AudioWindower::new();
        let windows = windower.append(&audio, 19.0);
        assert_eq!(windows.len(), 1);
        assert_eq!(windows[0].samples.len(), silence_start + FRAME / 2);
        assert_eq!(windows[0].start, 0.0);
    }

    #[test]
    fn flush_skips_silence_and_short_tails() {
        let mut windower = AudioWindower::new();
        windower.append(&vec![0.0; 32_000], 2.0);
        assert!(windower.flush().is_none());
        windower.append(&tone(0.2), 3.0);
        assert!(windower.flush().is_none());
        windower.append(&tone(2.0), 5.0);
        let tail = windower.flush().unwrap();
        assert!((tail.start - 3.0).abs() < 1e-9);
    }
}
