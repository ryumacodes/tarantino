use super::*;
use wgpu::util::DeviceExt;

#[test]
#[ignore="requires native GPU; compares filtered reads to independent f64 sampling"]
fn blurred_mip_reads_match_cpu() {
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
    let shader=device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label:None,source:wgpu::ShaderSource::Wgsl(format!("{}\n{}",SAMPLING_SHADER,r#"
struct Query { uv: vec2<f32>, radius: f32, lod: f32, noise: vec2<f32>, pad: vec2<f32> };
@group(0) @binding(0) var scene: texture_2d<f32>;
@group(0) @binding(1) var filtering: sampler;
@group(0) @binding(2) var<storage,read> queries: array<Query>;
@group(0) @binding(3) var<storage,read_write> results: array<vec4<f32>>;
@compute @workgroup_size(1)
fn main(@builtin(global_invocation_id) id: vec3<u32>) {
    let q=queries[id.x];
    results[id.x]=mip_sample_blurred(scene,filtering,q.uv,q.radius,q.lod,q.noise);
}"#).into()),
    });
    let pipeline=device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
        label:None,layout:None,module:&shader,entry_point:Some("main"),compilation_options:Default::default(),cache:None,
    });
    let sampler=device.create_sampler(&wgpu::SamplerDescriptor {
        mag_filter:wgpu::FilterMode::Linear,min_filter:wgpu::FilterMode::Linear,
        mipmap_filter:wgpu::MipmapFilterMode::Linear,..Default::default()
    });
    let mut queries=Vec::<[f32;8]>::new();
    for radius in [0.0,0.02,0.1,1.0,3.0,8.0,32.0,128.0] {
        for uv in [[0.07,0.13],[0.5,0.5],[0.91,0.87],[-0.1,1.1]] {
            for lod in [0.0,1.25,2.5] {
                for noise in [[0.0,0.0],[0.137,0.731],[0.999,0.999]] {
                    queries.push([uv[0],uv[1],radius,lod,noise[0],noise[1],0.0,0.0]);
                }
            }
        }
    }
    let query_buffer=device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label:None,contents:bytemuck::cast_slice(&queries),usage:wgpu::BufferUsages::STORAGE,
    });
    let result_size=(queries.len()*16) as u64;
    let result_buffer=device.create_buffer(&wgpu::BufferDescriptor {
        label:None,size:result_size,usage:wgpu::BufferUsages::STORAGE|wgpu::BufferUsages::COPY_SRC,mapped_at_creation:false,
    });
    let readback=device.create_buffer(&wgpu::BufferDescriptor {
        label:None,size:result_size,usage:wgpu::BufferUsages::MAP_READ|wgpu::BufferUsages::COPY_DST,mapped_at_creation:false,
    });
    for (width,height) in [(32,32),(35,27),(1,17)] {
        let input=device.create_texture(&wgpu::TextureDescriptor {
            label:None,size:wgpu::Extent3d {width,height,depth_or_array_layers:1},mip_level_count:1,sample_count:1,
            dimension:wgpu::TextureDimension::D2,format:wgpu::TextureFormat::Rgba8Unorm,
            usage:wgpu::TextureUsages::TEXTURE_BINDING|wgpu::TextureUsages::COPY_DST,view_formats:&[],
        });
        let pixels:Vec<u8>=(0..height).flat_map(|y|(0..width).flat_map(move |x|
            [((x*31+y*17)%256) as u8,if (x+y)%2==0 {0} else {255},80,128])).collect();
        queue.write_texture(wgpu::TexelCopyTextureInfo {texture:&input,mip_level:0,origin:wgpu::Origin3d::ZERO,
            aspect:wgpu::TextureAspect::All},&pixels,wgpu::TexelCopyBufferLayout {
                offset:0,bytes_per_row:Some(width*4),rows_per_image:Some(height)},input.size());
        let pyramid=MipPyramid::new(&device,&input).unwrap();
        let binding=device.create_bind_group(&wgpu::BindGroupDescriptor {
            label:None,layout:&pipeline.get_bind_group_layout(0),entries:&[
                wgpu::BindGroupEntry {binding:0,resource:wgpu::BindingResource::TextureView(&pyramid.texture().create_view(&Default::default()))},
                wgpu::BindGroupEntry {binding:1,resource:wgpu::BindingResource::Sampler(&sampler)},
                wgpu::BindGroupEntry {binding:2,resource:query_buffer.as_entire_binding()},
                wgpu::BindGroupEntry {binding:3,resource:result_buffer.as_entire_binding()},
            ],
        });
        let mut encoder=device.create_command_encoder(&Default::default());
        pyramid.encode(&mut encoder);
        {
            let mut pass=encoder.begin_compute_pass(&Default::default());
            pass.set_pipeline(&pipeline);pass.set_bind_group(0,&binding,&[]);
            pass.dispatch_workgroups(queries.len() as u32,1,1);
        }
        encoder.copy_buffer_to_buffer(&result_buffer,0,&readback,0,result_size);
        queue.submit([encoder.finish()]);
        let levels:Vec<_>=(0..pyramid.texture().mip_level_count()).map(|level| {
            let (w,h)=((width>>level).max(1),(height>>level).max(1));
            (w,h,super::tests::read_level(&device,&queue,pyramid.texture(),level,w,h))
        }).collect();
        let slice=readback.slice(..);
        let (tx,rx)=std::sync::mpsc::channel();
        slice.map_async(wgpu::MapMode::Read,move |r|tx.send(r).unwrap());
        device.poll(wgpu::PollType::wait_indefinitely()).unwrap();rx.recv().unwrap().unwrap();
        let data=slice.get_mapped_range().unwrap();
        let actual:&[[f32;4]]=bytemuck::cast_slice(&data);
        for (index,q) in queries.iter().enumerate() {
            let expected=blur(&levels,q.map(f64::from));
            for channel in 0..4 {
                // RGBA8 hardware filtering can quantize the interpolated value.
                assert!((actual[index][channel] as f64-expected[channel]).abs()<1.0/255.0+0.0001,
                    "{width}x{height} query{q:?} channel{channel}: {} vs {}",actual[index][channel],expected[channel]);
            }
        }
        drop(data);readback.unmap();
    }
}

