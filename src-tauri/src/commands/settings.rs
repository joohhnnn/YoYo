use crate::user_data;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;
use tauri::{AppHandle, Manager};

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
    #[serde(default = "default_app_blacklist")]
    pub app_blacklist: Vec<String>,
    #[serde(default)]
    pub onboarding_completed: bool,
    #[serde(default)]
    pub preferred_mic_device: String,
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

fn default_app_blacklist() -> Vec<String> {
    vec![
        "com.1password.1password".to_string(),
        "com.agilebits.onepassword7".to_string(),
        "com.bitwarden.desktop".to_string(),
        "org.keepassxc.keepassxc".to_string(),
        "com.lastpass.LastPass".to_string(),
        "com.dashlane.Dashlane".to_string(),
        "com.apple.keychainaccess".to_string(),
    ]
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
            app_blacklist: default_app_blacklist(),
            onboarding_completed: false,
            preferred_mic_device: String::new(),
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

pub fn load_data(app: &AppHandle) -> AppData {
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

pub fn save_data(app: &AppHandle, data: &AppData) -> Result<(), String> {
    let path = data_path(app);
    let json = serde_json::to_string_pretty(data).map_err(|e| e.to_string())?;
    fs::write(&path, json).map_err(|e| e.to_string())
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
