//! GPU compositor for video preview
//!
//! Uses wgpu for hardware-accelerated rendering of video frames with
//! zoom effects, camera PIP, and cursor compositing.

use anyhow::Result;
use std::sync::Arc;
use wgpu::*;

use super::textures;
use super::uniforms::{BackgroundUniforms, CameraUniforms, ZoomUniforms};
use super::{CompositorFrame, PreviewOptions};

// Types available via super::types for callers that need them
pub use super::types::ResolvedFrameData;

/// GPU compositor using wgpu for hardware-accelerated rendering
///
/// Rendering pipeline: Background → Display → Camera PIP → Cursor → Effects
pub struct WgpuCompositor {
    device: Arc<Device>,
    queue: Arc<Queue>,

    // Render pipeline and shaders
    render_pipeline: RenderPipeline,

    // Textures and buffers
    display_texture: Option<Texture>,
    camera_texture: Option<Texture>,
    cursor_texture: Option<Texture>,
    output_texture: Texture,

    // Uniform buffers for shader parameters
    camera_uniform_buffer: Buffer,
    zoom_uniform_buffer: Buffer,
    background_uniform_buffer: Buffer,

    // Bind groups for textures and uniforms
    uniform_bind_group: BindGroup,

    // Current options
    current_options: PreviewOptions,

    // Output dimensions
    output_width: u32,
    output_height: u32,
}

