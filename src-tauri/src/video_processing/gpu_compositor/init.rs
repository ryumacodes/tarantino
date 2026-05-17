use super::*;

impl GpuCompositor {
    /// Create a new GPU compositor with all pipelines and textures initialized.
    ///
    /// Cursor is now SDF-rendered, so `cursor_shape` is only used as a placeholder
    /// to keep the bind group layout stable. All cursor rendering is procedural in the shader.
    pub fn new(
        config: GpuCompositorConfig,
        cursor_shape: Option<&RgbaImage>,
        cursor_size: f32,
        ripple_color: [f32; 3],
        cursor_config: Option<&crate::video_processing::CursorSettings>,
    ) -> Result<Self> {
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
            backends: wgpu::Backends::all(),
            ..Default::default()
        });

        let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::HighPerformance,
            compatible_surface: None,
            force_fallback_adapter: false,
        }))
        .ok_or_else(|| anyhow!("Failed to find a suitable GPU adapter"))?;

        println!("[GPU] Using adapter: {:?}", adapter.get_info().name);

        let (device, queue) = pollster::block_on(adapter.request_device(
            &wgpu::DeviceDescriptor {
                label: Some("Export Compositor"),
                required_features: wgpu::Features::empty(),
                required_limits: wgpu::Limits::default(),
            },
            None,
        ))
        .map_err(|e| anyhow!("Failed to create GPU device: {}", e))?;

        let out_w = config.output_width;
        let out_h = config.output_height;
        let in_w = config.input_width;
        let in_h = config.input_height;
        let unpadded_bytes_per_row = out_w * 4;
        let align = wgpu::COPY_BYTES_PER_ROW_ALIGNMENT;
        let padded_bytes_per_row = (unpadded_bytes_per_row + align - 1) / align * align;
        let frame_size = (padded_bytes_per_row * out_h) as u64;

        // Source texture (video frame input — sized to input, not output)
        let source_texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("Source Video"),
            size: wgpu::Extent3d {
                width: in_w,
                height: in_h,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8Unorm,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });

        // Output texture (composited frame)
        let output_texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("Composited Output"),
            size: wgpu::Extent3d {
                width: out_w,
                height: out_h,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8Unorm,
            usage: wgpu::TextureUsages::STORAGE_BINDING | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        });

        // Staging buffers
        let upload_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Upload Staging"),
            size: frame_size,
            usage: wgpu::BufferUsages::COPY_SRC | wgpu::BufferUsages::MAP_WRITE,
            mapped_at_creation: false,
        });

        let download_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Download Staging"),
            size: frame_size,
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });

        // Uniform buffer
        let uniform_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Uniforms"),
            size: std::mem::size_of::<CompositeUniforms>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        // Trail buffer (30 trail points × 4 floats × 4 bytes = 480 bytes)
        let trail_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Trail Points"),
            size: (30 * std::mem::size_of::<TrailPointGpu>()) as u64,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        // Create cursor shape texture (1x1 placeholder — SDF rendering, no texture needed)
        let (cursor_texture, cursor_has_shape) = if let Some(shape) = cursor_shape {
            let cw = shape.width();
            let ch = shape.height();
            let tex = device.create_texture(&wgpu::TextureDescriptor {
                label: Some("Cursor Shape"),
                size: wgpu::Extent3d {
                    width: cw,
                    height: ch,
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: wgpu::TextureFormat::Rgba8Unorm,
                usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
                view_formats: &[],
            });
            queue.write_texture(
                wgpu::ImageCopyTexture {
                    texture: &tex,
                    mip_level: 0,
                    origin: wgpu::Origin3d::ZERO,
                    aspect: wgpu::TextureAspect::All,
                },
                shape.as_raw(),
                wgpu::ImageDataLayout {
                    offset: 0,
                    bytes_per_row: Some(cw * 4),
                    rows_per_image: Some(ch),
                },
                wgpu::Extent3d {
                    width: cw,
                    height: ch,
                    depth_or_array_layers: 1,
                },
            );
            println!(
                "[GPU] Cursor shape texture: {}x{} ({} bytes)",
                cw,
                ch,
                shape.as_raw().len()
            );
            (tex, true)
        } else {
            // 1x1 placeholder
            let tex = device.create_texture(&wgpu::TextureDescriptor {
                label: Some("Cursor Placeholder"),
                size: wgpu::Extent3d {
                    width: 1,
                    height: 1,
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: wgpu::TextureFormat::Rgba8Unorm,
                usage: wgpu::TextureUsages::TEXTURE_BINDING,
                view_formats: &[],
            });
            (tex, false)
        };

        // Pre-bake corner mask texture
        let corner_mask_texture = Self::create_corner_mask_texture(
            &device,
            &queue,
            config.content_width,
            config.content_height,
            config.corner_radius,
        );

        // Pre-bake shadow texture
        let shadow_texture = Self::create_shadow_texture(&device, &queue, &config);

        // Placeholder webcam texture (1x1 transparent)
        let webcam_texture = None;
        let background_texture = config.background_image.as_ref().map(|image| {
            println!(
                "[Wallpaper Image] creating GPU background texture: {}x{}, {} bytes",
                image.width(),
                image.height(),
                image.as_raw().len()
            );
            Self::create_rgba_texture(
                &device,
                &queue,
                "Background Image",
                image.width(),
                image.height(),
                image.as_raw(),
            )
        });

        // Create shader module
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Composite Shader"),
            source: wgpu::ShaderSource::Wgsl(COMPOSITE_SHADER.into()),
        });

        // Bind group layout
        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("Composite BGL"),
            entries: &[
                // 0: Uniforms
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                // 1: Source texture
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                // 2: Output texture (storage)
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::StorageTexture {
                        access: wgpu::StorageTextureAccess::WriteOnly,
                        format: wgpu::TextureFormat::Rgba8Unorm,
                        view_dimension: wgpu::TextureViewDimension::D2,
                    },
                    count: None,
                },
                // 3: Corner mask texture
                wgpu::BindGroupLayoutEntry {
                    binding: 3,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                // 4: Shadow texture
                wgpu::BindGroupLayoutEntry {
                    binding: 4,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                // 5: Cursor texture
                wgpu::BindGroupLayoutEntry {
                    binding: 5,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                // 6: Webcam texture
                wgpu::BindGroupLayoutEntry {
                    binding: 6,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                // 7: Bilinear sampler
                wgpu::BindGroupLayoutEntry {
                    binding: 7,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
                // 8: Trail points (storage buffer)
                wgpu::BindGroupLayoutEntry {
                    binding: 8,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Storage { read_only: true },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                // 9: Background image texture
                wgpu::BindGroupLayoutEntry {
                    binding: 9,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
            ],
        });

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Composite Pipeline Layout"),
            bind_group_layouts: &[&bind_group_layout],
            push_constant_ranges: &[],
        });

        let composite_pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: Some("Composite Pipeline"),
            layout: Some(&pipeline_layout),
            module: &shader,
            entry_point: "main",
        });

        // Create cached sampler and placeholder texture (reused every frame)
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("Bilinear Sampler"),
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });

        let placeholder_texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("Placeholder"),
            size: wgpu::Extent3d {
                width: 1,
                height: 1,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8Unorm,
            usage: wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        });

        // Extract cursor visual config for SDF rendering
        let cc = cursor_config;
        let cursor_style_str = cc.and_then(|c| c.style.as_deref()).unwrap_or("pointer");
        let cursor_style_val = match cursor_style_str {
            "pointer" => 0.0,
            "circle" => 1.0,
            "filled" => 2.0,
            "outline" => 3.0,
            "dotted" => 4.0,
            _ => 0.0,
        };
        let always_pointer = cc.and_then(|c| c.always_use_pointer).unwrap_or(false);
        let cursor_style_final = if always_pointer {
            0.0
        } else {
            cursor_style_val
        };

        let color_hex = cc.and_then(|c| c.color.as_deref()).unwrap_or("#ffffff");
        let (cr, cg, cb) = crate::cursor_renderer::parse_hex_rgb(color_hex);
        let cursor_color = [cr as f32 / 255.0, cg as f32 / 255.0, cb as f32 / 255.0];

        let hl_hex = cc
            .and_then(|c| c.highlight_color.as_deref())
            .unwrap_or("#ff6b6b");
        let (hr, hg, hb) = crate::cursor_renderer::parse_hex_rgb(hl_hex);
        let cursor_highlight_color = [hr as f32 / 255.0, hg as f32 / 255.0, hb as f32 / 255.0];

        let cursor_shadow_intensity = cc.and_then(|c| c.shadow_intensity).unwrap_or(30.0) as f32;

        let click_effect_str = cc
            .and_then(|c| c.click_effect.as_deref())
            .unwrap_or("ripple");
        let click_effect_val = match click_effect_str {
            "none" => 0.0,
            "circle" => 1.0,
            "ripple" => 2.0,
            _ => 2.0,
        };

        println!("[GPU] Compositor initialized: {}x{}, cursor={}, corner_radius={}, shadow={}, cursor_style={}",
            out_w, out_h, cursor_has_shape, config.corner_radius, config.shadow_enabled, cursor_style_str);

        Ok(Self {
            device,
            queue,
            composite_pipeline,
            bind_group_layout,
            source_texture,
            output_texture,
            upload_buffer,
            download_buffer,
            uniform_buffer,
            trail_buffer,
            cursor_texture,
            cursor_enabled: cursor_has_shape,
            cursor_size,
            ripple_color,
            cursor_style: cursor_style_final,
            cursor_color,
            cursor_highlight_color,
            cursor_shadow_intensity,
            click_effect: click_effect_val,
            webcam_texture,
            background_texture,
            corner_mask_texture,
            shadow_texture,
            sampler,
            placeholder_texture,
            bind_group: None,
            bind_group_dirty: true,
            padded_bytes_per_row,
            config,
            prev_zoom_state: None,
        })
    }
}
