use anyhow::Result;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};
use tauri::{AppHandle, Emitter, Manager, State};

use crate::commands::STOPPING_RECORDING;
use crate::recording::types::*;
use crate::state::UnifiedAppState;

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
        let _ =
            crate::commands::input::start_webview_webcam_recording(&app, &recording_output_path)
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
                std::path::PathBuf::from(recording_output_path.replace(".mp4", ".webcam.mp4"));
            state
                .webcam_stop_signal
                .store(false, std::sync::atomic::Ordering::SeqCst);
            let stop = Arc::clone(&state.webcam_stop_signal);
            let task = crate::webcam::spawn_webcam_task(frame_rx, webcam_path, 30, stop);
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
    println!("Stopping recording instantly with new architecture");

    // Prevent concurrent stop operations (race condition fix)
    if STOPPING_RECORDING
        .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
        .is_err()
    {
        println!("Already stopping recording, skipping duplicate call");
        return Ok("Already stopping".to_string());
    }

    // Ensure we reset the flag when done (use scopeguard pattern)
    struct StopGuard;
    impl Drop for StopGuard {
        fn drop(&mut self) {
            STOPPING_RECORDING.store(false, Ordering::SeqCst);
        }
    }
    let _guard = StopGuard;

    // Signal stop and get temp path immediately
    // Note: signal_stop_recording now returns (temp_path, recording_start_time) tuple
    // The recording_start_time is captured before stop_mouse_tracking() resets it
    let (temp_path, _recording_start_time) = state
        .signal_stop_recording()
        .await
        .map_err(|e| format!("Failed to signal stop: {}", e))?;

    let has_webcam = state.is_camera_enabled();
    if has_webcam {
        let _ = crate::commands::input::stop_webview_webcam_recording(&app, &temp_path).await;
    }

    // Stop native webcam capture and wait for encoding to finish
    #[cfg(target_os = "macos")]
    if has_webcam {
        // Signal encoding task to stop
        state
            .webcam_stop_signal
            .store(true, std::sync::atomic::Ordering::SeqCst);
        // Stop frame delivery from camera (but keep session for potential reuse)
        {
            let mut guard = state.webcam_capture.lock();
            if let Some(ref mut capture) = *guard {
                capture.stop_recording();
            }
            // Also fully stop and release the capture session
            *guard = None;
        }
        // Wait for encoding task to finish
        let webcam_task = state.webcam_task.lock().take();
        if let Some(task) = webcam_task {
            match tokio::time::timeout(std::time::Duration::from_secs(5), task).await {
                Ok(Ok(Ok(path))) => println!("[Webcam] Saved: {}", path),
                Ok(Ok(Err(e))) => println!("[Webcam] Encoding error: {}", e),
                Ok(Err(e)) => println!("[Webcam] Task panic: {}", e),
                Err(_) => println!("[Webcam] Timeout waiting for encoding"),
            }
        }
        state
            .camera_enabled
            .store(false, std::sync::atomic::Ordering::SeqCst);
    }

    if has_webcam {
        state
            .camera_enabled
            .store(false, std::sync::atomic::Ordering::SeqCst);
    }

    crate::commands::lifecycle::release_recording_surfaces(
        &app,
        Some(state.inner()),
        "recording stopped",
    );

    // Open editor immediately with loading state
    let webcam_shape = state.webcam_shape();
    open_editor_with_loading(&app, &temp_path, has_webcam, &webcam_shape).await?;

    // Start background processing
    spawn_background_completion(
        app.clone(),
        Arc::clone(&*state),
        temp_path.clone(),
        has_webcam,
        webcam_shape,
    );

    // Emit recording stopping event
    app.emit("recording-stopping", ())
        .map_err(|e| e.to_string())?;

    println!("Recording stop signaled, editor opened");
    Ok(temp_path)
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

