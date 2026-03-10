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
}

pub fn run() {
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
        })
        .setup(|app| {
            // Start window monitor
            let app_handle = app.handle().clone();
            window_monitor::start_monitoring(app_handle.clone());

            // Initialize ~/.yoyo/ directory, profile.md, context.md, yoyo.db
            if let Err(e) = user_data::ensure_initialized() {
                eprintln!("Warning: failed to initialize user data: {}", e);
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

            // Position bubble at top-right
            position_bubble_top_right(&bubble);

            // Auto-show onboarding on first run
            {
                let data = commands::settings::load_data(&app.handle());
                if !data.settings.onboarding_completed {
                    let handle = app.handle().clone();
                    tauri::async_runtime::spawn(async move {
                        tokio::time::sleep(std::time::Duration::from_millis(500)).await;
                        toggle_settings(&handle);
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
                            eprintln!("Screen change detection failed, proceeding: {}", e);
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

                            // Cache + broadcast (bubble is always visible, React handles state)
                            if let Ok(mut cache) = state.last_analysis.lock() {
                                *cache = Some(result.clone());
                            }
                            let _ = app.emit("analysis-complete", &result);

                            // Record activity to observation log
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

                            match user_data::record_activity(
                                &app_name,
                                &bundle_id,
                                &result.context,
                                &actions_json,
                            ) {
                                Ok(_) => {}
                                Err(e) => eprintln!("Failed to record activity: {}", e),
                            }

                            // Knowledge extraction — piggyback on analysis
                            let ctx = screen_context::capture(&app);
                            if screen_context::is_learning_context(&ctx) {
                                let app_clone = app.clone();
                                let context_str = result.context.clone();
                                tauri::async_runtime::spawn(async move {
                                    match extract_and_store_knowledge(
                                        &app_clone,
                                        &ctx,
                                        &context_str,
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
                                            eprintln!("Knowledge extraction failed: {}", e)
                                        }
                                    }
                                });
                            }
                        }
                        Err(e) => eprintln!("Auto-analysis failed: {}", e),
                    }
                });
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
            commands::onboarding::check_ax_permission,
            commands::onboarding::open_ax_settings,
            commands::audio::list_audio_devices,
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

/// Toggle the settings window (tray icon click).
fn toggle_settings(app: &tauri::AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
        if window.is_visible().unwrap_or(false) {
            let _ = window.hide();
        } else {
            let _ = window.move_window(Position::TrayBottomCenter);
            let _ = window.show();
            let _ = window.set_focus();
        }
    } else {
        let window = WebviewWindowBuilder::new(app, "main", WebviewUrl::App("index.html".into()))
            .title("YoYo Settings")
            .inner_size(320.0, 400.0)
            .resizable(false)
            .decorations(false)
            .always_on_top(true)
            .visible(false)
            .skip_taskbar(true)
            .build()
            .expect("Failed to create settings window");

        let _ = window.move_window(Position::TrayBottomCenter);
        let _ = window.show();
        let _ = window.set_focus();
    }
}

/// Bubble is always visible — this is now a no-op.
/// Kept for backward compatibility with analyze_screen command.
pub fn show_bubble(_app: &tauri::AppHandle) {
    // Bubble is pre-created and always visible.
    // State transitions are handled by React via events.
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
// Knowledge extraction helpers (Phase 2.2)
// ---------------------------------------------------------------------------

/// Extract knowledge from screen context and store in DB.
/// Returns the number of items stored.
async fn extract_and_store_knowledge(
    app: &tauri::AppHandle,
    ctx: &screen_context::ScreenContext,
    analysis_context: &str,
) -> Result<usize, String> {
    let data = commands::settings::load_data(app);
    let prompt = ai_engine::build_knowledge_prompt(&data.settings.language, ctx, analysis_context);

    let response = if data.settings.ai_mode == "api" && !data.settings.api_key.is_empty() {
        ai_engine::simple_chat_api(&prompt, &data.settings.api_key, &data.settings.model).await?
    } else {
        ai_engine::simple_chat_cli(&prompt, &data.settings.model).await?
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
