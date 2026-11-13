use super::*;

impl GpuCompositor {
    /// Window coordinates use the sampled source texture's normalized UV space.
    pub(crate) fn set_detail_texture(&mut self, texture: Option<&wgpu::Texture>,
        window: [f32; 4], strength: f32) -> Result<()> {
        let mut values = [0.0f32; 8];
        if let Some(texture) = texture {
            if !window.iter().all(|v| v.is_finite()) || window[2] <= 0.0 || window[3] <= 0.0
                || !strength.is_finite() || !(0.0..=1.0).contains(&strength) {
                return Err(anyhow!("Invalid detail sampling window or strength"));
            }
            values[..4].copy_from_slice(&window);
            values[4] = texture.width() as f32;
            values[5] = texture.height() as f32;
            values[6] = strength;
        }
        if self.detail_texture.as_ref() != texture {
            self.detail_texture = texture.cloned();
            self.bind_group_dirty = true;
        }
        self.queue.write_buffer(&self.detail_uniforms, 0,
            &values.into_iter().flat_map(f32::to_le_bytes).collect::<Vec<_>>());
        Ok(())
    }
}
