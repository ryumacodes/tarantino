use super::*;
use std::process::Command;

#[tokio::test]
#[ignore = "creates a native recording and editor fixture for Linux WebDriver tests"]
async fn linux_editor_e2e_fixture() -> Result<()> {
    use crate::input::{MouseButton, MouseEvent, MouseEventType};
    use crate::recording::artifacts::RecordingArtifacts;
    use std::time::{SystemTime, UNIX_EPOCH};

    let output_dir = PathBuf::from(std::env::var("TARANTINO_E2E_FIXTURE_DIR")?);
    std::fs::create_dir_all(&output_dir)?;
    let input = output_dir.join(format!("capture-{}.mp4", uuid::Uuid::new_v4()));
    runtime_preflight()?;
    let config = RecordingConfig {
        quality: QualityPreset::Low,
        ..Default::default()
    };
    let (kind, gpu) = select_working_encoder(&config)?;
    let source = gst::ElementFactory::make("videotestsrc")
        .property("is-live", true)
        .property("num-buffers", 180i32)
        .build()?;
    let sink = gst::ElementFactory::make("filesink")
        .property("location", input.to_string_lossy().as_ref())
        .build()?;
    let pipeline = build_pipeline_with_source(&config, source, sink, kind, gpu)?;
    let start = SystemTime::now();
    let capture = (|| {
        pipeline.set_state(gst::State::Playing)?;
        wait_for_pipeline(&pipeline)
    })();
    pipeline.set_state(gst::State::Null)?;
    capture?;
    let start_ms = start.duration_since(UNIX_EPOCH)?.as_millis() as u64;
    let events = vec![MouseEvent {
        timestamp: start_ms + 1500,
        x: 224.0,
        y: 96.0,
        event_type: MouseEventType::ButtonPress {
            button: MouseButton::Left,
        },
        display_id: None,
    }];
    // Run the same timestamp normalization, sidecar writing and zoom analysis
    // used when stopping a recording. No hand-authored zoom blocks in this test.
    let finalized = crate::commands::processing::process_recorded_file(
        input.to_str().context("Invalid fixture path")?,
        events,
        vec![],
        Some(start),
        (320, 240, None),
    )
    .await?;
    let source = RecordingArtifacts::new(&finalized);
    let destination = RecordingArtifacts::new(output_dir.join("recording.mp4"));
    let result = (|| {
        std::fs::copy(&finalized, output_dir.join("recording.mp4"))?;
        for (from, to) in source.sidecar_pairs(&destination) {
            if from.exists() {
                std::fs::copy(from, to)?;
            }
        }
        let analysis = crate::auto_zoom::load_analysis(&destination.auto_zoom())?;
        anyhow::ensure!(
            !analysis.zoom_blocks.is_empty(),
            "Recording produced no zoom blocks"
        );
        Ok(())
    })();
    // Remove only this generated recording's originals, retaining the fixture.
    for path in source.all_paths() {
        let _ = std::fs::remove_file(path);
    }
    result
}

#[test]
fn rejected_hardware_probes_fall_back_in_priority_order() {
    let mut attempted = Vec::new();
    let selected = select_h264_encoder(
        |name| {
            attempted.push(name.to_string());
            name == "x264enc"
        },
        true,
        true,
    )
    .unwrap();
    assert_eq!(selected, H264Encoder::X264);
    assert_eq!(
        attempted,
        ["nvh264enc", "vah264enc", "vaapih264enc", "x264enc"]
    );
}

#[test]
#[ignore = "requires installed GStreamer recording plugins and FFmpeg"]
fn linux_recording_runtime_smoke() -> Result<()> {
    runtime_preflight()?;
    // Exercise every installed software fallback, not just the preferred one.
    for kind in [H264Encoder::X264, H264Encoder::OpenH264] {
        if gst::ElementFactory::find(kind.factory_name()).is_some() {
            for quality in [
                QualityPreset::Low,
                QualityPreset::High,
                QualityPreset::Lossless,
            ] {
                probe_encoder(
                    &RecordingConfig {
                        quality,
                        ..Default::default()
                    },
                    kind,
                    false,
                )?;
            }
        }
    }
    let config = RecordingConfig {
        quality: QualityPreset::Low,
        ..Default::default()
    };
    let (kind, gpu) = select_working_encoder(&config)?;
    println!(
        "Linux recording smoke encoder: {} (VA conversion: {gpu})",
        kind.factory_name()
    );
    // Two sessions catch failure to release encoder/muxer resources after EOS.
    for _ in 0..2 {
        let path =
            std::env::temp_dir().join(format!("tarantino-smoke-{}.mp4", uuid::Uuid::new_v4()));
        let result = record_and_validate(&config, kind, gpu, &path);
        let _ = std::fs::remove_file(&path);
        result?;
    }
    Ok(())
}

fn record_and_validate(
    config: &RecordingConfig,
    kind: H264Encoder,
    gpu: bool,
    path: &Path,
) -> Result<()> {
    let source = gst::ElementFactory::make("videotestsrc")
        .property("is-live", true)
        .property("num-buffers", 30i32)
        .build()?;
    let sink = gst::ElementFactory::make("filesink")
        .property("location", path.to_string_lossy().as_ref())
        .build()?;
    let pipeline = build_pipeline_with_source(config, source, sink, kind, gpu)?;
    let result = (|| {
        pipeline.set_state(gst::State::Playing)?;
        wait_for_pipeline(&pipeline)
    })();
    pipeline.set_state(gst::State::Null)?;
    result?;
    let output = Command::new("ffprobe")
        .args([
            "-v",
            "error",
            "-count_frames",
            "-select_streams",
            "v:0",
            "-show_entries",
            "stream=codec_name,width,height,nb_read_frames,duration",
            "-of",
            "json",
        ])
        .arg(path)
        .output()?;
    anyhow::ensure!(
        output.status.success(),
        "ffprobe failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let json: serde_json::Value = serde_json::from_slice(&output.stdout)?;
    let stream = &json["streams"][0];
    anyhow::ensure!(stream["codec_name"] == "h264", "Not H.264: {stream}");
    anyhow::ensure!(
        stream["width"] == 320 && stream["height"] == 240,
        "Wrong dimensions: {stream}"
    );
    anyhow::ensure!(
        stream["nb_read_frames"] == "30",
        "Dropped or unreadable frames: {stream}"
    );
    let duration: f64 = stream["duration"]
        .as_str()
        .context("Missing duration")?
        .parse()?;
    anyhow::ensure!(
        (duration - 1.0).abs() < 0.1,
        "Unexpected duration: {duration}"
    );
    let decoded = Command::new("ffmpeg")
        .args(["-v", "error", "-xerror", "-i"])
        .arg(path)
        .args(["-f", "null", "-"])
        .output()?;
    anyhow::ensure!(
        decoded.status.success(),
        "Decode failed: {}",
        String::from_utf8_lossy(&decoded.stderr)
    );
    Ok(())
}
