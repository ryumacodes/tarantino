use crate::video_processing::{
    export,
    loupe::{self, LoupeTarget},
    ExportSettings,
};

/// Prepare source-time lens frames away from the UI thread.
#[tauri::command]
pub async fn compute_loupe_trajectory(
    input_path: String,
    duration_ms: u64,
    settings: ExportSettings,
) -> Result<Vec<LoupeTarget>, String> {
    let blocks = settings.zoom_blocks.clone().unwrap_or_default();
    if duration_ms == 0
        || !blocks
            .iter()
            .any(|block| block.loupe.is_some() && !block.is_disabled)
    {
        return Ok(Vec::new());
    }
    tauri::async_runtime::spawn_blocking(move || {
        let source = (
            settings.source_width.unwrap_or(1920),
            settings.source_height.unwrap_or(1080),
        );
        let mouse = export::resolve_mouse_events_path(std::path::Path::new(&input_path));
        let activity=export::load_camera_activity(&mouse,std::path::Path::new(&input_path),
            source,&settings,duration_ms).map_err(|e|e.to_string())?;
        loupe::prepare::trajectory_with_settings(
            &blocks,
            &[],
            source,
            duration_ms,
            loupe::animation::SIMULATION_FPS,
            export::get_zoom_spring_config(&settings),
            &settings,
         Some(&activity))
        .map_err(|error| error.to_string())
    })
    .await
    .map_err(|error| error.to_string())?
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn lens_fallback_preserves_raw_positions_release_lookahead_and_crop() {
        let input=std::env::temp_dir().join(format!("lens-fallback-{}.mp4",uuid::Uuid::new_v4()));
        let mouse=input.with_extension("mouse.json");
        for release in [600,601] {
            std::fs::write(&mouse,serde_json::to_vec(&serde_json::json!({
                "display_width":100,"display_height":100,"mouse_events":[
                    {"timestamp":0,"x":50,"y":50,"event_type":"Move"},
                    {"timestamp":90,"x":55,"y":50,"event_type":"Move"},
                    {"timestamp":release,"x":80,"y":70,"event_type":{"ButtonRelease":"Left"}}]
            })).unwrap()).unwrap();
            for smooth in [false,true] { for cropped in [false,true] {
                let mut settings:ExportSettings=serde_json::from_value(serde_json::json!({
                    "source_width":100,"source_height":100,"capture_mode":"window",
                    "cursor_settings":{"spring_tension":170,"spring_friction":30,"spring_mass":if smooth{1}else{0}},
                    "zoom_blocks":[{"start_time_ms":100,"end_time_ms":200,"zoom_level":2,
                        "center_x":0.5,"center_y":0.5,"instant_animation":true,
                        "loupe":{"radius":0.2,"glass":false,"bevel":0,"chromatic_aberration":0,"shadow_alpha":0}}]
                })).unwrap();
                if cropped { settings.source_crop=Some(crate::video_processing::source_crop::SourceCrop {
                    x:0.5,y:0.0,width:0.5,height:1.0 }); }
                let path=input.to_string_lossy().into_owned();
                let actual=compute_loupe_trajectory(path.clone(),700,settings.clone()).await.unwrap();
                let center=if smooth && release==600 {(0.8,0.7)} else {(0.55,0.5)};
                let block=&mut settings.zoom_blocks.as_mut().unwrap()[0];
                block.is_manual=true;
                block.center_x=if cropped {(center.0-0.5)*2.0} else {center.0};
                block.center_y=center.1;
                let expected=compute_loupe_trajectory(path,700,settings).await.unwrap();
                assert!((actual[18].center_x-expected[18].center_x).abs()<1e-12,
                    "smooth={smooth},cropped={cropped},release={release}");
                assert!((actual[18].center_y-expected[18].center_y).abs()<1e-12);
                assert_eq!(actual[18].radius,expected[18].radius);
            }}
        }
        std::fs::remove_file(mouse).unwrap();
    }

    #[tokio::test]
    async fn native_preview_request_preserves_crop_and_rejects_invalid_lens() {
        let mut settings: ExportSettings = serde_json::from_value(serde_json::json!({
            "source_width":1000,"source_height":500,
            "source_crop":{"x":0.5,"y":0,"width":0.5,"height":1},
            "zoom_blocks":[{"start_time_ms":0,"end_time_ms":1000,"zoom_level":2,
                "center_x":0.5,"center_y":0.5,"is_manual":true,"instant_animation":true,
                "loupe":{"radius":0.25,"glass":true,"bevel":0.1,
                    "chromatic_aberration":0.01,"shadow_alpha":0.35}}]
        })).unwrap();
        let path = std::env::temp_dir().join(format!("lens-preview-{}.mp4",uuid::Uuid::new_v4()));
        let path = path.to_string_lossy().into_owned();
        let frames = compute_loupe_trajectory(path.clone(),1000,settings.clone()).await.unwrap();
        assert_eq!(frames.len(),120);
        assert_eq!((frames[0].center_x,frames[0].radius,frames[0].zoom),(0.75,0.125,2.0));
        settings.zoom_blocks.as_mut().unwrap()[0].loupe.as_mut().unwrap().radius = 0.0;
        assert!(compute_loupe_trajectory(path.clone(),1000,settings.clone()).await.is_err());
        settings.zoom_blocks.as_mut().unwrap()[0].is_disabled = true;
        assert!(compute_loupe_trajectory(path,1000,settings).await.unwrap().is_empty());
    }
}
