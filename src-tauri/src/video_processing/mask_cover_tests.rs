use super::*;
#[test]
#[ignore="requires native GPU"]
fn averages_quadrants_and_preserves_alpha_on_gpu() {
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

    let source=device.create_texture(&wgpu::TextureDescriptor {
        label:None,size:wgpu::Extent3d{width:8,height:8,depth_or_array_layers:1},mip_level_count:1,sample_count:1,
        dimension:wgpu::TextureDimension::D2,format:wgpu::TextureFormat::Rgba8Unorm,
        usage:wgpu::TextureUsages::TEXTURE_BINDING|wgpu::TextureUsages::COPY_DST,view_formats:&[],
    });
    let colors=[[128u8,0,0,128],[0,255,0,255],[0,0,255,255],[64,64,64,128]];
    let pixels:Vec<u8>=(0..64).flat_map(|i| {
        let quadrant=(i/8/4)*2+(i%8/4);
        if quadrant==3 { let value=if i%2==0 {0} else {128}; [value,value,value,128] }
        else { colors[quadrant] }
    }).collect();
    queue.write_texture(wgpu::TexelCopyTextureInfo{texture:&source,mip_level:0,origin:wgpu::Origin3d::ZERO,aspect:wgpu::TextureAspect::All},
        &pixels,wgpu::TexelCopyBufferLayout{offset:0,bytes_per_row:Some(32),rows_per_image:Some(8)},source.size());
    let cover=MaskCover::new(&device,&source).unwrap();
    let readback=device.create_buffer(&wgpu::BufferDescriptor{label:None,size:512,
        usage:wgpu::BufferUsages::COPY_DST|wgpu::BufferUsages::MAP_READ,mapped_at_creation:false});
    let plan=CoverPlan{lattice:[0.0,0.0,8.0,8.0],level_span:[8.0,8.0],taps:[4,4],level:0};
    let mut encoder=device.create_command_encoder(&Default::default());
    cover.encode(&queue,&mut encoder,&plan).unwrap();
    encoder.copy_texture_to_buffer(wgpu::TexelCopyTextureInfo{texture:cover.texture(),mip_level:0,
        origin:wgpu::Origin3d::ZERO,aspect:wgpu::TextureAspect::All},
        wgpu::TexelCopyBufferInfo{buffer:&readback,layout:wgpu::TexelCopyBufferLayout{offset:0,bytes_per_row:Some(256),rows_per_image:Some(2)}},cover.texture().size());
    queue.submit([encoder.finish()]);
    let(tx,rx)=std::sync::mpsc::channel();
    readback.slice(..).map_async(wgpu::MapMode::Read,move|r|tx.send(r).unwrap());
    device.poll(wgpu::PollType::wait_indefinitely()).unwrap();rx.recv().unwrap().unwrap();
    let mapped=readback.slice(..).get_mapped_range().unwrap();
    for (i,color) in colors.iter().enumerate(){let start=(i/2)*256+(i%2)*4;assert_eq!(&mapped[start..start+4],color);}
    drop(mapped);readback.unmap();
    let mut encoder=device.create_command_encoder(&Default::default());
    assert!(cover.encode(&queue,&mut encoder,&CoverPlan{taps:[256,256],..plan}).is_err());
    println!("GPU quadrant colors and premultiplied alpha matched; excessive sample budget rejected");
}
