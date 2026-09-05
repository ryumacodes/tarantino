//! Video processing commands

use crate::state::UnifiedAppState;
use crate::video_processing::{ExportSettings, ProcessingProgress, VideoInfo, VideoProcessor};
use base64::{Engine as _, engine::general_purpose::STANDARD};
use std::path::Path;
use std::process::Command;
use std::sync::Arc;

const LINUX_PREVIEW_FPS: f64 = 30.0;
use tauri::{AppHandle, Emitter, State};

#[tauri::command]
pub fn get_video_output_directory() -> String {
    crate::video_processing::visual_effects::default_video_output_directory()
        .to_string_lossy()
        .into_owned()
}

#[tauri::command]
pub async fn get_video_info(video_path: String) -> Result<VideoInfo, String> {
    let processor = VideoProcessor::new().map_err(|e| e.to_string())?;
    processor
        .get_video_info(video_path)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn get_video_metadata(
    #[allow(non_snake_case)] filePath: String,
) -> Result<VideoInfo, String> {
    println!("Extracting video metadata for: {}", filePath);
    let processor = VideoProcessor::new().map_err(|e| e.to_string())?;
    processor
        .get_video_info(filePath)
        .await
        .map_err(|e| e.to_string())
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
        let _ = app_handle.emit(
            "thumbnail_ready",
            serde_json::json!({
                "index": index,
                "path": path.to_string_lossy()
            }),
        );
    });

    let thumbnails = processor
        .extract_thumbnails(video_path, thumbnail_count, thumbnail_width, Some(callback))
        .await
        .map_err(|e| e.to_string())?;

    read_thumbnail_data_urls(thumbnails)
}

fn read_thumbnail_data_urls(paths: Vec<std::path::PathBuf>) -> Result<Vec<String>, String> {
    // Process every path even if an earlier read fails, so later temporary
    // files are cleaned up too. Preserve the first reported error.
    let results: Vec<_> = paths
        .iter()
        .map(|path| read_thumbnail_data_url(path))
        .collect();
    results.into_iter().collect()
}

fn read_thumbnail_data_url(path: &Path) -> Result<String, String> {
    let source = std::fs::read(path)
        .map(|bytes| thumbnail_data_url(&bytes))
        .map_err(|error| {
            format!(
                "Failed to read generated thumbnail {}: {error}",
                path.display()
            )
        });
    let cleanup = std::fs::remove_file(path).map_err(|error| {
        format!(
            "Failed to remove generated thumbnail {}: {error}",
            path.display()
        )
    });

    match (source, cleanup) {
        (Ok(source), Ok(())) => Ok(source),
        (Err(error), Ok(())) | (Ok(_), Err(error)) => Err(error),
        (Err(read_error), Err(cleanup_error)) => Err(format!("{read_error}; {cleanup_error}")),
    }
}

#[tauri::command]
pub async fn extract_video_preview_frames(
    video_path: String,
    start_bucket: u64,
    frame_count: u32,
    preview_width: u32,
) -> Result<Vec<String>, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let width = preview_width.clamp(320, 1920);
        let count = frame_count.clamp(1, 60);
        let seek = start_bucket as f64 / LINUX_PREVIEW_FPS;
        let output = Command::new("ffmpeg")
            .args([
                "-v",
                "error",
                "-hwaccel",
                "none",
                "-ss",
                &format!("{seek:.3}"),
                "-i",
                &video_path,
                "-vf",
                &format!("fps=30,scale={width}:-2:flags=fast_bilinear"),
                "-frames:v",
                &count.to_string(),
                "-c:v",
                "mjpeg",
                "-q:v",
                "4",
                "-f",
                "image2pipe",
                "pipe:1",
            ])
            .output()
            .map_err(|error| format!("Failed to start Linux preview decoder: {error}"))?;

        if !output.status.success() {
            return Err(format!(
                "Linux preview decoder failed: {}",
                String::from_utf8_lossy(&output.stderr).trim()
            ));
        }

        let frames = split_jpeg_stream(&output.stdout);
        if frames.is_empty() {
            return Err("Linux preview decoder produced an invalid JPEG stream".to_string());
        }

        Ok(frames.into_iter().map(thumbnail_data_url).collect())
    })
    .await
    .map_err(|error| format!("Linux preview decoder task failed: {error}"))?
}

fn split_jpeg_stream(bytes: &[u8]) -> Vec<&[u8]> {
    let mut frames = Vec::new();
    let mut cursor = 0;
    while cursor + 1 < bytes.len() {
        let Some(start_offset) = bytes[cursor..]
            .windows(2)
            .position(|marker| marker == [0xff, 0xd8])
        else {
            break;
        };
        let start = cursor + start_offset;
        let Some(end_offset) = bytes[start + 2..]
            .windows(2)
            .position(|marker| marker == [0xff, 0xd9])
        else {
            break;
        };
        let end = start + 2 + end_offset + 2;
        frames.push(&bytes[start..end]);
        cursor = end;
    }
    frames
}

fn thumbnail_data_url(bytes: &[u8]) -> String {
    format!("data:image/jpeg;base64,{}", STANDARD.encode(bytes))
}

#[tauri::command]
pub fn linux_native_preview_required() -> bool {
    #[cfg(target_os = "linux")]
    {
        if std::env::var_os("TARANTINO_FORCE_NATIVE_PREVIEW").is_some() {
            return true;
        }

        let virtual_machine = [
            "/sys/class/dmi/id/product_name",
            "/sys/class/dmi/id/sys_vendor",
            "/sys/class/dmi/id/board_vendor",
        ]
        .into_iter()
        .filter_map(|path| std::fs::read_to_string(path).ok())
        .collect::<Vec<_>>()
        .join(" ")
        .to_ascii_lowercase();

        return ["vmware", "virtualbox", "qemu", "kvm", "hyper-v"]
            .iter()
            .any(|vendor| virtual_machine.contains(vendor));
    }

    #[cfg(not(target_os = "linux"))]
    false
}

