import Foundation
import AppKit
import ApplicationServices

// Usage: yoyo-ax <pid>
// Extracts text from the accessibility tree of the given process.
// Outputs JSON to stdout.

struct AXResult: Encodable {
    let text: String
    let app_name: String
    let window_title: String
    let node_count: Int
    let truncated: Bool
    let error: String?
}

// Privacy: password manager bundle IDs to skip entirely
let blockedBundleIds: Set<String> = [
    "com.1password.1password",
    "com.agilebits.onepassword7",
    "com.bitwarden.desktop",
    "org.keepassxc.keepassxc",
    "com.lastpass.LastPass",
    "com.dashlane.Dashlane",
    "com.apple.keychainaccess",
]

// Privacy: window title keywords that indicate sensitive content
let sensitiveKeywords = ["password", "private", "incognito", "keychain", "credential", "secret"]

// Roles to extract text from
let textRoles: Set<String> = [
    "AXStaticText", "AXTextField", "AXTextArea",
    "AXButton", "AXMenuItem", "AXCell",
    "AXHeading", "AXLink", "AXTab",
    "AXValue", "AXTitle",
]

// Roles to skip entirely (no children traversal)
let skipRoles: Set<String> = [
    "AXScrollBar", "AXImage", "AXSplitter",
    "AXMenuBar", "AXToolbar", "AXSecureTextField",
    "AXProgressIndicator", "AXMenu",
]

// Limits (inspired by Screenpipe)
let maxDepth = 30
let maxNodes = 5000
let walkTimeoutSecs: Double = 0.25  // 250ms total walk timeout
let elementTimeoutSecs: Double = 0.2

// MARK: - Helpers

func getStringAttribute(_ element: AXUIElement, _ attribute: String) -> String? {
    var value: AnyObject?
    let cfAttr = attribute as CFString
    let result = AXUIElementCopyAttributeValue(element, cfAttr, &value)
    guard result == .success, let str = value as? String else { return nil }
    return str
}

func getChildren(_ element: AXUIElement) -> [AXUIElement] {
    var value: AnyObject?
    let result = AXUIElementCopyAttributeValue(element, kAXChildrenAttribute as CFString, &value)
    guard result == .success, let children = value as? [AXUIElement] else { return [] }
    return children
}

func getFocusedWindow(_ appElement: AXUIElement) -> AXUIElement? {
    var value: AnyObject?
    let result = AXUIElementCopyAttributeValue(appElement, kAXFocusedWindowAttribute as CFString, &value)
    guard result == .success else { return nil }
    // AXUIElement is a CFTypeRef, need to cast properly
    return (value as! AXUIElement)
}

func getWindows(_ appElement: AXUIElement) -> [AXUIElement] {
    var value: AnyObject?
    let result = AXUIElementCopyAttributeValue(appElement, kAXWindowsAttribute as CFString, &value)
    guard result == .success, let windows = value as? [AXUIElement] else { return [] }
    return windows
}

// Enable enhanced UI for Chromium-based apps (Chrome, VSCode, Electron)
func enableEnhancedUI(_ appElement: AXUIElement) {
    let attr = "AXEnhancedUserInterface" as CFString
    AXUIElementSetAttributeValue(appElement, attr, true as CFTypeRef)
}

// MARK: - Tree Walker

var collectedTexts: [String] = []
var nodeCount = 0
var truncated = false
var walkStart: Date = Date()

func walkTree(_ element: AXUIElement, depth: Int) {
    // Check limits
    if depth > maxDepth || nodeCount >= maxNodes {
        truncated = true
        return
    }

    // Check timeout
    if Date().timeIntervalSince(walkStart) > walkTimeoutSecs {
        truncated = true
        return
    }

    nodeCount += 1

    // Get role
    let role = getStringAttribute(element, kAXRoleAttribute as String) ?? ""

    // Skip blocked roles entirely
    if skipRoles.contains(role) {
        return
    }

    // Extract text from text-bearing roles
    if textRoles.contains(role) {
        // Try AXValue first (text fields, text areas)
        if let value = getStringAttribute(element, kAXValueAttribute as String),
           !value.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty {
            collectedTexts.append(value.trimmingCharacters(in: .whitespacesAndNewlines))
        }
        // Try AXTitle (buttons, menu items, tabs)
        else if let title = getStringAttribute(element, kAXTitleAttribute as String),
                !title.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty {
            collectedTexts.append(title.trimmingCharacters(in: .whitespacesAndNewlines))
        }
        // Try AXDescription as fallback
        else if let desc = getStringAttribute(element, kAXDescriptionAttribute as String),
                !desc.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty {
            collectedTexts.append(desc.trimmingCharacters(in: .whitespacesAndNewlines))
        }
    }

    // Recurse into children
    let children = getChildren(element)
    for child in children {
        if truncated { return }
        walkTree(child, depth: depth + 1)
    }
}