type Level=(u32,u32,Vec<u8>);
fn bilinear(level:&Level,uv:[f64;2])->[f64;4] {
    let (w,h,data)=level;
    let x=uv[0]*f64::from(*w)-0.5;let y=uv[1]*f64::from(*h)-0.5;
    let mut result=[0.0;4];
    for dy in 0..2 {for dx in 0..2 {
        let sx=(x.floor() as i64+dx).clamp(0,i64::from(*w)-1) as usize;
        let sy=(y.floor() as i64+dy).clamp(0,i64::from(*h)-1) as usize;
        let weight=if dx==0 {1.0-(x-x.floor())} else {x-x.floor()}
            *if dy==0 {1.0-(y-y.floor())} else {y-y.floor()};
        for c in 0..4 {result[c]+=f64::from(data[(sy* *w as usize+sx)*4+c])/255.0*weight;}
    }}
    result
}
fn read(levels:&[Level],uv:[f64;2],lod:f64,correct:bool)->[f64;4] {
    let lod=lod.clamp(0.0,(levels.len()-1) as f64);
    let lo=lod.floor() as usize;let hi=(lo+1).min(levels.len()-1);let f=lod.fract();
    let size=[f64::from(levels[0].0),f64::from(levels[0].1)];
    let scale=|l:usize|size.map(|s|s/((s/2f64.powi(l as i32)).floor().max(1.0)*2f64.powi(l as i32)));
    let (s0,s1)=(scale(lo),scale(lo+1));
    let separate=(0..2).any(|i|(s1[i]-s0[i]).abs()>0.01);
    let uv_at=|scale:[f64;2]|std::array::from_fn(|i|if correct {uv[i]*scale[i]} else {uv[i]});
    let common=std::array::from_fn(|i|s0[i]*(1.0-f)+s1[i]*f);
    let a=bilinear(&levels[lo],uv_at(if separate {s0} else {common}));
    let b=bilinear(&levels[hi],uv_at(if separate {s1} else {common}));
    std::array::from_fn(|c|a[c]*(1.0-f)+b[c]*f)
}
fn blur(levels:&[Level],q:[f64;8])->[f64;4] {
    let uv=[q[0],q[1]];let sigma=q[2]*0.5;
    if sigma<=0.02 {return read(levels,uv,q[3],false);}
    let spread=0.6*(1.0-(sigma.log2()-1.0)/3.0).clamp(0.0,1.0);
    let r=0.5+0.4*spread;let a=1.6*spread;
    let variance=1.0/6.0+r*r*(1.0+a*a/12.0)/2.0;
    let mut t2=(sigma*sigma+0.75)/(0.25+variance);
    if t2<4.0 {t2=(sigma*sigma+1.0/12.0)/(1.0/12.0+variance);}
    let t=t2.sqrt();let lod=t.log2().max(q[3]).clamp(0.0,(levels.len()-1) as f64);
    let radius=r*t*(1.0+a*(q[5].fract()-0.5));
    let mut result=[0.0;4];
    for k in 0..3 {
        let angle=(q[4]+k as f64/3.0)*std::f64::consts::TAU;
        let tap=[uv[0]+radius*angle.cos()/f64::from(levels[0].0),uv[1]+radius*angle.sin()/f64::from(levels[0].1)];
        let color=read(levels,tap,lod,true);
        for c in 0..4 {result[c]+=color[c]/3.0;}
    }
    result
}
