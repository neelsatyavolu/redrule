//! Calendar access and the events around a moment, read through EventKit as plain data.
use block2::RcBlock;
use chrono::{DateTime, Utc};
use objc2::rc::Retained;
use objc2::runtime::Bool;
use objc2_event_kit::{EKAuthorizationStatus, EKEntityType, EKEvent, EKEventStore, EKParticipant, EKParticipantStatus};
use objc2_foundation::{NSDate, NSError};

use crate::core::calendar::{Attendee, CalendarEvent, WINDOW};

fn status() -> EKAuthorizationStatus {
    // SAFETY: a class method that only reads the app's privacy decision.
    unsafe { EKEventStore::authorizationStatusForEntityType(EKEntityType::Event) }
}

pub fn granted() -> bool {
    status() == EKAuthorizationStatus::FullAccess
}

/// Asks for full access to events, or opens System Settings if it was already decided.
/// Blocks until the person answers, so call it off the main thread.
pub fn request() {
    if granted() {
        return;
    }
    if status() != EKAuthorizationStatus::NotDetermined {
        super::permissions::open_settings("Privacy_Calendars");
        return;
    }
    let (sender, receiver) = std::sync::mpsc::channel::<()>();
    let handler = RcBlock::new(move |_granted: Bool, _error: *mut NSError| {
        let _ = sender.send(());
    });
    // SAFETY: plain allocation; the store stays alive on this thread until the handler has run.
    let store = unsafe { EKEventStore::new() };
    // SAFETY: EventKit copies the handler and calls it once, on an arbitrary queue.
    unsafe { store.requestFullAccessToEventsWithCompletion(RcBlock::as_ptr(&handler)) };
    let _ = receiver.recv();
}

/// Events overlapping `WINDOW` either side of `now`. Empty without access. Blocks briefly.
pub fn events_around(now: DateTime<Utc>) -> Vec<CalendarEvent> {
    if !granted() {
        return Vec::new();
    }
    // SAFETY: EventKit calls on a store and events owned by this thread; nothing escapes it but plain data.
    unsafe {
        let store = EKEventStore::new();
        let predicate = store.predicateForEventsWithStartDate_endDate_calendars(&date(now - WINDOW), &date(now + WINDOW), None);
        store.eventsMatchingPredicate(&predicate).iter().map(|event| read(&event)).collect()
    }
}

fn date(at: DateTime<Utc>) -> Retained<NSDate> {
    NSDate::dateWithTimeIntervalSince1970(at.timestamp_millis() as f64 / 1000.0)
}

fn time(date: &NSDate) -> DateTime<Utc> {
    DateTime::from_timestamp_millis((date.timeIntervalSince1970() * 1000.0) as i64).unwrap_or_default()
}

/// SAFETY: `event` must be a live event from an `EKEventStore`.
unsafe fn read(event: &EKEvent) -> CalendarEvent {
    unsafe {
        let url = event.URL().and_then(|url| url.absoluteString()).map(|s| s.to_string());
        let details = [url, event.location().map(|s| s.to_string()), event.notes().map(|s| s.to_string())];
        CalendarEvent {
            title: event.title().to_string(),
            start: time(&event.startDate()),
            end: time(&event.endDate()),
            all_day: event.isAllDay(),
            details: details.into_iter().flatten().collect::<Vec<_>>().join("\n"),
            attendees: event.attendees().map(|list| list.iter().map(|p| attendee(&p)).collect()).unwrap_or_default(),
        }
    }
}

/// SAFETY: `participant` must be a live attendee of an event.
unsafe fn attendee(participant: &EKParticipant) -> Attendee {
    unsafe {
        Attendee {
            name: participant.name().map(|name| name.to_string()),
            url: participant.URL().absoluteString().map(|s| s.to_string()).unwrap_or_default(),
            current_user: participant.isCurrentUser(),
            declined: participant.participantStatus() == EKParticipantStatus::Declined,
        }
    }
}

#[cfg(test)]
mod tests {
    /// Reads the live calendar; run with `cargo test -p minutes calendar -- --ignored --nocapture`.
    /// Test binaries have no usage description, so macOS may refuse access; the app is the real check.
    #[test]
    #[ignore]
    fn prints_the_current_events() {
        let now = chrono::Utc::now();
        println!("access: {}", super::granted());
        let events = super::events_around(now);
        for event in &events {
            println!("{} {} - {} all day: {} attendees: {:?}", event.title, event.start, event.end, event.all_day, event.attendee_names());
        }
        println!("picked: {:?}", crate::core::calendar::current_event(&events, now).map(|e| &e.title));
    }
}
