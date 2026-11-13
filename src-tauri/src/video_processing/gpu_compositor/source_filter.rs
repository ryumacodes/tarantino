use super::*;
use crate::video_processing::loupe::mip_pyramid::MipPyramid;

fn sampled_level(source:[f32;2],before:[f32;4],now:[f32;4]) -> u32 {
    let footprint=(source[0] / before[2].min(now[2]))
        .max(source[1] / before[3].min(now[3]));
    // Include both trilinear neighbors, also at exact integer LOD boundaries.
    footprint.log2().max(0.0).floor() as u32 + 1
}

impl GpuCompositorConfig {
    pub(crate) fn set_source_crop(&mut self,plan:crate::video_processing::source_crop::CropSampling,source:[u32;2]) {
        self.source_size=[plan.decode.width,plan.decode.height];
        if self.motion_blur_enabled {
            self.source_texture_size=Some(source);
            self.source_origin=[plan.decode.x,plan.decode.y];
            self.source_uv=std::array::from_fn(|i|(plan.bounds[i]/source[i%2] as f64) as f32);
        } else {
            self.source_texture_size=None;self.source_origin=[0,0];
            self.source_uv=plan.view.map(|value|value as f32);
        }
    }
    pub(crate) fn crop_sampling(&self, crop:crate::video_processing::source_crop::SourceCrop,
        source:[u32;2], cameras:impl IntoIterator<Item=(ZoomFrameState,bool)>, has_source_masks:bool,
    ) -> Result<crate::video_processing::source_crop::CropSampling> {
        if has_source_masks {
            return crop.sampling(source[0],source[1],u32::MAX);
        }
        let logical=crop.geometry(source[0],source[1])?;
        let size=[logical.width as f32,logical.height as f32];
        let output=[self.output_width as f32,self.output_height as f32];
        let mut previous=None;
        let mut last_level=1;
        for (camera,reset) in cameras {
            if reset {previous=None;}
            let (before,now)=motion::shutter_rect(&camera,previous.as_ref(),self);
            last_level=last_level.max(super::shutter_mips::level(size,before,now,output));
            previous=Some(camera);
        }
        crop.sampling_for_mips(source[0],source[1],last_level)
    }
}

#[test]
fn export_crop_plan_preserves_mask_filtering_and_shutter_bounds() {
    let settings:crate::video_processing::ExportSettings=serde_json::from_value(serde_json::json!({
        "resolution":{"width":128,"height":128},
        "visual_settings":{"padding":0,"motion_blur_enabled":true,"motion_blur_zoom_intensity":1.5}
    })).unwrap();
    let mut config=build_gpu_config_with_webcam(&settings,&None,Some(128),Some(128));
    config.window_mode=true;
    let crop=crate::video_processing::source_crop::SourceCrop{x:0.25,y:0.25,width:0.5,height:0.5};
    let cameras=[ZoomFrameState{rect:Some([0.0,0.0,0.9,0.8]),scale:0.9,center_x:0.5,center_y:0.5},
        ZoomFrameState{rect:Some([0.0,0.0,2.1,1.1]),scale:2.1,center_x:0.5,center_y:0.5}];
    let plan=config.crop_sampling(crop,[512,512],cameras.iter().cloned().map(|camera|(camera,false)),false).unwrap();
    assert_eq!([plan.decode.width,plan.decode.height],[512,512]);
    assert_eq!(plan.decode.x%8,0);
    for (source,masks) in [([511,509],false),([512,512],true)] {
        let full=config.crop_sampling(crop,source,cameras.iter().cloned().map(|camera|(camera,false)),masks).unwrap();
        assert_eq!([full.decode.width,full.decode.height],source);
    }
}

