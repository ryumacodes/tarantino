use super::super::{gpu_compositor::GpuCompositorConfig,source_crop::{SourceCrop,CropSampling},
    export_range::ExportRange,timeline::EditTimeline,zoom_trajectory::{ZoomFrameState,sample_camera_frame}};

pub(super) fn sampling(config:&GpuCompositorConfig,crop:SourceCrop,source:[u32;2],
    trajectory:&[ZoomFrameState],range:ExportRange,timeline:Option<&EditTimeline>,
    frames:u64,masks:bool) -> anyhow::Result<CropSampling> {
    let mut previous_segment=None;
    let cameras=(0..frames).map(|frame| {
        let output_ms=frame as f64*1000.0/config.frame_rate;
        let mapped=timeline.and_then(|plan|plan.source_at(output_ms));
        let segment=mapped.map(|(segment,_)|segment.output_start_ms);
        let reset=timeline.is_some()&&segment!=previous_segment;
        previous_segment=segment;
        let gap=timeline.is_some()&&mapped.is_none();
        let source_ms=mapped.map_or(range.start_ms as f64+output_ms,|(_,ms)|ms);
        let camera=(!gap).then(||sample_camera_frame(trajectory,source_ms)).flatten()
            .unwrap_or(ZoomFrameState{rect:None,scale:1.0,center_x:0.5,center_y:0.5});
        (camera,reset)
    });
    config.crop_sampling(crop,source,cameras,masks)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn trimmed_out_camera_motion_does_not_expand_the_decode_crop() {
        let settings=serde_json::from_value(serde_json::json!({
            "resolution":{"width":128,"height":128},
            "visual_settings":{"padding":0,"motion_blur_enabled":true}
        })).unwrap();
        let mut config=super::super::super::gpu_compositor::build_gpu_config_with_webcam(
            &settings,&None,Some(128),Some(128));
        config.window_mode=true;
        let crop=SourceCrop{x:0.25,y:0.25,width:0.5,height:0.5};
        let mut cameras=vec![ZoomFrameState{rect:None,scale:1.0,center_x:0.5,center_y:0.5};120];
        cameras[0].scale=0.001;
        let selected=sampling(&config,crop,[512,512],&cameras,
            ExportRange{start_ms:1000,end_ms:1100},None,3,false).unwrap();
        assert_eq!([selected.decode.width,selected.decode.height],[272,272]);
        let start=sampling(&config,crop,[512,512],&cameras,
            ExportRange{start_ms:0,end_ms:100},None,3,false).unwrap();
        assert_eq!([start.decode.width,start.decode.height],[512,512]);
    }
    #[test]
    fn retimed_cuts_and_gaps_use_output_order_and_reset_shutters() {
        let settings=serde_json::from_value(serde_json::json!({
            "resolution":{"width":128,"height":128},
            "visual_settings":{"padding":0,"motion_blur_enabled":true}
        })).unwrap();
        let mut config=super::super::super::gpu_compositor::build_gpu_config_with_webcam(
            &settings,&None,Some(128),Some(128));
        config.window_mode=true;config.frame_rate=30.0;
        let crop=SourceCrop{x:0.25,y:0.25,width:0.5,height:0.5};
        let base=ZoomFrameState{rect:None,scale:1.0,center_x:0.5,center_y:0.5};
        let mut cameras=vec![base.clone();120];
        cameras[60].scale=2.0;cameras[64].scale=2.1;
        cameras[1].scale=0.9;cameras[2].scale=1.1;
        let clips:Vec<super::super::super::timeline::EditClip>=serde_json::from_value(serde_json::json!([
            {"id":"fast","startTime":0,"duration":60,"sourceIn":1000,"sourceOut":1120,
             "playbackRate":2,"trackOrder":0,"muted":false},
            {"id":"back","startTime":100,"duration":100,"sourceIn":0,"sourceOut":50,
             "playbackRate":0.5,"trackOrder":0,"muted":false}
        ])).unwrap();
        let timeline=EditTimeline::build(&clips,0.0,200.0).unwrap();
        let actual=sampling(&config,crop,[512,512],&cameras,
            ExportRange{start_ms:0,end_ms:200},Some(&timeline),6,false).unwrap();
        let expected=config.crop_sampling(crop,[512,512],[
            (cameras[60].clone(),true),(cameras[64].clone(),false),(base,true),
            (cameras[0].clone(),true),(cameras[1].clone(),false),(cameras[2].clone(),false)
        ],false).unwrap();
        assert_eq!(actual,expected);
        assert!(actual.decode.width<512);
    }

}
