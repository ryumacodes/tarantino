use super::*;
use crate::video_processing::loupe::mip_pyramid::MipPyramid;

fn sampled_level(source:[f32;2],before:[f32;4],now:[f32;4]) -> u32 {
    let footprint=(source[0] / before[2].min(now[2]))
        .max(source[1] / before[3].min(now[3]));
    // Include both trilinear neighbors, also at exact integer LOD boundaries.
    footprint.log2().max(0.0).floor() as u32 + 1
}

impl GpuCompositor {
    pub(super) fn encode_source_filter(&self,encoder:&mut wgpu::CommandEncoder,before:[f32;4],now:[f32;4]) {
        if let Some(pyramid)=&self.source_pyramid {
            let [width,height]=self.config.source_size.map(|value|value as f32);
            let rect=self.config.source_uv;
            pyramid.encode_through(encoder,sampled_level([width*rect[2],height*rect[3]],before,now));
        }
    }

    pub(super) fn prepare_source_filter(&mut self) -> Result<()> {
        let [width, height] = self.config.source_size;
        if self.source_pyramid.is_none()
            && (width > self.config.input_width || height > self.config.input_height)
        {
            let source = if self.mask_placement.is_some() {
                self.mask_stage.as_ref().expect("Mask stage initialized").texture()
            } else {
                &self.source_texture
            };
            self.source_pyramid = Some(MipPyramid::new(&self.device, source)?);
            self.bind_group = None;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[ignore = "requires native GPU; source minification"]
    fn minified_stripes_remain_stable_across_subpixel_pan() {
        let settings: crate::video_processing::ExportSettings = serde_json::from_value(serde_json::json!({
            "resolution":{"width":130,"height":130},
            "visual_settings":{"padding":0,"corner_radius":0,"shadow_enabled":false,"motion_blur_enabled":false}
        })).unwrap();
        let mut config = build_gpu_config_with_webcam(&settings, &None, Some(512), Some(512));
        config.source_size = [512, 512];
        let mut compositor = GpuCompositor::new(config, None, 1.0, [1.0;3], None).unwrap();
        let source: Vec<_> = (0..512*512).flat_map(|i| {
            let value = if (i % 512) / 2 % 2 == 0 {0} else {255};
            [value, value, value, 255]
        }).collect();
        for offset in [0.0, 0.001, 0.002, 0.003] {
            let camera = ZoomFrameState {rect:None,scale:1.01,center_x:0.5+offset,center_y:0.5};
            let frame = compositor.composite_frame(&source, &camera, None).unwrap();
            for x in 16..114 {
                let value = frame[(65*130+x)*4];
                assert!((120..=136).contains(&value), "aliasing at offset {offset}, x={x}: {value}");
            }
        }
        let camera = ZoomFrameState {rect:None,scale:1.0,center_x:0.5,center_y:0.5};
        let baseline = compositor.composite_frame(&source, &camera, None).unwrap();
        for _ in 0..2 {
            compositor.set_source_mask(Some(crate::video_processing::mask_composite::MaskPlacement {
                rect:[128.0,128.0,256.0,256.0],radius:0.0,opacity:1.0,
                highlight_opacity:0.5,sensitive:false,
            })).unwrap();
            let masked = compositor.composite_frame(&source, &camera, None).unwrap();
            assert!(masked[(16*130+16)*4] < baseline[(16*130+16)*4] - 20);
            assert_eq!(masked[(65*130+65)*4], baseline[(65*130+65)*4]);
            compositor.set_source_mask(None).unwrap();
            assert_eq!(compositor.composite_frame(&source, &camera, None).unwrap(), baseline);
        }
    }
}

#[test]
#[ignore = "native GPU; measures full and sampled pyramid generation"]
fn profile_source_pyramid_generation() {
    let settings: crate::video_processing::ExportSettings = serde_json::from_value(serde_json::json!({
        "resolution":{"width":960,"height":624},
        "visual_settings":{"padding":10,"corner_radius":0,"shadow_enabled":false}
    })).unwrap();
    let mut config=build_gpu_config_with_webcam(&settings,&None,Some(2940),Some(1912));
    config.source_size=[2940,1912];
    let rect=[0.0,0.0,config.input_width as f32,config.input_height as f32];
    let last_level=sampled_level(config.source_size.map(|value|value as f32),rect,rect);
    let mut compositor=GpuCompositor::new(config,None,1.0,[1.0;3],None).unwrap();
    compositor.prepare_source_filter().unwrap();
    let pyramid=compositor.source_pyramid.as_ref().unwrap();
    let source=vec![127u8;2940*1912*4];
    compositor.queue.write_texture(wgpu::TexelCopyTextureInfo {texture:&compositor.source_texture,
        mip_level:0,origin:wgpu::Origin3d::ZERO,aspect:wgpu::TextureAspect::All},&source,
        wgpu::TexelCopyBufferLayout {offset:0,bytes_per_row:Some(2940*4),rows_per_image:Some(1912)},
        compositor.source_texture.size());
    let mut full=Vec::new();let mut bounded=Vec::new();
    for iteration in 0..40 {
        for limited in if iteration%2==0 {[false,true]} else {[true,false]} {
            let start=std::time::Instant::now();
            let mut encoder=compositor.device.create_command_encoder(&Default::default());
            if limited {pyramid.encode_through(&mut encoder,last_level);} else {pyramid.encode(&mut encoder);}
            compositor.queue.submit([encoder.finish()]);
            compositor.device.poll(wgpu::PollType::wait_indefinitely()).unwrap();
            if iteration>=10 {
                (if limited {&mut bounded} else {&mut full}).push(start.elapsed().as_secs_f64()*1000.0);
            }
        }
    }
    let summary=|mut values:Vec<f64>| {
        values.sort_by(f64::total_cmp);
        serde_json::json!({"median_ms":values[values.len()/2],"samples_ms":values})
    };
    let result=serde_json::json!({"source":[2940,1912],"plane":[rect[2],rect[3]],
        "full_levels":pyramid.texture().mip_level_count(),"sampled_levels":last_level+1,
        "scope":"CPU encode + submit + wait for GPU completion; alternating warmed samples, not full export",
        "full":summary(full),"bounded":summary(bounded)});
    let path=std::path::Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap().join("oss-research/source-pyramid-profile.json");
    std::fs::write(path,serde_json::to_vec_pretty(&result).unwrap()).unwrap();
    println!("{result}");
}

#[test]
#[ignore = "requires native GPU; fractional source texture coordinates"]
fn fractional_source_view_preserves_subpixel_sampling() {
    let settings: crate::video_processing::ExportSettings = serde_json::from_value(serde_json::json!({
        "resolution":{"width":128,"height":128},
        "visual_settings":{"padding":0,"corner_radius":0,"shadow_enabled":false,"motion_blur_enabled":false}
    })).unwrap();
    for rect in [[0.25,0.75,96.5,95.25], [7.75,12.25,84.5,90.5]] {
        for padding in [0,1,128] {
        let plan=crate::video_processing::source_crop::SourceCrop {
            x:rect[0] as f64/128.0,y:rect[1] as f64/128.0,
            width:rect[2] as f64/128.0,height:rect[3] as f64/128.0,
        }.sampling(128,128,padding).unwrap();
        let raster=plan.decode;
        let source:Vec<_>=(raster.y..raster.y+raster.height).flat_map(|y|
            (raster.x..raster.x+raster.width).flat_map(move |x|[(x*2) as u8,(y*2) as u8,0,255])).collect();
        let mut config = build_gpu_config_with_webcam(&settings,&None,Some(128),Some(128));
        config.source_size = [raster.width,raster.height];
        config.source_uv = plan.view.map(|value|value as f32);
        let mut compositor = GpuCompositor::new(config,None,1.0,[1.0;3],None).unwrap();
        let camera = ZoomFrameState {rect:None,scale:1.0,center_x:0.5,center_y:0.5};
        let frame = compositor.composite_frame(&source,&camera,None).unwrap();
        for y in 8..120 {for x in 8..120 {
            let expected = [(rect[0]+x as f32/128.0*rect[2]-0.5)*2.0,
                (rect[1]+y as f32/128.0*rect[3]-0.5)*2.0];
            let pixel=&frame[(y*128+x)*4..][..4];
            for channel in 0..2 {
                assert!((pixel[channel] as f32-expected[channel]).abs()<=1.1,
                    "rect={rect:?}, pixel=({x},{y}), channel={channel}: {} != {}",pixel[channel],expected[channel]);
            }
            assert_eq!(&pixel[2..],&[0,255]);
        }}
        }
    }
}

#[test]
#[ignore = "requires native GPU; cropped texture footprint"]
fn source_view_uses_logical_size_for_mip_selection() {
    let settings: crate::video_processing::ExportSettings = serde_json::from_value(serde_json::json!({
        "resolution":{"width":128,"height":128},
        "visual_settings":{"padding":0,"corner_radius":0,"shadow_enabled":false,"motion_blur_enabled":false}
    })).unwrap();
    let mut config=build_gpu_config_with_webcam(&settings,&None,Some(128),Some(128));
    config.source_size=[256,256];
    config.source_uv=[64.5/256.0,64.5/256.0,0.5,0.5];
    let mut compositor=GpuCompositor::new(config,None,1.0,[1.0;3],None).unwrap();
    let source:Vec<_>=(0..256*256).flat_map(|i| {
        let value=if i%2==0 {0} else {255}; [value,value,value,255]
    }).collect();
    let camera=ZoomFrameState{rect:None,scale:1.0,center_x:0.5,center_y:0.5};
    let frame=compositor.composite_frame(&source,&camera,None).unwrap();
    for x in 8..120 {
        let expected=if x%2==0 {0u8} else {255u8};
        assert!(frame[(64*128+x)*4].abs_diff(expected)<=1,"stripe {x} blurred despite a one-to-one logical footprint");
    }
}

#[test]
#[ignore = "requires native GPU; crop mip alignment"]
fn bounded_crop_mips_match_full_source_sampling() {
    use crate::video_processing::source_crop::{SourceCrop,CropSampling};
    let settings:crate::video_processing::ExportSettings=serde_json::from_value(serde_json::json!({
        "resolution":{"width":128,"height":128},
        "visual_settings":{"padding":0,"corner_radius":0,"shadow_enabled":false,"motion_blur_enabled":false}
    })).unwrap();
    for [width,height,last_level] in [[512,512,2],[511,509,2],[512,509,2],[1024,1024,3]] {
        for [x,y] in [[0.2563,0.1991],[0.001,0.001],[0.49,0.37]] {
        let crop=SourceCrop{x,y,width:0.51,height:0.63};
        let render=|plan:CropSampling| {
            let raster=plan.decode;
            let source:Vec<_>=(raster.y..raster.y+raster.height).flat_map(|y|
                (raster.x..raster.x+raster.width).flat_map(move |x| {
                    [((x*73+y*29)%256) as u8,((x*17+y*103)%256) as u8,((x^y)%256) as u8,255]
                })).collect();
            let mut config=build_gpu_config_with_webcam(&settings,&None,Some(128),Some(128));
            config.source_size=[raster.width,raster.height];
            config.source_uv=plan.view.map(|v|v as f32);
            let mut compositor=GpuCompositor::new(config,None,1.0,[1.0;3],None).unwrap();
            compositor.composite_frame(&source,&ZoomFrameState{rect:None,scale:1.0,center_x:0.5,center_y:0.5},None).unwrap()
        };
        let full=render(crop.sampling(width,height,u32::MAX).unwrap());
        let bounded=crop.sampling_for_mips(width,height,last_level).unwrap();
        if width%4==0 {assert!(bounded.decode.width<width);} else {assert_eq!(bounded.decode.width,width);}
        if height%4!=0 {assert_eq!(bounded.decode.height,height);}
        let actual=render(bounded);
        for (index,(&a,&b)) in actual.iter().zip(&full).enumerate() {
            assert!(a.abs_diff(b)<=1,"source={width}x{height} byte={index}: bounded {a}, full {b}");
        }
        }
    }
}
