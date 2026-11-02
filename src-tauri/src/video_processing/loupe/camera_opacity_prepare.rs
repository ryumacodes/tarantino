//! Resolve camera/lens overlap from the same cropped stage geometry as rendering.
use super::{camera_opacity::{CameraOpacityTimeline,CameraOpacity},camera_overlap::coverage,prepare,projection};
use crate::{cursor_renderer::CursorEvent,video_processing::{
    types::{ExportSettings,ZoomBlock},gpu_compositor::GpuCompositorConfig,
    webcam_timeline::WebcamTimeline, camera_motion::{CameraMotionTimeline,CameraMotionTarget},zoom_trajectory::ZoomFrameState,
}};
use crate::cursor_renderer::SpringConfig;
use anyhow::{Result,bail};

pub fn prepare_camera_opacity(
    blocks:&[ZoomBlock],events:&[CursorEvent],source:(u32,u32),duration_ms:u64,
    spring:SpringConfig,settings:&ExportSettings,config:&GpuCompositorConfig,
    camera_motion:&[ZoomFrameState],
    prepared_activity:Option<&crate::video_processing::camera_activity::Tracks>,
)->Result<CameraOpacityTimeline> {
    let count=(duration_ms as f64*120.0/1000.0).ceil() as usize;
    let has_lens=blocks.iter().any(|b|b.loupe.is_some() && !b.is_disabled);
    if has_lens && camera_motion.len()!=count {bail!("Camera overlap requires a complete 120 Hz camera trajectory");}
    let (targets,animated)=prepare::target_and_animated_trajectories(
        blocks,events,source,duration_ms,spring,settings,prepared_activity)?;
    let cameras=WebcamTimeline::new(settings,config.webcam.clone())?;
    let width=config.output_width as f64;
    let height=config.output_height as f64;
    let video_rect=[(config.content_offset_x+config.input_offset_x) as f64,
        (config.content_offset_y+config.input_offset_y) as f64,
        config.input_width as f64,config.input_height as f64];
    let mut motion_targets=Vec::with_capacity(count);
    for index in 0..count {
        let time=index as f64*1000.0/120.0;
        let mut camera=cameras.motion_target(time,config.output_width,config.output_height,false)
            .unwrap_or(CameraMotionTarget{rect:[0.0;4],opacity:0.0,hidden:true});
        let target=targets[index];
        if has_lens && !camera.hidden && target.presence>0.0 && target.radius>0.0 {
            let lens=projection::project(target,source,settings.source_crop,video_rect,
                &camera_motion[index],config.window_mode)?;
            if coverage((lens.center[0] as f64,lens.center[1] as f64),lens.radius as f64,camera.rect)>=0.5 {
                camera.opacity=0.0;
            }
        }
        motion_targets.push(camera);
    }
    let motion=CameraMotionTimeline::new(&motion_targets,spring)?;
    let frames=(0..count).map(|index|{
        let frame=motion.sample(index as f64*1000.0/120.0).expect("complete camera trajectory");
        CameraOpacity{stage:frame.opacity,lens_content:(1.0-animated[index].presence).clamp(0.0,1.0),
            rect:Some([frame.rect[0]/width,frame.rect[1]/height,frame.rect[2]/width,frame.rect[3]/height])}
    }).collect();
    Ok(CameraOpacityTimeline::from_frames(duration_ms,frames))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::video_processing::gpu_compositor::{build_gpu_config_with_webcam,WebcamConfig};
    #[test]
    fn cropped_lens_hides_only_the_camera_it_covers_and_recovers() {
        let settings:ExportSettings=serde_json::from_value(serde_json::json!({
            "resolution":{"width":128,"height":128},
            "source_crop":{"x":0.5,"y":0.0,"width":0.5,"height":1.0},
            "visual_settings":{"padding":0,"corner_radius":0,"shadow_enabled":false}
        })).unwrap();
        let block:ZoomBlock=serde_json::from_value(serde_json::json!({
            "start_time_ms":500,"end_time_ms":1500,"zoom_level":2,"is_manual":true,
            "center_x":0.5,"center_y":0.5,
            "loupe":{"radius":0.25,"glass":false,"bevel":0,"chromatic_aberration":0,"shadow_alpha":0}
        })).unwrap();
        let mut config=build_gpu_config_with_webcam(&settings,&None,Some(128),Some(128));
        config.webcam=Some(WebcamConfig{pos_x:0.5,pos_y:0.5,size:0.2,shape:"circle".into()});
        let frames=vec![ZoomFrameState{scale:1.0,center_x:0.5,center_y:0.5};360];
        let run=|config:&GpuCompositorConfig|prepare_camera_opacity(&[block.clone()],&[],(256,128),3000,
            SpringConfig{mass:3.0,tension:470.0,friction:70.0},&settings,config,&frames,None).unwrap();
        let covered=run(&config);
        assert_eq!(covered.sample(0.0).stage,1.0);
        assert!(covered.sample(1400.0).stage<0.01);
        assert_eq!(covered.sample(2990.0).stage,1.0);
        config.webcam.as_mut().unwrap().pos_x=0.1;
        config.webcam.as_mut().unwrap().pos_y=0.1;
        assert_eq!(run(&config).sample(1400.0).stage,1.0);
        assert!(covered.sample(1400.0).lens_content<0.01);
    }
}
