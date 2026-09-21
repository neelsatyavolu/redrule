import AppKit
import AVFoundation
import CoreGraphics

struct Permissions: Equatable {
    let microphone: Bool
    let screenRecording: Bool

    var allGranted: Bool { microphone && screenRecording }

    static func current() -> Permissions {
        Permissions(
            microphone: AVCaptureDevice.authorizationStatus(for: .audio) == .authorized,
            screenRecording: CGPreflightScreenCaptureAccess()
        )
    }

    /// Asks for the microphone, or opens System Settings if it was already denied.
    static func requestMicrophone() async {
        if AVCaptureDevice.authorizationStatus(for: .audio) == .notDetermined {
            _ = await AVCaptureDevice.requestAccess(for: .audio)
        } else {
            openSettings(pane: "Privacy_Microphone")
        }
    }

    /// macOS only shows its own prompt once; after that the user has to flip the switch in System Settings.
    static func requestScreenRecording() {
        if !CGRequestScreenCaptureAccess() {
            openSettings(pane: "Privacy_ScreenCapture")
        }
    }

    private static func openSettings(pane: String) {
        guard let url = URL(string: "x-apple.systempreferences:com.apple.preference.security?\(pane)") else { return }
        NSWorkspace.shared.open(url)
    }
}
