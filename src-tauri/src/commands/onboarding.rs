/// Check if AX (Accessibility) permission is granted.
#[tauri::command]
pub fn check_ax_permission() -> bool {
    crate::accessibility::is_ax_trusted()
}

/// Open macOS System Preferences > Accessibility pane.
#[tauri::command]
pub fn open_ax_settings() {
    crate::accessibility::open_ax_settings();
}
