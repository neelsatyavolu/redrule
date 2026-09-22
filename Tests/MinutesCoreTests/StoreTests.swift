import Foundation
import Testing
@testable import MinutesCore

@Suite struct StoreTests {
    private func makeStore() throws -> MeetingStore {
        let root = FileManager.default.temporaryDirectory.appendingPathComponent("minutes-tests-\(UUID().uuidString)")
        return try MeetingStore(root: root)
    }

    private func meeting(_ title: String, started: TimeInterval) -> Meeting {
        Meeting(id: UUID(), title: title, app: .zoom, startedAt: Date(timeIntervalSince1970: started), endedAt: nil, status: .recording, errorMessage: nil)
    }

    @Test func savesAndListsNewestFirst() throws {
        let store = try makeStore()
        try store.save(meeting("old", started: 100))
        try store.save(meeting("new", started: 200))
        #expect(try store.list().map(\.title) == ["new", "old"])
    }

    @Test func saveOverwritesSameMeeting() throws {
        let store = try makeStore()
        let original = meeting("draft", started: 1)
        try store.save(original)
        try store.save(original.with(title: "final", status: .done))
        let all = try store.list()
        #expect(all.count == 1)
        #expect(all.first?.title == "final")
        #expect(all.first?.status == .done)
    }

    @Test func transcriptAndNoteRoundTrip() throws {
        let store = try makeStore()
        let m = meeting("m", started: 1)
        try store.save(m)
        #expect(try store.transcript(for: m.id).isEmpty)
        #expect(try store.note(for: m.id) == nil)

        let segments = [TranscriptSegment(speaker: .me, start: 0, end: 1, text: "hi")]
        let note = MeetingNote(title: "T", tldr: "S", sections: [], decisions: ["d"], actionItems: [])
        try store.saveTranscript(segments, for: m.id)
        try store.saveNote(note, for: m.id)

        #expect(try store.transcript(for: m.id) == segments)
        #expect(try store.note(for: m.id) == note)
        let markdown = try String(contentsOf: store.folder(for: m.id).appendingPathComponent("notes.md"), encoding: .utf8)
        #expect(markdown == note.markdown)
    }

    @Test func deleteRemovesFolder() throws {
        let store = try makeStore()
        let m = meeting("m", started: 1)
        try store.save(m)
        try store.delete(m.id)
        #expect(try store.list().isEmpty)
    }

    @Test func listSkipsCorruptFolders() throws {
        let store = try makeStore()
        try store.save(meeting("good", started: 1))
        let bad = store.folder(for: UUID())
        try FileManager.default.createDirectory(at: bad, withIntermediateDirectories: true)
        try Data("nope".utf8).write(to: bad.appendingPathComponent("meeting.json"))
        #expect(try store.list().map(\.title) == ["good"])
    }

    @Test func interruptedMeetingsAreMarkedFailed() throws {
        let store = try makeStore()
        try store.save(meeting("crashed", started: 1))
        try store.recoverInterrupted()
        #expect(try store.list().first?.status == .failed)
    }
    @Test func speakerRenamePersistsWithoutChangingOtherSpeakers() throws {
        let store = try makeStore()
        defer { try? FileManager.default.removeItem(at: store.root) }
        let m = meeting("speakers", started: 1)
        try store.save(m)
        let a = TranscriptSegment(speaker: .them, start: 0, end: 1, text: "Hello", speakerID: "1")
        let b = TranscriptSegment(speaker: .them, start: 2, end: 3, text: "Hi", speakerID: "2")
        try store.saveTranscript([a, b], for: m.id)
        try store.renameSpeaker(a.speakerKey, to: " Alice ", for: m.id)
        #expect(try store.transcript(for: m.id).map(\.speakerLabel) == ["Alice", "Speaker 2"])
        try store.renameSpeaker(a.speakerKey, to: "", for: m.id)
        #expect(try store.transcript(for: m.id) == [a, b])
    }

    @Test func shareHandleSurvivesReloadAndCanBeRemoved() throws {
        let store = try makeStore()
        defer { try? FileManager.default.removeItem(at: store.root) }
        let m = meeting("shared", started: 1)
        try store.save(m)
        #expect(try store.share(for: m.id) == nil)
        let share = MeetingShare(id: "opaque", includesTranscript: false, url: URL(string: "https://example.com/s/opaque")!)
        try store.saveShare(share, for: m.id)
        let reopened = try MeetingStore(root: store.root)
        #expect(try reopened.share(for: m.id) == share)
        try reopened.removeShare(for: m.id)
        try reopened.removeShare(for: m.id)
        #expect(try store.share(for: m.id) == nil)
    }

}
