//! Cursor frame generation and video compositing
//!
//! Generates cursor overlay frames and composites them with video,
//! applying zoom/pan transforms using the spring physics trajectory
//! (matching the preview pipeline exactly).

use std::path::Path;
use std::io::{Write, BufWriter, BufReader, Read as IoRead};
use std::process::{Command, Stdio};
use anyhow::{Result, anyhow};
use image::RgbaImage;

use super::types::{CursorSettings, ProcessingProgress};
use super::zoom_trajectory::ZoomFrameState;
use crate::cursor_renderer::{CursorRenderer, parse_cursor_events};

/// Transform cursor coordinates through zoom. Returns None if cursor is off-screen.
fn transform_cursor_through_zoom(x: f64, y: f64, zoom: f64, center_x: f64, center_y: f64) -> Option<(f64, f64)> {
    if zoom <= 1.0 {
        return Some((x, y)); // No zoom active
    }
    // Transform: cursor position in zoomed output space
    // The visible region is centered at (center_x, center_y) with size (1/zoom, 1/zoom)
    let new_x = (x - center_x) * zoom + 0.5;
    let new_y = (y - center_y) * zoom + 0.5;

    // Check if cursor is within visible area
    if new_x >= 0.0 && new_x <= 1.0 && new_y >= 0.0 && new_y <= 1.0 {
        Some((new_x, new_y))
    } else {
        None // Cursor is off-screen after zoom
    }
}

/// Generate cursor frames in memory (no disk I/O).
///
/// Uses the pre-computed zoom trajectory (spring physics) for cursor position transforms
/// instead of the old smoothstep approximation, ensuring 1:1 match with preview.
pub fn generate_cursor_frames(
    mouse_events_path: &Path,
    cursor_settings: &CursorSettings,
    width: u32,
    height: u32,
    fps: f64,
    duration_ms: u64,
    zoom_trajectory: &Option<Vec<ZoomFrameState>>,
) -> Result<Vec<RgbaImage>> {
    println!("=== CURSOR FRAME GENERATION START ===");
    println!("  Path: {:?}", mouse_events_path);
    println!("  Dimensions: {}x{} @ {}fps for {}ms", width, height, fps, duration_ms);
    println!("  Settings: enabled={:?}, style={:?}", cursor_settings.enabled, cursor_settings.style);

    // Read mouse events from sidecar
    let content = std::fs::read_to_string(mouse_events_path)?;
    let sidecar: serde_json::Value = serde_json::from_str(&content)?;

    // Extract scale_factor and recording_area for coordinate normalization
    let (events, scale_factor, eff_x, eff_y, eff_w, eff_h) = if let Some(mouse_events) = sidecar.get("mouse_events") {
        let dw = sidecar.get("display_width").and_then(|v| v.as_f64()).unwrap_or(width as f64);
        let dh = sidecar.get("display_height").and_then(|v| v.as_f64()).unwrap_or(height as f64);
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
        (arr.clone(), 1.0, 0.0, 0.0, width as f64, height as f64)
    } else {
        return Err(anyhow!("Invalid sidecar format"));
    };

    // Parse cursor events with proper coordinate normalization
    println!("  Parsing {} raw events, scale_factor={}, area=({},{} {}x{})",
             events.len(), scale_factor, eff_x, eff_y, eff_w, eff_h);
    let mut cursor_events = parse_cursor_events(&events, scale_factor, eff_x, eff_y, eff_w, eff_h);
    println!("  Parsed {} cursor events", cursor_events.len());

    // Apply zoom transforms using the spring physics trajectory (matches preview exactly)
    if let Some(ref trajectory) = zoom_trajectory {
        if !trajectory.is_empty() {
            println!("  Applying spring-physics zoom trajectory ({} frames) to cursor coordinates", trajectory.len());
            for event in cursor_events.iter_mut() {
                // Map event timestamp → trajectory frame index
                let frame_idx = ((event.timestamp_ms as f64 * fps) / 1000.0) as usize;
                let frame_idx = frame_idx.min(trajectory.len().saturating_sub(1));
                let zoom_state = &trajectory[frame_idx];

                if let Some((new_x, new_y)) = transform_cursor_through_zoom(
                    event.x, event.y, zoom_state.scale, zoom_state.center_x, zoom_state.center_y
                ) {
                    event.x = new_x;
                    event.y = new_y;
                } else {
                    // Cursor is off-screen, move it way off so it won't render
                    event.x = -10.0;
                    event.y = -10.0;
                }
            }
        }
    }

    // Log first few events to verify coordinate normalization
    for (i, event) in cursor_events.iter().take(3).enumerate() {
        println!("  Event {}: t={}ms, pos=({:.4}, {:.4}), click={}",
                 i, event.timestamp_ms, event.x, event.y, event.is_click);
    }

    if cursor_events.is_empty() {
        return Err(anyhow!("No cursor events found"));
    }

    // Calculate duration from mouse events as a sanity check
    let events_duration_ms = cursor_events.iter()
        .map(|e| e.timestamp_ms)
        .max()
        .unwrap_or(0) + 100;

    let effective_duration_ms = duration_ms.max(events_duration_ms);

    if effective_duration_ms != duration_ms {
        println!("  Duration corrected: passed={}ms, events_max={}ms, using={}ms",
                 duration_ms, events_duration_ms, effective_duration_ms);
    }

    // Create cursor renderer with settings
    let mut renderer = CursorRenderer::new(cursor_settings.clone(), width, height, fps);

    // Generate all frames in memory
    let frames = renderer.generate_frames(&cursor_events, effective_duration_ms)?;
    println!("=== CURSOR FRAME GENERATION COMPLETE: {} frames ===", frames.len());

    Ok(frames)
}

