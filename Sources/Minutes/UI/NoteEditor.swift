import MinutesCore
import SwiftUI

struct NoteEditor: View {
    @Environment(AppModel.self) private var model
    @Environment(\.dismiss) private var dismiss
    let meeting: Meeting
    @State private var title = ""
    @State private var summary = ""
    @State private var sections: [SectionDraft] = []
    @State private var decisions = ""
    @State private var actions: [ActionDraft] = []
    @State private var loaded = false
    @State private var failure: String?

    private struct SectionDraft: Identifiable {
        let id = UUID()
        var heading: String
        var bullets: String
    }

    private struct ActionDraft: Identifiable {
        let id = UUID()
        var owner: String
        var task: String
    }

    var body: some View {
        VStack(spacing: 0) {
            HStack {
                Text("Edit Notes").font(.title2.bold())
                Spacer()
                Button("Cancel") { dismiss() }.keyboardShortcut(.cancelAction)
                Button("Save") { save() }
                    .keyboardShortcut(.defaultAction)
                    .disabled(!loaded || title.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty)
            }
            .padding(20)
            Divider()
            if let failure {
                Text(failure).foregroundStyle(.red).padding()
            }
            if loaded {
                Form {
                    Section("Title") { TextField("Title", text: $title) }
                    Section("Summary") { TextField("Summary", text: $summary, axis: .vertical) }
                    Section("Sections") {
                        ForEach($sections) { $section in
                            VStack(alignment: .leading, spacing: 8) {
                                HStack {
                                    TextField("Heading", text: $section.heading)
                                    Button("Remove section", systemImage: "minus.circle", role: .destructive) {
                                        sections.removeAll { $0.id == section.id }
                                    }.labelStyle(.iconOnly)
                                }
                                TextField("Bullets (one per line)", text: $section.bullets, axis: .vertical)
                            }
                        }
                        Button("Add Section", systemImage: "plus") { sections.append(.init(heading: "", bullets: "")) }
                    }
                    Section("Decisions") {
                        TextField("Decisions (one per line)", text: $decisions, axis: .vertical)
                    }
                    Section("Action items") {
                        ForEach($actions) { $action in
                            HStack(alignment: .top) {
                                TextField("Owner (optional)", text: $action.owner).frame(width: 130)
                                TextField("Task", text: $action.task, axis: .vertical)
                                Button("Remove action item", systemImage: "minus.circle", role: .destructive) {
                                    actions.removeAll { $0.id == action.id }
                                }.labelStyle(.iconOnly)
                            }
                        }
                        Button("Add Action Item", systemImage: "plus") { actions.append(.init(owner: "", task: "")) }
                    }
                    Text("Edits are saved on this Mac. Update the shared link to publish your changes.")
                        .font(.caption).foregroundStyle(.secondary)
                }
                .formStyle(.grouped)
            }
        }
        .frame(width: 640, height: 620)
        .task { load() }
    }

    private func load() {
        do {
            guard let note = try model.store?.note(for: meeting.id) else {
                failure = "No saved notes are available for this meeting."
                return
            }
            title = note.title
            summary = note.tldr
            sections = note.sections.map { .init(heading: $0.heading, bullets: $0.bullets.joined(separator: "\n")) }
            decisions = note.decisions.joined(separator: "\n")
            actions = note.actionItems.map { .init(owner: $0.owner ?? "", task: $0.task) }
            loaded = true
        } catch {
            failure = "The notes could not be opened. \(error.localizedDescription)"
        }
    }

    private func save() {
        let note = MeetingNote(
            title: title.trimmingCharacters(in: .whitespacesAndNewlines), tldr: summary,
            sections: sections.map { .init(heading: $0.heading, bullets: lines($0.bullets)) },
            decisions: lines(decisions),
            actionItems: actions.compactMap {
                let task = $0.task.trimmingCharacters(in: .whitespacesAndNewlines)
                let owner = $0.owner.trimmingCharacters(in: .whitespacesAndNewlines)
                return task.isEmpty ? nil : ActionItem(owner: owner.isEmpty ? nil : owner, task: task)
            }
        )
        if model.saveEditedNote(note, for: meeting) {
            dismiss()
        } else {
            failure = model.errorMessage ?? "This meeting is no longer available for editing."
            model.errorMessage = nil
        }
    }

    private func lines(_ text: String) -> [String] {
        text.components(separatedBy: .newlines)
            .map { $0.trimmingCharacters(in: .whitespaces) }
            .filter { !$0.isEmpty }
    }
}
