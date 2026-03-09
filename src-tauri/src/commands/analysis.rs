use crate::accessibility;
use crate::ai_engine::{self, AnalysisResult};
use crate::focus_capture;
use crate::ocr;
use crate::screenshot;
use crate::user_data;
use crate::window_list;
use crate::AppState;
use std::sync::atomic::Ordering;
use tauri::{AppHandle, Emitter, Manager};

use super::settings::load_data;

/// Adaptive depth based on current app type.
/// IDEs / terminals -> normal (read cursor area code)
/// Browsers / readers -> deep (capture article content)
/// Chat / media / system apps -> casual (just track app usage)
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

#[tauri::command]
pub fn take_screenshot() -> Result<String, String> {
    let path = screenshot::capture_screen()?;
    path.to_str()
        .map(|s| s.to_string())
        .ok_or_else(|| "Invalid path".to_string())
}

/// Core analysis logic, usable from both the Tauri command and Rust-side auto-analysis.
pub async fn do_analyze(app: &AppHandle) -> Result<AnalysisResult, String> {
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

    // Get current app bundle_id for adaptive depth
    let current_bundle = app
        .try_state::<AppState>()
        .and_then(|s| s.current_bundle_id.lock().ok().map(|b| b.clone()))
        .unwrap_or_default();

    // Adaptive depth based on current app type
    let effective_depth = depth_for_app(&current_bundle);

    // Non-deep modes: use cursor-area focus capture instead of full screen
    let use_focus_crop = effective_depth != "deep";

    let (image_path, is_focus_crop) = if use_focus_crop {
        match focus_capture::capture_focus_area() {
            Ok(capture) => (capture.image_path, true),
            Err(e) => {
                eprintln!(
                    "Focus capture failed, falling back to full screenshot: {}",
                    e
                );
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