#[test]
#[ignore = "requires native GPU; export-selected decode envelope under camera motion"]
fn export_crop_plan_matches_full_source_during_zoom_and_shutter_extrapolation() {
    let settings:crate::video_processing::ExportSettings=serde_json::from_value(serde_json::json!({
        "resolution":{"width":128,"height":128},
        "visual_settings":{"padding":0,"corner_radius":0,"shadow_enabled":false,
            "motion_blur_enabled":true,"motion_blur_zoom_intensity":1.5}
    })).unwrap();
    let crop=crate::video_processing::source_crop::SourceCrop{x:0.2513,y:0.2471,width:0.5,height:0.5};
    let cameras:Vec<_>=[1.0,2.1,0.9,1.4,1.0].into_iter().map(|scale|
        ZoomFrameState{rect:None,scale,center_x:0.45,center_y:0.55}).collect();
    for window in [false,true] {
        let mut config=build_gpu_config_with_webcam(&settings,&None,Some(128),Some(128));
        config.window_mode=window;
        let bounded=config.crop_sampling(crop,[512,512],cameras.iter().cloned().map(|camera|(camera,false)),false).unwrap();
        assert!(bounded.decode.width*bounded.decode.height<512*512);
        let render=|plan:crate::video_processing::source_crop::CropSampling| {
            let raster=plan.decode;
            let source:Vec<_>=(raster.y..raster.y+raster.height).flat_map(|y|
                (raster.x..raster.x+raster.width).flat_map(move |x|
                    [((x*73+y*29)%256) as u8,((x*17+y*103)%256) as u8,((x^y)%256) as u8,255])).collect();
            let mut config=config.clone();
            config.set_source_crop(plan,[512,512]);
            let mut compositor=GpuCompositor::new(config,None,1.0,[1.0;3],None).unwrap();
            cameras.iter().map(|camera|compositor.composite_frame(&source,camera,None).unwrap()).collect::<Vec<_>>()
        };
        let full=render(crop.sampling(512,512,u32::MAX).unwrap());
        for (frame,(actual,expected)) in render(bounded).iter().zip(&full).enumerate() {
            for (byte,(&a,&b)) in actual.iter().zip(expected).enumerate() {
                assert!(a.abs_diff(b)<=2,"window={window}, frame={frame}, byte={byte}: {a} != {b}");
            }
        }
    }
}

impl GpuCompositor {
    pub(super) fn encode_source_filter(&self,encoder:&mut wgpu::CommandEncoder,before:[f32;4],now:[f32;4]) {
        if let Some(pyramid)=&self.source_pyramid {
            let [width,height]=[self.source_texture.width(),self.source_texture.height()].map(|value|value as f32);
            let rect=self.config.source_uv;
            let level=super::shutter_mips::level([width*rect[2],height*rect[3]],before,now,
                [self.config.output_width as f32,self.config.output_height as f32]);
            pyramid.encode_through(encoder,level);
        }
    }

