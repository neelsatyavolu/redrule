import MinutesCore
import SwiftUI

struct MainWindow: View {
    @Environment(AppModel.self) private var model
    @State private var showsTranscript = false
    @State private var sharingMeeting: Meeting?
    @State private var pendingDelete: Meeting?
    @State private var renamingMeeting: Meeting?
    @State private var meetingTitle = ""
    @State private var editingMeeting: Meeting?
    @State private var columnVisibility: NavigationSplitViewVisibility = .all

    var body: some View {
        @Bindable var model = model
        NavigationSplitView(columnVisibility: $columnVisibility) {
            Sidebar { meeting in
                meetingMenu(for: meeting)
            }
                .navigationSplitViewColumnWidth(min: 220, ideal: 250, max: 320)
        } detail: {
            detail
                .inspector(isPresented: $showsTranscript) {
                    PadPage { TranscriptRows(segments: model.selectedTranscript) }
                        .inspectorColumnWidth(min: 300, ideal: 380, max: 520)
                }
                .environment(\.showsPadRule, columnVisibility == .detailOnly)
        }
        .toolbar { toolbar }
        .sheet(item: $sharingMeeting) { meeting in ShareView(meeting: meeting) }
        .sheet(item: $editingMeeting) { meeting in NoteEditor(meeting: meeting) }
        .alert("Rename meeting", isPresented: .init(get: { renamingMeeting != nil }, set: { if !$0 { renamingMeeting = nil } })) {
            TextField("Title", text: $meetingTitle)
            Button("Cancel", role: .cancel) { renamingMeeting = nil }
            Button("Save") {
                if let renamingMeeting { model.rename(renamingMeeting, to: meetingTitle) }
                renamingMeeting = nil
            }
            .disabled(meetingTitle.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty)
        }
        .background(Theme.sheet)
        .sheet(isPresented: .init(get: { !model.hasOnboarded }, set: { if !$0 { model.hasOnboarded = true } })) {
            OnboardingView()
        }
        .alert("Something went wrong", isPresented: .init(get: { model.errorMessage != nil }, set: { if !$0 { model.errorMessage = nil } })) {
            Button("OK") { model.errorMessage = nil }
        } message: {
            Text(model.errorMessage ?? "")
        }
        .confirmationDialog("Delete this meeting?", isPresented: .init(get: { pendingDelete != nil }, set: { if !$0 { pendingDelete = nil } })) {
            Button("Delete meeting", role: .destructive) {
                if let pendingDelete { model.delete(pendingDelete) }
                pendingDelete = nil
            }
        } message: {
            Text("Any shared link will be revoked, then the notes and transcript will be removed from this Mac. This cannot be undone.")
        }
    }

    @ViewBuilder private func meetingMenu(for meeting: Meeting) -> some View {
        Button("Rename…", systemImage: "pencil") {
            meetingTitle = meeting.title
            renamingMeeting = meeting
        }
        .disabled(!model.canEdit(meeting))
        Button("Edit Notes…", systemImage: "square.and.pencil") { editingMeeting = meeting }
            .disabled(!model.canEdit(meeting) || meeting.status != .done)
        Divider()
        Button("Share…", systemImage: "square.and.arrow.up") { sharingMeeting = meeting }
            .disabled(meeting.status != .done || model.sharingBusy)
        Button("Copy as Markdown", systemImage: "doc.on.doc") { model.copyMarkdown(for: meeting) }
            .disabled(meeting.status != .done)
        Divider()
        Button(meeting.isArchived ? "Restore from Archive" : "Archive", systemImage: "archivebox") {
            model.toggleArchive(meeting)
        }
        .disabled(!model.canEdit(meeting))
        Button("Delete…", systemImage: "trash", role: .destructive) { pendingDelete = meeting }
            .disabled(!model.canEdit(meeting) || model.sharingBusy)
    }

    @ViewBuilder private var detail: some View {
        if let meeting = model.selectedMeeting {
            if meeting.id == model.recording?.id {
                LiveView(meeting: meeting)
            } else if meeting.status == .done, let note = model.selectedNote {
                MeetingContentView(meeting: meeting, note: note).id(meeting.id)
            } else {
                ProcessingView(meeting: meeting)
            }
        } else {
            EmptyLibraryView()
        }
    }

