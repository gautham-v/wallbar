// The app icon's 1024pt master, drawn with AppKit: the same thin picture
// frame the menu bar shows, on the warm off-white tile the sibling apps use.
// Run via scripts/make-icon.sh, which folds the sizes into assets/AppIcon.icns.
//
//   swift scripts/make-icon.swift out.png

import AppKit

let S: CGFloat = 1024
let inset: CGFloat = 100
let bodyR: CGFloat = 185
let ink = NSColor(srgbRed: 0.12, green: 0.12, blue: 0.11, alpha: 1)

func rounded(_ r: NSRect, _ rad: CGFloat) -> NSBezierPath {
    NSBezierPath(roundedRect: r, xRadius: rad, yRadius: rad)
}

let rep = NSBitmapImageRep(bitmapDataPlanes: nil, pixelsWide: Int(S), pixelsHigh: Int(S),
    bitsPerSample: 8, samplesPerPixel: 4, hasAlpha: true, isPlanar: false,
    colorSpaceName: .calibratedRGB, bytesPerRow: 0, bitsPerPixel: 0)!
NSGraphicsContext.saveGraphicsState()
NSGraphicsContext.current = NSGraphicsContext(bitmapImageRep: rep)
let body = NSRect(x: inset, y: inset, width: S - 2*inset, height: S - 2*inset)
NSGraphicsContext.current!.cgContext.saveGState()
rounded(body, bodyR).addClip()
NSGradient(colors: [NSColor(srgbRed: 0.976, green: 0.972, blue: 0.960, alpha: 1),
                    NSColor(srgbRed: 0.898, green: 0.890, blue: 0.870, alpha: 1)])!
    .draw(in: body, angle: -90)
NSGraphicsContext.current!.cgContext.restoreGState()
NSColor(srgbRed: 0, green: 0, blue: 0, alpha: 0.10).setStroke()
let edge = rounded(body.insetBy(dx: 1.5, dy: 1.5), bodyR - 1.5); edge.lineWidth = 3; edge.stroke()

// The mark: a frame with a horizon of hills in it, 16-unit grid scaled up.
let u: CGFloat = 38
let ox = body.midX - 8 * u, oy = body.midY - 8 * u
func p(_ x: CGFloat, _ y: CGFloat) -> NSPoint { NSPoint(x: ox + x * u, y: oy + (16 - y) * u) }
let frame = rounded(NSRect(x: ox + 2 * u, y: oy + 2.5 * u, width: 12 * u, height: 11 * u), 1.2 * u)
frame.lineWidth = 1.1 * u
ink.setStroke()
frame.stroke()
let hills = NSBezierPath()
hills.move(to: p(2.6, 11)); hills.line(to: p(6, 7.5)); hills.line(to: p(8.5, 10))
hills.line(to: p(10.5, 8)); hills.line(to: p(13.4, 10.9))
hills.lineWidth = 1.1 * u; hills.lineCapStyle = .round; hills.lineJoinStyle = .round
hills.stroke()
NSGraphicsContext.restoreGraphicsState()
try! rep.representation(using: .png, properties: [:])!.write(to: URL(fileURLWithPath: CommandLine.arguments[1]))
