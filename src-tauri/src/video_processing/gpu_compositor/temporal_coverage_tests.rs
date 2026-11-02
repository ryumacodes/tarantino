use wgpu::util::DeviceExt;

#[test]
#[ignore="native GPU; analytic shutter coverage versus independent numerical integration"]
fn temporal_coverage_matches_numerical_integration() {
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
    let source=format!("{}\n{}",include_str!("temporal_coverage.wgsl"),r#"
struct Query { samples:vec4<f32>, settings:vec4<f32> };
@group(0) @binding(0) var<storage,read> queries:array<Query>;
@group(0) @binding(1) var<storage,read_write> answers:array<vec4<f32>>;
@compute @workgroup_size(1)
fn main(@builtin(global_invocation_id) id:vec3<u32>) {
    let q=queries[id.x];
    let p=motion_coverage_ramp(motion_fit_distance(q.samples.x,q.samples.y,q.samples.z),q.settings.x,q.samples.w);
    let full=motion_integrate_coverage(p,q.settings.y,q.settings.z);
    let half=(q.settings.y+q.settings.z)*0.5;
    answers[id.x]=vec4<f32>(full,motion_integrate_coverage(p,q.settings.y,half)+motion_integrate_coverage(p,half,q.settings.z),
        motion_integrate_coverage(p,q.settings.z,q.settings.y),0.0);
}"#);
    let module=device.create_shader_module(wgpu::ShaderModuleDescriptor {label:None,source:wgpu::ShaderSource::Wgsl(source.into())});
    let pipeline=device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
        label:None,layout:None,module:&module,entry_point:Some("main"),compilation_options:Default::default(),cache:None,
    });
    let mut queries=Vec::<[f32;8]>::new();
    // Static inside/outside, linear crossings, curved/grazing edges, fast motion,
    // zero antialias width, partial shutter intervals and inset outlines.
    for samples in [[-2.0,-2.0,-2.0],[2.0,2.0,2.0],[0.0,0.0,0.0],[-4.0,0.0,4.0],
        [4.0,-4.0,4.0],[-4.0,4.0,-4.0],[0.5,0.0,0.5],[1000.0,-0.5,1000.0],
        [-1000.0,0.0,1000.0],[1.0,1.000001,1.000002]] {
        for aa in [0.0,0.5,1.0,4.0] { for inset in [0.0,0.5] { for (start,end) in [(0.0,1.0),(0.13,0.87),(0.49,0.51)] {
            queries.push([samples[0],samples[1],samples[2],aa,inset,start,end,0.0]);
        }}}
    }
    let input=device.create_buffer_init(&wgpu::util::BufferInitDescriptor {label:None,contents:bytemuck::cast_slice(&queries),usage:wgpu::BufferUsages::STORAGE});
    let size=(queries.len()*16) as u64;
    let output=device.create_buffer(&wgpu::BufferDescriptor {label:None,size,usage:wgpu::BufferUsages::STORAGE|wgpu::BufferUsages::COPY_SRC,mapped_at_creation:false});
    let readback=device.create_buffer(&wgpu::BufferDescriptor {label:None,size,usage:wgpu::BufferUsages::COPY_DST|wgpu::BufferUsages::MAP_READ,mapped_at_creation:false});
    let bindings=device.create_bind_group(&wgpu::BindGroupDescriptor {label:None,layout:&pipeline.get_bind_group_layout(0),entries:&[
        wgpu::BindGroupEntry {binding:0,resource:input.as_entire_binding()},wgpu::BindGroupEntry {binding:1,resource:output.as_entire_binding()},
    ]});
    let mut encoder=device.create_command_encoder(&Default::default());
    {let mut pass=encoder.begin_compute_pass(&Default::default());pass.set_pipeline(&pipeline);pass.set_bind_group(0,&bindings,&[]);pass.dispatch_workgroups(queries.len() as u32,1,1);}
    encoder.copy_buffer_to_buffer(&output,0,&readback,0,size);queue.submit([encoder.finish()]);
    let (tx,rx)=std::sync::mpsc::channel();let slice=readback.slice(..);
    slice.map_async(wgpu::MapMode::Read,move |result|tx.send(result).unwrap());device.poll(wgpu::PollType::wait_indefinitely()).unwrap();rx.recv().unwrap().unwrap();
    let data=slice.get_mapped_range().unwrap();let answers:&[[f32;4]]=bytemuck::cast_slice(&data);
    if std::env::var_os("TARANTINO_WRITE_TEMPORAL_FIXTURES").is_some() {
        let path=std::path::Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap()
            .join("oss-research/temporal-coverage-native.json");
        std::fs::write(path,serde_json::to_vec(&serde_json::json!({"queries":queries,"answers":answers})).unwrap()).unwrap();
    }
    for (index,q) in queries.iter().enumerate() {
        // Dense midpoint quadrature evaluates the Lagrange basis directly, independent
        // of the GPU's polynomial coefficients, roots and analytic antiderivative.
        let n=100_000;let width=(q[6]-q[5]) as f64;let mut sum=0.0;
        for i in 0..n {
            let t=q[5] as f64+(i as f64+0.5)*width/n as f64;
            let distance=q[0] as f64*2.0*(t-0.5)*(t-1.0)+q[1] as f64*4.0*t*(1.0-t)+q[2] as f64*2.0*t*(t-0.5);
            sum+=(0.5-(distance-q[4] as f64)/(2.0*q[3] as f64).max(0.000001)).clamp(0.0,1.0);
        }
        let expected=sum*width/n as f64;let actual=answers[index];
        assert!((actual[0] as f64-expected).abs()<0.00025,"query{index} {q:?}: {} vs {expected}",actual[0]);
        assert!((actual[0]-actual[1]).abs()<0.00025,"interval additivity {q:?}");
        assert_eq!(actual[2],0.0);
    }
}
