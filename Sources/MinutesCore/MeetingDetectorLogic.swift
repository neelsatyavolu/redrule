import Foundation

public struct DetectionSnapshot: Equatable, Sendable {
    public let processNames: Set<String>
    /// Bundle identifiers of other processes currently reading from an audio input.
    public let micUserBundleIDs: Set<String>
    /// Titles of windows owned by web browsers.
    public let browserWindowTitles: [String]

    public init(processNames: Set<String>, micUserBundleIDs: Set<String>, browserWindowTitles: [String]) {
        self.processNames = processNames
        self.micUserBundleIDs = micUserBundleIDs
        self.browserWindowTitles = browserWindowTitles
    }
}

public enum DetectionEvent: Equatable, Sendable {
    case detected(MeetingApp)
    case ended
}

/// Debounced meeting detection. Pure: `ingest` returns the next state instead of mutating.
public struct MeetingDetectorLogic: Equatable, Sendable {
    public static let browserBundlePrefixes = [
        "com.google.Chrome", "com.apple.Safari", "com.apple.WebKit", "company.thebrowser",
        "com.microsoft.edgemac", "com.brave.Browser", "org.mozilla.firefox", "com.vivaldi.Vivaldi",
    ]
    /// Zoom only runs this helper while a meeting is in progress.
    static let zoomMeetingProcess = "CptHost"
    static let positivesToDetect = 2
    static let negativesToEnd = 3

    public let active: MeetingApp?
    let candidate: MeetingApp?
    let streak: Int

    public init() {
        self.init(active: nil, candidate: nil, streak: 0)
    }

    private init(active: MeetingApp?, candidate: MeetingApp?, streak: Int) {
        self.active = active
        self.candidate = candidate
        self.streak = streak
    }

    public func ingest(_ snapshot: DetectionSnapshot) -> (MeetingDetectorLogic, DetectionEvent?) {
        if let active {
            guard !Self.isStillRunning(active, in: snapshot) else {
                return (Self(active: active, candidate: nil, streak: 0), nil)
            }
            let misses = streak + 1
            if misses >= Self.negativesToEnd {
                return (Self(), .ended)
            }
            return (Self(active: active, candidate: nil, streak: misses), nil)
        }

        guard let app = Self.classify(snapshot) else { return (Self(), nil) }
        let hits = candidate == app ? streak + 1 : 1
        if hits >= Self.positivesToDetect {
            return (Self(active: app, candidate: nil, streak: 0), .detected(app))
        }
        return (Self(active: nil, candidate: app, streak: hits), nil)
    }

    static func classify(_ snapshot: DetectionSnapshot) -> MeetingApp? {
        if snapshot.processNames.contains(zoomMeetingProcess) { return .zoom }
        if browserHoldsMic(snapshot), snapshot.browserWindowTitles.contains(where: isMeetCallTitle) { return .googleMeet }
        return nil
    }

    /// Once a Meet call is active, the browser holding the mic is enough: the tab may be in the background.
    static func isStillRunning(_ app: MeetingApp, in snapshot: DetectionSnapshot) -> Bool {
        switch app {
        case .zoom: snapshot.processNames.contains(zoomMeetingProcess)
        case .googleMeet: browserHoldsMic(snapshot) || snapshot.browserWindowTitles.contains(where: isMeetCallTitle)
        case .manual: false
        }
    }

    static func browserHoldsMic(_ snapshot: DetectionSnapshot) -> Bool {
        snapshot.micUserBundleIDs.contains { id in browserBundlePrefixes.contains { id.hasPrefix($0) } }
    }

    /// In-call tabs are titled "Meet - <code or name>"; the landing page is just "Google Meet".
    static func isMeetCallTitle(_ title: String) -> Bool {
        title.hasPrefix("Meet - ") || title.hasPrefix("Meet – ") || title.hasPrefix("Meet: ")
    }
}
