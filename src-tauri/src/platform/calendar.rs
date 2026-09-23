//! Calendar access and the events around a moment, read through EventKit as plain data.
use std::sync::atomic::{AtomicBool, Ordering};

use block2::RcBlock;
use chrono::{DateTime, Utc};
use objc2::rc::Retained;
use objc2::runtime::Bool;
use objc2_event_kit::{EKAuthorizationStatus, EKCalendar, EKEntityType, EKEvent, EKEventStore, EKParticipant, EKParticipantStatus};
use objc2_foundation::{NSArray, NSDate, NSError};
use serde::Serialize;

use crate::core::calendar::{Attendee, CalendarEvent, WINDOW};

/// Set when the person allows access from the prompt. Until the app is relaunched, EventKit can
/// keep reporting "not determined" to it after access was allowed.
static ALLOWED_THIS_RUN: AtomicBool = AtomicBool::new(false);

/// A calendar that holds events, offered in settings so the person can leave some out.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Calendar {
    pub id: String,
    pub title: String,
    /// The account it syncs with, such as iCloud or a work address.
    pub account: String,
}

fn status() -> EKAuthorizationStatus {
    // SAFETY: a class method that only reads the app's privacy decision.
    unsafe { EKEventStore::authorizationStatusForEntityType(EKEntityType::Event) }
}

pub fn granted() -> bool {
    ALLOWED_THIS_RUN.load(Ordering::Relaxed) || status() == EKAuthorizationStatus::FullAccess
}

/// Asks for full access to events, or opens System Settings if it was already decided.
/// Returns whether macOS showed its prompt. Blocks until the person answers, so call it off the main thread.
pub fn request() -> bool {
    if granted() {
        return false;
    }
    if status() != EKAuthorizationStatus::NotDetermined {
        super::permissions::open_settings("Privacy_Calendars");
        return false;
    }
    let (sender, receiver) = std::sync::mpsc::channel::<()>();
    let handler = RcBlock::new(move |allowed: Bool, _error: *mut NSError| {
        if allowed.as_bool() {
            ALLOWED_THIS_RUN.store(true, Ordering::Relaxed);
        }
        let _ = sender.send(());
    });
    // SAFETY: plain allocation; the store stays alive on this thread until the handler has run.
    let store = unsafe { EKEventStore::new() };
    // SAFETY: EventKit copies the handler and calls it once, on an arbitrary queue.
    unsafe { store.requestFullAccessToEventsWithCompletion(RcBlock::as_ptr(&handler)) };
    let _ = receiver.recv();
    true
}

/// Every calendar that holds events, by account and then title. Empty without access.
pub fn calendars() -> Vec<Calendar> {
    if !granted() {
        return Vec::new();
    }
    // SAFETY: EventKit calls on a store and calendars owned by this thread; only plain data leaves it.
    let found: Vec<Calendar> = unsafe {
        let store = EKEventStore::new();
        store
            .calendarsForEntityType(EKEntityType::Event)
            .iter()
            .map(|calendar| Calendar {
                id: calendar.calendarIdentifier().to_string(),
                title: calendar.title().to_string(),
                account: calendar.source().map(|source| source.title().to_string()).unwrap_or_default(),
            })
            .collect()
    };
    let order = |c: &Calendar| (c.account.to_lowercase(), c.title.to_lowercase());
    let mut sorted = found;
    sorted.sort_by_key(order);
    sorted
}

/// Events overlapping `WINDOW` either side of `now`, from every calendar not in `ignored`.
/// Empty without access. Blocks briefly.
pub fn events_around(now: DateTime<Utc>, ignored: &[String]) -> Vec<CalendarEvent> {
    if !granted() {
        return Vec::new();
    }
    // SAFETY: EventKit calls on a store and events owned by this thread; nothing escapes it but plain data.
    unsafe {
        let store = EKEventStore::new();
        // No list means every calendar, so leaving them all out has to stop here.
        let calendars = if ignored.is_empty() {
            None
        } else {
            let chosen: Vec<Retained<EKCalendar>> = store
                .calendarsForEntityType(EKEntityType::Event)
                .iter()
                .filter(|calendar| !ignored.contains(&calendar.calendarIdentifier().to_string()))
                .collect();
            if chosen.is_empty() {
                return Vec::new();
            }
            Some(NSArray::from_retained_slice(&chosen))
        };
        let predicate =
            store.predicateForEventsWithStartDate_endDate_calendars(&date(now - WINDOW), &date(now + WINDOW), calendars.as_deref());
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
        println!("calendars: {:?}", super::calendars());
        let events = super::events_around(now, &[]);
        for event in &events {
            println!("{} {} - {} all day: {} attendees: {:?}", event.title, event.start, event.end, event.all_day, event.attendee_names());
        }
        println!("picked: {:?}", crate::core::calendar::current_event(&events, now).map(|e| &e.title));
    }
}
