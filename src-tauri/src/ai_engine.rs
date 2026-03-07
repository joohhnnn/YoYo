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
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub suggested_quest: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub key_concepts: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub need_full_context: Option<bool>,
}

const ANALYSIS_PROMPT: &str = r#"You are YoYo, a desktop workflow assistant. Based on the screenshot, determine:

1. What is the user currently doing? (1 sentence)
2. Suggest 2-4 most likely next actions the user might want to take.
3. If you detect a clear goal or project the user is working on (e.g., "building a web app", "studying for exam", "writing a report"), suggest it as a main quest. Only suggest when you're confident about the user's goal. Do NOT suggest if the user is just browsing or doing miscellaneous tasks.

Each action must use one of these types:
- open_url: Open a URL (provide the url in params)
- open_app: Switch to an app (provide the bundle_id in params.app, e.g. "com.apple.Safari"). If an [Open Windows] section is provided, use the exact bundle_id listed there.
- copy_to_clipboard: Copy detected text/data (provide the text in params)
- run_command: Run a terminal command (provide the command in params)
- notify: Set a reminder (provide the message in params)

Respond ONLY with valid JSON, no other text:
{
  "context": "User is ...",
  "actions": [
    {"type": "open_app", "label": "Open Excel", "params": {"app": "com.microsoft.Excel"}},
    {"type": "open_url", "label": "Open Docs", "params": {"url": "https://example.com"}}
  ],
  "suggested_quest": "Build the YoYo desktop assistant",
  "need_full_context": false
}

The "suggested_quest" field is OPTIONAL. Only include it when you detect a clear, meaningful goal. Omit the field entirely if no clear goal is detected.
The "need_full_context" field is OPTIONAL. Set to true ONLY if the visible content is clearly truncated at the edges and you need the full screen to analyze properly. Default is false — omit it in most cases."#;

/// Returns the analysis depth instruction to prepend to the prompt.
fn depth_instruction(depth: &str) -> &'static str {
    match depth {
        "casual" => r#"[Analysis Depth: Casual]
Focus ONLY on identifying the active application and the user's general activity category (e.g., "coding", "browsing", "chatting", "reading docs").
Do NOT read or transcribe specific text, code, variable names, or UI details from the screen.
Keep the "context" field to one short, general sentence.
Suggest broad, high-level actions only (e.g., "Open Terminal", "Switch to Safari")."#,
        "deep" => r#"[Analysis Depth: Deep]
Read and record ALL visible text on screen in detail. This includes:
- Article/document content, headings, and key paragraphs
- Code with function names, comments, and logic
- Chat/AI conversation messages (both user and AI responses)
- Vocabulary words, definitions, and example sentences
- Exercise questions, options, and answers
- Any learning material content

In the "context" field, provide a detailed summary that captures the specific content being viewed. If the user is learning or studying, extract key information (vocabulary, concepts, questions, formulas) into the context. Be thorough — the user wants everything recorded."#,
        // "normal" or fallback
        _ => r#"[Analysis Depth: Normal]
Focus on the user's active working area — the text cursor, input fields, active editor tabs, chat messages being composed or received. Read key details like file names, search queries, and AI conversation snippets, but do not transcribe entire documents or code blocks."#,
    }
}

