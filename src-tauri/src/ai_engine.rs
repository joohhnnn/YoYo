use crate::screen_context::ScreenContext;
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
    #[serde(skip_serializing_if = "Option::is_none")]
    pub prompt: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub directory: Option<String>,
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
    pub need_full_context: Option<bool>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct PlanStep {
    pub action_type: String,
    pub label: String,
    pub params: ActionParams,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct IntentResult {
    pub understanding: String,
    pub plan: Vec<PlanStep>,
    pub needs_confirmation: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub workflow_id: Option<i64>,
}

const ANALYSIS_PROMPT: &str = r#"You are YoYo, a desktop workflow assistant. Based on the screenshot, determine:

1. What is the user currently doing? (1 sentence)
2. Suggest 2-4 most likely next actions the user might want to take.
3. If you detect a clear goal or project the user is working on (e.g., "building a web app", "studying for exam", "writing a report"), suggest it as a main quest. Only suggest when you're confident about the user's goal. Do NOT suggest if the user is just browsing or doing miscellaneous tasks.

Each action must use one of these types:
- open_url: Open a URL (provide the url in params)
- open_app: Switch to an app (provide the bundle_id in params.app, e.g. "com.apple.Safari"). If an [Open Windows] section is provided, use the exact bundle_id listed there.
- copy_to_clipboard: Copy detected text/data (provide the text in params)
- insert_text: Type/paste text into the focused app (provide the text in params)
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

const INTENT_PROMPT: &str = r#"You are YoYo, a desktop workflow assistant. The user has given you a request. Based on the request AND the current screen context, create a plan to help them.

1. First, understand what the user wants — consider their words AND what's on screen.
2. Create an ordered plan of steps to achieve their goal.

Each step must use one of these action types:
- open_url: Open a URL (provide the url in params)
- open_app: Switch to an app (provide the bundle_id in params.app, e.g. "com.apple.Safari"). If an [Open Windows] section is provided, use the exact bundle_id listed there.
- copy_to_clipboard: Copy text (provide the text in params)
- insert_text: Type/paste text into the focused app (provide the text in params). Use for templates, drafts, code snippets.
- run_command: Run a terminal command (provide the command in params). Keep commands safe and non-destructive.
- notify: Send a notification (provide the message in params)
- claude_code: Run Claude Code CLI in a directory with a prompt (provide params.prompt and params.directory). Use for complex coding tasks like fixing bugs, writing code, or answering technical questions about a project.

Set "needs_confirmation" to true for plans with run_command, claude_code, insert_text, or multiple steps.
Set it to false for simple single-step actions (open_url, open_app).

Respond ONLY with valid JSON, no other text:
{
  "understanding": "User wants to ...",
  "plan": [
    {"action_type": "open_app", "label": "Open Terminal", "params": {"app": "com.apple.Terminal"}},
    {"action_type": "run_command", "label": "Navigate to project", "params": {"command": "cd ~/project && ls"}}
  ],
  "needs_confirmation": true
}"#;

/// Returns the analysis depth instruction to prepend to the prompt.
fn depth_instruction(depth: &str) -> &'static str {
    match depth {
        "casual" => {
            r#"[Analysis Depth: Casual]
Focus ONLY on identifying the active application and the user's general activity category (e.g., "coding", "browsing", "chatting", "reading docs").
Do NOT read or transcribe specific text, code, variable names, or UI details from the screen.
Keep the "context" field to one short, general sentence.
Suggest broad, high-level actions only (e.g., "Open Terminal", "Switch to Safari")."#
        }
        "deep" => {
            r#"[Analysis Depth: Deep]
Read and record ALL visible text on screen in detail. This includes:
- Article/document content, headings, and key paragraphs
- Code with function names, comments, and logic
- Chat/AI conversation messages (both user and AI responses)
- Vocabulary words, definitions, and example sentences
- Exercise questions, options, and answers
- Any learning material content

In the "context" field, provide a detailed summary that captures the specific content being viewed. If the user is learning or studying, extract key information (vocabulary, concepts, questions, formulas) into the context. Be thorough — the user wants everything recorded."#
        }
        // "normal" or fallback
        _ => {
            r#"[Analysis Depth: Normal]
Focus on the user's active working area — the text cursor, input fields, active editor tabs, chat messages being composed or received. Read key details like file names, search queries, and AI conversation snippets, but do not transcribe entire documents or code blocks."#
        }
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

/// Build shared context sections (language, profile, context, history, quests, screen info).
fn build_context_sections(
    language: &str,
    activity_summary: Option<&str>,
    recent_activities: &[ActivityRecord],
    main_quest: Option<&str>,
    current_scene: Option<&str>,
    ctx: &ScreenContext,
    is_focus_crop: bool,
    has_screenshot: bool,
) -> Vec<String> {
    let mut parts = Vec::new();

    // Language instruction
    match language {
        "en" => parts.push("Respond in English.".to_string()),
        _ => parts.push("请用中文回复。所有面向用户的文字字段都必须使用中文。".to_string()),
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

    // Inject activity summary (progressive summarization)
    if let Some(summary) = activity_summary {
        parts.push(format!("[Activity History Summary]\n{}", summary));
    }

    // Inject recent unsummarized activities
    if !recent_activities.is_empty() {
        let now = chrono::Local::now().naive_local();
        let mut history_lines = vec!["[Recent Activities]".to_string()];
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

    // Inject main quests
    if let Some(quest) = main_quest {
        parts.push(format!(
            "[Current Main Quests]\nThe user's active main goals:\n- {}",
            quest
        ));
    }

    // Inject current scene declaration
    if let Some(scene) = current_scene {
        parts.push(format!(
            "[Current Scene]\nThe user has declared they are currently: {}.\n\
             Tailor your analysis, context description, and action suggestions to this scene.",
            scene
        ));
    }

    // Screen context (app info, selected text, URL, AX/OCR text, open windows)
    let context_block = ctx.format_for_prompt();
    if !context_block.is_empty() {
        parts.push(context_block);
    }

    // Screenshot info
    if has_screenshot {
        if is_focus_crop {
            parts.push(
                "[Screenshot]\n\
                A screenshot of the area around the user's cursor is attached (800x600 crop).\n\
                If important content is cut off at the edges, set \"need_full_context\": true."
                    .to_string(),
            );
        } else {
            parts.push("[Screenshot]\nA full-screen screenshot is attached.".to_string());
        }
    } else {
        parts.push(
            "[Note] No screenshot attached. Analyze based on the text context above.".to_string(),
        );
    }

    // Depth-specific instruction
    parts.push(depth_instruction(&ctx.depth).to_string());

    parts
}

/// Build the full prompt for passive screen analysis.
pub fn build_full_prompt(
    language: &str,
    activity_summary: Option<&str>,
    recent_activities: &[ActivityRecord],
    main_quest: Option<&str>,
    current_scene: Option<&str>,
    ctx: &ScreenContext,
    is_focus_crop: bool,
    has_screenshot: bool,
) -> String {
    let mut parts = build_context_sections(
        language,
        activity_summary,
        recent_activities,
        main_quest,
        current_scene,
        ctx,
        is_focus_crop,
        has_screenshot,
    );

    // Analysis-specific: quest direction detection
    if main_quest.is_some() {
        parts.push(
            "IMPORTANT: If the user's current activity seems UNRELATED to any existing quest above, \
            do NOT force-fit it into an existing quest. Instead:\n\
            1. Describe what the user is ACTUALLY doing in the \"context\" field (be accurate, not forced)\n\
            2. Suggest the new direction as \"suggested_quest\" — phrase it as a question, e.g. \"为 Reth 项目做贡献？\" or \"学习 Rust 异步编程？\"\n\
            Do NOT re-suggest quests that already exist. Only suggest genuinely new, distinct directions."
                .to_string(),
        );
    }

    parts.push(ANALYSIS_PROMPT.to_string());
    parts.push("Consider the user's recent activity history above to provide more contextual and relevant suggestions. If you notice a workflow pattern, suggest the likely next step.".to_string());
    parts.join("\n\n")
}

/// Build the prompt for intent understanding (user request + screen context → plan).
pub fn build_intent_prompt(
    user_input: &str,
    language: &str,
    activity_summary: Option<&str>,
    recent_activities: &[ActivityRecord],
    main_quest: Option<&str>,
    current_scene: Option<&str>,
    ctx: &ScreenContext,
) -> String {
    let mut parts = build_context_sections(
        language,
        activity_summary,
        recent_activities,
        main_quest,
        current_scene,
        ctx,
        false,
        false,
    );
    parts.push(format!("[User Request]\n{}", user_input));
    parts.push(INTENT_PROMPT.to_string());
    parts.join("\n\n")
}

/// Analyze screen using Claude CLI.
/// When `screenshot_path` is None, no image is sent.
pub async fn analyze_with_cli(
    screenshot_path: Option<&Path>,
    model: &str,
    language: &str,
    activity_summary: Option<&str>,
    recent_activities: &[ActivityRecord],
    main_quest: Option<&str>,
    current_scene: Option<&str>,
    ctx: &ScreenContext,
    is_focus_crop: bool,
) -> Result<AnalysisResult, String> {
    let full_prompt = build_full_prompt(
        language,
        activity_summary,
        recent_activities,
        main_quest,
        current_scene,
        ctx,
        is_focus_crop,
        screenshot_path.is_some(),
    );

    let prompt = if let Some(path) = screenshot_path {
        let path_str = path.to_str().ok_or("Invalid screenshot path")?;
        format!(
            "Read the screenshot image at '{}' and analyze it.\n\n{}",
            path_str, full_prompt
        )
    } else {
        format!(
            "Analyze the user's current screen based on the context provided.\n\n{}",
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

/// Analyze screen using Claude API.
/// When `screenshot_path` is None, no image is sent, saving tokens.
pub async fn analyze_with_api(
    screenshot_path: Option<&Path>,
    api_key: &str,
    model: &str,
    language: &str,
    activity_summary: Option<&str>,
    recent_activities: &[ActivityRecord],
    main_quest: Option<&str>,
    current_scene: Option<&str>,
    ctx: &ScreenContext,
    is_focus_crop: bool,
) -> Result<AnalysisResult, String> {
    let prompt_text = build_full_prompt(
        language,
        activity_summary,
        recent_activities,
        main_quest,
        current_scene,
        ctx,
        is_focus_crop,
        screenshot_path.is_some(),
    );

    let content = if let Some(path) = screenshot_path {
        let image_data =
            std::fs::read(path).map_err(|e| format!("Failed to read screenshot: {}", e))?;
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
        "max_tokens": max_tokens_for_depth(&ctx.depth),
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

/// Extract JSON object from AI response (handles markdown code blocks).
pub fn extract_json_block(response: &str) -> &str {
    if let Some(start) = response.find('{') {
        if let Some(end) = response.rfind('}') {
            return &response[start..=end];
        }
    }
    response
}

/// Sanitize common AI JSON mistakes: Chinese punctuation, smart quotes, etc.
fn sanitize_json(raw: &str) -> String {
    raw
        // Chinese full-width punctuation → ASCII
        .replace('\u{FF0C}', ",") // ，
        .replace('\u{FF1A}', ":") // ：
        .replace('\u{3001}', ",") // 、
        // Smart quotes → escaped ASCII quotes (inside JSON strings)
        .replace('\u{201C}', "\\\"") // "
        .replace('\u{201D}', "\\\"") // "
        .replace('\u{2018}', "'") // '
        .replace('\u{2019}', "'") // '
}

/// Try to parse JSON, falling back to sanitized version on failure.
fn parse_json_lenient<T: serde::de::DeserializeOwned>(
    json_str: &str,
) -> Result<T, serde_json::Error> {
    // Try raw first
    match serde_json::from_str::<T>(json_str) {
        Ok(v) => Ok(v),
        Err(_) => {
            // Retry with sanitized JSON
            let sanitized = sanitize_json(json_str);
            serde_json::from_str::<T>(&sanitized)
        }
    }
}

/// Parse the AI response text into an AnalysisResult.
fn parse_ai_response(response: &str) -> Result<AnalysisResult, String> {
    let json_str = extract_json_block(response);
    parse_json_lenient::<AnalysisResult>(json_str).map_err(|e| {
        format!(
            "Failed to parse AI response as JSON: {}. Raw: {}",
            e, response
        )
    })
}

/// Parse the AI response text into an IntentResult.
fn parse_intent_response(response: &str) -> Result<IntentResult, String> {
    let json_str = extract_json_block(response);
    parse_json_lenient::<IntentResult>(json_str).map_err(|e| {
        format!(
            "Failed to parse intent response as JSON: {}. Raw: {}",
            e, response
        )
    })
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

// ---------------------------------------------------------------------------
// Intent understanding (user request + screen context → plan)
// ---------------------------------------------------------------------------

/// Understand user intent via Claude CLI (text-only, no image).
pub async fn intent_with_cli(
    user_input: &str,
    model: &str,
    language: &str,
    activity_summary: Option<&str>,
    recent_activities: &[ActivityRecord],
    main_quest: Option<&str>,
    current_scene: Option<&str>,
    ctx: &ScreenContext,
) -> Result<IntentResult, String> {
    let prompt = build_intent_prompt(
        user_input,
        language,
        activity_summary,
        recent_activities,
        main_quest,
        current_scene,
        ctx,
    );

    let output = tokio::process::Command::new("claude")
        .args([
            "-p",
            &prompt,
            "--output-format",
            "text",
            "--max-turns",
            "1",
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
    parse_intent_response(&response)
}

/// Understand user intent via Claude API (text-only, no image).
pub async fn intent_with_api(
    user_input: &str,
    api_key: &str,
    model: &str,
    language: &str,
    activity_summary: Option<&str>,
    recent_activities: &[ActivityRecord],
    main_quest: Option<&str>,
    current_scene: Option<&str>,
    ctx: &ScreenContext,
) -> Result<IntentResult, String> {
    let prompt = build_intent_prompt(
        user_input,
        language,
        activity_summary,
        recent_activities,
        main_quest,
        current_scene,
        ctx,
    );

    let client = reqwest::Client::new();
    let body = serde_json::json!({
        "model": model,
        "max_tokens": 1024,
        "messages": [{"role": "user", "content": prompt}]
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

    parse_intent_response(text)
}

// ---------------------------------------------------------------------------
// Simple text-only chat (no image) for session summary / chat
// ---------------------------------------------------------------------------

/// Simple text-only chat via CLI (no image).
pub async fn simple_chat_cli(
    prompt: &str,
    model: &str,
    max_tokens: Option<u32>,
) -> Result<String, String> {
    let tokens = max_tokens.unwrap_or(300).to_string();
    let output = tokio::process::Command::new("claude")
        .args(["-p", prompt, "--model", model, "--max-tokens", &tokens])
        .output()
        .await
        .map_err(|e| format!("Claude CLI failed: {}", e))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("Claude CLI error: {}", stderr));
    }
    Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

/// Simple text-only chat via API (no image).
pub async fn simple_chat_api(
    prompt: &str,
    api_key: &str,
    model: &str,
    max_tokens: Option<u32>,
) -> Result<String, String> {
    let client = reqwest::Client::new();
    let body = serde_json::json!({
        "model": model,
        "max_tokens": max_tokens.unwrap_or(300),
        "messages": [{"role": "user", "content": prompt}]
    });

    let resp = client
        .post("https://api.anthropic.com/v1/messages")
        .header("x-api-key", api_key)
        .header("anthropic-version", "2023-06-01")
        .header("content-type", "application/json")
        .json(&body)
        .send()
        .await
        .map_err(|e| format!("API request failed: {}", e))?;

    let json: serde_json::Value = resp
        .json()
        .await
        .map_err(|e| format!("Failed to parse response: {}", e))?;

    json["content"][0]["text"]
        .as_str()
        .map(|s| s.to_string())
        .ok_or_else(|| "No text in API response".to_string())
}

// ---------------------------------------------------------------------------
// Progressive summarization
// ---------------------------------------------------------------------------

const SUMMARIZE_PROMPT: &str = r#"You are summarizing a user's computer activity history for a desktop assistant.

Given:
1. A previous rolling summary (if any) that covers older activities
2. New activity records that haven't been summarized yet

Create a SINGLE PARAGRAPH that:
- Merges the previous summary with the new activities into one cohesive narrative
- Captures the user's workflow patterns, main focus areas, and transitions
- Preserves important specifics (project names, key tasks, tools used)
- Drops redundant or trivial details (e.g., repeated app switches)
- Stays under 150 words
- Is written in the same language as the activity records

Output ONLY the summary paragraph, no JSON, no headers, no extra text."#;

/// Build the summarization prompt from previous summary + new records.
pub fn build_summarize_prompt(
    prev_summary: Option<&str>,
    new_activities: &[ActivityRecord],
) -> String {
    let mut parts = Vec::new();

    if let Some(summary) = prev_summary {
        parts.push(format!("[Previous Summary]\n{}", summary));
    } else {
        parts.push("[Previous Summary]\nNo previous summary (first summarization).".to_string());
    }

    let mut lines = vec!["[New Activities]".to_string()];
    for a in new_activities {
        lines.push(format!("- {} [{}] {}", a.created_at, a.app_name, a.context));
    }
    parts.push(lines.join("\n"));

    parts.push(SUMMARIZE_PROMPT.to_string());
    parts.join("\n\n")
}

// ---------------------------------------------------------------------------
// Knowledge extraction (Phase 2.2)
// ---------------------------------------------------------------------------

const KNOWLEDGE_EXTRACTION_PROMPT: &str = r#"You are a knowledge extraction assistant. Based on the user's current screen context, extract any valuable knowledge items.

Extract ONLY items that are genuinely educational or useful to remember. Skip trivial or navigation-related text.

For each item, categorize as:
- "vocab": Technical terms, API names, concepts with clear definitions
- "reading": A brief summary of what the user is reading (1-2 sentences)
- "concept": Key concepts, patterns, or principles being studied

Respond ONLY with valid JSON:
{
  "items": [
    {"kind": "vocab", "content": "useEffect", "definition": "A React Hook that lets you synchronize a component with an external system"},
    {"kind": "reading", "content": "React Hooks documentation - explaining how to use side effects in functional components"},
    {"kind": "concept", "content": "Hooks must be called at the top level of a component, not inside conditions or loops"}
  ]
}

If there is nothing worth extracting, respond with: {"items": []}
Keep the total to a maximum of 5 items per extraction. Prefer quality over quantity."#;

#[derive(Debug, Serialize, Deserialize)]
pub struct KnowledgeItem {
    pub kind: String,
    pub content: String,
    #[serde(default)]
    pub definition: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct KnowledgeExtractionResult {
    pub items: Vec<KnowledgeItem>,
}

/// Build the knowledge extraction prompt from screen context.
pub fn build_knowledge_prompt(
    language: &str,
    ctx: &ScreenContext,
    analysis_context: &str,
    current_scene: Option<&str>,
) -> String {
    let mut parts = Vec::new();

    match language {
        "en" => parts.push("Respond in English.".to_string()),
        _ => parts.push("请用中文回复。".to_string()),
    }

    if let Some(scene) = current_scene {
        parts.push(format!(
            "[Current Scene]\nThe user is currently: {}. Focus extraction on content relevant to this scene.",
            scene
        ));
    }

    parts.push(format!("[Current Activity]\n{}", analysis_context));

    if let Some(ref ax) = ctx.ax_text {
        let truncated = if ax.len() > 3000 { &ax[..3000] } else { ax };
        parts.push(format!("[Screen Text]\n{}", truncated));
    }

    if let Some(ref sel) = ctx.selected_text {
        parts.push(format!("[Selected Text]\n{}", sel));
    }

    if let Some(ref url) = ctx.url {
        parts.push(format!("[Source URL]\n{}", url));
    }

    parts.push(KNOWLEDGE_EXTRACTION_PROMPT.to_string());
    parts.join("\n\n")
}

// ---------------------------------------------------------------------------
// Learning note generation
// ---------------------------------------------------------------------------

const NOTE_GENERATION_PROMPT: &str = r#"You are generating a structured learning note from a study session.
Based on the knowledge items extracted during this session, create a well-organized Markdown document.

Structure the note as follows:

## Overview
A 2-3 sentence summary of what was studied in this session.

## Key Concepts
List the important concepts, patterns, or principles learned. Use bullet points with bold terms and clear explanations.

## Terminology
If there are vocabulary/technical terms, create a table:
| Term | Definition |
|------|-----------|

## Reading Notes
If there are reading summaries, organize them into a coherent narrative.

## Connections
Note any relationships between concepts (optional, only if obvious connections exist).

Rules:
- Write in the same language as the knowledge items
- Keep it concise but informative
- Skip any section that has no relevant items (don't include empty sections)
- Do NOT wrap the output in a code block — output raw Markdown directly"#;

/// Build the note generation prompt from session knowledge items.
pub fn build_note_prompt(scene_name: &str, duration_str: &str, items: &[KnowledgeItem]) -> String {
    let mut parts = Vec::new();

    parts.push(format!(
        "[Session Info]\nScene: {}\nDuration: {}",
        scene_name, duration_str
    ));

    let mut items_text = String::from("[Extracted Knowledge Items]\n");
    for item in items {
        match item.kind.as_str() {
            "vocab" => {
                items_text.push_str(&format!(
                    "- [Vocab] {}: {}\n",
                    item.content,
                    item.definition.as_deref().unwrap_or("")
                ));
            }
            "concept" => {
                items_text.push_str(&format!("- [Concept] {}\n", item.content));
            }
            "reading" => {
                items_text.push_str(&format!("- [Reading] {}\n", item.content));
            }
            other => {
                items_text.push_str(&format!("- [{}] {}\n", other, item.content));
            }
        }
    }
    parts.push(items_text);

    parts.push(NOTE_GENERATION_PROMPT.to_string());
    parts.join("\n\n")
}
