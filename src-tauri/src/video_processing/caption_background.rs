//! Bounded reusable caption background, filtered entirely on the GPU.
use super::{
    caption_overlay::Part,
    loupe::mip_pyramid::{MipPyramid, SAMPLING_SHADER},
};
use anyhow::{Result, ensure};
const PAD: f32 = 32.0;
pub(super) struct CaptionBackground {
    shape: wgpu::Texture,
    pyramid: MipPyramid,
    shape_pipeline: wgpu::RenderPipeline,
    blur_pipeline: wgpu::RenderPipeline,
    shape_uniform: wgpu::Buffer,
    blur_uniform: wgpu::Buffer,
    shape_binding: wgpu::BindGroup,
    blur_binding: wgpu::BindGroup,
    color: [f32; 4],
    radius: f32,
    cached: Option<[u64; 3]>,
}
fn pipeline(device: &wgpu::Device, code: String, blend: bool) -> wgpu::RenderPipeline {
    let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("Caption background"),
        source: wgpu::ShaderSource::Wgsl(code.into()),
    });
    device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some("Caption background"),
        layout: None,
        vertex: wgpu::VertexState {
            module: &shader,
            entry_point: Some("vertex"),
            compilation_options: Default::default(),
            buffers: &[],
        },
        primitive: wgpu::PrimitiveState {
            topology: wgpu::PrimitiveTopology::TriangleStrip,
            ..Default::default()
        },
        depth_stencil: None,
        multisample: Default::default(),
        fragment: Some(wgpu::FragmentState {
            module: &shader,
            entry_point: Some("fragment"),
            compilation_options: Default::default(),
            targets: &[Some(wgpu::ColorTargetState {
                format: wgpu::TextureFormat::Rgba8Unorm,
                blend: blend.then_some(wgpu::BlendState::PREMULTIPLIED_ALPHA_BLENDING),
                write_mask: wgpu::ColorWrites::ALL,
            })],
        }),
        multiview_mask: None,
        cache: None,
    })
}
impl CaptionBackground {
    pub fn new(
        device: &wgpu::Device,
        max_width: f64,
        max_height: f64,
        color: [f32; 4],
        radius: f64,
    ) -> Result<Self> {
        ensure!(
            [max_width, max_height, radius]
                .iter()
                .all(|n| n.is_finite() && *n >= 0.0)
                && color
                    .iter()
                    .all(|n| n.is_finite() && (0.0..=1.0).contains(n)),
            "Invalid caption background"
        );
        let width = (max_width.ceil() + f64::from(PAD * 2.0)).max(1.0);
        let height = (max_height.ceil() + f64::from(PAD * 2.0)).max(1.0);
        ensure!(
            width <= device.limits().max_texture_dimension_2d as f64
                && height <= device.limits().max_texture_dimension_2d as f64
                && width * height * 4.0 <= 128.0 * 1024.0 * 1024.0,
            "Caption background exceeds GPU texture limit"
        );
        let shape = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("Caption background bounds"),
            size: wgpu::Extent3d {
                width: width as u32,
                height: height as u32,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8Unorm,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        });
        let pyramid = MipPyramid::new(device, &shape)?;
        let shape_pipeline = pipeline(
            device,
            include_str!("caption_background.wgsl").into(),
            false,
        );
        let blur_pipeline = pipeline(
            device,
            format!(
                "{}\n{}\n{}",
                SAMPLING_SHADER,
                include_str!("loupe/blue_noise.wgsl"),
                include_str!("shortcut_gpu.wgsl")
            ),
            true,
        );
        let uniform = |size| {
            device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("Caption background placement"),
                size,
                usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            })
        };
        let shape_uniform = uniform(112);
        let blur_uniform = uniform(32);
        let shape_binding = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: None,
            layout: &shape_pipeline.get_bind_group_layout(0),
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: shape_uniform.as_entire_binding(),
            }],
        });
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::MipmapFilterMode::Linear,
            ..Default::default()
        });
        let blur_binding = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: None,
            layout: &blur_pipeline.get_bind_group_layout(0),
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(
                        &pyramid.texture().create_view(&Default::default()),
                    ),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&sampler),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: blur_uniform.as_entire_binding(),
                },
            ],
        });
        Ok(Self {
            shape,
            pyramid,
            shape_pipeline,
            blur_pipeline,
            shape_uniform,
            blur_uniform,
            shape_binding,
            blur_binding,
            color,
            radius: radius as f32,
            cached: None,
        })
    }
    pub fn encode(
        &mut self,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        target: &wgpu::Texture,
        part: &Part,
    ) -> Result<()> {
        ensure!(
            target.format() == wgpu::TextureFormat::Rgba8Unorm
                && target
                    .usage()
                    .contains(wgpu::TextureUsages::RENDER_ATTACHMENT),
            "Invalid caption background target"
        );
        ensure!(
            [
                part.x,
                part.y,
                part.width,
                part.height,
                part.opacity,
                part.blur_radius
            ]
            .iter()
            .all(|n| n.is_finite())
                && part.width >= 0.0
                && part.height >= 0.0
                && part.width + f64::from(PAD * 2.0) <= self.shape.width() as f64
                && part.height + f64::from(PAD * 2.0) <= self.shape.height() as f64
                && (0.0..=1.0).contains(&part.opacity)
                && (0.0..=10.0).contains(&part.blur_radius),
            "Invalid caption background placement"
        );
        if part.width == 0.0 || part.height == 0.0 || part.opacity == 0.0 {
            return Ok(());
        }
        let key = [
            part.width.to_bits(),
            part.height.to_bits(),
            part.opacity.to_bits(),
        ];
        if self.cached != Some(key) {
            let (radius, curvature) =
                super::caption_corner::profile(part.width, part.height, self.radius as f64);
            let mut values = vec![
                PAD,
                PAD,
                part.width as f32,
                part.height as f32,
                self.color[0],
                self.color[1],
                self.color[2],
                self.color[3],
                self.shape.width() as f32,
                self.shape.height() as f32,
                radius,
                part.opacity as f32,
            ];
            values.extend(curvature);
            queue.write_buffer(&self.shape_uniform, 0, bytemuck::cast_slice(&values));
            let view = self.shape.create_view(&Default::default());
            render(
                encoder,
                &view,
                &self.shape_pipeline,
                &self.shape_binding,
                wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT),
            );
            self.pyramid.encode(encoder);
            self.cached = Some(key);
        }
        let values = [
            part.x as f32 - PAD,
            part.y as f32 - PAD,
            self.shape.width() as f32,
            self.shape.height() as f32,
            target.width() as f32,
            target.height() as f32,
            1.0,
            part.blur_radius as f32,
        ];
        queue.write_buffer(&self.blur_uniform, 0, bytemuck::cast_slice(&values));
        render(
            encoder,
            &target.create_view(&Default::default()),
            &self.blur_pipeline,
            &self.blur_binding,
            wgpu::LoadOp::Load,
        );
        Ok(())
    }
}
fn render(
    encoder: &mut wgpu::CommandEncoder,
    view: &wgpu::TextureView,
    pipeline: &wgpu::RenderPipeline,
    binding: &wgpu::BindGroup,
    load: wgpu::LoadOp<wgpu::Color>,
) {
    let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
        label: Some("Caption background"),
        color_attachments: &[Some(wgpu::RenderPassColorAttachment {
            view,
            depth_slice: None,
            resolve_target: None,
            ops: wgpu::Operations {
                load,
                store: wgpu::StoreOp::Store,
            },
        })],
        depth_stencil_attachment: None,
        timestamp_writes: None,
        occlusion_query_set: None,
        multiview_mask: None,
    });
    pass.set_pipeline(pipeline);
    pass.set_bind_group(0, binding, &[]);
    pass.draw(0..4, 0..1);
}

#[cfg(test)]
#[path = "caption_background_tests.rs"]
mod tests;
