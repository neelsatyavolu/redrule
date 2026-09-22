import MinutesCore
import SwiftUI

struct MeetingContentView: View {
    @Environment(AppModel.self) private var model
    let meeting: Meeting
    let note: MeetingNote
    @State private var showsTranscript = false
    @State private var editingSpeaker: TranscriptSegment?
    @State private var speakerName = ""

    var body: some View {
        VStack(spacing: 0) {
            Picker("Meeting content", selection: $showsTranscript) {
                Text("Notes").tag(false)
                Text("Transcript").tag(true)
            }
            .pickerStyle(.segmented)
            .frame(width: 260)
            .padding(16)
            Divider()
            if showsTranscript {
                PadPage {
                    PadRow { Text("Transcript").font(Theme.serif(31, .semibold)) }
                    PadRow(spacing: 10) {
                        Text("Speaker labels are estimated. Select a name below to rename that speaker throughout this meeting.")
                            .font(.system(size: 12)).foregroundStyle(Theme.pencil)
                    }
                    PadRow(spacing: 14) {
                        ViewThatFits(in: .horizontal) {
                            speakerButtons
                            ScrollView(.horizontal) { speakerButtons }
                        }
                    }
                    if model.selectedTranscript.isEmpty {
                        PadRow(spacing: 28) { Text("No transcript is available for this meeting.").foregroundStyle(Theme.pencil) }
                    } else {
                        Color.clear.frame(height: 14)
                        TranscriptRows(segments: model.selectedTranscript)
                    }
                }
            } else {
                NoteView(meeting: meeting, note: note)
            }
        }
        .alert("Rename speaker", isPresented: .init(get: { editingSpeaker != nil }, set: { if !$0 { editingSpeaker = nil } })) {
            TextField("Name", text: $speakerName)
            Button("Cancel", role: .cancel) { editingSpeaker = nil }
            Button("Save") {
                if let editingSpeaker { model.renameSpeaker(editingSpeaker.speakerKey, to: speakerName, in: meeting) }
                editingSpeaker = nil
            }
        } message: {
            Text("This updates the transcript on this Mac. Rewrite notes or update the shared link to include the new name there. Leave blank to restore the original label.")
        }
    }

    private var speakerButtons: some View {
        HStack(spacing: 8) {
            ForEach(speakers, id: \.speakerKey) { speaker in
                Button(speaker.speakerLabel) { speakerName = speaker.speakerName ?? ""; editingSpeaker = speaker }
                    .buttonStyle(.bordered)
                    .help("Rename \(speaker.speakerLabel)")
            }
        }
    }

    private var speakers: [TranscriptSegment] {
        var seen: Set<String> = []
        return model.selectedTranscript.filter { seen.insert($0.speakerKey).inserted }
    }
}
