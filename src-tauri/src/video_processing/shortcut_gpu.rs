//! Keycap atlas rendering into a premultiplied RGBA8 target, before readback.
use anyhow::{ensure,Result};
use super::{loupe::mip_pyramid::{MipPyramid,SAMPLING_SHADER},shortcut_motion::Draw,shortcut_overlay::Raster};
struct Slot {tile:usize,uniform:wgpu::Buffer,binding:wgpu::BindGroup}
pub(super) struct KeycapGpu {
    pipeline:wgpu::RenderPipeline,sampler:wgpu::Sampler,tiles:Vec<MipPyramid>,slots:Vec<Slot>,
}
impl KeycapGpu {
    pub(super) fn new(device:&wgpu::Device,queue:&wgpu::Queue,rasters:&[Raster])->Result<Self> {
        let shader=device.create_shader_module(wgpu::ShaderModuleDescriptor{label:Some("Keycap blur"),
            source:wgpu::ShaderSource::Wgsl(format!("{}\n{}\n{}",SAMPLING_SHADER,
                include_str!("loupe/blue_noise.wgsl"),include_str!("shortcut_gpu.wgsl")).into())});
        let pipeline=device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label:Some("Keycap atlas"),layout:None,
            vertex:wgpu::VertexState{module:&shader,entry_point:Some("vertex"),compilation_options:Default::default(),buffers:&[]},
            primitive:wgpu::PrimitiveState{topology:wgpu::PrimitiveTopology::TriangleStrip,..Default::default()},
            depth_stencil:None,multisample:Default::default(),
            fragment:Some(wgpu::FragmentState{module:&shader,entry_point:Some("fragment"),compilation_options:Default::default(),
                targets:&[Some(wgpu::ColorTargetState{format:wgpu::TextureFormat::Rgba8Unorm,
                    blend:Some(wgpu::BlendState::PREMULTIPLIED_ALPHA_BLENDING),write_mask:wgpu::ColorWrites::ALL})]}),
            multiview_mask:None,cache:None,
        });
        let sampler=device.create_sampler(&wgpu::SamplerDescriptor{mag_filter:wgpu::FilterMode::Linear,
            min_filter:wgpu::FilterMode::Linear,mipmap_filter:wgpu::MipmapFilterMode::Linear,..Default::default()});
        let mut tiles=Vec::new();let mut encoder=device.create_command_encoder(&Default::default());
        for raster in rasters {
            let size=wgpu::Extent3d{width:raster.width,height:raster.height,depth_or_array_layers:1};
            let input=device.create_texture(&wgpu::TextureDescriptor{label:Some("Keycap raster"),size,mip_level_count:1,
                sample_count:1,dimension:wgpu::TextureDimension::D2,format:wgpu::TextureFormat::Rgba8Unorm,
                usage:wgpu::TextureUsages::COPY_DST|wgpu::TextureUsages::TEXTURE_BINDING,view_formats:&[]});
            let mut pixels=raster.pixels.clone();
            for pixel in pixels.chunks_exact_mut(4) {for c in 0..3 {pixel[c]=((pixel[c] as u32*pixel[3] as u32+127)/255) as u8;}}
            queue.write_texture(wgpu::TexelCopyTextureInfo{texture:&input,mip_level:0,origin:wgpu::Origin3d::ZERO,
                aspect:wgpu::TextureAspect::All},&pixels,wgpu::TexelCopyBufferLayout{offset:0,bytes_per_row:Some(raster.width*4),
                rows_per_image:Some(raster.height)},size);
            let pyramid=MipPyramid::new(device,&input)?;pyramid.encode(&mut encoder);tiles.push(pyramid);
        }
        queue.submit([encoder.finish()]);
        Ok(Self{pipeline,sampler,tiles,slots:Vec::new()})
    }
    pub(super) fn encode(&mut self,device:&wgpu::Device,queue:&wgpu::Queue,encoder:&mut wgpu::CommandEncoder,
        target:&wgpu::Texture,draws:&[Draw],blur:bool,clear:bool)->Result<()> {
        self.encode_parts(device,queue,encoder,target,draws,clear,|index|if blur {5.0*(1.0-draws[index].opacity as f32)} else {0.0})
    }
    /// Explicit radii allow other text overlays to share the atlas without inheriting keycap motion.
    pub(super) fn encode_with_blurs(&mut self,device:&wgpu::Device,queue:&wgpu::Queue,encoder:&mut wgpu::CommandEncoder,
        target:&wgpu::Texture,draws:&[Draw],radii:&[f32],clear:bool)->Result<()> {
        ensure!(radii.len()==draws.len()&&radii.iter().all(|r|r.is_finite()&&(0.0..=10.0).contains(r)),
            "Invalid text overlay blur radii");
        self.encode_parts(device,queue,encoder,target,draws,clear,|index|radii[index])
    }
    fn encode_parts(&mut self,device:&wgpu::Device,queue:&wgpu::Queue,encoder:&mut wgpu::CommandEncoder,
        target:&wgpu::Texture,draws:&[Draw],clear:bool,radius:impl Fn(usize)->f32)->Result<()> {
        super::shortcut_motion::validate_draws(draws,self.tiles.len(),target.width(),target.height())?;
        ensure!(target.format()==wgpu::TextureFormat::Rgba8Unorm&&target.usage().contains(wgpu::TextureUsages::RENDER_ATTACHMENT),
            "Keycap target must be renderable RGBA8");
        for (index,draw) in draws.iter().enumerate() {
            if self.slots.get(index).is_none_or(|slot|slot.tile!=draw.tile) {
                let uniform=device.create_buffer(&wgpu::BufferDescriptor{label:Some("Keycap placement"),size:32,
                    usage:wgpu::BufferUsages::UNIFORM|wgpu::BufferUsages::COPY_DST,mapped_at_creation:false});
                let binding=device.create_bind_group(&wgpu::BindGroupDescriptor{label:Some("Keycap atlas draw"),
                    layout:&self.pipeline.get_bind_group_layout(0),entries:&[
                        wgpu::BindGroupEntry{binding:0,resource:wgpu::BindingResource::TextureView(&self.tiles[draw.tile].texture().create_view(&Default::default()))},
                        wgpu::BindGroupEntry{binding:1,resource:wgpu::BindingResource::Sampler(&self.sampler)},
                        wgpu::BindGroupEntry{binding:2,resource:uniform.as_entire_binding()},
                    ]});
                let slot=Slot{tile:draw.tile,uniform,binding};
                if index==self.slots.len(){self.slots.push(slot);}else{self.slots[index]=slot;}
            }
            let values=[draw.x as f32,draw.y as f32,draw.width as f32,draw.height as f32,
                target.width() as f32,target.height() as f32,draw.opacity as f32,radius(index)];
            queue.write_buffer(&self.slots[index].uniform,0,bytemuck::cast_slice(&values));
        }
        let view=target.create_view(&Default::default());
        let mut pass=encoder.begin_render_pass(&wgpu::RenderPassDescriptor{label:Some("Keycap blur overlay"),
            color_attachments:&[Some(wgpu::RenderPassColorAttachment{view:&view,depth_slice:None,resolve_target:None,
                ops:wgpu::Operations{load:if clear {wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT)}else{wgpu::LoadOp::Load},store:wgpu::StoreOp::Store}})],
            depth_stencil_attachment:None,timestamp_writes:None,occlusion_query_set:None,multiview_mask:None});
        pass.set_pipeline(&self.pipeline);
        for index in 0..draws.len(){pass.set_bind_group(0,&self.slots[index].binding,&[]);pass.draw(0..4,0..1);}
        Ok(())
    }
}

#[cfg(test)]
#[path="shortcut_gpu_tests.rs"]
mod tests;
