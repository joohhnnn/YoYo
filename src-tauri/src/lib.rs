mod accessibility;
mod ai_engine;
mod commands;
mod focus_capture;
mod frame_diff;
mod ocr;
mod screenshot;
mod user_data;
mod window_list;
mod window_monitor;

use crate::ai_engine::AnalysisResult;
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
    pub current_app_pid: AtomicI64,
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

            // Pre-create speech-bubble window (hidden) so JS event listener is ready
            let _ = WebviewWindowBuilder::new(
                app,
                "speech-bubble",
                WebviewUrl::App("index.html".into()),
            )
            .title("YoYo Speech")
            .inner_size(280.0, 120.0)
            .resizable(false)
            .decorations(false)
            .transparent(true)
            .always_on_top(true)
            .visible(false)
            .skip_taskbar(true)
            .focused(false)
            .build();

            // Listen for speech-bubble events to show the speech bubble window
            let app_for_bubble_event = app.handle().clone();
            app.listen("speech-bubble", move |_event| {
                show_speech_bubble(&app_for_bubble_event);
            });

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
                                Ok(_) => {}
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
        let window = WebviewWindowBuilder::new(app, "bubble", WebviewUrl::App("index.html".into()))
            .title("YoYo Bubble")
            .inner_size(340.0, 200.0)
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

/// Create or show the speech bubble window next to the BubbleApp.
fn show_speech_bubble(app: &tauri::AppHandle) {
    if let Some(window) = app.get_webview_window("speech-bubble") {
        position_speech_bubble(&window, app);
        let _ = window.show();
    } else {
        match WebviewWindowBuilder::new(app, "speech-bubble", WebviewUrl::App("index.html".into()))
            .title("YoYo Speech")
            .inner_size(280.0, 120.0)
            .resizable(false)
            .decorations(false)
            .transparent(true)
            .always_on_top(true)
            .visible(false)
            .skip_taskbar(true)
            .focused(false)
            .build()
        {
            Ok(window) => {
                position_speech_bubble(&window, app);
                let _ = window.show();
            }
            Err(e) => eprintln!("Failed to create speech bubble window: {}", e),
        }
    }
}

fn position_speech_bubble(window: &tauri::WebviewWindow, app: &tauri::AppHandle) {
    // Position to the left of the BubbleApp
    if let Some(bubble) = app.get_webview_window("bubble") {
        if let Ok(pos) = bubble.outer_position() {
            let scale = bubble.scale_factor().unwrap_or(1.0);
            let x = (pos.x as f64 / scale) - 290.0;
            let y = (pos.y as f64 / scale) + 40.0;
            let _ = window.set_position(LogicalPosition::new(x, y));
            return;
        }
    }
    // Fallback: top-right minus offset
    if let Ok(Some(monitor)) = window.current_monitor() {
        let size = monitor.size();
        let scale = monitor.scale_factor();
        let x = (size.width as f64 / scale) - 650.0;
        let y = 80.0;
        let _ = window.set_position(LogicalPosition::new(x, y));
    }
}
