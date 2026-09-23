//! Naming a recording after the calendar event happening when it starts.
use std::time::Duration;

use chrono::Utc;
use tauri::async_runtime::JoinHandle;

use super::state::App;
use crate::core::calendar::{CalendarEvent, current_event};
use crate::core::models::Meeting;
use crate::platform::calendar;

/// The longest a new recording waits for its event after capture has started.
const LOOKUP_LIMIT: Duration = Duration::from_secs(2);

pub type EventLookup = JoinHandle<Option<CalendarEvent>>;

impl App {
    /// Starts looking up the current event off the main thread, when the person allowed it.
    pub(super) fn look_up_event(&self) -> Option<EventLookup> {
        if !self.read(|state| state.settings.use_calendar) || !calendar::granted() {
            return None;
        }
        Some(tauri::async_runtime::spawn_blocking(|| {
            let now = Utc::now();
            current_event(&calendar::events_around(now), now).cloned()
        }))
    }

    /// The meeting with its event's title and attendees, saved; unchanged if there is none.
    /// Failures are logged only: the calendar never gets in the way of recording.
    pub(super) async fn name_after_event(&self, meeting: Meeting, lookup: Option<EventLookup>) -> Meeting {
        let Some(lookup) = lookup else { return meeting };
        let event = match tokio::time::timeout(LOOKUP_LIMIT, lookup).await {
            Ok(Ok(event)) => event,
            Ok(Err(error)) => {
                log::warn!("The calendar could not be read. {error}");
                None
            }
            Err(_) => {
                log::warn!("The calendar took too long to answer.");
                None
            }
        };
        let Some(event) = event else { return meeting };
        let named = meeting.with_event(&event);
        self.persist(&named);
        named
    }
}
