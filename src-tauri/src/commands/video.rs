//! Video processing commands

use std::path::Path;
use std::process::Command;
use tauri::{Emitter, AppHandle};
use crate::video_processing::{VideoProcessor, VideoInfo, ExportSettings, ProcessingProgress};

#[tauri::command]
pub async fn get_video_info(video_path: String) -> Result<VideoInfo, String> {
    let processor = VideoProcessor::new().map_err(|e| e.to_string())?;
    processor.get_video_info(video_path).await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn get_video_metadata(#[allow(non_snake_case)] filePath: String) -> Result<VideoInfo, String> {
    println!("Extracting video metadata for: {}", filePath);
    let processor = VideoProcessor::new().map_err(|e| e.to_string())?;
    processor.get_video_info(filePath).await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn extract_video_thumbnails(
    app: AppHandle,
    video_path: String,
    thumbnail_count: u32,
    thumbnail_width: u32,
) -> Result<Vec<String>, String> {
    let processor = VideoProcessor::new().map_err(|e| e.to_string())?;

    let app_handle = app.clone();
    let callback = std::sync::Arc::new(move |index: u32, path: std::path::PathBuf| {
        let _ = app_handle.emit("thumbnail_ready", serde_json::json!({
            "index": index,
            "path": path.to_string_lossy()
        }));
    });

    let thumbnails = processor
        .extract_thumbnails(video_path, thumbnail_count, thumbnail_width, Some(callback))
        .await
        .map_err(|e| e.to_string())?;

    let thumbnail_paths: Vec<String> = thumbnails
        .into_iter()
        .map(|p| p.to_string_lossy().to_string())
        .collect();

    Ok(thumbnail_paths)
}

#[tauri::command]
pub async fn export_video(
    app: AppHandle,
    input_path: String,
    settings: ExportSettings,
) -> Result<String, String> {
    let (progress_tx, progress_rx) = std::sync::mpsc::channel::<ProcessingProgress>();
    let (result_tx, result_rx) = std::sync::mpsc::channel::<Result<std::path::PathBuf, String>>();

    std::thread::spawn(move || {
        let rt = match tokio::runtime::Runtime::new() {
            Ok(rt) => rt,
            Err(e) => {
                let _ = result_tx.send(Err(format!("Failed to create runtime: {}", e)));
                return;
            }
        };

        let result = rt.block_on(async {
            let processor = match VideoProcessor::new() {
                Ok(p) => p,
                Err(e) => return Err(e.to_string()),
            };

            let progress_callback = move |progress: ProcessingProgress| {
                let _ = progress_tx.send(progress);
            };

            processor.export_video(input_path, settings, Some(Box::new(progress_callback)))
                .await
                .map_err(|e| e.to_string())
        });

        let _ = result_tx.send(result);
    });

    loop {
        match progress_rx.try_recv() {
            Ok(progress) => {
                let _ = app.emit("export:progress", serde_json::json!({
                    "current": progress.current_frame,
                    "total": progress.total_frames,
                    "percentage": progress.percentage,
                }));
            }
            Err(std::sync::mpsc::TryRecvError::Empty) => {}
            Err(std::sync::mpsc::TryRecvError::Disconnected) => break,
        }

        match result_rx.try_recv() {
            Ok(result) => return result.map(|p| p.to_string_lossy().to_string()),
            Err(std::sync::mpsc::TryRecvError::Empty) => {
                tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;
            }
            Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                return Err("Export worker thread terminated unexpectedly".to_string());
            }
        }
    }

    match result_rx.recv() {
        Ok(result) => result.map(|p| p.to_string_lossy().to_string()),
        Err(e) => Err(format!("Failed to receive export result: {}", e)),
    }
}

#[tauri::command]
pub async fn extract_audio_waveform(
    audio_path: String,
    samples_per_second: u32,
) -> Result<Vec<f32>, String> {
    let path = Path::new(&audio_path);
    if !path.exists() {
        return Err(format!("Audio file not found: {}", audio_path));
    }

    println!("Extracting waveform from: {} at {} samples/sec", audio_path, samples_per_second);

    // Get audio duration
    let duration_output = Command::new("ffprobe")
        .args([
            "-v", "quiet",
            "-show_entries", "format=duration",
            "-of", "default=noprint_wrappers=1:nokey=1",
            &audio_path,
        ])
        .output()
        .map_err(|e| format!("Failed to run ffprobe: {}", e))?;

    let duration_str = String::from_utf8_lossy(&duration_output.stdout);
    let duration: f32 = duration_str.trim().parse().unwrap_or(0.0);

    if duration <= 0.0 {
        return Err("Could not determine audio duration".to_string());
    }

    let total_samples = (duration * samples_per_second as f32) as usize;
    println!("Audio duration: {}s, extracting {} peak samples", duration, total_samples);

    // Extract raw audio as f32 mono PCM
    let raw_output = Command::new("ffmpeg")
        .args([
            "-i", &audio_path,
            "-vn",
            "-ac", "1",
            "-ar", "8000",
            "-f", "f32le",
            "-",
        ])
        .output()
        .map_err(|e| format!("Failed to run ffmpeg: {}", e))?;

    if !raw_output.status.success() {
        return Err(format!("FFmpeg failed: {}", String::from_utf8_lossy(&raw_output.stderr)));
    }

    // Parse raw PCM data as f32 samples
    let raw_samples: Vec<f32> = raw_output.stdout
        .chunks_exact(4)
        .map(|chunk| f32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]))
        .collect();

    if raw_samples.is_empty() {
        return Err("No audio samples extracted".to_string());
    }

    // Compute peaks by downsampling
    let samples_per_peak = raw_samples.len() / total_samples.max(1);
    let mut peaks: Vec<f32> = Vec::with_capacity(total_samples);

    for i in 0..total_samples {
        let start = i * samples_per_peak;
        let end = ((i + 1) * samples_per_peak).min(raw_samples.len());

        if start < raw_samples.len() {
            let peak = raw_samples[start..end]
                .iter()
                .map(|s| s.abs())
                .fold(0.0f32, f32::max);
            peaks.push(peak.min(1.0));
        }
    }

    println!("Extracted {} waveform peaks", peaks.len());
    Ok(peaks)
}

