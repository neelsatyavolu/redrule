import AppKit
import SwiftUI

/// Visual language: a stenographer's pad. Eye-ease green paper, green-black ink,
/// and the pad's red margin rule, which doubles as the recording colour.
enum Theme {
    static let pad = Color(light: 0xEFF3EA, dark: 0x151A17)
    static let sheet = Color(light: 0xF8FAF5, dark: 0x1B211D)
    static let ink = Color(light: 0x16211B, dark: 0xE3EAE2)
    static let pencil = Color(light: 0x5E6B62, dark: 0x93A096)
    static let rule = Color(light: 0xD5DFD2, dark: 0x2B342E)
    static let margin = Color(light: 0xC8372D, dark: 0xEF6A5E)

    static let gutter: CGFloat = 72
    static let gutterGap: CGFloat = 64
    static let measure: CGFloat = 600
    static let pageInset: CGFloat = 32

    static func serif(_ size: CGFloat, _ weight: Font.Weight = .regular) -> Font {
        .system(size: size, weight: weight, design: .serif)
    }
}

extension Color {
    init(light: UInt32, dark: UInt32) {
        self.init(nsColor: NSColor(name: nil) { appearance in
            let isDark = appearance.bestMatch(from: [.darkAqua, .aqua]) == .darkAqua
            return NSColor(hex: isDark ? dark : light)
        })
    }
}

private extension NSColor {
    convenience init(hex: UInt32) {
        self.init(
            srgbRed: CGFloat((hex >> 16) & 0xFF) / 255, green: CGFloat((hex >> 8) & 0xFF) / 255,
            blue: CGFloat(hex & 0xFF) / 255, alpha: 1
        )
    }
}

/// A scrolling page with the pad's red margin rule. Rows inside use `PadRow` to hang labels in the margin.
private extension EnvironmentValues {
    @Entry var compactPad = false
}

extension EnvironmentValues {
    @Entry var showsPadRule = true
}

struct PadPage<Content: View>: View {
    @Environment(\.showsPadRule) private var showsRule
    @ViewBuilder var content: Content

    var body: some View {
        GeometryReader { geometry in
            let compact = geometry.size.width < 540
            ScrollView {
                VStack(alignment: .leading, spacing: 0) { content }
                    .padding(.vertical, 48)
                    .padding(.horizontal, Theme.pageInset)
                    .frame(maxWidth: .infinity, alignment: .leading)
                    .environment(\.compactPad, compact)
            }
            .defaultScrollAnchor(.top)
            .frame(maxWidth: Theme.pageInset * 2 + Theme.gutter + Theme.gutterGap + Theme.measure)
            .background(alignment: .leading) {
                if !compact && showsRule {
                    Rectangle().fill(Theme.margin.opacity(0.35)).frame(width: 1)
                        .padding(.leading, Theme.pageInset + Theme.gutter + Theme.gutterGap / 2)
                }
            }
            .frame(maxWidth: .infinity, maxHeight: .infinity)
        }
        .background(Theme.sheet)
    }
}

/// One line of the pad: a right-aligned label in the margin and content on the text side of the rule.
struct PadRow<Label: View, Content: View>: View {
    @Environment(\.compactPad) private var compact
    var spacing: CGFloat = 0
    @ViewBuilder var label: Label
    @ViewBuilder var content: Content

    var body: some View {
        let layout = compact ? AnyLayout(VStackLayout(alignment: .leading, spacing: 6))
                             : AnyLayout(HStackLayout(alignment: .firstTextBaseline, spacing: Theme.gutterGap))
        layout {
            label
                .font(.system(size: 11.5))
                .foregroundStyle(Theme.pencil)
                .monospacedDigit()
                .lineLimit(2)
                .multilineTextAlignment(compact ? .leading : .trailing)
                .frame(width: compact ? nil : Theme.gutter, alignment: compact ? .leading : .trailing)
            content.frame(maxWidth: Theme.measure, alignment: .leading)
        }
        .padding(.top, spacing)
    }
}

extension PadRow where Label == Text {
    init(spacing: CGFloat = 0, @ViewBuilder content: () -> Content) {
        self.init(spacing: spacing, label: { Text("") }, content: content)
    }
}

/// The recording indicator. Pulses unless Reduce Motion is on.
struct RecordingDot: View {
    var size: CGFloat = 9
    @Environment(\.accessibilityReduceMotion) private var reduceMotion
    @State private var dimmed = false

    var body: some View {
        Circle()
            .fill(Theme.margin)
            .frame(width: size, height: size)
            // Scope the pulse to opacity so it cannot animate the surrounding page's layout.
            .animation(reduceMotion ? nil : .easeInOut(duration: 0.9).repeatForever(autoreverses: true)) { dot in
                dot.opacity(dimmed ? 0.35 : 1)
            }
            .onAppear {
                guard !reduceMotion else { return }
                dimmed = true
            }
            .accessibilityLabel("Recording")
    }
}

struct PrimaryButtonStyle: ButtonStyle {
    var tint: Color = Theme.ink

    func makeBody(configuration: Configuration) -> some View {
        configuration.label
            .font(.system(size: 13, weight: .medium))
            .foregroundStyle(Theme.sheet)
            .padding(.horizontal, 14)
            .padding(.vertical, 7)
            .background(tint.opacity(configuration.isPressed ? 0.8 : 1), in: RoundedRectangle(cornerRadius: 7, style: .continuous))
    }
}

struct QuietButtonStyle: ButtonStyle {
    func makeBody(configuration: Configuration) -> some View {
        configuration.label
            .font(.system(size: 13, weight: .medium))
            .foregroundStyle(Theme.ink)
            .padding(.horizontal, 14)
            .padding(.vertical, 7)
            .background(Theme.rule.opacity(configuration.isPressed ? 1 : 0.6), in: RoundedRectangle(cornerRadius: 7, style: .continuous))
    }
}

enum Format {
    static func clock(_ seconds: TimeInterval) -> String {
        let total = max(0, Int(seconds))
        let (h, m, s) = (total / 3600, (total % 3600) / 60, total % 60)
        return h > 0 ? String(format: "%d:%02d:%02d", h, m, s) : String(format: "%02d:%02d", m, s)
    }

    static func length(_ seconds: TimeInterval) -> String {
        let minutes = Int((seconds / 60).rounded())
        if minutes < 1 { return "under a minute" }
        if minutes < 60 { return minutes == 1 ? "1 minute" : "\(minutes) minutes" }
        let (h, m) = (minutes / 60, minutes % 60)
        return m == 0 ? "\(h) hr" : "\(h) hr \(m) min"
    }
}
