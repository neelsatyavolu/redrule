//! Preferences in `NSUserDefaults`, under the Swift app's keys.
use objc2::AllocAnyThread;
use objc2_foundation::{NSString, NSUserDefaults};
use serde::{Deserialize, Serialize};

use crate::core::api_providers::{ApiProvider, Provider, split_choice};
use crate::providers::clients::ModelChoice;
use minutes_engine::catalog;

const LEGACY_DOMAIN: &str = "co.nenu.minutes";
/// Model choices that write notes on this Mac are stored as "local:<note model id>".
pub const LOCAL_NOTES: &str = "local:";
/// What "Copy notice" puts on the clipboard until the person writes their own.
const MAX_CONSENT_NOTICE: usize = 500;
pub const DEFAULT_CONSENT_NOTICE: &str =
    "Heads up: I'm recording this call to take notes. Let me know if you'd rather I didn't.";

mod key {
    pub const MODEL_CHOICE: &str = "modelChoice";
    pub const ASK_MODEL_CHOICE: &str = "askModelChoice";
    pub const KEEP_AUDIO: &str = "keepAudio";
    pub const SHOW_IN_DOCK: &str = "showInDock";
    pub const MICROPHONE: &str = "microphoneUID";
    pub const ONBOARDED: &str = "onboarded";
    pub const SPEECH_MODEL: &str = "speechModel";
    pub const SPEAKER_MODEL: &str = "speakerModel";
    pub const CONSENT_REMINDER: &str = "consentReminder";
    pub const CONSENT_NOTICE: &str = "consentNotice";
    pub const CRASH_REPORTS: &str = "crashReports";
    pub const USE_CALENDAR: &str = "useCalendar";
    pub const COMPATIBLE_URL: &str = "compatibleURL";
    pub const COMPATIBLE_MODEL: &str = "compatibleModel";
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Settings {
    pub model_choice_id: String,
    /// The model that answers questions about a meeting, stored like `model_choice_id`.
    /// Empty to use the notes model.
    pub ask_model_choice_id: String,
    pub keep_audio: bool,
    pub show_in_dock: bool,
    /// CoreAudio device UID, or empty for the system default.
    pub microphone_id: String,
    pub onboarded: bool,
    /// On-device model ids from `minutes_engine::catalog`.
    pub speech_model_id: String,
    pub speaker_model_id: String,
    /// Reminds the person to tell everyone on the call that it is being recorded.
    pub consent_reminder: bool,
    /// The message "Copy notice" puts on the clipboard.
    pub consent_notice: String,
    /// Sends crash reports when the build has a Sentry DSN. Off by default.
    pub crash_reports: bool,
    /// Names meetings and lists attendees from the calendar. On by default, but it does nothing
    /// until the person allows calendar access, which Redrule only asks for when they choose to.
    pub use_calendar: bool,
    /// The OpenAI-compatible server's base URL and model name; empty when none is set up.
    /// Its key, if it needs one, is in the Keychain.
    pub compatible_url: String,
    pub compatible_model: String,
}

/// A partial update from the settings screen.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SettingsPatch {
    pub model_choice_id: Option<String>,
    pub ask_model_choice_id: Option<String>,
    pub keep_audio: Option<bool>,
    pub show_in_dock: Option<bool>,
    pub microphone_id: Option<String>,
    pub onboarded: Option<bool>,
    pub speech_model_id: Option<String>,
    pub speaker_model_id: Option<String>,
    pub consent_reminder: Option<bool>,
    pub consent_notice: Option<String>,
    pub crash_reports: Option<bool>,
    pub use_calendar: Option<bool>,
    /// Set only after the server has been checked, so not from the webview.
    #[serde(skip)]
    pub compatible_url: Option<String>,
    #[serde(skip)]
    pub compatible_model: Option<String>,
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
        let compatible_model = patch.compatible_model.unwrap_or_else(|| self.compatible_model.clone());
        let follow = |id: String| following_compatible(id, &compatible_model);
        let next = Self {
            model_choice_id: follow(
                patch.model_choice_id.map(|id| known_choice(&id)).unwrap_or_else(|| self.model_choice_id.clone()),
            ),
            ask_model_choice_id: follow(
                patch
                    .ask_model_choice_id
                    .map(|id| if id.is_empty() { id } else { known_choice(&id) })
                    .unwrap_or_else(|| self.ask_model_choice_id.clone()),
            ),
            keep_audio: patch.keep_audio.unwrap_or(self.keep_audio),
            show_in_dock: patch.show_in_dock.unwrap_or(self.show_in_dock),
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
            consent_reminder: patch.consent_reminder.unwrap_or(self.consent_reminder),
            consent_notice: patch.consent_notice.map(|notice| consent_notice(&notice)).unwrap_or_else(|| self.consent_notice.clone()),
            crash_reports: patch.crash_reports.unwrap_or(self.crash_reports),
            use_calendar: patch.use_calendar.unwrap_or(self.use_calendar),
            compatible_url: patch.compatible_url.unwrap_or_else(|| self.compatible_url.clone()),
            compatible_model,
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
        set_text(key::ASK_MODEL_CHOICE, &self.ask_model_choice_id);
        set_text(key::MICROPHONE, &self.microphone_id);
        set_text(key::SPEECH_MODEL, &self.speech_model_id);
        set_text(key::SPEAKER_MODEL, &self.speaker_model_id);
        set_text(key::CONSENT_NOTICE, &self.consent_notice);
        set_text(key::COMPATIBLE_URL, &self.compatible_url);
        set_text(key::COMPATIBLE_MODEL, &self.compatible_model);
        defaults.setBool_forKey(self.consent_reminder, &NSString::from_str(key::CONSENT_REMINDER));
        defaults.setBool_forKey(self.show_in_dock, &NSString::from_str(key::SHOW_IN_DOCK));
        defaults.setBool_forKey(self.keep_audio, &NSString::from_str(key::KEEP_AUDIO));
        defaults.setBool_forKey(self.onboarded, &NSString::from_str(key::ONBOARDED));
        defaults.setBool_forKey(self.crash_reports, &NSString::from_str(key::CRASH_REPORTS));
        defaults.setBool_forKey(self.use_calendar, &NSString::from_str(key::USE_CALENDAR));
    }

