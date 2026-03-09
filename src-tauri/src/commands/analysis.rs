use crate::ai_engine::{self, AnalysisResult};
use crate::focus_capture;
use crate::ocr;
use crate::screen_context;
use crate::screenshot;
use crate::user_data;
use crate::AppState;
use tauri::{AppHandle, Emitter, Manager};

use super::settings::load_data;

#[tauri::command]
pub fn take_screenshot() -> Result<String, String> {
    let path = screenshot::capture_screen()?;
    path.to_str()
        .map(|s| s.to_string())
        .ok_or_else(|| "Invalid path".to_string())
}

/// Core analysis logic, usable from both the Tauri command and Rust-side auto-analysis.
/// Context-first: captures rich context via AX APIs, uses screenshot only as fallback.
pub async fn do_analyze(app: &AppHandle) -> Result<AnalysisResult, String> {
    let _ = app.emit("analysis-progress", "Capturing context...");
    let data = load_data(app);

    // Step 1: Capture screen context (fast, no screenshot)
    let mut ctx = screen_context::capture(app);

    // Step 2: Check blacklist
    if screen_context::is_blacklisted(&ctx.bundle_id, &data.settings.app_blacklist) {
        return Err("App is blacklisted".to_string());
    }

    // Step 3: Gather activity history and quests
    let recent = user_data::get_recent_activities(30).unwrap_or_default();

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

    // Step 4: Decide if screenshot is needed
    // - casual depth: no screenshot (just app tracking)
    // - deep depth: always screenshot (reading-heavy apps need visual context)
    // - normal depth: screenshot only if AX text is insufficient
    let need_screenshot = match ctx.depth.as_str() {
        "casual" => false,
        "deep" => true,
        _ => !ctx.has_sufficient_text(),
    };

    let _ = app.emit("analysis-progress", "Analyzing...");

    // Step 5: Take screenshot if needed
    let (screenshot_path, is_focus_crop) = if need_screenshot {
        let use_focus_crop = ctx.depth != "deep";
        if use_focus_crop {
            match focus_capture::capture_focus_area() {
                Ok(capture) => (Some(capture.image_path), true),
                Err(e) => {
                    eprintln!("Focus capture failed, trying full: {}", e);
                    (Some(screenshot::capture_screen()?), false)
                }
            }
        } else {
            (Some(screenshot::capture_screen()?), false)
        }
    } else {
        (None, false)
    };

    // Step 5b: If we took a screenshot but have no AX text, try OCR
    if screenshot_path.is_some() && ctx.ax_text.is_none() {
        if let Some(ref path) = screenshot_path {
            match ocr::recognize_text(path) {
                Ok(r) if !r.text.trim().is_empty() => ctx.ocr_text = Some(r.text),
                _ => {}
            }
        }
    }

    // Step 6: Call AI
    let mut result = if data.settings.ai_mode == "api" && !data.settings.api_key.is_empty() {
        ai_engine::analyze_with_api(
            screenshot_path.as_deref(),
            &data.settings.api_key,
            &data.settings.model,
            &data.settings.language,
            &recent,
            main_quest.as_deref(),
            &ctx,
            is_focus_crop,
        )
        .await
    } else {
        ai_engine::analyze_with_cli(
            screenshot_path.as_deref(),
            &data.settings.model,
            &data.settings.language,
            &recent,
            main_quest.as_deref(),
            &ctx,
            is_focus_crop,
        )
        .await
    }?;

    // Step 7: Second round if AI needs full context and we used focus crop
    if is_focus_crop && result.need_full_context == Some(true) {
        eprintln!("AI requested full context — performing second-round full screen analysis");
        let full_screenshot = screenshot::capture_screen()?;

        // Try OCR on full screenshot if no AX text
        if ctx.ax_text.is_none() {
            match ocr::recognize_text(&full_screenshot) {
                Ok(r) if !r.text.trim().is_empty() => ctx.ocr_text = Some(r.text),
                _ => {}
            }
        }

        result = if data.settings.ai_mode == "api" && !data.settings.api_key.is_empty() {
            ai_engine::analyze_with_api(
                Some(full_screenshot.as_path()),
                &data.settings.api_key,
                &data.settings.model,
                &data.settings.language,
                &recent,
                main_quest.as_deref(),
                &ctx,
                false, // not a focus crop anymore
            )
            .await
        } else {
            ai_engine::analyze_with_cli(
                Some(full_screenshot.as_path()),
                &data.settings.model,
                &data.settings.language,
                &recent,
                main_quest.as_deref(),
                &ctx,
                false,
            )
            .await
        }?;
    }

    // Step 8: Filter out suggested_quest if it duplicates an existing active quest
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
