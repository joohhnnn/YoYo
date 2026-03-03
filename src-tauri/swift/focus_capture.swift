import Foundation
import AppKit
import CoreGraphics

// Usage: yoyo-focus <output-path>
// Captures a region around the mouse cursor and saves as PNG.
// Outputs JSON: {"cursor_x": f64, "cursor_y": f64, "width": f64, "height": f64}

let cropWidth: CGFloat = 800
let cropHeight: CGFloat = 600

guard CommandLine.arguments.count > 1 else {
    let err: [String: Any] = ["error": "Usage: yoyo-focus <output-path>"]
    if let data = try? JSONSerialization.data(withJSONObject: err),
       let str = String(data: data, encoding: .utf8) {
        print(str)
    }
    exit(1)
}

let outputPath = CommandLine.arguments[1]

// Get mouse cursor position (screen coordinates, top-left origin for CGEvent)
let mouseLocation = CGEvent(source: nil)?.location ?? CGPoint.zero

// Get main display bounds for clamping
let displayID = CGMainDisplayID()
let screenWidth = CGFloat(CGDisplayPixelsWide(displayID))
let screenHeight = CGFloat(CGDisplayPixelsHigh(displayID))

// On Retina displays, CGDisplayPixelsWide returns physical pixels.
// CGEvent locations and CGWindowListCreateImage rects use points.
// We need the display bounds in points.
let screenBounds = CGDisplayBounds(displayID)
let pointWidth = screenBounds.width
let pointHeight = screenBounds.height

// Calculate crop rect centered on cursor, clamped to screen bounds
var cropX = mouseLocation.x - cropWidth / 2
var cropY = mouseLocation.y - cropHeight / 2

// Clamp to screen edges
cropX = max(0, min(cropX, pointWidth - cropWidth))
cropY = max(0, min(cropY, pointHeight - cropHeight))

let cropRect = CGRect(x: cropX, y: cropY, width: cropWidth, height: cropHeight)

// Capture the screen region
guard let image = CGWindowListCreateImage(
    cropRect,
    .optionOnScreenOnly,
    kCGNullWindowID,
    .bestResolution
) else {
    let err: [String: Any] = ["error": "Failed to capture screen region"]
    if let data = try? JSONSerialization.data(withJSONObject: err),
       let str = String(data: data, encoding: .utf8) {
        print(str)
    }
    exit(1)
}

// Save as PNG
let url = URL(fileURLWithPath: outputPath)
guard let destination = CGImageDestinationCreateWithURL(url as CFURL, kUTTypePNG, 1, nil) else {
    let err: [String: Any] = ["error": "Failed to create image destination"]
    if let data = try? JSONSerialization.data(withJSONObject: err),
       let str = String(data: data, encoding: .utf8) {
        print(str)
    }
    exit(1)
}
CGImageDestinationAddImage(destination, image, nil)
guard CGImageDestinationFinalize(destination) else {
    let err: [String: Any] = ["error": "Failed to write PNG"]
    if let data = try? JSONSerialization.data(withJSONObject: err),
       let str = String(data: data, encoding: .utf8) {
        print(str)
    }
    exit(1)
}

// Output result as JSON
let result: [String: Any] = [
    "cursor_x": mouseLocation.x,
    "cursor_y": mouseLocation.y,
    "width": cropWidth,
    "height": cropHeight
]

if let jsonData = try? JSONSerialization.data(withJSONObject: result, options: []),
   let jsonString = String(data: jsonData, encoding: .utf8) {
    print(jsonString)
} else {
    print("{\"error\":\"JSON serialization failed\"}")
    exit(1)
}