    pub fn model_choice(&self) -> ModelChoice {
        ModelChoice::resolve(Some(&self.model_choice_id))
    }

    /// The on-device note model, when notes are written on this Mac instead of with an account.
    pub fn local_note_model(&self) -> Option<&'static catalog::NoteOption> {
        self.model_choice_id.strip_prefix(LOCAL_NOTES).map(catalog::note_option)
    }

    /// The choice that answers questions: its own, or the notes model's when it has none.
    fn ask_choice_id(&self) -> &str {
        if self.ask_model_choice_id.is_empty() { &self.model_choice_id } else { &self.ask_model_choice_id }
    }

    pub fn ask_model_choice(&self) -> ModelChoice {
        ModelChoice::resolve(Some(self.ask_choice_id()))
    }

    /// The on-device model that answers questions, when that runs on this Mac.
    pub fn local_ask_model(&self) -> Option<&'static catalog::NoteOption> {
        self.ask_choice_id().strip_prefix(LOCAL_NOTES).map(catalog::note_option)
    }

    /// The model to switch to for `provider`: its first listed model, or the custom server's model.
    pub fn default_choice(&self, provider: Provider) -> Option<ModelChoice> {
        match provider {
            Provider::Api(ApiProvider::Compatible) if !self.compatible_model.is_empty() => {
                Some(ModelChoice::custom(ApiProvider::Compatible, &self.compatible_model))
            }
            _ => ModelChoice::default_for(provider),
        }
    }
}

/// A blank notice goes back to the default wording.
fn consent_notice(text: &str) -> String {
    let text: String = text.trim().chars().take(MAX_CONSENT_NOTICE).collect();
    if text.is_empty() { DEFAULT_CONSENT_NOTICE.to_string() } else { text }
}

/// A choice that is still offered: retired account models and note models fall back to a default.
fn known_choice(id: &str) -> String {
    match id.strip_prefix(LOCAL_NOTES) {
        Some(local) => format!("{LOCAL_NOTES}{}", catalog::note_option(local).id),
        None => ModelChoice::resolve(Some(id)).id(),
    }
}

/// Choices of the custom server follow its model name when that changes.
fn following_compatible(id: String, model: &str) -> String {
    match split_choice(&id) {
        (Some(Provider::Api(ApiProvider::Compatible)), _) if !model.is_empty() => {
            format!("{}:{model}", ApiProvider::Compatible.raw())
        }
        _ => id,
    }
}

fn has_settings(defaults: &NSUserDefaults) -> bool {
    [key::ONBOARDED, key::MODEL_CHOICE].iter().any(|key| defaults.objectForKey(&NSString::from_str(key)).is_some())
}

