//! Shared cleanup for transient recording UI and input capture.

use std::sync::Arc;

use tauri::{AppHandle, Manager};

use crate::state::UnifiedAppState;

pub fn release_recording_surfaces(
    app: &AppHandle,
    state: Option<&Arc<UnifiedAppState>>,
    reason: &str,
) {
    println!("Releasing transient recording surfaces: {}", reason);

    if let Some(state) = state {
        state.cancel_tray_timer();
        state.get_mouse_tracker().lock().stop_tracking();
    }

    for label in ["display-preview", "webcam-preview", "recording-hud"] {
        if let Some(window) = app.get_webview_window(label) {
            let _ = window.set_always_on_top(false);
            let _ = window.hide();
        }
    }

    if let Some(bar) = app.get_webview_window("capture-bar") {
        let _ = bar.set_always_on_top(false);
        let _ = bar.hide();
    }

    if let Some(editor) = app.get_webview_window("editor") {
        let _ = editor.set_always_on_top(false);
    }
}
