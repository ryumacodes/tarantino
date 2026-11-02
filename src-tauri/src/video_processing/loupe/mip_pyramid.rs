//! Reusable filtered scene pyramid for magnifier sampling.
use anyhow::{bail,Result};

/// Shared shader functions for reading this pyramid.
pub const SAMPLING_SHADER: &str = include_str!("mip_sampling.wgsl");

pub struct MipPyramid {
    texture: wgpu::Texture,
    pipelines: [wgpu::ComputePipeline;3],
    levels: Vec<(wgpu::BindGroup,u32,u32)>,
}

impl MipPyramid {
    pub fn new(device:&wgpu::Device,input:&wgpu::Texture) -> Result<Self> {
        Self::with_premultiply(device,input,false)
    }
    pub fn with_premultiply(device:&wgpu::Device,input:&wgpu::Texture,premultiply:bool) -> Result<Self> {
        if input.width()==0 || input.height()==0 || input.depth_or_array_layers()!=1
            || input.dimension()!=wgpu::TextureDimension::D2 || input.sample_count()!=1
            || input.format()!=wgpu::TextureFormat::Rgba8Unorm {
            bail!("Magnifier pyramid requires a nonempty single-layer RGBA8 texture");
        }
        let count=input.width().max(input.height()).ilog2()+1;
        let texture=device.create_texture(&wgpu::TextureDescriptor {
            label:Some("Magnifier filtered pyramid"),size:input.size(),mip_level_count:count,sample_count:1,
            dimension:wgpu::TextureDimension::D2,format:wgpu::TextureFormat::Rgba8Unorm,
            usage:wgpu::TextureUsages::TEXTURE_BINDING|wgpu::TextureUsages::STORAGE_BINDING|wgpu::TextureUsages::COPY_SRC,
            view_formats:&[],
        });
        let shader=device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label:Some("Magnifier pyramid filters"),source:wgpu::ShaderSource::Wgsl(include_str!("mip_pyramid.wgsl").into()),
        });
        let pipelines=[if premultiply {"premultiply_level"}else{"copy_level"},"box_level","binomial_level"].map(|entry|
            device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
                label:Some(entry),layout:None,module:&shader,entry_point:Some(entry),
                compilation_options:Default::default(),cache:None,
            }));
        let mut levels=Vec::new();
        for level in 0..count {
            let source=if level==0 { input } else { &texture };
            let view=|texture:&wgpu::Texture,index| texture.create_view(&wgpu::TextureViewDescriptor {
                base_mip_level:index,mip_level_count:Some(1),..Default::default()
            });
            let binding=device.create_bind_group(&wgpu::BindGroupDescriptor {
                label:Some("Magnifier pyramid level"),layout:&pipelines[level.min(2) as usize].get_bind_group_layout(0),
                entries:&[
                    wgpu::BindGroupEntry {binding:0,resource:wgpu::BindingResource::TextureView(&view(source,level.saturating_sub(1)))},
                    wgpu::BindGroupEntry {binding:1,resource:wgpu::BindingResource::TextureView(&view(&texture,level))},
                ],
            });
            levels.push((binding,(input.width()>>level).max(1),(input.height()>>level).max(1)));
        }
        Ok(Self {texture,pipelines,levels})
    }

    pub fn texture(&self) -> &wgpu::Texture { &self.texture }

    /// Separate passes establish dependencies between adjacent mip levels.
    /// Bindings and textures are reused; there is no CPU pixel readback.
    pub fn encode(&self,encoder:&mut wgpu::CommandEncoder) {
        for (level,(binding,width,height)) in self.levels.iter().enumerate() {
            let mut pass=encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label:Some("Magnifier pyramid level"),timestamp_writes:None,
            });
            pass.set_pipeline(&self.pipelines[level.min(2)]);
            pass.set_bind_group(0,binding,&[]);
            pass.dispatch_workgroups(width.div_ceil(8),height.div_ceil(8),1);
        }
    }
}

#[cfg(test)]
#[path="mip_pyramid_tests.rs"]
mod tests;

#[cfg(test)]
#[path="mip_sampling_tests.rs"]
mod sampling_tests;
