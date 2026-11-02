//! Discover finalized local recordings that have not been saved as projects.
use super::{read_project, root};
use crate::recording::artifacts::RecordingArtifacts;
use serde::Serialize;
use std::{collections::HashSet, fs, path::{Path, PathBuf}};

#[derive(Serialize)]
pub struct RecordingSummary {
    path: String,
    title: String,
    modified_ms: u64,
    has_microphone: bool,
    has_system_audio: bool,
    has_webcam: bool,
}

fn identity(path: &Path) -> PathBuf { path.canonicalize().unwrap_or_else(|_| path.to_path_buf()) }

fn discover(projects: &Path, recordings: &Path) -> Result<Vec<RecordingSummary>, String> {
    let mut linked = HashSet::new();
    if projects.exists() {
        for entry in fs::read_dir(projects).map_err(|e| e.to_string())? {
            let entry = entry.map_err(|e| e.to_string())?;
            if !entry.file_name().to_string_lossy().ends_with(".tarantino.json") { continue; }
            if let Ok(project) = read_project(&entry.path()) {
                if let Some(path) = project.state["videoFilePath"].as_str() { linked.insert(identity(Path::new(path))); }
            }
        }
    }
    if !recordings.exists() { return Ok(Vec::new()); }
    let mut found = Vec::new();
    for entry in fs::read_dir(recordings).map_err(|e| e.to_string())? {
        let entry = entry.map_err(|e| e.to_string())?;
        let path = entry.path();
        let name = entry.file_name().to_string_lossy().into_owned();
        // Only the finalizer's names: never active Recording_*, exports or camera sidecars.
        let Some(stamp) = name.strip_prefix("recording_").and_then(|n| n.strip_suffix(".mp4")) else { continue; };
        if stamp.len() != 15 || !stamp.bytes().enumerate().all(|(i,b)| if i == 8 { b == b'_' } else { b.is_ascii_digit() }) { continue; }
        let metadata = entry.metadata().map_err(|e| e.to_string())?;
        if !metadata.is_file() || metadata.len() == 0 || linked.contains(&identity(&path)) { continue; }
        let artifacts = RecordingArtifacts::new(&path);
        let nonempty = |p:PathBuf| fs::metadata(p).is_ok_and(|m| m.is_file() && m.len()>0);
        found.push(RecordingSummary {
            path:path.to_string_lossy().into_owned(), title:format!("Recording {stamp}"),
            modified_ms:metadata.modified().ok().and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok()).map_or(0,|d|d.as_millis() as u64),
            has_microphone:nonempty(artifacts.microphone()), has_system_audio:nonempty(artifacts.system_audio()),
            has_webcam:nonempty(artifacts.webcam_mp4()) || nonempty(artifacts.webcam_webm()),
        });
    }
    found.sort_by_key(|r| std::cmp::Reverse(r.modified_ms));
    Ok(found)
}

#[tauri::command]
pub async fn list_unsaved_recordings(app:tauri::AppHandle) -> Result<Vec<RecordingSummary>,String> {
    let projects = root(&app)?;
    let recordings = PathBuf::from(std::env::var("HOME").map_err(|e|e.to_string())?).join("Movies/Tarantino");
    tauri::async_runtime::spawn_blocking(move || discover(&projects,&recordings)).await.map_err(|e|e.to_string())?
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn finds_unlinked_final_recordings_with_sidecars_and_ignores_working_files() {
        let root=std::env::temp_dir().join(format!("recording-discovery-{}",uuid::Uuid::new_v4()));
        let projects=root.join("projects"); let recordings=root.join("recordings");
        fs::create_dir_all(&projects).unwrap();fs::create_dir_all(&recordings).unwrap();
        for name in ["recording_20261027_010000.mp4","recording_20261027_020000.mp4","Recording_20261027_030000.mp4","processed_recording_20261027_010000.mp4","recording_20261027_020000.webcam.mp4"] {
            fs::write(recordings.join(name),b"fixture").unwrap();
        }
        fs::write(recordings.join("recording_20261027_040000.mp4"),b"").unwrap();
        let linked=recordings.join("recording_20261027_010000.mp4");
        fs::write(projects.join("valid.tarantino.json"),serde_json::to_vec(&serde_json::json!({
            "format":"tarantino-project","version":1,"state":{"videoFilePath":linked,"projectTitle":"Saved","duration":1000}
        })).unwrap()).unwrap();
        fs::write(projects.join("broken.tarantino.json"),b"invalid").unwrap();
        fs::write(recordings.join("recording_20261027_020000.mic.wav"),b"audio").unwrap();
        let found=discover(&projects,&recordings).unwrap();
        assert_eq!(found.len(),1);
        assert!(found[0].path.ends_with("recording_20261027_020000.mp4"));
        assert!(found[0].has_microphone && found[0].has_webcam);
        assert!(!found[0].has_system_audio);
        assert!(discover(&projects,&root.join("missing")).unwrap().is_empty());
        fs::remove_dir_all(root).unwrap();
    }
}
