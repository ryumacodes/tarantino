//! Self-contained project folders with relative asset references.
use super::copy_job::{CopyControl, CopyJob, CopyProgress};
use super::{read_project, root, validate, write_project, ProjectDocument};
use crate::recording::artifacts::RecordingArtifacts;
use serde_json::Value;
use std::{collections::HashMap, fs, path::{Component, Path, PathBuf}};

const MANIFEST: &str = "project.tarantino.json";

struct Copier<'a> {
    destination: &'a Path,
    control: &'a CopyControl,
    source_root: Option<&'a Path>,
    relative: bool,
    copied: HashMap<String, String>,
}
impl Copier<'_> {
    fn source(&self, value: &str) -> Result<PathBuf, String> {
        let path = Path::new(value);
        let source = if let Some(root) = self.source_root {
            if value.contains(['\\', ':']) || value.split('/').any(|part| part.is_empty() || part == "." || part == "..")
                || !path.components().all(|part| matches!(part, Component::Normal(_))) {
                return Err("Package asset paths must be relative and stay inside the package.".into());
            }
            let source = root.join(path).canonicalize().map_err(|e| format!("Missing package asset: {value}: {e}"))?;
            if !source.starts_with(root) { return Err("Package asset points outside the package.".into()); }
            source
        } else {
            path.canonicalize().map_err(|e| format!("Missing project asset: {value}: {e}"))?
        };
        if !source.is_file() { return Err(format!("Project asset is not a file: {value}")); }
        Ok(source)
    }
    fn copy_file(&self, source: &Path, destination: &Path) -> Result<(), String> {
        // Companion files must also be contained: a symlink must not escape a package.
        if let Some(root) = self.source_root {
            let real = source.canonicalize().map_err(|e| e.to_string())?;
            if !real.starts_with(root) { return Err("Package companion points outside the package.".into()); }
        }
        let before = fs::metadata(source).map_err(|e| e.to_string())?;
        if !before.is_file() { return Err("A project asset is not a regular file.".into()); }
        self.control.copy(source, destination)?;
        let after = fs::metadata(source).map_err(|e| e.to_string())?;
        if before.len() != after.len() || before.modified().ok() != after.modified().ok() {
            return Err("A recording changed while being copied. Stop recording and try again.".into());
        }
        Ok(())
    }
    fn asset(&mut self, value: &mut Value, recording: bool) -> Result<(), String> {
        let Some(original) = value.as_str().map(str::to_owned) else { return Ok(()); };
        if !recording && original.starts_with("data:") { return Ok(()); }
        if let Some(copied) = self.copied.get(&original) { *value = Value::String(copied.clone()); return Ok(()); }
        let source = self.source(&original)?;
        let folder = PathBuf::from("media").join(uuid::Uuid::new_v4().to_string());
        fs::create_dir_all(self.destination.join(&folder)).map_err(|e| e.to_string())?;
        // File names must be valid on every supported OS, independent of the source name.
        let extension = source.extension().and_then(|value| value.to_str())
            .filter(|value| !value.is_empty() && value.len() <= 16 && value.bytes().all(|byte| byte.is_ascii_alphanumeric()))
            .unwrap_or("bin");
        let relative = folder.join(format!("asset.{extension}"));
        let target = self.destination.join(&relative);
        self.copy_file(&source, &target)?;
        if recording {
            for (sidecar, destination) in RecordingArtifacts::new(&source).sidecar_pairs(&RecordingArtifacts::new(&target)) {
                if sidecar.try_exists().map_err(|e| e.to_string())? { self.copy_file(&sidecar, &destination)?; }
            }
        }
        let stored = if self.relative {
            relative.components().map(|part| part.as_os_str().to_string_lossy()).collect::<Vec<_>>().join("/")
        } else { target.to_string_lossy().into_owned() };
        self.copied.insert(original, stored.clone());
        *value = Value::String(stored);
        Ok(())
    }
    fn project(&mut self, document: &mut ProjectDocument) -> Result<(), String> {
        let state = &mut document.state;
        let primary = state["videoFilePath"].as_str().ok_or("Missing recording")?;
        let source = self.source(primary)?;
        let files = RecordingArtifacts::new(source);
        for (key, path) in [("hasMicrophone",files.microphone()),("hasSystemAudio",files.system_audio())] {
            if state[key].as_bool()==Some(true) && !path.is_file() { return Err(format!("Missing companion: {}",path.display())); }
        }
        if state["hasWebcam"].as_bool()==Some(true) && !files.webcam_mp4().is_file() && !files.webcam_webm().is_file() {
            return Err("Missing camera recording.".into());
        }
        if state["captureMode"].as_str().is_some() && !files.mouse().is_file() {
            return Err("Missing mouse recording. Restore the companion file before packaging.".into());
        }
        self.asset(&mut state["videoFilePath"], true)?;
        for key in ["transcripts", "masks"] {
            if let Some(items) = state.get_mut(key).and_then(Value::as_array_mut) {
                for item in items { if let Some(path) = item.get_mut("sourceFilePath") { self.asset(path, true)?; } }
            }
        }
        if let Some(clips) = state.get_mut("clips").and_then(Value::as_array_mut) {
            for clip in clips { if let Some(path) = clip.get_mut("sourceFilePath") { self.asset(path, true)?; } }
        }
        if let Some(tracks) = state.get_mut("tracks").and_then(Value::as_array_mut) {
            for track in tracks {
                if let Some(clips) = track.get_mut("clips").and_then(Value::as_array_mut) {
                    for clip in clips { if let Some(path) = clip.get_mut("sourceFilePath") { self.asset(path, true)?; } }
                }
            }
        }
        if let Some(path) = state.pointer_mut("/audioSettings/backgroundAudio/path") { self.asset(path, false)?; }
        if let Some(path) = state.pointer_mut("/visualSettings/customBackgroundImage") { self.asset(path, false)?; }
        Ok(())
    }
}

