//! Recover projects whose primary recording has moved.
use super::{project_path, read_project, root, write_project, ProjectDocument};
use crate::{recording::artifacts::RecordingArtifacts, video_processing::{VideoInfo, VideoProcessor}};
use serde_json::Value;
use std::path::Path;

fn validate_replacement(document: &ProjectDocument, path: &Path, info: &VideoInfo) -> Result<(), String> {
    let state = &document.state;
    let duration = state["duration"].as_f64().ok_or("Project has no duration")?;
    if (info.duration_ms as f64-duration).abs() > 100.0 {
        return Err("Recording duration does not match this project. Select the original recording.".into());
    }
    for (key, actual) in [("videoWidth",info.width),("videoHeight",info.height)] {
        if let Some(expected) = state[key].as_f64() {
            if expected > 0.0 && expected != actual as f64 {
                return Err("Recording dimensions do not match this project.".into());
            }
        }
    }
    let files = RecordingArtifacts::new(path);
    let nonempty = |path: &Path| path.metadata().is_ok_and(|m| m.is_file() && m.len()>0);
    for (key, file) in [("hasMicrophone",files.microphone()),("hasSystemAudio",files.system_audio())] {
        if state[key].as_bool()==Some(true) && !nonempty(&file) {
            return Err(format!("Move the companion recording file alongside the video: {}",file.display()));
        }
    }
    if state["captureMode"].as_str().is_some() && !nonempty(&files.mouse()) {
        return Err("Move the companion mouse recording alongside the video before relinking.".into());
    }
    if state["hasWebcam"].as_bool()==Some(true) && !nonempty(&files.webcam_mp4()) && !nonempty(&files.webcam_webm()) {
        return Err("Move the companion camera recording alongside the video before relinking.".into());
    }
    Ok(())
}

fn replace_primary_path(document: &mut ProjectDocument, old: &str, new: &str) {
    document.state["videoFilePath"] = Value::String(new.into());
    fn clips(value: &mut Value, old: &str, new: &str) {
        if let Some(clips) = value.as_array_mut() {
            for clip in clips {
                if clip["sourceFilePath"].as_str()==Some(old) {
                    clip["sourceFilePath"]=Value::String(new.into());
                }
            }
        }
    }
    clips(&mut document.state["clips"],old,new);
    for key in ["transcripts", "masks"] {
        if let Some(items)=document.state.get_mut(key) { clips(items,old,new); }
    }
    if let Some(tracks)=document.state["tracks"].as_array_mut() {
        for track in tracks { clips(&mut track["clips"],old,new); }
    }
}

#[tauri::command]
pub async fn relink_project_recording(app: tauri::AppHandle, project_id: String, recording_path: String) -> Result<(), String> {
    let root=root(&app)?;
    relink_recording(&root,&project_id,&recording_path).await
}

