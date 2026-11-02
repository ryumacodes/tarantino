use super::*;

#[test]
#[ignore="native GPU; writes temporal window fixtures for real preview comparison"]
fn write_temporal_window_preview_fixtures() {
    let mut fixtures=Vec::new();
    for (width,height) in [(96,64),(97,65)] {for shadow in [false,true] {for opacity in [128u8,255] {for zoom in [false,true] {
        let settings:crate::video_processing::ExportSettings=serde_json::from_value(serde_json::json!({
            "resolution":{"width":width,"height":height},"frame_rate":60,
            "visual_settings":{"padding":20,"corner_radius":8,"shadow_enabled":shadow}
        })).unwrap();
        let mut config=build_gpu_config_with_webcam(&settings,&None,Some(width),Some(height));
        config.window_mode=true;config.background_color=[0.0;4];config.shadow_enabled=shadow;
        config.motion_blur_enabled=true;config.motion_blur_pan_intensity=1.0;config.motion_blur_zoom_intensity=1.0;
        let (iw,ih)=(config.input_width,config.input_height);
        let source:Vec<u8>=(0..ih).flat_map(|y|(0..iw).flat_map(move |x|[((x*13+y*7)%256) as u8,if (x+y)%2==0 {255} else {0},80,opacity])).collect();
        let shadow_pixels:Vec<u8>=(0..height).flat_map(|y|(0..width).flat_map(move |x|[0,0,0,((x+y)%160) as u8])).collect();
        let before=ZoomFrameState {scale:if zoom {1.1} else {1.5},center_x:0.3,center_y:0.5};
        let now=ZoomFrameState {scale:1.5,center_x:0.7,center_y:0.5};
        let (start,end)=motion::shutter_rect(&now,Some(&before),&config);
        let base=[(config.content_offset_x+config.input_offset_x) as f32+iw as f32*0.5,
            (config.content_offset_y+config.input_offset_y) as f32+ih as f32*0.5];
        let radius=config.corner_radius;
        let mut compositor=GpuCompositor::new(config,None,1.0,[1.0;3],None).unwrap();
        compositor.queue.write_texture(wgpu::TexelCopyTextureInfo {texture:&compositor.shadow_texture,mip_level:0,origin:wgpu::Origin3d::ZERO,aspect:wgpu::TextureAspect::All},
            &shadow_pixels,wgpu::TexelCopyBufferLayout {offset:0,bytes_per_row:Some(width*4),rows_per_image:Some(height)},
            wgpu::Extent3d {width,height,depth_or_array_layers:1});
        compositor.set_loupe_target(Some(crate::video_processing::loupe::LoupeTarget {
            center_x:0.5,center_y:0.5,radius:0.3,zoom:2.0,presence:1.0,bevel:0.2,chromatic_aberration:0.5,shadow_alpha:0.0,
        }),(width,height),None,&now).unwrap();
        compositor.composite_frame(&source,&before,None).unwrap();
        compositor.composite_frame(&source,&now,None).unwrap();
        let expected=super::layer_tests::read_texture(&compositor,&compositor.loupe_layers.foreground);
        fixtures.push(serde_json::json!({"width":width,"height":height,"inputSize":[iw,ih],"baseCenter":base,
            "before":start,"now":end,"cornerRadius":radius,"shadowEnabled":shadow,"source":source,"shadow":shadow_pixels,"expected":expected}));
    }}}}
    let path=std::path::Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap().join("oss-research/temporal-window-native.json");
    std::fs::write(path,serde_json::to_vec(&fixtures).unwrap()).unwrap();
}

#[test]
#[ignore="writes native shadow field fixtures for preview comparison"]
fn write_shadow_preview_fixtures() {
    let mut fixtures=Vec::new();
    for (width,height) in [(96,64),(97,65),(18,10)] {for blur in [0,3,60,200] {for intensity in [0.0,0.4,1.0] {
        let settings:crate::video_processing::ExportSettings=serde_json::from_value(serde_json::json!({
            "resolution":{"width":width,"height":height},
            "visual_settings":{"padding":20,"corner_radius":18,"shadow_enabled":true,
                "shadow_blur":blur,"shadow_intensity":intensity*100.0,"shadow_offset_x":-7,"shadow_offset_y":11}
        })).unwrap();
        let config=build_gpu_config_with_webcam(&settings,&None,Some(width),Some(height));
        fixtures.push(serde_json::json!({"width":config.output_width,"height":config.output_height,
            "rect":{"x":(config.content_offset_x+config.input_offset_x) as i32-7,"y":(config.content_offset_y+config.input_offset_y) as i32+11,
                "width":config.input_width,"height":config.input_height},
            "radius":config.corner_radius,"blur":blur,"intensity":intensity,
            "expected":GpuCompositor::shadow_pixels(&config)}));
    }}}
    let path=std::path::Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap().join("oss-research/shadow-preview-native.json");
    std::fs::write(path,serde_json::to_vec(&fixtures).unwrap()).unwrap();
}

#[test]
fn shadow_silhouette_excludes_unused_layout_area() {
    let settings:crate::video_processing::ExportSettings=serde_json::from_value(serde_json::json!({
        "resolution":{"width":96,"height":64},
        "visual_settings":{"padding":20,"shadow_enabled":true,"shadow_blur":0,
            "corner_radius":0,"shadow_intensity":40,"shadow_offset_x":-7,"shadow_offset_y":11}
    })).unwrap();
    let mut config=build_gpu_config_with_webcam(&settings,&None,Some(96),Some(64));
    // A small recording within a desktop layout must not cast a desktop-size shadow.
    config.input_width=20;config.input_height=12;config.input_offset_x=19;config.input_offset_y=14;
    let pixels=GpuCompositor::shadow_pixels(&config);
    let left=config.content_offset_x as i32+19-7;
    let top=config.content_offset_y as i32+14+11;
    for y in 0..64 {for x in 0..96 {
        let expected=if x>=left && x<left+20 && y>=top && y<top+12 {102} else {0};
        assert_eq!(pixels[((y*96+x)*4+3) as usize],expected,"pixel {x},{y}");
    }}
}

#[test]
#[ignore="writes native shutter fixtures for preview timing comparison"]
fn write_camera_shutter_preview_fixtures() {
    let mut fixtures=Vec::new();
    for fps in [24u32,30,60,120] {for rate in [0.5,1.0,2.0] {
        let settings:crate::video_processing::ExportSettings=serde_json::from_value(serde_json::json!({
            "resolution":{"width":96,"height":64},"frame_rate":fps,
            "visual_settings":{"padding":0,"motion_blur_enabled":true,
                "motion_blur_pan_intensity":0.7,"motion_blur_zoom_intensity":0.8}
        })).unwrap();
        let mut config=build_gpu_config_with_webcam(&settings,&None,Some(96),Some(64));config.window_mode=true;
        let time=500.0;
        let camera=|timeline_time:f64| {
            let source_ms=1000.0+(timeline_time-250.0)*rate;
            let index=(source_ms*120.0/1000.0+1e-7).floor();
            ZoomFrameState {scale:1.0+index*index/10000.0,center_x:0.5+index/10000.0,center_y:0.5}
        };
        let now=camera(time);let before=camera(time-1000.0/fps as f64);
        let (start,current)=motion::shutter_rect(&now,Some(&before),&config);
        fixtures.push(serde_json::json!({"fps":fps,"rate":rate,"time":time,"before":start,"now":current}));
    }}
    let path=std::path::Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap().join("oss-research/camera-shutter-native.json");
    std::fs::write(path,serde_json::to_vec(&fixtures).unwrap()).unwrap();
}
