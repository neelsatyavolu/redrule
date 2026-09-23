import AppKit
import MinutesCore
import SwiftUI

/// The floating prompt in the top-right corner. It never takes focus away from the call.
@MainActor
final class DetectionPanelController {
    private static let size = CGSize(width: 340, height: 118)
    private static let autoDismiss: Duration = .seconds(45)

    private let model: AppModel
    private var panel: NSPanel?
    private var dismissTask: Task<Void, Never>?

    init(model: AppModel) {
        self.model = model
    }

    func update(_ banner: MeetingBanner?) {
        dismissTask?.cancel()
        guard let banner else {
            hide()
            return
        }
        show(banner)
        // An unanswered "meeting detected" prompt should not sit on screen for the whole call.
        if case .detected = banner {
            dismissTask = Task { [weak self] in
                try? await Task.sleep(for: Self.autoDismiss)
                if !Task.isCancelled { self?.model.banner = nil }
            }
        }
    }

    private func show(_ banner: MeetingBanner) {
        let panel = panel ?? makePanel()
        self.panel = panel
        panel.contentView = NSHostingView(rootView: DetectionBannerView(banner: banner).environment(model))

        let screen = NSScreen.main?.visibleFrame ?? .zero
        let target = CGRect(x: screen.maxX - Self.size.width - 16, y: screen.maxY - Self.size.height - 16, width: Self.size.width, height: Self.size.height)
        guard !panel.isVisible else {
            panel.setFrame(target, display: true)
            return
        }
        let reduceMotion = NSWorkspace.shared.accessibilityDisplayShouldReduceMotion
        panel.setFrame(reduceMotion ? target : target.offsetBy(dx: 0, dy: 14), display: false)
        panel.alphaValue = 0
        panel.orderFrontRegardless()
        NSAnimationContext.runAnimationGroup { context in
            context.duration = 0.28
            context.timingFunction = CAMediaTimingFunction(name: .easeOut)
            panel.animator().alphaValue = 1
            panel.animator().setFrame(target, display: true)
        }
    }

    private func hide() {
        guard let panel, panel.isVisible else { return }
        NSAnimationContext.runAnimationGroup { context in
            context.duration = 0.18
            panel.animator().alphaValue = 0
        } completionHandler: {
            MainActor.assumeIsolated { panel.orderOut(nil) }
        }
    }

    private func makePanel() -> NSPanel {
        let panel = NSPanel(contentRect: CGRect(origin: .zero, size: Self.size), styleMask: [.borderless, .nonactivatingPanel], backing: .buffered, defer: false)
        panel.level = .statusBar
        panel.collectionBehavior = [.canJoinAllSpaces, .fullScreenAuxiliary, .stationary]
        panel.isOpaque = false
        panel.backgroundColor = .clear
        panel.hasShadow = true
        panel.hidesOnDeactivate = false
        panel.isMovableByWindowBackground = true
        return panel
    }
}

private struct DetectionBannerView: View {
    @Environment(AppModel.self) private var model
    let banner: MeetingBanner

    var body: some View {
        HStack(alignment: .top, spacing: 14) {
            // The pad's margin rule, in miniature.
            Rectangle().fill(Theme.margin).frame(width: 2).padding(.vertical, 2)

            VStack(alignment: .leading, spacing: 4) {
                Text(title).font(Theme.serif(17, .semibold)).foregroundStyle(Theme.ink)
                Text(detail).font(.system(size: 12)).foregroundStyle(Theme.pencil).lineLimit(2)
                Spacer(minLength: 6)
                HStack(spacing: 8) {
                    Button(primaryTitle, action: primary).buttonStyle(PrimaryButtonStyle(tint: Theme.margin))
                    Button(secondaryTitle) { model.banner = nil }.buttonStyle(QuietButtonStyle())
                }
            }
            Spacer(minLength: 0)
        }
        .padding(16)
        .frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .topLeading)
        .background(Theme.sheet, in: RoundedRectangle(cornerRadius: 14, style: .continuous))
        .overlay(RoundedRectangle(cornerRadius: 14, style: .continuous).strokeBorder(Theme.rule))
    }

    private var title: String {
        switch banner {
        case .detected(let app): "You're in a \(app.displayName) call"
        case .ended: "The call looks finished"
        }
    }

    private var detail: String {
        switch banner {
        case .detected: "Redrule can record it and write up the notes."
        case .ended: "Stop recording and write the notes now?"
        }
    }

    private var primaryTitle: String {
        if case .detected = banner { "Record" } else { "Stop and write notes" }
    }

    private var secondaryTitle: String {
        if case .detected = banner { "Not this one" } else { "Keep recording" }
    }

    private func primary() {
        switch banner {
        case .detected(let app): model.startRecording(app: app)
        case .ended: model.stopRecording()
        }
    }
}
