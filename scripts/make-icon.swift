// Draws the app icon (a steno pad with its red margin rule) into an .iconset folder.
// Usage: swift scripts/make-icon.swift <output.iconset>
import AppKit

let output = URL(fileURLWithPath: CommandLine.arguments[1])
try FileManager.default.createDirectory(at: output, withIntermediateDirectories: true)

func color(_ hex: UInt32) -> NSColor {
    NSColor(srgbRed: CGFloat((hex >> 16) & 0xFF) / 255, green: CGFloat((hex >> 8) & 0xFF) / 255, blue: CGFloat(hex & 0xFF) / 255, alpha: 1)
}

func render(_ pixels: Int) -> Data {
    let rep = NSBitmapImageRep(
        bitmapDataPlanes: nil, pixelsWide: pixels, pixelsHigh: pixels, bitsPerSample: 8, samplesPerPixel: 4,
        hasAlpha: true, isPlanar: false, colorSpaceName: .deviceRGB, bytesPerRow: 0, bitsPerPixel: 0
    )!
    NSGraphicsContext.saveGraphicsState()
    NSGraphicsContext.current = NSGraphicsContext(bitmapImageRep: rep)
    let s = CGFloat(pixels) / 1024

    // Pad body, inset to the macOS icon grid.
    let body = NSRect(x: 100 * s, y: 100 * s, width: 824 * s, height: 824 * s)
    let shape = NSBezierPath(roundedRect: body, xRadius: 185 * s, yRadius: 185 * s)
    NSGradient(starting: color(0xF8FAF5), ending: color(0xE3EADD))!.draw(in: shape, angle: -90)
    shape.addClip()

    // Binding strip across the top.
    color(0x16211B).setFill()
    NSRect(x: body.minX, y: body.maxY - 150 * s, width: body.width, height: 150 * s).fill()

    // Ruled lines of "writing", the last one short.
    color(0x16211B).withAlphaComponent(0.82).setFill()
    let widths: [CGFloat] = [430, 380, 450, 250]
    for (index, width) in widths.enumerated() {
        let y = body.maxY - (290 + CGFloat(index) * 120) * s
        NSBezierPath(roundedRect: NSRect(x: 400 * s, y: y, width: width * s, height: 34 * s), xRadius: 17 * s, yRadius: 17 * s).fill()
    }

    // The red margin rule.
    color(0xC8372D).setFill()
    NSRect(x: 318 * s, y: body.minY, width: 16 * s, height: body.height - 150 * s).fill()

    NSGraphicsContext.restoreGraphicsState()
    return rep.representation(using: .png, properties: [:])!
}

for size in [16, 32, 128, 256, 512] {
    try render(size).write(to: output.appendingPathComponent("icon_\(size)x\(size).png"))
    try render(size * 2).write(to: output.appendingPathComponent("icon_\(size)x\(size)@2x.png"))
}
