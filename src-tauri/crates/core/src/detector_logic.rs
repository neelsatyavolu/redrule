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
/// Desktop apps that open the microphone only for calls, by bundle id prefix. FaceTime's audio
/// runs in `avconferenced`, which also carries Continuity phone calls.
const NATIVE_CALL_APPS: &[(&str, MeetingApp)] = &[
    ("com.microsoft.teams", MeetingApp::Teams),
    ("com.tinyspeck.slackmacgap", MeetingApp::Slack),
    ("Cisco-Systems.Spark", MeetingApp::Webex),
    ("com.cisco.webexmeetingsapp", MeetingApp::Webex),
    ("com.webex.meetingmanager", MeetingApp::Webex),
    ("com.apple.FaceTime", MeetingApp::FaceTime),
    ("com.apple.avconferenced", MeetingApp::FaceTime),
];
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
    /// The active call runs in a browser tab rather than the app's desktop client.
    in_browser: bool,
    candidate: Option<MeetingApp>,
    streak: u32,
}

impl DetectorLogic {
    pub fn ingest(&self, snapshot: &DetectionSnapshot) -> (Self, Option<DetectionEvent>) {
        if let Some(active) = self.active {
            if is_still_running(active, self.in_browser, snapshot) {
                return (Self { streak: 0, ..self.clone() }, None);
            }
            let misses = self.streak + 1;
            if misses >= NEGATIVES_TO_END {
                return (Self::default(), Some(DetectionEvent::Ended));
            }
            return (Self { streak: misses, ..self.clone() }, None);
        }

        let Some(app) = classify(snapshot) else { return (Self::default(), None) };
        let hits = if self.candidate == Some(app) { self.streak + 1 } else { 1 };
        if hits >= POSITIVES_TO_DETECT {
            let in_browser = app == MeetingApp::GoogleMeet || (app == MeetingApp::Teams && !native_holds_mic(app, snapshot));
            return (Self { active: Some(app), in_browser, candidate: None, streak: 0 }, Some(DetectionEvent::Detected(app)));
        }
        (Self { candidate: Some(app), streak: hits, ..Self::default() }, None)
    }
}

fn classify(snapshot: &DetectionSnapshot) -> Option<MeetingApp> {
    if snapshot.process_names.contains(ZOOM_MEETING_PROCESS) {
        return Some(MeetingApp::Zoom);
    }
    if let Some(app) = native_call_app(snapshot) {
        return Some(app);
    }
    if !browser_holds_mic(snapshot) {
        return None;
    }
    let titles = &snapshot.browser_window_titles;
    if titles.iter().any(|t| is_meet_call_title(t)) {
        return Some(MeetingApp::GoogleMeet);
    }
    titles.iter().any(|t| is_teams_title(t)).then_some(MeetingApp::Teams)
}

/// Once a browser call is active, the browser holding the mic is enough: the tab may be in the background.
fn is_still_running(app: MeetingApp, in_browser: bool, snapshot: &DetectionSnapshot) -> bool {
    match app {
        MeetingApp::Zoom => snapshot.process_names.contains(ZOOM_MEETING_PROCESS),
        MeetingApp::GoogleMeet => {
            browser_holds_mic(snapshot) || snapshot.browser_window_titles.iter().any(|t| is_meet_call_title(t))
        }
        MeetingApp::Teams if in_browser => browser_holds_mic(snapshot),
        MeetingApp::Teams | MeetingApp::Slack | MeetingApp::Webex | MeetingApp::FaceTime => native_holds_mic(app, snapshot),
        MeetingApp::Manual => false,
    }
}

fn native_call_app(snapshot: &DetectionSnapshot) -> Option<MeetingApp> {
    NATIVE_CALL_APPS
        .iter()
        .find(|(prefix, _)| snapshot.mic_user_bundle_ids.iter().any(|id| id.starts_with(prefix)))
        .map(|&(_, app)| app)
}

fn native_holds_mic(app: MeetingApp, snapshot: &DetectionSnapshot) -> bool {
    NATIVE_CALL_APPS
        .iter()
        .filter(|&&(_, candidate)| candidate == app)
        .any(|(prefix, _)| snapshot.mic_user_bundle_ids.iter().any(|id| id.starts_with(prefix)))
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

/// Teams on the web titles every tab "<view> | Microsoft Teams"; the browser holding the mic marks a call.
fn is_teams_title(title: &str) -> bool {
    title.ends_with("| Microsoft Teams")
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

    fn mic(bundle_id: &str) -> DetectionSnapshot {
        DetectionSnapshot { mic_user_bundle_ids: [bundle_id.to_string()].into(), ..Default::default() }
    }

    #[test]
    fn detects_desktop_call_apps_by_the_microphone() {
        for (bundle_id, app) in [
            ("com.microsoft.teams2", MeetingApp::Teams),
            ("com.tinyspeck.slackmacgap", MeetingApp::Slack),
            ("Cisco-Systems.Spark", MeetingApp::Webex),
            ("com.apple.avconferenced", MeetingApp::FaceTime),
        ] {
            assert_eq!(run(&[mic(bundle_id), mic(bundle_id)])[1], Some(DetectionEvent::Detected(app)), "{bundle_id}");
        }
    }

    #[test]
    fn a_desktop_call_ends_when_its_app_releases_the_mic() {
        let slack = mic("com.tinyspeck.slackmacgap");
        let other = mic("com.microsoft.teams2");
        let events = run(&[slack.clone(), slack, other.clone(), other.clone(), other]);
        assert_eq!(events[4], Some(DetectionEvent::Ended));
    }

    #[test]
    fn teams_on_the_web_needs_the_mic_and_a_teams_tab() {
        let tab = |mic: bool| DetectionSnapshot {
            mic_user_bundle_ids: if mic { ["com.google.Chrome".to_string()].into() } else { Default::default() },
            browser_window_titles: vec!["Meeting with Ada | Microsoft Teams".into()],
            ..Default::default()
        };
        assert_eq!(run(&[tab(false), tab(false)]), [None, None]);
        assert_eq!(run(&[tab(true), tab(true)])[1], Some(DetectionEvent::Detected(MeetingApp::Teams)));
    }

    #[test]
    fn a_desktop_teams_call_ends_even_while_a_browser_holds_the_mic() {
        let desktop = mic("com.microsoft.teams2");
        let browser = mic("com.google.Chrome");
        let events = run(&[desktop.clone(), desktop, browser.clone(), browser.clone(), browser]);
        assert_eq!(events[4], Some(DetectionEvent::Ended));
    }

    #[test]
    fn zoom_wins_over_other_mic_users() {
        let both = DetectionSnapshot { mic_user_bundle_ids: ["com.tinyspeck.slackmacgap".to_string()].into(), ..zoom() };
        assert_eq!(run(&[both.clone(), both])[1], Some(DetectionEvent::Detected(MeetingApp::Zoom)));
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
