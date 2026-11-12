//! GPU video composition with text overlays and a single encoder readback.
use anyhow::{Result, anyhow};
use std::io::{BufReader, BufWriter, Read as IoRead, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use super::audio_export::AudioTrackFiles;
use super::codec_config::{add_trim_settings, detect_export_acceleration};
use super::export_range::ExportRange;
use super::gpu_compositor::{GpuCompositor, build_gpu_config_with_webcam};
use super::types::{
    ExportSettings, ProcessingProgress, VideoInfo, ZoomBlock, load_zoom_blocks_from_sidecar,
    validate_export_zoom_blocks,
};
use super::visual_effects::{determine_output_path, get_cursor_config, get_webcam_info};
use super::zoom_trajectory::{CAMERA_SIMULATION_FPS, ZoomFrameState, simulate_zoom_trajectory_in_viewport, CameraViewport};
use crate::cursor_renderer::{parse_hex_rgb, simulate_cursor_frames_from};
pub(crate) mod encoder;
mod support;
pub(crate) mod resources;
use resources::{ExportChild, ExportOutput};
pub(crate) mod timeline_support;
use super::timeline_decode::{video_decode_args,video_decode_args_with_crop};
use super::source_crop::remap_motion;
pub(crate) use support::{parse_filtered_cursor_events,get_cursor_spring_config,load_camera_tracks,load_camera_activity};
pub use support::{load_raw_cursor_events, load_camera_events, load_filtered_cursor_events};
mod camera_samples;
pub(crate) use camera_samples::remove_shake as filter_pointer_shakes;
mod loupe_support;
pub(crate) use support::{get_zoom_spring_config, resolve_mouse_events_path, load_rotation_points};
use timeline_support::{PreparedAudio, resolve_plan};
/// Apply video effects and export using GPU-accelerated compositing.
pub async fn export_video(
    _temp_dir: &Path,
    input_path: &Path,
    mut settings: ExportSettings,
    video_info: &VideoInfo,
    progress_callback: Option<Box<dyn Fn(ProcessingProgress) + Send + Sync>>,
) -> Result<PathBuf> {
    let (shortcut_overlay,caption_overlay)=(settings.shortcut_overlay.take(),settings.caption_overlay.take());
    let control = settings.cancellation.clone().unwrap_or_default();
    control.check()?;
    if !input_path.exists() {
        return Err(anyhow!("Input video file not found: {:?}", input_path));
    }

    let camera_settings=settings.clone();
    let crop = settings.source_crop.map(|rect| rect.geometry(video_info.width, video_info.height)).transpose()?;
    if let Some(crop) = crop {
        let rect = crop.normalized();
        settings.source_crop = Some(rect);
        crop.apply_window_dimensions(&mut settings);
    }
    let background_start_ms = settings.trim_start.unwrap_or(0) as f64;
    let audio_timeline = resolve_plan(&settings, input_path, video_info.duration_ms)?;
    let cursor_timeline=timeline_support::cursor_plan(&settings,audio_timeline.as_ref())?;
    let timeline=timeline_support::video_plan(&settings,audio_timeline.as_ref(),video_info.duration_ms);
    let range = if let Some(plan) = &timeline {
        ExportRange {
            start_ms: 0,
            end_ms: plan.duration_ms.ceil() as u64,
        }
    } else {
        ExportRange::resolve(&settings, video_info.duration_ms)?
    };
    settings.trim_start = Some(range.start_ms);
    settings.trim_end = Some(range.end_ms);
    if !matches!(settings.frame_rate.unwrap_or(60), 24 | 30 | 60 | 120) {
        return Err(anyhow!("Unsupported export frame rate"));
    }
    // GIF's centisecond delays below 20ms are slowed by common viewers.
    if settings.format.as_deref() == Some("gif") {
        settings.frame_rate = Some(settings.frame_rate.unwrap_or(60).min(30));
    }

    println!(
        "[Export] project_title from settings: {:?}",
        settings.project_title
    );

    let final_output_path = determine_output_path(&settings, input_path)?;
    println!("[Export] Output path will be: {:?}", final_output_path);

    let output = ExportOutput::new(&final_output_path)?;
    let output_path = output.path().to_owned();

    let audio_tracks = if settings.format.as_deref() == Some("gif") {
        AudioTrackFiles {
            background: None,
            mic: None,
            system: None,
        }
    } else {
        AudioTrackFiles::discover(input_path)
    };
    if audio_tracks.has_any() {
        println!("Found audio files for export: {:?}", audio_tracks);
    }

    let prepared_audio = audio_timeline
        .as_ref()
        .filter(|_| audio_tracks.has_any())
        .map(|plan| PreparedAudio::with_control(_temp_dir, plan, &audio_tracks, &control))
        .transpose()?;
    let mut audio_tracks = prepared_audio
        .as_ref()
        .map(|p| &p.tracks)
        .unwrap_or(&audio_tracks).clone();
    let background_audio = if settings.format.as_deref() != Some("gif") {
        settings.background_audio.as_ref().map(|audio| {
            super::background_audio::PreparedBackgroundAudio::new(_temp_dir,Path::new(&audio.path),
                background_start_ms,range.duration_ms() as f64,
                if audio.muted.unwrap_or(false) {0.0} else {audio.volume.unwrap_or(1.0)},&control)
        }).transpose()?
    } else { None };
    audio_tracks.background = background_audio.as_ref().map(|audio| audio.path.clone());
    let audio_tracks = &audio_tracks;
    let mut audio_input_settings = settings.clone();
    if prepared_audio.is_some() {
        audio_input_settings.trim_start = None;
        audio_input_settings.trim_end = None;
    }

    let webcam_info = if settings.webcam_enabled == Some(false) { None } else { get_webcam_info(input_path) };
    let target_fps = settings.frame_rate.unwrap_or(60);
    let output_duration_ms=timeline.as_ref().map_or(range.duration_ms() as f64,|plan|plan.duration_ms);
    let expected_frames=super::export_range::output_frame_count(output_duration_ms,target_fps);
    let acceleration = detect_export_acceleration(&settings);
    control.check()?;
    println!("[Export] Video acceleration: {}", acceleration.label());
    let (source_width, source_height) = (video_info.width, video_info.height);
    let duration_ms = video_info.duration_ms;
    let is_window_mode = settings.capture_mode.as_deref() == Some("window");
    let mouse_events_path = resolve_mouse_events_path(input_path);
    let camera_activity=if settings.zoom_blocks.as_ref().is_none_or(|blocks|!blocks.is_empty()) || webcam_info.is_some() {
        load_camera_activity(&mouse_events_path,input_path,(source_width,source_height),&camera_settings,duration_ms)?
    } else {super::camera_activity::Tracks::default()};
    let cursor_enabled = settings
        .cursor_settings
        .as_ref()
        .and_then(|c| c.enabled)
        .unwrap_or(true);

    let mut loupe_trajectory = None;
    let mut source_zoom_blocks = Vec::new();
    // Load and validate zoom blocks
    let zoom_blocks_to_use: Option<Vec<ZoomBlock>> = {
        // An explicit empty list means the user removed every zoom. Only an
        // absent list may fall back to the recording's generated sidecar.
        let mut blocks = settings
            .zoom_blocks
            .clone()
            .or_else(|| load_zoom_blocks_from_sidecar(input_path));
        if let Some(ref mut zb) = blocks {
            validate_export_zoom_blocks(zb, duration_ms);
            source_zoom_blocks = zb.clone();
            loupe_trajectory = loupe_support::prepare(zb, &camera_settings, &camera_activity,
                (source_width,source_height), duration_ms)?;
            remap_motion(crop, &mut [], zb);
        }
        blocks
    };

    let zoom_trajectory: Option<Vec<ZoomFrameState>> =
        if let Some(ref zoom_blocks) = zoom_blocks_to_use {
            if !zoom_blocks.is_empty() {
                let zoom_spring_config = get_zoom_spring_config(&settings);

                println!(
                    "Simulating zoom trajectory with spring physics ({} blocks, {} cursor events)",
                    zoom_blocks.len(),
                    camera_activity.grouped.len()
                );

                let trajectory = simulate_zoom_trajectory_in_viewport(
                    zoom_blocks,
                    &[],
                    &zoom_spring_config,
                    settings
                        .screen_motion_style
                        .as_deref()
                        .is_some_and(|style| matches!(style, "focused" | "smooth")),
                    settings.animation_speed.as_deref().unwrap_or("mellow"),
                    CAMERA_SIMULATION_FPS,
                    if timeline.is_some() {
                        duration_ms
                    } else {
                        range.end_ms
                    },
                    is_window_mode,
                    CameraViewport::from_settings(&settings, (crop.map(|c| c.width).unwrap_or(source_width as f64),
                        crop.map(|c| c.height).unwrap_or(source_height as f64))),
                    Some(super::zoom_trajectory::targets::CameraActivity {events:&[],prepared:Some(&camera_activity),smooth:get_cursor_spring_config(&settings).mass>0.0}),
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
    let cursor_trajectory = if cursor_enabled && mouse_events_path.exists() && duration_ms > 0 {
        let activity = crate::cursor_renderer::typing_activity::TypingActivity::load(&mouse_events_path);
        let typing = |time| activity.active(time);
        let mut raw_events = load_filtered_cursor_events(&mouse_events_path, source_width, source_height, cursor_config.shake_threshold.unwrap_or(0.0));
        remap_motion(crop, &mut raw_events, &mut []);
        let cursor_spring = get_cursor_spring_config(&settings);

        // Window mode: don't pre-transform cursor through zoom — the shader's inverse-zoom
        // remap handles this at the composite level. Display mode: pre-transform as before.
        let cursor_zoom_ref = if is_window_mode {
            &None
        } else {
            &zoom_trajectory
        };
        let planned=crate::cursor_renderer::planner::runtime::prepare(&mouse_events_path,(source_width,source_height),
            settings.source_crop,cursor_timeline.as_ref(),input_path,duration_ms as f64,&cursor_config,cursor_spring,Some(&|| control.check()))?;
        let rotation_points = if cursor_config.rotate_while_moving.unwrap_or(false)
            && planned.as_ref().and_then(|plan| plan.rotation_keys()).is_none() {
            load_rotation_points(&mouse_events_path,cursor_config.shake_threshold.unwrap_or(0.0))
        } else {Vec::new()};
        let position=|time|planned.as_ref().unwrap().at(time);
        let pressed=|time|planned.as_ref().unwrap().pressed_at_source(time,cursor_spring.mass!=0.0);
        let positions = if let Some(plan)=cursor_timeline.as_ref() {
            crate::cursor_renderer::edited_simulation::simulate_with_planned_path(&raw_events,&rotation_points,plan,
                &input_path.to_string_lossy(),&cursor_spring,&cursor_config,cursor_zoom_ref,target_fps as f64,Some(&activity),planned.as_ref(),target_fps as f64)
        } else {
            let rotation_keys = crate::cursor_renderer::rotation::prepared_or_retimed_targets(
                planned.as_ref(), &cursor_config, duration_ms, || rotation_points);
            simulate_cursor_frames_from(&raw_events,&cursor_spring,target_fps as f64,range.end_ms,cursor_zoom_ref,
                &cursor_config,range.start_ms,CAMERA_SIMULATION_FPS,Some(&rotation_keys),Some(&typing),
                planned.as_ref().map(|_|&position as &dyn Fn(f64)->[f64;2]),planned.as_ref().map(|_|(&pressed as &dyn Fn(f64)->bool,target_fps as f64)),Clone::clone)
        };
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
    let cursor_shape_ref: Option<&image::RgbaImage> = if cursor_enabled {
        Some(&placeholder_shape)
    } else {
        None
    };

    let src_w = settings.source_width.or(Some(source_width));
    let src_h = settings.source_height.or(Some(source_height));

    let mut gpu_config = build_gpu_config_with_webcam(&settings, &webcam_info, src_w, src_h);
    gpu_config.source_size = [source_width, source_height];
    if let Some(crop) = crop {
        gpu_config.apply_source_geometry(&settings,(crop.width,crop.height));
    }
    let sampling=settings.source_crop.map(|crop| gpu_config.crop_sampling(crop,[source_width,source_height],
        zoom_trajectory.as_deref().unwrap_or(&[]),settings.mask_overlay.is_some())).transpose()?;
    let decode_crop=sampling.map(|plan|plan.decode);
    if let Some(plan)=sampling {
        gpu_config.source_size=[plan.decode.width,plan.decode.height];
        gpu_config.source_uv=plan.view.map(|value|value as f32);
    }
    let webcam_timeline = super::webcam_timeline::WebcamTimeline::new(&settings, gpu_config.webcam.clone())?;
    let camera_opacity = if webcam_info.is_some() {
        Some(super::loupe::camera_opacity_prepare::prepare_camera_opacity(
            &source_zoom_blocks, &[], (source_width,source_height),
            if timeline.is_some() {duration_ms} else {range.end_ms},
            get_zoom_spring_config(&camera_settings), &camera_settings, &gpu_config,
            zoom_trajectory.as_deref().unwrap_or(&[]),
         Some(&camera_activity))?)
    } else {None};
    let (out_width, out_height) = (gpu_config.output_width, gpu_config.output_height);
    let shortcuts=super::shortcut_overlay::Prepared::new(shortcut_overlay.as_ref(),out_width,out_height,expected_frames)?;
    let captions=caption_overlay.map(|input|super::caption_overlay::Prepared::new(input,out_width,out_height,expected_frames)).transpose()?;
    let [input_width, input_height] = gpu_config.source_size;
    println!("=== GPU EXPORT START ===");
    println!("  Input (decode): {}x{}", input_width, input_height);
    println!("  Output: {}x{} @ {}fps", out_width, out_height, target_fps);
    println!(
        "  Zoom: {} frames",
        zoom_trajectory.as_ref().map_or(0, |t| t.len())
    );
    println!(
        "  Cursor: {} positions",
        cursor_trajectory.as_ref().map_or(0, |t| t.len())
    );
    println!("  Motion blur: {}", gpu_config.motion_blur_enabled);
    println!("  Shadow: {}", gpu_config.shadow_enabled);
    println!("  Corner radius: {}", gpu_config.corner_radius);

    // Initialize GPU compositor (SDF cursor — pass cursor config for style/color)
    let cursor_settings_ref = if cursor_enabled {
        Some(&cursor_config)
    } else {
        None
    };
    control.check()?;
    let source_view = gpu_config.source_uv;
    let mut compositor = GpuCompositor::new(
        gpu_config,
        cursor_shape_ref,
        cursor_size,
        ripple_color,
        cursor_settings_ref,
    )?;
    compositor.configure_recorded_cursor(&mouse_events_path, &cursor_config)?;
    let gpu_shortcuts=compositor.configure_shortcuts(shortcuts.as_ref())?;
    compositor.configure_captions(captions)?;
    let masks=settings.mask_overlay.take().map(|m|super::mask_export::Prepared::new(m,expected_frames,[input_width,input_height],source_view)).transpose()?;
    // Spawn webcam decoder if webcam recording exists
    let webcam_frame_size: Option<(u32, u32)>;
    let mut webcam_decoder: Option<ExportChild> = None;
    if let Some((ref webcam_path, _, _, _, _)) = webcam_info {
        if webcam_path.exists() {
            // Get webcam dimensions via ffprobe
            let probe_output = control.output(Command::new(crate::ffmpeg::binary("ffprobe")).args([
                    "-v",
                    "quiet",
                    "-select_streams",
                    "v:0",
                    "-show_entries",
                    "stream=width,height",
                    "-of",
                    "csv=p=0",
                    &webcam_path.to_string_lossy(),
                ]));
            control.check()?;
            let (wc_w, wc_h) = probe_output
                .ok()
                .and_then(|o| String::from_utf8(o.stdout).ok())
                .and_then(|s| {
                    let parts: Vec<&str> = s.trim().split(',').collect();
                    if parts.len() == 2 {
                        Some((parts[0].parse::<u32>().ok()?, parts[1].parse::<u32>().ok()?))
                    } else {
                        None
                    }
                })
                .unwrap_or((1920, 1080)); // fallback

            if wc_w > 0 && wc_h > 0 {
                webcam_frame_size = Some((wc_w, wc_h));

                // Initialize webcam texture on GPU
                compositor.set_webcam_texture(wc_w, wc_h);

                // Spawn FFmpeg to decode webcam to raw RGBA, same fps as main video
                let mut wc_args = vec!["-y".to_string()];
                add_trim_settings(&mut wc_args, &settings);
                wc_args.extend(["-i".to_string(), webcam_path.to_string_lossy().into_owned()]);
                wc_args.extend([
                    "-vf".to_string(),
                    format!("setpts=PTS-STARTPTS,fps={},tpad=stop_mode=clone:stop=1,trim=end_frame={},scale={}:{}", target_fps, expected_frames, wc_w, wc_h),
                    "-f".to_string(),
                    "rawvideo".to_string(),
                    "-pix_fmt".to_string(),
                    "rgba".to_string(),
                    "-".to_string(),
                ]);
                if let Some(plan) = &timeline {
                    let mut linked_plan = plan.clone();
                    for segment in &mut linked_plan.segments {
                        segment.source_file_path = None;
                    }
                    wc_args = video_decode_args(&linked_plan, &webcam_path, wc_w, wc_h, target_fps);
                }
                let wc_decoder = Command::new(crate::ffmpeg::binary("ffmpeg"))
                    .args(&wc_args)
                    .stdout(Stdio::piped())
                    .stderr(Stdio::null())
                    .spawn();

                match wc_decoder {
                    Ok(child) => {
                        webcam_decoder = Some(ExportChild::new(child, &control));
                        println!(
                            "  Webcam decoder: {}x{} from {}",
                            wc_w,
                            wc_h,
                            webcam_path.display()
                        );
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
    let mut webcam_frame_buffer: Option<Vec<u8>> =
        webcam_frame_size.map(|(w, h)| vec![0u8; (w * h * 4) as usize]);
    let mut webcam_reader: Option<BufReader<std::process::ChildStdout>> = webcam_decoder
        .as_mut()
        .and_then(|d| d.stdout())
        .map(BufReader::new);

    // Spawn FFmpeg decoder (raw RGBA output)
    let mut decoder_args = vec!["-y".to_string()];
    acceleration.append_decoder_args(&mut decoder_args);
    // Seek before decoding; reset timestamps before frame-rate conversion.
    add_trim_settings(&mut decoder_args, &settings);
    decoder_args.extend([
        "-threads".to_string(),
        "0".to_string(),
        "-i".to_string(),
        input_path.to_string_lossy().to_string(),
    ]);


    decoder_args.extend([
        "-vf".to_string(),
        format!("{}setpts=PTS-STARTPTS,fps={},tpad=stop_mode=clone:stop=1,trim=end_frame={},scale={}:{}", decode_crop.map_or(String::new(),|crop|crop.decode_filter()), target_fps, expected_frames, input_width, input_height),
        "-f".to_string(),
        "rawvideo".to_string(),
        "-pix_fmt".to_string(),
        "rgba".to_string(),
        "-".to_string(),
    ]);

    if let Some(plan) = &timeline {
        decoder_args = video_decode_args_with_crop(plan, input_path, input_width, input_height, target_fps,decode_crop);
    }
    println!("  Decoder args: {:?}", decoder_args);

    control.check()?;
    let decoder = Command::new(crate::ffmpeg::binary("ffmpeg"))
        .args(&decoder_args)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map(|child| ExportChild::new(child, &control))
        .map_err(|e| anyhow!("Failed to spawn decoder: {}", e))?;

    let encoder_args = encoder::encoder_args(
        &settings,
        &acceleration,
        audio_tracks,
        &audio_input_settings,
        (out_width, out_height),
        target_fps,
        &output_path,
    );

    println!("  Encoder args: {:?}", encoder_args);

    let encoder = Command::new(crate::ffmpeg::binary("ffmpeg"))
        .args(&encoder_args)
        .stdin(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map(|child| ExportChild::new(child, &control))
        .map_err(|e| anyhow!("Failed to spawn encoder: {}", e))?;

    let decoder_stderr = decoder.stderr();
    let encoder_stderr = encoder.stderr();
    let decoder_stdout = decoder
        .stdout()
        .ok_or_else(|| anyhow!("Failed to get decoder stdout"))?;
    let encoder_stdin = encoder
        .stdin()
        .ok_or_else(|| anyhow!("Failed to get encoder stdin"))?;

    let input_frame_size = (input_width * input_height * 4) as usize;
    let output_frame_size = (out_width * out_height * 4) as usize;
    let mut reader = decoder_stdout;
    let mut writer = BufWriter::with_capacity(output_frame_size * 4, encoder_stdin);

    let mut frame_buffer = vec![0u8; input_frame_size];
    let mut frame_idx: usize = 0;
    let start_time = std::time::Instant::now();

    // Default zoom state (no zoom)
    let default_zoom = ZoomFrameState { rect: None,
        scale: 1.0,
        center_x: 0.5,
        center_y: 0.5,
    };

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

    let mut previous_segment = None;
    // Main frame processing loop: decode → GPU composite → encode
    loop {
        control.check()?;
        match reader.read_exact(&mut frame_buffer) {
            Ok(()) => {}
            Err(e) if e.kind() == std::io::ErrorKind::UnexpectedEof => {
                println!("  Decoder finished at frame {}", frame_idx);
                break;
            }
            Err(e) => return Err(anyhow!("Error reading frame {}: {}", frame_idx, e)),
        }

        if frame_idx as u64 >= expected_frames {
            return Err(anyhow!("Decoder emitted frames beyond the requested export range"));
        }
        let mapped = timeline
            .as_ref()
            .and_then(|plan| plan.source_at(frame_idx as f64 * 1000.0 / target_fps as f64));
        let gap = timeline.is_some() && mapped.is_none();
        compositor.set_recorded_cursor_time(mapped.map(|(_,ms)|ms).or_else(|| timeline.is_none().then_some(range.start_ms as f64 + frame_idx as f64 * 1000.0 / target_fps as f64)));
        let source_frame_idx = if timeline.is_some() {
            mapped.map_or(0, |(_, ms)| {
                (ms * CAMERA_SIMULATION_FPS / 1000.0 + 1e-7).floor() as usize
            })
        } else {
            range.source_sample(frame_idx, target_fps, CAMERA_SIMULATION_FPS)
        };
        // Motion remains in source time even when the decoded video is trimmed.
        let source_time_ms = mapped.map_or(
            range.start_ms as f64 + frame_idx as f64 * 1000.0 / target_fps as f64,
            |(_, ms)| ms,
        );
        let sampled_zoom = zoom_trajectory.as_ref().filter(|_| !gap)
            .and_then(|frames| super::zoom_trajectory::sample_camera_frame(frames, source_time_ms));
        let zoom_state = sampled_zoom.as_ref().unwrap_or(&default_zoom);

        // Get cursor state for this frame
        let cursor_state = cursor_trajectory.as_ref().filter(|_| !gap && !mapped.is_some_and(|(segment, _)| segment.hide_cursor)).and_then(|t| {
            if t.is_empty() {
                None
            } else {
                Some(
                    &t[frame_idx.min(t.len() - 1)],
                )
            }
        });

        // Upload webcam frame to GPU (if available)
        if let (Some(wc_reader), Some(wc_buf), Some((wc_w, wc_h))) = (
            &mut webcam_reader,
            &mut webcam_frame_buffer,
            webcam_frame_size,
        ) {
            match wc_reader.read_exact(wc_buf) {
                Ok(()) => compositor.upload_webcam_frame(wc_buf, wc_w, wc_h),
                Err(_) => {} // Webcam video ended — last frame stays on GPU
            }
        }

        let segment_start = mapped.map(|(segment, _)| segment.output_start_ms);
        if timeline.is_some() && segment_start != previous_segment {
            compositor.reset_motion_history();
        }
        previous_segment = segment_start;
        let source_ms = mapped.map_or(
            range.start_ms as f64 + frame_idx as f64 * 1000.0 / target_fps as f64,
            |(_, ms)| ms,
        );
        compositor.set_webcam_layout(webcam_timeline.sample_target(source_ms).map(|(config,_)|config));
        if let Some(frames)=&camera_opacity {
            compositor.set_camera_frame(frames.sample(source_ms))?;
        }
        let lens = loupe_trajectory.as_ref().filter(|_| !gap)
            .and_then(|frames| frames.get(source_frame_idx)).copied();
        compositor.set_loupe_target(lens,(source_width,source_height),settings.source_crop,zoom_state)?;
        compositor.set_caption_frame(frame_idx as u64); compositor.set_source_mask(masks.as_ref().and_then(|m|m.sample(frame_idx as u64)))?;
        if gpu_shortcuts {compositor.set_shortcut_frame(shortcuts.as_ref().unwrap(),frame_idx as u64); }
        let mut composited =
            compositor.composite_timeline_frame(&frame_buffer, zoom_state, cursor_state, !gap)?;

        if !gpu_shortcuts {if let Some(shortcuts)=&shortcuts {shortcuts.composite(frame_idx as u64,&mut composited)?;}}
        writer
            .write_all(&composited)
            .map_err(|e| anyhow!("Error writing frame {}: {}", frame_idx, e))?;

        frame_idx += 1;

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
            println!(
                "  GPU frame {}/{} ({:.1}%)",
                frame_idx,
                expected_frames,
                ((frame_idx as f64 / expected_frames as f64) * 100.0).min(100.0)
            );
        }
    }

    writer.flush().map_err(|e| anyhow!("Failed to finish writing export frames: {}", e))?;
    drop(writer);
    drop(webcam_reader);
    if let Some(wc) = webcam_decoder {
        let _ = wc.kill();
        let _ = wc.wait();
    }

    // Collect stderr
    if let Some(handle) = decoder_stderr_handle {
        if let Ok(lines) = handle.join() {
            let errors: Vec<_> = lines
                .iter()
                .filter(|l| l.contains("error") || l.contains("Error"))
                .collect();
            for line in errors.iter().take(5) {
                println!("  Decoder: {}", line);
            }
        }
    }
    if let Some(handle) = encoder_stderr_handle {
        if let Ok(lines) = handle.join() {
            let errors: Vec<_> = lines
                .iter()
                .filter(|l| l.contains("error") || l.contains("Error"))
                .collect();
            for line in errors.iter().take(5) {
                println!("  Encoder: {}", line);
            }
        }
    }

    let encoder_status = encoder
        .wait()
        .map_err(|e| anyhow!("Encoder wait failed: {}", e))?;
    if !encoder_status.success() {
        return Err(anyhow!("Encoder failed with status: {}", encoder_status));
    }

    let decoder_status = decoder
        .wait()
        .map_err(|e| anyhow!("Decoder wait failed: {}", e))?;
    if !decoder_status.success() {
        return Err(anyhow!("Decoder failed with status: {}", decoder_status));
    }

    if frame_idx as u64 != expected_frames {
        return Err(anyhow!("Export frame count mismatch: expected {}, decoded {}",expected_frames,frame_idx));
    }
    let final_output_path = control.commit(|| output.commit())?;

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
    println!(
        "=== GPU EXPORT COMPLETE: {} frames in {:.1}s ({:.1} fps) ===",
        frame_idx,
        elapsed.as_secs_f64(),
        fps
    );

    Ok(final_output_path)
}
