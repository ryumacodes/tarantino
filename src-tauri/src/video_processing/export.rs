//! Video export with GPU-accelerated compositing.
//!
//! All per-frame operations (zoom, cursor blend, rounded corners, shadow,
//! motion blur, webcam overlay, device frame) run on the GPU via wgpu
//! compute shaders. FFmpeg is used only for decode and encode.

use std::path::{Path, PathBuf};
use std::io::{Write, BufReader, BufWriter, Read as IoRead};
use std::process::{Command, Stdio};
use anyhow::{Result, anyhow};

use super::types::{
    ExportSettings, ProcessingProgress, VideoInfo, ZoomBlock,
    validate_export_zoom_blocks, load_zoom_blocks_from_sidecar,
};
use super::zoom_trajectory::{ZoomFrameState, simulate_zoom_trajectory};
use super::codec_config::{build_codec_args, add_trim_settings};
use super::visual_effects::{
    get_cursor_config, determine_output_path, get_webcam_info,
};
use super::gpu_compositor::{GpuCompositor, build_gpu_config_with_webcam};
use crate::cursor_renderer::{
    SpringConfig, parse_cursor_events, CursorEvent,
    simulate_cursor_positions, parse_hex_rgb,
};

/// Apply video effects and export using GPU-accelerated compositing.
pub async fn export_video(
    _temp_dir: &Path,
    input_path: &Path,
    settings: ExportSettings,
    video_info: &VideoInfo,
    progress_callback: Option<Box<dyn Fn(ProcessingProgress) + Send + Sync>>,
) -> Result<PathBuf> {
    if !input_path.exists() {
        return Err(anyhow!("Input video file not found: {:?}", input_path));
    }

    println!("[Export] project_title from settings: {:?}", settings.project_title);

    let final_output_path = determine_output_path(&settings, input_path)?;
    println!("[Export] Output path will be: {:?}", final_output_path);

    if let Some(parent) = final_output_path.parent() {
        std::fs::create_dir_all(parent)?;
    }

    // If input and output are the same file, use a temporary file
    let same_file = input_path.canonicalize().ok() == final_output_path.canonicalize().ok()
        || input_path == final_output_path;
    let output_path = if same_file {
        let stem = final_output_path.file_stem().and_then(|s| s.to_str()).unwrap_or("export");
        let ext = final_output_path.extension().and_then(|s| s.to_str()).unwrap_or("mp4");
        let temp_name = format!("{}.export_temp.{}", stem, ext);
        final_output_path.parent().unwrap_or(Path::new("/tmp")).join(temp_name)
    } else {
        final_output_path.clone()
    };

    // Check for audio sidecar
    let audio_path = input_path.with_extension("wav");
    let has_audio_file = audio_path.exists();
    if has_audio_file {
        println!("Found audio file to mux: {}", audio_path.display());
    }

    // Check for webcam sidecar
    let webcam_info = get_webcam_info(input_path);

    let target_fps = settings.frame_rate.unwrap_or(60);
    let (source_width, source_height) = (video_info.width, video_info.height);
    let duration_ms = video_info.duration_ms;
    // Look for mouse.json sidecar — for processed_ files, fall back to original recording path
    let mouse_events_path = {
        let direct = input_path.with_extension("mouse.json");
        if direct.exists() {
            direct
        } else if let Some(fname) = input_path.file_name().and_then(|f| f.to_str()) {
            if fname.starts_with("processed_") {
                let original = input_path.with_file_name(fname.replacen("processed_", "", 1));
                let fallback = original.with_extension("mouse.json");
                if fallback.exists() { fallback } else { direct }
            } else {
                direct
            }
        } else {
            direct
        }
    };
    let cursor_enabled = settings.cursor_settings.as_ref()
        .and_then(|c| c.enabled)
        .unwrap_or(true);

    // Load and validate zoom blocks
    let zoom_blocks_to_use: Option<Vec<ZoomBlock>> = {
        let mut blocks = if settings.zoom_blocks.as_ref().map_or(false, |zb| !zb.is_empty()) {
            settings.zoom_blocks.clone()
        } else {
            load_zoom_blocks_from_sidecar(input_path)
        };
        if let Some(ref mut zb) = blocks {
            validate_export_zoom_blocks(zb, duration_ms);
        }
        blocks
    };

    // Pre-simulate zoom trajectory using spring physics
    let zoom_trajectory: Option<Vec<ZoomFrameState>> = if let Some(ref zoom_blocks) = zoom_blocks_to_use {
        if !zoom_blocks.is_empty() {
            let raw_cursor_events = load_raw_cursor_events(&mouse_events_path, source_width, source_height);
            let zoom_spring_config = get_zoom_spring_config(&settings);
            let cursor_spring_config = get_cursor_spring_config(&settings);

            println!("Simulating zoom trajectory with spring physics ({} blocks, {} cursor events)",
                zoom_blocks.len(), raw_cursor_events.len());

            let trajectory = simulate_zoom_trajectory(
                zoom_blocks,
                &raw_cursor_events,
                &zoom_spring_config,
                &cursor_spring_config,
                target_fps as f64,
                duration_ms,
            );
            println!("Zoom trajectory: {} frames", trajectory.len());
            Some(trajectory)
        } else {
            None
        }
    } else {
        None
    };

    // Simulate cursor positions with spring physics (GPU SDF rendering path — no image generation)
    let cursor_config = get_cursor_config(&settings);
    let is_window_mode = settings.capture_mode.as_deref() == Some("window");
    let cursor_trajectory = if cursor_enabled && mouse_events_path.exists() && duration_ms > 0 {
        let raw_events = load_raw_cursor_events(&mouse_events_path, source_width, source_height);
        let cursor_spring = get_cursor_spring_config(&settings);

        // Window mode: don't pre-transform cursor through zoom — the shader's inverse-zoom
        // remap handles this at the composite level. Display mode: pre-transform as before.
        let cursor_zoom_ref = if is_window_mode { &None } else { &zoom_trajectory };
        let positions = simulate_cursor_positions(
            &raw_events, &cursor_spring, target_fps as f64, duration_ms, cursor_zoom_ref, &cursor_config,
        );
        println!("Simulated {} cursor positions (spring physics, SDF rendering)", positions.len());

        Some(positions)
    } else {
        None
    };

    // Cursor rendering config (SDF — no pre-baked shape texture needed)
    let cursor_size = cursor_config.size.unwrap_or(1.0) as f32;
    let ripple_hex = cursor_config.ripple_color.as_deref().unwrap_or("#64b4ff");
    let (rr, rg, rb) = parse_hex_rgb(ripple_hex);
    let ripple_color = [rr as f32 / 255.0, rg as f32 / 255.0, rb as f32 / 255.0];

    // Placeholder cursor shape (1x1 to keep bind group layout stable)
    let placeholder_shape = image::RgbaImage::from_pixel(1, 1, image::Rgba([0, 0, 0, 0]));
    let cursor_shape_ref: Option<&image::RgbaImage> = if cursor_enabled { Some(&placeholder_shape) } else { None };

    // Resolve source dimensions: prefer frontend-provided, fallback to video_info
    let src_w = settings.source_width.or(Some(source_width));
    let src_h = settings.source_height.or(Some(source_height));

    // Build GPU compositor config
    let gpu_config = build_gpu_config_with_webcam(&settings, &webcam_info, src_w, src_h);
    let (out_width, out_height) = (gpu_config.output_width, gpu_config.output_height);
    let (input_width, input_height) = (gpu_config.input_width, gpu_config.input_height);

    println!("=== GPU EXPORT START ===");
    println!("  Input (decode): {}x{}", input_width, input_height);
    println!("  Output: {}x{} @ {}fps", out_width, out_height, target_fps);
    println!("  Zoom: {} frames", zoom_trajectory.as_ref().map_or(0, |t| t.len()));
    println!("  Cursor: {} positions", cursor_trajectory.as_ref().map_or(0, |t| t.len()));
    println!("  Motion blur: {}", gpu_config.motion_blur_enabled);
    println!("  Shadow: {}", gpu_config.shadow_enabled);
    println!("  Corner radius: {}", gpu_config.corner_radius);

    // Initialize GPU compositor (SDF cursor — pass cursor config for style/color)
    let cursor_settings_ref = if cursor_enabled { Some(&cursor_config) } else { None };
    let mut compositor = GpuCompositor::new(
        gpu_config,
        cursor_shape_ref,
        cursor_size,
        ripple_color,
        cursor_settings_ref,
    )?;

    // Spawn webcam decoder if webcam recording exists
    let webcam_frame_size: Option<(u32, u32)>;
    let mut webcam_decoder: Option<std::process::Child> = None;
    if let Some((ref webcam_path, _, _, _, _)) = webcam_info {
        if webcam_path.exists() {
            // Get webcam dimensions via ffprobe
            let probe_output = Command::new("ffprobe")
                .args([
                    "-v", "quiet", "-select_streams", "v:0",
                    "-show_entries", "stream=width,height",
                    "-of", "csv=p=0",
                    &webcam_path.to_string_lossy(),
                ])
                .output();
            let (wc_w, wc_h) = probe_output.ok()
                .and_then(|o| String::from_utf8(o.stdout).ok())
                .and_then(|s| {
                    let parts: Vec<&str> = s.trim().split(',').collect();
                    if parts.len() == 2 {
                        Some((parts[0].parse::<u32>().ok()?, parts[1].parse::<u32>().ok()?))
                    } else { None }
                })
                .unwrap_or((1920, 1080)); // fallback

            if wc_w > 0 && wc_h > 0 {
                webcam_frame_size = Some((wc_w, wc_h));

                // Initialize webcam texture on GPU
                compositor.set_webcam_texture(wc_w, wc_h);

                // Spawn FFmpeg to decode webcam to raw RGBA, same fps as main video
                let wc_decoder = Command::new("ffmpeg")
                    .args([
                        "-y",
                        "-i", &webcam_path.to_string_lossy(),
                        "-vf", &format!("fps={},scale={}:{}", target_fps, wc_w, wc_h),
                        "-f", "rawvideo",
                        "-pix_fmt", "rgba",
                        "-",
                    ])
                    .stdout(Stdio::piped())
                    .stderr(Stdio::null())
                    .spawn();

                match wc_decoder {
                    Ok(child) => {
                        webcam_decoder = Some(child);
                        println!("  Webcam decoder: {}x{} from {}", wc_w, wc_h, webcam_path.display());
                    }
                    Err(e) => println!("  Warning: Failed to spawn webcam decoder: {}", e),
                }
            } else {
                webcam_frame_size = None;
            }
        } else {
            webcam_frame_size = None;
        }
    } else {
        webcam_frame_size = None;
    }

    // Webcam frame buffer (allocated once, reused per frame)
    let mut webcam_frame_buffer: Option<Vec<u8>> = webcam_frame_size.map(|(w, h)| {
        vec![0u8; (w * h * 4) as usize]
    });
    let mut webcam_reader: Option<BufReader<std::process::ChildStdout>> = webcam_decoder.as_mut()
        .and_then(|d| d.stdout.take())
        .map(BufReader::new);

    // Spawn FFmpeg decoder (raw RGBA output)
    let mut decoder_args = vec![
        "-y".to_string(),
        #[cfg(target_os = "macos")]
        "-hwaccel".to_string(),
        #[cfg(target_os = "macos")]
        "videotoolbox".to_string(),
        "-threads".to_string(), "0".to_string(),
        "-i".to_string(), input_path.to_string_lossy().to_string(),
    ];

    // Add trim
    add_trim_settings(&mut decoder_args, &settings);

    // Decode filter: fps + scale to input dims (aspect-correct, all effects on GPU)
    decoder_args.extend([
        "-vf".to_string(),
        format!("fps={},scale={}:{}", target_fps, input_width, input_height),
        "-f".to_string(), "rawvideo".to_string(),
        "-pix_fmt".to_string(), "rgba".to_string(),
        "-".to_string(),
    ]);

    println!("  Decoder args: {:?}", decoder_args);

    let mut decoder = Command::new("ffmpeg")
        .args(&decoder_args)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| anyhow!("Failed to spawn decoder: {}", e))?;

    // Spawn FFmpeg encoder
    let mut encoder_args = vec![
        "-y".to_string(),
        "-f".to_string(), "rawvideo".to_string(),
        "-pix_fmt".to_string(), "rgba".to_string(),
        "-s".to_string(), format!("{}x{}", out_width, out_height),
        "-r".to_string(), format!("{}", target_fps),
        "-threads".to_string(), "0".to_string(),
        "-i".to_string(), "-".to_string(),
    ];

    if has_audio_file {
        encoder_args.extend([
            "-i".to_string(), audio_path.to_string_lossy().to_string(),
            "-map".to_string(), "0:v".to_string(),
            "-map".to_string(), "1:a".to_string(),
            "-c:a".to_string(), "aac".to_string(),
            "-b:a".to_string(), "192k".to_string(),
            "-ac".to_string(), "2".to_string(),
        ]);
    }

    encoder_args.extend(build_codec_args(&settings));
    encoder_args.extend([
        "-vf".to_string(),
        format!("fps={},format=yuv420p", target_fps),
        "-movflags".to_string(), "+faststart".to_string(),
    ]);
    encoder_args.push(output_path.to_string_lossy().to_string());

    println!("  Encoder args: {:?}", encoder_args);

    let mut encoder = Command::new("ffmpeg")
        .args(&encoder_args)
        .stdin(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| anyhow!("Failed to spawn encoder: {}", e))?;

    let decoder_stderr = decoder.stderr.take();
    let encoder_stderr = encoder.stderr.take();
    let decoder_stdout = decoder.stdout.take()
        .ok_or_else(|| anyhow!("Failed to get decoder stdout"))?;
    let encoder_stdin = encoder.stdin.take()
        .ok_or_else(|| anyhow!("Failed to get encoder stdin"))?;

    let input_frame_size = (input_width * input_height * 4) as usize;
    let output_frame_size = (out_width * out_height * 4) as usize;
    let mut reader = BufReader::with_capacity(input_frame_size * 4, decoder_stdout);
    let mut writer = BufWriter::with_capacity(output_frame_size * 4, encoder_stdin);

    let mut frame_buffer = vec![0u8; input_frame_size];
    let mut frame_idx: usize = 0;
    let start_time = std::time::Instant::now();
    let expected_frames = ((duration_ms as f64 * target_fps as f64) / 1000.0).ceil() as u64;
    let max_frames = ((expected_frames as f64) * 1.20).max(expected_frames as f64 + 120.0).ceil() as u64;

    // Default zoom state (no zoom)
    let default_zoom = ZoomFrameState { scale: 1.0, center_x: 0.5, center_y: 0.5 };

    // Drain stderr in background threads
    let decoder_stderr_handle = decoder_stderr.map(|stderr| {
        std::thread::spawn(move || {
            use std::io::BufRead;
            let reader = std::io::BufReader::new(stderr);
            reader.lines().filter_map(|l| l.ok()).collect::<Vec<_>>()
        })
    });
    let encoder_stderr_handle = encoder_stderr.map(|stderr| {
        std::thread::spawn(move || {
            use std::io::BufRead;
            let reader = std::io::BufReader::new(stderr);
            reader.lines().filter_map(|l| l.ok()).collect::<Vec<_>>()
        })
    });

    // Main frame processing loop: decode → GPU composite → encode
    loop {
        match reader.read_exact(&mut frame_buffer) {
            Ok(()) => {}
            Err(e) if e.kind() == std::io::ErrorKind::UnexpectedEof => {
                println!("  Decoder finished at frame {}", frame_idx);
                break;
            }
            Err(e) => return Err(anyhow!("Error reading frame {}: {}", frame_idx, e)),
        }

        // Get zoom state for this frame
        let zoom_state = zoom_trajectory.as_ref()
            .and_then(|t| {
                if t.is_empty() { None }
                else { Some(&t[frame_idx.min(t.len() - 1)]) }
            })
            .unwrap_or(&default_zoom);

        // Get cursor state for this frame
        let cursor_state = cursor_trajectory.as_ref()
            .and_then(|t| {
                if t.is_empty() { None }
                else { Some(&t[frame_idx.min(t.len() - 1)]) }
            });

        // Upload webcam frame to GPU (if available)
        if let (Some(ref mut wc_reader), Some(ref mut wc_buf), Some((wc_w, wc_h))) =
            (&mut webcam_reader, &mut webcam_frame_buffer, webcam_frame_size)
        {
            match wc_reader.read_exact(wc_buf) {
                Ok(()) => compositor.upload_webcam_frame(wc_buf, wc_w, wc_h),
                Err(_) => {} // Webcam video ended — last frame stays on GPU
            }
        }

        // GPU composite: zoom + blur + corners + shadow + cursor + webcam + device frame
        let composited = compositor.composite_frame(
            &frame_buffer,
            zoom_state,
            cursor_state,
        )?;

        writer.write_all(&composited)
            .map_err(|e| anyhow!("Error writing frame {}: {}", frame_idx, e))?;

        frame_idx += 1;

        if frame_idx as u64 > max_frames {
            println!("  WARNING: Frame count {} exceeded max limit {}, stopping", frame_idx, max_frames);
            let _ = decoder.kill();
            break;
        }

        // Progress callback every 30 frames
        if frame_idx % 30 == 0 {
            let actual_total = expected_frames.max(frame_idx as u64);
            let percentage = ((frame_idx as f64 / actual_total as f64) * 100.0).min(99.0);

            if let Some(ref cb) = progress_callback {
                let elapsed = start_time.elapsed().as_millis() as u64;
                let fps = frame_idx as f64 / (elapsed as f64 / 1000.0);
                let remaining_frames = actual_total.saturating_sub(frame_idx as u64);
                let estimated_remaining = if fps > 0.0 {
                    Some((remaining_frames as f64 / fps * 1000.0) as u64)
                } else {
                    None
                };

                cb(ProcessingProgress {
                    current_frame: frame_idx as u64,
                    total_frames: actual_total,
                    percentage,
                    estimated_remaining_ms: estimated_remaining,
                });
            }
            println!("  GPU frame {}/{} ({:.1}%)", frame_idx, expected_frames,
                ((frame_idx as f64 / expected_frames as f64) * 100.0).min(100.0));
        }
    }

    drop(writer);
    drop(webcam_reader);
    if let Some(mut wc) = webcam_decoder {
        let _ = wc.kill();
        let _ = wc.wait();
    }

    // Collect stderr
    if let Some(handle) = decoder_stderr_handle {
        if let Ok(lines) = handle.join() {
            let errors: Vec<_> = lines.iter()
                .filter(|l| l.contains("error") || l.contains("Error"))
                .collect();
            for line in errors.iter().take(5) {
                println!("  Decoder: {}", line);
            }
        }
    }
    if let Some(handle) = encoder_stderr_handle {
        if let Ok(lines) = handle.join() {
            let errors: Vec<_> = lines.iter()
                .filter(|l| l.contains("error") || l.contains("Error"))
                .collect();
            for line in errors.iter().take(5) {
                println!("  Encoder: {}", line);
            }
        }
    }

    let encoder_status = encoder.wait()
        .map_err(|e| anyhow!("Encoder wait failed: {}", e))?;
    if !encoder_status.success() {
        return Err(anyhow!("Encoder failed with status: {}", encoder_status));
    }

    let decoder_status = decoder.wait()
        .map_err(|e| anyhow!("Decoder wait failed: {}", e))?;
    if !decoder_status.success() {
        println!("  Decoder finished with status: {} (may have been terminated early)", decoder_status);
    }

    if let Some(ref cb) = progress_callback {
        cb(ProcessingProgress {
            current_frame: frame_idx as u64,
            total_frames: frame_idx as u64,
            percentage: 100.0,
            estimated_remaining_ms: Some(0),
        });
    }

    let elapsed = start_time.elapsed();
    let fps = frame_idx as f64 / elapsed.as_secs_f64();
    println!("=== GPU EXPORT COMPLETE: {} frames in {:.1}s ({:.1} fps) ===",
        frame_idx, elapsed.as_secs_f64(), fps);

    // Rename temp file if needed
    if same_file {
        std::fs::rename(&output_path, &final_output_path)?;
    }

    Ok(final_output_path)
}

