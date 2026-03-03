use crate::commands::ChatMessage;
use crate::user_data::{self, ActivityRecord};
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

/// Build the full prompt with activity history for observation mode.
pub fn build_full_prompt_with_history(
    language: &str,
    recent_activities: &[ActivityRecord],
) -> String {
    let mut parts = Vec::new();

    // Language instruction
    match language {
        "en" => parts.push("Respond in English.".to_string()),
        _ => parts.push(
            "请用中文回复。context 字段和 label 字段都必须使用中文。".to_string(),
        ),
    }

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

    // Inject recent activity history with timestamps and duration
    if !recent_activities.is_empty() {
        let now = chrono::Local::now().naive_local();
        let mut history_lines = vec!["[Recent Activity History]".to_string()];
        for activity in recent_activities {
            let duration = format_activity_duration(&activity.created_at, &activity.updated_at);
            let relative = format_relative_time(&activity.created_at, &now);
            history_lines.push(format!(
                "- {} ({}) [{}] {}{}",
                activity.created_at, relative, activity.app_name, activity.context, duration
            ));
        }
        parts.push(history_lines.join("\n"));
    }

    parts.push(ANALYSIS_PROMPT.to_string());
    parts.push("Consider the user's recent activity history above to provide more contextual and relevant suggestions. If you notice a workflow pattern, suggest the likely next step.".to_string());
    parts.join("\n\n")
}