async fn relink_recording(root: &Path, project_id: &str, recording_path: &str) -> Result<(), String> {
    let path=project_path(root,project_id)?;
    let mut document=read_project(&path)?;
    let original=serde_json::to_vec(&document).map_err(|e|e.to_string())?;
    let old=document.state["videoFilePath"].as_str().ok_or("Project has no recording")?.to_string();
    if Path::new(&old).is_file() { return Err("The original recording is available. Reopen the project instead.".into()); }
    let replacement=Path::new(&recording_path).canonicalize().map_err(|e|format!("Cannot open replacement recording: {e}"))?;
    if !replacement.is_file() { return Err("Select a recording file.".into()); }
    let info=VideoProcessor::new().map_err(|e|e.to_string())?.get_video_info(&replacement).await.map_err(|e|e.to_string())?;
    validate_replacement(&document,&replacement,&info)?;
    let current=read_project(&path)?;
    if serde_json::to_vec(&current).map_err(|e|e.to_string())? != original {
        return Err("The project changed while checking the recording. Please try again.".into());
    }
    replace_primary_path(&mut document,&old,&replacement.to_string_lossy());
    write_project(root,project_id,&document)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    fn document() -> ProjectDocument {
        ProjectDocument {format:"tarantino-project".into(),version:1,state:json!({
            "videoFilePath":"/old/take.mp4","projectTitle":"Take","duration":5000,
            "videoWidth":1280,"videoHeight":720,"trimStart":1000,"trimEnd":4000,
            "transcripts":[{"sourceFilePath":"/old/take.mp4"},{"sourceFilePath":"/other.mp4"}],
            "masks":[{"sourceFilePath":"/old/take.mp4","sourceStartMs":1000},{"sourceFilePath":"/other.mp4"}],
            "clips":[{"sourceFilePath":"/old/take.mp4","playbackRate":2},{"sourceFilePath":"/other.mp4"}],
            "tracks":[{"clips":[{"sourceFilePath":"/old/take.mp4"}]}]
        })}
    }
    fn info() -> VideoInfo { VideoInfo {duration_ms:5000,width:1280,height:720,fps:30.,bitrate:0,format:"mp4".into(),size_bytes:1,frame_count:Some(150)} }
    #[test]
    fn relink_preserves_edits_and_other_sources_through_atomic_save() {
        let root=std::env::temp_dir().join(format!("relink-{}",uuid::Uuid::new_v4()));
        let id=uuid::Uuid::new_v4().to_string();
        let mut doc=document();
        replace_primary_path(&mut doc,"/old/take.mp4","/new/take.mp4");
        write_project(&root,&id,&doc).unwrap();
        let loaded=read_project(&project_path(&root,&id).unwrap()).unwrap();
        assert_eq!(loaded.state["videoFilePath"],"/new/take.mp4");
        assert_eq!(loaded.state["transcripts"][0]["sourceFilePath"],"/new/take.mp4");
        assert_eq!(loaded.state["transcripts"][1]["sourceFilePath"],"/other.mp4");
        assert_eq!(loaded.state["clips"][0]["sourceFilePath"],"/new/take.mp4");
        assert_eq!(loaded.state["tracks"][0]["clips"][0]["sourceFilePath"],"/new/take.mp4");
        assert_eq!(loaded.state["clips"][1]["sourceFilePath"],"/other.mp4");
        assert_eq!(loaded.state["clips"][0]["playbackRate"],2);
        assert_eq!(loaded.state["trimStart"],1000);
        assert_eq!(loaded.state["masks"][0]["sourceFilePath"],"/new/take.mp4");
        assert_eq!(loaded.state["masks"][1]["sourceFilePath"],"/other.mp4");
        assert_eq!(loaded.state["masks"][0]["sourceStartMs"],1000);
        std::fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn legacy_relink_does_not_add_null_transcripts() {
        let mut doc=document();doc.state.as_object_mut().unwrap().remove("transcripts");
        doc.state.as_object_mut().unwrap().remove("masks");
        replace_primary_path(&mut doc,"/old/take.mp4","/new/take.mp4");
        assert!(doc.state.get("transcripts").is_none());
        assert!(doc.state.get("masks").is_none());
    }
    #[test]
    fn rejects_mismatched_media_and_missing_companion_audio() {
        let mut doc=document();let mut metadata=info();let path=Path::new("/missing/take.mp4");
        assert!(validate_replacement(&doc,path,&metadata).is_ok());
        metadata.duration_ms=5500;assert!(validate_replacement(&doc,path,&metadata).is_err());
        metadata=info();metadata.width=1920;assert!(validate_replacement(&doc,path,&metadata).is_err());
        metadata=info();doc.state["hasMicrophone"]=json!(true);
        assert!(validate_replacement(&doc,path,&metadata).unwrap_err().contains("companion"));
        doc.state["hasMicrophone"]=json!(false);doc.state["hasWebcam"]=json!(true);
        assert!(validate_replacement(&doc,path,&metadata).unwrap_err().contains("camera"));
    }
    #[tokio::test]
    #[ignore = "requires FFmpeg and FFprobe"]
    async fn relink_moved_recording_with_real_media_probe() {
        let root=std::env::temp_dir().join(format!("relink-media-{}",uuid::Uuid::new_v4()));
        std::fs::create_dir_all(root.join("moved")).unwrap();
        let old=root.join("take.mp4");let moved=root.join("moved/take.mp4");
        let status=std::process::Command::new(crate::ffmpeg::binary("ffmpeg")).args(["-v","error","-f","lavfi","-i",
            "color=black:s=1280x720:r=30:d=5","-c:v","libx264","-pix_fmt","yuv420p","-y"])
            .arg(&old).status().unwrap();
        assert!(status.success());
        let id=uuid::Uuid::new_v4().to_string();let mut doc=document();
        replace_primary_path(&mut doc,"/old/take.mp4",&old.to_string_lossy());
        write_project(&root,&id,&doc).unwrap();
        assert!(relink_recording(&root,&id,&moved.to_string_lossy()).await.is_err());
        std::fs::rename(&old,&moved).unwrap();
        relink_recording(&root,&id,&moved.to_string_lossy()).await.unwrap();
        let restored=read_project(&project_path(&root,&id).unwrap()).unwrap();
        assert_eq!(restored.state["videoFilePath"],moved.canonicalize().unwrap().to_string_lossy().as_ref());
        assert_eq!(restored.state["trimEnd"],4000);
        std::fs::remove_dir_all(root).unwrap();
    }

}
