use anyhow::Result;
use std::path::Path;
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};
use tauri::{AppHandle, Emitter, Manager, State};

use crate::commands::STOPPING_RECORDING;
use crate::state::UnifiedAppState;

mod target;
mod timer;
mod ui;
use target::{
    build_recording_config, normalize_webcam_shape, resolve_recording_target,
    validate_recording_target,
};
use timer::start_recording_timer;
use ui::{hide_ui_elements, restore_ui_elements};

static STARTING_RECORDING: AtomicBool = AtomicBool::new(false);

#[tauri::command]
pub async fn record_start_new(
    target_type: String,
    target_id: String,
    quality: String,
    include_cursor: bool,
    include_microphone: bool,
    include_system_audio: bool,
    webcam_shape: Option<String>,
    output_path: Option<String>,
    app: AppHandle,
    state: State<'_, Arc<UnifiedAppState>>,
) -> Result<(), String> {
    if STARTING_RECORDING
        .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
        .is_err()
    {
        return Err("Recording is already starting".to_string());
    }
    struct StartGuard;
    impl Drop for StartGuard {
        fn drop(&mut self) {
            STARTING_RECORDING.store(false, Ordering::SeqCst);
        }
    }
    let _start_guard = StartGuard;
    println!("Starting recording with new architecture");

    if state.recording.is_recording() {
        return Err("Recording is already active".to_string());
    }

    #[cfg(target_os = "macos")]
    {
        use crate::permissions::check_permissions;
        let permissions = check_permissions();
        if !permissions.screen_recording_granted {
            println!("Screen recording permission missing; requesting access...");
            let granted = crate::permissions::request_screen_recording_permission()
                .map_err(|e| format!("Failed to request screen recording permission: {}", e))?;

            if !granted {
                let _ = crate::permissions::open_screen_recording_preferences();
                println!("Recording failed: Screen recording permission denied");
                return Err("Screen recording permission required. Enable the app that launched Tarantino in System Settings > Privacy & Security > Screen & System Audio Recording, then fully quit and reopen that terminal.".to_string());
            }
        }
        println!("Screen recording permission verified ✅");
    }

    let (target_type, target_id) =
        resolve_recording_target(state.inner().as_ref(), target_type, target_id)?;
    let webcam_shape = normalize_webcam_shape(webcam_shape.as_deref().unwrap_or("circle"));
    state.set_webcam_shape(webcam_shape.clone());

    let (effective_quality, maybe_mic_device) = {
        let app_read = state.app.read();
        (
            if quality.eq_ignore_ascii_case("Default") {
                app_read.settings.default_quality.clone()
            } else {
                quality
            },
            app_read.microphone_config.device_id.clone(),
        )
    };

    let mut recording_config = build_recording_config(
        target_type,
        target_id,
        effective_quality,
        include_cursor,
        include_microphone,
        include_system_audio,
        output_path,
    )?;

    if recording_config.include_microphone && recording_config.microphone_device.is_none() {
        recording_config.microphone_device = maybe_mic_device;
    }

    validate_recording_target(&state, &recording_config)?;

    let recording_output_path = recording_config.output_path.clone();

    if state.is_camera_enabled() {
        let _ = crate::commands::input::start_webview_webcam_recording(
            &app,
            &recording_output_path,
            state.inner(),
        )
        .await;
    }

    #[cfg(target_os = "macos")]
    if state.is_camera_enabled() {
        let mut capture_guard = state.webcam_capture.lock();
        if let Some(ref mut capture) = *capture_guard {
            capture.hide_preview();
            println!("[Webcam] Native preview hidden before screen capture start");
        }
    }

    if let Err(e) = hide_ui_elements(&app).await {
        #[cfg(target_os = "macos")]
        if state.is_camera_enabled() {
            let mut capture_guard = state.webcam_capture.lock();
            if let Some(ref mut capture) = *capture_guard {
                capture.show_preview();
            }
        }
        restore_ui_elements(&app).await;
        return Err(e);
    }

    if let Err(e) = state.start_recording(recording_config).await {
        println!("Recording failed during native start: {}", e);
        if state.is_camera_enabled() {
            let _ = crate::commands::input::stop_webview_webcam_recording(
                &app,
                &recording_output_path,
                state.inner(),
            )
            .await;
        }
        #[cfg(target_os = "macos")]
        if state.is_camera_enabled() {
            let mut capture_guard = state.webcam_capture.lock();
            if let Some(ref mut capture) = *capture_guard {
                capture.show_preview();
            }
        }
        restore_ui_elements(&app).await;
        return Err(format!("Failed to start recording: {}", e));
    }

    #[cfg(target_os = "macos")]
    if state.is_camera_enabled() {
        let mut capture_guard = state.webcam_capture.lock();
        if let Some(ref mut capture) = *capture_guard {
            let frame_rx = capture.start_recording();
            let webcam_path =
                crate::recording::artifacts::RecordingArtifacts::new(&recording_output_path)
                    .webcam_mp4();
            state
                .webcam_stop_signal
                .store(false, std::sync::atomic::Ordering::SeqCst);
            let stop = Arc::clone(&state.webcam_stop_signal);
            let task = crate::camera::spawn_webcam_task(frame_rx, webcam_path, 30, stop);
            *state.webcam_task.lock() = Some(task);
            println!("[Webcam] Frame recording started (preview hidden)");
        }
    }

    let started_at_ms = chrono::Utc::now().timestamp_millis();

    if let Err(e) =
        crate::commands::tray::update_main_tray_timer_cmd(app.clone(), "00:00:00".to_string()).await
    {
        println!("Warning: failed to set initial tray timer: {}", e);
    }

    let app_clone = app.clone();
    let state_clone = Arc::clone(&*state);
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

    println!("Recording started successfully");
    Ok(())
}