    pub(super) fn prepare_source_filter(&mut self) -> Result<()> {
        let [width, height] = [self.source_texture.width(),self.source_texture.height()];
        let view = self.config.source_uv;
        if self.source_pyramid.is_none()
            && (self.config.motion_blur_enabled || width as f32 * view[2] > self.config.input_width as f32
                || height as f32 * view[3] > self.config.input_height as f32)
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
#[ignore = "native GPU; measures cropped source upload, filtering and compositing"]
fn profile_cropped_source_compositing() {
    for moving in [false,true] {
    let settings:crate::video_processing::ExportSettings=serde_json::from_value(serde_json::json!({
        "resolution":{"width":960,"height":540},
        "visual_settings":{"padding":0,"corner_radius":0,"shadow_enabled":false,"motion_blur_enabled":moving,
            "motion_blur_zoom_intensity":1.0,"motion_blur_pan_intensity":0.5}
    })).unwrap();
    let crop=crate::video_processing::source_crop::SourceCrop{x:0.2513,y:0.2471,width:0.5,height:0.5};
    let mut config=build_gpu_config_with_webcam(&settings,&None,Some(1920),Some(1080));
    config.apply_source_geometry(&settings,(1920.0,1080.0));
    let cameras:Vec<_>=(0..40).map(|frame| {
        let phase=frame as f64/39.0*std::f64::consts::TAU;
        ZoomFrameState{rect:None,scale:if moving {1.25+phase.sin()*0.2}else{1.0},
            center_x:0.5+if moving {phase.cos()*0.1}else{0.0},center_y:0.5}
    }).collect();
    let plans=[crop.sampling(3840,2160,u32::MAX).unwrap(),
        config.crop_sampling(crop,[3840,2160],cameras.iter().cloned().map(|camera|(camera,false)),false).unwrap()];
    let mut runs:Vec<_>=plans.iter().map(|plan| {
        let raster=plan.decode;
        let source:Vec<_>=(raster.y..raster.y+raster.height).flat_map(|y|
            (raster.x..raster.x+raster.width).flat_map(move |x|
                [((x*73+y*29)%256) as u8,((x*17+y*103)%256) as u8,((x^y)%256) as u8,255])).collect();
        let mut config=config.clone();
        config.set_source_crop(*plan,[3840,2160]);
        let mut compositor=GpuCompositor::new(config,None,1.0,[1.0;3],None).unwrap();
        compositor.uniform_buffer=compositor.device.create_buffer(&wgpu::BufferDescriptor {
            label:Some("Profile uniforms"),size:std::mem::size_of::<CompositeUniforms>() as u64,
            usage:wgpu::BufferUsages::UNIFORM|wgpu::BufferUsages::COPY_DST|wgpu::BufferUsages::COPY_SRC,
            mapped_at_creation:false,
        });
        (compositor,source,Vec::new())
    }).collect();
    let mut images=Vec::new();
    for (compositor,source,_) in &mut runs {
        images.push(compositor.composite_frame(source,&cameras[0],None).unwrap());
    }
    assert!(images[0].iter().zip(&images[1]).all(|(a,b)|a.abs_diff(*b)<=2));
    for (iteration,camera) in cameras.iter().enumerate() {
        let mut compared=vec![Vec::new(),Vec::new()];
        for index in if iteration%2==0 {[0,1]} else {[1,0]} {
            let (compositor,source,samples)=&mut runs[index];
            let start=std::time::Instant::now();
            let frame=compositor.composite_frame(source,camera,None).unwrap();
            if iteration>=10 {samples.push(start.elapsed().as_secs_f64()*1000.0);}
            compared[index]=frame;
        }
        let (byte,error)=compared[0].iter().zip(&compared[1]).enumerate()
            .map(|(byte,(a,b))|(byte,a.abs_diff(*b))).max_by_key(|(_,error)|*error).unwrap();
        if error>2 {
            let root=std::path::Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap().join("oss-research");
            for ((name,bytes),(compositor,_,_)) in ["full","bounded"].into_iter().zip(&compared).zip(&runs) {
                std::fs::write(root.join(format!("crop-motion-failure-{name}.rgba")),bytes).unwrap();
                let size=compositor.uniform_buffer.size();
                let readback=compositor.device.create_buffer(&wgpu::BufferDescriptor {
                    label:None,size,usage:wgpu::BufferUsages::MAP_READ|wgpu::BufferUsages::COPY_DST,
                    mapped_at_creation:false,
                });
                let mut encoder=compositor.device.create_command_encoder(&Default::default());
                encoder.copy_buffer_to_buffer(&compositor.uniform_buffer,0,&readback,0,size);
                compositor.queue.submit([encoder.finish()]);
                let (tx,rx)=std::sync::mpsc::channel();
                readback.slice(..).map_async(wgpu::MapMode::Read,move|result|tx.send(result).unwrap());
                compositor.device.poll(wgpu::PollType::wait_indefinitely()).unwrap();
                rx.recv().unwrap().unwrap();
                let data=readback.slice(..).get_mapped_range().unwrap();
                std::fs::write(root.join(format!("crop-motion-failure-{name}.uniforms")),&*data).unwrap();
            }
        }
        assert!(error<=2,"moving={moving}, frame={iteration}, byte={byte}, error={error}, crop={:?}",plans[1]);
    }
    let summaries:Vec<_>=runs.iter_mut().zip(plans).map(|((_,source,samples),plan)| {
        samples.sort_by(f64::total_cmp);
        serde_json::json!({"decode_size":[plan.decode.width,plan.decode.height],"upload_bytes":source.len(),
            "median_ms":samples[samples.len()/2],"samples_ms":samples})
    }).collect();
    let result=serde_json::json!({"source":[3840,2160],"output":[960,540],"moving":moving,
        "scope":"Alternating warmed CPU upload, GPU filter/composite and readback; excludes video decoding and encoding",
        "full":summaries[0],"bounded":summaries[1]});
    let path=std::path::Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap().join(if moving {"oss-research/cropped-motion-profile.json"}else{"oss-research/cropped-source-profile.json"});
    std::fs::write(path,serde_json::to_vec_pretty(&result).unwrap()).unwrap();
    println!("{result}");
}
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
    assert!(compositor.source_pyramid.is_none(),"one-to-one crop should not allocate a minification pyramid");
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

#[test]
#[ignore = "requires FFmpeg and native GPU; fractional crops through timeline decode"]
fn decoded_fractional_crop_preserves_timeline_frames_and_gaps() {
    use crate::video_processing::{source_crop::SourceCrop,timeline::{EditClip,EditTimeline},timeline_decode::video_decode_args_with_crop};
    use std::process::Command;
    let root=std::env::temp_dir().join(format!("fractional-decode-{}",uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&root).unwrap();
    for [width,height] in [[512,512],[511,509]] {
    let input=root.join(format!("source-{width}x{height}.mkv"));
    let generator=format!("nullsrc=s={width}x{height}:r=20:d=1,format=rgb24,geq=r='mod(X*73+Y*29+N*31,256)':g='mod(X*17+Y*103+N*47,256)':b='mod(X+Y+N*61,256)'");
    let generated=Command::new(crate::ffmpeg::binary("ffmpeg")).args(["-v","error","-f","lavfi","-i",
        &generator,
        "-c:v","ffv1"]).arg(&input).output().unwrap();
    assert!(generated.status.success(),"{}",String::from_utf8_lossy(&generated.stderr));
    let clips:Vec<EditClip>=serde_json::from_value(serde_json::json!([
        {"id":"fast","startTime":0,"duration":250,"sourceIn":0,"sourceOut":500,"playbackRate":2,"trackOrder":0,"muted":false},
        {"id":"later","startTime":500,"duration":250,"sourceIn":500,"sourceOut":750,"playbackRate":1,"trackOrder":0,"muted":false}
    ])).unwrap();
    let timeline=EditTimeline::build(&clips,0.0,750.0).unwrap();
    let crop=SourceCrop{x:0.2563,y:0.1991,width:0.51,height:0.63};
    let settings:crate::video_processing::ExportSettings=serde_json::from_value(serde_json::json!({
        "resolution":{"width":128,"height":128},
        "visual_settings":{"padding":0,"corner_radius":0,"shadow_enabled":false,"motion_blur_enabled":false}
    })).unwrap();
    let mut outputs=Vec::new();
    let bounded=crop.sampling_for_mips(width,height,2).unwrap();
    if width%4==0 {assert!(bounded.decode.width<width);}
    for plan in [crop.sampling(width,height,u32::MAX).unwrap(),bounded] {
        let raster=plan.decode;
        let decoded=Command::new(crate::ffmpeg::binary("ffmpeg"))
            .args(video_decode_args_with_crop(&timeline,&input,raster.width,raster.height,20,Some(raster)))
            .output().unwrap();
        assert!(decoded.status.success(),"{}",String::from_utf8_lossy(&decoded.stderr));
        let frame_size=(raster.width*raster.height*4) as usize;
        assert_eq!(decoded.stdout.len(),15*frame_size);
        let mut config=build_gpu_config_with_webcam(&settings,&None,Some(128),Some(128));
        config.source_size=[raster.width,raster.height];
        config.source_uv=plan.view.map(|value|value as f32);
        let mut compositor=GpuCompositor::new(config,None,1.0,[1.0;3],None).unwrap();
        let camera=ZoomFrameState{rect:None,scale:1.0,center_x:0.5,center_y:0.5};
        let frames:Vec<_>=decoded.stdout.chunks_exact(frame_size).enumerate().map(|(index,frame)| {
            let rendered=compositor.composite_frame(frame,&camera,None).unwrap();
            if (5..10).contains(&index) {
                assert!(rendered.chunks_exact(4).all(|pixel|pixel[..3].iter().all(|value|*value<8)),"nonblack gap {index}");
            } else {
                assert!(rendered.chunks_exact(4).any(|pixel|pixel[0]>64),"empty source frame {index}");
            }
            rendered
        }).collect();
        assert_ne!(frames[0],frames[1],"speed-adjusted source frames must advance");
        outputs.push(frames);
    }
    for (full,bounded) in outputs[0].iter().flatten().zip(outputs[1].iter().flatten()) {
        assert!(full.abs_diff(*bounded)<=1);
    }
    }
    std::fs::remove_dir_all(root).unwrap();
}
