use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub struct WindowInfo {
    pub app: String,
    pub title: String,
    pub bundle_id: String,
}

/// Get all visible windows using the compiled Swift helper.
/// Returns a list of window entries with app name, title, and bundle ID.
pub fn get_visible_windows() -> Result<Vec<WindowInfo>, String> {
    let binary = env!("YOYO_WINDOWS_BINARY");

    let output = std::process::Command::new(binary)
        .output()
        .map_err(|e| format!("Failed to run window list helper: {}", e))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("Window list helper failed: {}", stderr));
    }

    let json_str = String::from_utf8_lossy(&output.stdout);
    let windows: Vec<WindowInfo> = serde_json::from_str(&json_str)
        .map_err(|e| format!("Failed to parse window list: {}. Raw: {}", e, json_str))?;

    Ok(windows)
}

/// Format window list as text for AI prompt injection.
pub fn format_for_prompt(windows: &[WindowInfo]) -> String {
    windows
        .iter()
        .map(|w| {
            if w.title.is_empty() {
                format!("- {} ({})", w.app, w.bundle_id)
            } else {
                format!("- {}: \"{}\" ({})", w.app, w.title, w.bundle_id)
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
}
