//! Shared decoded image geometry for the editor's cursor texture cache.
use base64::{engine::general_purpose::STANDARD, Engine};
use serde::Serialize;

#[derive(Serialize)]
pub struct Image {
    width: u32,
    height: u32,
    rgba_base64: String,
    rect: [f64;4],
}

#[derive(Serialize)]
pub struct Images {
    images: Vec<Option<Image>>,
    fallback_image_id: Option<usize>,
    changes: Vec<crate::input::cursor_images::CursorChange>,
}

#[tauri::command]
pub async fn load_cursor_images(mouse_json_path: String, optimize_cursor_types: Option<bool>, always_use_pointer: Option<bool>, style: Option<String>) -> Result<Images,String> {
    tauri::async_runtime::spawn_blocking(move || {
        let track = crate::cursor_renderer::recorded_images::Track::load_for_style(std::path::Path::new(&mouse_json_path),optimize_cursor_types.unwrap_or(true),always_use_pointer.unwrap_or(false),style.as_deref().unwrap_or("pointer"))
            .map_err(|error|error.to_string())?;
        let images=track.images.into_iter().map(|image|image.map(|image|Image {
            width:image.pixels.width(),height:image.pixels.height(),
            rgba_base64:STANDARD.encode(image.pixels.as_raw()),rect:image.geometry.rect([0.0;2],17.0),
        })).collect();
        Ok(Images { images,changes:track.changes,fallback_image_id:track.fallback })
    }).await.map_err(|error|error.to_string())?
}
