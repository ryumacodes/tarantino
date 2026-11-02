//! FFmpeg timeline and GPU export integration checks.
mod camera_silence;
mod camera_reference;
use super::{
    audio_export::{AudioTrackFiles, append_audio_inputs},
    codec_config::add_trim_settings,
    types::ExportSettings,
};
use std::{path::Path, process::Command};

fn run(args: &[String]) -> Vec<u8> {
    let output = Command::new("ffmpeg").args(args).output().unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    output.stdout
}
fn strings(args: &[&str]) -> Vec<String> {
    args.iter().map(|s| s.to_string()).collect()
}

#[test]
#[ignore = "requires local FFmpeg; synthetic video/audio trim integration"]
fn trimmed_video_and_audio_start_at_the_same_source_time() {
    let root = std::env::temp_dir().join(format!("trim-check-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&root).unwrap();
    let result = std::panic::catch_unwind(|| verify(&root));
    std::fs::remove_dir_all(root).unwrap();
    if let Err(error) = result {
        std::panic::resume_unwind(error);
    }
}
fn verify(root: &Path) {
    let video = root.join("colors.mkv");
    let audio = root.join("tones.wav");
    let mut args = strings(&[
        "-v",
        "error",
        "-f",
        "lavfi",
        "-i",
        "color=c=red:s=16x16:r=30:d=1",
        "-f",
        "lavfi",
        "-i",
        "color=c=blue:s=16x16:r=30:d=1",
        "-filter_complex",
        "[0:v][1:v]concat=n=2:v=1:a=0",
        "-c:v",
        "ffv1",
    ]);
    args.push(video.to_string_lossy().into_owned());
    run(&args);
    let mut args = strings(&[
        "-v",
        "error",
        "-f",
        "lavfi",
        "-i",
        "aevalsrc=if(lt(t\\,1)\\,sin(2*PI*220*t)\\,sin(2*PI*880*t)):s=48000:d=2",
        "-c:a",
        "pcm_s16le",
    ]);
    args.push(audio.to_string_lossy().into_owned());
    run(&args);
    let settings: ExportSettings =
        serde_json::from_value(serde_json::json!({"trim_start":1000,"trim_end":1500})).unwrap();
    let mut args = strings(&["-v", "error", "-i"]);
    args.push(video.to_string_lossy().into_owned());
    add_trim_settings(&mut args, &settings);
    args.extend(strings(&[
        "-vf",
        "fps=30,scale=16:16",
        "-f",
        "rawvideo",
        "-pix_fmt",
        "rgba",
        "-",
    ]));
    let frames = run(&args);
    assert_eq!(frames.len(), 15 * 16 * 16 * 4);
    for pixel in frames.chunks_exact(4) {
        assert!(pixel[2] > 200 && pixel[0] < 30);
    }
    for microphone in [true, false] {
        let tracks = AudioTrackFiles {
            background: None,
            mic: microphone.then(|| audio.clone()),
            system: (!microphone).then(|| audio.clone()),
        };
        let mut args = strings(&["-v", "error"]);
        append_audio_inputs(&mut args, &tracks, &settings);
        args.extend(strings(&["-f", "f32le", "-ac", "1", "-ar", "48000", "-"]));
        let output = run(&args);
        let samples: Vec<f32> = output
            .chunks_exact(4)
            .map(|b| f32::from_le_bytes(b.try_into().unwrap()))
            .collect();
        assert_eq!(samples.len(), 24_000);
        let rising = samples
            .windows(2)
            .filter(|w| w[0] <= 0.0 && w[1] > 0.0)
            .count();
        assert!(
            (435..=445).contains(&rising),
            "Expected 880 Hz segment, got {rising} crossings"
        );
    }
}

#[test]
#[ignore = "requires local FFmpeg; timeline reorder/speed integration"]
fn timeline_decoder_preserves_order_speed_gaps_and_audio_pitch() {
    use super::timeline::{EditClip, EditTimeline};
    use super::timeline_decode::{audio_decode_args, video_decode_args};
    let root = std::env::temp_dir().join(format!("edits-check-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&root).unwrap();
    let result = std::panic::catch_unwind(|| {
        // Reuse the known red/blue and 220/880 Hz fixture creator and trim check.
        verify(&root);
        let clip = |id: &str, start, source, duration, rate| EditClip {
            id: id.into(),
            start_time: start,
            duration,
            source_in: source,
            source_out: source + duration * rate,
            playback_rate: rate,
            source_file_path: None,
            track_order: 0,
            mic_volume: 1.0, system_volume: 1.0, muted: false,
            hide_cursor: false, disable_cursor_smoothing: false,
        };
        let plan = EditTimeline::build(
            &[
                clip("fast", 0.0, 1000.0, 500.0, 2.0),
                clip("slow", 750.0, 0.0, 1000.0, 0.5),
            ],
            0.0,
            1750.0,
        )
        .unwrap();
        let frames = run(&video_decode_args(
            &plan,
            &root.join("colors.mkv"),
            16,
            16,
            60,
        ));
        assert_eq!(frames.len(), 105 * 16 * 16 * 4);
        for (index, frame) in frames.chunks_exact(16 * 16 * 4).enumerate() {
            let pixel = &frame[..4];
            if index < 30 {
                assert!(pixel[2] > 200 && pixel[0] < 30);
            } else if index < 45 {
                assert!(pixel[0] < 10 && pixel[2] < 10);
            } else {
                assert!(pixel[0] > 200 && pixel[2] < 30);
            }
        }
        // Fractional frame clip boundaries must not accumulate one rounded frame per clip.
        let short = EditTimeline::build(
            &[
                clip("first", 0.0, 0.0, 100.0, 1.0),
                clip("second", 100.0, 1000.0, 100.0, 1.0),
            ],
            0.0,
            200.0,
        )
        .unwrap();
        let short_frames = run(&video_decode_args(
            &short,
            &root.join("colors.mkv"),
            16,
            16,
            24,
        ));
        assert_eq!(short_frames.len(), 5 * 16 * 16 * 4);
        for (i, frame) in short_frames.chunks_exact(16 * 16 * 4).enumerate() {
            if i < 3 {
                assert!(frame[0] > 200);
            } else {
                assert!(frame[2] > 200);
            }
        }
        let bytes = run(&audio_decode_args(&plan, &root.join("tones.wav")));
        let samples: Vec<f32> = bytes
            .chunks_exact(4)
            .map(|b| f32::from_le_bytes(b.try_into().unwrap()))
            .collect();
        assert_eq!(samples.len(), 84_000);
        assert!(
            samples[24_000..36_000].iter().all(|s| s.abs() < 0.0001),
            "gap nonzero {:?}",
            samples[24_000..36_000]
                .iter()
                .enumerate()
                .filter(|(_, s)| s.abs() >= 0.0001)
                .map(|(i, s)| (i, *s))
                .take(20)
                .collect::<Vec<_>>()
        );
        let crossings = |samples: &[f32]| {
            samples
                .windows(2)
                .filter(|w| w[0] <= 0.0 && w[1] > 0.0)
                .count()
        };
        // Exercise the same stereo file preparation and cleanup used by normal export.
        let linked = AudioTrackFiles {
            background: None,
            mic: Some(root.join("tones.wav")),
            system: Some(root.join("tones.wav")),
        };
        let prepared =
            super::export::timeline_support::PreparedAudio::new(&root, &plan, &linked).unwrap();
        let paths = [
            prepared.tracks.mic.clone().unwrap(),
            prepared.tracks.system.clone().unwrap(),
        ];
        for path in &paths {
            let decoded = run(&strings(&[
                "-v",
                "error",
                "-i",
                path.to_str().unwrap(),
                "-f",
                "f32le",
                "-ac",
                "2",
                "-",
            ]));
            assert_eq!(decoded.len(), 84_000 * 2 * 4);
            let stereo: Vec<f32> = decoded
                .chunks_exact(4)
                .map(|b| f32::from_le_bytes(b.try_into().unwrap()))
                .collect();
            assert!(stereo[48_000..72_000].iter().all(|v| v.abs() < 0.0001));
            assert!(
                (170..=182).contains(&crossings(
                    &stereo[9600..28800]
                        .iter()
                        .step_by(2)
                        .copied()
                        .collect::<Vec<_>>()
                ))
            );
        }
        drop(prepared);
        assert!(paths.iter().all(|p| !p.exists()));
        // Interior windows avoid atempo boundary padding. Pitch remains unchanged.
        assert!((170..=182).contains(&crossings(&samples[4800..14400])));
        assert!((40..=48).contains(&crossings(&samples[48000..57600])));
    });
    std::fs::remove_dir_all(root).unwrap();
    if let Err(error) = result {
        std::panic::resume_unwind(error);
    }
}

#[tokio::test]
#[ignore = "requires local FFmpeg and GPU; edited export integration"]
async fn edited_export_renders_reordered_media_and_linked_audio() {
    verify_edited_export("mp4", false, false).await;
}

#[tokio::test]
#[ignore = "requires local FFmpeg and GPU; animated GIF export integration"]
async fn gif_export_preserves_edited_frames_and_omits_audio() {
    verify_edited_export("gif", false, false).await;
}

#[tokio::test]
#[ignore = "requires local FFmpeg and GPU; source-time camera layout integration"]
async fn edited_camera_layouts_follow_source_time() {
    verify_edited_export("mp4", true, false).await;
}

#[tokio::test]
#[ignore = "requires local FFmpeg and GPU; cancellation while encoding"]
async fn cancelled_export_preserves_destination_and_cleans_staging() {
    verify_edited_export("mp4", false, true).await;
}

async fn verify_edited_export(format: &str, camera_changes: bool, cancel: bool) {
    use super::{export::export_video, types::VideoInfo};
    let root = std::env::temp_dir().join(format!("edited-export-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&root).unwrap();
    verify(&root);
    let input = root.join("colors.mkv");
    std::fs::write(input.with_extension("mouse.json"),
        r#"[{"timestamp":0,"x":1,"y":1},{"timestamp":1000,"x":4,"y":1},{"timestamp":2000,"x":1,"y":1}]"#).unwrap();
    if format == "gif" {
        // GIF must not read or prepare unused audio, even if a sidecar is invalid.
        std::fs::write(input.with_extension("mic.wav"), b"unused invalid audio").unwrap();
    } else {
        std::fs::copy(root.join("tones.wav"), input.with_extension("mic.wav")).unwrap();
    }
    run(&strings(&[
        "-v",
        "error",
        "-i",
        input.to_str().unwrap(),
        "-vf",
        "format=rgb24,negate,format=yuv420p",
        "-c:v",
        "libx264",
        input.with_extension("webcam.mp4").to_str().unwrap(),
    ]));
    let output = root.join(format!("edited.{format}"));
    let mut settings: ExportSettings = serde_json::from_value(serde_json::json!({
        "output_path": output, "resolution": {"width":64,"height":64},
        "source_width":16,"source_height":16,"frame_rate":60,"format":format,"codec":"h264",
        "cursor_settings":{"enabled":true,"size":0.3,"trail_enabled":true,"trail_length":15}, "zoom_blocks":[],
        "webcam_x":0.75,"webcam_y":0.75,"webcam_size":0.25,
        "visual_settings":{"padding":0,"corner_radius":0,"shadow_enabled":false},
        "timeline_clips":[
            {"id":"fast","startTime":0,"duration":500,"sourceIn":1000,"sourceOut":2000,"playbackRate":2,"trackOrder":0,"muted":false},
            {"id":"slow","startTime":750,"duration":1000,"sourceIn":0,"sourceOut":500,"playbackRate":0.5,"trackOrder":0,"muted":false}
        ]
    })).unwrap();
    if camera_changes {
        settings.cursor_settings.as_mut().unwrap().enabled=Some(false);
        settings.webcam_keyframes = Some(serde_json::from_value(serde_json::json!([
            {"id":"hide","time":1000,"x":0.75,"y":0.75,"size":0.25,"shape":"circle","visible":false},
            {"id":"full","time":1500,"x":0.75,"y":0.75,"size":0.25,"shape":"circle","visible":true,"layout":"fullscreen"},
            {"id":"overlay","time":1750,"x":0.75,"y":0.75,"size":0.25,"shape":"circle","visible":true}
        ])).unwrap());
    }
    let info = VideoInfo {
        duration_ms: 2000,
        width: 16,
        height: 16,
        fps: 30.0,
        bitrate: 0,
        format: "mkv".into(),
        size_bytes: 0,
        frame_count: Some(60),
    };
    let control = std::sync::Arc::new(super::export_control::ExportControl::default());
    settings.cancellation = Some(control.clone());
    if cancel { std::fs::write(&output, b"existing export").unwrap(); }
    let callback_control = control.clone();
    let callback = cancel.then(|| Box::new(move |_: super::types::ProcessingProgress| {
        callback_control.cancel();
    }) as Box<dyn Fn(super::types::ProcessingProgress) + Send + Sync>);
    let reference=if camera_changes {Some(camera_reference::render(&root,&input,&settings,&info).await)} else {None};
    let result = export_video(&root, &input, settings, &info, callback).await;
    if cancel {
        assert!(result.is_err());
        assert!(control.check().is_err());
        assert_eq!(std::fs::read(&output).unwrap(), b"existing export");
        for entry in std::fs::read_dir(&root).unwrap() {
            let name = entry.unwrap().file_name().to_string_lossy().into_owned();
            assert!(!name.starts_with(".tarantino-export-") && !name.starts_with("timeline-audio-"));
        }
        std::fs::remove_dir_all(root).unwrap();
        return;
    }
    if let Err(error) = result {
        let _ = std::fs::remove_dir_all(&root);
        panic!("Edited export failed: {error:#}");
    }
    let frames = run(&strings(&[
        "-v",
        "error",
        "-i",
        output.to_str().unwrap(),
        "-map",
        "0:v",
        "-fps_mode",
        "passthrough",
        "-f",
        "rawvideo",
        "-pix_fmt",
        "rgba",
        "-",
    ]));
    let fps = if format == "gif" { 30.0 } else { 60.0 };
    assert_eq!(frames.len(), (1.75_f64 * fps).ceil() as usize * 64 * 64 * 4);
    for (i, frame) in frames.chunks_exact(64 * 64 * 4).enumerate() {
        if let Some(reference)=&reference {camera_reference::compare(reference,frame,i);continue;}
        let p = &frame[(32 * 64 + 32) * 4..][..4];
        let camera = &frame[(48 * 64 + 48) * 4..][..4];
        if camera_changes && i < 15 {
            assert!(
                camera[2] > 180 && camera[0] < 40,
                "Hidden camera frame {i}: {camera:?}"
            );
        } else if i < (0.5_f64 * fps).ceil() as usize {
            assert!(
                camera[0] > 170 && camera[1] > 170 && camera[2] < 60,
                "webcam frame {i}: {camera:?}"
            );
        } else if i < (0.75_f64 * fps).ceil() as usize {
            assert!(
                camera[..3].iter().all(|v| *v < 15),
                "webcam gap {i}: {camera:?}"
            );
        } else {
            assert!(
                camera[0] < 60 && camera[1] > 170 && camera[2] > 170,
                "webcam frame {i}: {camera:?}"
            );
        }
        if camera_changes && (15..23).contains(&i) {
            assert!(
                p[0] > 170 && p[1] > 170 && p[2] < 60,
                "Fullscreen camera frame {i}: {p:?}"
            );
            let corner = &frame[(5 * 64 + 5) * 4..][..4];
            assert!(
                corner[0] > 170 && corner[1] > 170 && corner[2] < 60,
                "Fullscreen corner {i}: {corner:?}"
            );
        } else if i < (0.5_f64 * fps).ceil() as usize {
            assert!(p[2] > 180 && p[0] < 40, "frame {i}: {p:?}");
        } else if i < (0.75_f64 * fps).ceil() as usize {
            assert!(p[0] < 15 && p[2] < 15, "frame {i}: {p:?}");
        } else {
            assert!(p[0] > 180 && p[2] < 40, "frame {i}: {p:?}");
        }
    }
    if format == "gif" {
        let bytes = std::fs::read(&output).unwrap();
        assert!(bytes.starts_with(b"GIF89a"));
        let looping = bytes
            .windows(11)
            .position(|b| b == b"NETSCAPE2.0")
            .expect("GIF loop extension");
        assert_eq!(&bytes[looping + 11..looping + 16], &[3, 1, 0, 0, 0]);
        let probe = Command::new("ffprobe")
            .args(["-v", "error", "-show_streams", "-of", "json"])
            .arg(&output)
            .output()
            .unwrap();
        assert!(probe.status.success());
        let metadata: serde_json::Value = serde_json::from_slice(&probe.stdout).unwrap();
        let streams = metadata["streams"].as_array().unwrap();
        assert_eq!(streams.len(), 1);
        assert_eq!(streams[0]["codec_name"], "gif");
        let duration: f64 = streams[0]["duration"].as_str().unwrap().parse().unwrap();
        assert!((duration - 1.75).abs() <= 0.041, "GIF timing: {duration}");
        std::fs::remove_dir_all(root).unwrap();
        return;
    }
    let audio = run(&strings(&[
        "-v",
        "error",
        "-i",
        output.to_str().unwrap(),
        "-map",
        "0:a",
        "-f",
        "f32le",
        "-ac",
        "1",
        "-ar",
        "48000",
        "-",
    ]));
    let samples: Vec<f32> = audio
        .chunks_exact(4)
        .map(|b| f32::from_le_bytes(b.try_into().unwrap()))
        .collect();
    assert!(
        samples.len() >= 84_000 && samples.len() <= 85_024,
        "AAC padding: {}",
        samples.len()
    );
    assert!(samples[25_000..35_000].iter().all(|s| s.abs() < 0.001));
    for (start, expected) in [(4800, 176), (48000, 44)] {
        let count = samples[start..start + 9600]
            .windows(2)
            .filter(|w| w[0] <= 0.0 && w[1] > 0.0)
            .count();
        assert!(
            (count as i32 - expected).abs() <= 8,
            "pitch crossings: {count}"
        );
    }
    std::fs::remove_dir_all(root).unwrap();
}