fn export_with_control(mut document: ProjectDocument, destination: &Path, control: &CopyControl) -> Result<(), String> {
    control.check()?;
    validate(&document)?;
    // Reserve a fresh folder. Never merge with or replace existing user data.
    fs::create_dir(destination).map_err(|e| format!("Choose a new package folder: {e}"))?;
    let result = (|| {
        Copier { destination, control, source_root: None, relative: true, copied: HashMap::new() }.project(&mut document)?;
        control.check()?;
        let bytes = serde_json::to_vec(&document).map_err(|e| e.to_string())?;
        if bytes.len() as u64 > super::MAX_BYTES { return Err("Project is too large".into()); }
        fs::write(destination.join(MANIFEST), bytes).map_err(|e| e.to_string())?;
        fs::File::open(destination.join(MANIFEST)).and_then(|file| file.sync_all()).map_err(|e| e.to_string())
    })();
    if result.is_err() { let _ = fs::remove_dir_all(destination); }
    result
}
fn import_with_control(projects: &Path, package: &Path, control: &CopyControl) -> Result<String, String> {
    control.check()?;
    let package = package.canonicalize().map_err(|e| e.to_string())?;
    let manifest = package.join(MANIFEST).canonicalize().map_err(|e| e.to_string())?;
    if !manifest.starts_with(&package) { return Err("Project manifest points outside the package.".into()); }
    let mut document = read_project(&manifest)?;
    let id = uuid::Uuid::new_v4().to_string();
    let owned = projects.join("media").join(&id);
    fs::create_dir_all(&owned).map_err(|e| e.to_string())?;
    let result = (|| {
        Copier { destination: &owned, control, source_root: Some(&package), relative: false, copied: HashMap::new() }.project(&mut document)?;
        control.check()?;
        write_project(projects, &id, &document)?;
        Ok(id)
    })();
    if result.is_err() { let _ = fs::remove_dir_all(owned); }
    result
}
#[tauri::command]
pub async fn export_project_package(window:tauri::WebviewWindow, project: ProjectDocument, destination: String,
    operation_id:String, progress:tauri::ipc::Channel<CopyProgress>) -> Result<(), String> {
    let job=CopyJob::start(operation_id,window.label().to_string(),progress)?;
    tauri::async_runtime::spawn_blocking(move || export_with_control(project, Path::new(&destination), &job.control)).await.map_err(|e| e.to_string())?
}
#[tauri::command]
pub async fn import_project_package(app: tauri::AppHandle, window:tauri::WebviewWindow, package_path: String,
    operation_id:String, progress:tauri::ipc::Channel<CopyProgress>) -> Result<String, String> {
    let projects = root(&app)?;
    let job=CopyJob::start(operation_id,window.label().to_string(),progress)?;
    tauri::async_runtime::spawn_blocking(move || import_with_control(&projects, Path::new(&package_path), &job.control)).await.map_err(|e| e.to_string())?
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    fn export(document:ProjectDocument,path:&Path)->Result<(),String> {export_with_control(document,path,&CopyControl::default())}
    fn import(projects:&Path,path:&Path)->Result<String,String> {import_with_control(projects,path,&CopyControl::default())}
    fn fixture(root: &Path) -> ProjectDocument {
        fs::create_dir_all(root).unwrap();
        for name in ["take.mp4","take.mouse.json","take.mic.wav","take.window-mask.png","music.wav","background.png"] {
            fs::write(root.join(name), name.as_bytes()).unwrap();
        }
        ProjectDocument {format:"tarantino-project".into(),version:1,state:json!({
            "videoFilePath":root.join("take.mp4"),"projectTitle":"Edited take","duration":5000,
            "trimStart":500,"trimEnd":3500,"hasMicrophone":true,"captureMode":"window",
            "transcripts":[{"sourceFilePath":root.join("take.mp4"),"words":[{"text":"hello"}]}],
            "masks":[{"sourceFilePath":root.join("take.mp4"),"sourceStartMs":1000,"bounds":[{"x":10,"y":20,"width":100,"height":50}]}],
            "clips":[{"sourceFilePath":root.join("take.mp4"),"sourceIn":1000,"playbackRate":2}],
            "tracks":[{"clips":[{"sourceFilePath":root.join("take.mp4")}]}],
            "audioSettings":{"backgroundAudio":{"path":root.join("music.wav")}},
            "visualSettings":{"customBackgroundImage":root.join("background.png")},
            "zoomKeyframes":[{"time":500,"scale":2}]
        })}
    }
    #[test]
    fn package_survives_move_and_removal_of_original_and_preserves_edits() {
        let root=std::env::temp_dir().join(format!("portable-{}",uuid::Uuid::new_v4()));
        let original=fixture(&root.join("original"));
        export(original,&root.join("package")).unwrap();
        let packed=read_project(&root.join("package").join(MANIFEST)).unwrap();
        let packed_video = packed.state["videoFilePath"].as_str().unwrap();
        assert!(!Path::new(packed_video).is_absolute());
        assert!(!packed_video.contains('\\'));
        assert!(packed_video.ends_with("/asset.mp4"));
        fs::rename(root.join("package"),root.join("moved")).unwrap();
        fs::remove_dir_all(root.join("original")).unwrap();
        let projects=root.join("projects");
        let id=import(&projects,&root.join("moved")).unwrap();
        fs::remove_dir_all(root.join("moved")).unwrap();
        let restored=read_project(&super::super::project_path(&projects,&id).unwrap()).unwrap();
        let video=restored.state["videoFilePath"].as_str().unwrap();
        assert_eq!(fs::read(video).unwrap(),b"take.mp4");
        assert_eq!(fs::read(RecordingArtifacts::new(video).mouse()).unwrap(),b"take.mouse.json");
        assert_eq!(fs::read(RecordingArtifacts::new(video).window_mask()).unwrap(),b"take.window-mask.png");
        assert_eq!(restored.state["masks"][0]["sourceFilePath"],video);
        assert_eq!(restored.state["masks"][0]["sourceStartMs"],1000);
        assert_eq!(restored.state["masks"][0]["bounds"][0]["width"],100);
        assert_eq!(restored.state["transcripts"][0]["sourceFilePath"],video);
        assert_eq!(restored.state["transcripts"][0]["words"][0]["text"],"hello");
        assert_eq!(restored.state["clips"][0]["sourceFilePath"],video);
        assert_eq!(restored.state["tracks"][0]["clips"][0]["sourceFilePath"],video);
        assert_eq!(restored.state["clips"][0]["playbackRate"],2);
        assert_eq!(restored.state["trimEnd"],3500);
        assert_eq!(restored.state["zoomKeyframes"][0]["scale"],2);
        for pointer in ["/audioSettings/backgroundAudio/path","/visualSettings/customBackgroundImage"] {
            assert!(Path::new(restored.state.pointer(pointer).unwrap().as_str().unwrap()).is_file());
        }
        fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn failures_preserve_existing_folders_and_clean_partial_copies() {
        let root=std::env::temp_dir().join(format!("portable-{}",uuid::Uuid::new_v4()));
        let doc=fixture(&root.join("original"));
        fs::create_dir(root.join("existing")).unwrap();
        fs::write(root.join("existing/keep"),b"keep").unwrap();
        assert!(export(doc,&root.join("existing")).is_err());
        assert_eq!(fs::read(root.join("existing/keep")).unwrap(),b"keep");
        let doc=fixture(&root.join("original"));
        fs::remove_file(root.join("original/music.wav")).unwrap();
        assert!(export(doc,&root.join("failed")).is_err());
        assert!(!root.join("failed").exists());
        let doc=fixture(&root.join("original"));
        export(doc,&root.join("package")).unwrap();
        let manifest=root.join("package").join(MANIFEST);
        let mut bad=read_project(&manifest).unwrap();
        bad.state["videoFilePath"]=json!("../original/take.mp4");
        fs::write(manifest,serde_json::to_vec(&bad).unwrap()).unwrap();
        assert!(import(&root.join("projects"),&root.join("package")).unwrap_err().contains("relative"));
        assert_eq!(fs::read_dir(root.join("projects/media")).unwrap().count(),0);
        fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn rejects_platform_specific_or_ambiguous_package_paths() {
        let root=std::env::temp_dir();
        let control=CopyControl::default();
        let copier=Copier { destination:&root, control:&control, source_root:Some(&root), relative:true, copied:HashMap::new() };
        for path in ["C:/take.mp4", "media\\take.mp4", "media/../take.mp4", "media/./take.mp4", "media//take.mp4", "/take.mp4"] {
            assert!(copier.source(path).unwrap_err().contains("relative"), "{path}");
        }
    }
    #[test]
    fn cancellation_rolls_back_partial_export_and_import() {
        let root=std::env::temp_dir().join(format!("portable-{}",uuid::Uuid::new_v4()));
        let document=fixture(&root.join("original"));
        let cancelled=CopyControl::cancel_on_first_progress();
        assert!(export_with_control(document,&root.join("cancelled"),&cancelled).unwrap_err().contains("cancelled"));
        assert!(!root.join("cancelled").exists());
        export(fixture(&root.join("original")),&root.join("package")).unwrap();
        let cancelled=CopyControl::cancel_on_first_progress();
        assert!(import_with_control(&root.join("projects"),&root.join("package"),&cancelled).unwrap_err().contains("cancelled"));
        assert_eq!(fs::read_dir(root.join("projects/media")).unwrap().count(),0);
        assert_eq!(fs::read_dir(root.join("projects")).unwrap().count(),1);
        assert!(root.join("package").join(MANIFEST).is_file());
        fs::remove_dir_all(root).unwrap();
    }
    #[cfg(unix)]
    #[test]
    fn rejects_companion_symlinks_outside_package() {
        let root=std::env::temp_dir().join(format!("portable-{}",uuid::Uuid::new_v4()));
        let doc=fixture(&root.join("original"));
        export(doc,&root.join("package")).unwrap();
        let packed=read_project(&root.join("package").join(MANIFEST)).unwrap();
        let mouse=RecordingArtifacts::new(root.join("package").join(packed.state["videoFilePath"].as_str().unwrap())).mouse();
        fs::remove_file(&mouse).unwrap();
        std::os::unix::fs::symlink(root.join("original/take.mouse.json"),mouse).unwrap();
        assert!(import(&root.join("projects"),&root.join("package")).unwrap_err().contains("outside"));
        fs::remove_dir_all(root).unwrap();
    }
}
