//! Recording control commands
//!
//! Handles starting, stopping, pausing, and resuming recordings.

use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use anyhow::Result;
use tauri::{Emitter, Manager, State};

use crate::state::UnifiedAppState;

// Global flag to prevent concurrent stop operations
pub static STOPPING_RECORDING: AtomicBool = AtomicBool::new(false);

/// Start recording with the given configuration
#[tauri::command]
pub async fn record_start(
    _fps: u32,
    _out_width: u32,
    _out_height: u32,
    _container: String,
    _encoder_pref: String,
    _cursor: bool,
    _cursor_size: String,
    _highlight_clicks: bool,
    path: String,
    app: tauri::AppHandle,
    state: State<'_, Arc<UnifiedAppState>>,
) -> Result<(), String> {
    use tauri::{LogicalPosition, LogicalSize};

    println!("!!! RECORD_START CALLED !!!");

    // Hide webcam preview FIRST so it doesn't appear in the screen recording
    if let Some(wc) = app.get_webview_window("webcam-preview") {
        wc.hide().map_err(|e| e.to_string())?;
    }

    // Hide preview windows
    if let Some(preview) = app.get_webview_window("display-preview") {
        preview.hide().map_err(|e| e.to_string())?;
    }

    // Hide capture bar
    let (_bar_pos, _bar_size) = if let Some(bar) = app.get_webview_window("capture-bar") {
        let pos = bar
            .outer_position()
            .map_err(|e| e.to_string())?
            .to_logical::<f64>(bar.scale_factor().unwrap_or(1.0));
        let size = bar
            .outer_size()
            .map_err(|e| e.to_string())?
            .to_logical::<f64>(bar.scale_factor().unwrap_or(1.0));
        bar.hide().map_err(|e| e.to_string())?;
        (pos, size)
    } else {
        (
            LogicalPosition::new(100.0, 80.0),
            LogicalSize::new(710.0, 60.0),
        )
    };

    // Hide recording HUD
    if let Some(hud) = app.get_webview_window("recording-hud") {
        let _ = hud.hide();
    }

    let started_at = chrono::Utc::now().timestamp_millis();

    // Start mouse tracking
    println!("🖱️ [RECORD_START] Starting mouse tracking...");
    match state.start_mouse_tracking().await {
        Ok(_) => println!("✅ [RECORD_START] Mouse tracking started successfully"),
        Err(e) => println!("❌ [RECORD_START] Failed to start mouse tracking: {}", e),
    }

    // Update tray to recording state
    if let Err(e) =
        crate::commands::tray::update_main_tray_timer_cmd(app.clone(), "00:00:00".to_string()).await
    {
        println!("Warning: Failed to update tray: {}", e);
    }

    // Start tray timer background task
    let app_clone = app.clone();
    let timer_cancel_flag = Arc::new(AtomicBool::new(false));
    let timer_cancel_flag_clone = Arc::clone(&timer_cancel_flag);

    let timer_handle = tokio::spawn(async move {
        let mut interval = tokio::time::interval(tokio::time::Duration::from_secs(1));
        loop {
            if timer_cancel_flag_clone.load(Ordering::Acquire) {
                break;
            }
            match tokio::time::timeout(
                tokio::time::Duration::from_millis(500),
                interval.tick(),
            )
            .await
            {
                Ok(_) => {
                    if timer_cancel_flag_clone.load(Ordering::Acquire) {
                        break;
                    }
                    let elapsed = chrono::Utc::now().timestamp_millis() - started_at;
                    let seconds = elapsed / 1000;
                    let time_str = format!(
                        "{:02}:{:02}:{:02}",
                        seconds / 3600,
                        (seconds % 3600) / 60,
                        seconds % 60
                    );
                    if crate::commands::tray::update_main_tray_timer_cmd(
                        app_clone.clone(),
                        time_str,
                    )
                    .await
                    .is_err()
                    {
                        break;
                    }
                }
                Err(_) => continue,
            }
        }
    });

    state.set_tray_timer_handle_with_flag(timer_handle, timer_cancel_flag);

    // Start actual recording
    let mut config = crate::recording::RecordingConfig::default();
    config.output_path = path;

    match state.start_recording(config).await {
        Ok(_) => println!("Recording started successfully"),
        Err(ref e) => {
            println!("Recording failed: {}", e);
            let _ = crate::commands::tray::reset_tray_to_idle_impl(app).await;
            return Err(e.to_string());
        }
    }

    Ok(())
}

