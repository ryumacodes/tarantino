//! Local speech commands. Only one model/setup job runs at a time.
use crate::transcription::{self, Cancellation, Limits, response::Transcript};
use serde_json::Value;
use std::{path::Path, sync::Mutex, time::Duration};

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SpeechSource {
    path: String,
    audio_track: u32,
    label: String,
}

#[tauri::command]
pub async fn speech_sources(source_file_path: String) -> Result<Vec<SpeechSource>, String> {
    let artifacts = crate::recording::artifacts::RecordingArtifacts::new(&source_file_path);
    let mut sources = Vec::new();
    for (path, label) in [
        (artifacts.microphone(), "Microphone"),
        (artifacts.system_audio(), "System audio"),
    ] {
        if path.is_file() {
            sources.push(SpeechSource {
                path: path.to_string_lossy().into_owned(),
                audio_track: 0,
                label: label.into(),
            });
        }
    }
    let path = Path::new(&source_file_path)
        .canonicalize()
        .map_err(|e| e.to_string())?;
    let metadata = transcription::run_json(
        Path::new("ffprobe"),
        &[
            "-v".into(),
            "error".into(),
            "-select_streams".into(),
            "a".into(),
            "-show_entries".into(),
            "stream=index".into(),
            "-of".into(),
            "json".into(),
            path.into_os_string(),
        ],
        &Cancellation::default(),
        Limits {
            timeout: Duration::from_secs(30),
            stdout: 64 * 1024,
            stderr: 4096,
        },
    )
    .await
    .map_err(|e| e.to_string())?;
    if let Some(streams) = metadata["streams"].as_array() {
        for (index, _) in streams.iter().enumerate() {
            sources.push(SpeechSource {
                path: source_file_path.clone(),
                audio_track: index as u32,
                label: format!("Recording audio {}", index + 1),
            });
        }
    }
    Ok(sources)
}

#[derive(Default)]
struct Jobs(Mutex<Option<(String, Cancellation)>>);
static JOBS: Jobs = Jobs(Mutex::new(None));

struct Job<'a> {
    jobs: &'a Jobs,
    cancel: Cancellation,
}
impl Jobs {
    fn begin(&self, id: &str) -> Result<Job<'_>, String> {
        if id.is_empty() || id.len() > 128 {
            return Err("Invalid transcription job ID".into());
        }
        let mut active = self
            .0
            .lock()
            .map_err(|_| "Transcription state unavailable")?;
        if active.is_some() {
            return Err("Another transcription or model setup is still running".into());
        }
        let cancel = Cancellation::default();
        *active = Some((id.to_owned(), cancel.clone()));
        Ok(Job { jobs: self, cancel })
    }
    fn cancel(&self, id: &str) -> Result<bool, String> {
        let active = self
            .0
            .lock()
            .map_err(|_| "Transcription state unavailable")?;
        if let Some((current, cancel)) = active.as_ref().filter(|(current, _)| current == id) {
            let _ = current;
            cancel.cancel();
            return Ok(true);
        }
        Ok(false)
    }
}
impl Drop for Job<'_> {
    fn drop(&mut self) {
        // Dropped IPC futures must also signal any in-flight helper work.
        self.cancel.cancel();
        if let Ok(mut active) = self.jobs.0.lock() {
            *active = None;
        }
    }
}
fn validate_locale(locale: &str) -> Result<(), String> {
    if locale.is_empty() || locale.len() > 128 {
        return Err("Choose a speech language".into());
    }
    Ok(())
}

