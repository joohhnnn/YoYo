mod accessibility;
mod ai_engine;
mod commands;
mod focus_capture;
mod frame_diff;
mod ocr;
mod screen_context;
mod screenshot;
mod speech;
mod user_data;
mod window_list;
mod window_monitor;

use crate::ai_engine::AnalysisResult;
use crate::window_monitor::AppSwitchEvent;
use std::sync::atomic::{AtomicBool, AtomicI64, Ordering};
use std::sync::{Arc, Mutex};
use tauri::{
    tray::TrayIconEvent, Emitter, Listener, LogicalPosition, Manager, WebviewUrl,
    WebviewWindowBuilder,
};
use tauri_plugin_positioner::{Position, WindowExt};

/// State for an active audio recording session.
pub struct RecordingState {
    pub _stream: cpal::Stream,
    pub writer: Arc<Mutex<Option<hound::WavWriter<std::io::BufWriter<std::fs::File>>>>>,
    pub file_path: String,
    pub start_time: std::time::Instant,
}

// cpal::Stream is not Send by default but we manage it safely via Mutex
unsafe impl Send for RecordingState {}

/// Shared app state for caching the latest analysis result.
pub struct AppState {
    pub last_analysis: Mutex<Option<AnalysisResult>>,
    pub debounce_counter: AtomicI64,
    pub last_analysis_time: AtomicI64,
    // Current foreground app (updated on app-switch)
    pub current_app_name: Mutex<String>,
    pub current_bundle_id: Mutex<String>,
    pub current_app_pid: AtomicI64,
    // Abort flag for cancelling plan execution mid-step
    pub abort_flag: AtomicBool,
    // Active audio recording session
    pub recording: Mutex<Option<RecordingState>>,
    // Last nudge emission time (millis) — 30min cooldown
    pub last_nudge_time: AtomicI64,
    // Last known window title (for title-change detection)
    pub last_window_title: Mutex<String>,
}