/// Pause the current recording
#[tauri::command]
pub async fn record_pause(state: State<'_, Arc<UnifiedAppState>>) -> Result<(), String> {
    state.pause_recording().await.map_err(|e| e.to_string())
}

/// Resume a paused recording
#[tauri::command]
pub async fn record_resume(state: State<'_, Arc<UnifiedAppState>>) -> Result<(), String> {
    state.resume_recording().await.map_err(|e| e.to_string())
}

/// Stop recording and process the file
#[tauri::command]
pub async fn record_stop(
    app: tauri::AppHandle,
    state: State<'_, Arc<UnifiedAppState>>,
) -> Result<String, String> {
    println!("record_stop called");

    let temp_path = state.stop_recording().await.map_err(|e| e.to_string())?;
    println!("Recording stopped, temp path: {}", temp_path);

    println!("🖱️ [RECORD_STOP] Getting mouse events from tracker...");
    let mouse_tracker = state.get_mouse_tracker();
    let mouse_events = {
        let guard = mouse_tracker.lock();
        println!("   - is_tracking: {}", guard.is_tracking);
        println!("   - recording_start_time: {:?}", guard.recording_start_time);
        let events = guard.get_events();
        let click_count = events.iter().filter(|e| matches!(e.event_type, crate::mouse_tracking::MouseEventType::ButtonPress { .. })).count();
        println!("   - total events: {}", events.len());
        println!("   - click events: {}", click_count);
        events
    };
    println!("✅ [RECORD_STOP] Collected {} mouse events", mouse_events.len());

    // Move to permanent location
    let home_dir = std::env::var("HOME").unwrap_or_else(|_| "/tmp".to_string());
    let movies_dir = PathBuf::from(&home_dir).join("Movies").join("Tarantino");
    std::fs::create_dir_all(&movies_dir).map_err(|e| e.to_string())?;

    let timestamp = chrono::Local::now().format("%Y%m%d_%H%M%S");
    let final_path = movies_dir.join(format!("recording_{}.mp4", timestamp));

    let mut media_path = temp_path.clone();
    let mut file_found = false;
    let mut last_file_size = 0u64;
    let mut stable_size_count = 0;

    for attempt in 1..=15 {
        if std::path::Path::new(&temp_path).exists() {
            if let Ok(metadata) = std::fs::metadata(&temp_path) {
                if metadata.len() > 1024 {
                    let current_size = metadata.len();
                    if current_size == last_file_size {
                        stable_size_count += 1;
                    } else {
                        stable_size_count = 0;
                        last_file_size = current_size;
                    }
                    if stable_size_count >= 2 || attempt >= 10 {
                        if let Ok(true) =
                            crate::commands::video_validation::validate_video_file(&temp_path)
                        {
                            println!("Video validated on attempt {}", attempt);
                            file_found = true;
                            break;
                        }
                    }
                }
            }
        }
        if attempt < 15 {
            std::thread::sleep(std::time::Duration::from_millis(
                std::cmp::min(1000 + (attempt * 200), 3000) as u64,
            ));
        }
    }

    if file_found {
        if std::fs::rename(&temp_path, &final_path).is_ok() {
            println!("Moved recording to {}", final_path.display());
            media_path = final_path.to_str().unwrap_or("").to_string();
        }
    }

    // Create sidecar
    let sidecar_path = format!("{}.sidecar.json", media_path);
    let mut sidecar = crate::sidecar::Sidecar::new_for_recording(&media_path, 30, 10000, 1920, 1080);
    sidecar.mouse_events = mouse_events;
    if let Err(e) = sidecar.save(&sidecar_path) {
        println!("Warning: Failed to save sidecar: {}", e);
    }

    let (has_mic, has_system_audio) = state.get_recording_audio_flags();
    let has_webcam = state.is_camera_enabled();

    // Stop native webcam capture and wait for encoding to finish
    #[cfg(target_os = "macos")]
    if has_webcam {
        state.webcam_stop_signal.store(true, std::sync::atomic::Ordering::SeqCst);
        {
            let mut guard = state.webcam_capture.lock();
            if let Some(ref mut capture) = *guard {
                capture.stop_recording();
            }
            *guard = None;
        }
        let webcam_task = state.webcam_task.lock().take();
        if let Some(task) = webcam_task {
            match tokio::time::timeout(std::time::Duration::from_secs(5), task).await {
                Ok(Ok(Ok(path))) => println!("[Webcam] Saved: {}", path),
                Ok(Ok(Err(e))) => println!("[Webcam] Encoding error: {}", e),
                Ok(Err(e)) => println!("[Webcam] Task panic: {}", e),
                Err(_) => println!("[Webcam] Timeout waiting for encoding"),
            }
        }
        state.camera_enabled.store(false, std::sync::atomic::Ordering::SeqCst);
    }

    // Post-processing
    let final_media_path = crate::commands::processing::apply_post_processing(&media_path)
        .await
        .unwrap_or(media_path.clone());

    crate::commands::processing::open_editor(&app, &final_media_path, &sidecar_path, has_webcam, has_mic, has_system_audio)
        .await?;

    app.emit("recording-stopped", final_media_path.clone())
        .map_err(|e| e.to_string())?;
    Ok(final_media_path)
}