fn read(defaults: &NSUserDefaults) -> Settings {
    let text = |key: &str| defaults.stringForKey(&NSString::from_str(key)).map(|s| s.to_string());
    let flag = |key: &str| defaults.boolForKey(&NSString::from_str(key));
    let unset = |key: &str| defaults.objectForKey(&NSString::from_str(key)).is_none();
    // New users start on the models that suit their Mac. Earlier users keep the speech model they
    // already downloaded and move to the default speaker model, a small download that fixes split voices.
    let (speech, speaker) = if flag(key::ONBOARDED) {
        (catalog::DEFAULT_SPEECH, catalog::DEFAULT_SPEAKER)
    } else {
        super::speech_models::recommended()
    };
    Settings {
        model_choice_id: text(key::MODEL_CHOICE)
            .unwrap_or_else(|| ModelChoice::resolve(None).id()),
        ask_model_choice_id: text(key::ASK_MODEL_CHOICE).map(|id| if id.is_empty() { id } else { known_choice(&id) }).unwrap_or_default(),
        keep_audio: flag(key::KEEP_AUDIO),
        show_in_dock: defaults.objectForKey(&NSString::from_str(key::SHOW_IN_DOCK)).is_none() || flag(key::SHOW_IN_DOCK),
        microphone_id: text(key::MICROPHONE).unwrap_or_default(),
        onboarded: flag(key::ONBOARDED),
        speech_model_id: catalog::speech_option(&text(key::SPEECH_MODEL).unwrap_or_else(|| speech.into())).id.into(),
        speaker_model_id: catalog::speaker_option(&text(key::SPEAKER_MODEL).unwrap_or_else(|| speaker.into())).id.into(),
        consent_reminder: defaults.objectForKey(&NSString::from_str(key::CONSENT_REMINDER)).is_none() || flag(key::CONSENT_REMINDER),
        consent_notice: consent_notice(&text(key::CONSENT_NOTICE).unwrap_or_default()),
        crash_reports: flag(key::CRASH_REPORTS),
        use_calendar: unset(key::USE_CALENDAR) || flag(key::USE_CALENDAR),
        compatible_url: text(key::COMPATIBLE_URL).unwrap_or_default(),
        compatible_model: text(key::COMPATIBLE_MODEL).unwrap_or_default(),
    }
}

/// Whether crash reports are on, read without loading or migrating the other settings.
pub fn crash_reports_on() -> bool {
    NSUserDefaults::standardUserDefaults().boolForKey(&NSString::from_str(key::CRASH_REPORTS))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn local_note_choices_survive_and_fall_back_to_the_default_note_model() {
        assert_eq!(known_choice("local:qwen3.5-9b"), "local:qwen3.5-9b");
        assert_eq!(known_choice("local:retired"), format!("local:{}", catalog::DEFAULT_NOTES));
        assert_eq!(known_choice("grok:grok-4.6"), "grok:grok-4.6");
        assert_eq!(known_choice("codex:retired"), ModelChoice::resolve(None).id());
        assert_eq!(known_choice("anthropic:claude-opus-5"), "anthropic:claude-opus-5");
        assert_eq!(known_choice("openai:my-fine-tune"), "openai:my-fine-tune");
    }

    #[test]
    fn custom_server_choices_follow_its_model_name() {
        assert_eq!(following_compatible("compatible:llama3.2".into(), "qwen3"), "compatible:qwen3");
        assert_eq!(following_compatible("compatible:llama3.2".into(), ""), "compatible:llama3.2");
        assert_eq!(following_compatible("codex:gpt-6-astra".into(), "qwen3"), "codex:gpt-6-astra");
        assert_eq!(following_compatible(String::new(), "qwen3"), "");
    }

    #[test]
    fn the_custom_server_supplies_its_own_default_model() {
        let mut custom = settings("codex:gpt-6-astra", "");
        assert_eq!(custom.default_choice(Provider::Api(ApiProvider::Compatible)), None);
        custom.compatible_model = "llama3.2".into();
        assert_eq!(custom.default_choice(Provider::Api(ApiProvider::Compatible)).map(|c| c.id()).as_deref(), Some("compatible:llama3.2"));
        assert_eq!(
            custom.default_choice(Provider::Api(ApiProvider::Gemini)).map(|c| c.model).as_deref(),
            Some("gemini-3.8-flash")
        );
    }

    fn settings(notes: &str, ask: &str) -> Settings {
        Settings {
            model_choice_id: notes.into(),
            ask_model_choice_id: ask.into(),
            keep_audio: false,
            show_in_dock: true,
            microphone_id: String::new(),
            onboarded: true,
            speech_model_id: catalog::DEFAULT_SPEECH.into(),
            speaker_model_id: catalog::DEFAULT_SPEAKER.into(),
            consent_reminder: true,
            consent_notice: DEFAULT_CONSENT_NOTICE.into(),
            crash_reports: false,
            use_calendar: true,
            compatible_url: String::new(),
            compatible_model: String::new(),
        }
    }

    #[test]
    fn a_blank_consent_notice_returns_to_the_default() {
        assert_eq!(consent_notice("  "), DEFAULT_CONSENT_NOTICE);
        assert_eq!(consent_notice(" Recording for notes. "), "Recording for notes.");
        assert_eq!(consent_notice(&"x".repeat(600)).len(), MAX_CONSENT_NOTICE);
    }

    #[test]
    fn questions_use_their_own_model_or_follow_the_notes_model() {
        let follows = settings("local:qwen3.5-9b", "");
        assert_eq!(follows.local_ask_model().map(|option| option.id), Some("qwen3.5-9b"));
        let separate = settings("local:qwen3.5-9b", "grok:grok-4.6");
        assert_eq!(separate.local_ask_model().map(|option| option.id), None);
        assert_eq!(separate.ask_model_choice().model, "grok-4.6");
        assert_eq!(separate.local_note_model().map(|option| option.id), Some("qwen3.5-9b"));
    }
}
