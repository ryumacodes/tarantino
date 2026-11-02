use super::*;
#[test]
#[ignore="requires native GPU"]
fn masks_cover_sensitive_pixels_and_dim_only_outside_highlights() {
    check_composite(false);
    check_composite(true);
}
fn check_composite(encoded:bool) {
    let instance = wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle());
    let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
        power_preference: wgpu::PowerPreference::HighPerformance,
        compatible_surface: None,
        force_fallback_adapter: false,
        apply_limit_buckets: false,
    }))
    .unwrap();
    let (device, queue) = pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
        label: None,
        required_features: wgpu::Features::empty(),
        required_limits: wgpu::Limits::default(),
        experimental_features: wgpu::ExperimentalFeatures::disabled(),
        memory_hints: wgpu::MemoryHints::Performance,
        trace: wgpu::Trace::Off,
    }))
    .unwrap();

    let texture=|width,height,usage|device.create_texture(&wgpu::TextureDescriptor{label:None,
        size:wgpu::Extent3d{width,height,depth_or_array_layers:1},mip_level_count:1,sample_count:1,
        dimension:wgpu::TextureDimension::D2,format:wgpu::TextureFormat::Rgba8Unorm,usage,view_formats:&[]});
    let scene=texture(64,64,wgpu::TextureUsages::TEXTURE_BINDING|wgpu::TextureUsages::COPY_DST);
    let colors=texture(2,2,wgpu::TextureUsages::TEXTURE_BINDING|wgpu::TextureUsages::COPY_DST);
    let output=texture(64,64,wgpu::TextureUsages::RENDER_ATTACHMENT|wgpu::TextureUsages::COPY_SRC);
    let write=|target:&wgpu::Texture,bytes:&[u8]|queue.write_texture(wgpu::TexelCopyTextureInfo{texture:target,mip_level:0,
        origin:wgpu::Origin3d::ZERO,aspect:wgpu::TextureAspect::All},bytes,
        wgpu::TexelCopyBufferLayout{offset:0,bytes_per_row:Some(target.width()*4),rows_per_image:Some(target.height())},target.size());
    write(&scene,&[40u8,80,120,255].repeat(64*64));
    // Premultiplied encoded red averages must become opaque linear red, not dark red.
    write(&colors,&[128u8,0,0,128].repeat(4));
    let mut composite=MaskComposite::with_encoding(&device,&scene.create_view(&Default::default()),&colors.create_view(&Default::default()),wgpu::TextureFormat::Rgba8Unorm,encoded);
    let readback=device.create_buffer(&wgpu::BufferDescriptor{label:None,size:64*256,
        usage:wgpu::BufferUsages::COPY_DST|wgpu::BufferUsages::MAP_READ,mapped_at_creation:false});
    for alpha in if encoded {vec![255u8,128,0]}else{vec![255u8]} {
    for (sensitive,opacity) in [(true,1.0),(false,1.0),(true,0.0)] {
        let original=[40u8,80,120,alpha];
        let pixels=if opacity==0.0 {(0..64*64*4).map(|i|((i*37+i/13)%256) as u8).collect::<Vec<_>>()}else{original.repeat(64*64)};
        write(&scene,&pixels);
        let mut encoder=device.create_command_encoder(&Default::default());
        composite.encode(&queue,&mut encoder,&output.create_view(&Default::default()),&MaskPlacement{
            rect:[16.0,16.0,32.0,32.0],radius:8.0,opacity,highlight_opacity:0.5,sensitive}).unwrap();
        encoder.copy_texture_to_buffer(wgpu::TexelCopyTextureInfo{texture:&output,mip_level:0,
            origin:wgpu::Origin3d::ZERO,aspect:wgpu::TextureAspect::All},wgpu::TexelCopyBufferInfo{buffer:&readback,
            layout:wgpu::TexelCopyBufferLayout{offset:0,bytes_per_row:Some(256),rows_per_image:Some(64)}},output.size());
        queue.submit([encoder.finish()]);
        let(tx,rx)=std::sync::mpsc::channel();
        readback.slice(..).map_async(wgpu::MapMode::Read,move|r|tx.send(r).unwrap());
        device.poll(wgpu::PollType::wait_indefinitely()).unwrap();rx.recv().unwrap().unwrap();
        let mapped=readback.slice(..).get_mapped_range().unwrap();
        let pixel=|x:usize,y:usize|&mapped[y*256+x*4..y*256+x*4+4];
        if opacity==0.0 {assert_eq!(&mapped[..],&pixels[..]);}
        else if sensitive {
            assert_eq!(pixel(32,32),[255,0,0,255]);assert_eq!(pixel(0,0),original);
            // Entire interior, not just one sample, must lose all original green/blue data.
            for y in 24..40 {for x in 24..40 {assert_eq!(pixel(x,y),[255,0,0,255]);}}
        } else {
            assert_eq!(pixel(32,32),original);
            let expected=if encoded {
                let scale=(f32::from(alpha)/(f32::from(alpha)+255.0)).sqrt();
                [40.0*scale,80.0*scale,120.0*scale,(f32::from(alpha)+255.0)/2.0].map(|v|v.round() as u8)
            }else{[20,40,60,255]};
            assert!(pixel(0,0).iter().zip(expected).all(|(a,b)|a.abs_diff(b)<=1));
        }
        drop(mapped);readback.unmap();
    }
    }
    println!("Sensitive interior opaque, highlight interior unchanged, exterior dimmed, disabled pass exact");
}
