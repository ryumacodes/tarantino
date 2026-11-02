use super::*;

pub(super) struct StageLayers {
    pub foreground: wgpu::Texture,
    pub background: wgpu::Texture,
}
impl StageLayers {
    pub fn new(device:&wgpu::Device,width:u32,height:u32)->Self {
        let texture=|label|device.create_texture(&wgpu::TextureDescriptor {
            label:Some(label),size:wgpu::Extent3d {width,height,depth_or_array_layers:1},
            mip_level_count:1,sample_count:1,dimension:wgpu::TextureDimension::D2,
            format:wgpu::TextureFormat::Rgba8Unorm,
            usage:wgpu::TextureUsages::STORAGE_BINDING|wgpu::TextureUsages::TEXTURE_BINDING|wgpu::TextureUsages::COPY_SRC,
            view_formats:&[],
        });
        Self {foreground:texture("Screen and pointer layer"),background:texture("Unobscured backdrop")}
    }
}
impl GpuCompositor {
    pub fn set_camera_frame(&mut self, frame:crate::video_processing::loupe::camera_opacity::CameraOpacity) -> anyhow::Result<()> {
        let (w,h)=(self.config.output_width as f64,self.config.output_height as f64);
        self.set_webcam_opacity(frame.stage as f32);
        self.set_webcam_rect(frame.rect.map(|r|[(r[0]*w) as f32,(r[1]*h) as f32,(r[2]*w) as f32,(r[3]*h) as f32]))
    }

    /// Set an animated stage rectangle after selecting the camera mask/layout.
    /// Passing None restores layout-derived bounds without reallocating textures.
    pub fn set_webcam_rect(&mut self, rect: Option<[f32;4]>) -> anyhow::Result<()> {
        if let Some(rect) = rect {
            if !rect.iter().all(|value| value.is_finite()) || rect[2] < 0.0 || rect[3] < 0.0 {
                anyhow::bail!("Invalid camera rectangle");
            }
        }
        self.webcam_rect = rect;
        Ok(())
    }

    pub(super) fn resolved_webcam_rect(&self) -> [f32;4] {
        if let Some(rect) = self.webcam_rect { return rect; }
        let Some(camera) = &self.config.webcam else { return [0.0;4]; };
        let (w,h) = (self.config.output_width,self.config.output_height);
        if camera.shape == "fullscreen" { return [0.0,0.0,w as f32,h as f32]; }
        let bounds = crate::video_processing::webcam_timeline::contain_webcam(camera,w,h);
        let side = bounds.size * w as f32;
        [bounds.pos_x * w as f32 - side / 2.0,bounds.pos_y * h as f32 - side / 2.0,side,side]
    }

    pub(super) fn prepare_loupe_layers(&mut self)->bool {
        let active=self.loupe_content.is_some_and(|lens|(lens.bevel>0.0 || self.config.webcam.is_some()) && lens.presence>0.0 && lens.radius>0.0);
        if active && (self.loupe_layers.foreground.width()!=self.config.output_width
            || self.loupe_layers.foreground.height()!=self.config.output_height) {
            self.loupe_layers=StageLayers::new(&self.device,self.config.output_width,self.config.output_height);
            self.loupe_layered_pass=None;
            self.bind_group_dirty=true;
        }
        active
    }
}