/// Analyze a screenshot using Claude CLI
pub async fn analyze_with_cli(
    screenshot_path: &Path,
    model: &str,
    language: &str,
    recent_activities: &[ActivityRecord],
) -> Result<AnalysisResult, String> {
    let path_str = screenshot_path
        .to_str()
        .ok_or("Invalid screenshot path")?;

    let full_prompt = build_full_prompt_with_history(language, recent_activities);
    let prompt = format!(
        "Read the screenshot image at '{}' and analyze it.\n\n{}",
        path_str, full_prompt
    );

    let output = tokio::process::Command::new("claude")
        .args([
            "-p", &prompt,
            "--output-format", "text",
            "--max-turns", "2",
            "--model", model,
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
    model: &str,
    language: &str,
    recent_activities: &[ActivityRecord],
) -> Result<AnalysisResult, String> {
    let image_data = std::fs::read(screenshot_path)
        .map_err(|e| format!("Failed to read screenshot: {}", e))?;
    let base64_image = base64::engine::general_purpose::STANDARD.encode(&image_data);

    let client = reqwest::Client::new();
    let body = serde_json::json!({
        "model": model,
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
                    "text": build_full_prompt_with_history(language, recent_activities)
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

// ---------------------------------------------------------------------------
// Onboarding conversation
// ---------------------------------------------------------------------------

const ONBOARDING_SYSTEM_PROMPT: &str = r#"You are YoYo, a friendly AI desktop assistant conducting an onboarding conversation. Your goal is to learn about the user so you can personalize their experience.

Rules:
1. Ask ONE question at a time, keep it concise and friendly.
2. Follow this flow (4-6 turns total):
   - Q1: What's your name and what do you do? (profession/role)
   - Q2: What are your main tools? (IDE, apps, services)
   - Q3: What are you currently working on? (projects/goals)
   - Q4: Any specific workflow habits or preferences?
3. After gathering enough info (4-6 turns), respond with EXACTLY this format:

[PROFILE_COMPLETE]
# YoYo User Profile

## About Me
(structured summary based on conversation)

## Tools I Use
(bulleted list)

## Current Projects
(what they're working on)

## Preferences
(language, habits, etc.)
[/PROFILE_COMPLETE]

4. Be warm but efficient. Each response should be 1-2 sentences max (question only).
5. Do NOT ask all questions at once."#;

/// Onboarding chat using Claude CLI (multi-turn via prompt concatenation).
pub async fn onboarding_chat_cli(
    history: &[ChatMessage],
    model: &str,
    language: &str,
) -> Result<String, String> {
    let lang_instruction = match language {
        "en" => "Respond in English.",
        _ => "请用中文回复。",
    };

    let mut prompt = format!("{}\n\n{}\n\n", lang_instruction, ONBOARDING_SYSTEM_PROMPT);
    for msg in history {
        match msg.role.as_str() {
            "user" => prompt.push_str(&format!("User: {}\n", msg.content)),
            "assistant" => prompt.push_str(&format!("Assistant: {}\n", msg.content)),
            _ => {}
        }
    }
    prompt.push_str("Assistant: ");

    let output = tokio::process::Command::new("claude")
        .args(["-p", &prompt, "--output-format", "text", "--max-turns", "1", "--model", model])
        .output()
        .await
        .map_err(|e| format!("Failed to run claude CLI: {}", e))?;

    if !output.status.success() {
        return Err(format!(
            "Claude CLI failed: {}",
            String::from_utf8_lossy(&output.stderr)
        ));
    }

    Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

/// Onboarding chat using Claude API (native multi-turn messages).
pub async fn onboarding_chat_api(
    history: &[ChatMessage],
    api_key: &str,
    model: &str,
    language: &str,
) -> Result<String, String> {
    let lang_instruction = match language {
        "en" => "Respond in English.",
        _ => "请用中文回复。",
    };
    let system_prompt = format!("{}\n\n{}", lang_instruction, ONBOARDING_SYSTEM_PROMPT);

    let messages: Vec<serde_json::Value> = history
        .iter()
        .map(|msg| {
            serde_json::json!({
                "role": msg.role,
                "content": msg.content
            })
        })
        .collect();

    let client = reqwest::Client::new();
    let body = serde_json::json!({
        "model": model,
        "max_tokens": 512,
        "system": system_prompt,
        "messages": messages
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

    json["content"][0]["text"]
        .as_str()
        .map(|s| s.trim().to_string())
        .ok_or_else(|| "No text in API response".to_string())
}

// ---------------------------------------------------------------------------
// Reflection (long-term memory)
// ---------------------------------------------------------------------------

/// Generate a reflection summary from recent activities using Claude CLI.
pub async fn generate_reflection_cli(
    activities: &[ActivityRecord],
    model: &str,
    language: &str,
) -> Result<String, String> {
    let lang_instruction = match language {
        "en" => "Respond in English.",
        _ => "请用中文回复。",
    };

    let activity_text = format_activities_for_reflection(activities);
    let prompt = format!(
        "{}\n\nYou are analyzing a user's work activity log to identify patterns.\n\nActivities (chronological):\n{}\n\nGenerate a concise summary (3-5 sentences) covering:\n1. Main work themes/projects observed\n2. Tool usage patterns\n3. Workflow patterns\n4. Current focus areas\n\nOutput ONLY the summary text, no JSON, no headers.",
        lang_instruction, activity_text
    );

    let output = tokio::process::Command::new("claude")
        .args(["-p", &prompt, "--output-format", "text", "--max-turns", "1", "--model", model])
        .output()
        .await
        .map_err(|e| format!("Failed to run claude CLI: {}", e))?;

    if !output.status.success() {
        return Err(format!(
            "Claude CLI failed: {}",
            String::from_utf8_lossy(&output.stderr)
        ));
    }

    Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

/// Generate a reflection summary from recent activities using Claude API.
pub async fn generate_reflection_api(
    activities: &[ActivityRecord],
    api_key: &str,
    model: &str,
    language: &str,
) -> Result<String, String> {
    let lang_instruction = match language {
        "en" => "Respond in English.",
        _ => "请用中文回复。",
    };

    let activity_text = format_activities_for_reflection(activities);
    let prompt = format!(
        "You are analyzing a user's work activity log to identify patterns.\n\nActivities (chronological):\n{}\n\nGenerate a concise summary (3-5 sentences) covering:\n1. Main work themes/projects observed\n2. Tool usage patterns\n3. Workflow patterns\n4. Current focus areas\n\nOutput ONLY the summary text, no JSON, no headers.",
        activity_text
    );

    let client = reqwest::Client::new();
    let body = serde_json::json!({
        "model": model,
        "max_tokens": 512,
        "system": lang_instruction,
        "messages": [{
            "role": "user",
            "content": prompt
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

    json["content"][0]["text"]
        .as_str()
        .map(|s| s.trim().to_string())
        .ok_or_else(|| "No text in API response".to_string())
}

fn format_activities_for_reflection(activities: &[ActivityRecord]) -> String {
    activities
        .iter()
        .rev() // chronological order (oldest first)
        .map(|a| {
            let duration = format_activity_duration(&a.created_at, &a.updated_at);
            format!("- {} [{}] {}{}", a.created_at, a.app_name, a.context, duration)
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// Format duration between created_at and updated_at (e.g., " (stayed 5 min)").
fn format_activity_duration(created_at: &str, updated_at: &str) -> String {
    let fmt = "%Y-%m-%d %H:%M:%S";
    let created = chrono::NaiveDateTime::parse_from_str(created_at, fmt);
    let updated = chrono::NaiveDateTime::parse_from_str(updated_at, fmt);
    match (created, updated) {
        (Ok(c), Ok(u)) => {
            let secs = (u - c).num_seconds();
            if secs < 30 {
                String::new()
            } else if secs < 60 {
                format!(" ({}s)", secs)
            } else {
                format!(" ({}min)", secs / 60)
            }
        }
        _ => String::new(),
    }
}

/// Format relative time from a timestamp to now (e.g., "2 min ago").
fn format_relative_time(timestamp: &str, now: &chrono::NaiveDateTime) -> String {
    let fmt = "%Y-%m-%d %H:%M:%S";
    match chrono::NaiveDateTime::parse_from_str(timestamp, fmt) {
        Ok(t) => {
            let secs = (*now - t).num_seconds();
            if secs < 0 {
                "just now".to_string()
            } else if secs < 60 {
                "just now".to_string()
            } else if secs < 3600 {
                format!("{} min ago", secs / 60)
            } else if secs < 86400 {
                format!("{} hr ago", secs / 3600)
            } else {
                format!("{} days ago", secs / 86400)
            }
        }
        Err(_) => "unknown".to_string(),
    }
}
