use super::*;
use base64::{Engine as _,engine::general_purpose::STANDARD};
#[test]
#[ignore="requires native GPU and browser pixel fixture"]
fn keycap_gpu_matches_browser_pixels() {
    #[derive(serde::Deserialize)]
    struct Tile {width:u32,height:u32,pixels:Vec<u8>}
    #[derive(serde::Deserialize)]
    struct Fixture {width:u32,height:u32,tiles:Vec<Tile>,draws:Vec<Draw>,expected:String}
    let path=std::env::var("TARANTINO_KEYCAP_GPU_FIXTURE").expect("Set browser fixture path");
    let value:serde_json::Value=serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap();
    let fixtures:Vec<Fixture>=if value.is_array(){serde_json::from_value(value).unwrap()}else{vec![serde_json::from_value(value).unwrap()]};
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
    for (case,fixture) in fixtures.into_iter().enumerate() {
    let rasters=fixture.tiles.into_iter().map(|t|Raster{x:0,y:0,width:t.width,height:t.height,pixels:t.pixels}).collect::<Vec<_>>();
    let mut pass=KeycapGpu::new(&device,&queue,&rasters).unwrap();
    let size=wgpu::Extent3d{width:fixture.width,height:fixture.height,depth_or_array_layers:1};
    let output=device.create_texture(&wgpu::TextureDescriptor{label:None,size,mip_level_count:1,sample_count:1,
        dimension:wgpu::TextureDimension::D2,format:wgpu::TextureFormat::Rgba8Unorm,
        usage:wgpu::TextureUsages::RENDER_ATTACHMENT|wgpu::TextureUsages::COPY_SRC,view_formats:&[]});
    let stride=(fixture.width*4).div_ceil(256)*256;
    let readback=device.create_buffer(&wgpu::BufferDescriptor{label:None,size:(stride*fixture.height) as u64,
        usage:wgpu::BufferUsages::COPY_DST|wgpu::BufferUsages::MAP_READ,mapped_at_creation:false});
    let mut encoder=device.create_command_encoder(&Default::default());
    pass.encode(&device,&queue,&mut encoder,&output,&fixture.draws,true,true).unwrap();
    encoder.copy_texture_to_buffer(wgpu::TexelCopyTextureInfo{texture:&output,mip_level:0,
        origin:wgpu::Origin3d::ZERO,aspect:wgpu::TextureAspect::All},wgpu::TexelCopyBufferInfo{buffer:&readback,
        layout:wgpu::TexelCopyBufferLayout{offset:0,bytes_per_row:Some(stride),rows_per_image:Some(fixture.height)}},size);
    queue.submit([encoder.finish()]);
    let (tx,rx)=std::sync::mpsc::channel();readback.slice(..).map_async(wgpu::MapMode::Read,move|r|tx.send(r).unwrap());
    device.poll(wgpu::PollType::wait_indefinitely()).unwrap();rx.recv().unwrap().unwrap();
    let mapped=readback.slice(..).get_mapped_range().unwrap();
    let actual=mapped.chunks_exact(stride as usize).flat_map(|row|row[..fixture.width as usize*4].iter().copied()).collect::<Vec<_>>();
    let expected=STANDARD.decode(&fixture.expected).unwrap();assert_eq!(actual.len(),expected.len());
    let max=actual.iter().zip(&expected).map(|(a,b)|a.abs_diff(*b)).max().unwrap();
    let changed=actual.iter().zip(&expected).filter(|(a,b)|a!=b).count();
    println!("Case {case} keycap native/browser comparison: {} channels; {changed} differ; maximum {max}",actual.len());
    assert!(max<=2,"Keycap GPU pixel mismatch case {case}: {max}");
    drop(mapped);readback.unmap();
    let base=(0..fixture.width*fixture.height).flat_map(|i|[30,90,150,[0,128,255][i as usize%3]]).collect::<Vec<u8>>();
    let scene=device.create_texture(&wgpu::TextureDescriptor{label:None,size,mip_level_count:1,sample_count:1,
        dimension:wgpu::TextureDimension::D2,format:wgpu::TextureFormat::Rgba8Unorm,
        usage:wgpu::TextureUsages::TEXTURE_BINDING|wgpu::TextureUsages::COPY_DST,view_formats:&[]});
    queue.write_texture(wgpu::TexelCopyTextureInfo{texture:&scene,mip_level:0,origin:wgpu::Origin3d::ZERO,
        aspect:wgpu::TextureAspect::All},&base,wgpu::TexelCopyBufferLayout{offset:0,bytes_per_row:Some(fixture.width*4),
        rows_per_image:Some(fixture.height)},size);
    let mut stage=super::super::shortcut_stage::ShortcutStage::new(&device,&queue,&rasters,fixture.width,fixture.height).unwrap();
    let mut encoder=device.create_command_encoder(&Default::default());
    stage.encode(&device,&queue,&mut encoder,&scene,&fixture.draws).unwrap();
    encoder.copy_texture_to_buffer(wgpu::TexelCopyTextureInfo{texture:stage.output(),mip_level:0,
        origin:wgpu::Origin3d::ZERO,aspect:wgpu::TextureAspect::All},wgpu::TexelCopyBufferInfo{buffer:&readback,
        layout:wgpu::TexelCopyBufferLayout{offset:0,bytes_per_row:Some(stride),rows_per_image:Some(fixture.height)}},size);
    queue.submit([encoder.finish()]);
    let (tx,rx)=std::sync::mpsc::channel();readback.slice(..).map_async(wgpu::MapMode::Read,move|r|tx.send(r).unwrap());
    device.poll(wgpu::PollType::wait_indefinitely()).unwrap();rx.recv().unwrap().unwrap();
    let mapped=readback.slice(..).get_mapped_range().unwrap();let mut blend_max=0;
    for (y,row) in mapped.chunks_exact(stride as usize).enumerate() {
        for (x,pixel) in row[..fixture.width as usize*4].chunks_exact(4).enumerate() {
            let index=(y*fixture.width as usize+x)*4;let key=&actual[index..index+4];let source=&base[index..index+4];
            let alpha=key[3] as f64/255.0;let retained=source[3] as f64/255.0*(1.0-alpha);let total=alpha+retained;
            for c in 0..4 {
                let expected=if alpha==0.0 {source[c]}else if c==3 {(total*255.0).round() as u8}
                    else {((key[c] as f64+source[c] as f64*retained)/total).round() as u8};
                blend_max=blend_max.max(pixel[c].abs_diff(expected));
            }
        }
    }
    assert!(blend_max<=1,"Stage alpha mismatch case{case}: {blend_max}");
    println!("Case {case} straight-alpha stage maximum difference {blend_max}");
    }
}