pub fn run() {
    // Initialize file logger to ~/.yoyo/yoyo.log
    if let Ok(dir) = user_data::yoyo_dir() {
        let log_path = dir.join("yoyo.log");
        // Truncate if over 1 MB
        if let Ok(meta) = std::fs::metadata(&log_path) {
            if meta.len() > 1_048_576 {
                let _ = std::fs::write(&log_path, "");
            }
        }
        if let Ok(file) = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&log_path)
        {
            let _ = simplelog::WriteLogger::init(
                simplelog::LevelFilter::Info,
                simplelog::Config::default(),
                file,
            );
        }
    }

    tauri::Builder::default()
        .plugin(tauri_plugin_positioner::init())
        .plugin(tauri_plugin_global_shortcut::Builder::new().build())
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_shell::init())
        .manage(AppState {
            last_analysis: Mutex::new(None),
            debounce_counter: AtomicI64::new(0),
            last_analysis_time: AtomicI64::new(0),
            current_app_name: Mutex::new(String::new()),
            current_bundle_id: Mutex::new(String::new()),
            current_app_pid: AtomicI64::new(0),
            abort_flag: AtomicBool::new(false),
            recording: Mutex::new(None),
            last_nudge_time: AtomicI64::new(0),
            last_window_title: Mutex::new(String::new()),
        })
        .setup(|app| {
            // Start window monitor
            let app_handle = app.handle().clone();
            window_monitor::start_monitoring(app_handle.clone());
            window_monitor::start_title_monitoring(app_handle.clone());

            // Initialize ~/.yoyo/ directory, profile.md, context.md, yoyo.db
            if let Err(e) = user_data::ensure_initialized() {
                log::warn!("Failed to initialize user data: {}", e);
            }

            // Prune old activity/execution records on startup
            if let Err(e) = user_data::cleanup_old_data() {
                log::warn!("Disk cleanup failed: {}", e);
            }

            // Set activation policy to Accessory (hide from Dock)
            #[cfg(target_os = "macos")]
            {
                app.set_activation_policy(tauri::ActivationPolicy::Accessory);
            }

            // Pre-create bubble window (always running, starts in ambient dot state)
            let bubble =
                WebviewWindowBuilder::new(app, "bubble", WebviewUrl::App("index.html".into()))
                    .title("YoYo")
                    .inner_size(48.0, 48.0)
                    .resizable(false)
                    .decorations(false)
                    .transparent(true)
                    .always_on_top(true)
                    .visible_on_all_workspaces(true)
                    .visible(true)
                    .skip_taskbar(true)
                    .focused(false)
                    .build()
                    .expect("Failed to create bubble window");

            // Restore saved position or default to top-right
            {
                let data = commands::settings::load_data(&app.handle());
                if let (Some(x), Some(y)) = (data.settings.bubble_x, data.settings.bubble_y) {
                    let _ = bubble.set_position(tauri::PhysicalPosition::new(x as i32, y as i32));
                } else {
                    position_bubble_top_right(&bubble);
                }
            }

            // Auto-show onboarding on first run
            {
                let data = commands::settings::load_data(&app.handle());
                if !data.settings.onboarding_completed {
                    let handle = app.handle().clone();
                    tauri::async_runtime::spawn(async move {
                        tokio::time::sleep(std::time::Duration::from_millis(500)).await;
                        show_settings_centered(&handle);
                    });
                }
            }

            // Listen for app-switch events and auto-analyze from Rust side.
            // Uses debounce counter: only the latest switch triggers analysis after settling.
            let app_for_switch = app.handle().clone();
            app.listen("app-switched", move |event| {
                let app = app_for_switch.clone();
                let state = app.state::<AppState>();

                // Update current app info from event payload
                if let Ok(payload) = serde_json::from_str::<AppSwitchEvent>(event.payload()) {
                    if let Ok(mut name) = state.current_app_name.lock() {
                        *name = payload.app_name;
                    }
                    if let Ok(mut bid) = state.current_bundle_id.lock() {
                        *bid = payload.bundle_id;
                    }
                    state
                        .current_app_pid
                        .store(payload.pid as i64, Ordering::Relaxed);
                    // Reset window title on app switch (first poll will set it)
                    if let Ok(mut title) = state.last_window_title.lock() {
                        *title = String::new();
                    }
                }

                // Increment counter; only the latest event will match after debounce
                let my_counter = state.debounce_counter.fetch_add(1, Ordering::Relaxed) + 1;

                tauri::async_runtime::spawn(async move {
                    // Wait for user to settle on an app
                    tokio::time::sleep(std::time::Duration::from_secs(2)).await;

                    let state = app.state::<AppState>();

                    // If counter changed, a newer switch happened — skip this one
                    if state.debounce_counter.load(Ordering::Relaxed) != my_counter {
                        return;
                    }

                    // Check if auto-analyze is enabled
                    if !commands::get_auto_analyze(&app) {
                        return;
                    }

                    // Check app blacklist before any analysis
                    {
                        let bundle_id = state
                            .current_bundle_id
                            .lock()
                            .map(|b| b.clone())
                            .unwrap_or_default();
                        let data = commands::settings::load_data(&app);
                        if screen_context::is_blacklisted(&bundle_id, &data.settings.app_blacklist)
                        {
                            return;
                        }
                    }

                    // Quick screen change detection: take 2 frames 500ms apart.
                    // If screen is changing rapidly (typing, scrolling), wait 2s more.
                    match frame_diff::is_screen_changing(12) {
                        Ok(true) => {
                            // Screen is actively changing — wait a bit more
                            tokio::time::sleep(std::time::Duration::from_secs(2)).await;
                            // Re-check debounce (user may have switched apps during the wait)
                            if state.debounce_counter.load(Ordering::Relaxed) != my_counter {
                                return;
                            }
                        }
                        Ok(false) => {} // Screen is calm, proceed
                        Err(e) => {
                            log::warn!("Screen change detection failed, proceeding: {}", e);
                        }
                    }

                    // Check cooldown against last completed analysis
                    let now = chrono_millis();
                    let last = state.last_analysis_time.load(Ordering::Relaxed);
                    let cooldown_ms = (commands::get_cooldown_secs(&app) as i64) * 1000;
                    if now - last < cooldown_ms {
                        return;
                    }

                    match commands::do_analyze(&app).await {
                        Ok(result) => {
                            // Update last analysis timestamp
                            state
                                .last_analysis_time
                                .store(chrono_millis(), Ordering::Relaxed);

                            // Load scene to determine mode
                            let scene_data = commands::settings::load_data(&app);
                            let scene = scene_data.settings.current_scene.clone();

                            // Cache result always
                            if let Ok(mut cache) = state.last_analysis.lock() {
                                *cache = Some(result.clone());
                            }

                            // Only show bubble if scene is set (observation mode = silent)
                            if scene.is_some() {
                                let _ = app.emit("analysis-complete", &result);
                                play_sound_if_enabled(&app, "Tink");
                            }

                            // Record activity always (both modes)
                            let app_name = state
                                .current_app_name
                                .lock()
                                .map(|n| n.clone())
                                .unwrap_or_default();
                            let bundle_id = state
                                .current_bundle_id
                                .lock()
                                .map(|b| b.clone())
                                .unwrap_or_default();
                            let actions_json =
                                serde_json::to_string(&result.actions).unwrap_or_default();

                            // Capture context ONCE (reused for raw_context + knowledge extraction)
                            let ctx = screen_context::capture(&app);

                            match user_data::record_activity(
                                &app_name,
                                &bundle_id,
                                &result.context,
                                &actions_json,
                            ) {
                                Ok(true) => {
                                    // Store raw context alongside activity
                                    if let Err(e) = user_data::insert_raw_context(
                                        &ctx.app_name,
                                        &ctx.bundle_id,
                                        &ctx.window_title,
                                        ctx.url.as_deref(),
                                        ctx.ax_text.as_deref(),
                                        ctx.ocr_text.as_deref(),
                                        ctx.selected_text.as_deref(),
                                        &ctx.depth,
                                        "analysis",
                                    ) {
                                        log::error!("Failed to store raw context: {}", e);
                                    }

                                    // New record inserted — check if summarization needed
                                    let app_for_summary = app.clone();
                                    tauri::async_runtime::spawn(async move {
                                        maybe_run_summarization(&app_for_summary).await;
                                    });
                                }
                                Ok(false) => {} // deduplicated
                                Err(e) => log::error!("Failed to record activity: {}", e),
                            }

                            // Knowledge extraction — reuse ctx captured above
                            if screen_context::is_learning_context(&ctx, scene.as_deref()) {
                                let app_clone = app.clone();
                                let context_str = result.context.clone();
                                tauri::async_runtime::spawn(async move {
                                    match extract_and_store_knowledge(
                                        &app_clone,
                                        &ctx,
                                        &context_str,
                                        scene.as_deref(),
                                    )
                                    .await
                                    {
                                        Ok(count) if count > 0 => {
                                            if should_nudge(&app_clone) {
                                                let _ = app_clone.emit("nudge-available", count);
                                            }
                                        }
                                        Ok(_) => {}
                                        Err(e) => {
                                            log::error!("Knowledge extraction failed: {}", e)
                                        }
                                    }
                                });
                            }
                        }
                        Err(e) => {
                            log::error!("Auto-analysis failed: {}", e);
                            play_sound_if_enabled(&app, "Basso");
                        }
                    }
                });
            });

            // Save bubble position on move (throttled)
            let move_handle = app.handle().clone();
            let last_save = std::sync::Arc::new(AtomicI64::new(0));
            bubble.on_window_event(move |event| {
                if let tauri::WindowEvent::Moved(pos) = event {
                    let now = chrono_millis();
                    let prev = last_save.load(Ordering::Relaxed);
                    if now - prev < 500 {
                        return; // throttle: at most every 500ms
                    }
                    last_save.store(now, Ordering::Relaxed);
                    let x = pos.x as f64;
                    let y = pos.y as f64;
                    let app = move_handle.clone();
                    std::thread::spawn(move || {
                        let mut data = commands::settings::load_data(&app);
                        data.settings.bubble_x = Some(x);
                        data.settings.bubble_y = Some(y);
                        let _ = commands::settings::save_data(&app, &data);
                    });
                }
            });

            Ok(())
        })
        .on_tray_icon_event(|app, event| {
            // Let positioner plugin handle tray events
            tauri_plugin_positioner::on_tray_event(app.app_handle(), &event);

            if let TrayIconEvent::Click { button_state, .. } = event {
                if matches!(button_state, tauri::tray::MouseButtonState::Down) {
                    let app_handle = app.app_handle().clone();
                    toggle_settings(&app_handle);
                }
            }
        })
        .invoke_handler(tauri::generate_handler![
            commands::analysis::take_screenshot,
            commands::analysis::analyze_screen,
            commands::actions::execute_action,
            commands::actions::start_execution,
            commands::actions::cancel_execution,
            commands::actions::check_inserted_text,
            commands::audio::check_voice_permission,
            commands::audio::request_voice_permission,
            commands::audio::start_recording,
            commands::audio::stop_and_transcribe,
            commands::settings::get_settings,
            commands::settings::save_settings,
            commands::settings::get_tasks,
            commands::settings::save_tasks,
            commands::settings::get_profile,
            commands::settings::save_profile,
            commands::settings::get_context,
            commands::settings::save_context,
            commands::analysis::get_last_analysis,
            commands::activity::get_recent_activities,
            commands::activity::search_raw_context,
            commands::intent::understand_intent,
            commands::workflow::record_execution,
            commands::workflow::complete_execution,
            commands::workflow::feedback_execution,
            commands::workflow::save_workflow,
            commands::workflow::get_workflows,
            commands::workflow::delete_workflow,
            commands::workflow::update_workflow_count,
            commands::workflow::get_recent_executions,
            commands::knowledge::get_due_knowledge,
            commands::knowledge::get_knowledge_by_kind,
            commands::knowledge::review_knowledge,
            commands::knowledge::delete_knowledge,
            commands::knowledge::get_knowledge_stats,
            commands::knowledge::generate_note,
            commands::onboarding::check_ax_permission,
            commands::onboarding::open_ax_settings,
            commands::onboarding::open_mic_settings,
            commands::audio::list_audio_devices,
            commands::settings::play_sound,
            commands::settings::set_scene,
        ])
        .run(tauri::generate_context!())
        .expect("error while running YoYo");
}

