use super::*;
#[test]
#[ignore = "requires native GPU and browser caption fixture"]
fn renders_browser_caption_tiles_with_explicit_blur() {
    let path =
        std::env::var("TARANTINO_CAPTION_PAYLOAD").expect("set browser caption payload path");
    let input: super::super::caption_overlay::CaptionOverlay =
        serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap();
    let (width, height, frames) = (input.width, input.height, input.frames);
    let prepared =
        super::super::caption_overlay::Prepared::new(input, width, height, frames).unwrap();
    let words = prepared
        .spans
        .iter()
        .find(|s| {
            s.words
                .iter()
                .any(|w| w.part.opacity > 0.4 && w.part.opacity < 0.8)
        })
        .unwrap()
        .words
        .clone();
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
    let mut gpu = CaptionTextGpu::new(&device, &queue, prepared.tiles).unwrap();
    let size = wgpu::Extent3d {
        width,
        height,
        depth_or_array_layers: 1,
    };
    let target = device.create_texture(&wgpu::TextureDescriptor {
        label: None,
        size,
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Rgba8Unorm,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
        view_formats: &[],
    });
    let stride = (width * 4).div_ceil(256) * 256;
    let readback = device.create_buffer(&wgpu::BufferDescriptor {
        label: None,
        size: (stride * height) as u64,
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });
    let mut outputs = Vec::new();
    for explicit in [true, false] {
        let mut sample = words.clone();
        if !explicit {
            for word in &mut sample {
                word.part.blur_radius = 0.0;
            }
        }
        let mut encoder = device.create_command_encoder(&Default::default());
        gpu.encode(&device, &queue, &mut encoder, &target, &sample, true)
            .unwrap();
        encoder.copy_texture_to_buffer(
            wgpu::TexelCopyTextureInfo {
                texture: &target,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            wgpu::TexelCopyBufferInfo {
                buffer: &readback,
                layout: wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(stride),
                    rows_per_image: Some(height),
                },
            },
            size,
        );
        queue.submit([encoder.finish()]);
        let (tx, rx) = std::sync::mpsc::channel();
        readback
            .slice(..)
            .map_async(wgpu::MapMode::Read, move |result| tx.send(result).unwrap());
        device.poll(wgpu::PollType::wait_indefinitely()).unwrap();
        rx.recv().unwrap().unwrap();
        let mapped = readback.slice(..).get_mapped_range().unwrap();
        let pixels = mapped
            .chunks_exact(stride as usize)
            .flat_map(|row| row[..width as usize * 4].iter().copied())
            .collect::<Vec<_>>();
        assert!(pixels.chunks_exact(4).any(|p| p[3] > 0));
        assert!(
            pixels
                .chunks_exact(4)
                .all(|p| p[..3].iter().all(|c| *c <= p[3]))
        );
        outputs.push(pixels);
        drop(mapped);
        readback.unmap();
    }
    let changed = outputs[0]
        .iter()
        .zip(&outputs[1])
        .filter(|(a, b)| a != b)
        .count();
    assert!(changed > 100, "Explicit blur must change glyph pixels");
    println!(
        "Native caption atlas rendered {} words; explicit blur changed {changed} channels",
        words.len()
    );
    let mut invalid = words;
    invalid[0].part.blur_radius = f64::NAN;
    let mut encoder = device.create_command_encoder(&Default::default());
    assert!(
        gpu.encode(&device, &queue, &mut encoder, &target, &invalid, true)
            .is_err()
    );
}
