//! App state and behavior, ported from the Swift `AppModel`.
mod accounts;
mod api_keys;
mod ask;
mod background;
mod calendar;
mod folder_sync;
mod folders;
mod library;
mod recording;
mod search;
mod settings;
mod sharing;
mod speech_models;
mod state;

pub use library::MeetingDetail;
pub use settings::{crash_reports_on, Settings, SettingsPatch};
pub use speech_models::{LocalModels, ModelKind};
pub use state::{App, State};