// MARK: - Main

guard CommandLine.arguments.count >= 2,
      let pid = Int32(CommandLine.arguments[1]) else {
    let result = AXResult(text: "", app_name: "", window_title: "", node_count: 0, truncated: false, error: "Usage: yoyo-ax <pid>")
    let encoder = JSONEncoder()
    if let data = try? encoder.encode(result), let json = String(data: data, encoding: .utf8) {
        print(json)
    }
    exit(1)
}

// Check accessibility permission
if !AXIsProcessTrusted() {
    let result = AXResult(text: "", app_name: "", window_title: "", node_count: 0, truncated: false, error: "accessibility_not_trusted")
    let encoder = JSONEncoder()
    if let data = try? encoder.encode(result), let json = String(data: data, encoding: .utf8) {
        print(json)
    }
    exit(1)
}

// Get app info
let app = NSRunningApplication(processIdentifier: pid)
let appName = app?.localizedName ?? "Unknown"
let bundleId = app?.bundleIdentifier ?? ""

// Privacy: skip password managers
if blockedBundleIds.contains(bundleId) {
    let result = AXResult(text: "", app_name: appName, window_title: "", node_count: 0, truncated: false, error: "blocked_privacy")
    let encoder = JSONEncoder()
    if let data = try? encoder.encode(result), let json = String(data: data, encoding: .utf8) {
        print(json)
    }
    exit(0)
}

// Create AX application element
let appElement = AXUIElementCreateApplication(pid)

// Enable enhanced UI for Chromium-based apps
let chromiumBundleIds = ["com.google.Chrome", "com.microsoft.VSCode", "com.brave.Browser",
                         "com.vivaldi.Vivaldi", "com.operasoftware.Opera", "com.electron."]
if chromiumBundleIds.contains(where: { bundleId.hasPrefix($0) || bundleId == $0 }) {
    enableEnhancedUI(appElement)
    // Small delay for Chromium to update its AX tree
    usleep(50000) // 50ms
}

// Get focused window (or first window as fallback)
let targetWindow: AXUIElement
var windowTitle = ""

if let focused = getFocusedWindow(appElement) {
    targetWindow = focused
    windowTitle = getStringAttribute(focused, kAXTitleAttribute as String) ?? ""
} else {
    let windows = getWindows(appElement)
    if let first = windows.first {
        targetWindow = first
        windowTitle = getStringAttribute(first, kAXTitleAttribute as String) ?? ""
    } else {
        // No windows found, try walking the app element directly
        targetWindow = appElement
    }
}

// Privacy: check for sensitive window titles
let titleLower = windowTitle.lowercased()
if sensitiveKeywords.contains(where: { titleLower.contains($0) }) {
    let result = AXResult(text: "", app_name: appName, window_title: windowTitle, node_count: 0, truncated: false, error: "blocked_privacy")
    let encoder = JSONEncoder()
    if let data = try? encoder.encode(result), let json = String(data: data, encoding: .utf8) {
        print(json)
    }
    exit(0)
}

// Walk the accessibility tree
walkStart = Date()
walkTree(targetWindow, depth: 0)

// Deduplicate consecutive identical texts
var dedupedTexts: [String] = []
for text in collectedTexts {
    if dedupedTexts.last != text {
        dedupedTexts.append(text)
    }
}

let combinedText = dedupedTexts.joined(separator: "\n")

// Truncate if too long (keep first ~32KB)
let maxTextLength = 32768
let finalText: String
if combinedText.count > maxTextLength {
    finalText = String(combinedText.prefix(maxTextLength))
    truncated = true
} else {
    finalText = combinedText
}

let result = AXResult(
    text: finalText,
    app_name: appName,
    window_title: windowTitle,
    node_count: nodeCount,
    truncated: truncated,
    error: nil
)

let encoder = JSONEncoder()
if let data = try? encoder.encode(result),
   let json = String(data: data, encoding: .utf8) {
    print(json)
} else {
    print("{\"text\":\"\",\"app_name\":\"\",\"window_title\":\"\",\"node_count\":0,\"truncated\":false,\"error\":\"json_encode_failed\"}")
}
