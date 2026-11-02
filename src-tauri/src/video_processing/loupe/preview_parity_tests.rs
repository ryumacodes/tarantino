use super::*;

/// Produces native pixel fixtures for the browser harness. Intentionally manual:
/// native GPU access and a running WebGL harness are required for the comparison.
#[test]
#[ignore="native GPU fixture generation for browser pixel comparison"]
fn write_preview_pixel_fixtures() {
    let instance=wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle());
    let adapter=pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
        power_preference:wgpu::PowerPreference::HighPerformance,compatible_surface:None,
        force_fallback_adapter:false,apply_limit_buckets:false,
    })).unwrap();
    let (device,queue)=pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
        label:None,required_features:wgpu::Features::empty(),required_limits:wgpu::Limits::default(),
        experimental_features:wgpu::ExperimentalFeatures::disabled(),memory_hints:wgpu::MemoryHints::Performance,
        trace:wgpu::Trace::Off,
    })).unwrap();
    let mut fixtures=Vec::new();
    for (width,height) in [(96,64),(97,65),(64,96),(1,17)] {
        let texture=||device.create_texture(&wgpu::TextureDescriptor {
            label:None,size:wgpu::Extent3d {width,height,depth_or_array_layers:1},mip_level_count:1,sample_count:1,
            dimension:wgpu::TextureDimension::D2,format:wgpu::TextureFormat::Rgba8Unorm,
            usage:wgpu::TextureUsages::TEXTURE_BINDING|wgpu::TextureUsages::COPY_DST,view_formats:&[],
        });
        let (input,foreground,background)=(texture(),texture(),texture());
        let original:Vec<u8>=(0..height).flat_map(|y|(0..width).flat_map(move |x|
            [((x*37+y*11)%128) as u8,if (x+y)%2==0 {0} else {128},80,128])).collect();
        let write=|texture:&wgpu::Texture,pixels:&[u8]|queue.write_texture(wgpu::TexelCopyTextureInfo {
            texture,mip_level:0,origin:wgpu::Origin3d::ZERO,aspect:wgpu::TextureAspect::All},pixels,
            wgpu::TexelCopyBufferLayout {offset:0,bytes_per_row:Some(width*4),rows_per_image:Some(height)},texture.size());
        for layered in [false,true] {
            let mut front=original.clone();
            for y in 0..height {for x in 0..width/2 {let i=((y*width+x)*4) as usize;front[i..i+4].fill(0);}}
            let back:Vec<u8>=(0..height).flat_map(|y|(0..width).flat_map(move |x|[x as u8*2,y as u8*2,80,255])).collect();
            let pixels=if layered {front.chunks_exact(4).zip(back.chunks_exact(4)).flat_map(|(f,b)|
                (0..4).map(move |c|(f[c] as f64+b[c] as f64*(1.0-f[3] as f64/255.0)).round() as u8)).collect()}
                else {original.clone()};
            write(&input,&pixels);write(&foreground,&front);write(&background,&back);
            let pass=if layered {ContentPass::new_layered(&device,&input,&foreground,&background,(width,height))}
                else {ContentPass::new(&device,&input,(width,height))}.unwrap();
        for aberration in [0.0,0.5] {for lighting in [0.0,1.0] {
            let lens=ContentLens {center:[width as f32*0.5,height as f32*0.5],radius:20.0,zoom:2.0,
                presence:1.0,bevel:8.0,reflection:1.0,aberration,lighting,
                shadow_alpha:0.35,shadow_reach:40.0,light_tilt:1.0,..Default::default()};
            let output=super::tests::render(&device,&queue,&pass,lens,width,height);
            fixtures.push(serde_json::json!({"width":width,"height":height,"aberration":aberration,"lighting":lighting,"layered":layered,"foreground":front,"background":back,"input":pixels,"output":output}));
        }}
        }
    }
    let path=std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../oss-research/loupe-preview-native.json");
    std::fs::write(path,serde_json::to_vec(&fixtures).unwrap()).unwrap();
}
