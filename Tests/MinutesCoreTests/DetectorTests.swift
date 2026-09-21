import Testing
@testable import MinutesCore

@Suite struct DetectorTests {
    let zoom = DetectionSnapshot(processNames: ["zoom.us", "CptHost"], micUserBundleIDs: [], browserWindowTitles: [])
    let meet = DetectionSnapshot(
        processNames: ["Google Chrome"],
        micUserBundleIDs: ["com.google.Chrome.helper"],
        browserWindowTitles: ["Meet - abc-defg-hij - Google Chrome"]
    )
    let nothing = DetectionSnapshot(processNames: ["Finder"], micUserBundleIDs: [], browserWindowTitles: ["Inbox"])

    private func run(_ snapshots: [DetectionSnapshot]) -> [DetectionEvent] {
        var logic = MeetingDetectorLogic()
        var events: [DetectionEvent] = []
        for snapshot in snapshots {
            let (next, event) = logic.ingest(snapshot)
            logic = next
            if let event { events.append(event) }
        }
        return events
    }

    @Test func zoomNeedsTwoPositiveSnapshots() {
        #expect(run([zoom]).isEmpty)
        #expect(run([zoom, zoom]) == [.detected(.zoom)])
    }

    @Test func detectionFiresOncePerMeeting() {
        #expect(run([zoom, zoom, zoom, zoom]) == [.detected(.zoom)])
    }

    @Test func meetNeedsBrowserUsingMic() {
        let muted = DetectionSnapshot(processNames: [], micUserBundleIDs: [], browserWindowTitles: meet.browserWindowTitles)
        #expect(run([muted, muted]).isEmpty)
        #expect(run([meet, meet]) == [.detected(.googleMeet)])
    }

    @Test func meetLandingPageIsNotAMeeting() {
        let landing = DetectionSnapshot(processNames: [], micUserBundleIDs: [], browserWindowTitles: ["Google Meet"])
        #expect(run([landing, landing]).isEmpty)
    }

    @Test func endsAfterThreeNegativeSnapshots() {
        #expect(run([zoom, zoom, nothing, nothing]) == [.detected(.zoom)])
        #expect(run([zoom, zoom, nothing, nothing, nothing]) == [.detected(.zoom), .ended])
    }

    @Test func meetSurvivesTabSwitchWhileBrowserHoldsMic() {
        let otherTab = DetectionSnapshot(
            processNames: [], micUserBundleIDs: ["com.google.Chrome.helper"], browserWindowTitles: ["Docs"]
        )
        #expect(run([meet, meet, otherTab, otherTab, otherTab, otherTab]) == [.detected(.googleMeet)])
    }

    @Test func blipResetsDebounce() {
        #expect(run([zoom, nothing, zoom]).isEmpty)
    }

    @Test func canDetectAgainAfterEnding() {
        let events = run([zoom, zoom, nothing, nothing, nothing, meet, meet])
        #expect(events == [.detected(.zoom), .ended, .detected(.googleMeet)])
    }
}
