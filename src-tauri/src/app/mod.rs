//! App state and behavior, ported from the Swift `AppModel`.
mod accounts;
mod background;
mod library;
mod recording;
mod settings;
mod sharing;
mod state;

pub use library::MeetingDetail;
pub use settings::{Settings, SettingsPatch};
pub use state::{App, State};
