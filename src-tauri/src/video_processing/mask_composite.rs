//! Mask composition on a linear scene texture. Instances retain GPU resources.
use anyhow::{ensure,Result};
use super::caption_corner;
pub(super) struct MaskPlacement {
    pub rect:[f32;4],
    pub radius:f32,
    pub opacity:f32,
    pub highlight_opacity:f32,
    pub sensitive:bool,
}
pub(super) struct MaskComposite {
    pipeline:wgpu::RenderPipeline,
    uniform:wgpu::Buffer,
    binding:wgpu::BindGroup,
    corner:Option<([u32;3],(f32,[f32;16]))>,
}
impl MaskComposite {
    pub fn new(device:&wgpu::Device,scene:&wgpu::TextureView,colors:&wgpu::TextureView,format:wgpu::TextureFormat)->Self {
        Self::with_encoding(device,scene,colors,format,false)
    }
    pub fn with_encoding(device:&wgpu::Device,scene:&wgpu::TextureView,colors:&wgpu::TextureView,format:wgpu::TextureFormat,encoded_straight:bool)->Self {
        let shader=device.create_shader_module(wgpu::ShaderModuleDescriptor{label:Some("Mask composition"),
            source:wgpu::ShaderSource::Wgsl(include_str!("mask_composite.wgsl").into())});
        let pipeline=device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label:Some("Mask composition"),layout:None,
            vertex:wgpu::VertexState {module:&shader,entry_point:Some("vertex"),compilation_options:Default::default(),buffers:&[]},
            primitive:wgpu::PrimitiveState{topology:wgpu::PrimitiveTopology::TriangleStrip,..Default::default()},
            depth_stencil:None,multisample:Default::default(),
            fragment:Some(wgpu::FragmentState{module:&shader,entry_point:Some(if encoded_straight {"fragment_encoded"} else {"fragment"}),compilation_options:Default::default(),
                targets:&[Some(wgpu::ColorTargetState{format,blend:None,write_mask:wgpu::ColorWrites::ALL})]}),
            multiview_mask:None,cache:None,
        });
        let uniform=device.create_buffer(&wgpu::BufferDescriptor{label:None,size:96,
            usage:wgpu::BufferUsages::UNIFORM|wgpu::BufferUsages::COPY_DST,mapped_at_creation:false});
        let sampler=device.create_sampler(&wgpu::SamplerDescriptor{mag_filter:wgpu::FilterMode::Linear,
            min_filter:wgpu::FilterMode::Linear,..Default::default()});
        let binding=device.create_bind_group(&wgpu::BindGroupDescriptor{label:None,layout:&pipeline.get_bind_group_layout(0),entries:&[
            wgpu::BindGroupEntry{binding:0,resource:wgpu::BindingResource::TextureView(scene)},
            wgpu::BindGroupEntry{binding:1,resource:wgpu::BindingResource::TextureView(colors)},
            wgpu::BindGroupEntry{binding:2,resource:wgpu::BindingResource::Sampler(&sampler)},
            wgpu::BindGroupEntry{binding:3,resource:uniform.as_entire_binding()},
        ]});
        Self{pipeline,uniform,binding,corner:None}
    }
    /// Output must match the source dimensions and use a separate texture.
    /// Submit before encoding this instance again (one reusable uniform).
    pub fn encode(&mut self,queue:&wgpu::Queue,encoder:&mut wgpu::CommandEncoder,target:&wgpu::TextureView,p:&MaskPlacement)->Result<()> {
        ensure!(p.rect.iter().all(|v|v.is_finite())&&p.rect[2]>0.0&&p.rect[3]>0.0
            &&p.radius.is_finite()&&p.radius>=0.0
            &&[p.opacity,p.highlight_opacity].iter().all(|v|v.is_finite()&&(0.0..=1.0).contains(v)),"Invalid mask placement");
        let key=[p.rect[2].to_bits(),p.rect[3].to_bits(),p.radius.to_bits()];
        let(radius,table)=match self.corner {
            Some((cached,profile)) if cached==key=>profile,
            _=>{let profile=caption_corner::profile(p.rect[2] as f64,p.rect[3] as f64,p.radius as f64);
                self.corner=Some((key,profile));profile}
        };
        let mut values=[0f32;24];values[..4].copy_from_slice(&p.rect);
        values[4..8].copy_from_slice(&[radius,p.opacity,p.highlight_opacity,if p.sensitive{1.0}else{0.0}]);
        values[8..].copy_from_slice(&table);
        let mut bytes=[0u8;96];for(chunk,value)in bytes.chunks_exact_mut(4).zip(values){chunk.copy_from_slice(&value.to_le_bytes());}
        queue.write_buffer(&self.uniform,0,&bytes);
        let mut pass=encoder.begin_render_pass(&wgpu::RenderPassDescriptor{label:Some("Mask composition"),
            color_attachments:&[Some(wgpu::RenderPassColorAttachment{view:target,depth_slice:None,resolve_target:None,
                ops:wgpu::Operations{load:wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT),store:wgpu::StoreOp::Store}})],
            depth_stencil_attachment:None,timestamp_writes:None,occlusion_query_set:None,multiview_mask:None});
        pass.set_pipeline(&self.pipeline);pass.set_bind_group(0,&self.binding,&[]);pass.draw(0..4,0..1);
        Ok(())
    }
}
#[cfg(test)]
#[path="mask_composite_tests.rs"]
mod tests;