fn resolve_recording_target(
    state: &UnifiedAppState,
    frontend_target_type: String,
    frontend_target_id: String,
) -> Result<(String, String), String> {
    let app_read = state.app.read();
    let backend_mode = app_read.capture_mode.as_str();
    let resolved = match frontend_target_type.as_str() {
        "desktop" if frontend_target_id != "0" => {
            ("desktop".to_string(), frontend_target_id.clone())
        }
        "window" if frontend_target_id != "0" => ("window".to_string(), frontend_target_id.clone()),
        "device" if frontend_target_id != "0" => ("device".to_string(), frontend_target_id.clone()),
        _ => match backend_mode {
            "desktop" => {
                let id = app_read
                    .selected_display_id
                    .clone()
                    .or_else(|| app_read.displays.first().map(|display| display.id.clone()))
                    .or_else(|| {
                        if frontend_target_type == "desktop" && frontend_target_id != "0" {
                            Some(frontend_target_id.clone())
                        } else {
                            None
                        }
                    })
                    .ok_or_else(|| {
                        "No display is selected yet. Wait for displays to load, then try again."
                            .to_string()
                    })?;
                ("desktop".to_string(), id)
            }
            "window" => {
                let id = app_read
                    .selected_window_id
                    .clone()
                    .or_else(|| {
                        if frontend_target_type == "window" && frontend_target_id != "0" {
                            Some(frontend_target_id.clone())
                        } else {
                            None
                        }
                    })
                    .ok_or_else(|| {
                        "No window is selected yet. Pick a window after the list loads.".to_string()
                    })?;
                ("window".to_string(), id)
            }
            "device" => {
                let id = app_read
                    .selected_device_id
                    .clone()
                    .unwrap_or_else(|| frontend_target_id.clone());
                ("device".to_string(), id)
            }
            _ => (frontend_target_type.clone(), frontend_target_id.clone()),
        },
    };

    println!(
        "Recording target resolved: frontend={}:{} backend_mode={} => {}:{}",
        frontend_target_type, frontend_target_id, backend_mode, resolved.0, resolved.1
    );
    Ok(resolved)
}

/// Build recording configuration from parameters
fn build_recording_config(
    target_type: String,
    target_id: String,
    quality: String,
    _include_cursor: bool, // Unused - always false, we use overlay cursor
    include_microphone: bool,
    include_system_audio: bool,
    output_path: Option<String>,
) -> Result<RecordingConfig, String> {
    // Parse target
    let target = match target_type.as_str() {
        "desktop" => {
            // Support optional area selection via target_id syntax: "<display_id>:x,y,w,h"
            let mut parts = target_id.split(':');
            let display_id_str = parts.next().unwrap_or("");
            let display_id = display_id_str
                .parse::<u32>()
                .map_err(|_| "Invalid display ID")?;

            let area = if let Some(area_str) = parts.next() {
                // Expect x,y,w,h
                let nums: Vec<&str> = area_str.split(',').collect();
                if nums.len() == 4 {
                    let x = nums[0].parse::<i32>().map_err(|_| "Invalid area x")?;
                    let y = nums[1].parse::<i32>().map_err(|_| "Invalid area y")?;
                    let w = nums[2].parse::<u32>().map_err(|_| "Invalid area width")?;
                    let h = nums[3].parse::<u32>().map_err(|_| "Invalid area height")?;
                    Some(RecordingArea {
                        x,
                        y,
                        width: w,
                        height: h,
                    })
                } else {
                    None
                }
            } else {
                None
            };

            RecordingTarget::Desktop { display_id, area }
        }
        "window" => {
            let window_id = target_id.parse::<u64>().map_err(|_| "Invalid window ID")?;
            RecordingTarget::Window {
                window_id,
                include_shadow: true,
            }
        }
        "device" => {
            RecordingTarget::Device {
                device_id: target_id,
                device_type: DeviceType::Camera, // Default to camera for now
            }
        }
        _ => return Err("Invalid target type".to_string()),
    };

    // Parse quality preset
    let quality_preset = match quality.as_str() {
        "Lossless" => QualityPreset::Lossless,
        "High" => QualityPreset::High,
        "Medium" => QualityPreset::Medium,
        "Low" => QualityPreset::Low,
        _ => QualityPreset::High, // Default to high
    };

    // Generate output path if not provided
    let final_output_path = output_path.unwrap_or_else(|| {
        let home_dir = std::env::var("HOME").unwrap_or_else(|_| "/tmp".to_string());
        let timestamp = chrono::Local::now().format("%Y-%m-%d_%H-%M-%S");
        format!("{}/Movies/Tarantino/Recording_{}.mp4", home_dir, timestamp)
    });

    Ok(RecordingConfig {
        target,
        quality: quality_preset,
        output_format: OutputFormat {
            container: Container::MP4,
            codec: VideoCodec::H264,
            audio_codec: Some(AudioCodec::AAC),
        },
        output_path: final_output_path,
        // Always false - we use overlay cursor in editor for styling/effects
        include_cursor: false,
        cursor_size: 1.0,
        highlight_clicks: false,
        include_microphone,
        microphone_device: None, // TODO: Support device selection
        include_system_audio,
    })
}

