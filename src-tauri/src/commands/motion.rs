use crate::video_processing::{ExportSettings, export, zoom_trajectory};

/// Build the same source-time camera path used by export, away from the UI thread.
#[tauri::command]
pub async fn compute_zoom_trajectory(
    input_path: String,
    duration_ms: u64,
    mut settings: ExportSettings,
) -> Result<Vec<[f64; 3]>, String> {
    if duration_ms == 0
        || settings
            .zoom_blocks
            .as_ref()
            .is_none_or(|blocks| blocks.is_empty())
    {
        return Ok(Vec::new());
    }
    let spring = export::get_zoom_spring_config(&settings);
    if !spring.tension.is_finite()
        || spring.tension <= 0.0
        || !spring.friction.is_finite()
        || spring.friction < 0.0
        || !spring.mass.is_finite()
        || spring.mass < 0.0
    {
        return Err("Invalid camera motion settings".into());
    }
    tauri::async_runtime::spawn_blocking(move || {
        let path = std::path::Path::new(&input_path);
        let mouse_path = export::resolve_mouse_events_path(path);
        let activity=export::load_camera_activity(&mouse_path,path,
            (settings.source_width.unwrap_or(1920),settings.source_height.unwrap_or(1080)),
            &settings,duration_ms).map_err(|e|e.to_string())?;
        let crop = settings.source_crop.map(|rect| rect.resolve(settings.source_width.unwrap_or(1920),
            settings.source_height.unwrap_or(1080))).transpose().map_err(|error| error.to_string())?;
        let mut blocks = settings.zoom_blocks.clone().unwrap_or_default();
        crate::video_processing::source_crop::remap_motion(crop, &mut [], &mut blocks);
        let source = crop.map(|c| (c.width,c.height)).unwrap_or((settings.source_width.unwrap_or(1920),settings.source_height.unwrap_or(1080)));
        if let Some(crop) = crop {
            let rect = crop.normalized();
            settings.window_width = settings.window_width.map(|w| (w as f64 * rect.width).round().max(1.0) as u32);
            settings.window_height = settings.window_height.map(|h| (h as f64 * rect.height).round().max(1.0) as u32);
        }
        let viewport = zoom_trajectory::CameraViewport::from_settings(&settings, source);
        let frames = zoom_trajectory::simulate_zoom_trajectory_in_viewport(
            &blocks,
            &[],
            &spring,
            settings
                .screen_motion_style
                .as_deref()
                .is_some_and(|s| matches!(s, "focused" | "smooth")),
            settings.animation_speed.as_deref().unwrap_or("mellow"),
            zoom_trajectory::CAMERA_SIMULATION_FPS,
            duration_ms,
            settings.capture_mode.as_deref() == Some("window"),
            viewport,
            Some(zoom_trajectory::targets::CameraActivity {events:&[],prepared:Some(&activity),smooth:export::get_cursor_spring_config(&settings).mass>0.0}),
        );
        Ok(frames
            .into_iter()
            .map(|frame| [frame.scale, frame.center_x, frame.center_y])
            .collect())
    })
    .await
    .map_err(|error| error.to_string())?
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn preview_trim_excludes_future_release_from_camera_target() {
        let input=std::env::temp_dir().join(format!("camera-window-{}.mp4",uuid::Uuid::new_v4()));
        let sidecar=input.with_extension("mouse.json");
        std::fs::write(&sidecar,serde_json::json!({"display_width":100,"display_height":100,
            "mouse_events":[{"timestamp":0,"x":10,"y":50},
                {"timestamp":300,"x":55,"y":50},
                {"timestamp":800,"x":90,"y":50,"event_type":{"ButtonRelease":"Left"}}]}).to_string()).unwrap();
        let mut settings:ExportSettings=serde_json::from_value(serde_json::json!({
            "source_width":100,"source_height":100,"capture_mode":"window",
            "zoom_blocks":[{"start_time_ms":500,"end_time_ms":600,"zoom_level":2,
                "center_x":0.5,"center_y":0.5,"instant_animation":true}],
            "cursor_settings":{"spring_tension":170,"spring_friction":30,"spring_mass":1}
        })).unwrap();
        let full=compute_zoom_trajectory(input.to_string_lossy().into_owned(),1000,settings.clone()).await.unwrap();
        settings.trim_start=Some(200);settings.trim_end=Some(700);
        let trimmed=compute_zoom_trajectory(input.to_string_lossy().into_owned(),1000,settings).await.unwrap();
        assert!((full[66][1]-0.9).abs()<1e-12);
        assert!((trimmed[66][1]-0.55).abs()<1e-12);
        std::fs::remove_file(sidecar).unwrap();
    }

    #[tokio::test]
    async fn empty_camera_range_uses_raw_motion_and_optional_release_lookahead() {
        let input=std::env::temp_dir().join(format!("camera-fallback-{}.mp4",uuid::Uuid::new_v4()));
        let mouse=input.with_extension("mouse.json");
        for release in [600,601] {
            std::fs::write(&mouse,serde_json::to_vec(&serde_json::json!({
                "display_width":100,"display_height":100,"mouse_events":[
                    {"timestamp":0,"x":50,"y":50,"event_type":"Move"},
                    {"timestamp":90,"x":55,"y":50,"event_type":"Move"},
                    {"timestamp":release,"x":80,"y":70,"event_type":{"ButtonRelease":{"button":"Left"}}}]
            })).unwrap()).unwrap();
            let (grouped,raw)=export::load_camera_tracks(&mouse,100,100,0.0);
            assert!(!grouped.iter().any(|event|event.timestamp_ms==90));
            assert!(raw.last().unwrap().is_release);
            assert!(!raw.last().unwrap().is_click);
            for smooth in [false,true] {
                let settings:ExportSettings=serde_json::from_value(serde_json::json!({
                    "source_width":100,"source_height":100,"capture_mode":"window",
                    "cursor_settings":{"spring_tension":170,"spring_friction":30,"spring_mass":if smooth{1}else{0}},
                    "zoom_blocks":[{"start_time_ms":100,"end_time_ms":200,"zoom_level":2,
                        "center_x":0.5,"center_y":0.5,"instant_animation":true}]
                })).unwrap();
                let frames=compute_zoom_trajectory(input.to_string_lossy().into_owned(),700,settings).await.unwrap();
                let expected=if smooth && release==600 {(0.8,0.7)} else {(0.55,0.5)};
                assert!((frames[18][1]-expected.0).abs()<1e-12,"smooth={smooth},release={release}");
                assert!((frames[18][2]-expected.1).abs()<1e-12);
            }
        }
        std::fs::remove_file(mouse).unwrap();
    }

    #[tokio::test]
    async fn preview_and_export_share_camera_frames_at_every_output_rate() {
        let input =
            std::env::temp_dir().join(format!("camera-motion-{}.mp4", uuid::Uuid::new_v4()));
        let mouse = input.with_extension("mouse.json");
        std::fs::write(
            &mouse,
            r#"[{"timestamp":0,"x":20,"y":30},{"timestamp":200,"x":99,"y":99},
                {"timestamp":250,"x":20,"y":30},{"timestamp":300,"x":20,"y":30},
                {"timestamp":750,"x":80,"y":60}]"#,
        )
        .unwrap();
        let settings: ExportSettings = serde_json::from_value(serde_json::json!({
            "source_width": 100, "source_height": 100, "capture_mode": "window",
            "cursor_settings": {"shake_threshold": 50},
            "screen_motion_style": "focused", "zoom_spring_tension": 250,
            "zoom_spring_friction": 40, "zoom_spring_mass": 2.25,
            "zoom_blocks": [{"start_time_ms":0,"end_time_ms":400,"zoom_level":2,
                "center_x":0.2,"center_y":0.3,"loupe":{"radius":0.2,"glass":false,
                    "bevel":0,"chromatic_aberration":0,"shadow_alpha":0}},
                {"start_time_ms": 500, "end_time_ms": 1000,
                "animation_lead_in_ms": 300,
                "zoom_level": 2, "center_x": 0.2, "center_y": 0.3},
                {"start_time_ms": 1200, "end_time_ms": 1500, "zoom_level": 3,
                "center_x": 0.4, "center_y": 0.4, "instant_animation": true}]
        }))
        .unwrap();
        let preview =
            compute_zoom_trajectory(input.to_string_lossy().into_owned(), 2000, settings.clone())
                .await
                .unwrap();
        assert!(preview[..=48].iter().all(|frame| frame[0] == 1.0),
            "magnifier must block screen lead-in through its inclusive endpoint");
        assert!(preview[54][0]>1.0,"screen lead-in should resume after magnifier ends");
        let events = export::load_camera_events(&mouse, 100, 100, 50.0);
        assert!(!events.iter().any(|event| event.timestamp_ms == 200));
        let activity=export::load_camera_activity(&mouse,&input,(100,100),&settings,2000).unwrap();
        let spring = export::get_zoom_spring_config(&settings);
        for fps in [24.0, 30.0, 60.0, 120.0] {
            let exported = zoom_trajectory::simulate_zoom_trajectory_in_viewport(
                settings.zoom_blocks.as_ref().unwrap(),
                &[],
                &spring,
                true,
                "mellow",
                fps,
                2000,
                true,
                zoom_trajectory::CameraViewport::from_settings(&settings, (100,100)),
                Some(zoom_trajectory::targets::CameraActivity{events:&[],smooth:true,prepared:Some(&activity)}),
            );
            for (i, frame) in exported.iter().enumerate() {
                let preview_index = (i as f64 * 120.0 / fps).round() as usize;
                assert_eq!(
                    preview[preview_index],
                    [frame.scale, frame.center_x, frame.center_y]
                );
            }
        }
        std::fs::remove_file(mouse).unwrap();
    }

    #[tokio::test]
    async fn portrait_preview_holds_a_vertical_activity_group_visible_on_stage() {
        let input = std::env::temp_dir().join(format!("camera-viewport-{}.mp4", uuid::Uuid::new_v4()));
        let mouse = input.with_extension("mouse.json");
        std::fs::write(&mouse, r#"[{"timestamp":0,"x":500,"y":100},{"timestamp":500,"x":500,"y":300}]"#).unwrap();
        let mut settings: ExportSettings = serde_json::from_value(serde_json::json!({
            "source_width":1000, "source_height":500, "capture_mode":"window",
            "resolution":{"width":500,"height":1000},
            "zoom_blocks":[{"start_time_ms":0,"end_time_ms":900,"zoom_level":2,
                "center_x":0.5,"center_y":0.2,"instant_animation":true}]
        })).unwrap();
        let portrait = compute_zoom_trajectory(input.to_string_lossy().into_owned(), 1000, settings.clone()).await.unwrap();
        settings.resolution = Some(crate::video_processing::types::Resolution { width:1000, height:500 });
        let landscape = compute_zoom_trajectory(input.to_string_lossy().into_owned(), 1000, settings).await.unwrap();
        // Both points fit vertically on the portrait stage, so their dwell-weighted
        // center remains fixed. Landscape must change groups when the pointer moves.
        let weighted_y = (0.2 * 500.0 + 0.6 * 100.0) / 600.0;
        for index in [20,40,80] { assert!((portrait[index][2] - weighted_y).abs() < 1e-9); }
        assert!((landscape[20][2] - 0.2).abs() < 1e-9);
        assert!((landscape[80][2] - 0.6).abs() < 1e-9);
        std::fs::remove_file(mouse).unwrap();
    }
}
