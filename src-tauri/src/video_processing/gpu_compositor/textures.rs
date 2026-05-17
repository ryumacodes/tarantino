use super::*;

impl GpuCompositor {
    pub(super) fn create_rgba_texture(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        label: &str,
        width: u32,
        height: u32,
        data: &[u8],
    ) -> wgpu::Texture {
        println!(
            "[Wallpaper Image] uploading RGBA texture '{}': {}x{}, {} bytes",
            label,
            width,
            height,
            data.len()
        );
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some(label),
            size: wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8Unorm,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        queue.write_texture(
            wgpu::ImageCopyTexture {
                texture: &texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            data,
            wgpu::ImageDataLayout {
                offset: 0,
                bytes_per_row: Some(width * 4),
                rows_per_image: Some(height),
            },
            wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
        );
        texture
    }

    pub fn set_webcam_texture(&mut self, width: u32, height: u32) {
        self.webcam_texture = Some(self.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("Webcam"),
            size: wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8Unorm,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        }));
        self.bind_group_dirty = true;
    }

    /// Build or return the cached bind group.
    pub(super) fn ensure_bind_group(&mut self) {
        if !self.bind_group_dirty && self.bind_group.is_some() {
            return;
        }

        let source_view = self
            .source_texture
            .create_view(&wgpu::TextureViewDescriptor::default());
        let output_view = self
            .output_texture
            .create_view(&wgpu::TextureViewDescriptor::default());
        let corner_mask_view = self
            .corner_mask_texture
            .create_view(&wgpu::TextureViewDescriptor::default());
        let shadow_view = self
            .shadow_texture
            .create_view(&wgpu::TextureViewDescriptor::default());
        let placeholder_view_for_webcam = self
            .placeholder_texture
            .create_view(&wgpu::TextureViewDescriptor::default());
        let placeholder_view_for_background = self
            .placeholder_texture
            .create_view(&wgpu::TextureViewDescriptor::default());

        let cursor_view = self
            .cursor_texture
            .create_view(&wgpu::TextureViewDescriptor::default());

        let webcam_view = self
            .webcam_texture
            .as_ref()
            .map(|t| t.create_view(&wgpu::TextureViewDescriptor::default()))
            .unwrap_or(placeholder_view_for_webcam);

        let background_view = self
            .background_texture
            .as_ref()
            .map(|t| t.create_view(&wgpu::TextureViewDescriptor::default()))
            .unwrap_or(placeholder_view_for_background);

        self.bind_group = Some(self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Composite BG"),
            layout: &self.bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: self.uniform_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(&source_view),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::TextureView(&output_view),
                },
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: wgpu::BindingResource::TextureView(&corner_mask_view),
                },
                wgpu::BindGroupEntry {
                    binding: 4,
                    resource: wgpu::BindingResource::TextureView(&shadow_view),
                },
                wgpu::BindGroupEntry {
                    binding: 5,
                    resource: wgpu::BindingResource::TextureView(&cursor_view),
                },
                wgpu::BindGroupEntry {
                    binding: 6,
                    resource: wgpu::BindingResource::TextureView(&webcam_view),
                },
                wgpu::BindGroupEntry {
                    binding: 7,
                    resource: wgpu::BindingResource::Sampler(&self.sampler),
                },
                wgpu::BindGroupEntry {
                    binding: 8,
                    resource: self.trail_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 9,
                    resource: wgpu::BindingResource::TextureView(&background_view),
                },
            ],
        }));
        self.bind_group_dirty = false;
    }

    /// Upload a webcam frame for the current video frame.
    #[allow(dead_code)]
    pub fn upload_webcam_frame(&self, data: &[u8], width: u32, height: u32) {
        if let Some(ref tex) = self.webcam_texture {
            self.queue.write_texture(
                wgpu::ImageCopyTexture {
                    texture: tex,
                    mip_level: 0,
                    origin: wgpu::Origin3d::ZERO,
                    aspect: wgpu::TextureAspect::All,
                },
                data,
                wgpu::ImageDataLayout {
                    offset: 0,
                    bytes_per_row: Some(width * 4),
                    rows_per_image: Some(height),
                },
                wgpu::Extent3d {
                    width,
                    height,
                    depth_or_array_layers: 1,
                },
            );
        }
    }

    /// Composite a single frame on the GPU.
    ///
    /// Takes raw RGBA source frame, applies zoom, motion blur, rounded corners,
    /// shadow, cursor, webcam, device frame — returns composited RGBA output.
    ///
    /// Bind group and sampler are cached across frames to avoid per-frame allocation.

    pub(super) fn create_corner_mask_texture(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        width: u32,
        height: u32,
        corner_radius: f32,
    ) -> wgpu::Texture {
        let mut data = vec![255u8; (width * height * 4) as usize];

        if corner_radius > 0.0 {
            let r = corner_radius.min(width.min(height) as f32 / 2.0);
            for y in 0..height {
                for x in 0..width {
                    let idx = ((y * width + x) * 4) as usize;
                    // Check each corner
                    let alpha = Self::rounded_corner_alpha(
                        x as f32,
                        y as f32,
                        width as f32,
                        height as f32,
                        r,
                    );
                    let a = (alpha * 255.0).clamp(0.0, 255.0) as u8;
                    data[idx] = 255; // R
                    data[idx + 1] = 255; // G
                    data[idx + 2] = 255; // B
                    data[idx + 3] = a; // A
                }
            }
        }

        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("Corner Mask"),
            size: wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8Unorm,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });

        queue.write_texture(
            wgpu::ImageCopyTexture {
                texture: &texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            &data,
            wgpu::ImageDataLayout {
                offset: 0,
                bytes_per_row: Some(width * 4),
                rows_per_image: Some(height),
            },
            wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
        );

        texture
    }

    fn rounded_corner_alpha(x: f32, y: f32, w: f32, h: f32, r: f32) -> f32 {
        // For each corner, compute distance to the corner arc
        let corners: [(f32, f32); 4] = [
            (r, r),         // top-left
            (w - r, r),     // top-right
            (r, h - r),     // bottom-left
            (w - r, h - r), // bottom-right
        ];

        for &(cx, cy) in &corners {
            let in_corner_x = if cx < w / 2.0 { x < cx } else { x > cx };
            let in_corner_y = if cy < h / 2.0 { y < cy } else { y > cy };
            if in_corner_x && in_corner_y {
                let dx = x - cx;
                let dy = y - cy;
                let dist = (dx * dx + dy * dy).sqrt();
                // Smooth antialiased edge
                return 1.0 - (dist - r + 0.5).clamp(0.0, 1.0);
            }
        }
        1.0
    }

    /// Pre-bake shadow texture (computed once, reused every frame).
    pub(super) fn create_shadow_texture(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        config: &GpuCompositorConfig,
    ) -> wgpu::Texture {
        let out_w = config.output_width;
        let out_h = config.output_height;
        let mut data = vec![0u8; (out_w * out_h * 4) as usize];

        if config.shadow_enabled {
            let blur_radius = config.shadow_blur as i32;
            let intensity = config.shadow_intensity;
            let offset_x = config.shadow_offset_x as i32;
            let offset_y = config.shadow_offset_y as i32;
            let content_w = config.content_width as i32;
            let content_h = config.content_height as i32;
            let cx = config.content_offset_x as i32 + offset_x;
            let cy = config.content_offset_y as i32 + offset_y;
            let corner_radius = config.corner_radius;

            // Generate shadow shape (content rect with rounded corners, offset)
            let mut shadow_mask = vec![0.0f32; (out_w * out_h) as usize];
            for y in 0..out_h as i32 {
                for x in 0..out_w as i32 {
                    let lx = x - cx;
                    let ly = y - cy;
                    if lx >= 0 && lx < content_w && ly >= 0 && ly < content_h {
                        let alpha = Self::rounded_corner_alpha(
                            lx as f32,
                            ly as f32,
                            content_w as f32,
                            content_h as f32,
                            corner_radius,
                        );
                        shadow_mask[(y as u32 * out_w + x as u32) as usize] = alpha * intensity;
                    }
                }
            }

            // Box blur (separable: horizontal then vertical)
            if blur_radius > 0 {
                let mut temp = vec![0.0f32; (out_w * out_h) as usize];
                let kernel = (blur_radius * 2 + 1) as f32;

                // Horizontal pass
                for y in 0..out_h as i32 {
                    for x in 0..out_w as i32 {
                        let mut sum = 0.0;
                        for dx in -blur_radius..=blur_radius {
                            let sx = (x + dx).clamp(0, out_w as i32 - 1);
                            sum += shadow_mask[(y as u32 * out_w + sx as u32) as usize];
                        }
                        temp[(y as u32 * out_w + x as u32) as usize] = sum / kernel;
                    }
                }

                // Vertical pass
                for y in 0..out_h as i32 {
                    for x in 0..out_w as i32 {
                        let mut sum = 0.0;
                        for dy in -blur_radius..=blur_radius {
                            let sy = (y + dy).clamp(0, out_h as i32 - 1);
                            sum += temp[(sy as u32 * out_w + x as u32) as usize];
                        }
                        shadow_mask[(y as u32 * out_w + x as u32) as usize] = sum / kernel;
                    }
                }
            }

            // Write shadow to texture data (black color with alpha from mask)
            for y in 0..out_h {
                for x in 0..out_w {
                    let idx = ((y * out_w + x) * 4) as usize;
                    let alpha = shadow_mask[(y * out_w + x) as usize];
                    data[idx] = 0; // R
                    data[idx + 1] = 0; // G
                    data[idx + 2] = 0; // B
                    data[idx + 3] = (alpha * 255.0).clamp(0.0, 255.0) as u8;
                }
            }
        }

        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("Shadow"),
            size: wgpu::Extent3d {
                width: out_w,
                height: out_h,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8Unorm,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });

        queue.write_texture(
            wgpu::ImageCopyTexture {
                texture: &texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            &data,
            wgpu::ImageDataLayout {
                offset: 0,
                bytes_per_row: Some(out_w * 4),
                rows_per_image: Some(out_h),
            },
            wgpu::Extent3d {
                width: out_w,
                height: out_h,
                depth_or_array_layers: 1,
            },
        );

        texture
    }
}
