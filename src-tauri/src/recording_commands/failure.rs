use crate::{
    commands::STOPPING_RECORDING,
    state::{InterfaceMode, UnifiedAppState},
};
use std::sync::{Arc, atomic::Ordering};
use tauri::{AppHandle, Emitter};

pub(super) async fn watch_recording(app: AppHandle, state: Arc<UnifiedAppState>) {
    let Some(mut failure) = state.recording.failure_receiver().await else {
        return;
    };
    tokio::spawn(async move {
        let error = loop {
            if let Some(error) = failure.borrow().clone() {
                break error;
            }
            if failure.changed().await.is_err() {
                return;
            }
        };
        if STOPPING_RECORDING
            .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
            .is_err()
        {
            return;
        }
        struct StopGuard;
        impl Drop for StopGuard {
            fn drop(&mut self) {
                STOPPING_RECORDING.store(false, Ordering::Release);
            }
        }
        let _guard = StopGuard;
        let Some(path) = state.recording.abort_failed_recording(&failure).await else {
            return;
        };
        if state.is_camera_enabled() {
            let _ =
                crate::commands::input::stop_webview_webcam_recording(&app, &path, &state).await;
        }
        crate::commands::lifecycle::release_recording_surfaces(
            &app,
            Some(&state),
            "recording failed",
        );
        state.ui.set_interface_mode(InterfaceMode::Setup);
        state.ui.set_tray_idle();
        let _ = crate::commands::tray::reset_tray_to_idle_impl(app.clone()).await;
        super::ui::restore_ui_elements(&app).await;
        let _ = app.emit("recording:failed", error);
    });
}
