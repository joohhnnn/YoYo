use serde::Deserialize;
use std::path::Path;

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
pub struct OcrResult {
    pub text: String,
    pub block_count: usize,
    #[serde(default)]
    pub error: Option<String>,
}

/// Run Apple Vision OCR on a screenshot image.
/// Returns extracted text and block count.
pub fn recognize_text(image_path: &Path) -> Result<OcrResult, String> {
    let ocr_binary = env!("YOYO_OCR_BINARY");

    let output = std::process::Command::new(ocr_binary)
        .arg(image_path.to_str().ok_or("Invalid image path")?)
        .output()
        .map_err(|e| format!("Failed to run OCR helper: {}", e))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("OCR helper failed: {}", stderr));
    }

    let json_str = String::from_utf8_lossy(&output.stdout);
    let result: OcrResult = serde_json::from_str(&json_str)
        .map_err(|e| format!("Failed to parse OCR output: {}. Raw: {}", e, json_str))?;

    if let Some(ref err) = result.error {
        return Err(format!("OCR error: {}", err));
    }

    Ok(result)
}
