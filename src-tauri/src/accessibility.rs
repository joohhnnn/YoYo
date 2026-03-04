use serde::Deserialize;

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
pub struct AccessibilityResult {
    pub text: String,
    pub app_name: String,
    pub window_title: String,
    pub node_count: usize,
    pub truncated: bool,
    pub error: Option<String>,
}

/// Extract text from the accessibility tree of the given process.
/// Returns the extracted text, or an error if AX is not available.
pub fn extract_text(pid: i32) -> Result<AccessibilityResult, String> {
    let binary = env!("YOYO_AX_BINARY");

    let output = std::process::Command::new(binary)
        .arg(pid.to_string())
        .output()
        .map_err(|e| format!("Failed to run AX helper: {}", e))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        // Try parsing stdout for structured error (helper outputs JSON even on exit(1))
        let stdout = String::from_utf8_lossy(&output.stdout);
        if let Ok(result) = serde_json::from_str::<AccessibilityResult>(&stdout) {
            if let Some(ref err) = result.error {
                return Err(err.clone());
            }
        }
        return Err(format!("AX helper failed: {}", stderr));
    }

    let json_str = String::from_utf8_lossy(&output.stdout);
    let result: AccessibilityResult = serde_json::from_str(&json_str)
        .map_err(|e| format!("Failed to parse AX output: {}. Raw: {}", e, json_str))?;

    // Check for privacy blocks or errors
    if let Some(ref err) = result.error {
        if err == "blocked_privacy" {
            return Err("blocked_privacy".to_string());
        }
        return Err(err.clone());
    }

    Ok(result)
}
