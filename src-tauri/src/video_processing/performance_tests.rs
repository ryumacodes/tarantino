//! Opt-in end-to-end benchmark; excluded from production binaries.
use super::{ExportSettings, VideoInfo, export::export_video};
use anyhow::{Context, Result, ensure};
use std::{path::Path, process::Command, time::Instant};

#[tokio::test]
#[ignore = "requires FFmpeg and a GPU; run explicitly on an idle machine"]
async fn export_performance_benchmark() -> Result<()> {
    let root = std::env::temp_dir().join(format!("tarantino-bench-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&root)?;
    let result = benchmark(&root).await;
    let _ = std::fs::remove_dir_all(&root);
    result
}

async fn benchmark(root: &Path) -> Result<()> {
    let input = root.join("fixture.mp4");
    // Generate outside the timed region. Use the same FFmpeg build on both
    // revisions, or share one fixture with TARANTINO_BENCH_INPUT.
    if let Some(fixture) = std::env::var_os("TARANTINO_BENCH_INPUT") {
        std::fs::copy(fixture, &input)?;
    } else {
        let generated = Command::new("ffmpeg")
            .args([
                "-v",
                "error",
                "-f",
                "lavfi",
                "-i",
                "testsrc2=size=1920x1080:rate=60",
                "-frames:v",
                "300",
                "-c:v",
                "libx264",
                "-preset",
                "veryfast",
                "-crf",
                "18",
                "-pix_fmt",
                "yuv420p",
            ])
            .arg(&input)
            .output()?;
        ensure!(
            generated.status.success(),
            "Fixture generation failed: {}",
            String::from_utf8_lossy(&generated.stderr)
        );
    }
    let stream = probe(&input)?;
    let frames: u64 = stream["nb_read_frames"]
        .as_str()
        .context("Missing frame count")?
        .parse()?;
    ensure!(
        stream["width"] == 1920
            && stream["height"] == 1080
            && frames == 300
            && stream["r_frame_rate"] == "60/1",
        "Fixture must be 300 frames at 1920x1080/60fps"
    );
    let info = VideoInfo {
        duration_ms: 5000,
        width: 1920,
        height: 1080,
        fps: 60.0,
        bitrate: 0,
        format: "mp4".into(),
        size_bytes: std::fs::metadata(&input)?.len(),
        frame_count: Some(frames),
    };
    let runs: usize = std::env::var("TARANTINO_BENCH_RUNS")
        .unwrap_or_else(|_| "5".into())
        .parse()?;
    ensure!((1..=20).contains(&runs), "Use 1–20 measured runs");
    let mut timings = Vec::new();
    for run in 0..=runs {
        let output = root.join(format!("export-{run}.mp4"));
        let settings: ExportSettings = serde_json::from_value(serde_json::json!({
            "output_path": output, "resolution": {"width": 1920, "height": 1080},
            "frame_rate": 60, "format": "mp4", "codec": "h264", "quality": "high",
            "source_width": 1920, "source_height": 1080,
            "cursor_settings": {"enabled": false},
            "visual_settings": {"padding": 32, "corner_radius": 16,
                "shadow_enabled": true, "background_type": "solid", "background_color": "#202020"}
        }))?;
        let start = Instant::now();
        export_video(root, &input, settings, &info, None).await?;
        let elapsed = start.elapsed().as_secs_f64();
        // Validate outside the timed region; a fast truncated export is a failure.
        let stream = probe(&output)?;
        ensure!(
            stream["nb_read_frames"] == "300"
                && stream["width"] == 1920
                && stream["height"] == 1080,
            "Invalid benchmark output: {stream}"
        );
        if run > 0 {
            timings.push(elapsed);
            println!(
                "BENCH run={run} seconds={elapsed:.6} fps={:.3}",
                frames as f64 / elapsed
            );
        }
        std::fs::remove_file(output)?;
    }
    timings.sort_by(f64::total_cmp);
    let median = if runs % 2 == 0 {
        (timings[runs / 2 - 1] + timings[runs / 2]) / 2.0
    } else {
        timings[runs / 2]
    };
    println!(
        "BENCH_RESULT {}",
        serde_json::json!({
            "os": std::env::consts::OS, "arch": std::env::consts::ARCH,
            "debug_build": cfg!(debug_assertions), "runs": runs,
            "median_seconds": median, "median_fps": frames as f64 / median,
            "sorted_seconds": timings,
        })
    );
    Ok(())
}

fn probe(path: &Path) -> Result<serde_json::Value> {
    let result = Command::new("ffprobe")
        .args([
            "-v",
            "error",
            "-count_frames",
            "-select_streams",
            "v:0",
            "-show_entries",
            "stream=width,height,nb_read_frames,r_frame_rate",
            "-of",
            "json",
        ])
        .arg(path)
        .output()?;
    ensure!(
        result.status.success(),
        "ffprobe failed: {}",
        String::from_utf8_lossy(&result.stderr)
    );
    let json: serde_json::Value = serde_json::from_slice(&result.stdout)?;
    Ok(json["streams"][0].clone())
}
