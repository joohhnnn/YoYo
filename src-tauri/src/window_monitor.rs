use objc2_app_kit::{NSRunningApplication, NSWorkspace};
use objc2_foundation::NSString;
use std::ptr::NonNull;
use std::sync::atomic::Ordering;
use tauri::{AppHandle, Emitter};

use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct AppSwitchEvent {
    pub app_name: String,
    pub bundle_id: String,
    pub pid: i32,
}

/// Start monitoring for application switch events on macOS.
/// Emits "app-switched" event on the Tauri app handle when user switches apps.
pub fn start_monitoring(app_handle: AppHandle) {
    std::thread::spawn(move || {
        unsafe {
            let workspace = NSWorkspace::sharedWorkspace();
            let center = workspace.notificationCenter();

            let app_handle_clone = app_handle.clone();

            let notification_name =
                NSString::from_str("NSWorkspaceDidActivateApplicationNotification");

            center.addObserverForName_object_queue_usingBlock(
                Some(&notification_name),
                None,
                None,
                &block2::RcBlock::new(
                    move |notification: NonNull<objc2_foundation::NSNotification>| {
                        let notification = notification.as_ref();
                        let Some(user_info) = notification.userInfo() else {
                            return;
                        };

                        let key = NSString::from_str("NSWorkspaceApplicationKey");
                        let Some(app_obj) = user_info.objectForKey(&key) else {
                            return;
                        };

                        // Cast the AnyObject to NSRunningApplication via raw pointer
                        let app_ptr: *const NSRunningApplication =
                            objc2::rc::Retained::as_ptr(&app_obj) as *const NSRunningApplication;
                        let app: &NSRunningApplication = &*app_ptr;

                        let app_name = app
                            .localizedName()
                            .map(|n| n.to_string())
                            .unwrap_or_else(|| "Unknown".to_string());

                        let bundle_id = app
                            .bundleIdentifier()
                            .map(|b| b.to_string())
                            .unwrap_or_else(|| "unknown".to_string());

                        let pid = app.processIdentifier();

                        // Skip our own app (bundle ID in .app, name in dev mode)
                        if bundle_id == "com.yoyo.app" || app_name.eq_ignore_ascii_case("yoyo") {
                            return;
                        }

                        let event = AppSwitchEvent {
                            app_name,
                            bundle_id,
                            pid,
                        };

                        let _ = app_handle_clone.emit("app-switched", event);
                    },
                ),
            );
        }

        // Keep the thread alive to maintain the observer
        loop {
            std::thread::sleep(std::time::Duration::from_secs(3600));
        }
    });
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct TitleChangeEvent {
    pub app_name: String,
    pub bundle_id: String,
    pub old_title: String,
    pub new_title: String,
    pub url: Option<String>,
}

/// Poll for window title changes every 5 seconds.
/// When the frontmost window's title changes, store a lightweight record.
pub fn start_title_monitoring(app_handle: AppHandle) {
    use tauri::Manager;

    std::thread::spawn(move || {
        loop {
            std::thread::sleep(std::time::Duration::from_secs(5));

            let state = match app_handle.try_state::<crate::AppState>() {
                Some(s) => s,
                None => continue,
            };

            // Check auto-analyze setting
            if !crate::commands::get_auto_analyze(&app_handle) {
                continue;
            }

            let pid = state.current_app_pid.load(Ordering::Relaxed) as i32;
            if pid <= 0 {
                continue;
            }

            // Get current app info
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

            // Check blacklist
            {
                let data = crate::commands::settings::load_data(&app_handle);
                if crate::screen_context::is_blacklisted(&bundle_id, &data.settings.app_blacklist) {
                    continue;
                }
            }

            // Extract current window title via AX helper
            let (new_title, url) = match crate::accessibility::extract_text(pid) {
                Ok(result) if result.error.is_none() => (result.window_title, result.url),
                _ => continue,
            };

            if new_title.is_empty() {
                continue;
            }

            // Compare with cached title
            let title_changed = {
                let last = match state.last_window_title.lock() {
                    Ok(l) => l,
                    Err(e) => e.into_inner(),
                };
                *last != new_title
            };

            if !title_changed {
                continue;
            }

            // Update cached title and get old value
            let old_title = {
                let mut last = match state.last_window_title.lock() {
                    Ok(l) => l,
                    Err(e) => e.into_inner(),
                };
                let old = last.clone();
                *last = new_title.clone();
                old
            };

            // Skip first title after app switch (already handled by app-switch flow)
            if old_title.is_empty() {
                continue;
            }

            log::info!(
                "Window title changed: {:?} -> {:?} ({})",
                old_title,
                new_title,
                app_name
            );

            // Store lightweight record (no AI call)
            if let Err(e) = crate::user_data::insert_title_change(
                &app_name,
                &bundle_id,
                &new_title,
                url.as_deref(),
            ) {
                log::error!("Failed to store title change: {}", e);
            }

            // Emit event for frontend
            let event = TitleChangeEvent {
                app_name,
                bundle_id,
                old_title,
                new_title,
                url,
            };
            let _ = app_handle.emit("title-changed", event);
        }
    });
}