/// Load raw cursor events from sidecar for zoom trajectory simulation.
pub fn load_raw_cursor_events(mouse_events_path: &Path, source_width: u32, source_height: u32) -> Vec<CursorEvent> {
    let content = match std::fs::read_to_string(mouse_events_path) {
        Ok(c) => c,
        Err(_) => return Vec::new(),
    };
    let sidecar: serde_json::Value = match serde_json::from_str(&content) {
        Ok(s) => s,
        Err(_) => return Vec::new(),
    };

    let (events, _sf, ex, ey, ew, eh) = if let Some(mouse_events) = sidecar.get("mouse_events") {
        let dw = sidecar.get("display_width").and_then(|v| v.as_f64()).unwrap_or(source_width as f64);
        let dh = sidecar.get("display_height").and_then(|v| v.as_f64()).unwrap_or(source_height as f64);
        let sf = sidecar.get("scale_factor").and_then(|v| v.as_f64()).unwrap_or(1.0);
        let recording_area = sidecar.get("recording_area");
        let (ex, ey, ew, eh) = if let Some(area) = recording_area {
            (
                area.get("x").and_then(|v| v.as_f64()).unwrap_or(0.0),
                area.get("y").and_then(|v| v.as_f64()).unwrap_or(0.0),
                area.get("width").and_then(|v| v.as_f64()).unwrap_or(dw),
                area.get("height").and_then(|v| v.as_f64()).unwrap_or(dh),
            )
        } else {
            (0.0, 0.0, dw, dh)
        };
        (mouse_events.as_array().cloned().unwrap_or_default(), sf, ex, ey, ew, eh)
    } else if let Some(arr) = sidecar.as_array() {
        (arr.clone(), 1.0, 0.0, 0.0, source_width as f64, source_height as f64)
    } else {
        return Vec::new();
    };

    parse_cursor_events(&events, 1.0, ex, ey, ew, eh)
}

