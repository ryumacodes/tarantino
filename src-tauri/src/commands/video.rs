//! Video processing commands

pub mod waveform;
pub mod cursor_images;
pub mod silence;

mod edited_cursor;
pub mod thumbnail_samples;

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
        let output = Command::new(crate::ffmpeg::binary("ffmpeg"))
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
    #[tokio::test]
    async fn preview_fixed_pointer_suppresses_shape_motion_constraints() {
        let path = std::env::temp_dir().join(format!("cursor-shape-preview-{}.json",uuid::Uuid::new_v4()));
        let events=(0..=100).map(|i|serde_json::json!({"timestamp":i*10,"x":i*4,"y":50,
            "event_type":"Move"})).collect::<Vec<_>>();
        std::fs::write(&path,serde_json::to_vec(&serde_json::json!({
            "display_width":1000,"display_height":500,"mouse_events":events,
            "cursor_images":{"images":[{},{}],"changes":[
                {"timestamp":0,"image_id":0},{"timestamp":250,"image_id":1}]}
        })).unwrap()).unwrap();
        let mut errors=Vec::new();
        for fixed in [false,true] {
            let response=super::compute_cursor_trajectory(path.to_string_lossy().into_owned(),1000,120,1000,500,
                1.0,false,2000,false,0,false,None,None,Some(470.0),Some(70.0),Some(3.0),
                None,None,None, None,None,Some(fixed),Some(false), None).await.unwrap();
            let frames:Vec<Vec<f32>>=serde_json::from_str(&response).unwrap();
            errors.push((frames[30][0] as f64*1000.0-100.0).abs());
        }
        std::fs::remove_file(path).unwrap();
        assert!(errors[0]<=6.0,"shape constraint error: {}",errors[0]);
        assert!(errors[1]>errors[0]+1.0,"fixed pointer should not anticipate a hidden shape change: {errors:?}");
    }

    #[tokio::test]
    async fn preview_cursor_uses_the_same_crop_coordinates_as_export() {
        let path = std::env::temp_dir().join(format!("cursor-crop-{}.json",uuid::Uuid::new_v4()));
        std::fs::write(&path,r#"[{"timestamp":0,"x":96,"y":32}]"#).unwrap();
        let crop = crate::video_processing::source_crop::SourceCrop{x:0.5,y:0.0,width:0.5,height:1.0};
        let result = super::compute_cursor_trajectory(path.to_string_lossy().into_owned(),100,60,128,64,
            1.0,false,2000,false,0,false,None,Some(crop),None,None,None,None,None,None, None,None,None,Some(false), None).await.unwrap();
        let frames:Vec<Vec<f32>>=serde_json::from_str(&result).unwrap();
        assert!(frames.iter().all(|frame| frame[0]==0.5 && frame[1]==0.5 && frame[2]==1.0));
        std::fs::remove_file(path).unwrap();
    }

    #[tokio::test]
    async fn preview_cursor_command_applies_selected_spring() {
        let path =
            std::env::temp_dir().join(format!("cursor-motion-{}.json", uuid::Uuid::new_v4()));
        std::fs::write(
            &path,
            r#"[{"timestamp":0,"x":20,"y":20},{"timestamp":500,"x":80,"y":80}]"#,
        )
        .unwrap();
        let mut positions = Vec::new();
        for (tension, friction, mass) in [(1.0, 0.0, 0.0), (470.0, 70.0, 3.0)] {
            let result = super::compute_cursor_trajectory(
                path.to_string_lossy().into_owned(),
                1000,
                60,
                100,
                100,
                3.0,
                false,
                2000,
                false,
                0,
                false,
                None,
                None,
                Some(tension),
                Some(friction),
                Some(mass),
                None,None,None, None,None,None,Some(false), None)
            .await
            .unwrap();
            let frames: Vec<Vec<f32>> = serde_json::from_str(&result).unwrap();
            positions.push(frames[30][0]);
        }
        std::fs::remove_file(path).unwrap();
        assert!((positions[0] - 0.8).abs() < 0.0001);
        assert!(positions[1] < positions[0]);
    }
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
    mut settings: ExportSettings,
) -> Result<String, String> {
    let job = super::export_jobs::ExportJob::begin(window.label())?;
    settings.cancellation = Some(job.control.clone());
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

    let _ = window.emit("export:progress", serde_json::json!({"current":0,"total":0,"percentage":0}));
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

        let result = if job.control.check().is_err() {
            Err(crate::video_processing::export_control::CANCELLED.into())
        } else { result };
        drop(job);
        let _ = result_tx.send(result);
    });

    loop {
        match progress_rx.try_recv() {
            Ok(progress) => {
                let _ = window.emit(
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
    waveform::extract(&audio_path,samples_per_second).await.map(|data|data.peaks)
}

#[tauri::command]
pub async fn read_sidecar_file(path: String) -> Result<String, String> {
    std::fs::read_to_string(&path).map_err(|e| e.to_string())
}

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HoverRequest {
    recording_path: String,
    enabled: bool,
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
    shake_threshold: Option<f64>,
    source_crop: Option<crate::video_processing::source_crop::SourceCrop>,
    spring_tension: Option<f64>,
    spring_friction: Option<f64>,
    spring_mass: Option<f64>,
    rotation: Option<f64>,
    rotate_while_moving: Option<bool>,
    rotation_intensity: Option<f64>,
    rotation_mode: Option<String>,
    edit: Option<edited_cursor::EditedCursorRequest>,
    always_use_pointer: Option<bool>,
    optimize_cursor_types: Option<bool>,
    hover: Option<HoverRequest>,
) -> Result<String, String> {
    use crate::cursor_renderer::simulate_cursor_frames_from;
    use crate::video_processing::export::parse_filtered_cursor_events;
    use crate::video_processing::types::CursorSettings;

    let spring = crate::cursor_renderer::SpringConfig {
        tension: spring_tension.unwrap_or(170.0),
        friction: spring_friction.unwrap_or(30.0),
        mass: spring_mass.unwrap_or(1.0),
    };
    if !spring.tension.is_finite()
        || spring.tension <= 0.0
        || !spring.friction.is_finite()
        || spring.friction < 0.0
        || !spring.mass.is_finite()
        || spring.mass < 0.0
        || rotation.is_some_and(|value| !value.is_finite())
        || rotation_intensity.is_some_and(|value| !value.is_finite())
        || rotation_mode.as_deref().is_some_and(|mode| !matches!(mode,"horizontal"|"directional"))
        || !(1..=240).contains(&fps)
    {
        return Err("Invalid cursor motion settings".to_string());
    }
    tauri::async_runtime::spawn_blocking(move || {
        let path = std::path::Path::new(&mouse_json_path);
        if !path.exists() {
            return Err(format!("Mouse events file not found: {}", mouse_json_path));
        }

        let (activity, mut events) = {
            let bytes = std::fs::read(path).map_err(|error| error.to_string())?;
            let input = serde_json::from_slice(&bytes).map_err(|error| error.to_string())?;
            (crate::cursor_renderer::typing_activity::TypingActivity::from_value(&input),
                parse_filtered_cursor_events(&input, video_width, video_height, shake_threshold.unwrap_or(0.0)))
        };
        let typing = |time| activity.active(time);
        let crop = source_crop.map(|rect| rect.resolve(video_width,video_height)).transpose().map_err(|error| error.to_string())?;
        crate::video_processing::source_crop::remap_motion(crop,&mut events,&mut []);
        if events.is_empty() {
            return Err("No cursor events found".to_string());
        }

        let settings = CursorSettings {
            shake_threshold,
            enabled: Some(true),
            size: Some(cursor_scale),
            style: Some("pointer".to_string()),
            speed_preset: Some("mellow".to_string()),
            spring_tension: Some(spring.tension),
            spring_friction: Some(spring.friction),
            spring_mass: Some(spring.mass),
            hide_when_idle: Some(hide_cursor_when_idle),
            idle_timeout: Some(idle_timeout),
            stop_at_end: Some(stop_cursor_at_end),
            stop_duration_ms: Some(stop_cursor_duration),
            loop_to_start: Some(loop_cursor_position),
            loop_duration_ms: Some(500),
            rotation,
            rotate_while_moving,
            rotation_mode,
            rotation_intensity,
            trail_enabled: Some(false),
            trail_length: Some(10),
            trail_opacity: Some(0.5),
            click_effect: Some("ripple".to_string()),
            highlight_clicks: Some(true),
            smoothing: Some(0.15),
            always_use_pointer,
            optimize_cursor_types,
            follow_hover: hover.as_ref().map(|request| request.enabled),
            color: Some("#ffffff".to_string()),
            highlight_color: Some("#ff6b6b".to_string()),
            ripple_color: Some("#64b4ff".to_string()),
            shadow_intensity: Some(30.0),
        };

        // Project directly to compact arrays without allocating unused trail data.
        let rotation_points = if edit.is_some() && rotate_while_moving.unwrap_or(false) {
            crate::video_processing::export::load_rotation_points(path,shake_threshold.unwrap_or(0.0))
        } else {Vec::new()};
        if let Some(request)=edit.as_ref() {
            if let Some(result)=edited_cursor::prepare_with_input(request,&events,&rotation_points,&settings,&spring,duration_ms,fps,Some(&activity),
                Some((path,(video_width,video_height),source_crop)))? {
                return Ok(result);
            }
        }
        let recording = hover.as_ref().map_or(path, |request| Path::new(&request.recording_path));
        let planned=crate::cursor_renderer::planner::runtime::prepare(path,(video_width,video_height),source_crop,None,recording,
            duration_ms as f64,&settings,spring,None).map_err(|e|e.to_string())?;
        let rotation_keys = crate::cursor_renderer::rotation::prepared_or_retimed_targets(
            planned.as_ref(), &settings, duration_ms, || {
                if edit.is_none() && rotate_while_moving.unwrap_or(false) {
                    crate::video_processing::export::load_rotation_points(path,shake_threshold.unwrap_or(0.0))
                } else { rotation_points }
            });
        let position=|time|planned.as_ref().unwrap().at(time);
        let pressed=|time|planned.as_ref().unwrap().pressed_at_source(time,spring.mass!=0.0);
        let frames = simulate_cursor_frames_from(
            &events,
            &spring,
            fps as f64,
            duration_ms,
            &None,
            &settings,
            0,
            fps as f64,
            Some(&rotation_keys),
            Some(&typing),
            planned.as_ref().map(|_|&position as &dyn Fn(f64)->[f64;2]),
            planned.as_ref().map(|_|(&pressed as &dyn Fn(f64)->bool,60.0)),
            |p| {
                [
                    p.x,
                    p.y,
                    p.opacity,
                    p.is_clicking,
                    p.ripple_progress,
                    p.ripple_x,
                    p.ripple_y,
                    p.rotation,
                    p.fade_blur_radius,
                    p.press_amount,
                ]
            },
        );
        serde_json::to_string(&frames).map_err(|e| e.to_string())
    })
    .await
    .map_err(|error| error.to_string())?
}

#[cfg(test)]
mod cursor_rotation_tests {
    #[tokio::test]
    async fn preview_rotation_matches_export_simulation() {
        use crate::cursor_renderer::{simulate_cursor_positions_for_range_with_rotation,SpringConfig};
        let path=std::env::temp_dir().join(format!("cursor-rotation-{}.json",uuid::Uuid::new_v4()));
        std::fs::write(&path,r#"[{"timestamp":0,"x":20,"y":20},{"timestamp":500,"x":80,"y":20},{"timestamp":800,"x":30,"y":20}]"#).unwrap();
        let events=crate::video_processing::export::load_filtered_cursor_events(&path,100,100,0.0);
        for (dynamic, mode) in [(false,None),(true,None),(false,Some("directional")),(true,Some("directional"))] {
            let result=super::compute_cursor_trajectory(path.to_string_lossy().into_owned(),1000,120,100,100,
                1.0,false,2000,false,0,false,None,None,None,None,None,Some(45.0),Some(dynamic),Some(100.0), mode.map(str::to_owned),None,None,Some(false), None).await.unwrap();
            let frames:Vec<Vec<f32>>=serde_json::from_str(&result).unwrap();
            let settings=serde_json::from_value(serde_json::json!({"rotation":45,"rotate_while_moving":dynamic,"rotation_intensity":100,"rotation_mode":mode})).unwrap();
            let points=crate::video_processing::export::load_rotation_points(&path,0.0);
            let keys=crate::cursor_renderer::rotation::targets(&points,&settings);
            let expected=simulate_cursor_positions_for_range_with_rotation(&events,&SpringConfig{tension:170.0,friction:30.0,mass:1.0},120.0,
                crate::video_processing::export_range::ExportRange {start_ms:0,end_ms:1000},&None,&settings,Some(&keys))
                .into_iter().map(|p|p.rotation).collect::<Vec<_>>();
            assert_eq!(frames.iter().map(|p|p[7]).collect::<Vec<_>>(),expected);
            if dynamic {
                assert!(frames.iter().any(|p|(p[7]-45.0).abs()>1.0));
                let resized=super::compute_cursor_trajectory(path.to_string_lossy().into_owned(),1000,120,400,300,
                    1.0,false,2000,false,0,false,None,None,None,None,None,Some(45.0),Some(true),Some(100.0), mode.map(str::to_owned),None,None,Some(false), None).await.unwrap();
                let resized:Vec<Vec<f32>>=serde_json::from_str(&resized).unwrap();
                assert_eq!(frames.iter().map(|p|p[7]).collect::<Vec<_>>(),resized.iter().map(|p|p[7]).collect::<Vec<_>>());
            }
            else {assert!(frames.iter().all(|p|p[7]==45.0));}
        }
        std::fs::remove_file(path).unwrap();
    }
}
