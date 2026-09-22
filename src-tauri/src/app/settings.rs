//! Preferences in `NSUserDefaults`, under the Swift app's keys, so they carry over.
use objc2_foundation::{NSString, NSUserDefaults};
use serde::{Deserialize, Serialize};

use crate::providers::clients::ModelChoice;

mod key {
    pub const MODEL_CHOICE: &str = "modelChoice";
    pub const KEEP_AUDIO: &str = "keepAudio";
    pub const MICROPHONE: &str = "microphoneUID";
    pub const ONBOARDED: &str = "onboarded";
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Settings {
    pub model_choice_id: String,
    pub keep_audio: bool,
    /// CoreAudio device UID, or empty for the system default.
    pub microphone_id: String,
    pub onboarded: bool,
}

/// A partial update from the settings screen.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SettingsPatch {
    pub model_choice_id: Option<String>,
    pub keep_audio: Option<bool>,
    pub microphone_id: Option<String>,
    pub onboarded: Option<bool>,
}

impl Settings {
    pub fn load() -> Self {
        let defaults = NSUserDefaults::standardUserDefaults();
        let text = |key: &str| defaults.stringForKey(&NSString::from_str(key)).map(|s| s.to_string());
        let flag = |key: &str| defaults.boolForKey(&NSString::from_str(key));
        Self {
            model_choice_id: text(key::MODEL_CHOICE).unwrap_or_else(|| ModelChoice::default_for(crate::core::oauth::ProviderId::Codex).id()),
            keep_audio: flag(key::KEEP_AUDIO),
            microphone_id: text(key::MICROPHONE).unwrap_or_default(),
            onboarded: flag(key::ONBOARDED),
        }
    }

    /// Returns the updated settings and writes the changed values through.
    pub fn apply(&self, patch: SettingsPatch) -> Self {
        let next = Self {
            model_choice_id: patch
                .model_choice_id
                .map(|id| ModelChoice::resolve(Some(&id)).id())
                .unwrap_or_else(|| self.model_choice_id.clone()),
            keep_audio: patch.keep_audio.unwrap_or(self.keep_audio),
            microphone_id: patch.microphone_id.unwrap_or_else(|| self.microphone_id.clone()),
            onboarded: patch.onboarded.unwrap_or(self.onboarded),
        };
        next.save();
        next
    }

    fn save(&self) {
        let defaults = NSUserDefaults::standardUserDefaults();
        let set_text = |key: &str, value: &str| {
            let value = NSString::from_str(value);
            // SAFETY: an NSString is a valid property-list object.
            unsafe { defaults.setObject_forKey(Some(&value), &NSString::from_str(key)) };
        };
        set_text(key::MODEL_CHOICE, &self.model_choice_id);
        set_text(key::MICROPHONE, &self.microphone_id);
        defaults.setBool_forKey(self.keep_audio, &NSString::from_str(key::KEEP_AUDIO));
        defaults.setBool_forKey(self.onboarded, &NSString::from_str(key::ONBOARDED));
    }

    pub fn model_choice(&self) -> ModelChoice {
        ModelChoice::resolve(Some(&self.model_choice_id))
    }
}
