use anyhow::Result;
use serde::Serialize;
use std::path::Path;
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};
use tauri::{AppHandle, Manager, State};

use crate::commands::STOPPING_RECORDING;
use crate::state::UnifiedAppState;

#[cfg(target_os = "linux")]
mod failure;
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

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RecordingCapabilities {
    recording_available: bool,
    unavailable_reason: Option<String>,
    uses_system_source_picker: bool,
    display_preview_available: bool,
    automatic_zoom_available: bool,
    system_audio_available: bool,
}

#[tauri::command]
pub async fn get_recording_capabilities() -> RecordingCapabilities {
    #[cfg(target_os = "linux")]
    {
        let portal = crate::recording::linux::portal_source_types().await;
        let unavailable_reason = portal
            .and_then(|_| crate::recording::linux::runtime_preflight())
            .err()
            .map(|error| error.to_string());
        return RecordingCapabilities {
            recording_available: unavailable_reason.is_none(),
            unavailable_reason,
            uses_system_source_picker: true,
            display_preview_available: false,
            automatic_zoom_available: crate::input::pointer_tracking_available(),
            system_audio_available: crate::recording::linux::system_audio_available(),
        };
    }

    #[cfg(not(target_os = "linux"))]
    RecordingCapabilities {
        recording_available: true,
        unavailable_reason: None,
        uses_system_source_picker: false,
        display_preview_available: true,
        automatic_zoom_available: true,
        system_audio_available: true,
    }
}

#[tauri::command]
pub async fn record_start_new(
    target_type: String,
    target_id: String,
    quality: String,
    include_cursor: bool,
    include_microphone: bool,
    include_system_audio: bool,
    capture_pointer_events: bool,
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

    #[cfg(not(target_os = "linux"))]
    let _ = capture_pointer_events;

    if state.recording.is_recording() || STOPPING_RECORDING.load(Ordering::Acquire) {
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

    #[cfg(target_os = "linux")]
    let previous_pointer_capture_consent = crate::input::pointer_capture_consented();
    #[cfg(target_os = "linux")]
    crate::input::set_pointer_capture_consent(capture_pointer_events);

    if let Err(e) = state.start_recording(recording_config).await {
        #[cfg(target_os = "linux")]
        crate::input::set_pointer_capture_consent(previous_pointer_capture_consent);
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

    start_recording_timer(app.clone(), Arc::clone(&*state), &recording_output_path).await;

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
