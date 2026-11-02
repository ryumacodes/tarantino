pub mod recordings;
pub mod relink;
pub mod portable;
pub mod picker;
pub mod copy_job;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{
    fs,
    io::Write,
    path::{Path, PathBuf},
};
use tauri::Manager;

const MAX_BYTES: u64 = 32 * 1024 * 1024;
#[derive(Serialize, Deserialize)]
pub struct ProjectDocument {
    format: String,
    version: u32,
    state: Value,
}
#[derive(Serialize)]
pub struct ProjectSummary {
    id: String,
    title: String,
    modified_ms: u64,
    media_path: String,
    duration_ms: f64,
}

fn validate(document: &ProjectDocument) -> Result<(), String> {
    if document.format != "tarantino-project" || document.version != 1 {
        return Err("Unsupported project format or version".into());
    }
    let state = document
        .state
        .as_object()
        .ok_or("Project state must be an object")?;
    if state
        .get("videoFilePath")
        .and_then(Value::as_str)
        .is_none_or(str::is_empty)
        || state.get("projectTitle").and_then(Value::as_str).is_none()
        || state
            .get("duration")
            .and_then(Value::as_f64)
            .is_none_or(|d| !d.is_finite() || d < 0.0)
    {
        return Err("Project is missing valid media metadata".into());
    }
    Ok(())
}
fn project_path(root: &Path, id: &str) -> Result<PathBuf, String> {
    let id = uuid::Uuid::parse_str(id).map_err(|_| "Invalid project identifier")?;
    Ok(root.join(format!("{id}.tarantino.json")))
}
fn read_project(path: &Path) -> Result<ProjectDocument, String> {
    if fs::metadata(path).map_err(|e| e.to_string())?.len() > MAX_BYTES {
        return Err("Project is too large".into());
    }
    let document = serde_json::from_slice(&fs::read(path).map_err(|e| e.to_string())?)
        .map_err(|e| format!("Invalid project: {e}"))?;
    validate(&document)?;
    Ok(document)
}
fn write_project(root: &Path, id: &str, document: &ProjectDocument) -> Result<(), String> {
    validate(document)?;
    let target = project_path(root, id)?;
    let bytes = serde_json::to_vec(document).map_err(|e| e.to_string())?;
    if bytes.len() as u64 > MAX_BYTES {
        return Err("Project is too large".into());
    }
    fs::create_dir_all(root).map_err(|e| e.to_string())?;
    let temporary = root.join(format!(".{}.tmp", uuid::Uuid::new_v4()));
    let result = (|| -> std::io::Result<()> {
        let mut file = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temporary)?;
        file.write_all(&bytes)?;
        file.sync_all()?;
        fs::rename(&temporary, target)?;
        #[cfg(unix)]
        fs::File::open(root)?.sync_all()?;
        Ok(())
    })();
    if result.is_err() {
        let _ = fs::remove_file(temporary);
    }
    result.map_err(|e| e.to_string())
}
fn root(app: &tauri::AppHandle) -> Result<PathBuf, String> {
    Ok(app
        .path()
        .app_data_dir()
        .map_err(|e| e.to_string())?
        .join("projects"))
}
#[tauri::command]
pub async fn save_project(
    app: tauri::AppHandle,
    project: ProjectDocument,
    project_id: Option<String>,
) -> Result<String, String> {
    let root = root(&app)?;
    let id = project_id.unwrap_or_else(|| uuid::Uuid::new_v4().to_string());
    tauri::async_runtime::spawn_blocking(move || {
        write_project(&root, &id, &project)?;
        Ok(id)
    })
    .await
    .map_err(|e| e.to_string())?
}
#[tauri::command]
pub async fn load_project(
    app: tauri::AppHandle,
    project_id: String,
) -> Result<ProjectDocument, String> {
    let path = project_path(&root(&app)?, &project_id)?;
    tauri::async_runtime::spawn_blocking(move || {
        let document = read_project(&path)?;
        let media = document.state["videoFilePath"]
            .as_str()
            .ok_or("Project has no recording")?;
        if !Path::new(media).is_file() {
            return Err(format!(
                "Recording is missing: {media}. Restore it to this location before reopening."
            ));
        }
        Ok(document)
    })
    .await
    .map_err(|e| e.to_string())?
}
#[tauri::command]
pub async fn list_saved_projects(app: tauri::AppHandle) -> Result<Vec<ProjectSummary>, String> {
    let root = root(&app)?;
    tauri::async_runtime::spawn_blocking(move || {
        if !root.exists() {
            return Ok(Vec::new());
        }
        let mut projects = Vec::new();
        for entry in fs::read_dir(root).map_err(|e| e.to_string())? {
            let entry = entry.map_err(|e| e.to_string())?;
            let name = entry.file_name().to_string_lossy().into_owned();
            let Some(id) = name.strip_suffix(".tarantino.json") else {
                continue;
            };
            if uuid::Uuid::parse_str(id).is_err() {
                continue;
            }
            if let Ok(document) = read_project(&entry.path()) {
                let modified_ms = entry
                    .metadata()
                    .ok()
                    .and_then(|m| m.modified().ok())
                    .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                    .map_or(0, |d| d.as_millis() as u64);
                projects.push(ProjectSummary {
                    id: id.into(),
                    title: document.state["projectTitle"]
                        .as_str()
                        .unwrap_or("Untitled")
                        .into(),
                    modified_ms,
                    media_path: document.state["videoFilePath"].as_str().unwrap_or_default().into(),
                    duration_ms: (document.state["trimEnd"].as_f64().unwrap_or_else(|| document.state["duration"].as_f64().unwrap_or(0.0))
                        - document.state["trimStart"].as_f64().unwrap_or(0.0)).max(0.0),
                });
            }
        }
        projects.sort_by_key(|p| std::cmp::Reverse(p.modified_ms));
        Ok(projects)
    })
    .await
    .map_err(|e| e.to_string())?
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn atomic_save_reopens_edits_and_rejects_invalid_replacements() {
        let root = std::env::temp_dir().join(format!("project-test-{}", uuid::Uuid::new_v4()));
        let id = uuid::Uuid::new_v4().to_string();
        let mut project = ProjectDocument {
            format: "tarantino-project".into(),
            version: 1,
            state: serde_json::json!({"videoFilePath":"/recording.mp4", "projectTitle":"Take One", "duration":3000,
                "clips":[{"sourceIn":1000,"playbackRate":2}], "visualSettings":{"screenMotionStyle":"smooth"}}),
        };
        write_project(&root, &id, &project).unwrap();
        project.state["projectTitle"] = Value::String("Take Two".into());
        write_project(&root, &id, &project).unwrap();
        assert_eq!(
            read_project(&project_path(&root, &id).unwrap())
                .unwrap()
                .state,
            project.state
        );
        project.version = 99;
        assert!(write_project(&root, &id, &project).is_err());
        assert_eq!(
            read_project(&project_path(&root, &id).unwrap())
                .unwrap()
                .version,
            1
        );
        assert!(project_path(&root, "../escape").is_err());
        assert_eq!(fs::read_dir(&root).unwrap().count(), 1);
        fs::remove_dir_all(root).unwrap();
    }
}

/// Open the local project library, preserving an already-open editor's edits.
pub async fn open_project_library(app: tauri::AppHandle) -> Result<(), String> {
    use tauri::Emitter;
    if let Some(editor) = app.get_webview_window("editor") {
        editor.unminimize().map_err(|e| e.to_string())?;
        editor.show().map_err(|e| e.to_string())?;
        editor.set_focus().map_err(|e| e.to_string())?;
        return editor
            .emit("project-library-open", ())
            .map_err(|e| e.to_string());
    }
    let editor = tauri::WebviewWindowBuilder::new(
        &app,
        "editor",
        tauri::WebviewUrl::App("editor.html?library=true".into()),
    )
    .title("Tarantino — Projects")
    .inner_size(1280.0, 840.0)
    .min_inner_size(1100.0, 720.0)
    .resizable(true)
    .build()
    .map_err(|e| e.to_string())?;
    editor.show().map_err(|e| e.to_string())?;
    editor.set_focus().map_err(|e| e.to_string())?;
    if let Some(bar) = app.get_webview_window("capture-bar") {
        let _ = bar.hide();
    }
    Ok(())
}