#[tauri::command]
pub async fn read_sidecar_file(path: String) -> Result<String, String> {
    std::fs::read_to_string(&path).map_err(|e| e.to_string())
}

/// Pre-compute cursor trajectory using the same simulation as export.
/// Returns per-frame cursor states (x, y in video-normalized 0-1 coords).
#[tauri::command]
pub async fn compute_cursor_trajectory(
    mouse_json_path: String,
    duration_ms: u64,
    fps: u32,
    video_width: u32,
    video_height: u32,
    cursor_scale: f64,
) -> Result<String, String> {
    use crate::cursor_renderer::simulate_cursor_positions;
    use crate::video_processing::types::CursorSettings;
    use crate::video_processing::export::load_raw_cursor_events;

    let path = std::path::Path::new(&mouse_json_path);
    if !path.exists() {
        return Err(format!("Mouse events file not found: {}", mouse_json_path));
    }

    let events = load_raw_cursor_events(path, video_width, video_height);
    if events.is_empty() {
        return Err("No cursor events found".to_string());
    }

    let settings = CursorSettings {
        enabled: Some(true),
        size: Some(cursor_scale),
        style: Some("pointer".to_string()),
        speed_preset: Some("mellow".to_string()),
        spring_tension: Some(170.0),
        spring_friction: Some(30.0),
        spring_mass: Some(1.0),
        hide_when_idle: Some(false),
        idle_timeout: Some(3000),
        rotation: Some(0.0),
        rotate_while_moving: Some(false),
        rotation_intensity: Some(50.0),
        trail_enabled: Some(false),
        trail_length: Some(10),
        trail_opacity: Some(0.5),
        click_effect: Some("ripple".to_string()),
        highlight_clicks: Some(true),
        smoothing: Some(0.15),
        always_use_pointer: Some(false),
        color: Some("#ffffff".to_string()),
        highlight_color: Some("#ff6b6b".to_string()),
        ripple_color: Some("#64b4ff".to_string()),
        shadow_intensity: Some(30.0),
    };

    let spring = crate::video_processing::export::resolve_spring_preset("mellow");

    // No zoom trajectory for preview — zoom is handled by the video mesh transform
    let positions = simulate_cursor_positions(
        &events, &spring, fps as f64, duration_ms, &None, &settings,
    );

    // Serialize to compact JSON array of [x, y, opacity, isClicking, rippleProgress, rippleX, rippleY]
    let frames: Vec<Vec<f32>> = positions.iter().map(|p| {
        vec![p.x, p.y, p.opacity, p.is_clicking, p.ripple_progress, p.ripple_x, p.ripple_y]
    }).collect();

    serde_json::to_string(&frames).map_err(|e| e.to_string())
}