/// Returns scene-specific instructions that shape AI behavior.
fn scene_instruction(scene: &str) -> &'static str {
    match scene {
        "learning" => r#"[Scene: Learning Mode]
The user is in a learning/studying session. Your primary job is to help track and record their learning.

Focus on:
- What learning material is on screen (article, docs, course, video, exercises, AI conversation about a topic)
- Extract key concepts, terms, vocabulary, or formulas from the visible content
- Track the learning topic and note any progression from previous activities
- Suggest learning-oriented next actions (take notes, review earlier concepts, search related topics, do exercises)

IMPORTANT: In your JSON output, include a "key_concepts" field — an array of 3-8 key terms, concepts, or vocabulary words visible on screen. Example:
"key_concepts": ["ownership", "borrow checker", "lifetime annotations"]

Do NOT suggest generic productivity actions like "Open Terminal". Keep all suggestions learning-focused."#,
        "working" => r#"[Scene: Working Mode]
The user is in a work/productivity session. Your primary job is to help them stay in flow and track progress.

Focus on:
- What project or task they're working on
- What stage of their workflow (coding, debugging, testing, reviewing, writing, communicating)
- Whether they seem stuck or context-switching frequently
- Suggest workflow-oriented next actions (run tests, commit code, open relevant tool, check docs)

Keep the "context" field brief — describe workflow state, not content details.
Do NOT read or transcribe specific code lines, document text, or chat messages in detail. Just identify what they're doing and where they are in their workflow."#,
        _ => "", // general mode: no scene-specific instruction
    }
}

/// Returns the max_tokens value based on analysis depth.
fn max_tokens_for_depth(depth: &str) -> u32 {
    match depth {
        "casual" => 512,
        "deep" => 4096,
        _ => 1024,
    }
}

/// Build the full prompt with activity history for observation mode.
pub fn build_full_prompt_with_history(
    language: &str,
    recent_activities: &[ActivityRecord],
    main_quest: Option<&str>,
    analysis_depth: &str,
    ocr_text: Option<&str>,
    scene_mode: &str,
    is_focus_crop: bool,
    app_name: Option<&str>,
    open_windows: Option<&str>,
    obsidian_context: Option<&str>,
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

    // Inject main quests — detect direction changes and ask about new quests
    if let Some(quest) = main_quest {
        parts.push(format!(
            "[Current Main Quests]\nThe user's active main goals:\n- {}\n\
            Prioritize suggesting actions that help achieve these quests.\n\
            IMPORTANT: If the user's current activity seems UNRELATED to any existing quest above, \
            do NOT force-fit it into an existing quest. Instead:\n\
            1. Describe what the user is ACTUALLY doing in the \"context\" field (be accurate, not forced)\n\
            2. Suggest the new direction as \"suggested_quest\" — phrase it as a question, e.g. \"为 Reth 项目做贡献？\" or \"学习 Rust 异步编程？\"\n\
            Do NOT re-suggest quests that already exist. Only suggest genuinely new, distinct directions.",
            quest
        ));
    }

    // Inject OCR-extracted screen text
    if let Some(text) = ocr_text {
        let trimmed = text.trim();
        if !trimmed.is_empty() {
            parts.push(format!("[Screen Text (OCR)]\n{}", trimmed));
        }
    }

    // Inject open windows list so AI knows what apps are available
    if let Some(windows) = open_windows {
        if !windows.is_empty() {
            parts.push(format!(
                "[Open Windows]\n\
                The following windows are currently open on the user's screen:\n\
                {}\n\n\
                When suggesting \"open_app\" actions, use the exact bundle_id from above as the \"app\" parameter.\n\
                Only suggest switching to apps that are actually listed here.",
                windows
            ));
        }
    }

    // Focus crop context — tell AI this is a cursor-area crop, not full screen
    if is_focus_crop {
        let app_info = app_name
            .map(|n| format!("Current app: {}", n))
            .unwrap_or_default();
        parts.push(format!(
            "[Focus Area]\n\
            This screenshot/text is cropped around the user's cursor position (approximate gaze area), \
            covering approximately 800x600 points of the screen.\n\
            {}\n\n\
            If you see content clearly cut off at the edges that would be important for your analysis, \
            set \"need_full_context\": true in your JSON response. Only do this when the missing content \
            would significantly change your understanding of what the user is doing.",
            app_info
        ));
    }

    // Scene-specific instruction (learning/working behavior)
    let scene_inst = scene_instruction(scene_mode);
    if !scene_inst.is_empty() {
        parts.push(scene_inst.to_string());
    }

    // Depth-specific instruction
    parts.push(depth_instruction(analysis_depth).to_string());

    // Inject relevant Obsidian vault notes
    if let Some(notes) = obsidian_context {
        if !notes.is_empty() {
            parts.push(format!(
                "[Obsidian Notes]\nRelevant notes from the user's knowledge base:\n{}",
                notes
            ));
        }
    }

    parts.push(ANALYSIS_PROMPT.to_string());
    parts.push("Consider the user's recent activity history above to provide more contextual and relevant suggestions. If you notice a workflow pattern, suggest the likely next step.".to_string());
    parts.join("\n\n")
}

