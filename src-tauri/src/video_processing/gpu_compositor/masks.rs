use super::*;
use crate::video_processing::{mask_composite::MaskPlacement,mask_stage::MaskStage};
impl GpuCompositor {
    pub(crate) fn set_source_mask(&mut self,placement:Option<MaskPlacement>)->Result<()> {
        let placement=placement.filter(|p|p.opacity!=0.0);
        if placement.is_some()&&self.mask_stage.is_none() {
            self.mask_stage=Some(MaskStage::new(&self.device,&self.source_texture)?);
        }
        if self.mask_placement.is_some()!=placement.is_some(){self.bind_group=None;}
        self.mask_placement=placement;
        Ok(())
    }
    pub(super) fn encode_source_mask(&mut self,encoder:&mut wgpu::CommandEncoder)->Result<()> {
        if let (Some(stage),Some(placement))=(&mut self.mask_stage,&self.mask_placement){
            stage.encode(&self.queue,encoder,placement)?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    #[ignore="requires native GPU"]
    fn source_masks_run_before_framing_and_clear_without_stale_pixels() {
        let settings:crate::video_processing::ExportSettings=serde_json::from_value(serde_json::json!({
            "resolution":{"width":128,"height":128},"source_type":"window",
            "visual_settings":{"padding":20,"corner_radius":0,"shadow_enabled":false,"background_color":"#204080"}
        })).unwrap();
        let mut config=build_gpu_config_with_webcam(&settings,&None,Some(64),Some(64));
        config.motion_blur_enabled=false;
        let (w,h)=(config.input_width,config.input_height);
        let mut compositor=GpuCompositor::new(config,None,1.0,[1.0;3],None).unwrap();
        let camera=ZoomFrameState{rect:None,scale:1.0,center_x:0.5,center_y:0.5};
        let source:Vec<u8>=(0..w*h).flat_map(|i|[if i%2==0 {0}else{255},0,0,255]).collect();
        let baseline=compositor.composite_frame(&source,&camera,None).unwrap();
        for opacity in [1.0,0.0,1.0] {
            compositor.set_source_mask(Some(MaskPlacement{rect:[16.0,16.0,32.0,32.0],radius:0.0,
                opacity,highlight_opacity:0.5,sensitive:true})).unwrap();
            let frame=compositor.composite_frame(&source,&camera,None).unwrap();
            assert_eq!(&frame[..4],&baseline[..4],"mask must not touch the framed backdrop");
            if opacity==0.0 {assert_eq!(frame,baseline);assert!(compositor.mask_placement.is_none());}
            else {
                let raw=crate::video_processing::gpu_compositor::layer_tests::read_texture(&compositor,compositor.mask_stage.as_ref().unwrap().texture());
                let center=((32*w+32)*4) as usize;
                assert!((110..=145).contains(&raw[center]),"checkerboard should become its color average: {}",raw[center]);
                assert_eq!(&raw[center+1..center+4],&[0,0,255]);
                assert_eq!(&raw[..4],&source[..4]);assert_ne!(frame,baseline);
            }
        }
        compositor.set_source_mask(None).unwrap();
        assert_eq!(compositor.composite_frame(&source,&camera,None).unwrap(),baseline);
    }
}
