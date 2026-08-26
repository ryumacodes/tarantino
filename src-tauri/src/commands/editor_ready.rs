use anyhow::Result;
use tauri::Manager;

#[cfg(not(target_os = "linux"))]
use tauri::Emitter;

pub async fn notify_editor_ready(
    app: &tauri::AppHandle,
    final_path: &str,
    has_webcam: bool,
    has_mic: bool,
    has_system_audio: bool,
    webcam_shape: &str,
    webcam_x: f32,
    webcam_y: f32,
    webcam_size: f32,
) -> Result<()> {
    let Some(editor) = app.get_webview_window("editor") else {
        return Ok(());
    };

    #[cfg(target_os = "linux")]
    {
        let mut url = editor
            .url()
            .map_err(|error| anyhow::anyhow!("Failed to read editor URL: {error}"))?;
        url.set_path("/editor.html");
        url.set_query(Some(&format!(
            "media={}&webcam={}&mic={}&system_audio={}&webcam_shape={}&webcam_x={webcam_x:.4}&webcam_y={webcam_y:.4}&webcam_size={webcam_size:.4}",
            urlencoding::encode(final_path),
            has_webcam,
            has_mic,
            has_system_audio,
            urlencoding::encode(webcam_shape),
        )));
        editor
            .navigate(url)
            .map_err(|error| anyhow::anyhow!("Failed to load finalized recording: {error}"))?;
    }

    #[cfg(not(target_os = "linux"))]
    editor
        .emit(
            "recording-ready",
            serde_json::json!({
                "path": final_path,
                "has_webcam": has_webcam,
                "has_mic": has_mic,
                "has_system_audio": has_system_audio,
                "webcam_shape": webcam_shape,
                "webcam_x": webcam_x,
                "webcam_y": webcam_y,
                "webcam_size": webcam_size,
            }),
        )
        .map_err(|error| anyhow::anyhow!(error))?;

    Ok(())
}
