//! Bounded source-time thumbnail decoding for edited timeline ranges.
use base64::{Engine as _, engine::general_purpose::STANDARD};
use serde::Serialize;
use std::sync::OnceLock;
use std::time::Duration;
use tokio::process::Command;
use tokio::sync::Semaphore;

#[derive(Debug, Serialize)]
pub struct ThumbnailSample {
    /// Requested source seek time, not an inferred presentation timestamp.
    pub source_time_ms: f64,
    pub image: String,
}

fn validate_times(times: &[f64]) -> Result<(), String> {
    if times.is_empty() || times.len() > 64 {
        return Err("Request between 1 and 64 thumbnail samples".into());
    }
    if times.iter().any(|time| !time.is_finite() || *time < 0.0) {
        return Err("Thumbnail source times must be finite and nonnegative".into());
    }
    Ok(())
}

#[tauri::command]
pub async fn extract_timeline_thumbnail_samples(
    video_path: String,
    source_times_ms: Vec<f64>,
    thumbnail_width: u32,
) -> Result<Vec<ThumbnailSample>, String> {
    validate_times(&source_times_ms)?;
    static DECODERS: OnceLock<Semaphore> = OnceLock::new();
    // Bound all callers together; scrubbing must not launch an unbounded queue of decoders.
    let _permit = DECODERS.get_or_init(|| Semaphore::new(2)).acquire().await
        .map_err(|error| error.to_string())?;
    let mut samples = Vec::with_capacity(source_times_ms.len());
    for time in source_times_ms {
        let time = (time * 1000.0).round() / 1000.0;
        let mut command = Command::new(crate::ffmpeg::binary("ffmpeg"));
        command.kill_on_drop(true).args([
            "-v", "error", "-hwaccel", "none", "-ss", &format!("{:.6}", time / 1000.0),
            "-i", &video_path, "-an", "-sn", "-frames:v", "1", "-vf",
            &format!("scale={}:-2:flags=fast_bilinear", thumbnail_width.clamp(32, 640)),
            "-c:v", "mjpeg", "-q:v", "3", "-f", "image2pipe", "pipe:1",
        ]);
        let output = tokio::time::timeout(Duration::from_secs(15), command.output()).await
            .map_err(|_| "Thumbnail decoding timed out".to_string())?
            .map_err(|error| format!("Could not start thumbnail decoder: {error}"))?;
        if !output.status.success() {
            return Err(format!("Thumbnail decoding failed: {}", String::from_utf8_lossy(&output.stderr).trim()));
        }
        if !output.stdout.starts_with(&[0xff, 0xd8]) || !output.stdout.ends_with(&[0xff, 0xd9]) {
            return Err(format!("No thumbnail frame at source time {time} ms"));
        }
        samples.push(ThumbnailSample {
            source_time_ms: time,
            image: format!("data:image/jpeg;base64,{}", STANDARD.encode(output.stdout)),
        });
    }
    Ok(samples)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn rejects_unbounded_or_invalid_sample_requests() {
        for values in [vec![], vec![0.0;65], vec![f64::NAN], vec![f64::INFINITY], vec![-1.0]] {
            assert!(validate_times(&values).is_err());
        }
        assert!(validate_times(&[0.0, 250.125]).is_ok());
    }

    #[tokio::test]
    async fn samples_requested_source_frames_without_fallback_to_recording_start() {
        if std::env::var_os("TARANTINO_TEST_FFMPEG").is_none() { return; }
        let input = std::env::temp_dir().join(format!("thumbnail-source-{}.mkv",uuid::Uuid::new_v4()));
        let status = std::process::Command::new(crate::ffmpeg::binary("ffmpeg")).args([
            "-v","error","-f","lavfi","-i","color=red:s=64x64:r=30:d=1",
            "-f","lavfi","-i","color=blue:s=64x64:r=30:d=1",
            "-filter_complex","[0:v][1:v]concat=n=2:v=1:a=0","-c:v","ffv1","-y",
        ]).arg(&input).status().unwrap();
        assert!(status.success());
        let samples = extract_timeline_thumbnail_samples(input.to_string_lossy().into_owned(),vec![500.0,1500.0],64).await.unwrap();
        for (index,sample) in samples.iter().enumerate() {
            let jpeg=STANDARD.decode(sample.image.split_once(',').unwrap().1).unwrap();
            let image=image::load_from_memory(&jpeg).unwrap().to_rgb8();
            let pixel=image.get_pixel(32,32).0;
            if index==0 { assert!(pixel[0]>200 && pixel[2]<30, "{pixel:?}"); }
            else { assert!(pixel[2]>200 && pixel[0]<30, "{pixel:?}"); }
        }
        assert_eq!(samples[1].source_time_ms,1500.0);
        assert!(extract_timeline_thumbnail_samples(input.to_string_lossy().into_owned(),vec![5000.0],64).await.is_err());
        std::fs::remove_file(input).unwrap();
    }
}
