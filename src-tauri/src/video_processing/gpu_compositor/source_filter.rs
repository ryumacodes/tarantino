use super::*;
use crate::video_processing::loupe::mip_pyramid::MipPyramid;

fn sampled_level(source:[u32;2],before:[f32;4],now:[f32;4]) -> u32 {
    let footprint=(source[0] as f32 / before[2].min(now[2]))
        .max(source[1] as f32 / before[3].min(now[3]));
    // Include both trilinear neighbors, also at exact integer LOD boundaries.
    footprint.log2().max(0.0).floor() as u32 + 1
}

impl GpuCompositor {
    pub(super) fn encode_source_filter(&self,encoder:&mut wgpu::CommandEncoder,before:[f32;4],now:[f32;4]) {
        if let Some(pyramid)=&self.source_pyramid {
            pyramid.encode_through(encoder,sampled_level(self.config.source_size,before,now));
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
    let last_level=sampled_level(config.source_size,rect,rect);
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
