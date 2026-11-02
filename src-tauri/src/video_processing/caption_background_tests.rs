use super::*;
#[test]
#[ignore = "requires native GPU"]
fn rounded_background_preserves_alpha_blur_and_cached_shape() {
    check_background(8.0);
}

#[test]
#[ignore = "requires native GPU"]
fn capsule_background_preserves_alpha_blur_and_cached_shape() {
    check_background(12.0);
}

fn check_background(radius: f64) {
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

    let mut background =
        CaptionBackground::new(&device, 48.0, 24.0, [1.0, 1.0, 1.0, 128.0 / 255.0], radius).unwrap();
    assert_eq!(
        (background.shape.width(), background.shape.height()),
        (112, 88)
    );
    let size = wgpu::Extent3d {
        width: 128,
        height: 96,
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
    let readback = device.create_buffer(&wgpu::BufferDescriptor {
        label: None,
        size: 512 * 96,
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });
    let mut outputs = Vec::new();
    for blur_radius in [0.0, 10.0, 0.0] {
        let mut encoder = device.create_command_encoder(&Default::default());
        {
            let view = target.create_view(&Default::default());
            let _clear = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: None,
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
        }
        let part = Part {
            x: 40.0,
            y: 30.0,
            width: 48.0,
            height: 24.0,
            opacity: 0.5,
            blur_radius,
        };
        background
            .encode(&queue, &mut encoder, &target, &part)
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
                    bytes_per_row: Some(512),
                    rows_per_image: Some(96),
                },
            },
            size,
        );
        queue.submit([encoder.finish()]);
        let (tx, rx) = std::sync::mpsc::channel();
        readback
            .slice(..)
            .map_async(wgpu::MapMode::Read, move |r| tx.send(r).unwrap());
        device.poll(wgpu::PollType::wait_indefinitely()).unwrap();
        rx.recv().unwrap().unwrap();
        let mapped = readback.slice(..).get_mapped_range().unwrap();
        outputs.push(mapped.to_vec());
        drop(mapped);
        readback.unmap();
    }
    if radius == 12.0 {
        if let Ok(path) = std::env::var("TARANTINO_CAPTION_BACKGROUND_PIXELS") {
            std::fs::write(path, &outputs[0]).unwrap();
        }
    }
    let pixel = |case: usize, x: usize, y: usize| -> &[u8] {
        &outputs[case][(y * 128 + x) * 4..(y * 128 + x) * 4 + 4]
    };
    assert_eq!(pixel(0, 64, 42), &[64, 64, 64, 70]);
    assert_eq!(pixel(0, 40, 30), &[0, 0, 0, 13]);
    assert_eq!(pixel(0, 38, 42), &[0, 0, 0, 13]);
    assert_eq!(pixel(0, 28, 42), &[0, 0, 0, 0]);
    assert!(
        pixel(1, 28, 42)[3] > 0,
        "Blur must extend beyond the rectangle"
    );
    assert_eq!(
        outputs[0], outputs[2],
        "Cached shape must reproduce the original pixels"
    );
    assert!(outputs.iter().all(|bytes| {
        bytes
            .chunks_exact(4)
            .all(|p| p[..3].iter().all(|c| *c <= p[3]))
    }));
    println!(
        "Rounded caption background: corner/center alpha, blur padding and cached redraw passed"
    );
}
