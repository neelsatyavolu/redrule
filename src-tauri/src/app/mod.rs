//! App state and behavior, ported from the Swift `AppModel`.
mod accounts;
mod ask;
mod background;
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
pub use settings::{Settings, SettingsPatch};
pub use speech_models::{LocalModels, ModelKind};
pub use state::{App, State};
