//! Miscellaneous commands

use tauri::{AppHandle, Manager};

#[tauri::command]
pub fn log_to_terminal(message: String, level: Option<String>) {
    let prefix = match level.as_deref() {
        Some("error") => "[FRONTEND ERROR]",
        Some("warn") => "[FRONTEND WARN]",
        Some("info") => "[FRONTEND INFO]",
        _ => "[FRONTEND LOG]",
    };
    println!("{} {}", prefix, message);
}

#[tauri::command]
pub async fn show_capture_bar(app: AppHandle) -> Result<(), String> {
    if let Some(bar) = app.get_webview_window("capture-bar") {
        bar.show().map_err(|e| e.to_string())?;
        bar.set_focus().map_err(|e| e.to_string())?;
        println!("Capture bar shown and focused");
    } else {
        println!("Warning: capture-bar window not found");
    }
    Ok(())
}

#[tauri::command]
pub async fn reset_tray_to_idle(app: AppHandle) -> Result<(), String> {
    crate::commands::tray::reset_tray_to_idle_impl(app).await
}

#[tauri::command]
pub async fn exit(app: AppHandle) -> Result<(), String> {
    app.exit(0);
    Ok(())
}
