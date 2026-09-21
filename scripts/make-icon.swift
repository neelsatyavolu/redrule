// Draws the app icon into an .iconset folder: a steno pad where the top line is still sound
// (a waveform) and the lines under it have become writing. The red margin rule is the brand mark.
// Usage: swift scripts/make-icon.swift <output.iconset>
import AppKit

let output = URL(fileURLWithPath: CommandLine.arguments[1])
try FileManager.default.createDirectory(at: output, withIntermediateDirectories: true)

func color(_ hex: UInt32, _ alpha: CGFloat = 1) -> NSColor {
    NSColor(srgbRed: CGFloat((hex >> 16) & 0xFF) / 255, green: CGFloat((hex >> 8) & 0xFF) / 255, blue: CGFloat(hex & 0xFF) / 255, alpha: alpha)
}

func pill(_ rect: NSRect) -> NSBezierPath {
    let radius = min(rect.width, rect.height) / 2
    return NSBezierPath(roundedRect: rect, xRadius: radius, yRadius: radius)
}

func render(_ pixels: Int) -> Data {
    let rep = NSBitmapImageRep(
        bitmapDataPlanes: nil, pixelsWide: pixels, pixelsHigh: pixels, bitsPerSample: 8, samplesPerPixel: 4,
        hasAlpha: true, isPlanar: false, colorSpaceName: .deviceRGB, bytesPerRow: 0, bitsPerPixel: 0
    )!
    NSGraphicsContext.saveGraphicsState()
    NSGraphicsContext.current = NSGraphicsContext(bitmapImageRep: rep)
    let s = CGFloat(pixels) / 1024
    func r(_ x: CGFloat, _ y: CGFloat, _ w: CGFloat, _ h: CGFloat) -> NSRect { NSRect(x: x * s, y: y * s, width: w * s, height: h * s) }

    // Pad body on the macOS icon grid (824 pt square, continuous-looking corners), with a soft drop shadow.
    let body = r(100, 100, 824, 824)
    let shape = NSBezierPath(roundedRect: body, xRadius: 186 * s, yRadius: 186 * s)
    NSGraphicsContext.saveGraphicsState()
    let shadow = NSShadow()
    shadow.shadowColor = color(0x0B140F, 0.35)
    shadow.shadowBlurRadius = 28 * s
    shadow.shadowOffset = NSSize(width: 0, height: -12 * s)
    shadow.set()
    color(0xF1F5EC).setFill()
    shape.fill()
    NSGraphicsContext.restoreGraphicsState()

    NSGraphicsContext.saveGraphicsState()
    shape.addClip()
    NSGradient(colors: [color(0xFAFCF7), color(0xE4ECDD)])!.draw(in: body, angle: -90)

    // Faint ruled lines across the page.
    color(0x9DB39F, 0.35).setFill()
    for index in 0..<5 {
        r(100, 250 + CGFloat(index) * 112, 824, 3).fill()
    }

    // The red margin rule.
    color(0xC8372D).setFill()
    r(300, 100, 14, 824).fill()

    // Binding strip with spiral rings.
    NSGradient(colors: [color(0x22322A), color(0x121C16)])!.draw(in: r(100, 776, 824, 148), angle: -90)
    for index in 0..<7 {
        let x = 196 + CGFloat(index) * 105
        color(0x0A100C).setFill()
        NSBezierPath(ovalIn: r(x - 17, 772, 34, 34)).fill()
        NSGradient(colors: [color(0xF4F7F1), color(0xAEBBAF)])!.draw(in: pill(r(x - 9, 786, 18, 104)), angle: 0)
    }

    // Line one is still sound: a waveform in the recording red.
    let bars: [CGFloat] = [34, 74, 118, 62, 136, 92, 48, 108, 70, 30]
    color(0xC8372D).setFill()
    for (index, height) in bars.enumerated() {
        pill(r(372 + CGFloat(index) * 46, 606 - height / 2, 24, height)).fill()
    }

    // The lines under it have become writing.
    color(0x16211B, 0.88).setFill()
    let lines: [(y: CGFloat, width: CGFloat)] = [(452, 440), (340, 380), (228, 250)]
    for line in lines {
        pill(r(372, line.y, line.width, 36)).fill()
    }

    // Top sheen so the paper reads as a surface.
    NSGradient(colors: [color(0xFFFFFF, 0.0), color(0xFFFFFF, 0.10)])!.draw(in: r(100, 512, 824, 412), angle: 90)
    NSGraphicsContext.restoreGraphicsState()

    // Hairline edge.
    color(0x0B140F, 0.18).setStroke()
    shape.lineWidth = max(1, 2 * s)
    shape.stroke()

    NSGraphicsContext.restoreGraphicsState()
    return rep.representation(using: .png, properties: [:])!
}

for size in [16, 32, 128, 256, 512] {
    try render(size).write(to: output.appendingPathComponent("icon_\(size)x\(size).png"))
    try render(size * 2).write(to: output.appendingPathComponent("icon_\(size)x\(size)@2x.png"))
}
