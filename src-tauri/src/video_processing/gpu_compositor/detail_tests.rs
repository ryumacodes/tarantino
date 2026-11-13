use super::*;

#[test]
#[ignore = "native GPU; verifies enhanced source binding, blend and reset"]
fn detail_texture_reaches_composite_pixels_and_resets() {
    let settings=serde_json::from_value(serde_json::json!({
        "resolution":{"width":128,"height":128},"capture_mode":"window",
        "visual_settings":{"padding":0,"corner_radius":0,"shadow_enabled":false}
    })).unwrap();
    let mut config=build_gpu_config_with_webcam(&settings,&None,Some(128),Some(128));
    config.motion_blur_enabled=false;
    let source=[0,0,255,255].repeat((config.input_width*config.input_height)as usize);
    let mut compositor=GpuCompositor::new(config,None,1.0,[1.0;3],None).unwrap();
    let texture=compositor.device.create_texture(&wgpu::TextureDescriptor {
        label:None,size:wgpu::Extent3d{width:256,height:256,depth_or_array_layers:1},
        mip_level_count:1,sample_count:1,dimension:wgpu::TextureDimension::D2,
        format:wgpu::TextureFormat::Rgba8Unorm,
        usage:wgpu::TextureUsages::TEXTURE_BINDING|wgpu::TextureUsages::COPY_DST,view_formats:&[] });
    compositor.queue.write_texture(texture.as_image_copy(),&[255,0,0,255].repeat(256*256),
        wgpu::TexelCopyBufferLayout{offset:0,bytes_per_row:Some(1024),rows_per_image:Some(256)},texture.size());
    let mut camera=ZoomFrameState{rect:None,scale:2.0,center_x:0.5,center_y:0.5};
    for window in [true,false] {
    compositor.config.window_mode=window;
    compositor.config.motion_blur_enabled=true;
    compositor.reset_motion_history();
    camera.center_x=0.5;
    for (enabled,strength,expected) in [(false,0.0,[0u8,0,255]),(true,0.5,[128,0,128]),
        (true,0.1875,[0,0,255]),(true,1.0,[255,0,0]),(false,0.0,[0,0,255])] {
        compositor.set_detail_texture(enabled.then_some(&texture),[0.0,0.0,1.0,1.0],strength).unwrap();
        camera.center_x+=0.01;
        let pixels=compositor.composite_frame(&source,&camera,None).unwrap();
        for (actual,expected) in pixels[(64*128+64)*4..][..3].iter().zip(expected) {
            assert!(actual.abs_diff(expected)<=1,"enabled={enabled}, strength={strength}, actual={actual}, expected={expected}");
        }
    }
    }
}
