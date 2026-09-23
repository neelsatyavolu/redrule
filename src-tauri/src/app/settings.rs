//! Preferences in `NSUserDefaults`, under the Swift app's keys.
use objc2::AllocAnyThread;
use objc2_foundation::{NSString, NSUserDefaults};
use serde::{Deserialize, Serialize};

use crate::providers::clients::ModelChoice;
use minutes_engine::catalog;

const LEGACY_DOMAIN: &str = "co.nenu.minutes";
/// Model choices that write notes on this Mac are stored as "local:<note model id>".
pub const LOCAL_NOTES: &str = "local:";

mod key {
    pub const MODEL_CHOICE: &str = "modelChoice";
    pub const KEEP_AUDIO: &str = "keepAudio";
    pub const MICROPHONE: &str = "microphoneUID";
    pub const ONBOARDED: &str = "onboarded";
    pub const SPEECH_MODEL: &str = "speechModel";
    pub const SPEAKER_MODEL: &str = "speakerModel";
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Settings {
    pub model_choice_id: String,
    pub keep_audio: bool,
    /// CoreAudio device UID, or empty for the system default.
    pub microphone_id: String,
    pub onboarded: bool,
    /// On-device model ids from `minutes_engine::catalog`.
    pub speech_model_id: String,
    pub speaker_model_id: String,
}

/// A partial update from the settings screen.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SettingsPatch {
    pub model_choice_id: Option<String>,
    pub keep_audio: Option<bool>,
    pub microphone_id: Option<String>,
    pub onboarded: Option<bool>,
    pub speech_model_id: Option<String>,
    pub speaker_model_id: Option<String>,
}

impl Settings {
    pub fn load() -> Self {
        let defaults = NSUserDefaults::standardUserDefaults();
        if has_settings(&defaults) {
            return read(&defaults);
        }
        // Preferences from the previous app (co.nenu.minutes) are copied over once.
        let legacy = NSUserDefaults::initWithSuiteName(NSUserDefaults::alloc(), Some(&NSString::from_str(LEGACY_DOMAIN)));
        match legacy.filter(|legacy| has_settings(legacy)) {
            Some(legacy) => {
                let settings = read(&legacy);
                settings.save();
                settings
            }
            None => read(&defaults),
        }
    }

    /// Returns the updated settings and writes the changed values through.
    pub fn apply(&self, patch: SettingsPatch) -> Self {
        let next = Self {
            model_choice_id: patch
                .model_choice_id
                .map(|id| known_choice(&id))
                .unwrap_or_else(|| self.model_choice_id.clone()),
            keep_audio: patch.keep_audio.unwrap_or(self.keep_audio),
            microphone_id: patch.microphone_id.unwrap_or_else(|| self.microphone_id.clone()),
            onboarded: patch.onboarded.unwrap_or(self.onboarded),
            speech_model_id: patch
                .speech_model_id
                .map(|id| catalog::speech_option(&id).id.to_string())
                .unwrap_or_else(|| self.speech_model_id.clone()),
            speaker_model_id: patch
                .speaker_model_id
                .map(|id| catalog::speaker_option(&id).id.to_string())
                .unwrap_or_else(|| self.speaker_model_id.clone()),
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
        set_text(key::SPEECH_MODEL, &self.speech_model_id);
        set_text(key::SPEAKER_MODEL, &self.speaker_model_id);
        defaults.setBool_forKey(self.keep_audio, &NSString::from_str(key::KEEP_AUDIO));
        defaults.setBool_forKey(self.onboarded, &NSString::from_str(key::ONBOARDED));
    }

    pub fn model_choice(&self) -> ModelChoice {
        ModelChoice::resolve(Some(&self.model_choice_id))
    }

    /// The on-device note model, when notes are written on this Mac instead of with an account.
    pub fn local_note_model(&self) -> Option<&'static catalog::NoteOption> {
        self.model_choice_id.strip_prefix(LOCAL_NOTES).map(catalog::note_option)
    }
}

/// A choice that is still offered: retired account models and note models fall back to a default.
fn known_choice(id: &str) -> String {
    match id.strip_prefix(LOCAL_NOTES) {
        Some(local) => format!("{LOCAL_NOTES}{}", catalog::note_option(local).id),
        None => ModelChoice::resolve(Some(id)).id(),
    }
}

fn has_settings(defaults: &NSUserDefaults) -> bool {
    [key::ONBOARDED, key::MODEL_CHOICE].iter().any(|key| defaults.objectForKey(&NSString::from_str(key)).is_some())
}

fn read(defaults: &NSUserDefaults) -> Settings {
    let text = |key: &str| defaults.stringForKey(&NSString::from_str(key)).map(|s| s.to_string());
    let flag = |key: &str| defaults.boolForKey(&NSString::from_str(key));
    // New users start on the models that suit their Mac. Earlier users keep the speech model they
    // already downloaded and move to the default speaker model, a small download that fixes split voices.
    let (speech, speaker) = if flag(key::ONBOARDED) {
        (catalog::DEFAULT_SPEECH, catalog::DEFAULT_SPEAKER)
    } else {
        super::speech_models::recommended()
    };
    Settings {
        model_choice_id: text(key::MODEL_CHOICE)
            .unwrap_or_else(|| ModelChoice::default_for(crate::core::oauth::ProviderId::Codex).id()),
        keep_audio: flag(key::KEEP_AUDIO),
        microphone_id: text(key::MICROPHONE).unwrap_or_default(),
        onboarded: flag(key::ONBOARDED),
        speech_model_id: catalog::speech_option(&text(key::SPEECH_MODEL).unwrap_or_else(|| speech.into())).id.into(),
        speaker_model_id: catalog::speaker_option(&text(key::SPEAKER_MODEL).unwrap_or_else(|| speaker.into())).id.into(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn local_note_choices_survive_and_fall_back_to_the_default_note_model() {
        assert_eq!(known_choice("local:qwen3.5-9b"), "local:qwen3.5-9b");
        assert_eq!(known_choice("local:retired"), format!("local:{}", catalog::DEFAULT_NOTES));
        assert_eq!(known_choice("grok:grok-4.6"), "grok:grok-4.6");
    }
}
