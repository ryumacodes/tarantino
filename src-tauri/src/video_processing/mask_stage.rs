//! Source-space masks run before framing, zoom and cursor composition.
use anyhow::Result;
use super::{loupe::mip_pyramid::MipPyramid,mask_cover::MaskCover,mask_composite::{MaskComposite,MaskPlacement},mask_sampling};
pub(super) struct MaskStage {
    pyramid:MipPyramid,
    cover:MaskCover,
    composite:MaskComposite,
    output:wgpu::Texture,
    size:[u32;2],
}
impl MaskStage {
    pub fn new(device:&wgpu::Device,source:&wgpu::Texture)->Result<Self> {
        let pyramid=MipPyramid::with_premultiply(device,source,true)?;
        let cover=MaskCover::new(device,pyramid.texture())?;
        let output=device.create_texture(&wgpu::TextureDescriptor{label:Some("Masked recording"),size:source.size(),
            mip_level_count:1,sample_count:1,dimension:wgpu::TextureDimension::D2,format:wgpu::TextureFormat::Rgba8Unorm,
            usage:wgpu::TextureUsages::TEXTURE_BINDING|wgpu::TextureUsages::RENDER_ATTACHMENT|wgpu::TextureUsages::COPY_SRC,view_formats:&[]});
        let composite=MaskComposite::with_encoding(device,&source.create_view(&Default::default()),
            &cover.texture().create_view(&Default::default()),wgpu::TextureFormat::Rgba8Unorm,true);
        Ok(Self{pyramid,cover,composite,output,size:[source.width(),source.height()]})
    }
    pub fn texture(&self)->&wgpu::Texture {&self.output}
    pub fn encode(&mut self,queue:&wgpu::Queue,encoder:&mut wgpu::CommandEncoder,placement:&MaskPlacement)->Result<()> {
        let [w,h]=self.size.map(f64::from);
        let rect=placement.rect.map(f64::from);
        let plan=mask_sampling::plan([rect[0]/w,rect[1]/h,rect[2]/w,rect[3]/h],[0.0,0.0,1.0,1.0],self.size,self.pyramid.texture().mip_level_count())?;
        if let Some(plan)=plan {
            self.pyramid.encode(encoder);
            self.cover.encode(queue,encoder,&plan)?;
        }
        // Offscreen sensitive bounds leave the source unchanged; highlights still dim it.
        self.composite.encode(queue,encoder,&self.output.create_view(&Default::default()),placement)
    }
}
