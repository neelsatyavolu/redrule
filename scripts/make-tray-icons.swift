// Renders the menu bar icons from SF Symbols. Usage: swift scripts/make-tray-icons.swift <output folder>
import AppKit

let output = URL(fileURLWithPath: CommandLine.arguments.dropFirst().first ?? "src-tauri/icons")
let points: CGFloat = 18, scale: CGFloat = 2

func render(_ symbol: String, color: NSColor, to name: String) throws {
    let configuration = NSImage.SymbolConfiguration(pointSize: 15, weight: .medium)
        .applying(.init(paletteColors: [color]))
    guard let image = NSImage(systemSymbolName: symbol, accessibilityDescription: nil)?.withSymbolConfiguration(configuration) else {
        throw NSError(domain: "tray", code: 1, userInfo: [NSLocalizedDescriptionKey: "Missing symbol \(symbol)"])
    }
    let pixels = Int(points * scale)
    let bitmap = NSBitmapImageRep(bitmapDataPlanes: nil, pixelsWide: pixels, pixelsHigh: pixels, bitsPerSample: 8,
                                  samplesPerPixel: 4, hasAlpha: true, isPlanar: false, colorSpaceName: .deviceRGB,
                                  bytesPerRow: 0, bitsPerPixel: 0)!
    bitmap.size = NSSize(width: points, height: points)
    NSGraphicsContext.saveGraphicsState()
    NSGraphicsContext.current = NSGraphicsContext(bitmapImageRep: bitmap)
    let size = image.size
    image.draw(in: NSRect(x: (points - size.width) / 2, y: (points - size.height) / 2, width: size.width, height: size.height))
    NSGraphicsContext.restoreGraphicsState()
    try bitmap.representation(using: .png, properties: [:])!.write(to: output.appendingPathComponent(name))
}

// The idle icon is a template: macOS tints it for light and dark menu bars.
try render("text.quote", color: .black, to: "tray.png")
try render("record.circle.fill", color: NSColor(srgbRed: 0.86, green: 0.2, blue: 0.16, alpha: 1), to: "tray-recording.png")
