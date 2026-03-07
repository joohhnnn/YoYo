use std::path::PathBuf;

/// Capture the main screen using macOS screencapture command.
/// Returns the path to the saved PNG file.
/// Uses a fixed path in the system temp dir to avoid leaking temp files.
pub fn capture_screen() -> Result<PathBuf, String> {
    let screenshot_path = std::env::temp_dir().join("yoyo-screenshot.png");
    let path_str = screenshot_path.to_str().ok_or("Invalid path")?.to_string();

    let output = std::process::Command::new("screencapture")
        .args(["-x", "-C", &path_str])
        .output()
        .map_err(|e| format!("Failed to run screencapture: {}", e))?;

    if !output.status.success() {
        return Err(format!(
            "screencapture failed: {}",
            String::from_utf8_lossy(&output.stderr)
        ));
    }

    Ok(screenshot_path)
}
