//! Caption background and glyph composition before the encoder's single readback.
use super::{
    caption_background::CaptionBackground,
    caption_overlay::{Prepared, Span},
    caption_text_gpu::CaptionTextGpu,
};
use anyhow::{Result, ensure};
pub(super) struct CaptionStage {
    text: CaptionTextGpu,
    background: CaptionBackground,
    spans: Vec<Span>,
    overlay: wgpu::Texture,
    output: wgpu::Texture,
    pipeline: wgpu::ComputePipeline,
}
impl CaptionStage {
    pub fn new(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        prepared: Prepared,
        width: u32,
        height: u32,
    ) -> Result<Self> {
        let max = prepared
            .spans
            .iter()
            .filter_map(|span| span.background.as_ref())
            .fold((0.0_f64, 0.0_f64), |(w, h), part| {
                (w.max(part.width), h.max(part.height))
            });
        let background = CaptionBackground::new(
            device,
            max.0,
            max.1,
            prepared.background_color,
            prepared.corner_radius,
        )?;
        let text = CaptionTextGpu::new(device, queue, prepared.tiles)?;
        let texture = |usage| {
            device.create_texture(&wgpu::TextureDescriptor {
                label: Some("Caption stage"),
                size: wgpu::Extent3d {
                    width,
                    height,
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: wgpu::TextureFormat::Rgba8Unorm,
                usage,
                view_formats: &[],
            })
        };
        let overlay =
            texture(wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING);
        let output = texture(
            wgpu::TextureUsages::STORAGE_BINDING
                | wgpu::TextureUsages::TEXTURE_BINDING
                | wgpu::TextureUsages::COPY_SRC,
        );
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Caption alpha composition"),
            source: wgpu::ShaderSource::Wgsl(super::shortcut_stage::COMPOSITE.into()),
        });
        let pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: Some("Caption alpha composition"),
            layout: None,
            module: &shader,
            entry_point: Some("main"),
            compilation_options: Default::default(),
            cache: None,
        });
        Ok(Self {
            text,
            background,
            spans: prepared.spans,
            overlay,
            output,
            pipeline,
        })
    }
    /// Returns false for a caption-free frame, allowing the original scene to bypass this stage.
    pub fn encode(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        scene: &wgpu::Texture,
        frame: u64,
    ) -> Result<bool> {
        let index = self.spans.partition_point(|span| span.start_frame <= frame);
        let Some(span) = index
            .checked_sub(1)
            .and_then(|i| self.spans.get(i))
            .filter(|span| frame < span.end_frame)
        else {
            return Ok(false);
        };
        ensure!(
            scene.width() == self.output.width()
                && scene.height() == self.output.height()
                && scene.format() == wgpu::TextureFormat::Rgba8Unorm
                && scene.usage().contains(wgpu::TextureUsages::TEXTURE_BINDING),
            "Invalid caption scene"
        );
        // Clear once, then draw background before glyphs.
        {
            let view = self.overlay.create_view(&Default::default());
            let _clear = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Clear captions"),
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
        if let Some(part) = &span.background {
            self.background
                .encode(queue, encoder, &self.overlay, part)?;
        }
        self.text
            .encode(device, queue, encoder, &self.overlay, &span.words, false)?;
        let binding = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Caption stage inputs"),
            layout: &self.pipeline.get_bind_group_layout(0),
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(
                        &scene.create_view(&Default::default()),
                    ),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(
                        &self.overlay.create_view(&Default::default()),
                    ),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::TextureView(
                        &self.output.create_view(&Default::default()),
                    ),
                },
            ],
        });
        let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
            label: Some("Caption stage composite"),
            timestamp_writes: None,
        });
        pass.set_pipeline(&self.pipeline);
        pass.set_bind_group(0, &binding, &[]);
        pass.dispatch_workgroups(
            self.output.width().div_ceil(8),
            self.output.height().div_ceil(8),
            1,
        );
        Ok(true)
    }
    pub fn output(&self) -> &wgpu::Texture {
        &self.output
    }
}