fn chrono_millis() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as i64
}

/// Position the window near the tray icon, falling back to screen center.
fn move_window_near_tray(window: &tauri::WebviewWindow) {
    use std::panic::{catch_unwind, AssertUnwindSafe};
    let w = AssertUnwindSafe(window);
    if catch_unwind(move || {
        let _ = w.move_window(Position::TrayBottomCenter);
    })
    .is_err()
    {
        let _ = window.move_window(Position::Center);
    }
}

/// Create or get the settings window (shared by toggle and onboarding).
fn get_or_create_settings(app: &tauri::AppHandle) -> tauri::WebviewWindow {
    if let Some(window) = app.get_webview_window("main") {
        window
    } else {
        WebviewWindowBuilder::new(app, "main", WebviewUrl::App("index.html".into()))
            .title("YoYo Settings")
            .inner_size(320.0, 400.0)
            .resizable(false)
            .decorations(false)
            .always_on_top(true)
            .visible(false)
            .skip_taskbar(true)
            .build()
            .expect("Failed to create settings window")
    }
}

/// Toggle the settings window (tray icon click).
fn toggle_settings(app: &tauri::AppHandle) {
    let window = get_or_create_settings(app);
    if window.is_visible().unwrap_or(false) {
        let _ = window.hide();
    } else {
        move_window_near_tray(&window);
        let _ = window.show();
        let _ = window.set_focus();
    }
}

