mod ai_engine;
mod commands;
mod frame_diff;
mod screenshot;
mod user_data;
mod window_monitor;

use crate::ai_engine::AnalysisResult;
use crate::commands::ChatMessage;
use crate::window_monitor::AppSwitchEvent;
use std::sync::atomic::{AtomicI64, Ordering};
use std::sync::Mutex;
use tauri::{
    tray::TrayIconEvent, Emitter, Listener, LogicalPosition, Manager, WebviewUrl,
    WebviewWindowBuilder,
};
use tauri_plugin_positioner::{Position, WindowExt};

/// Shared app state for caching the latest analysis result.
pub struct AppState {
    pub last_analysis: Mutex<Option<AnalysisResult>>,
    pub debounce_counter: AtomicI64,
    pub last_analysis_time: AtomicI64,
    // Current foreground app (updated on app-switch)
    pub current_app_name: Mutex<String>,
    pub current_bundle_id: Mutex<String>,
    // Observation mode: count new activities since last reflection
    pub activities_since_reflection: AtomicI64,
    // Onboarding state
    pub onboarding_active: Mutex<bool>,
    pub onboarding_history: Mutex<Vec<ChatMessage>>,
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
            activities_since_reflection: AtomicI64::new(0),
            onboarding_active: Mutex::new(false),
            onboarding_history: Mutex::new(Vec::new()),
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

            // Restore reflection counter from DB on startup
            {
                let state = app.state::<AppState>();
                if let Ok(total) = user_data::get_total_activity_count() {
                    if let Ok(Some(last_ref)) = user_data::get_latest_reflection() {
                        let diff = total - last_ref.activity_count;
                        state
                            .activities_since_reflection
                            .store(diff.max(0), Ordering::Relaxed);
                    }
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

                            // Cache + broadcast + show bubble
                            if let Ok(mut cache) = state.last_analysis.lock() {
                                *cache = Some(result.clone());
                            }
                            let _ = app.emit("analysis-complete", &result);
                            show_bubble(&app);

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
                                Ok(true) => {
                                    // New distinct activity — check reflection trigger
                                    let count = state
                                        .activities_since_reflection
                                        .fetch_add(1, Ordering::Relaxed)
                                        + 1;
                                    if count >= 30 {
                                        state
                                            .activities_since_reflection
                                            .store(0, Ordering::Relaxed);
                                        let app_for_reflect = app.clone();
                                        tauri::async_runtime::spawn(async move {
                                            if let Err(e) =
                                                commands::trigger_reflection(&app_for_reflect).await
                                            {
                                                eprintln!("Reflection failed: {}", e);
                                            }
                                        });
                                    }
                                }
                                Ok(false) => {} // Deduplicated, no action
                                Err(e) => eprintln!("Failed to record activity: {}", e),
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
                    toggle_panel(&app_handle);
                }
            }
        })
        .invoke_handler(tauri::generate_handler![
            commands::take_screenshot,
            commands::analyze_screen,
            commands::execute_action,
            commands::get_settings,
            commands::save_settings,
            commands::get_tasks,
            commands::save_tasks,
            commands::get_profile,
            commands::save_profile,
            commands::get_context,
            commands::save_context,
            commands::get_last_analysis,
            commands::check_needs_onboarding,
            commands::start_onboarding,
            commands::send_onboarding_message,
            commands::finish_onboarding,
            commands::get_recent_activities,
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

fn toggle_panel(app: &tauri::AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
        if window.is_visible().unwrap_or(false) {
            let _ = window.hide();
        } else {
            let _ = window.move_window(Position::TrayBottomCenter);
            let _ = window.show();
            let _ = window.set_focus();
        }
    } else {
        // Create the panel window on first toggle
        let window = WebviewWindowBuilder::new(app, "main", WebviewUrl::App("index.html".into()))
            .title("YoYo")
            .inner_size(320.0, 400.0)
            .resizable(false)
            .decorations(false)
            .always_on_top(true)
            .visible(false)
            .skip_taskbar(true)
            .build()
            .expect("Failed to create panel window");

        let _ = window.move_window(Position::TrayBottomCenter);
        let _ = window.show();
        let _ = window.set_focus();
    }
}

/// Create or show the floating action bubble at top-right of screen.
/// Does NOT steal focus — the user can keep typing.
pub fn show_bubble(app: &tauri::AppHandle) {
    if let Some(window) = app.get_webview_window("bubble") {
        position_bubble_top_right(&window);
        let _ = window.show();
    } else {
        let window =
            WebviewWindowBuilder::new(app, "bubble", WebviewUrl::App("index.html".into()))
                .title("YoYo Bubble")
                .inner_size(340.0, 380.0)
                .resizable(false)
                .decorations(false)
                .transparent(true)
                .always_on_top(true)
                .visible_on_all_workspaces(true)
                .visible(false)
                .skip_taskbar(true)
                .focused(false)
                .build()
                .expect("Failed to create bubble window");

        position_bubble_top_right(&window);
        let _ = window.show();
    }
}

fn position_bubble_top_right(window: &tauri::WebviewWindow) {
    if let Ok(Some(monitor)) = window.current_monitor() {
        let size = monitor.size();
        let scale = monitor.scale_factor();
        let x = (size.width as f64 / scale) - 360.0;
        let y = 40.0; // Below menu bar
        let _ = window.set_position(LogicalPosition::new(x, y));
    }
}
