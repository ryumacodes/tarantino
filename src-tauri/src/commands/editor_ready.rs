use anyhow::Result;
use tauri::Manager;


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
        // A loaded editor saves its current document before replacement. If the
        // shell has not mounted yet, there are no edits and navigation is safe.
        let destination = serde_json::to_string(url.as_str())?;
        editor.eval(format!(
            "if (window.dispatchEvent(new CustomEvent('tarantino-recording-handoff', {{detail:{destination},cancelable:true}}))) window.location.assign({destination});"
        )).map_err(|error| anyhow::anyhow!("Failed to open finalized recording: {error}"))?;
    }

    editor.set_title("Tarantino — Editor")?;
    editor.show()?;
    editor.set_focus()?;

    Ok(())
}
