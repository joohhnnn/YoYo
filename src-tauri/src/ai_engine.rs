use crate::user_data;
use base64::Engine;
use serde::{Deserialize, Serialize};
use std::path::Path;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ActionParams {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub app: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub text: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub command: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct SuggestedAction {
    #[serde(rename = "type")]
    pub action_type: String,
    pub label: String,
    pub params: ActionParams,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct AnalysisResult {
    pub context: String,
    pub actions: Vec<SuggestedAction>,
}

const ANALYSIS_PROMPT: &str = r#"You are YoYo, a desktop workflow assistant. Based on the screenshot, determine:

1. What is the user currently doing? (1 sentence)
2. Suggest 2-4 most likely next actions the user might want to take.

Each action must use one of these types:
- open_url: Open a URL (provide the url in params)
- open_app: Switch to an app (provide the app name in params)
- copy_to_clipboard: Copy detected text/data (provide the text in params)
- run_command: Run a terminal command (provide the command in params)
- notify: Set a reminder (provide the message in params)

Respond ONLY with valid JSON, no other text:
{
  "context": "User is ...",
  "actions": [
    {"type": "open_app", "label": "Open Excel", "params": {"app": "Microsoft Excel"}},
    {"type": "open_url", "label": "Open Docs", "params": {"url": "https://example.com"}}
  ]
}"#;

/// Build the full prompt by prepending user profile and context.
fn build_full_prompt() -> String {
    let mut parts = Vec::new();

    if let Ok(profile) = user_data::read_profile() {
        let trimmed = profile.trim();
        if !trimmed.is_empty() {
            parts.push(format!("[User Profile]\n{}", trimmed));
        }
    }

    if let Ok(context) = user_data::read_context() {
        let trimmed = context.trim();
        if !trimmed.is_empty() {
            parts.push(format!("[User Context]\n{}", trimmed));
        }
    }

    parts.push(ANALYSIS_PROMPT.to_string());
    parts.join("\n\n")
}

/// Analyze a screenshot using Claude CLI
pub async fn analyze_with_cli(screenshot_path: &Path) -> Result<AnalysisResult, String> {
    let path_str = screenshot_path
        .to_str()
        .ok_or("Invalid screenshot path")?;

    let full_prompt = build_full_prompt();
    let prompt = format!(
        "Read the screenshot image at '{}' and analyze it.\n\n{}",
        path_str, full_prompt
    );

    let output = tokio::process::Command::new("claude")
        .args([
            "-p", &prompt,
            "--output-format", "text",
            "--max-turns", "2",
        ])
        .output()
        .await
        .map_err(|e| format!("Failed to run claude CLI: {}", e))?;

    if !output.status.success() {
        return Err(format!(
            "Claude CLI failed: {}",
            String::from_utf8_lossy(&output.stderr)
        ));
    }

    let response = String::from_utf8_lossy(&output.stdout).to_string();
    parse_ai_response(&response)
}

/// Analyze a screenshot using Claude API
pub async fn analyze_with_api(
    screenshot_path: &Path,
    api_key: &str,
) -> Result<AnalysisResult, String> {
    let image_data = std::fs::read(screenshot_path)
        .map_err(|e| format!("Failed to read screenshot: {}", e))?;
    let base64_image = base64::engine::general_purpose::STANDARD.encode(&image_data);

    let client = reqwest::Client::new();
    let body = serde_json::json!({
        "model": "claude-sonnet-4-20250514",
        "max_tokens": 1024,
        "messages": [{
            "role": "user",
            "content": [
                {
                    "type": "image",
                    "source": {
                        "type": "base64",
                        "media_type": "image/png",
                        "data": base64_image
                    }
                },
                {
                    "type": "text",
                    "text": build_full_prompt()
                }
            ]
        }]
    });

    let response = client
        .post("https://api.anthropic.com/v1/messages")
        .header("x-api-key", api_key)
        .header("anthropic-version", "2023-06-01")
        .header("content-type", "application/json")
        .json(&body)
        .send()
        .await
        .map_err(|e| format!("API request failed: {}", e))?;

    if !response.status().is_success() {
        let err = response.text().await.unwrap_or_default();
        return Err(format!("API error: {}", err));
    }

    let json: serde_json::Value = response
        .json()
        .await
        .map_err(|e| format!("Failed to parse API response: {}", e))?;

    let text = json["content"][0]["text"]
        .as_str()
        .ok_or("No text in API response")?;

    parse_ai_response(text)
}

/// Parse the AI response text into an AnalysisResult
fn parse_ai_response(response: &str) -> Result<AnalysisResult, String> {
    // Try to find JSON in the response (might be wrapped in markdown code blocks)
    let json_str = if let Some(start) = response.find('{') {
        if let Some(end) = response.rfind('}') {
            &response[start..=end]
        } else {
            response
        }
    } else {
        response
    };

    serde_json::from_str::<AnalysisResult>(json_str)
        .map_err(|e| format!("Failed to parse AI response as JSON: {}. Raw: {}", e, response))
}
