use std::sync::Arc;

use tauri::{AppHandle, Emitter};

use crate::state::UnifiedAppState;

pub(super) async fn start_recording_timer(
    app: AppHandle,
    state: Arc<UnifiedAppState>,
    recording_output_path: &str,
) {
    let started_at_ms = chrono::Utc::now().timestamp_millis();

    if let Err(e) =
        crate::commands::tray::update_main_tray_timer_cmd(app.clone(), "00:00:00".to_string()).await
    {
        println!("Warning: failed to set initial tray timer: {}", e);
    }

    let app_clone = app.clone();
    let state_clone = Arc::clone(&state);
    let timer_id = uuid::Uuid::new_v4().to_string()[..8].to_string();

    let timer_cancel_flag = Arc::new(std::sync::atomic::AtomicBool::new(false));
    let timer_cancel_flag_clone = Arc::clone(&timer_cancel_flag);

    let timer_handle = tokio::spawn(async move {
        let mut interval = tokio::time::interval(tokio::time::Duration::from_secs(1));

        loop {
            if timer_cancel_flag_clone.load(std::sync::atomic::Ordering::Acquire) {
                break;
            }

            match tokio::time::timeout(tokio::time::Duration::from_millis(500), interval.tick())
                .await
            {
                Ok(_) => {
                    if timer_cancel_flag_clone.load(std::sync::atomic::Ordering::Acquire) {
                        break;
                    }

                    let elapsed = chrono::Utc::now().timestamp_millis() - started_at_ms;
                    let seconds = elapsed / 1000;
                    let hours = seconds / 3600;
                    let minutes = (seconds % 3600) / 60;
                    let secs = seconds % 60;
                    let time_str = format!("{:02}:{:02}:{:02}", hours, minutes, secs);

                    state_clone.ui.set_tray_recording(Some(time_str.clone()));

                    if let Err(e) = crate::commands::tray::update_main_tray_timer_cmd(
                        app_clone.clone(),
                        time_str,
                    )
                    .await
                    {
                        println!(
                            "=== TRAY_TIMER[{}]: Failed to update tray: {} ===",
                            timer_id, e
                        );
                        break;
                    }
                }
                Err(_) => continue,
            }
        }
    });

    state.set_tray_timer_handle_with_flag(timer_handle, timer_cancel_flag);

    let payload = serde_json::json!({
        "started_at_ms": started_at_ms,
        "output_path": recording_output_path,
    });
    if let Err(e) = app.emit("recording:started", payload) {
        println!("Warning: failed to emit recording:started: {}", e);
    }
}
