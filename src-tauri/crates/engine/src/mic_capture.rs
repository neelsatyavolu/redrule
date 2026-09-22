//! Microphone capture through cpal. cpal streams are not `Send` on macOS, so each capture owns a
//! thread that holds the stream and rebuilds it when the device goes away or changes format.

use std::sync::Arc;
use std::sync::mpsc::{Sender, channel};
use std::thread::JoinHandle;
use std::time::Duration;

use cpal::traits::{DeviceTrait, StreamTrait};
use cpal::{ErrorKind, FromSample, SampleFormat, SizedSample, StreamConfig};
use minutes_core::{Error, Result};

use crate::microphones::resolve;
use crate::resampler::{Resampler, downmix};
use crate::worker::ErrorSink;

pub(crate) type SampleSink = Arc<dyn Fn(Vec<f32>) + Send + Sync>;

const SELECTION_FAILED: &str = "The selected microphone could not be opened. Choose another microphone in Settings.";
const OPEN_TIMEOUT: Duration = Duration::from_secs(5);

enum Control {
    Rebuild,
    Stop,
}

/// Stops capture when dropped: the stream's error callback holds a sender, so the thread would
/// otherwise wait forever with the microphone open.
pub(crate) struct MicCapture {
    control: Sender<Control>,
    thread: Option<JoinHandle<()>>,
}

impl MicCapture {
    /// Opens the microphone with this UID ("" = system default) and returns once audio is flowing.
    /// `on_error` is only called when a rebuild fails; capture has then stopped.
    pub(crate) fn start(uid: String, on_samples: SampleSink, on_error: ErrorSink) -> Result<Self> {
        let (control, commands) = channel();
        let (ready, started) = channel();
        let rebuild = control.clone();
        let thread = std::thread::Builder::new().name("minutes-microphone".into()).spawn(move || {
            let open = || open_stream(&uid, on_samples.clone(), rebuild.clone());
            let mut stream = match open() {
                Ok(stream) => stream,
                Err(error) => {
                    let _ = ready.send(Err(error));
                    return;
                }
            };
            let _ = ready.send(Ok(()));
            while let Ok(Control::Rebuild) = commands.recv() {
                // Plugging in headphones or AirPods can change the device or its format; start over.
                drop(stream);
                stream = match open() {
                    Ok(stream) => stream,
                    Err(error) => {
                        on_error(error.to_string());
                        return;
                    }
                };
            }
        })?;
        match started.recv() {
            Ok(Ok(())) => Ok(Self { control, thread: Some(thread) }),
            Ok(Err(error)) => Err(error),
            Err(_) => Err(Error::message(SELECTION_FAILED)),
        }
    }

    /// Blocking: waits for the stream to close.
    pub(crate) fn stop(self) {
        drop(self);
    }
}

impl Drop for MicCapture {
    fn drop(&mut self) {
        // The thread may already have ended after a failed rebuild.
        let _ = self.control.send(Control::Stop);
        if self.thread.take().is_some_and(|thread| thread.join().is_err()) {
            log::error!("The microphone thread panicked");
        }
    }
}

fn open_stream(uid: &str, on_samples: SampleSink, rebuild: Sender<Control>) -> Result<cpal::Stream> {
    let device = resolve(uid)?;
    let config = device.default_input_config().map_err(|error| failed(&error))?;
    let format = config.sample_format();
    let on_error = move |error: cpal::Error| match error.kind() {
        ErrorKind::DeviceNotAvailable | ErrorKind::StreamInvalidated => {
            let _ = rebuild.send(Control::Rebuild);
        }
        _ => log::warn!("Microphone: {error}"),
    };
    let config: StreamConfig = config.into();
    let stream = match format {
        SampleFormat::F32 => build::<f32>(&device, config, on_samples, on_error),
        SampleFormat::F64 => build::<f64>(&device, config, on_samples, on_error),
        SampleFormat::I16 => build::<i16>(&device, config, on_samples, on_error),
        SampleFormat::I32 => build::<i32>(&device, config, on_samples, on_error),
        SampleFormat::I8 => build::<i8>(&device, config, on_samples, on_error),
        SampleFormat::U16 => build::<u16>(&device, config, on_samples, on_error),
        SampleFormat::U8 => build::<u8>(&device, config, on_samples, on_error),
        other => {
            log::error!("Unsupported microphone sample format {other}");
            return Err(Error::message(SELECTION_FAILED));
        }
    }
    .map_err(|error| failed(&error))?;
    stream.play().map_err(|error| failed(&error))?;
    Ok(stream)
}

fn build<T>(
    device: &cpal::Device,
    config: StreamConfig,
    on_samples: SampleSink,
    on_error: impl FnMut(cpal::Error) + Send + 'static,
) -> std::result::Result<cpal::Stream, cpal::Error>
where
    T: SizedSample,
    f32: FromSample<T>,
{
    let (channels, rate) = (config.channels as usize, config.sample_rate);
    let mut resampler = Resampler::default();
    let on_data = move |data: &[T], _: &cpal::InputCallbackInfo| {
        let floats: Vec<f32> = data.iter().map(|sample| sample.to_sample::<f32>()).collect();
        let samples = resampler.process(rate, &downmix(&floats, channels));
        if !samples.is_empty() {
            on_samples(samples);
        }
    };
    device.build_input_stream(config, on_data, on_error, Some(OPEN_TIMEOUT))
}

fn failed(error: &dyn std::fmt::Display) -> Error {
    log::error!("Microphone: {error}");
    Error::message(SELECTION_FAILED)
}
