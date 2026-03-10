use objc2_app_kit::{NSRunningApplication, NSWorkspace};
use objc2_foundation::NSString;
use std::ptr::NonNull;
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
                        if bundle_id == "com.yoyo.app"
                            || app_name.eq_ignore_ascii_case("yoyo")
                        {
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