/// Export video with cursor composited in Rust (bypasses FFmpeg overlay stall).
///
/// When a zoom trajectory is provided, applies per-frame zoom/pan transforms
/// using the same spring physics as the preview (no FFmpeg zoompan filter needed).
pub fn export_with_cursor_compositing(
    input_path: &Path,
    output_path: &Path,
    cursor_frames: &[RgbaImage],
    width: u32,
    height: u32,
    decoder_fps: u32,
    output_fps: u32,
    duration_ms: u64,
    audio_path: Option<&Path>,
    codec_args: &[String],
    pre_filters: Option<&str>,
    zoom_trajectory: &Option<Vec<ZoomFrameState>>,
    progress_callback: Option<&Box<dyn Fn(ProcessingProgress) + Send + Sync>>,
) -> Result<()> {
    println!("=== RUST COMPOSITING START ===");
    println!("  Input: {:?}", input_path);
    println!("  Output: {:?}", output_path);
    println!("  Dimensions: {}x{} @ decoder_fps={}, output_fps={}", width, height, decoder_fps, output_fps);
    println!("  Cursor frames available: {}", cursor_frames.len());
    println!("  Zoom trajectory: {} frames", zoom_trajectory.as_ref().map_or(0, |t| t.len()));
    println!("  Pre-filters: {:?}", pre_filters);

    let frame_size = (width * height * 4) as usize; // RGBA

    let expected_frames = ((duration_ms as f64 * decoder_fps as f64) / 1000.0).ceil() as u64;
    let max_frames = ((expected_frames as f64) * 1.20).max(expected_frames as f64 + 120.0).ceil() as u64;
    println!("  Expected frames: {}, max limit: {}", expected_frames, max_frames);

    // Build decoder command with hardware acceleration
    let mut decoder_args = vec![
        "-y".to_string(),
        #[cfg(target_os = "macos")]
        "-hwaccel".to_string(),
        #[cfg(target_os = "macos")]
        "videotoolbox".to_string(),
        "-threads".to_string(),
        "0".to_string(),
        "-i".to_string(),
        input_path.to_string_lossy().to_string(),
    ];

    if let Some(filters) = pre_filters {
        if filters.contains("[") {
            decoder_args.push("-filter_complex".to_string());
            let filter_with_output = if filters.ends_with("[out]") {
                filters.to_string()
            } else {
                format!("{}[out]", filters)
            };
            decoder_args.push(filter_with_output);
            decoder_args.push("-map".to_string());
            decoder_args.push("[out]".to_string());
        } else {
            decoder_args.push("-vf".to_string());
            decoder_args.push(filters.to_string());
        }
    }

    decoder_args.extend([
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

    let decoder_stderr = decoder.stderr.take();

    let mut encoder_args = vec![
        "-y".to_string(),
        "-f".to_string(), "rawvideo".to_string(),
        "-pix_fmt".to_string(), "rgba".to_string(),
        "-s".to_string(), format!("{}x{}", width, height),
        "-r".to_string(), format!("{}", decoder_fps),
        "-threads".to_string(), "0".to_string(),
        "-i".to_string(), "-".to_string(),
    ];

    if let Some(audio) = audio_path {
        encoder_args.extend([
            "-i".to_string(),
            audio.to_string_lossy().to_string(),
            "-map".to_string(), "0:v".to_string(),
            "-map".to_string(), "1:a".to_string(),
            "-c:a".to_string(), "aac".to_string(),
            "-b:a".to_string(), "192k".to_string(),
            "-ac".to_string(), "2".to_string(),
        ]);
    }

    encoder_args.extend(codec_args.iter().cloned());
    encoder_args.extend([
        "-vf".to_string(),
        format!("fps={},format=yuv420p", output_fps),
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

    let encoder_stderr = encoder.stderr.take();

    let decoder_stdout = decoder.stdout.take()
        .ok_or_else(|| anyhow!("Failed to get decoder stdout"))?;
    let encoder_stdin = encoder.stdin.take()
        .ok_or_else(|| anyhow!("Failed to get encoder stdin"))?;

    let mut reader = BufReader::with_capacity(frame_size * 4, decoder_stdout);
    let mut writer = BufWriter::with_capacity(frame_size * 4, encoder_stdin);

    let mut frame_buffer = vec![0u8; frame_size];
    // Reusable temp buffer for zoom transform (avoids per-frame allocation)
    let mut zoom_temp_buffer: Vec<u8> = Vec::new();
    let mut frame_idx: usize = 0;
    let start_time = std::time::Instant::now();

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

    // Process frames: read → zoom → cursor composite → write
    loop {
        match reader.read_exact(&mut frame_buffer) {
            Ok(()) => {}
            Err(e) if e.kind() == std::io::ErrorKind::UnexpectedEof => {
                println!("  Decoder finished at frame {}", frame_idx);
                break;
            }
            Err(e) => {
                return Err(anyhow!("Error reading frame {}: {}", frame_idx, e));
            }
        }

        // Apply zoom/pan transform using spring physics trajectory
        if let Some(ref trajectory) = zoom_trajectory {
            if !trajectory.is_empty() {
                let traj_idx = frame_idx.min(trajectory.len().saturating_sub(1));
                super::zoom_trajectory::apply_zoom_to_frame(
                    &mut frame_buffer,
                    &mut zoom_temp_buffer,
                    width,
                    height,
                    &trajectory[traj_idx],
                );
            }
        }

        // Composite cursor
        if !cursor_frames.is_empty() {
            let cursor_idx = frame_idx.min(cursor_frames.len().saturating_sub(1));
            composite_cursor_frame(&mut frame_buffer, &cursor_frames[cursor_idx], width, height);
        }

        writer.write_all(&frame_buffer)
            .map_err(|e| anyhow!("Error writing frame {}: {}", frame_idx, e))?;

        frame_idx += 1;

        if frame_idx as u64 > max_frames {
            println!("  WARNING: Frame count {} exceeded max limit {}, stopping", frame_idx, max_frames);
            let _ = decoder.kill();
            break;
        }

        if frame_idx % 30 == 0 {
            let actual_total = expected_frames.max(frame_idx as u64);
            let percentage = ((frame_idx as f64 / actual_total as f64) * 100.0).min(99.0);

            if let Some(cb) = progress_callback {
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
            println!("  Processed frame {}/{} ({:.1}%)", frame_idx, expected_frames,
                ((frame_idx as f64 / expected_frames as f64) * 100.0).min(100.0));
        }
    }

    drop(writer);

    if let Some(handle) = decoder_stderr_handle {
        match handle.join() {
            Ok(lines) => {
                let errors: Vec<_> = lines.iter()
                    .filter(|l| l.contains("error") || l.contains("Error") || l.contains("WARNING"))
                    .collect();
                if !errors.is_empty() {
                    println!("  Decoder stderr warnings/errors:");
                    for line in errors.iter().take(10) {
                        println!("    {}", line);
                    }
                }
            }
            Err(_) => println!("  WARNING: Decoder stderr thread panicked"),
        }
    }

    if let Some(handle) = encoder_stderr_handle {
        match handle.join() {
            Ok(lines) => {
                let errors: Vec<_> = lines.iter()
                    .filter(|l| l.contains("error") || l.contains("Error") || l.contains("WARNING"))
                    .collect();
                if !errors.is_empty() {
                    println!("  Encoder stderr warnings/errors:");
                    for line in errors.iter().take(10) {
                        println!("    {}", line);
                    }
                }
            }
            Err(_) => println!("  WARNING: Encoder stderr thread panicked"),
        }
    }

    println!("  Waiting for encoder to finish...");
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

    if let Some(cb) = progress_callback {
        cb(ProcessingProgress {
            current_frame: frame_idx as u64,
            total_frames: frame_idx as u64,
            percentage: 100.0,
            estimated_remaining_ms: Some(0),
        });
    }

    let elapsed = start_time.elapsed();
    let fps = frame_idx as f64 / elapsed.as_secs_f64();
    println!("=== RUST COMPOSITING COMPLETE: {} frames in {:.1}s ({:.1} fps) ===",
        frame_idx, elapsed.as_secs_f64(), fps);
    Ok(())
}

/// Alpha-composite a cursor frame onto a video frame buffer (in-place)
pub fn composite_cursor_frame(
    video_frame: &mut [u8],
    cursor_frame: &RgbaImage,
    width: u32,
    height: u32,
) {
    let expected_size = (width * height * 4) as usize;
    if video_frame.len() != expected_size {
        println!("Warning: frame size mismatch: {} vs {}", video_frame.len(), expected_size);
        return;
    }

    for (i, cursor_pixel) in cursor_frame.pixels().enumerate() {
        let alpha = cursor_pixel[3];
        if alpha == 0 {
            continue;
        }

        let idx = i * 4;
        if idx + 3 >= video_frame.len() {
            break;
        }

        if alpha == 255 {
            video_frame[idx] = cursor_pixel[0];
            video_frame[idx + 1] = cursor_pixel[1];
            video_frame[idx + 2] = cursor_pixel[2];
        } else {
            let a = alpha as f32 / 255.0;
            let inv_a = 1.0 - a;
            video_frame[idx] = blend_channel(video_frame[idx], cursor_pixel[0], a, inv_a);
            video_frame[idx + 1] = blend_channel(video_frame[idx + 1], cursor_pixel[1], a, inv_a);
            video_frame[idx + 2] = blend_channel(video_frame[idx + 2], cursor_pixel[2], a, inv_a);
        }
    }
}

#[inline]
fn blend_channel(dst: u8, src: u8, alpha: f32, inv_alpha: f32) -> u8 {
    ((src as f32 * alpha) + (dst as f32 * inv_alpha)) as u8
}
