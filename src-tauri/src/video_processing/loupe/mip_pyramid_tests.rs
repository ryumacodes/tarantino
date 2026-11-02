use super::*;

#[test]
#[ignore="requires native GPU; verifies every mip pixel and resource reuse"]
fn filtered_pyramid_matches_cpu_kernels() {
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
    for (width,height) in [(16,16),(9,7),(1,17),(3,1)] {
        let extent=wgpu::Extent3d {width,height,depth_or_array_layers:1};
        let input=device.create_texture(&wgpu::TextureDescriptor {
            label:None,size:extent,mip_level_count:1,sample_count:1,dimension:wgpu::TextureDimension::D2,
            format:wgpu::TextureFormat::Rgba8Unorm,
            usage:wgpu::TextureUsages::TEXTURE_BINDING|wgpu::TextureUsages::COPY_DST,view_formats:&[],
        });
        let pyramid=MipPyramid::new(&device,&input).unwrap();
        for phase in [0,1] {
            let mut expected=Vec::new();
            for y in 0..height {for x in 0..width {
                expected.extend_from_slice(&[((x*37+y*11+phase*71)%256) as u8,
                    if (x+y+phase)%2==0 {0} else {255},80,128]);
            }}
            queue.write_texture(wgpu::TexelCopyTextureInfo {texture:&input,mip_level:0,
                origin:wgpu::Origin3d::ZERO,aspect:wgpu::TextureAspect::All},&expected,
                wgpu::TexelCopyBufferLayout {offset:0,bytes_per_row:Some(width*4),rows_per_image:Some(height)},extent);
            let mut encoder=device.create_command_encoder(&Default::default());
            pyramid.encode(&mut encoder);
            queue.submit([encoder.finish()]);
            let (mut w,mut h)=(width,height);
            for level in 0..pyramid.texture().mip_level_count() {
                if level>0 {
                    expected=downsample(&expected,w,h,level==1);
                    w=(w/2).max(1);h=(h/2).max(1);
                }
                let pixels=read_level(&device,&queue,pyramid.texture(),level,w,h);
                assert_eq!(pixels.len(),expected.len());
                for (index,(actual,wanted)) in pixels.iter().zip(&expected).enumerate() {
                    assert!(actual.abs_diff(*wanted)<=u8::from(level>0),
                        "{width}x{height} phase{phase} mip{level} byte{index}: {actual} expected {wanted}");
                }
            }
        }
    }
}

fn downsample(input:&[u8],width:u32,height:u32,first:bool)->Vec<u8> {
    let weights:&[f64]=if first {&[0.5,0.5]} else {&[0.125,0.375,0.375,0.125]};
    let start=if first {0} else {-1};
    let mut result=Vec::new();
    for y in 0..(height/2).max(1) {for x in 0..(width/2).max(1) {for channel in 0..4 {
        let mut sum=0.0;
        for (ky,wy) in weights.iter().enumerate() {for (kx,wx) in weights.iter().enumerate() {
            let sx=(x as i32*2+start+kx as i32).clamp(0,width as i32-1) as usize;
            let sy=(y as i32*2+start+ky as i32).clamp(0,height as i32-1) as usize;
            sum+=input[(sy*width as usize+sx)*4+channel] as f64*wx*wy;
        }}
        result.push(sum.round() as u8);
    }}}
    result
}

pub(super) fn read_level(device:&wgpu::Device,queue:&wgpu::Queue,texture:&wgpu::Texture,level:u32,width:u32,height:u32)->Vec<u8> {
    let stride=(width*4).div_ceil(256)*256;
    let buffer=device.create_buffer(&wgpu::BufferDescriptor {label:None,size:(stride*height) as u64,
        usage:wgpu::BufferUsages::COPY_DST|wgpu::BufferUsages::MAP_READ,mapped_at_creation:false});
    let mut encoder=device.create_command_encoder(&Default::default());
    encoder.copy_texture_to_buffer(wgpu::TexelCopyTextureInfo {texture,mip_level:level,
        origin:wgpu::Origin3d::ZERO,aspect:wgpu::TextureAspect::All},wgpu::TexelCopyBufferInfo {
            buffer:&buffer,layout:wgpu::TexelCopyBufferLayout {offset:0,bytes_per_row:Some(stride),rows_per_image:Some(height)},
        },wgpu::Extent3d {width,height,depth_or_array_layers:1});
    queue.submit([encoder.finish()]);
    let slice=buffer.slice(..);
    let (sender,receiver)=std::sync::mpsc::channel();
    slice.map_async(wgpu::MapMode::Read,move |result|sender.send(result).unwrap());
    device.poll(wgpu::PollType::wait_indefinitely()).unwrap();
    receiver.recv().unwrap().unwrap();
    let data=slice.get_mapped_range().unwrap();
    let result=data.chunks_exact(stride as usize).flat_map(|row|row[..(width*4) as usize].iter().copied()).collect();
    drop(data);buffer.unmap();result
}

#[test]
#[ignore="requires native GPU and generated keycap mip fixtures"]
fn keycap_browser_mips_match_native_pyramid() {
    #[derive(serde::Deserialize)]
    struct Level {width:u32,height:u32,pixels:Vec<u8>}
    let path=std::env::var("TARANTINO_KEYCAP_MIP_FIXTURE").expect("Set keycap fixture path");
    let cases:Vec<Vec<Level>>=serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap();
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
    let mut compared=0;let mut maximum=0;
    for levels in cases {
        let first=&levels[0];let extent=wgpu::Extent3d{width:first.width,height:first.height,depth_or_array_layers:1};
        let input=device.create_texture(&wgpu::TextureDescriptor {
            label:None,size:extent,mip_level_count:1,sample_count:1,dimension:wgpu::TextureDimension::D2,
            format:wgpu::TextureFormat::Rgba8Unorm,
            usage:wgpu::TextureUsages::TEXTURE_BINDING|wgpu::TextureUsages::COPY_DST,view_formats:&[],
        });
        queue.write_texture(wgpu::TexelCopyTextureInfo{texture:&input,mip_level:0,origin:wgpu::Origin3d::ZERO,
            aspect:wgpu::TextureAspect::All},&first.pixels,wgpu::TexelCopyBufferLayout{offset:0,
            bytes_per_row:Some(first.width*4),rows_per_image:Some(first.height)},extent);
        let pyramid=MipPyramid::new(&device,&input).unwrap();
        let mut encoder=device.create_command_encoder(&Default::default());pyramid.encode(&mut encoder);queue.submit([encoder.finish()]);
        assert_eq!(levels.len(),pyramid.texture().mip_level_count() as usize);
        for (index,level) in levels.iter().enumerate() {
            let actual=read_level(&device,&queue,pyramid.texture(),index as u32,level.width,level.height);
            for (a,b) in actual.iter().zip(&level.pixels) {
                let difference=a.abs_diff(*b);maximum=maximum.max(difference);compared+=1;
                assert!(difference<=1,"mip{index}: {a} vs {b}");
            }
        }
    }
    println!("Keycap mip comparison: {compared} channels; maximum byte difference {maximum}");
}
