import MinutesCore
import SwiftUI

/// The finished note, set on the pad. Owners of action items hang in the margin.
struct NoteView: View {
    let meeting: Meeting
    let note: MeetingNote

    var body: some View {
        PadPage {
            PadRow {
                Text(meeting.startedAt.formatted(.dateTime.day().month(.abbreviated)))
            } content: {
                Text(note.title)
                    .font(Theme.serif(31, .semibold))
                    .foregroundStyle(Theme.ink)
                    .textSelection(.enabled)
            }

            PadRow(spacing: 6) {
                Text(MeetingHeader.sentence(for: meeting))
                    .font(.system(size: 13))
                    .foregroundStyle(Theme.pencil)
            }

            PadRow(spacing: 28) { prose(note.tldr, size: 16) }

            ForEach(Array(note.sections.enumerated()), id: \.offset) { _, section in
                PadRow(spacing: 36) { heading(section.heading) }
                ForEach(Array(section.bullets.enumerated()), id: \.offset) { _, bullet in
                    PadRow(spacing: 10) {
                        HStack(alignment: .firstTextBaseline, spacing: 10) {
                            Text("–").font(Theme.serif(15)).foregroundStyle(Theme.pencil)
                            prose(bullet)
                        }
                    }
                }
            }

            if !note.decisions.isEmpty {
                PadRow(spacing: 34) { heading("Decisions") }
                ForEach(Array(note.decisions.enumerated()), id: \.offset) { _, decision in
                    PadRow(spacing: 10) {
                        Image(systemName: "checkmark").font(.system(size: 10, weight: .semibold))
                    } content: {
                        prose(decision)
                    }
                }
            }

            if !note.actionItems.isEmpty {
                PadRow(spacing: 34) { heading("Action items") }
                ForEach(Array(note.actionItems.enumerated()), id: \.offset) { _, item in
                    PadRow(spacing: 10) {
                        Text(item.owner ?? "")
                    } content: {
                        HStack(alignment: .firstTextBaseline, spacing: 10) {
                            Image(systemName: "square").font(.system(size: 12)).foregroundStyle(Theme.pencil)
                            prose(item.task)
                        }
                    }
                }
            }
        }
    }

    private func heading(_ text: String) -> some View {
        Text(text).font(Theme.serif(19, .semibold)).foregroundStyle(Theme.ink).textSelection(.enabled)
    }

    private func prose(_ text: String, size: CGFloat = 15) -> some View {
        Text(text)
            .font(.system(size: size))
            .lineSpacing(6)
            .foregroundStyle(Theme.ink)
            .textSelection(.enabled)
            .fixedSize(horizontal: false, vertical: true)
    }
}

enum MeetingHeader {
    /// "Monday at 14:05, 42 minutes on Zoom"
    static func sentence(for meeting: Meeting) -> String {
        let day = meeting.startedAt.formatted(.dateTime.weekday(.wide))
        let time = meeting.startedAt.formatted(date: .omitted, time: .shortened)
        let length = meeting.duration.map { ", \(Format.length($0))" } ?? ""
        let place = meeting.app == .manual ? "" : " on \(meeting.app.displayName)"
        return "\(day) at \(time)\(length)\(place)"
    }
}