/// Signal stop recording - instant response
#[tauri::command]
pub async fn record_stop_instant_new(
    app: AppHandle,
    state: State<'_, Arc<UnifiedAppState>>,
) -> Result<String, String> {
    crate::commands::recording_control::record_stop_instant(app, state).await
}

/// Discard the current recording and immediately begin a fresh take with the same config.
#[tauri::command]
pub async fn record_restart_new(
    app: AppHandle,
    state: State<'_, Arc<UnifiedAppState>>,
) -> Result<(), String> {
    println!("Restarting recording with new architecture");

    if STOPPING_RECORDING
        .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
        .is_err()
    {
        return Err("Recording is already stopping".to_string());
    }

    struct StopGuard;
    impl Drop for StopGuard {
        fn drop(&mut self) {
            STOPPING_RECORDING.store(false, Ordering::SeqCst);
        }
    }
    let _stop_guard = StopGuard;

    let mut next_config = state
        .recording
        .get_current_config()
        .ok_or_else(|| "No active recording to restart".to_string())?;
    let old_output_path = next_config.output_path.clone();

    // Stop the active recorder without opening the editor or processing sidecars.
    let stopped_path = state
        .recording
        .signal_stop_recording()
        .await
        .map_err(|e| format!("Failed to stop current recording: {}", e))?;
    state
        .stop_mouse_tracking()
        .await
        .map_err(|e| format!("Failed to stop mouse tracking: {}", e))?;

    let completed_path = state
        .recording
        .wait_for_completion()
        .await
        .unwrap_or_else(|_| stopped_path.clone());
    discard_recording_files(&completed_path);
    if completed_path != old_output_path {
        discard_recording_files(&old_output_path);
    }

    next_config.output_path = fresh_restart_output_path(&old_output_path);
    let recording_output_path = next_config.output_path.clone();

    if let Err(e) = hide_ui_elements(&app).await {
        restore_ui_elements(&app).await;
        return Err(e);
    }

    if let Err(e) = state.start_recording(next_config).await {
        restore_ui_elements(&app).await;
        return Err(format!("Failed to restart recording: {}", e));
    }

    start_recording_timer(app.clone(), Arc::clone(&*state), &recording_output_path).await;

    if let Some(hud) = app.get_webview_window("recording-hud") {
        let _ = hud.show();
        let _ = hud.set_always_on_top(true);
    }

    println!("Recording restarted successfully");
    Ok(())
}

/// Pause recording
#[tauri::command]
pub async fn record_pause_new(state: State<'_, Arc<UnifiedAppState>>) -> Result<(), String> {
    state
        .pause_recording()
        .await
        .map_err(|e| format!("Failed to pause recording: {}", e))?;

    println!("Recording paused");
    Ok(())
}

/// Resume recording
#[tauri::command]
pub async fn record_resume_new(state: State<'_, Arc<UnifiedAppState>>) -> Result<(), String> {
    state
        .resume_recording()
        .await
        .map_err(|e| format!("Failed to resume recording: {}", e))?;

    println!("Recording resumed");
    Ok(())
}

/// Get recording status
#[tauri::command]
pub async fn get_recording_status(
    state: State<'_, Arc<UnifiedAppState>>,
) -> Result<serde_json::Value, String> {
    let status = state.get_app_status().await;

    let status_json = serde_json::json!({
        "is_recording": status.is_recording,
        "interface_mode": status.interface_mode,
        "tray_state": status.tray_state,
        "recording_info": status.recording_info,
    });

    Ok(status_json)
}

/// Update tray with recording duration
#[tauri::command]
pub async fn update_recording_duration(
    duration: String,
    state: State<'_, Arc<UnifiedAppState>>,
) -> Result<(), String> {
    state.update_recording_duration(duration).await;
    Ok(())
}

fn fresh_restart_output_path(previous_output_path: &str) -> String {
    let previous = Path::new(previous_output_path);
    let parent = previous.parent().unwrap_or_else(|| Path::new("/tmp"));
    let extension = previous
        .extension()
        .and_then(|ext| ext.to_str())
        .unwrap_or("mp4");
    let timestamp = chrono::Local::now().format("%Y-%m-%d_%H-%M-%S-%3f");
    parent
        .join(format!("restart_{}.{}", timestamp, extension))
        .to_string_lossy()
        .to_string()
}

fn discard_recording_files(output_path: &str) {
    let artifacts = crate::recording::artifacts::RecordingArtifacts::new(output_path);
    for candidate in artifacts.all_paths() {
        if candidate.exists() {
            if let Err(error) = std::fs::remove_file(&candidate) {
                println!(
                    "Warning: failed to discard recording file {}: {}",
                    candidate.display(),
                    error
                );
            }
        }
    }
}