#[tauri::command]
pub async fn speech_status(locale: String) -> Result<Value, String> {
    validate_locale(&locale)?;
    let helper = transcription::helper_path().map_err(|e| e.to_string())?;
    transcription::run_json(
        &helper,
        &["status".into(), locale.into()],
        &Cancellation::default(),
        Limits {
            timeout: Duration::from_secs(30),
            stdout: 64 * 1024,
            stderr: 4096,
        },
    )
    .await
    .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn install_speech_model(job_id: String, locale: String) -> Result<Value, String> {
    validate_locale(&locale)?;
    let helper = transcription::helper_path().map_err(|e| e.to_string())?;
    let job = JOBS.begin(&job_id)?;
    transcription::run_json(
        &helper,
        &["install".into(), locale.into()],
        &job.cancel,
        Limits {
            timeout: Duration::from_secs(3600),
            stdout: 64 * 1024,
            stderr: 4096,
        },
    )
    .await
    .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn generate_transcript(
    job_id: String,
    source_file_path: String,
    audio_track: u32,
    locale: String,
) -> Result<Transcript, String> {
    validate_locale(&locale)?;
    let helper = transcription::helper_path().map_err(|e| e.to_string())?;
    let job = JOBS.begin(&job_id)?;
    transcription::transcribe_media(
        &helper,
        Path::new(&source_file_path),
        audio_track,
        &locale,
        &job.cancel,
    )
    .await
    .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn cancel_transcription(job_id: String) -> Result<bool, String> {
    JOBS.cancel(&job_id)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[tokio::test]
    #[ignore = "requires local FFmpeg and ffprobe"]
    async fn discovers_sidecars_and_embedded_tracks() {
        let directory =
            std::env::temp_dir().join(format!("tarantino-source-test-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir(&directory).unwrap();
        struct Cleanup(std::path::PathBuf);
        impl Drop for Cleanup {
            fn drop(&mut self) {
                let _ = std::fs::remove_dir_all(&self.0);
            }
        }
        let _cleanup = Cleanup(directory.clone());
        let source = directory.join("recording.mkv");
        let output = tokio::process::Command::new(crate::ffmpeg::binary("ffmpeg"))
            .args([
                "-v",
                "error",
                "-f",
                "lavfi",
                "-i",
                "color=size=16x16:duration=0.1",
                "-f",
                "lavfi",
                "-i",
                "sine=duration=0.1",
                "-map",
                "0:v",
                "-map",
                "1:a",
                "-map",
                "1:a",
                "-c:v",
                "ffv1",
                "-c:a",
                "pcm_s16le",
            ])
            .arg(&source)
            .output()
            .await
            .unwrap();
        assert!(output.status.success());
        let artifacts = crate::recording::artifacts::RecordingArtifacts::new(&source);
        let output = tokio::process::Command::new(crate::ffmpeg::binary("ffmpeg"))
            .args(["-v", "error", "-i"])
            .arg(&source)
            .args(["-map", "0:a:0"])
            .arg(artifacts.microphone())
            .output()
            .await
            .unwrap();
        assert!(output.status.success());
        let sources = speech_sources(source.to_string_lossy().into_owned())
            .await
            .unwrap();
        assert_eq!(sources.len(), 3);
        assert_eq!(sources[0].label, "Microphone");
        assert_eq!(sources[1].audio_track, 0);
        assert_eq!(sources[2].audio_track, 1);
        assert_eq!(sources[2].path, source.to_string_lossy());
        // Stream selection is audio-relative, even with a video stream at global index zero.
        assert!(!sources.iter().any(|source| source.label == "System audio"));
    }
    #[test]
    fn cancellation_keeps_slot_until_worker_finishes() {
        let jobs = Jobs::default();
        assert!(jobs.begin("").is_err());
        let job = jobs.begin("first").unwrap();
        assert!(jobs.begin("second").is_err());
        assert!(!jobs.cancel("other").unwrap());
        assert!(job.cancel.check().is_ok());
        assert!(jobs.cancel("first").unwrap());
        assert!(job.cancel.check().is_err());
        assert!(jobs.begin("second").is_err());
        drop(job);
        let next = jobs.begin("second").unwrap();
        assert!(next.cancel.check().is_ok());
        drop(next);
        assert!(!jobs.cancel("second").unwrap());
    }
}
