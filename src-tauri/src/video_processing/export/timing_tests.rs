//! Exercise the actual raw-frame encoder arguments, including hardware timestamps.
use super::*;
use std::{
    io::Write,
    process::{Command, Stdio},
};

#[test]
#[ignore = "requires local FFmpeg and VideoToolbox hardware access"]
fn hardware_exports_preserve_frame_cadence_and_duration() {
    let root = std::env::temp_dir().join(format!("export-timing-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&root).unwrap();
    let result = std::panic::catch_unwind(|| {
        for codec in ["h264", "h265"] {
            for fps in [30, 60] {
                verify_timing(&root, codec, fps);
            }
        }
    });
    std::fs::remove_dir_all(root).unwrap();
    if let Err(error) = result {
        std::panic::resume_unwind(error);
    }
}

fn verify_timing(root: &Path, codec: &str, fps: u32) {
    let settings: ExportSettings = serde_json::from_value(serde_json::json!({
        "format": "mp4", "codec": codec, "quality": "high"
    }))
    .unwrap();
    let tracks = AudioTrackFiles {
        background: None,
        mic: None,
        system: None,
    };
    let path = root.join(format!("{codec}-{fps}.mp4"));
    let args = encoder_args(
        &settings,
        &ExportAcceleration::VideoToolbox,
        &tracks,
        &settings,
        (128, 128),
        fps,
        &path,
    );
    let mut child = Command::new("ffmpeg")
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let source = Command::new("ffmpeg")
        .args([
            "-v",
            "error",
            "-f",
            "lavfi",
            "-i",
            &format!("testsrc2=size=128x128:rate={fps}:duration=8"),
            "-pix_fmt",
            "rgba",
            "-f",
            "rawvideo",
            "-",
        ])
        .output()
        .unwrap();
    assert!(source.status.success());
    child
        .stdin
        .take()
        .unwrap()
        .write_all(&source.stdout)
        .unwrap();
    let output = child.wait_with_output().unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let probe = Command::new("ffprobe")
        .args([
            "-v",
            "error",
            "-select_streams",
            "v:0",
            "-show_entries",
            "frame=best_effort_timestamp_time:stream=duration,nb_frames",
            "-of",
            "json",
        ])
        .arg(path)
        .output()
        .unwrap();
    assert!(probe.status.success());
    let data: serde_json::Value = serde_json::from_slice(&probe.stdout).unwrap();
    let frames = data["frames"].as_array().unwrap();
    assert_eq!(
        frames.len(),
        fps as usize * 8,
        "{codec}/{fps} decoded frame count"
    );
    let duration: f64 = data["streams"][0]["duration"]
        .as_str()
        .unwrap()
        .parse()
        .unwrap();
    assert!(
        (duration - 8.0).abs() < 0.0001,
        "{codec}/{fps} duration: {duration}"
    );
    for (index, frame) in frames.iter().enumerate() {
        let timestamp: f64 = frame["best_effort_timestamp_time"]
            .as_str()
            .unwrap()
            .parse()
            .unwrap();
        let expected = index as f64 / fps as f64;
        assert!(
            (timestamp - expected).abs() < 0.0001,
            "{codec}/{fps} frame {index}: {timestamp}, expected {expected}"
        );
    }
}
