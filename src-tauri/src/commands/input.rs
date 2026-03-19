//! Input configuration commands (camera, mic, system audio, webcam)

use std::sync::Arc;
use tauri::State;
use serde::Deserialize;
use crate::state::UnifiedAppState;

#[tauri::command]
pub async fn input_set_camera(enabled: bool, device_id: Option<String>, shape: String, state: State<'_, Arc<UnifiedAppState>>) -> Result<(), String> {
    state.set_camera_input(enabled, device_id, shape).await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn input_set_mic(enabled: bool, device_id: Option<String>, state: State<'_, Arc<UnifiedAppState>>) -> Result<(), String> {
    state.set_mic_input(enabled, device_id).await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn input_set_system_audio(enabled: bool, source_id: Option<String>, state: State<'_, Arc<UnifiedAppState>>) -> Result<(), String> {
    state.set_system_audio(enabled, source_id).await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn webcam_set_transform(x_norm: f32, y_norm: f32, size_norm: f32, shape: String, state: State<'_, Arc<UnifiedAppState>>) -> Result<(), String> {
    state.set_webcam_transform(x_norm, y_norm, size_norm, shape).await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn webcam_set_autododge(enabled: bool, radius_norm: f32, strength: f32, state: State<'_, Arc<UnifiedAppState>>) -> Result<(), String> {
    state.set_webcam_autododge(enabled, radius_norm, strength).await.map_err(|e| e.to_string())
}

#[derive(Deserialize)]
pub struct WebcamPosition { pub x: f32, pub y: f32 }

#[tauri::command]
pub async fn save_webcam_recording(data: Vec<u8>, position: WebcamPosition, size: f32, shape: String, state: State<'_, Arc<UnifiedAppState>>) -> Result<(), String> {
    use std::fs;
    use std::io::Write;

    let sidecar_path = state.get_current_sidecar_path()
        .ok_or_else(|| "No active recording session".to_string())?;

    let webcam_path = std::path::Path::new(&sidecar_path).join("webcam.webm");
    let mut file = fs::File::create(&webcam_path).map_err(|e| format!("Failed to create webcam file: {}", e))?;
    file.write_all(&data).map_err(|e| format!("Failed to write webcam data: {}", e))?;

    let metadata = serde_json::json!({ "position": { "x": position.x, "y": position.y }, "size": size, "shape": shape });
    let metadata_path = std::path::Path::new(&sidecar_path).join("webcam.json");
    fs::write(&metadata_path, serde_json::to_string_pretty(&metadata).unwrap())
        .map_err(|e| format!("Failed to write webcam metadata: {}", e))?;

    println!("Webcam recording saved: {} ({} bytes)", webcam_path.display(), data.len());
    Ok(())
}
