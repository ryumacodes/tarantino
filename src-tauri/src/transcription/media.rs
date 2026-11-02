//! Decode one selected audio track; the owned directory lives through recognition.
use super::{Cancellation, Limits, process};
use anyhow::{Context, Result, ensure};
use std::{
    path::{Path, PathBuf},
    time::Duration,
};
use tokio::process::Command;

pub(crate) struct PreparedAudio {
    directory: PathBuf,
    pub path: PathBuf,
    pub duration_ms: f64,
}
impl Drop for PreparedAudio {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.directory);
    }
}
impl PreparedAudio {
    fn create() -> Result<Self> {
        let directory =
            std::env::temp_dir().join(format!("tarantino-speech-{}", uuid::Uuid::new_v4()));
        let mut builder = std::fs::DirBuilder::new();
        #[cfg(unix)]
        {
            use std::os::unix::fs::DirBuilderExt;
            builder.mode(0o700);
        }
        builder
            .create(&directory)
            .context("Cannot create transcription workspace")?;
        Ok(Self {
            path: directory.join("audio.wav"),
            directory,
            duration_ms: 0.0,
        })
    }
}

pub(crate) async fn prepare_audio(
    source: &Path,
    audio_track: u32,
    cancel: &Cancellation,
) -> Result<PreparedAudio> {
    cancel.check()?;
    // Canonical absolute paths also prevent filenames being interpreted as protocols/options.
    let source = source
        .canonicalize()
        .context("The selected recording is missing")?;
    ensure!(source.is_file(), "Select a recording file");
    let mut prepared = PreparedAudio::create()?;
    let mut command = Command::new("ffmpeg");
    command
        .args(["-nostdin", "-hide_banner", "-v", "error", "-n", "-i"])
        .arg(source)
        .args([
            "-map",
            &format!("0:a:{audio_track}"),
            "-vn",
            "-ac",
            "1",
            "-ar",
            "16000",
            "-c:a",
            "pcm_s16le",
        ])
        .arg(&prepared.path);
    let limits = Limits {
        timeout: Duration::from_secs(3600),
        stdout: 1024,
        stderr: 64 * 1024,
    };
    let (status, _, stderr) = process::run(&mut command, cancel, limits)
        .await
        .context("Audio preparation failed")?;
    ensure!(
        status.success(),
        "Could not prepare the selected audio track: {}",
        String::from_utf8_lossy(&stderr).trim()
    );
    let metadata = process::run_json(
        Path::new("ffprobe"),
        &[
            "-v".into(),
            "error".into(),
            "-show_entries".into(),
            "format=duration".into(),
            "-of".into(),
            "json".into(),
            prepared.path.as_os_str().into(),
        ],
        cancel,
        Limits {
            timeout: Duration::from_secs(30),
            stdout: 4096,
            stderr: 4096,
        },
    )
    .await?;
    let duration = metadata["format"]["duration"]
        .as_str()
        .context("Prepared audio has no duration")?
        .parse::<f64>()
        .context("Invalid prepared audio duration")?
        * 1000.0;
    ensure!(
        duration.is_finite() && duration > 0.0,
        "The selected audio track is empty"
    );
    prepared.duration_ms = duration;
    cancel.check()?;
    Ok(prepared)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[tokio::test]
    #[ignore = "requires local FFmpeg and ffprobe"]
    async fn extracts_selected_track_and_cleans_workspace() {
        let fixture = PreparedAudio::create().unwrap();
        let source = fixture.directory.join("two tracks.mkv");
        let output = Command::new("ffmpeg")
            .args([
                "-v",
                "error",
                "-f",
                "lavfi",
                "-i",
                "sine=frequency=440:duration=0.3",
                "-f",
                "lavfi",
                "-i",
                "sine=frequency=880:duration=0.7",
                "-map",
                "0:a",
                "-map",
                "1:a",
                "-c:a",
                "pcm_s16le",
            ])
            .arg(&source)
            .output()
            .await
            .unwrap();
        assert!(output.status.success());
        let cancel = Cancellation::default();
        let first = prepare_audio(&source, 0, &cancel).await.unwrap();
        let second = prepare_audio(&source, 1, &cancel).await.unwrap();
        assert!((first.duration_ms - 300.0).abs() < 1.0);
        assert!((second.duration_ms - 700.0).abs() < 1.0);
        let directory = first.directory.clone();
        drop(first);
        assert!(!directory.exists());
        assert!(prepare_audio(&source, 2, &cancel).await.is_err());
        cancel.cancel();
        assert!(prepare_audio(&source, 0, &cancel).await.is_err());
    }
}
