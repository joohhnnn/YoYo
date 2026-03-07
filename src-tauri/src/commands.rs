use crate::accessibility;
use crate::ai_engine::{self, AnalysisResult};
use crate::focus_capture;
use crate::ocr;
use crate::screenshot;
use crate::user_data::{self, ActivityRecord};
use crate::window_list;
use crate::AppState;
use std::sync::atomic::Ordering;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;
use tauri::{AppHandle, Emitter, Manager};

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ChatMessage {
    pub role: String, // "user" or "assistant"
    pub content: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Settings {
    pub ai_mode: String, // "cli" or "api"
    pub api_key: String,
    #[serde(default = "default_model")]
    pub model: String, // e.g. "claude-haiku-4-5-20251001", "claude-sonnet-4-20250514"
    pub shortcut_toggle: String,
    pub shortcut_analyze: String,
    pub analysis_cooldown_secs: u64,
    #[serde(default = "default_bubble_opacity")]
    pub bubble_opacity: f64,
    #[serde(default = "default_language")]
    pub language: String, // "zh" or "en"
    #[serde(default = "default_auto_analyze")]
    pub auto_analyze: bool,
    #[serde(default = "default_analysis_depth")]
    pub analysis_depth: String, // "casual" | "normal" | "deep"
    #[serde(default = "default_scene_mode")]
    pub scene_mode: String, // "general" | "learning" | "working"
    #[serde(default)]
    pub obsidian_vault_path: String,
}

fn default_model() -> String {
    "claude-haiku-4-5-20251001".to_string()
}

fn default_bubble_opacity() -> f64 {
    0.85
}

fn default_language() -> String {
    "zh".to_string()
}

fn default_auto_analyze() -> bool {
    true
}

fn default_analysis_depth() -> String {
    "normal".to_string()
}

fn default_scene_mode() -> String {
    "general".to_string()
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            ai_mode: "cli".to_string(),
            api_key: String::new(),
            model: "claude-haiku-4-5-20251001".to_string(),
            shortcut_toggle: "CmdOrCtrl+Shift+Y".to_string(),
            shortcut_analyze: "CmdOrCtrl+Shift+R".to_string(),
            analysis_cooldown_secs: 2,
            bubble_opacity: 0.85,
            language: "zh".to_string(),
            auto_analyze: true,
            analysis_depth: "normal".to_string(),
            scene_mode: "general".to_string(),
            obsidian_vault_path: String::new(),
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct TaskItem {
    pub id: String,
    pub text: String,
    pub done: bool,
    #[serde(default = "default_quest_type")]
    pub quest_type: String, // "main" or "side"
    #[serde(default)]
    pub progress: Option<u32>,
    #[serde(default)]
    pub target: Option<u32>,
}

fn default_quest_type() -> String {
    "side".to_string()
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct AppData {
    pub settings: Settings,
    pub tasks: Vec<TaskItem>,
}

impl Default for AppData {
    fn default() -> Self {
        Self {
            settings: Settings::default(),
            tasks: Vec::new(),
        }
    }
}

fn data_path(app: &AppHandle) -> PathBuf {
    let dir = app
        .path()
        .app_data_dir()
        .unwrap_or_else(|_| PathBuf::from("."));
    fs::create_dir_all(&dir).ok();
    dir.join("yoyo_data.json")
}

fn load_data(app: &AppHandle) -> AppData {
    let path = data_path(app);
    if path.exists() {
        fs::read_to_string(&path)
            .ok()
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default()
    } else {
        AppData::default()
    }
}

/// Adaptive depth based on current app type (used in "general" scene mode).
/// IDEs / terminals → normal (read cursor area code)
/// Browsers / readers → deep (capture article content)
/// Chat / media / system apps → casual (just track app usage)
fn depth_for_app(bundle_id: &str) -> &'static str {
    let bid = bundle_id.to_lowercase();

    // Deep: reading-heavy apps (browsers, document viewers, ebooks)
    if bid.contains("safari")
        || bid.contains("chrome")
        || bid.contains("firefox")
        || bid.contains("edge")
        || bid.contains("arc")
        || bid.contains("orion")
        || bid.contains("preview")
        || bid.contains("books")
        || bid.contains("kindle")
        || bid.contains("pdf")
        || bid.contains("reader")
        || bid.contains("notion")
        || bid.contains("obsidian")
        || bid.contains("pages")
        || bid.contains("word")
    {
        return "deep";
    }

    // Casual: chat, media, system utilities
    if bid.contains("slack")
        || bid.contains("discord")
        || bid.contains("telegram")
        || bid.contains("wechat")
        || bid.contains("messages")
        || bid.contains("whatsapp")
        || bid.contains("spotify")
        || bid.contains("music")
        || bid.contains("photos")
        || bid.contains("finder")
        || bid.contains("systempreferences")
        || bid.contains("systemsettings")
        || bid.contains("activity")
    {
        return "casual";
    }

    // Normal (default): IDEs, terminals, editors, everything else
    "normal"
}

fn save_data(app: &AppHandle, data: &AppData) -> Result<(), String> {
    let path = data_path(app);
    let json = serde_json::to_string_pretty(data).map_err(|e| e.to_string())?;
    fs::write(&path, json).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn take_screenshot() -> Result<String, String> {
    let path = screenshot::capture_screen()?;
    path.to_str()
        .map(|s| s.to_string())
        .ok_or_else(|| "Invalid path".to_string())
}

/// Core analysis logic, usable from both the Tauri command and Rust-side auto-analysis.
pub async fn do_analyze(app: &AppHandle) -> Result<AnalysisResult, String> {
    // Skip analysis during onboarding
    if let Some(state) = app.try_state::<AppState>() {
        if let Ok(active) = state.onboarding_active.lock() {
            if *active {
                return Err("Skipped: onboarding in progress".to_string());
            }
        }
    }

    let _ = app.emit("analysis-progress", "Capturing...");
    let data = load_data(app);

    // Get current app name from state
    let current_app_name = app
        .try_state::<AppState>()
        .and_then(|s| s.current_app_name.lock().ok().map(|n| n.clone()))
        .unwrap_or_default();
    let app_name_ref = if current_app_name.is_empty() {
        None
    } else {
        Some(current_app_name.as_str())
    };

    // Get visible windows for AI context
    let windows = window_list::get_visible_windows().unwrap_or_default();
    let windows_text = if windows.is_empty() {
        None
    } else {
        Some(window_list::format_for_prompt(&windows))
    };

    // Fetch recent activities for context injection
    let recent = user_data::get_recent_activities(30).unwrap_or_default();

    // Extract all active main quests for prompt injection
    let main_quests: Vec<String> = data
        .tasks
        .iter()
        .filter(|t| t.quest_type == "main" && !t.done)
        .map(|t| {
            if let (Some(progress), Some(target)) = (t.progress, t.target) {
                format!("{} ({}/{})", t.text, progress, target)
            } else {
                t.text.clone()
            }
        })
        .collect();
    let main_quest = if main_quests.is_empty() {
        None
    } else {
        Some(main_quests.join("\n- "))
    };

    let has_active_quests = !main_quests.is_empty();

    let scene = &data.settings.scene_mode;

    // Get current app bundle_id for adaptive depth
    let current_bundle = app
        .try_state::<AppState>()
        .and_then(|s| s.current_bundle_id.lock().ok().map(|b| b.clone()))
        .unwrap_or_default();

    // Scene auto-determines effective depth:
    // - learning → deep (need to read all content)
    // - working → casual (just track workflow)
    // - general → adaptive by app type, fallback "normal"
    let effective_depth = match scene.as_str() {
        "learning" => "deep",
        "working" => "casual",
        _ => depth_for_app(&current_bundle),
    };

    // Non-deep modes: use cursor-area focus capture instead of full screen
    let use_focus_crop = effective_depth != "deep";

    let (image_path, is_focus_crop) = if use_focus_crop {
        match focus_capture::capture_focus_area() {
            Ok(capture) => (capture.image_path, true),
            Err(e) => {
                eprintln!("Focus capture failed, falling back to full screenshot: {}", e);
                (screenshot::capture_screen()?, false)
            }
        }
    } else {
        // Deep mode: always use full screenshot
        (screenshot::capture_screen()?, false)
    };

    let _ = app.emit("analysis-progress", "Extracting text...");
    // Text extraction: try Accessibility API first, then fall back to OCR
    let current_pid = app
        .try_state::<AppState>()
        .map(|s| s.current_app_pid.load(Ordering::Relaxed) as i32)
        .unwrap_or(0);

    let ax_text = if current_pid > 0 {
        match accessibility::extract_text(current_pid) {
            Ok(result) if !result.text.trim().is_empty() => {
                eprintln!(
                    "AX extracted {} nodes, {} chars from {}",
                    result.node_count,
                    result.text.len(),
                    result.app_name
                );
                Some(result.text)
            }
            Ok(_) => {
                eprintln!("AX returned empty text, falling back to OCR");
                None
            }
            Err(e) => {
                eprintln!("AX extraction failed ({}), falling back to OCR", e);
                None
            }
        }
    } else {
        None
    };

    // Use AX text if available, otherwise fall back to OCR
    let ocr_text = if ax_text.is_some() {
        ax_text
    } else {
        match ocr::recognize_text(&image_path) {
            Ok(result) => {
                if result.text.trim().is_empty() {
                    None
                } else {
                    Some(result.text)
                }
            }
            Err(e) => {
                eprintln!("OCR failed, falling back to image-only: {}", e);
                None
            }
        }
    };

    // Decide whether to send image based on depth:
    // - casual/normal: text-only (OCR text), skip image to save tokens
    // - deep: send both OCR text + image for maximum detail
    // - fallback: if OCR failed (no text), always send image
    let send_image = effective_depth == "deep" || ocr_text.is_none();

    let _ = app.emit("analysis-progress", "Analyzing...");
    let mut result = if data.settings.ai_mode == "api" && !data.settings.api_key.is_empty() {
        ai_engine::analyze_with_api(
            &image_path,
            &data.settings.api_key,
            &data.settings.model,
            &data.settings.language,
            &recent,
            main_quest.as_deref(),
            effective_depth,
            ocr_text.as_deref(),
            send_image,
            scene,
            is_focus_crop,
            app_name_ref,
            windows_text.as_deref(),
        )
        .await
    } else {
        ai_engine::analyze_with_cli(
            &image_path,
            &data.settings.model,
            &data.settings.language,
            &recent,
            main_quest.as_deref(),
            effective_depth,
            ocr_text.as_deref(),
            send_image,
            scene,
            is_focus_crop,
            app_name_ref,
            windows_text.as_deref(),
        )
        .await
    }?;

    // If AI requested full context and we used a focus crop, do a second round
    if is_focus_crop && result.need_full_context == Some(true) {
        eprintln!("AI requested full context — performing second-round full screen analysis");
        let full_screenshot = screenshot::capture_screen()?;
        let full_ocr = match ocr::recognize_text(&full_screenshot) {
            Ok(r) if !r.text.trim().is_empty() => Some(r.text),
            _ => None,
        };
        let full_send_image = effective_depth == "deep" || full_ocr.is_none();

        result = if data.settings.ai_mode == "api" && !data.settings.api_key.is_empty() {
            ai_engine::analyze_with_api(
                &full_screenshot,
                &data.settings.api_key,
                &data.settings.model,
                &data.settings.language,
                &recent,
                main_quest.as_deref(),
                effective_depth,
                full_ocr.as_deref(),
                full_send_image,
                scene,
                false, // not a focus crop anymore
                app_name_ref,
                windows_text.as_deref(),
            )
            .await
        } else {
            ai_engine::analyze_with_cli(
                &full_screenshot,
                &data.settings.model,
                &data.settings.language,
                &recent,
                main_quest.as_deref(),
                effective_depth,
                full_ocr.as_deref(),
                full_send_image,
                scene,
                false,
                app_name_ref,
                windows_text.as_deref(),
            )
            .await
        }?;
    }

    // Filter out suggested_quest if it duplicates an existing active quest
    if has_active_quests {
        if let Some(ref suggested) = result.suggested_quest {
            let suggested_lower = suggested.to_lowercase();
            let is_duplicate = main_quests.iter().any(|q| {
                let q_lower = q.to_lowercase();
                q_lower.contains(&suggested_lower) || suggested_lower.contains(&q_lower)
            });
            if is_duplicate {
                result.suggested_quest = None;
            }
        }
    }

    Ok(result)
}

/// Read the analysis cooldown from persisted settings.
pub fn get_cooldown_secs(app: &AppHandle) -> u64 {
    load_data(app).settings.analysis_cooldown_secs
}

/// Check if auto-analyze on app switch is enabled.
pub fn get_auto_analyze(app: &AppHandle) -> bool {
    load_data(app).settings.auto_analyze
}

#[tauri::command]
pub async fn analyze_screen(app: AppHandle) -> Result<AnalysisResult, String> {
    let result = do_analyze(&app).await?;

    // Cache result for bubble window to pick up on mount
    if let Some(state) = app.try_state::<AppState>() {
        if let Ok(mut cache) = state.last_analysis.lock() {
            *cache = Some(result.clone());
        }
    }

    // Broadcast to all windows (bubble listens for this)
    let _ = app.emit("analysis-complete", &result);

    // Show the floating action bubble
    crate::show_bubble(&app);

    Ok(result)
}

#[tauri::command]
pub fn get_last_analysis(app: AppHandle) -> Option<AnalysisResult> {
    let state = app.try_state::<AppState>()?;
    let cache = state.last_analysis.lock().ok()?;
    cache.clone()
}

#[tauri::command]
pub async fn execute_action(
    app: AppHandle,
    action_type: String,
    params: serde_json::Value,
) -> Result<(), String> {
    match action_type.as_str() {
        "open_url" => {
            let url = params["url"]
                .as_str()
                .ok_or("Missing url parameter")?;
            // Only allow http/https URLs to prevent file:// or custom scheme attacks
            if !url.starts_with("http://") && !url.starts_with("https://") {
                return Err(format!("Blocked URL with unsupported scheme: {}", url));
            }
            open::that(url).map_err(|e| format!("Failed to open URL: {}", e))
        }
        "open_app" => {
            let app_id = params["app"]
                .as_str()
                .ok_or("Missing app parameter")?;
            // Sanitize: only allow alphanumeric, spaces, dots, hyphens
            if !app_id.chars().all(|c| c.is_alphanumeric() || c == ' ' || c == '.' || c == '-') {
                return Err("Invalid app identifier".to_string());
            }
            // Use bundle_id (-b) for reverse-DNS identifiers, app name (-a) for plain names
            let is_bundle_id = app_id.contains('.') && !app_id.contains(' ');
            let flag = if is_bundle_id { "-b" } else { "-a" };
            let output = std::process::Command::new("open")
                .args([flag, app_id])
                .output()
                .map_err(|e| format!("Failed to open app: {}", e))?;
            if output.status.success() {
                Ok(())
            } else {
                Err(String::from_utf8_lossy(&output.stderr).to_string())
            }
        }
        "copy_to_clipboard" => {
            let text = params["text"]
                .as_str()
                .ok_or("Missing text parameter")?;
            let mut child = std::process::Command::new("pbcopy")
                .stdin(std::process::Stdio::piped())
                .spawn()
                .map_err(|e| format!("Failed to run pbcopy: {}", e))?;
            use std::io::Write;
            child
                .stdin
                .as_mut()
                .unwrap()
                .write_all(text.as_bytes())
                .map_err(|e| format!("Failed to write to pbcopy: {}", e))?;
            child.wait().map_err(|e| e.to_string())?;
            Ok(())
        }
        "run_command" => {
            let cmd = params["command"]
                .as_str()
                .ok_or("Missing command parameter")?;
            validate_command(cmd)?;
            let output = std::process::Command::new("sh")
                .args(["-c", cmd])
                .output()
                .map_err(|e| format!("Failed to run command: {}", e))?;
            if output.status.success() {
                Ok(())
            } else {
                Err(String::from_utf8_lossy(&output.stderr).to_string())
            }
        }
        "notify" => {
            let message = params["message"]
                .as_str()
                .ok_or("Missing message parameter")?;
            // Use tauri-plugin-notification — no shell injection risk
            tauri_plugin_notification::NotificationExt::notification(&app)
                .builder()
                .title("YoYo")
                .body(message)
                .show()
                .map_err(|e| format!("Failed to send notification: {}", e))?;
            Ok(())
        }
        _ => Err(format!("Unknown action type: {}", action_type)),
    }
}

#[tauri::command]
pub fn get_settings(app: AppHandle) -> Settings {
    load_data(&app).settings
}

#[tauri::command]
pub fn save_settings(app: AppHandle, settings: Settings) -> Result<(), String> {
    let mut data = load_data(&app);
    data.settings = settings;
    save_data(&app, &data)
}

#[tauri::command]
pub fn get_tasks(app: AppHandle) -> Vec<TaskItem> {
    load_data(&app).tasks
}

#[tauri::command]
pub fn save_tasks(app: AppHandle, tasks: Vec<TaskItem>) -> Result<(), String> {
    let mut data = load_data(&app);
    data.tasks = tasks;
    save_data(&app, &data)
}

#[tauri::command]
pub fn get_profile() -> Result<String, String> {
    user_data::read_profile()
}

#[tauri::command]
pub fn save_profile(content: String) -> Result<(), String> {
    user_data::write_profile(&content)
}

#[tauri::command]
pub fn get_context() -> Result<String, String> {
    user_data::read_context()
}

#[tauri::command]
pub fn save_context(content: String) -> Result<(), String> {
    user_data::write_context(&content)
}

// ---------------------------------------------------------------------------
// Onboarding commands
// ---------------------------------------------------------------------------

#[tauri::command]
pub fn check_needs_onboarding() -> Result<bool, String> {
    user_data::is_profile_default()
}

#[tauri::command]
pub async fn start_onboarding(app: AppHandle) -> Result<ChatMessage, String> {
    let state = app.state::<AppState>();

    // Mark onboarding as active
    {
        let mut active = state.onboarding_active.lock().map_err(|e| e.to_string())?;
        *active = true;
    }
    {
        let mut history = state.onboarding_history.lock().map_err(|e| e.to_string())?;
        history.clear();
    }

    // Call AI with empty history to get first question
    let data = load_data(&app);
    let response = if data.settings.ai_mode == "api" && !data.settings.api_key.is_empty() {
        ai_engine::onboarding_chat_api(&[], &data.settings.api_key, &data.settings.model, &data.settings.language).await?
    } else {
        ai_engine::onboarding_chat_cli(&[], &data.settings.model, &data.settings.language).await?
    };

    let msg = ChatMessage {
        role: "assistant".to_string(),
        content: response,
    };

    // Store in history
    {
        let mut history = state.onboarding_history.lock().map_err(|e| e.to_string())?;
        history.push(msg.clone());
    }

    Ok(msg)
}

#[tauri::command]
pub async fn send_onboarding_message(
    app: AppHandle,
    message: String,
) -> Result<ChatMessage, String> {
    let state = app.state::<AppState>();

    // Add user message to history
    {
        let mut history = state.onboarding_history.lock().map_err(|e| e.to_string())?;
        history.push(ChatMessage {
            role: "user".to_string(),
            content: message,
        });
    }

    // Get history snapshot
    let history_snapshot = {
        let history = state.onboarding_history.lock().map_err(|e| e.to_string())?;
        history.clone()
    };

    // Call AI
    let data = load_data(&app);
    let response = if data.settings.ai_mode == "api" && !data.settings.api_key.is_empty() {
        ai_engine::onboarding_chat_api(
            &history_snapshot,
            &data.settings.api_key,
            &data.settings.model,
            &data.settings.language,
        )
        .await?
    } else {
        ai_engine::onboarding_chat_cli(&history_snapshot, &data.settings.model, &data.settings.language).await?
    };

    // Check for profile completion marker
    if response.contains("[PROFILE_COMPLETE]") {
        if let (Some(start), Some(end)) = (
            response.find("[PROFILE_COMPLETE]"),
            response.find("[/PROFILE_COMPLETE]"),
        ) {
            let profile_content = &response[start + "[PROFILE_COMPLETE]".len()..end];
            if !profile_content.trim().is_empty() {
                user_data::write_profile(profile_content.trim())?;
            }
        }

        // End onboarding
        {
            let mut active = state.onboarding_active.lock().map_err(|e| e.to_string())?;
            *active = false;
        }

        let _ = app.emit("onboarding-complete", ());

        return Ok(ChatMessage {
            role: "assistant".to_string(),
            content: "Profile saved! Switching to normal mode...".to_string(),
        });
    }

    // Normal conversation turn
    let assistant_msg = ChatMessage {
        role: "assistant".to_string(),
        content: response,
    };

    {
        let mut history = state.onboarding_history.lock().map_err(|e| e.to_string())?;
        history.push(assistant_msg.clone());
    }

    Ok(assistant_msg)
}

#[tauri::command]
pub fn finish_onboarding(app: AppHandle) -> Result<(), String> {
    let state = app.state::<AppState>();
    let mut active = state.onboarding_active.lock().map_err(|e| e.to_string())?;
    *active = false;
    let mut history = state.onboarding_history.lock().map_err(|e| e.to_string())?;
    history.clear();
    Ok(())
}

// ---------------------------------------------------------------------------
// Command validation
// ---------------------------------------------------------------------------

/// Validate a shell command against dangerous patterns.
/// Uses an expanded blocklist + structural pattern detection.
fn validate_command(cmd: &str) -> Result<(), String> {
    let lower = cmd.to_lowercase();

    // Blocked command patterns (case-insensitive)
    let blocked_patterns = [
        "rm -rf", "rm -r -f", "rm -fr",
        "sudo", "su -",
        "mkfs", "fdisk", "parted",
        "dd if=", "dd of=",
        "> /dev/", ">/dev/",
        "chmod -r 777", "chmod 777",
        "curl | sh", "curl |sh", "curl|sh",
        "wget | sh", "wget |sh", "wget|sh",
        "curl | bash", "curl |bash", "curl|bash",
        "wget | bash", "wget |bash", "wget|bash",
        "eval ", "exec ",
        ":(){ ", ":(){",          // fork bomb
        "/etc/passwd", "/etc/shadow",
        "launchctl", "defaults write",
        "networksetup", "systemsetup",
        "osascript",              // prevent AppleScript via command
        "security delete", "security add",
        "killall", "pkill -9",
        "shutdown", "reboot", "halt",
    ];

    for pattern in &blocked_patterns {
        if lower.contains(pattern) {
            return Err(format!("Blocked dangerous command pattern: {}", pattern));
        }
    }

    // Block shell injection patterns: $(...), `...`, ${...}
    if cmd.contains("$(") || cmd.contains('`') || cmd.contains("${") {
        return Err("Blocked: command substitution not allowed".to_string());
    }

    // Block output redirection to arbitrary files (allow /dev/null)
    let stripped = cmd.replace("/dev/null", "");
    if stripped.contains(">>") || stripped.contains("> /") || stripped.contains(">/") {
        return Err("Blocked: output redirection not allowed".to_string());
    }

    // Block piping to interpreters
    let pipe_targets = ["sh", "bash", "zsh", "python", "perl", "ruby", "node"];
    if let Some(pipe_pos) = cmd.find('|') {
        let after_pipe = cmd[pipe_pos + 1..].trim();
        for target in &pipe_targets {
            if after_pipe.starts_with(target)
                && after_pipe[target.len()..]
                    .starts_with(|c: char| c.is_whitespace() || c == '\0')
                || after_pipe == *target
            {
                return Err(format!("Blocked: piping to {} not allowed", target));
            }
        }
    }

    Ok(())
}

// ---------------------------------------------------------------------------
// Activity history query
// ---------------------------------------------------------------------------

#[tauri::command]
pub fn get_recent_activities(limit: Option<usize>) -> Result<Vec<ActivityRecord>, String> {
    user_data::get_recent_activities(limit.unwrap_or(30))
}

#[tauri::command]
pub fn get_latest_reflection() -> Result<Option<user_data::ReflectionRecord>, String> {
    user_data::get_latest_reflection()
}

// ---------------------------------------------------------------------------
// Reflection (long-term memory)
// ---------------------------------------------------------------------------

/// Trigger a reflection analysis of recent activities.
pub async fn trigger_reflection(app: &AppHandle) -> Result<(), String> {
    let activities = user_data::get_recent_activities(100)?;
    if activities.is_empty() {
        return Ok(());
    }

    let data = load_data(app);
    let summary = if data.settings.ai_mode == "api" && !data.settings.api_key.is_empty() {
        ai_engine::generate_reflection_api(
            &activities,
            &data.settings.api_key,
            &data.settings.model,
            &data.settings.language,
        )
        .await?
    } else {
        ai_engine::generate_reflection_cli(&activities, &data.settings.model, &data.settings.language).await?
    };

    let total = user_data::get_total_activity_count()?;
    let period_start = activities
        .last()
        .map(|a| a.created_at.clone())
        .unwrap_or_default();
    let period_end = activities
        .first()
        .map(|a| a.created_at.clone())
        .unwrap_or_default();

    user_data::save_reflection(&summary, total, &period_start, &period_end)?;
    user_data::update_context_with_reflection(&summary, &period_end)?;

    Ok(())
}
