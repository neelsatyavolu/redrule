//! Choosing the calendar event a recording belongs to, from plain data read out of the calendar.
use chrono::{DateTime, TimeDelta, Utc};

use super::models::Meeting;

/// How far before and after the start of a recording to look for its event.
pub const WINDOW: TimeDelta = TimeDelta::minutes(10);
/// A call that runs a little over still matches its event; one started after the event ended does not.
pub const ENDED_GRACE: TimeDelta = TimeDelta::minutes(2);
/// Attendees kept on a meeting; larger invites are mostly lists nobody reads.
pub const MAX_ATTENDEES: usize = 20;
const CALL_HOSTS: [&str; 4] = ["zoom.us", "meet.google.com", "teams.microsoft.com", "webex.com"];

#[derive(Debug, Clone, PartialEq)]
pub struct CalendarEvent {
    pub title: String,
    pub start: DateTime<Utc>,
    pub end: DateTime<Utc>,
    pub all_day: bool,
    /// The event's URL, location and notes, where a video-call link would be.
    pub details: String,
    pub attendees: Vec<Attendee>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Attendee {
    pub name: Option<String>,
    /// Usually `mailto:` and the address.
    pub url: String,
    /// The person using this Mac.
    pub current_user: bool,
    pub declined: bool,
}

impl CalendarEvent {
    fn in_progress(&self, now: DateTime<Utc>) -> bool {
        self.start <= now && now < self.end
    }

    fn has_call_link(&self) -> bool {
        let details = self.details.to_lowercase();
        CALL_HOSTS.iter().any(|host| details.contains(host))
    }

    fn declined(&self) -> bool {
        self.attendees.iter().any(|a| a.current_user && a.declined)
    }