/// Analyze a screenshot using Claude CLI.
/// When `send_image` is false, only OCR text is sent (no screenshot reference).
pub async fn analyze_with_cli(
    screenshot_path: &Path,
    model: &str,
    language: &str,
    recent_activities: &[ActivityRecord],
    main_quest: Option<&str>,
    analysis_depth: &str,
    ocr_text: Option<&str>,
    send_image: bool,
    scene_mode: &str,
    is_focus_crop: bool,
    app_name: Option<&str>,
    open_windows: Option<&str>,
    obsidian_context: Option<&str>,
) -> Result<AnalysisResult, String> {
    let full_prompt = build_full_prompt_with_history(
        language,
        recent_activities,
        main_quest,
        analysis_depth,
        ocr_text,
        scene_mode,
        is_focus_crop,
        app_name,
        open_windows,
        obsidian_context,
    );

    let prompt = if send_image {
        let path_str = screenshot_path
            .to_str()
            .ok_or("Invalid screenshot path")?;
        format!(
            "Read the screenshot image at '{}' and analyze it.\n\n{}",
            path_str, full_prompt
        )
    } else {
        format!(
            "Analyze the user's current screen based on the OCR text provided.\n\n{}",
            full_prompt
        )
    };

    let output = tokio::process::Command::new("claude")
        .args([
            "-p",
            &prompt,
            "--output-format",
            "text",
            "--max-turns",
            "2",
            "--model",
            model,
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

/// Analyze a screenshot using Claude API.
/// When `send_image` is false, only OCR text is sent (no base64 image), saving tokens.
pub async fn analyze_with_api(
    screenshot_path: &Path,
    api_key: &str,
    model: &str,
    language: &str,
    recent_activities: &[ActivityRecord],
    main_quest: Option<&str>,
    analysis_depth: &str,
    ocr_text: Option<&str>,
    send_image: bool,
    scene_mode: &str,
    is_focus_crop: bool,
    app_name: Option<&str>,
    open_windows: Option<&str>,
    obsidian_context: Option<&str>,
) -> Result<AnalysisResult, String> {
    let prompt_text = build_full_prompt_with_history(
        language,
        recent_activities,
        main_quest,
        analysis_depth,
        ocr_text,
        scene_mode,
        is_focus_crop,
        app_name,
        open_windows,
        obsidian_context,
    );

    let content = if send_image {
        let image_data = std::fs::read(screenshot_path)
            .map_err(|e| format!("Failed to read screenshot: {}", e))?;
        let base64_image = base64::engine::general_purpose::STANDARD.encode(&image_data);
        serde_json::json!([
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
                "text": prompt_text
            }
        ])
    } else {
        // Text-only mode: no image, just OCR text in the prompt
        serde_json::json!([
            {
                "type": "text",
                "text": prompt_text
            }
        ])
    };

    let client = reqwest::Client::new();
    let body = serde_json::json!({
        "model": model,
        "max_tokens": max_tokens_for_depth(analysis_depth),
        "messages": [{
            "role": "user",
            "content": content
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

    let mut messages: Vec<serde_json::Value> = history
        .iter()
        .map(|msg| {
            serde_json::json!({
                "role": msg.role,
                "content": msg.content
            })
        })
        .collect();

    // Claude API requires at least one user message; bootstrap with a greeting
    if messages.is_empty() {
        messages.push(serde_json::json!({
            "role": "user",
            "content": "Hi! Let's get started."
        }));
    }

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