    @ToolbarContentBuilder private var toolbar: some ToolbarContent {
        ToolbarItem(placement: .navigation) {
            if model.isRecording {
                Button { model.stopRecording() } label: { Label("Stop recording", systemImage: "stop.circle.fill") }
                    .tint(Theme.margin)
            } else {
                Button { model.startRecording(app: .manual) } label: { Label("Record", systemImage: "record.circle") }
                    .help("Record a meeting now")
            }
        }
        if let meeting = model.selectedMeeting, meeting.id != model.recording?.id {
            ToolbarItemGroup(placement: .primaryAction) {
                if model.selectedNote != nil {
                    Button { sharingMeeting = meeting } label: { Label("Share", systemImage: "square.and.arrow.up") }
                        .help("Share a copyable link")
                        .disabled(model.sharingBusy)
                    Button { model.copyMarkdown() } label: { Label("Copy as Markdown", systemImage: "doc.on.doc") }
                        .help("Copy the notes as Markdown")
                    Button { Task { await model.generateNotes(for: meeting) } } label: { Label("Rewrite notes", systemImage: "arrow.clockwise") }
                        .help("Write the notes again from the transcript")
                        .disabled(meeting.status != .done)
                }
                Button { showsTranscript.toggle() } label: { Label("Transcript", systemImage: "text.quote") }
                    .help("Show the transcript")
                    .disabled(model.selectedTranscript.isEmpty)
                Button { pendingDelete = meeting } label: { Label("Delete", systemImage: "trash") }
                    .help("Delete this meeting")
                    .disabled(model.sharingBusy)
            }
        }
    }
}

private struct Sidebar<MenuContent: View>: View {
    @Environment(AppModel.self) private var model
    @ViewBuilder let menu: (Meeting) -> MenuContent

    var body: some View {
        @Bindable var model = model
        List(selection: $model.selection) {
            ForEach(groups, id: \.day) { group in
                Section(group.label) {
                    ForEach(group.meetings) { meeting in
                        SidebarRow(meeting: meeting, isLive: meeting.id == model.recording?.id).tag(meeting.id)
                            .contextMenu { menu(meeting) }
                    }
                }
            }
        }
        .listStyle(.sidebar)
        .safeAreaInset(edge: .top) {
            Picker("Library", selection: $model.showsArchived) {
                Text("Meetings").tag(false)
                Text("Archive").tag(true)
            }
            .pickerStyle(.segmented)
            .padding(10)
        }
        .onChange(of: model.showsArchived) { _, _ in
            model.selection = model.visibleMeetings.first?.id
        }
        .onChange(of: model.recording?.id) { _, id in
            if let id {
                model.showsArchived = false
                model.selection = id
            }
        }
        .safeAreaInset(edge: .bottom) { speechModelNotice }
    }

    private var groups: [(day: Date, label: String, meetings: [Meeting])] {
        let calendar = Calendar.current
        return Dictionary(grouping: model.visibleMeetings) { calendar.startOfDay(for: $0.startedAt) }
            .sorted { $0.key > $1.key }
            .map { day, meetings in
                let label = calendar.isDateInToday(day) ? "Today"
                    : calendar.isDateInYesterday(day) ? "Yesterday"
                    : day.formatted(.dateTime.weekday(.wide).day().month(.wide))
                return (day, label, meetings)
            }
    }

    @ViewBuilder private var speechModelNotice: some View {
        switch model.speechModel {
        case .ready:
            EmptyView()
        case .loading:
            HStack(spacing: 8) {
                ProgressView().controlSize(.small)
                Text("Getting the speech model ready").font(.system(size: 11.5)).foregroundStyle(.secondary)
            }
            .padding(12)
            .frame(maxWidth: .infinity, alignment: .leading)
        case .failed:
            Button("Speech model failed to load. Retry") { model.prepareSpeechModel() }
                .buttonStyle(.link)
                .font(.system(size: 11.5))
                .padding(12)
                .frame(maxWidth: .infinity, alignment: .leading)
        }
    }
}

private struct SidebarRow: View {
    let meeting: Meeting
    let isLive: Bool

    var body: some View {
        HStack(spacing: 8) {
            VStack(alignment: .leading, spacing: 2) {
                Text(meeting.title).font(.system(size: 13, weight: .medium)).lineLimit(1)
                Text(subtitle).font(.system(size: 11.5)).foregroundStyle(.secondary).monospacedDigit()
            }
            Spacer(minLength: 0)
            if isLive {
                RecordingDot(size: 7)
            } else if meeting.status == .failed {
                Image(systemName: "exclamationmark.triangle").font(.system(size: 11)).foregroundStyle(.secondary)
            } else if meeting.status != .done {
                ProgressView().controlSize(.mini)
            }
        }
        .padding(.vertical, 3)
    }

    private var subtitle: String {
        let time = meeting.startedAt.formatted(date: .omitted, time: .shortened)
        guard let duration = meeting.duration else { return time }
        return "\(time), \(Format.length(duration))"
    }
}
