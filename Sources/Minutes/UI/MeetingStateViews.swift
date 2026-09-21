import MinutesCore
import SwiftUI

/// Transcript lines on the pad, with the speaker and time in the margin.
struct TranscriptRows: View {
    let segments: [TranscriptSegment]

    var body: some View {
        ForEach(segments) { segment in
            PadRow(spacing: 16) {
                Text("\(segment.speaker.label)\n\(TranscriptMerger.timestamp(segment.start))")
            } content: {
                Text(segment.text)
                    .font(Theme.serif(15))
                    .lineSpacing(6)
                    .foregroundStyle(segment.speaker == .me ? Theme.ink : Theme.ink.opacity(0.78))
                    .textSelection(.enabled)
                    .fixedSize(horizontal: false, vertical: true)
            }
        }
    }
}

/// Shown while a meeting is being recorded: the running clock and the transcript as it arrives.
struct LiveView: View {
    @Environment(AppModel.self) private var model
    let meeting: Meeting

    var body: some View {
        PadPage {
            PadRow {
                RecordingDot()
            } content: {
                TimelineView(.periodic(from: meeting.startedAt, by: 1)) { context in
                    Text(Format.clock(context.date.timeIntervalSince(meeting.startedAt)))
                        .font(Theme.serif(64, .medium))
                        .monospacedDigit()
                        .foregroundStyle(Theme.ink)
                        .contentTransition(.numericText())
                }
            }

            PadRow(spacing: 4) {
                Text(meeting.app == .manual ? "Recording your microphone and this Mac's audio." : "Recording your \(meeting.app.displayName) call.")
                    .font(.system(size: 13))
                    .foregroundStyle(Theme.pencil)
            }

            PadRow(spacing: 18) {
                Button("Stop and write notes") { model.stopRecording() }
                    .buttonStyle(PrimaryButtonStyle(tint: Theme.margin))
                    .keyboardShortcut(".", modifiers: .command)
            }

            if model.liveSegments.isEmpty {
                PadRow(spacing: 40) {
                    Text(waitingMessage)
                        .font(Theme.serif(15))
                        .italic()
                        .foregroundStyle(Theme.pencil)
                }
            } else {
                Color.clear.frame(height: 22)
                TranscriptRows(segments: model.liveSegments)
            }
        }
    }

    private var waitingMessage: String {
        switch model.speechModel {
        case .loading: "The speech model is still downloading. Audio is being kept and will be transcribed as soon as it is ready."
        case .failed: "The speech model could not be loaded. Check the connection, then retry from Settings."
        case .ready: "The transcript appears here a few seconds after people start talking."
        }
    }
}

/// Shown after recording while the transcript is finished and notes are written, or when that failed.
struct ProcessingView: View {
    @Environment(AppModel.self) private var model
    let meeting: Meeting

    var body: some View {
        PadPage {
            PadRow {
                Text(meeting.startedAt.formatted(.dateTime.day().month(.abbreviated)))
            } content: {
                Text(meeting.title).font(Theme.serif(31, .semibold)).foregroundStyle(Theme.ink)
            }
            PadRow(spacing: 6) {
                Text(MeetingHeader.sentence(for: meeting)).font(.system(size: 13)).foregroundStyle(Theme.pencil)
            }

            PadRow(spacing: 28) {
                if meeting.status == .failed {
                    VStack(alignment: .leading, spacing: 14) {
                        Text(meeting.errorMessage ?? "Notes could not be written.")
                            .font(Theme.serif(16))
                            .lineSpacing(6)
                            .foregroundStyle(Theme.ink)
                            .fixedSize(horizontal: false, vertical: true)
                        HStack(spacing: 10) {
                            if !model.selectedTranscript.isEmpty {
                                Button("Write notes") { Task { await model.generateNotes(for: meeting) } }
                                    .buttonStyle(PrimaryButtonStyle())
                            }
                            if model.connected.isEmpty {
                                SettingsLink { Text("Connect an account") }.buttonStyle(QuietButtonStyle())
                            }
                        }
                    }
                } else {
                    HStack(spacing: 10) {
                        ProgressView().controlSize(.small)
                        Text(meeting.status == .transcribing ? "Finishing the transcript" : "Writing notes")
                            .font(Theme.serif(16))
                            .foregroundStyle(Theme.pencil)
                    }
                }
            }

            if !model.selectedTranscript.isEmpty {
                Color.clear.frame(height: 22)
                TranscriptRows(segments: model.selectedTranscript)
            }
        }
    }
}

struct EmptyLibraryView: View {
    @Environment(AppModel.self) private var model

    var body: some View {
        PadPage {
            PadRow {
                Text("No meetings yet").font(Theme.serif(31, .semibold)).foregroundStyle(Theme.ink)
            }
            PadRow(spacing: 14) {
                Text("Join a Zoom or Google Meet call and Minutes will offer to take notes. It listens to the call and your microphone, transcribes on this Mac, and writes up the meeting when it ends.")
                    .font(Theme.serif(16))
                    .lineSpacing(7)
                    .foregroundStyle(Theme.ink)
                    .fixedSize(horizontal: false, vertical: true)
            }
            PadRow(spacing: 22) {
                Button("Record now") { model.startRecording(app: .manual) }.buttonStyle(PrimaryButtonStyle())
            }
        }
    }
}
