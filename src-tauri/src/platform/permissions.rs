//! Microphone and Screen & System Audio Recording permissions, and the optional calendar.
use block2::RcBlock;
use objc2::runtime::Bool;
use objc2_av_foundation::{AVAuthorizationStatus, AVCaptureDevice, AVMediaTypeAudio};
use objc2_core_graphics::{CGPreflightScreenCaptureAccess, CGRequestScreenCaptureAccess};
use serde::Serialize;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Permissions {
    pub microphone: bool,
    pub screen_recording: bool,
    /// Optional: only names meetings, never needed to record.
    pub calendar: bool,
}

impl Permissions {
    pub fn current() -> Self {
        Self {
            microphone: microphone_status() == AVAuthorizationStatus::Authorized,
            screen_recording: CGPreflightScreenCaptureAccess(),
            calendar: super::calendar::granted(),
        }
    }

    pub fn all_granted(&self) -> bool {
        self.microphone && self.screen_recording
    }
}

fn microphone_status() -> AVAuthorizationStatus {
    // SAFETY: AVMediaTypeAudio is an immutable framework constant.
    match unsafe { AVMediaTypeAudio } {
        Some(audio) => unsafe { AVCaptureDevice::authorizationStatusForMediaType(audio) },
        None => AVAuthorizationStatus::NotDetermined,
    }
}

/// Asks for the microphone, or opens System Settings if it was already decided.
pub async fn request_microphone() {
    if microphone_status() != AVAuthorizationStatus::NotDetermined {
        open_settings("Privacy_Microphone");
        return;
    }
    if let Some(answered) = ask_for_microphone() {
        let _ = answered.await;
    }
}

/// Shows the system prompt. The Objective-C block is not `Send`, so it never crosses an await.
fn ask_for_microphone() -> Option<tokio::sync::oneshot::Receiver<()>> {
    let audio = unsafe { AVMediaTypeAudio }?;
    let (sender, receiver) = tokio::sync::oneshot::channel::<()>();
    let sender = std::sync::Mutex::new(Some(sender));
    let handler = RcBlock::new(move |_granted: Bool| {
        if let Some(sender) = sender.lock().ok().and_then(|mut s| s.take()) {
            let _ = sender.send(());
        }
    });
    // SAFETY: AVFoundation retains the handler until it runs, on an arbitrary queue.
    unsafe { AVCaptureDevice::requestAccessForMediaType_completionHandler(audio, &handler) };
    Some(receiver)
}

/// macOS only shows its own prompt once; after that the switch has to be flipped in System Settings.
pub fn request_screen_recording() {
    if !CGRequestScreenCaptureAccess() {
        open_settings("Privacy_ScreenCapture");
    }
}

pub(super) fn open_settings(pane: &str) {
    let url = format!("x-apple.systempreferences:com.apple.preference.security?{pane}");
    let _ = std::process::Command::new("/usr/bin/open").arg(url).spawn();
}
