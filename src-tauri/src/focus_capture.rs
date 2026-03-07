use serde::Deserialize;
use std::path::PathBuf;

#[derive(Debug, Deserialize)]
struct FocusCaptureOutput {
    cursor_x: f64,
    cursor_y: f64,
    width: f64,
    height: f64,
    #[serde(default)]
    error: Option<String>,
}

#[allow(dead_code)]
pub struct FocusCaptureResult {
    pub cursor_x: f64,
    pub cursor_y: f64,
    pub width: f64,
    pub height: f64,
    pub image_path: PathBuf,
}

/// Capture an 800x600 region around the mouse cursor using the compiled Swift helper.
/// Returns the cursor position, crop dimensions, and path to the saved PNG.
pub fn capture_focus_area() -> Result<FocusCaptureResult, String> {
    let focus_binary = env!("YOYO_FOCUS_BINARY");
    let output_path = std::env::temp_dir().join("yoyo-focus.png");
    let output_path_str = output_path
        .to_str()
        .ok_or("Invalid temp path for focus capture")?;

    let output = std::process::Command::new(focus_binary)
        .arg(output_path_str)
        .output()
        .map_err(|e| format!("Failed to run focus capture helper: {}", e))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("Focus capture helper failed: {}", stderr));
    }

    let json_str = String::from_utf8_lossy(&output.stdout);
    let parsed: FocusCaptureOutput = serde_json::from_str(&json_str).map_err(|e| {
        format!(
            "Failed to parse focus capture output: {}. Raw: {}",
            e, json_str
        )
    })?;

    if let Some(ref err) = parsed.error {
        return Err(format!("Focus capture error: {}", err));
    }

    Ok(FocusCaptureResult {
        cursor_x: parsed.cursor_x,
        cursor_y: parsed.cursor_y,
        width: parsed.width,
        height: parsed.height,
        image_path: output_path,
    })
}
