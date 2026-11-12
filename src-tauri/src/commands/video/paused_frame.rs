#[tauri::command]
pub async fn extract_paused_video_frame(video_path: String, source_time_ms: f64, width: u32) -> Result<tauri::ipc::Response, String> {
    if !source_time_ms.is_finite() || source_time_ms < 0.0 || width == 0 || width > 8192 {
        return Err("Invalid preview frame request".into());
    }
    #[cfg(target_os = "macos")]
    {
        static DECODERS: tokio::sync::Semaphore = tokio::sync::Semaphore::const_new(1);
        let permit = DECODERS.acquire().await.map_err(|error| error.to_string())?;
        let image = tokio::task::spawn_blocking(move ||
            super::native_frame::extract(&video_path, source_time_ms, width, true))
            .await.map_err(|error| error.to_string())??;
        drop(permit);
        Ok(tauri::ipc::Response::new(image))
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = video_path;
        Err("Native paused frame recovery is unavailable on this platform".into())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[tokio::test]
    async fn rejects_invalid_preview_requests() {
        for (time, width) in [(f64::NAN, 96), (-1.0, 96), (0.0, 0), (0.0, 8193)] {
            assert!(extract_paused_video_frame("/missing.mp4".into(), time, width).await.is_err());
        }
    }
    #[cfg(target_os = "macos")]
    #[tokio::test]
    #[ignore = "requires an existing recording and host decoder access"]
    async fn recovers_lossless_held_frames() {
        let path = std::env::var("TARANTINO_HELD_FRAME_INPUT").unwrap();
        let times: Vec<f64> = std::env::var("TARANTINO_HELD_FRAME_TIMES").unwrap()
            .split(',').map(|time| time.parse().unwrap()).collect();
        let mut previous = None;
        for time in times {
            let data = extract_paused_video_frame(path.clone(), time, 96).await.unwrap();
            use tauri::ipc::{IpcResponse, InvokeResponseBody};
            let InvokeResponseBody::Raw(bytes) = data.body().unwrap() else { panic!("Expected binary frame"); };
            let image = image::load_from_memory(&bytes).unwrap().to_rgba8();
            assert_eq!(image.width(), 96);
            if let Some(previous) = previous { assert_eq!(image, previous); }
            previous = Some(image);
        }
    }
}
