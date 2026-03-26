use tauri::{AppHandle, Manager};

/// Handle second-instance activation by focusing the existing main window.
pub fn handle_second_instance(app: &AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
        window.unminimize().ok();
        window.set_focus().ok();
    }
}
