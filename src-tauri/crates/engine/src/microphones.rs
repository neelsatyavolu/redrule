//! Input devices, identified by CoreAudio UID because UIDs survive reconnects; numeric ids do not.

use cpal::traits::{DeviceTrait, HostTrait};
use minutes_core::{Error, Result};

pub(crate) const NO_MICROPHONE: &str = "No microphone is available.";
pub(crate) const MICROPHONE_DISCONNECTED: &str =
    "The selected microphone is disconnected. Connect it or choose another microphone in Settings.";

#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Microphone {
    /// CoreAudio device UID.
    pub id: String,
    pub name: String,
}

/// Input-capable devices, sorted by name.
pub fn microphones() -> Vec<Microphone> {
    let Ok(devices) = cpal::default_host().input_devices() else {
        return Vec::new();
    };
    let mut list: Vec<Microphone> = devices
        .filter_map(|device| {
            Some(Microphone {
                id: device.id().ok()?.id().to_string(),
                name: device.description().ok()?.name().to_string(),
            })
        })
        .collect();
    list.sort_by_cached_key(|microphone| microphone.name.to_lowercase());
    list
}

/// The device for a saved UID; an empty UID means the system default input.
pub(crate) fn resolve(uid: &str) -> Result<cpal::Device> {
    let host = cpal::default_host();
    if uid.is_empty() {
        return host.default_input_device().ok_or_else(|| Error::message(NO_MICROPHONE));
    }
    host.input_devices()
        .ok()
        .and_then(|mut devices| devices.find(|device| device.id().is_ok_and(|id| id.id() == uid)))
        .ok_or_else(|| Error::message(MICROPHONE_DISCONNECTED))
}
