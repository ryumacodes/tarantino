use super::*;

#[test]
#[ignore="native GPU; verifies independent premultiplied foreground/backdrop displacement"]
fn layered_magnifier_uses_distinct_bend_coordinates() {
    let instance=wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle());
    let adapter=pollster::block_on(instance.request_adapter(&Default::default())).unwrap();
    let (device,queue)=pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
        required_features:wgpu::Features::empty(),required_limits:wgpu::Limits::default(),
        ..Default::default()
    })).unwrap();
    let (width,height)=(96,64);
    let texture=||device.create_texture(&wgpu::TextureDescriptor {
        label:None,size:wgpu::Extent3d {width,height,depth_or_array_layers:1},mip_level_count:1,sample_count:1,
        dimension:wgpu::TextureDimension::D2,format:wgpu::TextureFormat::Rgba8Unorm,
        usage:wgpu::TextureUsages::TEXTURE_BINDING|wgpu::TextureUsages::COPY_DST,view_formats:&[],
    });
    let (composed,foreground,background)=(texture(),texture(),texture());
    let write=|texture:&wgpu::Texture,pixels:&[u8]|queue.write_texture(wgpu::TexelCopyTextureInfo {
        texture,mip_level:0,origin:wgpu::Origin3d::ZERO,aspect:wgpu::TextureAspect::All},pixels,
        wgpu::TexelCopyBufferLayout {offset:0,bytes_per_row:Some(width*4),rows_per_image:Some(height)},texture.size());
    let pass=ContentPass::new_layered(&device,&composed,&foreground,&background,(width,height)).unwrap();
    let export_pass=ContentPass::new_layered(&device,&composed,&foreground,&background,(width,height)).unwrap().with_straight_output();
    for alpha in [0u8,128,255] {
        let front=[0,alpha/2,0,alpha];
        let mut back=Vec::new();let mut combined=Vec::new();
        for y in 0..height {for x in 0..width {
            let pixel=[x as u8*2,y as u8*2,80,255];back.extend_from_slice(&pixel);
            for c in 0..4 {combined.push((front[c] as f64+pixel[c] as f64*(1.0-alpha as f64/255.0)).round() as u8);}
        }}
        write(&foreground,&front.repeat((width*height) as usize));write(&background,&back);write(&composed,&combined);
        for zoom in [1.0,2.0,4.0] {for bevel in [0.0,8.0] {
            let lens=ContentLens {center:[48.5,32.5],radius:20.0,zoom,bevel,presence:1.0,..Default::default()};
            let pixels=super::tests::render(&device,&queue,&pass,lens,width,height);
            for (radius,bend) in [(0,0.0),(12,0.0),(18,2.2677868),(19,4.2172485)] {
                let bend=if bevel==0.0 {0.0} else {bend};
                for direction in [-1,1] {
                    let x=48+radius*direction;let offset=(32*width as usize+x as usize)*4;
                    let backdrop_x=48.0+direction as f64*(radius as f64/zoom as f64-bend);
                    let expected=[backdrop_x*2.0,64.0,80.0,255.0].map(|v|v*(1.0-alpha as f64/255.0));
                    for c in 0..4 {assert!((pixels[offset+c] as f64-(expected[c]+front[c] as f64)).abs()<=1.1,
                        "alpha{alpha} zoom{zoom} bevel{bevel} radius{radius} side{direction} channel{c}: {} vs {}",pixels[offset+c],expected[c]+front[c] as f64);}
                }
            }
            assert_eq!(&pixels[..4],&combined[..4],"outside must preserve composed input");
        }}
    }
    let front=[20u8,40,60,128];let back=[10u8,20,30,128];
    let combined:[u8;4]=std::array::from_fn(|c|(front[c] as f64+back[c] as f64*(1.0-128.0/255.0)).round() as u8);
    write(&foreground,&front.repeat((width*height) as usize));write(&background,&back.repeat((width*height) as usize));
    write(&composed,&combined.repeat((width*height) as usize));
    let lens=ContentLens {center:[48.5,32.5],radius:20.0,zoom:2.0,presence:1.0,..Default::default()};
    let straight=super::tests::render(&device,&queue,&export_pass,lens,width,height);
    for pixel in straight.chunks_exact(4) {
        assert_eq!(pixel[3],combined[3]);
        for c in 0..3 {assert!((pixel[c] as f64-combined[c] as f64*255.0/combined[3] as f64).abs()<=2.0,
            "export must return straight RGBA, not premultiplied RGB");}
    }

}
