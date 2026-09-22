import MinutesCore
import SwiftUI

struct ShareView: View {
    @Environment(AppModel.self) private var model
    @Environment(\.dismiss) private var dismiss
    let meeting: Meeting
    @State private var includeTranscript = false
    @State private var sharedURL: URL?
    @State private var hasShare = false
    @State private var copied = false

    var body: some View {
        VStack(alignment: .leading, spacing: 18) {
            Text("Share meeting notes").font(Theme.serif(25, .semibold))
            Text(meeting.title).font(.headline).lineLimit(2)
            Text("Anyone with the link can read the shared copy. Audio recordings are never uploaded.")
                .foregroundStyle(.secondary)
            Toggle("Include transcript and speaker names", isOn: $includeTranscript)
                .onChange(of: includeTranscript) { copied = false }
            if let sharedURL {
                Text(sharedURL.absoluteString).font(.system(size: 11, design: .monospaced))
                    .textSelection(.enabled).lineLimit(3)
            }
            if hasShare {
                Text("Local edits are included when you update the link. Stopping sharing disables it; copies someone already saved cannot be recalled.")
                    .font(.caption).foregroundStyle(.secondary)
            }
            HStack {
                if hasShare {
                    Button("Stop sharing", role: .destructive) {
                        Task {
                            if await model.revokeShare(for: meeting) { hasShare = false; sharedURL = nil; copied = false }
                        }
                    }
                }
                Spacer()
                if model.sharingBusy { ProgressView().controlSize(.small) }
                Button("Done") { dismiss() }
                Button(copied ? "Link copied" : (hasShare ? "Update & copy link" : "Create & copy link")) {
                    Task {
                        if let url = await model.publishShare(for: meeting, includeTranscript: includeTranscript) {
                            sharedURL = url; copied = true
                        }
                        hasShare = model.share(for: meeting) != nil
                    }
                }.buttonStyle(.borderedProminent)
            }
        }
        .padding(28)
        .frame(width: 500)
        .disabled(model.sharingBusy)
        .interactiveDismissDisabled(model.sharingBusy)
        .onAppear {
            if let share = model.share(for: meeting) {
                includeTranscript = share.includesTranscript; sharedURL = share.url; hasShare = true
            }
        }
    }
}