impl WgpuCompositor {
    pub async fn new(device: Arc<Device>, queue: Arc<Queue>) -> Result<Self> {
        let (output_width, output_height) = (1920, 1080);

        // Create shader module
        let shader = device.create_shader_module(ShaderModuleDescriptor {
            label: Some("Tarantino Compositor Shader"),
            source: ShaderSource::Wgsl(include_str!("shaders/compositor.wgsl").into()),
        });

        // Create bind group layout
        let bind_group_layout = create_bind_group_layout(&device);

        let pipeline_layout = device.create_pipeline_layout(&PipelineLayoutDescriptor {
            label: Some("Compositor Pipeline Layout"),
            bind_group_layouts: &[&bind_group_layout],
            push_constant_ranges: &[],
        });

        // Create render pipeline
        let render_pipeline = device.create_render_pipeline(&RenderPipelineDescriptor {
            label: Some("Compositor Render Pipeline"),
            layout: Some(&pipeline_layout),
            vertex: VertexState {
                module: &shader,
                entry_point: "vs_main",
                buffers: &[],
            },
            fragment: Some(FragmentState {
                module: &shader,
                entry_point: "fs_main",
                targets: &[Some(ColorTargetState {
                    format: TextureFormat::Rgba8UnormSrgb,
                    blend: Some(BlendState::ALPHA_BLENDING),
                    write_mask: ColorWrites::ALL,
                })],
            }),
            primitive: PrimitiveState {
                topology: PrimitiveTopology::TriangleStrip,
                strip_index_format: None,
                front_face: FrontFace::Ccw,
                cull_mode: None,
                unclipped_depth: false,
                polygon_mode: PolygonMode::Fill,
                conservative: false,
            },
            depth_stencil: None,
            multisample: MultisampleState::default(),
            multiview: None,
        });

        // Create output texture
        let output_texture = textures::create_output_texture(&device, output_width, output_height);

        // Create uniform buffers
        let camera_uniform_buffer = device.create_buffer(&BufferDescriptor {
            label: Some("Camera Uniforms"),
            size: std::mem::size_of::<CameraUniforms>() as u64,
            usage: BufferUsages::UNIFORM | BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let zoom_uniform_buffer = device.create_buffer(&BufferDescriptor {
            label: Some("Zoom Uniforms"),
            size: std::mem::size_of::<ZoomUniforms>() as u64,
            usage: BufferUsages::UNIFORM | BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let background_uniform_buffer = device.create_buffer(&BufferDescriptor {
            label: Some("Background Uniforms"),
            size: std::mem::size_of::<BackgroundUniforms>() as u64,
            usage: BufferUsages::UNIFORM | BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        // Create sampler
        let sampler = device.create_sampler(&SamplerDescriptor {
            address_mode_u: AddressMode::ClampToEdge,
            address_mode_v: AddressMode::ClampToEdge,
            address_mode_w: AddressMode::ClampToEdge,
            mag_filter: FilterMode::Linear,
            min_filter: FilterMode::Linear,
            mipmap_filter: FilterMode::Nearest,
            ..Default::default()
        });

        // Create placeholder texture
        let placeholder_texture = textures::create_placeholder_texture(&device, &queue);
        let placeholder_view = placeholder_texture.create_view(&TextureViewDescriptor::default());

        // Create initial bind group with placeholder textures
        let uniform_bind_group = device.create_bind_group(&BindGroupDescriptor {
            label: Some("Compositor Bind Group"),
            layout: &bind_group_layout,
            entries: &[
                BindGroupEntry {
                    binding: 0,
                    resource: BindingResource::TextureView(&placeholder_view),
                },
                BindGroupEntry {
                    binding: 1,
                    resource: BindingResource::Sampler(&sampler),
                },
                BindGroupEntry {
                    binding: 2,
                    resource: BindingResource::TextureView(&placeholder_view),
                },
                BindGroupEntry {
                    binding: 3,
                    resource: BindingResource::Sampler(&sampler),
                },
                BindGroupEntry {
                    binding: 4,
                    resource: camera_uniform_buffer.as_entire_binding(),
                },
                BindGroupEntry {
                    binding: 5,
                    resource: zoom_uniform_buffer.as_entire_binding(),
                },
                BindGroupEntry {
                    binding: 6,
                    resource: background_uniform_buffer.as_entire_binding(),
                },
            ],
        });

        Ok(Self {
            device,
            queue,
            render_pipeline,
            display_texture: None,
            camera_texture: None,
            cursor_texture: None,
            output_texture,
            camera_uniform_buffer,
            zoom_uniform_buffer,
            background_uniform_buffer,
            uniform_bind_group,
            current_options: PreviewOptions::default(),
            output_width,
            output_height,
        })
    }

    /// Render a frame using the resolved frame data
    pub async fn render_frame(
        &mut self,
        frame_data: &ResolvedFrameData,
        options: &PreviewOptions,
    ) -> Result<CompositorFrame> {
        // Update options if changed
        if !self.options_equal(options) {
            self.update_options(options.clone()).await?;
        }

        // Update textures with frame data
        self.update_textures(frame_data).await?;

        // Update uniform buffers
        self.update_uniforms(frame_data, options);

        // Create command encoder
        let mut encoder = self
            .device
            .create_command_encoder(&CommandEncoderDescriptor {
                label: Some("Compositor Render"),
            });

        // Render pass
        {
            let output_view = self
                .output_texture
                .create_view(&TextureViewDescriptor::default());

            let mut render_pass = encoder.begin_render_pass(&RenderPassDescriptor {
                label: Some("Compositor Render Pass"),
                color_attachments: &[Some(RenderPassColorAttachment {
                    view: &output_view,
                    resolve_target: None,
                    ops: Operations {
                        load: LoadOp::Clear(Color {
                            r: 0.0,
                            g: 0.0,
                            b: 0.0,
                            a: 1.0,
                        }),
                        store: StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                occlusion_query_set: None,
                timestamp_writes: None,
            });

            render_pass.set_pipeline(&self.render_pipeline);
            render_pass.set_bind_group(0, &self.uniform_bind_group, &[]);

            // Draw full-screen quad (using triangle strip)
            render_pass.draw(0..4, 0..1);
        }

        // Submit commands
        self.queue.submit(std::iter::once(encoder.finish()));

        // Read back the rendered frame
        let texture_data = textures::read_output_texture(
            &self.device,
            &self.queue,
            &self.output_texture,
            self.output_width,
            self.output_height,
        )
        .await?;

        Ok(CompositorFrame {
            timestamp_ms: frame_data.timestamp_ms,
            texture_data,
            width: self.output_width,
            height: self.output_height,
        })
    }

    /// Update compositor options
    pub async fn update_options(&mut self, options: PreviewOptions) -> Result<()> {
        self.current_options = options;

        // Resize output texture if needed
        if self.current_options.base_resolution != (self.output_width, self.output_height) {
            self.resize_output(self.current_options.base_resolution);
        }

        Ok(())
    }

    /// Update textures with new frame data
    async fn update_textures(&mut self, frame_data: &ResolvedFrameData) -> Result<()> {
        // Update display texture
        if let Some(display_frame) = &frame_data.display_frame {
            textures::update_frame_texture(
                &self.device,
                &self.queue,
                &mut self.display_texture,
                display_frame,
                "Display Texture",
            )?;
        }

        // Update camera texture
        if let Some(camera_frame) = &frame_data.camera_frame {
            textures::update_frame_texture(
                &self.device,
                &self.queue,
                &mut self.camera_texture,
                camera_frame,
                "Camera Texture",
            )?;
        }

        // Update cursor texture
        if let Some(cursor_data) = &frame_data.cursor_data {
            textures::update_cursor_texture(
                &self.device,
                &self.queue,
                &mut self.cursor_texture,
                cursor_data,
            )?;
        }

        Ok(())
    }

    /// Update uniform buffers with current frame parameters
    fn update_uniforms(&mut self, frame_data: &ResolvedFrameData, options: &PreviewOptions) {
        // Update camera uniforms
        let camera_uniforms = CameraUniforms::from_options(
            &options.camera_pip,
            self.output_width,
            self.output_height,
        );
        self.queue.write_buffer(
            &self.camera_uniform_buffer,
            0,
            bytemuck::cast_slice(&[camera_uniforms]),
        );

        // Update zoom uniforms
        let zoom_uniforms = ZoomUniforms::from_frame_data(frame_data);
        self.queue.write_buffer(
            &self.zoom_uniform_buffer,
            0,
            bytemuck::cast_slice(&[zoom_uniforms]),
        );

        // Update background uniforms
        let background_uniforms = BackgroundUniforms::new(options.background_blur);
        self.queue.write_buffer(
            &self.background_uniform_buffer,
            0,
            bytemuck::cast_slice(&[background_uniforms]),
        );
    }

    /// Resize output texture
    fn resize_output(&mut self, new_size: (u32, u32)) {
        self.output_width = new_size.0;
        self.output_height = new_size.1;

        self.output_texture =
            textures::create_output_texture(&self.device, self.output_width, self.output_height);

        println!(
            "Compositor output resized to {}x{}",
            self.output_width, self.output_height
        );
    }

    /// Check if options are equal (to avoid unnecessary updates)
    fn options_equal(&self, other: &PreviewOptions) -> bool {
        self.current_options.base_resolution == other.base_resolution
            && (self.current_options.background_blur - other.background_blur).abs() < 0.001
    }
}

/// Create the bind group layout for the compositor
fn create_bind_group_layout(device: &Device) -> BindGroupLayout {
    device.create_bind_group_layout(&BindGroupLayoutDescriptor {
        label: Some("Compositor Bind Group Layout"),
        entries: &[
            // Display texture
            BindGroupLayoutEntry {
                binding: 0,
                visibility: ShaderStages::FRAGMENT,
                ty: BindingType::Texture {
                    multisampled: false,
                    view_dimension: TextureViewDimension::D2,
                    sample_type: TextureSampleType::Float { filterable: true },
                },
                count: None,
            },
            // Display sampler
            BindGroupLayoutEntry {
                binding: 1,
                visibility: ShaderStages::FRAGMENT,
                ty: BindingType::Sampler(SamplerBindingType::Filtering),
                count: None,
            },
            // Camera texture
            BindGroupLayoutEntry {
                binding: 2,
                visibility: ShaderStages::FRAGMENT,
                ty: BindingType::Texture {
                    multisampled: false,
                    view_dimension: TextureViewDimension::D2,
                    sample_type: TextureSampleType::Float { filterable: true },
                },
                count: None,
            },
            // Camera sampler
            BindGroupLayoutEntry {
                binding: 3,
                visibility: ShaderStages::FRAGMENT,
                ty: BindingType::Sampler(SamplerBindingType::Filtering),
                count: None,
            },
            // Camera uniforms
            BindGroupLayoutEntry {
                binding: 4,
                visibility: ShaderStages::FRAGMENT,
                ty: BindingType::Buffer {
                    ty: BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            },
            // Zoom uniforms
            BindGroupLayoutEntry {
                binding: 5,
                visibility: ShaderStages::FRAGMENT,
                ty: BindingType::Buffer {
                    ty: BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            },
            // Background uniforms
            BindGroupLayoutEntry {
                binding: 6,
                visibility: ShaderStages::FRAGMENT,
                ty: BindingType::Buffer {
                    ty: BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            },
        ],
    })
}
