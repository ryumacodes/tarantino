//! Four quadrant averages remain on the GPU for opaque region rendering.
use anyhow::{ensure, Result};
pub(super) struct CoverPlan {
    pub lattice: [f32; 4],
    pub level_span: [f32; 2],
    pub taps: [u32; 2],
    pub level: u32,
}
pub(super) struct MaskCover {
    pipeline: wgpu::ComputePipeline,
    uniform: wgpu::Buffer,
    binding: wgpu::BindGroup,
    output: wgpu::Texture,
    levels: u32,
}
impl MaskCover {
    pub fn new(device: &wgpu::Device, source: &wgpu::Texture) -> Result<Self> {
        ensure!(source.dimension()==wgpu::TextureDimension::D2 && source.depth_or_array_layers()==1
            && source.sample_count()==1 && source.usage().contains(wgpu::TextureUsages::TEXTURE_BINDING),
            "Invalid mask source texture");
        let shader=device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label:Some("Mask quadrant averages"),source:wgpu::ShaderSource::Wgsl(include_str!("mask_cover.wgsl").into()),
        });
        let pipeline=device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label:Some("Mask quadrant averages"),layout:None,module:&shader,entry_point:Some("main"),
            compilation_options:Default::default(),cache:None,
        });
        let output=device.create_texture(&wgpu::TextureDescriptor {
            label:Some("Mask cover colors"),size:wgpu::Extent3d {width:2,height:2,depth_or_array_layers:1},
            mip_level_count:1,sample_count:1,dimension:wgpu::TextureDimension::D2,format:wgpu::TextureFormat::Rgba8Unorm,
            usage:wgpu::TextureUsages::STORAGE_BINDING|wgpu::TextureUsages::TEXTURE_BINDING|wgpu::TextureUsages::COPY_SRC,view_formats:&[],
        });
        let uniform=device.create_buffer(&wgpu::BufferDescriptor {label:None,size:48,
            usage:wgpu::BufferUsages::UNIFORM|wgpu::BufferUsages::COPY_DST,mapped_at_creation:false});
        let sampler=device.create_sampler(&wgpu::SamplerDescriptor {
            mag_filter:wgpu::FilterMode::Linear,min_filter:wgpu::FilterMode::Linear,
            mipmap_filter:wgpu::MipmapFilterMode::Nearest,..Default::default()
        });
        let binding=device.create_bind_group(&wgpu::BindGroupDescriptor {
            label:None,layout:&pipeline.get_bind_group_layout(0),entries:&[
                wgpu::BindGroupEntry {binding:0,resource:wgpu::BindingResource::TextureView(&source.create_view(&Default::default()))},
                wgpu::BindGroupEntry {binding:1,resource:wgpu::BindingResource::Sampler(&sampler)},
                wgpu::BindGroupEntry {binding:2,resource:uniform.as_entire_binding()},
                wgpu::BindGroupEntry {binding:3,resource:wgpu::BindingResource::TextureView(&output.create_view(&Default::default()))},
            ],
        });
        Ok(Self {pipeline,uniform,binding,output,levels:source.mip_level_count()})
    }
    pub fn texture(&self)->&wgpu::Texture { &self.output }
    /// Submit before encoding this instance again: each instance owns one uniform.
    pub fn encode(&self,queue:&wgpu::Queue,encoder:&mut wgpu::CommandEncoder,plan:&CoverPlan)->Result<()> {
        ensure!(plan.lattice.iter().chain(plan.level_span.iter()).all(|x|x.is_finite())
            && plan.lattice.iter().all(|x|*x>=0.0) && plan.level_span.iter().all(|x|*x>0.0)
            && plan.taps.iter().all(|x|*x>0 && *x<=256) && u64::from(plan.taps[0])*u64::from(plan.taps[1])<=256
            && plan.level<self.levels,"Invalid mask cover sampling plan");
        let values=[plan.lattice[0],plan.lattice[1],plan.lattice[2],plan.lattice[3],
            plan.level_span[0],plan.level_span[1],plan.taps[0] as f32,plan.taps[1] as f32,plan.level as f32,0.0,0.0,0.0];
        let mut bytes=[0u8;48];
        for (chunk,value) in bytes.chunks_exact_mut(4).zip(values) { chunk.copy_from_slice(&value.to_le_bytes()); }
        queue.write_buffer(&self.uniform,0,&bytes);
        let mut pass=encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {label:Some("Mask cover"),timestamp_writes:None});
        pass.set_pipeline(&self.pipeline);pass.set_bind_group(0,&self.binding,&[]);pass.dispatch_workgroups(1,1,1);
        Ok(())
    }
}
#[cfg(test)]
#[path="mask_cover_tests.rs"]
mod tests;
