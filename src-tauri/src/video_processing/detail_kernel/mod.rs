mod model;
mod shader;
mod cache;
pub(crate) use cache::DetailCache;
pub(crate) use model::Model;
use anyhow::{bail, Result};
use wgpu::util::DeviceExt;
use super::detail_region::Plan;

pub(crate) struct DetailKernel {
    pipeline: wgpu::ComputePipeline,
    scale: u32,
}
impl DetailKernel {
    pub fn new(device: &wgpu::Device, model: &Model) -> Result<Self> {
        model.validate()?;
        let module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("detail inference"),
            source: wgpu::ShaderSource::Wgsl(shader::generate(model).into()),
        });
        let pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: Some("detail tile"), layout: None, module: &module,
            entry_point: Some("tile"),
            compilation_options: Default::default(), cache: None,
        });
        Ok(Self { pipeline, scale: model.scale })
    }

    pub fn encode(&self, device: &wgpu::Device, encoder: &mut wgpu::CommandEncoder,
        source: &wgpu::Texture, plan: &Plan) -> Result<wgpu::Texture> {
        self.validate_region(device, source, plan)?;
        let output = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("enhanced frame"), size: wgpu::Extent3d {
                width: plan.output_size[0], height: plan.output_size[1], depth_or_array_layers: 1 },
            mip_level_count: 1, sample_count: 1, dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8Unorm,
            usage: wgpu::TextureUsages::STORAGE_BINDING | wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        });
        self.record(device, encoder, source, plan, &output);
        Ok(output)
    }

    pub fn encode_into(&self, device: &wgpu::Device, encoder: &mut wgpu::CommandEncoder,
        source: &wgpu::Texture, plan: &Plan, output: &wgpu::Texture) -> Result<()> {
        self.validate_region(device, source, plan)?;
        if [output.width(), output.height()] != plan.output_size
            || output.format() != wgpu::TextureFormat::Rgba8Unorm
            || !output.usage().contains(wgpu::TextureUsages::STORAGE_BINDING)
            || output.mip_level_count() != 1 || output.sample_count() != 1
            || output.dimension() != wgpu::TextureDimension::D2 || output.depth_or_array_layers() != 1 {
            bail!("Detail output does not match the requested region");
        }
        self.record(device, encoder, source, plan, output);
        Ok(())
    }

    fn validate_region(&self, device: &wgpu::Device, source: &wgpu::Texture, plan: &Plan) -> Result<()> {
        let [x, y, width, height] = plan.region;
        let limits = device.limits();
        if plan.scale != self.scale || width == 0 || height == 0
            || x as u64 + width as u64 > source.width() as u64
            || y as u64 + height as u64 > source.height() as u64
            || plan.output_size != [width.saturating_mul(self.scale), height.saturating_mul(self.scale)]
            || plan.output_size.iter().any(|v| *v > limits.max_texture_dimension_2d)
        {
            bail!("Detail region exceeds device or source limits");
        }
        Ok(())
    }

    fn record(&self, device: &wgpu::Device, encoder: &mut wgpu::CommandEncoder,
        source: &wgpu::Texture, plan: &Plan, output: &wgpu::Texture) {
        let [_, _, width, height] = plan.region;
        let region = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("detail region"), contents: &plan.region.into_iter().flat_map(u32::to_le_bytes).collect::<Vec<_>>(),
            usage: wgpu::BufferUsages::UNIFORM });
        let source_view = source.create_view(&Default::default());
        let output_view = output.create_view(&Default::default());
        let group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("detail inputs"), layout: &self.pipeline.get_bind_group_layout(0), entries: &[
                wgpu::BindGroupEntry { binding: 0, resource: wgpu::BindingResource::TextureView(&source_view) },
                wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::TextureView(&output_view) },
                wgpu::BindGroupEntry { binding: 3, resource: region.as_entire_binding() },
            ] });
        {
            let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor::default());
            pass.set_pipeline(&self.pipeline);
            pass.set_bind_group(0, &group, &[]);
            pass.dispatch_workgroups(width.div_ceil(8), height.div_ceil(8), 1);
        }
    }

}
