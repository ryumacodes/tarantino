//! Auto-zoom commands

use crate::auto_zoom::{self, ZoomAnalysis};

#[tauri::command]
pub async fn load_auto_zoom_data(video_path: String) -> Result<Option<ZoomAnalysis>, String> {
    println!("📂 [LOAD_ZOOM] load_auto_zoom_data called");
    println!("📂 [LOAD_ZOOM] Video path: {}", video_path);

    let auto_zoom_path = format!("{}.auto_zoom.json", video_path.trim_end_matches(".mp4"));
    println!("📂 [LOAD_ZOOM] Looking for auto_zoom file at: {}", auto_zoom_path);

    // Also check if mouse.json exists for debugging
    let mouse_path = format!("{}.mouse.json", video_path.trim_end_matches(".mp4"));
    println!("📂 [LOAD_ZOOM] Mouse file would be at: {}", mouse_path);
    println!("   - auto_zoom.json exists: {}", std::path::Path::new(&auto_zoom_path).exists());
    println!("   - mouse.json exists: {}", std::path::Path::new(&mouse_path).exists());

    if !std::path::Path::new(&auto_zoom_path).exists() {
        println!("⚠️ [LOAD_ZOOM] No auto-zoom file found!");
        return Ok(None);
    }

    match auto_zoom::load_analysis(&auto_zoom_path) {
        Ok(analysis) => {
            println!("✅ [LOAD_ZOOM] Successfully loaded zoom data:");
            println!("   - zoom_blocks: {}", analysis.zoom_blocks.len());
            println!("   - total_clicks: {}", analysis.total_clicks);
            println!("   - session_duration: {}ms", analysis.session_duration);
            for (i, block) in analysis.zoom_blocks.iter().enumerate() {
                println!("   - Block {}: {}ms-{}ms, {:.1}x at ({:.2}, {:.2})",
                    i, block.start_time, block.end_time, block.zoom_factor, block.center_x, block.center_y);
            }
            Ok(Some(analysis))
        }
        Err(e) => {
            println!("❌ [LOAD_ZOOM] Failed to load auto-zoom data: {}", e);
            Err(format!("Failed to load auto-zoom data: {}", e))
        }
    }
}

#[tauri::command]
pub async fn save_auto_zoom_data(video_path: String, analysis: ZoomAnalysis) -> Result<(), String> {
    println!("Saving auto-zoom data for video: {}", video_path);

    let auto_zoom_path = format!("{}.auto_zoom.json", video_path.trim_end_matches(".mp4"));

    auto_zoom::save_analysis(&analysis, &auto_zoom_path)
        .map_err(|e| format!("Failed to save auto-zoom data: {}", e))?;

    println!("Successfully saved auto-zoom data to: {}", auto_zoom_path);
    Ok(())
}
