//! Actual encoder validation using locally generated browser caption data.
use super::{
    export::export_video,
    types::{ExportSettings, VideoInfo},
};
use std::process::Command;
fn ffmpeg(args: &[&str]) -> Vec<u8> {
    let result = Command::new(crate::ffmpeg::binary("ffmpeg")).args(args).output().unwrap();
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    result.stdout
}
#[tokio::test]
#[ignore = "requires browser caption fixture, FFmpeg and Metal"]
async fn captions_reach_encoded_video_and_leave_blank_frames_clean() {
    let path = std::env::var("TARANTINO_CAPTION_PAYLOAD").expect("Set caption browser payload");
    let payload: super::caption_overlay::CaptionOverlay =
        serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap();
    validate_caption_export(payload).await;
}

async fn validate_caption_export(payload: super::caption_overlay::CaptionOverlay) {
    assert_eq!(
        (payload.width, payload.height, payload.frames),
        (960, 540, 150)
    );
    validate_edited_export(
        payload,
        None,
        &[
            ("0.1", false, false),
            ("1.65", true, false),
            ("4.8", false, false),
        ],
    )
    .await;
}
async fn validate_edited_export(
    payload: super::caption_overlay::CaptionOverlay,
    timeline: Option<serde_json::Value>,
    samples: &[(&str, bool, bool)],
) {
    let expected_frames = payload.frames;
    let root = std::env::temp_dir().join(format!("caption-export-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&root).unwrap();
    let input = root.join("blue.mkv");
    let output = root.join("caption.mp4");
    ffmpeg(&[
        "-v",
        "error",
        "-f",
        "lavfi",
        "-i",
        "color=c=0x224466:s=960x540:r=30:d=5",
        "-c:v",
        "ffv1",
        input.to_str().unwrap(),
    ]);
    let info = VideoInfo {
        duration_ms: 5000,
        width: 960,
        height: 540,
        fps: 30.0,
        bitrate: 0,
        format: "mkv".into(),
        size_bytes: 0,
        frame_count: Some(150),
    };
    let mut config = serde_json::json!({
        "output_path":output,"resolution":{"width":960,"height":540},"source_width":960,"source_height":540,
        "frame_rate":30,"format":"mp4","codec":"h264","zoom_blocks":[],"caption_overlay":payload,
        "cursor_settings":{"enabled":false},"visual_settings":{"padding":0,"corner_radius":0,"shadow_enabled":false}
    });
    if let Some(mut timeline) = timeline {
        for clip in timeline["timeline_clips"].as_array_mut().unwrap() {
            clip["sourceFilePath"] = serde_json::json!(input);
        }
        config
            .as_object_mut()
            .unwrap()
            .extend(timeline.as_object().unwrap().clone());
    }
    let settings: ExportSettings = serde_json::from_value(config).unwrap();
    export_video(&root, &input, settings, &info, None)
        .await
        .unwrap();
    // Decode representative early, active and late frames rather than loading the entire video.
    for &(time, active, black) in samples {
        let pixels = ffmpeg(&[
            "-v",
            "error",
            "-ss",
            time,
            "-i",
            output.to_str().unwrap(),
            "-frames:v",
            "1",
            "-f",
            "rawvideo",
            "-pix_fmt",
            "rgb24",
            "-",
        ]);
        assert_eq!(pixels.len(), 960 * 540 * 3);
        let white = pixels
            .chunks_exact(3)
            .filter(|p| p.iter().all(|c| *c > 180))
            .count();
        if active {
            assert!(
                white > 100,
                "Caption text missing at {time}; {white} white pixels"
            );
        } else {
            assert_eq!(white, 0, "Stale caption at {time}");
        }
        for (index, pixel) in pixels.chunks_exact(3).enumerate() {
            if index / 960 < 300 {
                assert!(
                    if black {
                        pixel.iter().all(|c| *c < 15)
                    } else {
                        pixel[0] < 60 && pixel[1] < 100 && pixel[2] > 70
                    },
                    "Caption changed unrelated region at {time}"
                );
            }
        }
    }
    let probe = Command::new(crate::ffmpeg::binary("ffprobe"))
        .args([
            "-v",
            "error",
            "-select_streams",
            "v:0",
            "-count_frames",
            "-show_entries",
            "stream=nb_read_frames",
            "-of",
            "csv=p=0",
        ])
        .arg(&output)
        .output()
        .unwrap();
    assert!(probe.status.success());
    assert_eq!(
        String::from_utf8_lossy(&probe.stdout).trim(),
        expected_frames.to_string()
    );
    println!("Caption export verified: {}", output.display());
}

#[tokio::test]
#[ignore = "requires browser caption style matrix, FFmpeg and Metal"]
async fn all_caption_styles_reach_encoded_video() {
    #[derive(serde::Deserialize)]
    struct Case {
        name: String,
        payload: super::caption_overlay::CaptionOverlay,
    }
    let path = std::env::var("TARANTINO_CAPTION_MATRIX").expect("Set caption style matrix");
    let cases: Vec<Case> = serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap();
    assert_eq!(cases.len(), 18);
    let names = cases
        .iter()
        .map(|case| &case.name)
        .collect::<std::collections::HashSet<_>>();
    assert_eq!(names.len(), 18);
    for case in cases {
        println!("Checking caption style {}", case.name);
        validate_caption_export(case.payload).await;
    }
}

#[tokio::test]
#[ignore = "requires edited browser caption fixture, FFmpeg and Metal"]
async fn captions_follow_cuts_gaps_repeats_rates_and_trim() {
    let path = std::env::var("TARANTINO_CAPTION_EDITED").expect("Set edited caption payload");
    let mut value: serde_json::Value =
        serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap();
    let payload: super::caption_overlay::CaptionOverlay =
        serde_json::from_value(value["payload"].take()).unwrap();
    assert_eq!(payload.frames, 108);
    validate_edited_export(
        payload,
        Some(value["timeline"].take()),
        &[
            ("0.1", true, false),
            ("1.0", false, true),
            ("1.5", true, false),
            ("2.8", true, false),
        ],
    )
    .await;
}