fn get_zoom_spring_config(settings: &ExportSettings) -> SpringConfig {
    if let (Some(t), Some(f), Some(m)) = (
        settings.zoom_spring_tension,
        settings.zoom_spring_friction,
        settings.zoom_spring_mass,
    ) {
        return SpringConfig { tension: t, friction: f, mass: m };
    }
    let name = settings.animation_speed.as_deref().unwrap_or("mellow");
    resolve_spring_preset(name)
}

fn get_cursor_spring_config(settings: &ExportSettings) -> SpringConfig {
    if let Some(ref cursor) = settings.cursor_settings {
        if let (Some(t), Some(f), Some(m)) = (
            cursor.spring_tension,
            cursor.spring_friction,
            cursor.spring_mass,
        ) {
            return SpringConfig { tension: t, friction: f, mass: m };
        }
        if let Some(ref preset) = cursor.speed_preset {
            return resolve_spring_preset(preset);
        }
    }
    SpringConfig { tension: 170.0, friction: 30.0, mass: 1.0 }
}

pub fn resolve_spring_preset(name: &str) -> SpringConfig {
    match name {
        "slow" => SpringConfig { tension: 120.0, friction: 28.0, mass: 1.0 },
        "mellow" => SpringConfig { tension: 170.0, friction: 30.0, mass: 1.0 },
        "quick" => SpringConfig { tension: 280.0, friction: 38.0, mass: 1.0 },
        "rapid" => SpringConfig { tension: 400.0, friction: 44.0, mass: 1.0 },
        _ => SpringConfig { tension: 170.0, friction: 30.0, mass: 1.0 },
    }
}
