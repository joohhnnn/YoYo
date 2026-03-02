mod ai_engine;
mod commands;
mod screenshot;
mod window_monitor;

use tauri::{
    tray::TrayIconEvent, Manager, WebviewUrl, WebviewWindowBuilder,
};
use tauri_plugin_positioner::{Position, WindowExt};

pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_positioner::init())
        .plugin(tauri_plugin_global_shortcut::Builder::new().build())
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_shell::init())
        .setup(|app| {
            // Start window monitor
            window_monitor::start_monitoring(app.handle().clone());

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
        // Create the panel window
        let window = WebviewWindowBuilder::new(app, "main", WebviewUrl::App("index.html".into()))
            .title("YoYo")
            .inner_size(320.0, 560.0)
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
