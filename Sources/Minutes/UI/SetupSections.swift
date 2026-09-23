import MinutesCore
import SwiftUI

/// A titled line with a status and one action. Shared by onboarding and Settings.
struct SetupRow<Action: View>: View {
    let title: String
    let detail: String
    let done: Bool
    @ViewBuilder var action: Action

    var body: some View {
        HStack(alignment: .firstTextBaseline, spacing: 12) {
            Image(systemName: done ? "checkmark.circle.fill" : "circle")
                .foregroundStyle(done ? Theme.ink : Theme.pencil)
                .font(.system(size: 14))
            VStack(alignment: .leading, spacing: 3) {
                Text(title).font(.system(size: 13, weight: .medium)).foregroundStyle(Theme.ink)
                Text(detail).font(.system(size: 12)).foregroundStyle(Theme.pencil).fixedSize(horizontal: false, vertical: true)
            }
            Spacer(minLength: 12)
            action
        }
        .padding(.vertical, 10)
    }
}

struct PermissionRows: View {
    @Environment(AppModel.self) private var model

    var body: some View {
        SetupRow(
            title: "Microphone",
            detail: "So your side of the conversation is in the notes.",
            done: model.permissions.microphone
        ) {
            if !model.permissions.microphone {
                Button("Allow") {
                    Task {
                        await Permissions.requestMicrophone()
                        model.refreshPermissions()
                    }
                }
            }
        }
        Divider()
        SetupRow(
            title: "Screen & System Audio Recording",
            detail: "macOS groups call audio under this permission. Redrule records sound only, never the screen. Quit and reopen Redrule after switching it on.",
            done: model.permissions.screenRecording
        ) {
            if !model.permissions.screenRecording {
                Button("Allow") { Permissions.requestScreenRecording() }
            }
        }
    }
}

struct ConnectionRows: View {
    @Environment(AppModel.self) private var model
    @State private var pastedCode = ""

    var body: some View {
        ForEach(ProviderID.allCases) { provider in
            if provider != ProviderID.allCases.first { Divider() }
            SetupRow(
                title: provider.displayName,
                detail: provider == .codex ? "Writes notes with your ChatGPT plan." : "Writes notes with your Grok account.",
                done: model.connected.contains(provider)
            ) {
                if model.connected.contains(provider) {
                    Button("Disconnect") { model.disconnect(provider) }
                } else if model.connecting == provider {
                    Button("Cancel") { model.cancelConnecting() }
                } else {
                    Button("Connect") { model.connect(provider) }.disabled(model.connecting != nil)
                }
            }
            if model.connecting == provider { pasteFallback }
        }
        if let error = model.connectionError {
            Text(error).font(.system(size: 12)).foregroundStyle(Theme.margin).padding(.top, 6)
        }
    }

    private var pasteFallback: some View {
        VStack(alignment: .leading, spacing: 6) {
            Text("Finish signing in in your browser. If the browser ends on a page that will not load, paste that page's address here.")
                .font(.system(size: 12))
                .foregroundStyle(Theme.pencil)
                .fixedSize(horizontal: false, vertical: true)
            HStack {
                TextField("Code or address", text: $pastedCode).textFieldStyle(.roundedBorder)
                Button("Finish connecting") {
                    model.submitPastedCode(pastedCode)
                    pastedCode = ""
                }
                .disabled(pastedCode.trimmingCharacters(in: .whitespaces).isEmpty)
            }
        }
        .padding(.leading, 26)
        .padding(.bottom, 10)
    }
}
