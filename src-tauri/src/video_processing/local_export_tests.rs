//! Opt-in validation of an existing recording through the production exporter.
//! Paths/settings are supplied locally; no personal recordings are test assets.
use super::{export::export_video, ExportSettings, VideoProcessor};
use anyhow::{Context,Result,ensure};
use serde::Deserialize;
use std::{path::PathBuf,process::Command};

#[derive(Deserialize)]
struct ExportCase { settings:ExportSettings, expected_frames:u64 }
#[derive(Deserialize)]
struct Manifest { input:PathBuf, cases:Vec<ExportCase> }

#[tokio::test]
#[ignore="requires explicit local manifest, FFmpeg and GPU"]
async fn validate_existing_recording_exports()->Result<()> {
    let manifest_path=PathBuf::from(std::env::var_os("TARANTINO_EXPORT_VALIDATION")
        .context("Set TARANTINO_EXPORT_VALIDATION to a local export manifest")?);
    let manifest:Manifest=serde_json::from_slice(&std::fs::read(&manifest_path)?)?;
    ensure!(!manifest.cases.is_empty(),"No exports requested");
    let processor=VideoProcessor::new()?;
    let info=processor.get_video_info(&manifest.input).await?;
    let root=manifest_path.parent().context("Manifest has no parent")?;
    let mut results=Vec::new();
    for case in manifest.cases {
        let expected_size=case.settings.resolution.as_ref().context("Specify output dimensions")?;
        let (width,height)=(expected_size.width,expected_size.height);
        let start=std::time::Instant::now();
        let output=export_video(root,&manifest.input,case.settings,&info,None).await?;
        let seconds=start.elapsed().as_secs_f64();
        let probe=Command::new(crate::ffmpeg::binary("ffprobe")).args(["-v","error","-select_streams","v:0","-count_frames",
            "-show_entries","stream=width,height,nb_read_frames,codec_name,r_frame_rate,duration","-of","json"])
            .arg(&output).output()?;
        ensure!(probe.status.success(),"Export probe failed: {}",String::from_utf8_lossy(&probe.stderr));
        let value:serde_json::Value=serde_json::from_slice(&probe.stdout)?;
        let stream=&value["streams"][0];
        ensure!(stream["width"]==width && stream["height"]==height,"Export dimensions differ: {stream}");
        let frames=stream["nb_read_frames"].as_str().context("No decoded frame count")?.parse::<u64>()?;
        ensure!(frames==case.expected_frames,"Expected {} frames, decoded {frames}",case.expected_frames);
        results.push(serde_json::json!({"output":output,"seconds":seconds,"stream":stream}));
    }
    std::fs::write(root.join("export-validation-results.json"),serde_json::to_vec_pretty(&results)?)?;
    Ok(())
}
