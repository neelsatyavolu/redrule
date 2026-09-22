//! System audio capture through ScreenCaptureKit, excluding this app's own sound.

use std::sync::Mutex;

use minutes_core::{Error, Result};
use screencapturekit::cm::AudioBuffer;
use screencapturekit::prelude::*;

use crate::mic_capture::SampleSink;
use crate::resampler::{Resampler, downmix};
use crate::worker::ErrorSink;

const NO_DISPLAY: &str = "No display was found to capture system audio from.";

pub(crate) struct SystemCapture {
    stream: SCStream,
}

impl SystemCapture {
    /// Starts capturing. Blocking: waits for ScreenCaptureKit, which also triggers the Screen
    /// Recording permission prompt on first use. `on_stopped` fires if macOS ends the stream.
    pub(crate) fn start(on_samples: SampleSink, on_stopped: ErrorSink) -> Result<Self> {
        let content = SCShareableContent::get().map_err(|error| Error::message(error.to_string()))?;
        let display = content.displays().into_iter().next().ok_or_else(|| Error::message(NO_DISPLAY))?;
        let filter = SCContentFilter::create().with_display(&display).with_excluding_windows(&[]).build();
        // Video is required by the API; keep it as cheap as possible.
        let configuration = SCStreamConfiguration::new()
            .with_width(2)
            .with_height(2)
            .with_fps(1)
            .with_captures_audio(true)
            .with_excludes_current_process_audio(true)
            .with_sample_rate(48_000)
            .with_channel_count(1);

        let delegate = ErrorHandler::new(move |error| on_stopped(error.to_string()));
        let mut stream = SCStream::new_with_delegate(&filter, &configuration, delegate);
        let resampler = Mutex::new(Resampler::default());
        stream.add_output_handler(
            move |sample: CMSampleBuffer, _: SCStreamOutputType| {
                let Some((rate, mono)) = mono_samples(&sample) else { return };
                let Ok(mut resampler) = resampler.lock() else { return };
                let samples = resampler.process(rate, &mono);
                if !samples.is_empty() {
                    on_samples(samples);
                }
            },
            SCStreamOutputType::Audio,
        );
        // Without a screen handler ScreenCaptureKit logs every dropped video frame.
        stream.add_output_handler(|_: CMSampleBuffer, _: SCStreamOutputType| {}, SCStreamOutputType::Screen);
        stream.start_capture().map_err(|error| Error::message(error.to_string()))?;
        Ok(Self { stream })
    }

    /// Blocking.
    pub(crate) fn stop(self) {
        if let Err(error) = self.stream.stop_capture() {
            log::warn!("Stopping system audio capture: {error}");
        }
    }
}

/// Reads a sample buffer as (rate, mono f32). The buffer is only valid inside the callback, so it
/// is copied out here. Handles float and integer PCM, interleaved or one buffer per channel.
fn mono_samples(sample: &CMSampleBuffer) -> Option<(u32, Vec<f32>)> {
    let format = sample.format_description()?;
    let rate = format.audio_sample_rate()? as u32;
    let pcm = Pcm::new(format.audio_is_float(), format.audio_bits_per_channel()?)?;
    let list = sample.audio_buffer_list()?;
    let planes: Vec<Vec<f32>> = list.iter().map(|buffer| pcm.plane(buffer)).collect();
    let frames = planes.iter().map(Vec::len).min()?;
    let mono = (0..frames).map(|i| planes.iter().map(|p| p[i]).sum::<f32>() / planes.len() as f32).collect();
    Some((rate, mono))
}

#[derive(Clone, Copy)]
enum Pcm {
    F32,
    I16,
    I32,
}

impl Pcm {
    fn new(is_float: bool, bits: u32) -> Option<Self> {
        match (is_float, bits) {
            (true, 32) => Some(Self::F32),
            (false, 16) => Some(Self::I16),
            (false, 32) => Some(Self::I32),
            _ => {
                log::error!("Unsupported system audio format: float={is_float}, {bits} bits");
                None
            }
        }
    }

    /// One buffer's samples, its interleaved channels averaged.
    fn plane(self, buffer: &AudioBuffer) -> Vec<f32> {
        let width = if matches!(self, Self::I16) { 2 } else { 4 };
        let samples: Vec<f32> = buffer.data().chunks_exact(width).map(|bytes| self.decode(bytes)).collect();
        downmix(&samples, buffer.number_channels().max(1) as usize)
    }

    fn decode(self, b: &[u8]) -> f32 {
        match self {
            Self::F32 => f32::from_ne_bytes([b[0], b[1], b[2], b[3]]),
            Self::I16 => i16::from_ne_bytes([b[0], b[1]]) as f32 / 32_768.0,
            Self::I32 => (i32::from_ne_bytes([b[0], b[1], b[2], b[3]]) as f64 / 2_147_483_648.0) as f32,
        }
    }
}