/// Show the settings window centered (for onboarding, before any tray click).
fn show_settings_centered(app: &tauri::AppHandle) {
    let window = get_or_create_settings(app);
    let _ = window.move_window(Position::Center);
    let _ = window.show();
    let _ = window.set_focus();
}

/// Bubble is always visible — this is now a no-op.
/// Kept for backward compatibility with analyze_screen command.
pub fn show_bubble(_app: &tauri::AppHandle) {
    // Bubble is pre-created and always visible.
    // State transitions are handled by React via events.
}

/// Play a macOS system sound if sound_enabled is true.
fn play_sound_if_enabled(app: &tauri::AppHandle, sound: &str) {
    let data = commands::settings::load_data(app);
    if !data.settings.sound_enabled {
        return;
    }
    let path = format!("/System/Library/Sounds/{}.aiff", sound);
    std::thread::spawn(move || {
        let _ = std::process::Command::new("afplay").arg(&path).output();
    });
}

fn position_bubble_top_right(window: &tauri::WebviewWindow) {
    if let Ok(Some(monitor)) = window.current_monitor() {
        let size = monitor.size();
        let scale = monitor.scale_factor();
        let x = (size.width as f64 / scale) - 68.0; // 48px dot + 20px margin
        let y = 40.0; // Below menu bar
        let _ = window.set_position(LogicalPosition::new(x, y));
    }
}

// ---------------------------------------------------------------------------
// Progressive summarization
// ---------------------------------------------------------------------------

