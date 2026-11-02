//! Blend premultiplied keycaps with a straight-alpha video stage on the GPU.
use anyhow::{ensure,Result};
use super::{shortcut_gpu::KeycapGpu,shortcut_motion::Draw,shortcut_overlay::Raster};
pub(super) struct ShortcutStage {
    keys:KeycapGpu,overlay:wgpu::Texture,output:wgpu::Texture,pipeline:wgpu::ComputePipeline,
}
pub(super) const COMPOSITE:&str=r#"
@group(0) @binding(0) var scene:texture_2d<f32>;
@group(0) @binding(1) var overlay:texture_2d<f32>;
@group(0) @binding(2) var output:texture_storage_2d<rgba8unorm,write>;
@compute @workgroup_size(8,8)
fn main(@builtin(global_invocation_id) id:vec3<u32>) {
    if(any(id.xy>=textureDimensions(output))){return;}
    let p=vec2<i32>(id.xy);let base=textureLoad(scene,p,0);let key=textureLoad(overlay,p,0);
    if(key.a<=0.0){textureStore(output,p,base);return;}
    let retained=base.a*(1.0-key.a);let alpha=key.a+retained;
    textureStore(output,p,vec4<f32>((key.rgb+base.rgb*retained)/alpha,alpha));
}"#;
impl ShortcutStage {
    pub(super) fn new(device:&wgpu::Device,queue:&wgpu::Queue,rasters:&[Raster],width:u32,height:u32)->Result<Self> {
        let keys=KeycapGpu::new(device,queue,rasters)?;
        let texture=|label,usage|device.create_texture(&wgpu::TextureDescriptor{label:Some(label),
            size:wgpu::Extent3d{width,height,depth_or_array_layers:1},mip_level_count:1,sample_count:1,
            dimension:wgpu::TextureDimension::D2,format:wgpu::TextureFormat::Rgba8Unorm,usage,view_formats:&[]});
        let overlay=texture("Shortcut transparent overlay",wgpu::TextureUsages::RENDER_ATTACHMENT|wgpu::TextureUsages::TEXTURE_BINDING);
        let output=texture("Shortcut composited stage",wgpu::TextureUsages::STORAGE_BINDING|wgpu::TextureUsages::TEXTURE_BINDING|wgpu::TextureUsages::COPY_SRC);
        let shader=device.create_shader_module(wgpu::ShaderModuleDescriptor{label:Some("Shortcut alpha composition"),
            source:wgpu::ShaderSource::Wgsl(COMPOSITE.into())});
        let pipeline=device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor{label:Some("Shortcut alpha composition"),
            layout:None,module:&shader,entry_point:Some("main"),compilation_options:Default::default(),cache:None});
        Ok(Self{keys,overlay,output,pipeline})
    }
    pub(super) fn encode(&mut self,device:&wgpu::Device,queue:&wgpu::Queue,encoder:&mut wgpu::CommandEncoder,
        scene:&wgpu::Texture,draws:&[Draw])->Result<()> {
        ensure!(scene.width()==self.output.width()&&scene.height()==self.output.height()
            &&scene.format()==wgpu::TextureFormat::Rgba8Unorm,"Shortcut stage dimensions do not match");
        self.keys.encode(device,queue,encoder,&self.overlay,draws,true,true)?;
        let binding=device.create_bind_group(&wgpu::BindGroupDescriptor{label:Some("Shortcut stage inputs"),
            layout:&self.pipeline.get_bind_group_layout(0),entries:&[
                wgpu::BindGroupEntry{binding:0,resource:wgpu::BindingResource::TextureView(&scene.create_view(&Default::default()))},
                wgpu::BindGroupEntry{binding:1,resource:wgpu::BindingResource::TextureView(&self.overlay.create_view(&Default::default()))},
                wgpu::BindGroupEntry{binding:2,resource:wgpu::BindingResource::TextureView(&self.output.create_view(&Default::default()))},
            ]});
        let mut pass=encoder.begin_compute_pass(&wgpu::ComputePassDescriptor{label:Some("Shortcut stage composite"),timestamp_writes:None});
        pass.set_pipeline(&self.pipeline);pass.set_bind_group(0,&binding,&[]);
        pass.dispatch_workgroups(self.output.width().div_ceil(8),self.output.height().div_ceil(8),1);
        Ok(())
    }
    pub(super) fn output(&self)->&wgpu::Texture {&self.output}
}
