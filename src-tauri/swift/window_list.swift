import Foundation
import AppKit
import CoreGraphics

// Usage: yoyo-windows
// Lists all visible windows with app name, window title, and bundle ID.
// Outputs JSON array to stdout.

struct WindowEntry: Encodable {
    let app: String
    let title: String
    let bundle_id: String
}

guard let windowList = CGWindowListCopyWindowInfo(.optionOnScreenOnly, kCGNullWindowID) as? [[String: Any]] else {
    print("[]")
    exit(0)
}

var entries: [WindowEntry] = []
var seen = Set<String>() // dedupe key: "bundle_id|title"

for window in windowList {
    // Only include normal windows (layer 0)
    guard let layer = window[kCGWindowLayer as String] as? Int, layer == 0 else {
        continue
    }

    guard let ownerName = window[kCGWindowOwnerName as String] as? String else {
        continue
    }

    // Skip YoYo itself
    if ownerName == "YoYo" || ownerName == "yoyo" {
        continue
    }

    let title = window[kCGWindowName as String] as? String ?? ""
    let pid = window[kCGWindowOwnerPID as String] as? Int ?? 0

    // Get bundle ID from PID via NSRunningApplication
    var bundleId = ""
    if pid > 0, let app = NSRunningApplication(processIdentifier: pid_t(pid)) {
        bundleId = app.bundleIdentifier ?? ""
    }

    // Skip windows without bundle ID (system processes)
    if bundleId.isEmpty {
        continue
    }

    // Dedupe: same bundle_id + title
    let key = "\(bundleId)|\(title)"
    if seen.contains(key) {
        continue
    }
    seen.insert(key)

    entries.append(WindowEntry(app: ownerName, title: title, bundle_id: bundleId))
}

let encoder = JSONEncoder()
if let data = try? encoder.encode(entries),
   let jsonString = String(data: data, encoding: .utf8) {
    print(jsonString)
} else {
    print("[]")
}