/// Check if enough unsummarized activities have accumulated, and if so,
/// generate a new rolling summary in the background.
async fn maybe_run_summarization(app: &tauri::AppHandle) {
    const SUMMARIZE_THRESHOLD: i64 = 10;

    let unsummarized = match user_data::count_unsummarized() {
        Ok(n) => n,
        Err(e) => {
            log::error!("count_unsummarized failed: {}", e);
            return;
        }
    };

    if unsummarized < SUMMARIZE_THRESHOLD {
        return;
    }

    log::info!(
        "Triggering progressive summarization ({} unsummarized records)",
        unsummarized
    );

    let prev_summary = user_data::get_latest_summary().unwrap_or(None);
    let last_id = prev_summary
        .as_ref()
        .map(|s| s.last_activity_id)
        .unwrap_or(0);
    let prev_total = prev_summary
        .as_ref()
        .map(|s| s.total_summarized)
        .unwrap_or(0);
    let prev_text = prev_summary.as_ref().map(|s| s.summary_text.as_str());

    let new_activities = match user_data::get_activities_since(last_id) {
        Ok(a) => a,
        Err(e) => {
            log::error!("get_activities_since failed: {}", e);
            return;
        }
    };

    if new_activities.is_empty() {
        return;
    }

    let new_last_id = new_activities.last().unwrap().id;
    let new_count = new_activities.len() as i64;

    let prompt = ai_engine::build_summarize_prompt(prev_text, &new_activities);
    let data = commands::settings::load_data(app);

    let summary_result = if data.settings.ai_mode == "api" && !data.settings.api_key.is_empty() {
        ai_engine::simple_chat_api(&prompt, &data.settings.api_key, &data.settings.model, None)
            .await
    } else {
        ai_engine::simple_chat_cli(&prompt, &data.settings.model, None).await
    };

    match summary_result {
        Ok(text) => {
            let trimmed = text.trim().to_string();
            if trimmed.is_empty() {
                log::warn!("Summarization returned empty text, skipping");
                return;
            }
            match user_data::insert_summary(
                &trimmed,
                new_last_id,
                new_count,
                prev_total + new_count,
            ) {
                Ok(id) => log::info!(
                    "Saved activity summary #{} (covers {} total records)",
                    id,
                    prev_total + new_count
                ),
                Err(e) => log::error!("Failed to save summary: {}", e),
            }
        }
        Err(e) => {
            log::error!("Summarization AI call failed: {}", e);
        }
    }
}

// ---------------------------------------------------------------------------
// Knowledge extraction helpers (Phase 2.2)
// ---------------------------------------------------------------------------

/// Extract knowledge from screen context and store in DB.
/// Returns the number of items stored.
async fn extract_and_store_knowledge(
    app: &tauri::AppHandle,
    ctx: &screen_context::ScreenContext,
    analysis_context: &str,
    current_scene: Option<&str>,
) -> Result<usize, String> {
    let data = commands::settings::load_data(app);
    let prompt = ai_engine::build_knowledge_prompt(
        &data.settings.language,
        ctx,
        analysis_context,
        current_scene,
    );

    let response = if data.settings.ai_mode == "api" && !data.settings.api_key.is_empty() {
        ai_engine::simple_chat_api(&prompt, &data.settings.api_key, &data.settings.model, None)
            .await?
    } else {
        ai_engine::simple_chat_cli(&prompt, &data.settings.model, None).await?
    };

    let json_str = ai_engine::extract_json_block(&response);
    let extraction: ai_engine::KnowledgeExtractionResult = serde_json::from_str(json_str)
        .map_err(|e| format!("Failed to parse knowledge extraction: {}", e))?;

    let source = format!(
        "{} ({})",
        ctx.app_name,
        ctx.url.as_deref().unwrap_or("screen")
    );

    let mut stored = 0;
    for item in &extraction.items {
        if user_data::knowledge_exists(&item.kind, &item.content)? {
            continue;
        }

        let now = chrono::Local::now().naive_local();
        let next_review = now + chrono::Duration::hours(1);

        let metadata = serde_json::json!({
            "definition": item.definition,
            "review_count": 0,
            "interval_level": 0,
            "next_review": next_review.format("%Y-%m-%d %H:%M:%S").to_string(),
            "last_reviewed": null
        });

        user_data::insert_knowledge(&item.kind, &item.content, &source, &metadata.to_string())?;
        stored += 1;
    }

    Ok(stored)
}

/// Check if a nudge should be emitted (30min cooldown + due items exist).
fn should_nudge(app: &tauri::AppHandle) -> bool {
    if let Some(state) = app.try_state::<AppState>() {
        let now = chrono_millis();
        let last = state.last_nudge_time.load(Ordering::Relaxed);
        let cooldown = 30 * 60 * 1000; // 30 minutes in ms

        if now - last >= cooldown {
            if let Ok(due) = user_data::get_due_knowledge(1) {
                if !due.is_empty() {
                    state.last_nudge_time.store(now, Ordering::Relaxed);
                    return true;
                }
            }
        }
    }
    false
}
