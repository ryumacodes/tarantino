use super::*;
use std::time::Instant;

#[test]
#[ignore = "native GPU benchmark; reports isolated pass cost, not export throughput"]
fn magnifier_pass_cost_at_delivery_resolutions() {
    let instance = wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle());
    let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
        power_preference: wgpu::PowerPreference::HighPerformance,
        compatible_surface: None, force_fallback_adapter: false, apply_limit_buckets: false,
    })).unwrap();
    let timestamps = wgpu::Features::TIMESTAMP_QUERY;
    assert!(adapter.features().contains(timestamps), "benchmark requires GPU timestamps");
    let (device,queue) = pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
        label: Some("Magnifier timing"), required_features: timestamps,
        required_limits: wgpu::Limits::default(),
        experimental_features: wgpu::ExperimentalFeatures::disabled(),
        memory_hints: wgpu::MemoryHints::Performance, trace: wgpu::Trace::Off,
    })).unwrap();
    let query = device.create_query_set(&wgpu::QuerySetDescriptor {
        label: Some("Magnifier pass timestamps"), ty: wgpu::QueryType::Timestamp, count: 2,
    });
    let resolved = device.create_buffer(&wgpu::BufferDescriptor {
        label: None,size:16,usage:wgpu::BufferUsages::QUERY_RESOLVE|wgpu::BufferUsages::COPY_SRC,mapped_at_creation:false,
    });
    let readback = device.create_buffer(&wgpu::BufferDescriptor {
        label: None,size:16,usage:wgpu::BufferUsages::MAP_READ|wgpu::BufferUsages::COPY_DST,mapped_at_creation:false,
    });
    let mut results = Vec::new();
    for (width,height) in [(1920,1080),(3840,2160)] {
        let extent=wgpu::Extent3d {width,height,depth_or_array_layers:1};
        let input=device.create_texture(&wgpu::TextureDescriptor {
            label:Some("Timing source"),size:extent,mip_level_count:1,sample_count:1,
            dimension:wgpu::TextureDimension::D2,format:wgpu::TextureFormat::Rgba8Unorm,
            usage:wgpu::TextureUsages::TEXTURE_BINDING|wgpu::TextureUsages::COPY_DST,view_formats:&[],
        });
        let mut pixels=vec![0u8;(width*height*4) as usize];
        for y in 0..height { for x in 0..width {
            let i=((y*width+x)*4) as usize;
            pixels[i..i+4].copy_from_slice(&[(x%256) as u8,(y%256) as u8,128,255]);
        }}
        queue.write_texture(wgpu::TexelCopyTextureInfo {texture:&input,mip_level:0,
            origin:wgpu::Origin3d::ZERO,aspect:wgpu::TextureAspect::All},&pixels,
            wgpu::TexelCopyBufferLayout {offset:0,bytes_per_row:Some(width*4),rows_per_image:Some(height)},extent);
        let pass=ContentPass::new(&device,&input,(width,height)).unwrap();
        let base=ContentLens {center:[width as f32*0.5,height as f32*0.5],radius:width as f32*0.35,
            zoom:2.0,presence:1.0,..Default::default()};
        let shadow=ContentLens {shadow_alpha:0.35,shadow_reach:width as f32*0.9,
            light_direction:[0.0,-1.0],light_tilt:0.7,..base};
        let curved=ContentLens {bevel:base.radius*0.2,..shadow};
        let spectral=ContentLens {aberration:0.5,..curved};
        let glass=ContentLens {lighting:1.0,reflection:1.0,shine_exponent:8.0,..spectral};
        for (name,lens) in [("flat",base),("shadow",shadow),("curved",curved),("spectral",spectral),("glass",glass)] {
            let mut gpu_ms=Vec::new();
            let mut completion_ms=Vec::new();
            for iteration in 0..50 {
                let started=Instant::now();
                let mut encoder=device.create_command_encoder(&Default::default());
                pass.encode_with_timestamps(&queue,&mut encoder,lens,Some(wgpu::ComputePassTimestampWrites {
                    query_set:&query,beginning_of_pass_write_index:Some(0),end_of_pass_write_index:Some(1),
                })).unwrap();
                encoder.resolve_query_set(&query,0..2,&resolved,0);
                encoder.copy_buffer_to_buffer(&resolved,0,&readback,0,16);
                queue.submit([encoder.finish()]);
                let slice=readback.slice(..);
                let (sender,receiver)=std::sync::mpsc::channel();
                slice.map_async(wgpu::MapMode::Read,move |result|sender.send(result).unwrap());
                device.poll(wgpu::PollType::wait_indefinitely()).unwrap();
                receiver.recv().unwrap().unwrap();
                let completion=started.elapsed().as_secs_f64()*1000.0;
                let data=slice.get_mapped_range().unwrap();
                let start=u64::from_ne_bytes(data[0..8].try_into().unwrap());
                let end=u64::from_ne_bytes(data[8..16].try_into().unwrap());
                let gpu=(end-start) as f64*queue.get_timestamp_period() as f64/1_000_000.0;
                drop(data);
                readback.unmap();
                if iteration>=20 { gpu_ms.push(gpu);completion_ms.push(completion); }
            }
            gpu_ms.sort_by(f64::total_cmp);
            completion_ms.sort_by(f64::total_cmp);
            results.push(serde_json::json!({"width":width,"height":height,"mode":name,
                "gpu_median_ms":gpu_ms[15],"gpu_max_ms":gpu_ms[29],"gpu_sorted_ms":gpu_ms,
                "completion_median_ms":completion_ms[15]}));
        }
    }
    println!("MAGNIFIER_BENCHMARK {}",serde_json::json!({"adapter":format!("{:?}",adapter.get_info()),
        "warmup_frames":20,"measured_frames":30,"scope":"GPU timestamps: content pass only; CPU completion: pyramid + content + queue/readback wait; no decoding/encoding/preview",
        "results":results}));
}
