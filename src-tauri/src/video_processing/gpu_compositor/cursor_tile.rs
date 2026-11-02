//! Cached cursor silhouette and mip chain used while fading. Motion, scale and
//! opacity are applied during sampling, so they never invalidate this texture.
use super::*;
use crate::video_processing::loupe::mip_pyramid::MipPyramid;

pub(super) struct CursorTile {
    pub pyramid: MipPyramid,
    pipeline: wgpu::ComputePipeline,
    binding: wgpu::BindGroup,
    uniforms: wgpu::Buffer,
    key: Option<[f32;5]>,
}
impl CursorTile {
    pub fn new(device:&wgpu::Device)->Result<Self> {
        let texture=device.create_texture(&wgpu::TextureDescriptor {
            label:Some("Cursor fade silhouette"),size:wgpu::Extent3d {width:256,height:256,depth_or_array_layers:1},
            mip_level_count:1,sample_count:1,dimension:wgpu::TextureDimension::D2,
            format:wgpu::TextureFormat::Rgba8Unorm,
            usage:wgpu::TextureUsages::STORAGE_BINDING|wgpu::TextureUsages::TEXTURE_BINDING,view_formats:&[],
        });
        let pyramid=MipPyramid::new(device,&texture)?;
        let shader=device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label:Some("Cursor tile renderer"),source:wgpu::ShaderSource::Wgsl(format!("{}\n{}",COMPOSITE_SHADER,include_str!("cursor_tile.wgsl")).into()),
        });
        let pipeline=device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label:Some("Cursor tile"),layout:None,module:&shader,entry_point:Some("cursor_tile"),
            compilation_options:Default::default(),cache:None,
        });
        let uniforms=device.create_buffer(&wgpu::BufferDescriptor {
            label:Some("Cursor tile style"),size:std::mem::size_of::<CompositeUniforms>() as u64,
            usage:wgpu::BufferUsages::UNIFORM|wgpu::BufferUsages::COPY_DST,mapped_at_creation:false,
        });
        let view=texture.create_view(&Default::default());
        let binding=device.create_bind_group(&wgpu::BindGroupDescriptor {
            label:Some("Cursor tile"),layout:&pipeline.get_bind_group_layout(0),entries:&[
                wgpu::BindGroupEntry {binding:0,resource:uniforms.as_entire_binding()},
                wgpu::BindGroupEntry {binding:2,resource:wgpu::BindingResource::TextureView(&view)},
            ],
        });
        Ok(Self {pyramid,pipeline,binding,uniforms,key:None})
    }
    pub fn update(&mut self,device:&wgpu::Device,queue:&wgpu::Queue,uniforms:&CompositeUniforms) {
        let color=if uniforms.is_clicking>0.5 {[uniforms.cursor_highlight_r,uniforms.cursor_highlight_g,uniforms.cursor_highlight_b]}
            else {[uniforms.cursor_color_r,uniforms.cursor_color_g,uniforms.cursor_color_b]};
        let key=[uniforms.cursor_style,uniforms.cursor_shadow_intensity,color[0],color[1],color[2]];
        if self.key==Some(key) {return;}
        let mut tile_uniforms=*uniforms;tile_uniforms.layer_outputs=0.0;
        queue.write_buffer(&self.uniforms,0,bytemuck::bytes_of(&tile_uniforms));
        let mut encoder=device.create_command_encoder(&wgpu::CommandEncoderDescriptor {label:Some("Cursor tile update")});
        {
            let mut pass=encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {label:Some("Cursor silhouette"),timestamp_writes:None});
            pass.set_pipeline(&self.pipeline);pass.set_bind_group(0,&self.binding,&[]);pass.dispatch_workgroups(32,32,1);
        }
        self.pyramid.encode(&mut encoder);
        queue.submit(Some(encoder.finish()));self.key=Some(key);
    }
}
