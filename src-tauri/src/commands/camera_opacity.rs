use crate::video_processing::{export, ExportSettings,
    gpu_compositor::build_gpu_config_with_webcam,
    loupe::{camera_opacity::CameraOpacity,camera_opacity_prepare::prepare_camera_opacity},
    visual_effects::get_webcam_info,zoom_trajectory::ZoomFrameState};

/// Return export's fixed-rate camera fade samples; no UI-frame integration.
#[tauri::command]
pub async fn compute_camera_opacity(
    input_path:String,duration_ms:u64,settings:ExportSettings,
)->Result<Vec<CameraOpacity>,String> {
    if duration_ms==0 || settings.webcam_enabled==Some(false) {
        return Ok(Vec::new());
    }
    let webcam=get_webcam_info(std::path::Path::new(&input_path));
    if webcam.is_none() {return Ok(Vec::new());}
    let motion=super::motion::compute_zoom_trajectory(input_path.clone(),duration_ms,settings.clone()).await?;
    tauri::async_runtime::spawn_blocking(move || {
        let source=(settings.source_width.unwrap_or(1920),settings.source_height.unwrap_or(1080));
        let crop=settings.source_crop.map(|c|c.resolve(source.0,source.1)).transpose()?;
        let cropped=crop.map(|c|(c.width,c.height)).unwrap_or(source);
        let config=build_gpu_config_with_webcam(&settings,&webcam,Some(cropped.0),Some(cropped.1));
        let mouse=export::resolve_mouse_events_path(std::path::Path::new(&input_path));
        let activity=export::load_camera_activity(&mouse,std::path::Path::new(&input_path),
            source,&settings,duration_ms)?;
        let camera:Vec<_>=motion.into_iter().map(|v|ZoomFrameState{scale:v[0],center_x:v[1],center_y:v[2]}).collect();
        let timeline=prepare_camera_opacity(settings.zoom_blocks.as_deref().unwrap_or(&[]),&[],source,
            duration_ms,export::get_zoom_spring_config(&settings),&settings,&config,&camera,Some(&activity))?;
        Ok::<_,anyhow::Error>((0..(duration_ms as f64*120.0/1000.0).ceil() as usize).map(|index|timeline.sample(index as f64*1000.0/120.0)).collect())
    }).await.map_err(|e|e.to_string())?.map_err(|e|e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[tokio::test]
    async fn preview_camera_opacity_uses_export_geometry_and_hidden_camera_gate() {
        let root=std::env::temp_dir().join(format!("camera-opacity-preview-{}",uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&root).unwrap();
        let input=root.join("source.mp4");
        // This command only needs sidecar identity, not decoded footage.
        std::fs::write(input.with_extension("webcam.mp4"),[]).unwrap();
        std::fs::write(input.with_extension("mouse.json"),"[]").unwrap();
        let mut settings:ExportSettings=serde_json::from_value(serde_json::json!({
            "source_width":128,"source_height":128,"resolution":{"width":128,"height":128},
            "webcam_x":0.5,"webcam_y":0.5,"webcam_size":0.2,
            "visual_settings":{"padding":0,"corner_radius":0},
            "zoom_blocks":[{"start_time_ms":500,"end_time_ms":1500,"zoom_level":2,
                "center_x":0.5,"center_y":0.5,"is_manual":true,
                "loupe":{"radius":0.25,"glass":false,"bevel":0,"chromatic_aberration":0,"shadow_alpha":0}}]
        })).unwrap();
        let path=input.to_string_lossy().into_owned();
        let frames=compute_camera_opacity(path.clone(),3000,settings.clone()).await.unwrap();
        assert_eq!(frames.len(),360);
        assert_eq!(frames[0].stage,1.0);
        assert!(frames[168].stage<0.05);
        assert!(frames[168].lens_content<0.05);
        assert_eq!(frames[359].stage,1.0);
        settings.zoom_blocks=Some(Vec::new());
        settings.webcam_keyframes=Some(serde_json::from_value(serde_json::json!([
            {"id":"move","time":500,"x":0.8,"y":0.8,"size":0.3,"shape":"circle","visible":true},
            {"id":"hide","time":1500,"x":0.8,"y":0.8,"size":0.3,"shape":"circle","visible":false}
        ])).unwrap());
        let motion=compute_camera_opacity(path.clone(),3000,settings.clone()).await.unwrap();
        assert_eq!(motion.len(),360,"camera movement must work without magnifier blocks");
        let start=motion[0].rect.unwrap();
        let moving=motion[72].rect.unwrap();
        let settled=motion[170].rect.unwrap();
        assert!(moving[0]>start[0] && moving[0]<settled[0]);
        assert!(moving[2]>start[2] && moving[2]<settled[2]);
        assert!(motion[192].stage>0.0 && motion[192].stage<1.0,"hidden layout should fade");
        assert_eq!(motion[359].stage,0.0);
        assert!(motion[359].rect.unwrap()[2]<settled[2],"hidden camera should shrink");
        assert!(motion.iter().all(|frame|frame.lens_content==1.0));
        settings.webcam_enabled=Some(false);
        assert!(compute_camera_opacity(path,3000,settings).await.unwrap().is_empty());
        std::fs::remove_dir_all(root).unwrap();
    }
}
