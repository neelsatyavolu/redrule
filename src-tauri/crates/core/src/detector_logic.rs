//! Debounced meeting detection. Pure: `ingest` returns the next state instead of mutating.
use std::collections::HashSet;

use super::models::MeetingApp;

pub const BROWSER_BUNDLE_PREFIXES: &[&str] = &[
    "com.google.Chrome",
    "com.apple.Safari",
    "com.apple.WebKit",
    "company.thebrowser",
    "com.microsoft.edgemac",
    "com.brave.Browser",
    "org.mozilla.firefox",
    "com.vivaldi.Vivaldi",
];
/// Zoom only runs this helper while a meeting is in progress.
const ZOOM_MEETING_PROCESS: &str = "CptHost";
const POSITIVES_TO_DETECT: u32 = 2;
const NEGATIVES_TO_END: u32 = 3;

#[derive(Debug, Clone, Default, PartialEq)]
pub struct DetectionSnapshot {
    pub process_names: HashSet<String>,
    /// Bundle identifiers of other processes currently reading from an audio input.
    pub mic_user_bundle_ids: HashSet<String>,
    /// Titles of windows owned by web browsers.
    pub browser_window_titles: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DetectionEvent {
    Detected(MeetingApp),
    Ended,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct DetectorLogic {
    pub active: Option<MeetingApp>,
    candidate: Option<MeetingApp>,
    streak: u32,
}

impl DetectorLogic {
    pub fn ingest(&self, snapshot: &DetectionSnapshot) -> (Self, Option<DetectionEvent>) {
        if let Some(active) = self.active {
            if is_still_running(active, snapshot) {
                return (Self { active: Some(active), candidate: None, streak: 0 }, None);
            }
            let misses = self.streak + 1;
            if misses >= NEGATIVES_TO_END {
                return (Self::default(), Some(DetectionEvent::Ended));
            }
            return (Self { active: Some(active), candidate: None, streak: misses }, None);
        }

        let Some(app) = classify(snapshot) else { return (Self::default(), None) };
        let hits = if self.candidate == Some(app) { self.streak + 1 } else { 1 };
        if hits >= POSITIVES_TO_DETECT {
            return (Self { active: Some(app), candidate: None, streak: 0 }, Some(DetectionEvent::Detected(app)));
        }
        (Self { active: None, candidate: Some(app), streak: hits }, None)
    }
}

fn classify(snapshot: &DetectionSnapshot) -> Option<MeetingApp> {
    if snapshot.process_names.contains(ZOOM_MEETING_PROCESS) {
        return Some(MeetingApp::Zoom);
    }
    if browser_holds_mic(snapshot) && snapshot.browser_window_titles.iter().any(|t| is_meet_call_title(t)) {
        return Some(MeetingApp::GoogleMeet);
    }
    None
}

/// Once a Meet call is active, the browser holding the mic is enough: the tab may be in the background.
fn is_still_running(app: MeetingApp, snapshot: &DetectionSnapshot) -> bool {
    match app {
        MeetingApp::Zoom => snapshot.process_names.contains(ZOOM_MEETING_PROCESS),
        MeetingApp::GoogleMeet => {
            browser_holds_mic(snapshot) || snapshot.browser_window_titles.iter().any(|t| is_meet_call_title(t))
        }
        MeetingApp::Manual => false,
    }
}

fn browser_holds_mic(snapshot: &DetectionSnapshot) -> bool {
    snapshot
        .mic_user_bundle_ids
        .iter()
        .any(|id| BROWSER_BUNDLE_PREFIXES.iter().any(|prefix| id.starts_with(prefix)))
}

/// In-call tabs are titled "Meet - <code or name>"; the landing page is just "Google Meet".
fn is_meet_call_title(title: &str) -> bool {
    title.starts_with("Meet - ") || title.starts_with("Meet – ") || title.starts_with("Meet: ")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn zoom() -> DetectionSnapshot {
        DetectionSnapshot { process_names: ["CptHost".to_string()].into(), ..Default::default() }
    }

    fn meet(title: &str) -> DetectionSnapshot {
        DetectionSnapshot {
            mic_user_bundle_ids: ["com.google.Chrome.helper".to_string()].into(),
            browser_window_titles: vec![title.to_string()],
            ..Default::default()
        }
    }

    fn run(snapshots: &[DetectionSnapshot]) -> Vec<Option<DetectionEvent>> {
        let mut logic = DetectorLogic::default();
        snapshots
            .iter()
            .map(|s| {
                let (next, event) = logic.ingest(s);
                logic = next;
                event
            })
            .collect()
    }

    #[test]
    fn detects_zoom_after_two_positive_polls() {
        assert_eq!(run(&[zoom(), zoom()]), [None, Some(DetectionEvent::Detected(MeetingApp::Zoom))]);
    }

    #[test]
    fn a_single_blip_is_ignored() {
        assert_eq!(run(&[zoom(), DetectionSnapshot::default(), zoom()]), [None, None, None]);
    }

    #[test]
    fn ends_after_three_negative_polls() {
        let empty = DetectionSnapshot::default();
        let events = run(&[zoom(), zoom(), empty.clone(), empty.clone(), empty]);
        assert_eq!(events[4], Some(DetectionEvent::Ended));
        assert_eq!(events[3], None);
    }

    #[test]
    fn meet_needs_a_call_title_not_the_landing_page() {
        assert_eq!(run(&[meet("Google Meet"), meet("Google Meet")]), [None, None]);
        assert_eq!(
            run(&[meet("Meet - abc-defg-hij"), meet("Meet - abc-defg-hij")])[1],
            Some(DetectionEvent::Detected(MeetingApp::GoogleMeet))
        );
    }

    #[test]
    fn meet_stays_active_while_the_browser_holds_the_mic() {
        let background = DetectionSnapshot {
            mic_user_bundle_ids: ["com.google.Chrome".to_string()].into(),
            ..Default::default()
        };
        let events = run(&[meet("Meet - x"), meet("Meet - x"), background.clone(), background.clone(), background]);
        assert!(events[2..].iter().all(Option::is_none));
    }
}
