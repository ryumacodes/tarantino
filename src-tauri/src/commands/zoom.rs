//! Auto-zoom commands

use crate::auto_zoom::{self, ZoomAnalysis};

#[tauri::command]
pub async fn load_auto_zoom_data(video_path: String) -> Result<Option<ZoomAnalysis<f64>>, String> {
    println!("📂 [LOAD_ZOOM] load_auto_zoom_data called");
    println!("📂 [LOAD_ZOOM] Video path: {}", video_path);

    let artifacts = crate::recording::artifacts::RecordingArtifacts::new(&video_path);
    let auto_zoom_path = artifacts.auto_zoom();
    println!(
        "📂 [LOAD_ZOOM] Looking for auto_zoom file at: {}",
        auto_zoom_path.display()
    );

    // Also check if mouse.json exists for debugging
    let mouse_path = artifacts.mouse();
    println!(
        "📂 [LOAD_ZOOM] Mouse file would be at: {}",
        mouse_path.display()
    );
    println!("   - auto_zoom.json exists: {}", auto_zoom_path.exists());
    println!("   - mouse.json exists: {}", mouse_path.exists());

    if !auto_zoom_path.exists() {
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
                println!(
                    "   - Block {}: {}ms-{}ms, {:.1}x at ({:.2}, {:.2})",
                    i,
                    block.start_time,
                    block.end_time,
                    block.zoom_factor,
                    block.center_x,
                    block.center_y
                );
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
pub async fn save_auto_zoom_data(video_path: String, analysis: ZoomAnalysis<f64>) -> Result<(), String> {
    println!("Saving auto-zoom data for video: {}", video_path);

    let auto_zoom_path =
        crate::recording::artifacts::RecordingArtifacts::new(&video_path).auto_zoom();

    auto_zoom::validate_saved_times(&analysis).map_err(|error| error.to_string())?;
    auto_zoom::save_analysis(&analysis, &auto_zoom_path)
        .map_err(|e| format!("Failed to save auto-zoom data: {}", e))?;

    println!(
        "Successfully saved auto-zoom data to: {}",
        auto_zoom_path.display()
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn edited_sidecars_keep_fractional_times_through_commands_and_export_loading() {
        let directory=std::env::temp_dir().join(format!("zoom-sidecar-{}",uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&directory).unwrap();
        let video=directory.join("recording.mp4");
        let path=video.to_string_lossy().into_owned();
        let value=serde_json::json!({
            "zoom_blocks":[{"id":"typing","click_x":0.3,"click_y":0.4,
                "center_x":0.3,"center_y":0.4,"start_time":16.6,"end_time":33.4,
                "zoom_factor":2,"is_manual":false,"kind":"typing",
                "centers":[{"time":20.125,"x":0.3,"y":0.4},{"time":20.875,"x":0.4,"y":0.4}]}],
            "total_clicks":0,"session_duration":1000,
            "config":{"enabled":true,"zoom_factor":2,"zoom_duration":2800,"min_click_spacing":0}
        });
        save_auto_zoom_data(path.clone(),serde_json::from_value(value.clone()).unwrap()).await.unwrap();
        let loaded=load_auto_zoom_data(path.clone()).await.unwrap().unwrap();
        assert_eq!((loaded.zoom_blocks[0].start_time,loaded.zoom_blocks[0].end_time),(16.6,33.4));
        assert_eq!(loaded.zoom_blocks[0].centers[1].time,20.875);
        let export=crate::video_processing::types::load_zoom_blocks_from_sidecar(&video).unwrap();
        assert_eq!((export[0].start_time_ms,export[0].end_time_ms),(16.6,33.4));
        assert_eq!(export[0].centers[0].time,20.125);
        let sidecar=crate::recording::artifacts::RecordingArtifacts::new(&video).auto_zoom();
        let saved=std::fs::read(&sidecar).unwrap();
        let mut invalid=loaded;invalid.zoom_blocks[0].start_time=-0.25;
        assert!(save_auto_zoom_data(path.clone(),invalid).await.is_err());
        assert_eq!(std::fs::read(&sidecar).unwrap(),saved);
        let mut legacy=value;
        legacy["zoom_blocks"][0]["start_time"]=10.into();
        legacy["zoom_blocks"][0]["end_time"]=40.into();
        legacy["zoom_blocks"][0]["centers"]=serde_json::json!([]);
        let legacy:ZoomAnalysis=serde_json::from_value(legacy).unwrap();
        auto_zoom::save_analysis(&legacy,&sidecar).unwrap();
        let loaded=load_auto_zoom_data(path).await.unwrap().unwrap();
        assert_eq!((loaded.zoom_blocks[0].start_time,loaded.zoom_blocks[0].end_time),(10.0,40.0));
        std::fs::remove_dir_all(directory).unwrap();
    }
}
