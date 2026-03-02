mod ai_engine;
mod commands;
mod screenshot;
mod user_data;
mod window_monitor;

use crate::ai_engine::AnalysisResult;
use std::sync::Mutex;
use tauri::{
    tray::TrayIconEvent, LogicalPosition, Manager, WebviewUrl, WebviewWindowBuilder,
};
use tauri_plugin_positioner::{Position, WindowExt};

/// Shared app state for caching the latest analysis result.
pub struct AppState {
    pub last_analysis: Mutex<Option<AnalysisResult>>,
}

pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_positioner::init())
        .plugin(tauri_plugin_global_shortcut::Builder::new().build())
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_shell::init())
        .manage(AppState {
            last_analysis: Mutex::new(None),
        })
        .setup(|app| {
            // Start window monitor
            window_monitor::start_monitoring(app.handle().clone());

            // Initialize ~/.yoyo/ directory, profile.md, context.md, yoyo.db
            if let Err(e) = user_data::ensure_initialized() {
                eprintln!("Warning: failed to initialize user data: {}", e);
            }

            // Set activation policy to Accessory (hide from Dock)
            #[cfg(target_os = "macos")]
            {
                app.set_activation_policy(tauri::ActivationPolicy::Accessory);
            }

            Ok(())
        })
        .on_tray_icon_event(|app, event| {
            // Let positioner plugin handle tray events
            tauri_plugin_positioner::on_tray_event(app.app_handle(), &event);

            if let TrayIconEvent::Click { button_state, .. } = event {
                // Only toggle on press — the event fires for both press and
                // release, which would otherwise show then immediately hide.
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
        // Create the panel window (simplified: Status + Tasks only)
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

/// Create or show the floating action bubble at bottom-right of screen.
pub fn show_bubble(app: &tauri::AppHandle) {
    if let Some(window) = app.get_webview_window("bubble") {
        let _ = window.show();
        let _ = window.set_focus();
    } else {
        let window =
            WebviewWindowBuilder::new(app, "bubble", WebviewUrl::App("index.html".into()))
                .title("YoYo Bubble")
                .inner_size(320.0, 200.0)
                .resizable(false)
                .decorations(false)
                .transparent(true)
                .always_on_top(true)
                .visible(false)
                .skip_taskbar(true)
                .build()
                .expect("Failed to create bubble window");

        // Position at bottom-right of primary monitor
        if let Ok(Some(monitor)) = window.current_monitor() {
            let size = monitor.size();
            let scale = monitor.scale_factor();
            let x = (size.width as f64 / scale) - 340.0;
            let y = (size.height as f64 / scale) - 300.0;
            let _ = window.set_position(LogicalPosition::new(x, y));
        }

        let _ = window.show();
        let _ = window.set_focus();
    }
}