#[cfg(test)]
mod tests {
    use super::{
        read_thumbnail_data_url, read_thumbnail_data_urls, split_jpeg_stream, thumbnail_data_url,
    };

    #[test]
    fn thumbnail_failure_still_cleans_up_later_files() {
        let root = std::env::temp_dir().join(format!("thumbnail-cleanup-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&root).unwrap();
        let missing = root.join("missing.jpg");
        let valid = root.join("valid.jpg");
        std::fs::write(&valid, [0xff, 0xd8, 0xff]).unwrap();
        let error = read_thumbnail_data_urls(vec![missing, valid.clone()]).unwrap_err();
        assert!(error.contains("Failed to read generated thumbnail"));
        assert!(!valid.exists());
        std::fs::remove_dir(root).unwrap();
    }

    #[test]
    fn thumbnail_source_is_a_self_contained_jpeg_data_url() {
        assert_eq!(
            thumbnail_data_url(&[0xff, 0xd8, 0xff]),
            "data:image/jpeg;base64,/9j/"
        );
    }

    #[test]
    fn splits_concatenated_preview_jpegs() {
        let stream = [
            &[0x00, 0xff, 0xd8, 0x01, 0xff, 0xd9][..],
            &[0xff, 0xd8, 0x02, 0x03, 0xff, 0xd9, 0x00][..],
        ]
        .concat();
        let frames = split_jpeg_stream(&stream);
        assert_eq!(frames.len(), 2);
        assert_eq!(frames[0], [0xff, 0xd8, 0x01, 0xff, 0xd9]);
        assert_eq!(frames[1], [0xff, 0xd8, 0x02, 0x03, 0xff, 0xd9]);
    }

    #[test]
    fn thumbnail_source_removes_temporary_file() {
        let path = std::env::temp_dir().join(format!(
            "tarantino-thumbnail-test-{}.jpg",
            std::process::id()
        ));
        std::fs::write(&path, [0xff, 0xd8, 0xff]).unwrap();

        let source = read_thumbnail_data_url(&path).unwrap();

        assert_eq!(source, "data:image/jpeg;base64,/9j/");
        assert!(!path.exists());
    }
}

#[tauri::command]
pub async fn export_video(
    app: AppHandle,
    window: tauri::WebviewWindow,
    state: State<'_, Arc<UnifiedAppState>>,
    input_path: String,
    settings: ExportSettings,
) -> Result<String, String> {
    if state.get_app_status().await.is_recording {
        return Err("Finish the active recording before exporting.".to_string());
    }

    crate::commands::lifecycle::release_recording_surfaces_except(
        &app,
        Some(state.inner()),
        "export",
        Some(window.label()),
    );

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

            processor
                .export_video(input_path, settings, Some(Box::new(progress_callback)))
                .await
                .map_err(|e| e.to_string())
        });

        let _ = result_tx.send(result);
    });

    loop {
        match progress_rx.try_recv() {
            Ok(progress) => {
                let _ = app.emit(
                    "export:progress",
                    serde_json::json!({
                        "current": progress.current_frame,
                        "total": progress.total_frames,
                        "percentage": progress.percentage,
                    }),
                );
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

    println!(
        "Extracting waveform from: {} at {} samples/sec",
        audio_path, samples_per_second
    );

    // Get audio duration
    let duration_output = Command::new("ffprobe")
        .args([
            "-v",
            "quiet",
            "-show_entries",
            "format=duration",
            "-of",
            "default=noprint_wrappers=1:nokey=1",
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
    println!(
        "Audio duration: {}s, extracting {} peak samples",
        duration, total_samples
    );

    // Extract raw audio as f32 mono PCM
    let raw_output = Command::new("ffmpeg")
        .args([
            "-i",
            &audio_path,
            "-vn",
            "-ac",
            "1",
            "-ar",
            "8000",
            "-f",
            "f32le",
            "-",
        ])
        .output()
        .map_err(|e| format!("Failed to run ffmpeg: {}", e))?;

    if !raw_output.status.success() {
        return Err(format!(
            "FFmpeg failed: {}",
            String::from_utf8_lossy(&raw_output.stderr)
        ));
    }

    // Parse raw PCM data as f32 samples
    let raw_samples: Vec<f32> = raw_output
        .stdout
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
    hide_cursor_when_idle: bool,
    idle_timeout: u64,
    stop_cursor_at_end: bool,
    stop_cursor_duration: u64,
    loop_cursor_position: bool,
) -> Result<String, String> {
    use crate::cursor_renderer::simulate_cursor_positions;
    use crate::video_processing::export::load_raw_cursor_events;
    use crate::video_processing::types::CursorSettings;

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
        hide_when_idle: Some(hide_cursor_when_idle),
        idle_timeout: Some(idle_timeout),
        stop_at_end: Some(stop_cursor_at_end),
        stop_duration_ms: Some(stop_cursor_duration),
        loop_to_start: Some(loop_cursor_position),
        loop_duration_ms: Some(500),
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
    let positions =
        simulate_cursor_positions(&events, &spring, fps as f64, duration_ms, &None, &settings);

    // Serialize to compact JSON array of [x, y, opacity, isClicking, rippleProgress, rippleX, rippleY]
    let frames: Vec<Vec<f32>> = positions
        .iter()
        .map(|p| {
            vec![
                p.x,
                p.y,
                p.opacity,
                p.is_clicking,
                p.ripple_progress,
                p.ripple_x,
                p.ripple_y,
            ]
        })
        .collect();

    serde_json::to_string(&frames).map_err(|e| e.to_string())
}
