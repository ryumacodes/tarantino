use anyhow::Result;
use tauri::{AppHandle, State, Emitter, Manager};
use std::sync::Arc;
use std::sync::atomic::Ordering;

use crate::state::UnifiedAppState;
use crate::commands::STOPPING_RECORDING;
use crate::recording::types::*;

/// Start recording with the new architecture
#[tauri::command]
pub async fn record_start_new(
    target_type: String,       // "desktop", "window", "device"
    target_id: String,         // display ID, window ID, or device ID
    quality: String,           // "High", "Medium", "Low", "Lossless"
    include_cursor: bool,
    include_microphone: bool,
    include_system_audio: bool,
    output_path: Option<String>,
    app: AppHandle,
    state: State<'_, Arc<UnifiedAppState>>,
) -> Result<(), String> {
    println!("Starting recording with new architecture");
    
    // Check screen recording permissions first
    #[cfg(target_os = "macos")]
    {
        use crate::permissions::check_permissions;
        let permissions = check_permissions();
        if !permissions.screen_recording_granted {
            println!("Recording failed: Screen recording permission denied");
            return Err("Screen recording permission required. Please enable in System Preferences > Security & Privacy > Privacy > Screen Recording, then restart the app.".to_string());
        }
        println!("Screen recording permission verified ✅");
    }
    
    // Hide UI elements during recording
    hide_ui_elements(&app).await?;

    // Resolve defaults from app settings when requested and apply mic defaults
    let (effective_quality, maybe_mic_device, mic_enabled_default) = {
        let app_read = state.app.read();
        (
            if quality.eq_ignore_ascii_case("Default") {
                app_read.settings.default_quality.clone()
            } else { quality }
            ,
            app_read.microphone_config.device_id.clone(),
            app_read.microphone_config.enabled,
        )
    };

    // Build recording configuration
    let mut recording_config = build_recording_config(
        target_type,
        target_id,
        effective_quality,
        include_cursor,
        include_microphone,
        include_system_audio,
        output_path,
    )?;

    // Apply microphone defaults if not explicitly provided
    if recording_config.include_microphone || mic_enabled_default {
        recording_config.include_microphone = true;
        if recording_config.microphone_device.is_none() {
            recording_config.microphone_device = maybe_mic_device;
        }
    }

    // Start recording through unified state
    state.start_recording(recording_config).await
        .map_err(|e| format!("Failed to start recording: {}", e))?;

    // Get the current timestamp for recording start
    let started_at_ms = chrono::Utc::now().timestamp_millis();

    // No floating HUD during recording; tray timer handles UX

    // Start a background task to update the tray timer in real-time (every second)
    let app_clone = app.clone();
    let state_clone = Arc::clone(&*state);
    let timer_id = uuid::Uuid::new_v4().to_string()[..8].to_string();
    let timer_id_clone = timer_id.clone();

    // Create a unique cancel flag for this specific timer instance
    let timer_cancel_flag = Arc::new(std::sync::atomic::AtomicBool::new(false));
    let timer_cancel_flag_clone = Arc::clone(&timer_cancel_flag);

    let timer_handle = tokio::spawn(async move {
        let mut interval = tokio::time::interval(tokio::time::Duration::from_secs(1));
        println!("=== TRAY_TIMER[{}]: Timer loop started ===", timer_id_clone);

        loop {
            // Check cancellation flag
            if timer_cancel_flag_clone.load(std::sync::atomic::Ordering::Acquire) {
                println!("=== TRAY_TIMER[{}]: Cancellation detected, stopping ===", timer_id_clone);
                break;
            }

            // Wait for next tick with timeout for faster cancellation
            match tokio::time::timeout(tokio::time::Duration::from_millis(500), interval.tick()).await {
                Ok(_) => {
                    // Check cancellation again after tick
                    if timer_cancel_flag_clone.load(std::sync::atomic::Ordering::Acquire) {
                        break;
                    }

                    // Calculate elapsed time
                    let elapsed = chrono::Utc::now().timestamp_millis() - started_at_ms;
                    let seconds = elapsed / 1000;
                    let hours = seconds / 3600;
                    let minutes = (seconds % 3600) / 60;
                    let secs = seconds % 60;
                    let time_str = format!("{:02}:{:02}:{:02}", hours, minutes, secs);

                    // Update tray through unified state
                    state_clone.ui.set_tray_recording(Some(time_str.clone()));

                    // Actually update the system tray menu (not just internal state)
                    match crate::commands::tray::update_main_tray_timer_cmd(app_clone.clone(), time_str.clone()).await {
                        Ok(()) => {
                            // Successfully updated tray
                        }
                        Err(e) => {
                            println!("=== TRAY_TIMER[{}]: Failed to update tray: {} ===", timer_id_clone, e);
                            break;
                        }
                    }
                }
                Err(_) => {
                    // Timeout, continue to check cancellation
                    continue;
                }
            }
        }

        println!("=== TRAY_TIMER[{}]: Timer loop ended ===", timer_id_clone);
    });

    // Store the timer handle and its cancel flag
    println!("=== RECORDING: Storing timer handle[{}] ===", timer_id);
    state.set_tray_timer_handle_with_flag(timer_handle, timer_cancel_flag);

    // Emit recording started event with timestamp for HUD
    let payload = format!(r#"{{"started_at_ms": {}}}"#, started_at_ms);
    app.emit("recording:started", payload).map_err(|e| e.to_string())?;

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
    if STOPPING_RECORDING.compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst).is_err() {
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
    let (temp_path, _recording_start_time) = state.signal_stop_recording().await
        .map_err(|e| format!("Failed to signal stop: {}", e))?;

    // Open editor immediately with loading state
    open_editor_with_loading(&app, &temp_path).await?;

    // Start background processing
    // Note: This path uses generate_zoom_analysis() which happens synchronously in signal_stop_recording(),
    // so we don't need to pass recording_start_time to spawn_background_completion
    spawn_background_completion(app.clone(), Arc::clone(&*state), temp_path.clone());
    
    // Emit recording stopping event
    app.emit("recording-stopping", ()).map_err(|e| e.to_string())?;
    
    println!("Recording stop signaled, editor opened");
    Ok(temp_path)
}

/// Pause recording
#[tauri::command]
pub async fn record_pause_new(
    state: State<'_, Arc<UnifiedAppState>>,
) -> Result<(), String> {
    state.pause_recording().await
        .map_err(|e| format!("Failed to pause recording: {}", e))?;
    
    println!("Recording paused");
    Ok(())
}

/// Resume recording
#[tauri::command]
pub async fn record_resume_new(
    state: State<'_, Arc<UnifiedAppState>>,
) -> Result<(), String> {
    state.resume_recording().await
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
            let display_id = display_id_str.parse::<u32>()
                .map_err(|_| "Invalid display ID")?;

            let area = if let Some(area_str) = parts.next() {
                // Expect x,y,w,h
                let nums: Vec<&str> = area_str.split(',').collect();
                if nums.len() == 4 {
                    let x = nums[0].parse::<i32>().map_err(|_| "Invalid area x")?;
                    let y = nums[1].parse::<i32>().map_err(|_| "Invalid area y")?;
                    let w = nums[2].parse::<u32>().map_err(|_| "Invalid area width")?;
                    let h = nums[3].parse::<u32>().map_err(|_| "Invalid area height")?;
                    Some(RecordingArea { x, y, width: w, height: h })
                } else { None }
            } else { None };

            RecordingTarget::Desktop {
                display_id,
                area,
            }
        },
        "window" => {
            let window_id = target_id.parse::<u64>()
                .map_err(|_| "Invalid window ID")?;
            RecordingTarget::Window {
                window_id,
                include_shadow: true,
            }
        },
        "device" => {
            RecordingTarget::Device {
                device_id: target_id,
                device_type: DeviceType::Camera, // Default to camera for now
            }
        },
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

/// Hide UI elements during recording
async fn hide_ui_elements(app: &AppHandle) -> Result<(), String> {
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


/// Open editor with loading state
async fn open_editor_with_loading(app: &AppHandle, temp_path: &str) -> Result<(), String> {
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
        return Err(format!("Recording file not ready after {}ms timeout", max_attempts * 100));
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

    // Hide capture bar (don't close - it keeps the app alive)
    if let Some(bar) = app.get_webview_window("capture-bar") {
        let _ = bar.hide();
        println!("Hidden capture bar window");
    }

    // Create new editor window with overlay titlebar (native traffic lights, no title chrome)
    let builder = WebviewWindowBuilder::new(app, "editor", tauri::WebviewUrl::App("editor.html".into()))
        .title("Tarantino Editor")
        .decorations(true)
        .inner_size(1400.0, 900.0)
        .min_inner_size(800.0, 600.0)
        .center();
    #[cfg(target_os = "macos")]
    let builder = builder
        .title_bar_style(tauri::TitleBarStyle::Overlay)
        .hidden_title(true);
    let editor_window = builder.build()
        .map_err(|e| e.to_string())?;

    // Position native traffic lights inside our custom titlebar
    #[cfg(target_os = "macos")]
    {
        use tauri_plugin_decorum::WebviewWindowExt;
        editor_window.set_traffic_lights_inset(16.0, 20.0).ok();
    }

    // Emit event with temp path
    editor_window.emit("editor-loading", temp_path).map_err(|e| e.to_string())?;

    println!("Editor opened with loading state");
    Ok(())
}

/// Spawn background completion processing
fn spawn_background_completion(
    app: AppHandle,
    state: Arc<UnifiedAppState>,
    _temp_path: String,
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
            },
            Err(e) => {
                println!("Recording completion failed: {}", e);
                
                // Show error
                state.show_error(&format!("Recording failed: {}", e));
                
                // Notify editor of error
                if let Some(editor) = app.get_webview_window("editor") {
                    if let Err(e) = editor.emit("recording-error", format!("Recording failed: {}", e)) {
                        println!("Failed to notify editor of error: {}", e);
                    }
                }
            }
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_build_recording_config() {
        let config = build_recording_config(
            "desktop".to_string(),
            "0".to_string(),
            "High".to_string(),
            true,
            false,
            false,
            Some("/tmp/test.mp4".to_string()),
        ).unwrap();
        
        assert!(matches!(config.target, RecordingTarget::Desktop { .. }));
        assert!(matches!(config.quality, QualityPreset::High));
        assert_eq!(config.output_path, "/tmp/test.mp4");
        // include_cursor is always false - we use overlay cursor in editor
        assert!(!config.include_cursor);
        assert!(!config.include_microphone);
    }
    
    #[test]
    fn test_invalid_target_type() {
        let result = build_recording_config(
            "invalid".to_string(),
            "0".to_string(),
            "High".to_string(),
            true,
            false,
            false,
            None,
        );
        
        assert!(result.is_err());
    }
}