mod ai_engine;
mod commands;
mod screenshot;
mod user_data;
mod window_monitor;

use crate::ai_engine::AnalysisResult;
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
    pub last_auto_analysis: AtomicI64,
}

pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_positioner::init())
        .plugin(tauri_plugin_global_shortcut::Builder::new().build())
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_shell::init())
        .manage(AppState {
            last_analysis: Mutex::new(None),
            last_auto_analysis: AtomicI64::new(0),
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

            // Listen for app-switch events and auto-analyze from Rust side.
            // This way analysis works even before the tray panel is opened.
            let app_for_switch = app.handle().clone();
            app.listen("app-switched", move |_event| {
                let app = app_for_switch.clone();
                let state = app.state::<AppState>();
                let now = chrono_millis();
                let last = state.last_auto_analysis.load(Ordering::Relaxed);
                if now - last < 12_000 {
                    return; // Cooldown: at least 12s between auto-analyses
                }
                state.last_auto_analysis.store(now, Ordering::Relaxed);

                // Debounce: wait 2s then analyze
                tauri::async_runtime::spawn(async move {
                    tokio::time::sleep(std::time::Duration::from_secs(2)).await;
                    match commands::do_analyze(&app).await {
                        Ok(result) => {
                            // Cache + broadcast + show bubble
                            if let Some(state) = app.try_state::<AppState>() {
                                if let Ok(mut cache) = state.last_analysis.lock() {
                                    *cache = Some(result.clone());
                                }
                            }
                            let _ = app.emit("analysis-complete", &result);
                            show_bubble(&app);
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
                .inner_size(340.0, 260.0)
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
