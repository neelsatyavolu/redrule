import AppKit
import SwiftUI

@main
struct MinutesApp: App {
    @NSApplicationDelegateAdaptor private var delegate: AppDelegate

    var body: some Scene {
        Window("Minutes", id: "main") {
            MainWindow()
                .environment(delegate.model)
                .frame(minWidth: 860, minHeight: 560)
        }
        .defaultSize(width: 1080, height: 720)
        .commands {
            CommandGroup(replacing: .newItem) {
                Button(delegate.model.isRecording ? "Stop Recording" : "Record Meeting") {
                    delegate.model.isRecording ? delegate.model.stopRecording() : delegate.model.startRecording(app: .manual)
                }
                .keyboardShortcut("r", modifiers: [.command, .shift])
            }
        }

        Settings {
            SettingsView().environment(delegate.model)
        }
        .windowResizability(.contentSize)

        MenuBarExtra {
            MenuBarContent().environment(delegate.model)
        } label: {
            MenuBarLabel().environment(delegate.model)
        }
    }
}

@MainActor
final class AppDelegate: NSObject, NSApplicationDelegate {
    let model = AppModel()
    private var panel: DetectionPanelController?

    func applicationDidFinishLaunching(_ notification: Notification) {
        let panel = DetectionPanelController(model: model)
        self.panel = panel
        model.onBannerChange = { [weak panel] in panel?.update($0) }
        model.start()
    }

    func applicationDidBecomeActive(_ notification: Notification) {
        model.refreshPermissions()
    }

    /// Minutes keeps watching for calls from the menu bar after its window is closed.
    func applicationShouldTerminateAfterLastWindowClosed(_ sender: NSApplication) -> Bool {
        false
    }

    func applicationShouldTerminate(_ sender: NSApplication) -> NSApplication.TerminateReply {
        guard model.isRecording else { return .terminateNow }
        let alert = NSAlert()
        alert.messageText = "A meeting is being recorded"
        alert.informativeText = "Quitting now stops the recording without writing notes. The transcript so far is kept."
        alert.addButton(withTitle: "Keep recording")
        alert.addButton(withTitle: "Quit")
        return alert.runModal() == .alertSecondButtonReturn ? .terminateNow : .terminateCancel
    }
}

private struct MenuBarLabel: View {
    @Environment(AppModel.self) private var model

    var body: some View {
        // TimelineView in a MenuBarExtra label can continuously invalidate the status
        // item on macOS, starving the main run loop. Keep the clock in LiveView.
        Image(systemName: model.isRecording ? "record.circle.fill" : "text.quote")
            .accessibilityLabel(model.isRecording ? "Minutes recording" : "Minutes")
    }
}

private struct MenuBarContent: View {
    @Environment(AppModel.self) private var model
    @Environment(\.openWindow) private var openWindow

    var body: some View {
        if model.isRecording {
            Button("Stop and write notes") { model.stopRecording() }
        } else {
            Button("Record meeting") { model.startRecording(app: .manual) }
        }
        Divider()
        Button("Open Minutes") {
            openWindow(id: "main")
            NSApp.activate(ignoringOtherApps: true)
        }
        SettingsLink { Text("Settings…") }
        Divider()
        Button("Quit Minutes") { NSApp.terminate(nil) }
    }
}
