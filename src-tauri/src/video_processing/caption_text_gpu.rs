//! Reusable caption glyph textures and placement buffers; no per-frame text rasterization.
use super::{
    caption_overlay::{Raster, Word},
    shortcut_gpu::KeycapGpu,
    shortcut_motion::Draw,
};
use anyhow::Result;
pub(super) struct CaptionTextGpu {
    atlas: KeycapGpu,
    draws: Vec<Draw>,
    radii: Vec<f32>,
}
impl CaptionTextGpu {
    pub fn new(device: &wgpu::Device, queue: &wgpu::Queue, tiles: Vec<Raster>) -> Result<Self> {
        let rasters = tiles
            .into_iter()
            .map(|tile| super::shortcut_overlay::Raster {
                x: 0,
                y: 0,
                width: tile.width,
                height: tile.height,
                pixels: tile.pixels,
            })
            .collect::<Vec<_>>();
        Ok(Self {
            atlas: KeycapGpu::new(device, queue, &rasters)?,
            draws: Vec::new(),
            radii: Vec::new(),
        })
    }
    pub fn encode(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        target: &wgpu::Texture,
        words: &[Word],
        clear: bool,
    ) -> Result<()> {
        self.draws.clear();
        self.radii.clear();
        for word in words {
            let part = &word.part;
            self.draws.push(Draw {
                tile: word.tile,
                x: part.x,
                y: part.y,
                width: part.width,
                height: part.height,
                opacity: part.opacity,
            });
            self.radii.push(part.blur_radius as f32);
        }
        self.atlas.encode_with_blurs(
            device,
            queue,
            encoder,
            target,
            &self.draws,
            &self.radii,
            clear,
        )
    }
}

#[cfg(test)]
#[path = "caption_text_gpu_tests.rs"]
mod tests;
