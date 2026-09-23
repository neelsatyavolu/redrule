import MinutesCore
import SwiftUI

struct SettingsView: View {
    var body: some View {
        TabView {
            GeneralSettings().tabItem { Label("General", systemImage: "gearshape") }
            pane { ConnectionRows() }.tabItem { Label("Accounts", systemImage: "person.crop.circle") }
            pane { PermissionRows() }.tabItem { Label("Permissions", systemImage: "hand.raised") }
        }
        .frame(width: 520)
        .fixedSize(horizontal: false, vertical: true)
    }

    private func pane<Content: View>(@ViewBuilder _ content: () -> Content) -> some View {
        VStack(alignment: .leading, spacing: 0) { content() }
            .padding(24)
            .frame(maxWidth: .infinity, alignment: .leading)
    }
}

private struct GeneralSettings: View {
    @Environment(AppModel.self) private var model
    @State private var microphones: [MicrophoneDevice] = []

    var body: some View {
        @Bindable var model = model
        Form {
            Picker("Microphone", selection: $model.microphoneUID) {
                Text("System Default").tag("")
                ForEach(microphones) { microphone in
                    Text(microphone.name).tag(microphone.id)
                }
                if !model.microphoneUID.isEmpty, !microphones.contains(where: { $0.id == model.microphoneUID }) {
                    Text("Selected microphone (disconnected)").tag(model.microphoneUID)
                }
            }
            Text("Microphone changes apply to the next recording.")
                .font(.system(size: 12))
                .foregroundStyle(.secondary)
            Picker("Write notes with", selection: $model.modelChoiceID) {
                let _ = model.modelCatalogRevision
                ForEach(ProviderID.allCases) { provider in
                    Section(provider.displayName) {
                        ForEach(ModelChoice.all.filter { $0.provider == provider }) { choice in
                            Text(choice.label).tag(choice.id)
                        }
                    }
                }
            }
            Toggle("Keep audio recordings", isOn: $model.keepAudio)
            Text("Off by default: audio is transcribed on this Mac as the meeting runs and is never written to disk. Turn this on to keep a WAV file of each side next to the notes.")
                .font(.system(size: 12))
                .foregroundStyle(.secondary)
            LabeledContent("Speech model") {
                switch model.speechModel {
                case .ready: Text("Parakeet v3, ready")
                case .loading: Text("Downloading")
                case .failed(let message):
                    VStack(alignment: .trailing) {
                        Text(message).foregroundStyle(.secondary).lineLimit(2)
                        Button("Retry") { model.prepareSpeechModel() }
                    }
                }
            }
        }
        .formStyle(.grouped)
        .task {
            while !Task.isCancelled {
                microphones = MicrophoneDevice.available()
                do { try await Task.sleep(for: .seconds(2)) } catch { break }
            }
        }
    }
}

struct OnboardingView: View {
    @Environment(AppModel.self) private var model
    @Environment(\.scenePhase) private var scenePhase

    var body: some View {
        VStack(alignment: .leading, spacing: 0) {
            Text("Set up Redrule").font(Theme.serif(28, .semibold)).foregroundStyle(Theme.ink)
            Text("Redrule records your calls, transcribes them on this Mac, and writes the notes with your own AI account. Recording other people can require their consent, so tell them.")
                .font(Theme.serif(15))
                .lineSpacing(6)
                .foregroundStyle(Theme.pencil)
                .fixedSize(horizontal: false, vertical: true)
                .padding(.top, 8)
                .padding(.bottom, 18)

            PermissionRows()
            Divider()
            ConnectionRows()

            HStack {
                Spacer()
                Button(ready ? "Start using Redrule" : "Finish later") { model.hasOnboarded = true }
                    .buttonStyle(PrimaryButtonStyle())
                    .keyboardShortcut(.defaultAction)
            }
            .padding(.top, 22)
        }
        .padding(32)
        .frame(width: 540)
        .background(Theme.sheet)
        .onChange(of: scenePhase) { model.refreshPermissions() }
    }

    private var ready: Bool {
        model.permissions.allGranted && !model.connected.isEmpty
    }
}
