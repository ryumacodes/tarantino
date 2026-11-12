use crate::video_processing::loupe::mip_pyramid::MipPyramid;

#[test]
#[ignore = "native GPU; explicit cursor trail gradients in compute"]
fn cursor_trail_gradients_filter_high_frequency_content() {
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
    let texture=device.create_texture(&wgpu::TextureDescriptor {
        label:Some("Cursor checker"),size:wgpu::Extent3d {width:64,height:64,depth_or_array_layers:1},
        mip_level_count:1,sample_count:1,dimension:wgpu::TextureDimension::D2,
        format:wgpu::TextureFormat::Rgba8Unorm,
        usage:wgpu::TextureUsages::COPY_DST|wgpu::TextureUsages::TEXTURE_BINDING,view_formats:&[],
    });
    let pixels=image::RgbaImage::from_fn(64,64,|x,y| {
        let value=if (x+y)%2==0 {0} else {255}; image::Rgba([value,value,value,255])
    });
    queue.write_texture(texture.as_image_copy(),pixels.as_raw(),wgpu::TexelCopyBufferLayout {
        offset:0,bytes_per_row:Some(256),rows_per_image:Some(64),
    },texture.size());
    let pyramid=MipPyramid::new(&device,&texture).unwrap();
    let sampler=device.create_sampler(&wgpu::SamplerDescriptor {
        mag_filter:wgpu::FilterMode::Linear,min_filter:wgpu::FilterMode::Linear,
        mipmap_filter:wgpu::MipmapFilterMode::Linear,anisotropy_clamp:4,..Default::default()
    });
    let shader=device.create_shader_module(wgpu::ShaderModuleDescriptor {label:None,source:wgpu::ShaderSource::Wgsl(r#"
@group(0) @binding(0) var image:texture_2d<f32>;
@group(0) @binding(1) var filtering:sampler;
@group(0) @binding(2) var<storage,read_write> answers:array<vec4<f32>>;
@compute @workgroup_size(1)
fn main(@builtin(global_invocation_id) id:vec3<u32>) {
    let angle=f32(id.x/16u)*0.78539816339;
    let along=vec2<f32>(cos(angle),sin(angle));
    let across=vec2<f32>(-along.y,along.x);
    let uv=vec2<f32>(24.5+f32(id.x%16u),32.5)/64.0;
    let color=textureSampleGrad(image,filtering,uv,along*8.0/64.0,across/64.0);
    answers[id.x]=color;
}
"#.into())});
    let pipeline=device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
        label:None,layout:None,module:&shader,entry_point:Some("main"),compilation_options:Default::default(),cache:None,
    });
    let size=64*16;
    let output=device.create_buffer(&wgpu::BufferDescriptor {label:None,size,
        usage:wgpu::BufferUsages::STORAGE|wgpu::BufferUsages::COPY_SRC,mapped_at_creation:false});
    let readback=device.create_buffer(&wgpu::BufferDescriptor {label:None,size,
        usage:wgpu::BufferUsages::MAP_READ|wgpu::BufferUsages::COPY_DST,mapped_at_creation:false});
    let view=pyramid.texture().create_view(&Default::default());
    let binding=device.create_bind_group(&wgpu::BindGroupDescriptor {label:None,layout:&pipeline.get_bind_group_layout(0),entries:&[
        wgpu::BindGroupEntry {binding:0,resource:wgpu::BindingResource::TextureView(&view)},
        wgpu::BindGroupEntry {binding:1,resource:wgpu::BindingResource::Sampler(&sampler)},
        wgpu::BindGroupEntry {binding:2,resource:output.as_entire_binding()},
    ]});
    let mut encoder=device.create_command_encoder(&Default::default());pyramid.encode(&mut encoder);
    {let mut pass=encoder.begin_compute_pass(&Default::default());pass.set_pipeline(&pipeline);
        pass.set_bind_group(0,&binding,&[]);pass.dispatch_workgroups(64,1,1);}
    encoder.copy_buffer_to_buffer(&output,0,&readback,0,size);queue.submit([encoder.finish()]);
    let (tx,rx)=std::sync::mpsc::channel();let slice=readback.slice(..);
    slice.map_async(wgpu::MapMode::Read,move |result|tx.send(result).unwrap());
    device.poll(wgpu::PollType::wait_indefinitely()).unwrap();rx.recv().unwrap().unwrap();
    let data=slice.get_mapped_range().unwrap();let answers:&[[f32;4]]=bytemuck::cast_slice(&data);
    for (index,color) in answers.iter().enumerate() {
        assert!(color[..3].iter().all(|v|(*v-0.5).abs()<0.01),"sample {index}: {color:?}");
        assert_eq!(color[3],1.0);
    }
}
