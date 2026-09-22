import Foundation
import Testing
@testable import MinutesCore

@Suite struct LibraryTests {
    @Test func oldMeetingsDecodeWithoutArchiveState() throws {
        let json = #"{"id":"00000000-0000-0000-0000-000000000001","title":"Old","app":"manual","startedAt":0,"status":"done"}"#
        let meeting = try JSONDecoder().decode(Meeting.self, from: Data(json.utf8))
        #expect(!meeting.isArchived)
    }

    @Test func archiveSurvivesUpdatesAndRestore() throws {
        let root = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
        defer { try? FileManager.default.removeItem(at: root) }
        let store = try MeetingStore(root: root)
        let meeting = Meeting(id: UUID(), title: "Original", app: .manual, startedAt: Date(), endedAt: nil, status: .done, errorMessage: nil)
        try store.save(meeting.with(isArchived: true))
        let archived = try #require(try store.list().first)
        #expect(archived.isArchived)
        try store.save(archived.with(title: "Renamed"))
        #expect(try store.list().first?.isArchived == true)
        try store.save(archived.with(isArchived: false))
        #expect(try store.list().first?.isArchived == false)
    }

    @Test func renameUpdatesNoteAndMarkdownWithoutLosingContent() throws {
        let root = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
        defer { try? FileManager.default.removeItem(at: root) }
        let store = try MeetingStore(root: root)
        let meeting = Meeting(id: UUID(), title: "Original", app: .manual, startedAt: Date(), endedAt: nil, status: .done, errorMessage: nil)
        try store.save(meeting)
        let note = MeetingNote(title: "Original", tldr: "Summary", sections: [.init(heading: "Topic", bullets: ["Detail"])], decisions: ["Decision"], actionItems: [.init(owner: "Me", task: "Task")])
        try store.saveNote(note, for: meeting.id)
        try store.rename(meeting, to: "  Renamed  ")
        #expect(try store.list().first?.title == "Renamed")
        let saved = try #require(try store.note(for: meeting.id))
        #expect(saved.title == "Renamed")
        #expect(saved.sections == note.sections)
        #expect(saved.actionItems == note.actionItems)
        #expect(saved.decisions == note.decisions)
        #expect(saved.tldr == note.tldr)
        #expect(try String(contentsOf: store.folder(for: meeting.id).appendingPathComponent("notes.md"), encoding: .utf8) == saved.markdown)
        try store.rename(meeting.with(title: "Renamed"), to: " \n ")
        #expect(try store.list().first?.title == "Renamed")
    }
}