fn validate_recording_target(
    state: &State<'_, Arc<UnifiedAppState>>,
    config: &RecordingConfig,
) -> Result<(), String> {
    match &config.target {
        RecordingTarget::Desktop { display_id, .. } => {
            let cached_displays = state.cached_displays();
            if cached_displays
                .iter()
                .any(|display| display.id == display_id.to_string())
            {
                Ok(())
            } else {
                Err("Selected display is not available yet. Try again after the display picker loads.".to_string())
            }
        }
        RecordingTarget::Window { window_id, .. } => {
            let cached_windows = state.cached_windows();
            let window = cached_windows
                .iter()
                .find(|window| window.id == window_id.to_string())
                .ok_or_else(|| {
                    "Selected window is stale. Pick a window again after the list loads."
                        .to_string()
                })?;
            let app_name = window.app_name.to_lowercase();
            let title = window.title.to_lowercase();
            if app_name == "tarantino" || title == "tarantino" || title.contains("web inspector") {
                return Err("Cannot record Tarantino's own windows. Choose another window or switch to display capture.".to_string());
            }
            Ok(())
        }
        RecordingTarget::Device { .. } => Ok(()),
    }
}

fn normalize_webcam_shape(shape: &str) -> String {
    match shape {
        "roundrect" | "rounded" => "roundrect".to_string(),
        _ => "circle".to_string(),
    }
}

/// Hide UI elements during recording
async fn hide_ui_elements(app: &AppHandle) -> Result<(), String> {
    // Hide webcam preview FIRST so it doesn't appear in the screen recording
    if let Some(wc) = app.get_webview_window("webcam-preview") {
        wc.hide().map_err(|e| e.to_string())?;
        println!("Webcam preview hidden for recording");
    }

    // Hide preview windows
    if let Some(preview) = app.get_webview_window("display-preview") {
        preview.hide().map_err(|e| e.to_string())?;
    }

    // Hide capture bar
    if let Some(bar) = app.get_webview_window("capture-bar") {
        bar.hide().map_err(|e| e.to_string())?;
    }

    println!("UI elements hidden for recording");
    Ok(())
}

async fn restore_ui_elements(app: &AppHandle) {
    if let Some(bar) = app.get_webview_window("capture-bar") {
        let _ = bar.show();
        let _ = bar.set_focus();
    }

    if let Some(preview) = app.get_webview_window("display-preview") {
        let _ = preview.hide();
    }

    if let Some(wc) = app.get_webview_window("webcam-preview") {
        let _ = wc.show();
    }
}