    /// Everyone invited but the person using this Mac, once each, in invite order.
    pub fn attendee_names(&self) -> Vec<String> {
        let mut names: Vec<String> = Vec::new();
        for name in self.attendees.iter().filter(|a| !a.current_user).filter_map(Attendee::display_name) {
            if names.len() < MAX_ATTENDEES && !names.iter().any(|n| n.to_lowercase() == name.to_lowercase()) {
                names.push(name);
            }
        }
        names
    }
}

impl Attendee {
    /// The name, or the part of the address before the "@".
    fn display_name(&self) -> Option<String> {
        let name = self.name.as_deref().map(str::trim).filter(|n| !n.is_empty());
        if let Some(name) = name.filter(|n| !n.contains('@')) {
            return Some(name.to_string());
        }
        let address = name.or_else(|| mailto(&self.url))?;
        address.split('@').next().map(str::trim).filter(|local| !local.is_empty()).map(str::to_string)
    }
}

fn mailto(url: &str) -> Option<&str> {
    let (scheme, address) = url.split_once(':')?;
    scheme.eq_ignore_ascii_case("mailto").then_some(address)
}

/// The event happening at `now`: ones in progress first, then ones with a video-call link,
/// then the one starting closest to now. Skips all-day and declined events.
pub fn current_event(events: &[CalendarEvent], now: DateTime<Utc>) -> Option<&CalendarEvent> {
    events
        .iter()
        .filter(|e| !e.all_day && !e.declined() && e.start <= now + WINDOW && e.end > now - ENDED_GRACE)
        .min_by_key(|e| (!e.in_progress(now), !e.has_call_link(), (e.start - now).abs()))
}

impl Meeting {
    /// Takes the event's attendees, and its title unless the meeting was already named.
    pub fn with_event(&self, event: &CalendarEvent) -> Self {
        let title = event.title.trim();
        let named = if self.has_default_title() && !title.is_empty() { self.titled(title) } else { self.clone() };
        named.with_attendees(event.attendee_names())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{MeetingApp, MeetingStatus};

    fn now() -> DateTime<Utc> {
        DateTime::parse_from_rfc3339("2026-09-23T15:02:00Z").unwrap().with_timezone(&Utc)
    }

    /// An event starting `from` minutes after now and ending `to` minutes after now.
    fn event(title: &str, from: i64, to: i64) -> CalendarEvent {
        CalendarEvent {
            title: title.into(),
            start: now() + TimeDelta::minutes(from),
            end: now() + TimeDelta::minutes(to),
            all_day: false,
            details: String::new(),
            attendees: vec![],
        }
    }

    fn attendee(name: Option<&str>, url: &str) -> Attendee {
        Attendee { name: name.map(Into::into), url: url.into(), current_user: false, declined: false }
    }

    fn picked(events: &[CalendarEvent]) -> Option<&str> {
        current_event(events, now()).map(|e| e.title.as_str())
    }

    #[test]
    fn prefers_events_in_progress_over_ones_about_to_start() {
        assert_eq!(picked(&[event("Next", 5, 35), event("Now", -2, 28)]), Some("Now"));
        assert_eq!(picked(&[event("Just ended", -40, -5), event("Next", 5, 35)]), Some("Next"));
        assert_eq!(picked(&[event("Ended five minutes ago", -40, -5)]), None);
        assert_eq!(picked(&[event("Running over", -31, -1)]), Some("Running over"));
    }

    #[test]
    fn then_prefers_video_calls_then_the_nearest_start() {
        let call = CalendarEvent { details: "https://acme.ZOOM.us/j/123".into(), ..event("Call", -30, 30) };
        assert_eq!(picked(&[event("Block", -1, 60), call]), Some("Call"));
        assert_eq!(picked(&[event("Earlier", -30, 30), event("Closer", -1, 30)]), Some("Closer"));
        assert_eq!(picked(&[event("Later", 8, 30), event("Sooner", 3, 30)]), Some("Sooner"));
    }

    #[test]
    fn skips_all_day_declined_and_distant_events() {
        let all_day = CalendarEvent { all_day: true, ..event("Holiday", -900, 540) };
        let me = Attendee { current_user: true, declined: true, ..attendee(Some("Me"), "mailto:me@x.com") };
        let declined = CalendarEvent { attendees: vec![me], ..event("Declined", -5, 25) };
        assert_eq!(picked(&[all_day, declined, event("Tomorrow", 1440, 1500), event("Earlier", -90, -11)]), None);
        assert_eq!(picked(&[]), None);
    }

    #[test]
    fn names_attendees_by_name_or_address_without_the_current_user() {
        let me = Attendee { current_user: true, ..attendee(Some("Neel"), "mailto:neel@x.com") };
        let attendees = vec![
            me,
            attendee(Some(" Ada Lovelace "), "mailto:ada@x.com"),
            attendee(None, "MAILTO:grace.hopper@x.com"),
            attendee(Some("dana@x.com"), "mailto:dana@x.com"),
            attendee(Some("ada lovelace"), "mailto:ada2@x.com"),
            attendee(None, "urn:uuid:1234"),
        ];
        let names = CalendarEvent { attendees, ..event("Sync", 0, 30) }.attendee_names();
        assert_eq!(names, vec!["Ada Lovelace", "grace.hopper", "dana"]);
    }

    #[test]
    fn caps_the_attendee_list() {
        let attendees = (0..30).map(|n| attendee(Some(&format!("Person {n}")), "")).collect();
        assert_eq!(CalendarEvent { attendees, ..event("All hands", 0, 60) }.attendee_names().len(), MAX_ATTENDEES);
    }

    #[test]
    fn a_calendar_title_replaces_only_the_default_one() {
        let meeting = Meeting {
            id: "A".into(),
            title: MeetingApp::Zoom.default_title(),
            app: MeetingApp::Zoom,
            started_at: now(),
            ended_at: None,
            status: MeetingStatus::Recording,
            error_message: None,
            archived_at: None,
            tags: vec![],
            folder_id: None,
            attendees: vec![],
        };
        let event = CalendarEvent { attendees: vec![attendee(Some("Ada"), "")], ..event(" Acme kickoff ", 0, 30) };
        let named = meeting.with_event(&event);
        assert_eq!(named.title, "Acme kickoff");
        assert_eq!(named.attendees, vec!["Ada"]);
        assert_eq!(meeting.titled("Typed").with_event(&event).title, "Typed");
        assert_eq!(meeting.with_event(&CalendarEvent { title: " ".into(), ..event.clone() }).title, "Zoom meeting");
    }
}
