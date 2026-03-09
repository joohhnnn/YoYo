use crate::accessibility;
use crate::window_list;
use crate::AppState;
use serde::{Deserialize, Serialize};
use std::sync::atomic::Ordering;
use tauri::{AppHandle, Manager};

/// Bundled screen context for analysis.
/// Replaces scattered variables in do_analyze().
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScreenContext {
    pub app_name: String,
    pub bundle_id: String,
    pub pid: i32,
    pub window_title: String,
    pub selected_text: Option<String>,
    pub url: Option<String>,
    pub ax_text: Option<String>,
    pub ocr_text: Option<String>, // filled later if screenshot taken
    pub open_windows: Vec<WindowSummary>,
    pub depth: String, // "casual" | "normal" | "deep"
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WindowSummary {
    pub app: String,
    pub title: String,
    pub bundle_id: String,
}

impl ScreenContext {
    /// Whether context is rich enough to skip screenshot.
    pub fn has_sufficient_text(&self) -> bool {
        if let Some(ref ax) = self.ax_text {
            if ax.len() > 100 {
                return true;
            }
        }
        if self.selected_text.is_some() && !self.window_title.is_empty() {
            return true;
        }
        false
    }

    /// Format open windows list for prompt injection.
    fn format_windows_for_prompt(&self) -> Option<String> {
        if self.open_windows.is_empty() {
            return None;
        }
        let lines: Vec<String> = self
            .open_windows
            .iter()
            .map(|w| {
                if w.title.is_empty() {
                    format!("- {} ({})", w.app, w.bundle_id)
                } else {
                    format!("- {}: \"{}\" ({})", w.app, w.title, w.bundle_id)
                }
            })
            .collect();
        Some(lines.join("\n"))
    }

    /// Compose a structured text block for the AI prompt.
    pub fn format_for_prompt(&self) -> String {
        let mut parts = Vec::new();

        // App info (always present)
        if !self.app_name.is_empty() {
            parts.push(format!(
                "[Current App]\nApp: {} ({})\nWindow: {}",
                self.app_name, self.bundle_id, self.window_title
            ));
        }

        // Browser URL
        if let Some(ref url) = self.url {
            if !url.is_empty() {
                parts.push(format!("[Browser URL]\n{}", url));
            }
        }

        // Selected text
        if let Some(ref sel) = self.selected_text {
            if !sel.is_empty() {
                let display = if sel.len() > 2000 {
                    format!("{}... (truncated)", &sel[..2000])
                } else {
                    sel.clone()
                };
                parts.push(format!("[Selected Text]\n{}", display));
            }
        }

        // Screen text: prefer AX text, fall back to OCR
        if let Some(ref ax) = self.ax_text {
            if !ax.trim().is_empty() {
                parts.push(format!("[Screen Text (Accessibility)]\n{}", ax));
            }
        } else if let Some(ref ocr) = self.ocr_text {
            if !ocr.trim().is_empty() {
                parts.push(format!("[Screen Text (OCR)]\n{}", ocr));
            }
        }

        // Open windows
        if let Some(windows_text) = self.format_windows_for_prompt() {
            parts.push(format!(
                "[Open Windows]\n\
                The following windows are currently open on the user's screen:\n\
                {}\n\n\
                When suggesting \"open_app\" actions, use the exact bundle_id from above as the \"app\" parameter.\n\
                Only suggest switching to apps that are actually listed here.",
                windows_text
            ));
        }

        parts.join("\n\n")
    }
}

/// Adaptive depth based on current app type.
/// IDEs / terminals -> normal (read cursor area code)
/// Browsers / readers -> deep (capture article content)
/// Chat / media / system apps -> casual (just track app usage)
pub fn depth_for_app(bundle_id: &str) -> &'static str {
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

/// Check if the given bundle_id is blacklisted.
pub fn is_blacklisted(bundle_id: &str, blacklist: &[String]) -> bool {
    blacklist.iter().any(|b| b == bundle_id)
}

/// Capture all available screen context without taking a screenshot.
/// This is fast (~50-300ms) compared to screenshot+OCR (~500-1500ms).
pub fn capture(app: &AppHandle) -> ScreenContext {
    // Get app info from state (each try_state call returns a fresh Option)
    let (app_name, bundle_id, pid) = {
        let name = app
            .try_state::<AppState>()
            .and_then(|s| s.current_app_name.lock().ok().map(|n| n.clone()))
            .unwrap_or_default();
        let bid = app
            .try_state::<AppState>()
            .and_then(|s| s.current_bundle_id.lock().ok().map(|b| b.clone()))
            .unwrap_or_default();
        let pid = app
            .try_state::<AppState>()
            .map(|s| s.current_app_pid.load(Ordering::Relaxed) as i32)
            .unwrap_or(0);
        (name, bid, pid)
    };

    // Determine analysis depth
    let depth = depth_for_app(&bundle_id).to_string();

    // Extract AX data (includes selected_text and URL now)
    let (ax_text, window_title, selected_text, url) = if pid > 0 {
        match accessibility::extract_text(pid) {
            Ok(result) if result.error.is_none() => {
                let text = if result.text.trim().is_empty() {
                    None
                } else {
                    eprintln!(
                        "AX extracted {} nodes, {} chars from {}",
                        result.node_count,
                        result.text.len(),
                        result.app_name
                    );
                    Some(result.text)
                };
                (text, result.window_title, result.selected_text, result.url)
            }
            Ok(result) => {
                eprintln!("AX returned error: {:?}", result.error);
                (None, result.window_title, None, None)
            }
            Err(e) => {
                eprintln!("AX extraction failed: {}", e);
                (None, String::new(), None, None)
            }
        }
    } else {
        (None, String::new(), None, None)
    };

    // Get visible windows
    let open_windows = window_list::get_visible_windows()
        .unwrap_or_default()
        .into_iter()
        .map(|w| WindowSummary {
            app: w.app,
            title: w.title,
            bundle_id: w.bundle_id,
        })
        .collect();

    ScreenContext {
        app_name,
        bundle_id,
        pid,
        window_title,
        selected_text,
        url,
        ax_text,
        ocr_text: None, // filled later if screenshot is taken
        open_windows,
        depth,
    }
}