/// Open editor with loading state
async fn open_editor_with_loading(
    app: &AppHandle,
    temp_path: &str,
    has_webcam: bool,
    webcam_shape: &str,
) -> Result<(), String> {
    use tauri::WebviewWindowBuilder;

    // Wait for file to be readable and valid
    let path = std::path::Path::new(temp_path);
    let mut attempts = 0;
    let max_attempts = 30; // 3 seconds total

    println!("Waiting for recording file to be ready: {}", temp_path);

    while attempts < max_attempts {
        if let Ok(metadata) = std::fs::metadata(path) {
            // Check file has reasonable size (at least 10KB)
            if metadata.len() > 10000 {
                println!("Recording file ready: {} bytes", metadata.len());
                break;
            }
        }
        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
        attempts += 1;
    }

    if attempts >= max_attempts {
        return Err(format!(
            "Recording file not ready after {}ms timeout",
            max_attempts * 100
        ));
    }

    // Early exit if editor already exists - prevents duplicate windows from racing stop commands
    if let Some(_existing) = app.get_webview_window("editor") {
        println!("Editor already exists, skipping creation");
        return Ok(());
    }

    // Close display preview overlay
    if let Some(preview) = app.get_webview_window("display-preview") {
        let _ = preview.close();
        println!("Closed display preview window");
    }

    // Close recording HUD
    if let Some(hud) = app.get_webview_window("recording-hud") {
        let _ = hud.close();
        println!("Closed recording HUD window");
    }

    // Close webcam preview
    if let Some(wc) = app.get_webview_window("webcam-preview") {
        let _ = wc.close();
        println!("Closed webcam preview window");
    }

    // Hide capture bar (don't close - it keeps the app alive)
    if let Some(bar) = app.get_webview_window("capture-bar") {
        let _ = bar.hide();
        println!("Hidden capture bar window");
    }

    // Create new editor window with overlay titlebar (native traffic lights, no title chrome)
    let url = if has_webcam {
        format!(
            "editor.html?webcam=true&webcam_shape={}",
            urlencoding::encode(webcam_shape)
        )
    } else {
        "editor.html".to_string()
    };
    let builder = WebviewWindowBuilder::new(app, "editor", tauri::WebviewUrl::App(url.into()))
        .title("Tarantino Editor")
        .decorations(true)
        .inner_size(1400.0, 900.0)
        .min_inner_size(800.0, 600.0)
        .center();
    #[cfg(target_os = "macos")]
    let builder = builder
        .title_bar_style(tauri::TitleBarStyle::Overlay)
        .hidden_title(true);
    let editor_window = builder.build().map_err(|e| e.to_string())?;

    // Position native traffic lights inside our custom titlebar
    #[cfg(target_os = "macos")]
    {
        use tauri_plugin_decorum::WebviewWindowExt;
        editor_window.set_traffic_lights_inset(16.0, 20.0).ok();
    }

    // Emit event with temp path
    editor_window
        .emit("editor-loading", temp_path)
        .map_err(|e| e.to_string())?;

    println!("Editor opened with loading state");
    Ok(())
}

/// Spawn background completion processing
fn spawn_background_completion(
    app: AppHandle,
    state: Arc<UnifiedAppState>,
    _temp_path: String,
    has_webcam: bool,
    webcam_shape: String,
) {
    tokio::spawn(async move {
        println!("Starting background completion processing");

        match state.wait_for_recording_completion().await {
            Ok(final_path) => {
                println!("Recording completed: {}", final_path);

                // Notify editor that recording is ready (include audio flags)
                if let Some(editor) = app.get_webview_window("editor") {
                    let (has_mic, has_system_audio) = state.get_recording_audio_flags();
                    let payload = serde_json::json!({
                        "path": final_path,
                        "has_mic": has_mic,
                        "has_system_audio": has_system_audio,
                        "has_webcam": has_webcam,
                        "webcam_shape": webcam_shape,
                    });
                    if let Err(e) = editor.emit("recording-ready", payload) {
                        println!("Failed to notify editor: {}", e);
                    }
                }

                // Notify state that editor is ready
                state.notify_editor_ready();

                // Emit completion event
                if let Err(e) = app.emit("recording-completed", final_path) {
                    println!("Failed to emit completion event: {}", e);
                }
            }
            Err(e) => {
                println!("Recording completion failed: {}", e);

                // Show error
                state.show_error(&format!("Recording failed: {}", e));

                // Notify editor of error
                if let Some(editor) = app.get_webview_window("editor") {
                    if let Err(e) =
                        editor.emit("recording-error", format!("Recording failed: {}", e))
                    {
                        println!("Failed to notify editor of error: {}", e);
                    }
                }
            }
        }
    });
}