/// Stop recording instantly and spawn background processing
#[tauri::command]
pub async fn record_stop_instant(
    app: tauri::AppHandle,
    state: State<'_, Arc<UnifiedAppState>>,
) -> Result<String, String> {
    println!("=== RECORD_STOP_INSTANT ===");

    // Debug: Check mouse tracker state before stopping
    {
        let tracker = state.get_mouse_tracker();
        let guard = tracker.lock();
        let click_count = guard.events.lock().iter().filter(|e| matches!(e.event_type, crate::mouse_tracking::MouseEventType::ButtonPress { .. })).count();
        println!("🖱️ [STOP_INSTANT] Mouse tracker state before stop:");
        println!("   - is_tracking: {}", guard.is_tracking);
        println!("   - recording_start_time: {:?}", guard.recording_start_time);
        println!("   - total events: {}", guard.events.lock().len());
        println!("   - click events: {}", click_count);
    }

    if STOPPING_RECORDING
        .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
        .is_err()
    {
        return Ok("Already stopping".to_string());
    }

    struct StopGuard;
    impl Drop for StopGuard {
        fn drop(&mut self) {
            STOPPING_RECORDING.store(false, Ordering::SeqCst);
        }
    }
    let _guard = StopGuard;

    let stop_result = state.signal_stop_recording().await;
    let _ = crate::commands::tray::reset_tray_to_idle_impl(app.clone()).await;

    match stop_result {
        Ok((temp_path, recording_start_time)) => {
            println!("Recording stop signaled, temp path: {}", temp_path);
            println!("🖱️ [STOP_INSTANT] Captured recording_start_time: {:?}", recording_start_time);
            let has_webcam = state.is_camera_enabled();

            // Stop native webcam capture and wait for encoding to finish
            #[cfg(target_os = "macos")]
            if has_webcam {
                state.webcam_stop_signal.store(true, std::sync::atomic::Ordering::SeqCst);
                {
                    let mut guard = state.webcam_capture.lock();
                    if let Some(ref mut capture) = *guard {
                        capture.stop_recording();
                    }
                    *guard = None;
                }
                let webcam_task = state.webcam_task.lock().take();
                if let Some(task) = webcam_task {
                    match tokio::time::timeout(std::time::Duration::from_secs(5), task).await {
                        Ok(Ok(Ok(path))) => println!("[Webcam] Saved: {}", path),
                        Ok(Ok(Err(e))) => println!("[Webcam] Encoding error: {}", e),
                        Ok(Err(e)) => println!("[Webcam] Task panic: {}", e),
                        Err(_) => println!("[Webcam] Timeout waiting for encoding"),
                    }
                }
                state.camera_enabled.store(false, std::sync::atomic::Ordering::SeqCst);
            }

            crate::commands::processing::open_editor_with_loading(&app, &temp_path, has_webcam)
                .await
                .map_err(|e| e.to_string())?;
            crate::commands::processing::spawn_background_recording_processing(
                app,
                state.inner().clone(),
                temp_path.clone(),
                recording_start_time,
            );
            Ok(temp_path)
        }
        Err(e) => {
            println!("No active recording: {}", e);
            let placeholder_path = format!(
                "{}/no_recording_{}.mp4",
                std::env::temp_dir().display(),
                chrono::Local::now().timestamp()
            );
            crate::commands::processing::open_editor_with_loading(&app, &placeholder_path, false)
                .await
                .map_err(|e| e.to_string())?;
            Ok(placeholder_path)
        }
    }
}

/// Stop recording via HUD button
#[tauri::command]
pub async fn hud_stop(
    app: tauri::AppHandle,
    state: State<'_, Arc<UnifiedAppState>>,
) -> Result<(), String> {
    println!("HUD stop button clicked");
    record_stop(app, state).await.map(|_| ())
}
