//! GPU-accelerated video compositing using wgpu.
//!
//! Replaces all CPU-based per-pixel operations (zoom, cursor blend, rounded corners,
//! shadow, webcam mask, motion blur, background compositing) with GPU compute shaders.
//! Matches CapCut-style GPU rendering for near-instant per-frame compositing.

use anyhow::{anyhow, Result};
use image::RgbaImage;
mod config;
mod init;
mod shader;
mod textures;

use shader::COMPOSITE_SHADER;

/// Per-frame zoom state (re-exported from zoom_trajectory)
use super::zoom_trajectory::ZoomFrameState;
pub use config::{build_gpu_config, build_gpu_config_with_webcam};

/// All visual effect settings needed to configure the GPU pipeline
#[derive(Debug, Clone)]
pub struct GpuCompositorConfig {
    /// Output dimensions
    pub output_width: u32,
    pub output_height: u32,
    /// Source content dimensions (after padding calculation)
    pub content_width: u32,
    pub content_height: u32,
    /// Content placement offset (padding)
    pub content_offset_x: u32,
    pub content_offset_y: u32,
    /// Input dimensions (aspect-fit source within content area)
    pub input_width: u32,
    pub input_height: u32,
    /// Input placement offset within content area (letterbox/pillarbox)
    pub input_offset_x: u32,
    pub input_offset_y: u32,
    /// Background color (RGBA, 0.0-1.0)
    pub background_color: [f32; 4],
    /// Background gradient settings
    pub background_gradient: Option<GradientConfig>,
    /// Background image for custom wallpaper export
    pub background_image: Option<RgbaImage>,
    /// Corner radius in pixels (0 = no rounding)
    pub corner_radius: f32,
    /// Shadow settings
    pub shadow_enabled: bool,
    pub shadow_blur: f32,
    pub shadow_intensity: f32,
    pub shadow_offset_x: f32,
    pub shadow_offset_y: f32,
    /// Webcam settings (None if no webcam)
    pub webcam: Option<WebcamConfig>,
    /// Device frame settings
    pub device_frame: Option<DeviceFrameConfig>,
    /// Motion blur settings
    pub motion_blur_enabled: bool,
    pub motion_blur_pan_intensity: f32,
    pub motion_blur_zoom_intensity: f32,
    /// Window mode: zoom applies to entire composite (video + background) as one unit
    pub window_mode: bool,
}

#[derive(Debug, Clone)]
pub struct WebcamConfig {
    pub pos_x: f32,
    pub pos_y: f32,
    pub size: f32,
    pub shape: String, // "circle" or "rounded"
}

#[derive(Debug, Clone)]
pub struct GradientConfig {
    pub direction: u32,
    pub colors: [[f32; 4]; 4],
    pub positions: [f32; 4],
    pub count: u32,
}

#[derive(Debug, Clone)]
pub struct DeviceFrameConfig {
    pub bezel: u32,
    pub corner_radius: u32,
    pub color: [f32; 4],
}

/// Uniforms pushed to the GPU each frame
#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
struct CompositeUniforms {
    // Dimensions
    output_width: f32,
    output_height: f32,
    content_width: f32,
    content_height: f32,

    // Content placement
    content_offset_x: f32,
    content_offset_y: f32,
    // Input (aspect-fit within content)
    input_width: f32,
    input_height: f32,

    input_offset_x: f32,
    input_offset_y: f32,
    // Zoom
    zoom_scale: f32,
    zoom_center_x: f32,

    zoom_center_y: f32,
    // Corner radius
    corner_radius: f32,
    // Background color
    bg_r: f32,
    bg_g: f32,

    bg_b: f32,
    bg_a: f32,
    bg_type: f32,
    bg_gradient_direction: f32,

    bg_gradient_count: f32,
    bg_pos_0: f32,
    bg_pos_1: f32,
    bg_pos_2: f32,

    bg_pos_3: f32,
    bg_color_0_r: f32,
    bg_color_0_g: f32,
    bg_color_0_b: f32,

    bg_color_0_a: f32,
    bg_color_1_r: f32,
    bg_color_1_g: f32,
    bg_color_1_b: f32,

    bg_color_1_a: f32,
    bg_color_2_r: f32,
    bg_color_2_g: f32,
    bg_color_2_b: f32,

    bg_color_2_a: f32,
    bg_color_3_r: f32,
    bg_color_3_g: f32,
    bg_color_3_b: f32,

    bg_color_3_a: f32,
    // Shadow
    shadow_enabled: f32,
    shadow_blur: f32,

    shadow_intensity: f32,
    shadow_offset_x: f32,
    shadow_offset_y: f32,
    // Motion blur
    motion_blur_enabled: f32,

    motion_blur_pan_intensity: f32,
    motion_blur_zoom_intensity: f32,
    velocity_x: f32,
    velocity_y: f32,

    velocity_scale: f32,
    // Webcam
    webcam_enabled: f32,
    webcam_pos_x: f32,
    webcam_pos_y: f32,

    webcam_size: f32,
    webcam_shape: f32, // 0=circle, 1=rounded
    // Device frame
    device_frame_enabled: f32,
    device_frame_bezel: f32,

    device_frame_corner_radius: f32,
    device_frame_r: f32,
    device_frame_g: f32,
    device_frame_b: f32,

    // Cursor (SDF rendering — no texture needed)
    cursor_enabled: f32,
    cursor_x: f32,
    cursor_y: f32,
    cursor_size: f32,

    cursor_opacity: f32,
    cursor_rotation: f32,
    cursor_style: f32, // 0=pointer,1=circle,2=filled,3=outline,4=dotted
    is_clicking: f32,

    click_effect: f32, // 0=none,1=circle,2=ripple
    cursor_color_r: f32,
    cursor_color_g: f32,
    cursor_color_b: f32,

    cursor_highlight_r: f32,
    cursor_highlight_g: f32,
    cursor_highlight_b: f32,
    cursor_shadow_intensity: f32,

    // Ripple effect
    ripple_progress: f32,
    ripple_x: f32,
    ripple_y: f32,
    ripple_r: f32,

    ripple_g: f32,
    ripple_b: f32,
    // Circle highlight
    circle_hl_progress: f32,
    circle_hl_x: f32,

    circle_hl_y: f32,
    // Trail
    trail_enabled: f32,
    trail_count: f32,
    trail_opacity: f32,

    // Window mode (zoom entire composite)
    window_mode: f32,
}

/// Trail point data sent to GPU (matches CursorFrameState trail_points)
#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
struct TrailPointGpu {
    x: f32,
    y: f32,
    alpha: f32,
    size: f32,
}

/// GPU compositor that holds all wgpu resources for the export pipeline.
pub struct GpuCompositor {
    device: wgpu::Device,
    queue: wgpu::Queue,
    composite_pipeline: wgpu::ComputePipeline,
    bind_group_layout: wgpu::BindGroupLayout,
    // Textures
    source_texture: wgpu::Texture,
    output_texture: wgpu::Texture,
    // Staging buffers for CPU↔GPU transfer
    #[allow(dead_code)]
    upload_buffer: wgpu::Buffer,
    download_buffer: wgpu::Buffer,
    // Uniform buffer
    uniform_buffer: wgpu::Buffer,
    // Trail buffer (30 × vec4 = 480 bytes)
    trail_buffer: wgpu::Buffer,
    // Placeholder cursor texture (1x1, keeps bind group layout stable)
    cursor_texture: wgpu::Texture,
    cursor_enabled: bool,
    cursor_size: f32,
    ripple_color: [f32; 3],
    // Cursor visual config (for SDF rendering)
    cursor_style: f32,
    cursor_color: [f32; 3],
    cursor_highlight_color: [f32; 3],
    cursor_shadow_intensity: f32,
    click_effect: f32,
    // Webcam texture
    webcam_texture: Option<wgpu::Texture>,
    // Custom background image texture
    background_texture: Option<wgpu::Texture>,
    // Pre-baked mask textures
    corner_mask_texture: wgpu::Texture,
    shadow_texture: wgpu::Texture,
    // Cached per-frame resources (avoid re-creation each frame)
    sampler: wgpu::Sampler,
    placeholder_texture: wgpu::Texture,
    // Cached bind group (rebuilt only when textures change)
    bind_group: Option<wgpu::BindGroup>,
    bind_group_dirty: bool,
    // Row alignment for GPU readback
    padded_bytes_per_row: u32,
    // Config
    config: GpuCompositorConfig,
    // Previous frame state for velocity calculation
    prev_zoom_state: Option<ZoomFrameState>,
}

/// WGSL compute shader that performs ALL compositing in one dispatch.
impl GpuCompositor {
    pub fn composite_frame(
        &mut self,
        source_frame: &[u8],
        zoom_state: &ZoomFrameState,
        cursor_state: Option<&crate::cursor_renderer::CursorFrameState>,
    ) -> Result<Vec<u8>> {
        let out_w = self.config.output_width;
        let out_h = self.config.output_height;
        let in_w = self.config.input_width;
        let in_h = self.config.input_height;
        let input_frame_size = (in_w * in_h * 4) as usize;

        if source_frame.len() != input_frame_size {
            return Err(anyhow!(
                "Source frame size mismatch: got {}, expected {} ({}x{})",
                source_frame.len(),
                input_frame_size,
                in_w,
                in_h
            ));
        }

        // Calculate velocity from previous frame for motion blur
        let (vel_x, vel_y, vel_scale) = if let Some(ref prev) = self.prev_zoom_state {
            (
                (zoom_state.center_x - prev.center_x) as f32,
                (zoom_state.center_y - prev.center_y) as f32,
                (zoom_state.scale - prev.scale) as f32,
            )
        } else {
            (0.0, 0.0, 0.0)
        };
        self.prev_zoom_state = Some(zoom_state.clone());

        // Build uniforms (only per-frame varying data)
        let webcam_cfg = &self.config.webcam;
        let df_cfg = &self.config.device_frame;
        let gradient = self.config.background_gradient.as_ref();
        let gradient_colors = gradient.map_or([[0.0, 0.0, 0.0, 1.0]; 4], |g| g.colors);
        let gradient_positions = gradient.map_or([0.0; 4], |g| g.positions);

        let uniforms = CompositeUniforms {
            output_width: out_w as f32,
            output_height: out_h as f32,
            content_width: self.config.content_width as f32,
            content_height: self.config.content_height as f32,
            content_offset_x: self.config.content_offset_x as f32,
            content_offset_y: self.config.content_offset_y as f32,
            input_width: self.config.input_width as f32,
            input_height: self.config.input_height as f32,
            input_offset_x: self.config.input_offset_x as f32,
            input_offset_y: self.config.input_offset_y as f32,
            zoom_scale: zoom_state.scale as f32,
            zoom_center_x: zoom_state.center_x as f32,
            zoom_center_y: zoom_state.center_y as f32,
            corner_radius: self.config.corner_radius,
            bg_r: self.config.background_color[0],
            bg_g: self.config.background_color[1],
            bg_b: self.config.background_color[2],
            bg_a: self.config.background_color[3],
            bg_type: if self.config.background_image.is_some() {
                2.0
            } else if gradient.is_some() {
                1.0
            } else {
                0.0
            },
            bg_gradient_direction: gradient.map_or(0.0, |g| g.direction as f32),
            bg_gradient_count: gradient.map_or(0.0, |g| g.count as f32),
            bg_pos_0: gradient_positions[0],
            bg_pos_1: gradient_positions[1],
            bg_pos_2: gradient_positions[2],
            bg_pos_3: gradient_positions[3],
            bg_color_0_r: gradient_colors[0][0],
            bg_color_0_g: gradient_colors[0][1],
            bg_color_0_b: gradient_colors[0][2],
            bg_color_0_a: gradient_colors[0][3],
            bg_color_1_r: gradient_colors[1][0],
            bg_color_1_g: gradient_colors[1][1],
            bg_color_1_b: gradient_colors[1][2],
            bg_color_1_a: gradient_colors[1][3],
            bg_color_2_r: gradient_colors[2][0],
            bg_color_2_g: gradient_colors[2][1],
            bg_color_2_b: gradient_colors[2][2],
            bg_color_2_a: gradient_colors[2][3],
            bg_color_3_r: gradient_colors[3][0],
            bg_color_3_g: gradient_colors[3][1],
            bg_color_3_b: gradient_colors[3][2],
            bg_color_3_a: gradient_colors[3][3],
            shadow_enabled: if self.config.shadow_enabled { 1.0 } else { 0.0 },
            shadow_blur: self.config.shadow_blur,
            shadow_intensity: self.config.shadow_intensity,
            shadow_offset_x: self.config.shadow_offset_x,
            shadow_offset_y: self.config.shadow_offset_y,
            motion_blur_enabled: if self.config.motion_blur_enabled {
                1.0
            } else {
                0.0
            },
            motion_blur_pan_intensity: self.config.motion_blur_pan_intensity,
            motion_blur_zoom_intensity: self.config.motion_blur_zoom_intensity,
            velocity_x: vel_x,
            velocity_y: vel_y,
            velocity_scale: vel_scale,
            webcam_enabled: if webcam_cfg.is_some() && self.webcam_texture.is_some() {
                1.0
            } else {
                0.0
            },
            webcam_pos_x: webcam_cfg.as_ref().map_or(0.0, |w| w.pos_x),
            webcam_pos_y: webcam_cfg.as_ref().map_or(0.0, |w| w.pos_y),
            webcam_size: webcam_cfg.as_ref().map_or(0.0, |w| w.size),
            webcam_shape: webcam_cfg.as_ref().map_or(0.0, |w| {
                if w.shape == "circle" {
                    0.0
                } else {
                    1.0
                }
            }),
            device_frame_enabled: if df_cfg.is_some() { 1.0 } else { 0.0 },
            device_frame_bezel: df_cfg.as_ref().map_or(0.0, |d| d.bezel as f32),
            device_frame_corner_radius: df_cfg.as_ref().map_or(0.0, |d| d.corner_radius as f32),
            device_frame_r: df_cfg.as_ref().map_or(0.0, |d| d.color[0]),
            device_frame_g: df_cfg.as_ref().map_or(0.0, |d| d.color[1]),
            device_frame_b: df_cfg.as_ref().map_or(0.0, |d| d.color[2]),
            cursor_enabled: if self.cursor_enabled && cursor_state.is_some() {
                1.0
            } else {
                0.0
            },
            cursor_x: cursor_state.map_or(0.0, |c| c.x),
            cursor_y: cursor_state.map_or(0.0, |c| c.y),
            cursor_size: self.cursor_size,
            cursor_opacity: cursor_state.map_or(1.0, |c| c.opacity),
            cursor_rotation: cursor_state.map_or(0.0, |c| c.rotation),
            cursor_style: self.cursor_style,
            is_clicking: cursor_state.map_or(0.0, |c| c.is_clicking),
            click_effect: self.click_effect,
            cursor_color_r: self.cursor_color[0],
            cursor_color_g: self.cursor_color[1],
            cursor_color_b: self.cursor_color[2],
            cursor_highlight_r: self.cursor_highlight_color[0],
            cursor_highlight_g: self.cursor_highlight_color[1],
            cursor_highlight_b: self.cursor_highlight_color[2],
            cursor_shadow_intensity: self.cursor_shadow_intensity,
            ripple_progress: cursor_state.map_or(0.0, |c| c.ripple_progress),
            ripple_x: cursor_state.map_or(0.0, |c| c.ripple_x),
            ripple_y: cursor_state.map_or(0.0, |c| c.ripple_y),
            ripple_r: self.ripple_color[0],
            ripple_g: self.ripple_color[1],
            ripple_b: self.ripple_color[2],
            circle_hl_progress: cursor_state.map_or(0.0, |c| c.circle_hl_progress),
            circle_hl_x: cursor_state.map_or(0.0, |c| c.circle_hl_x),
            circle_hl_y: cursor_state.map_or(0.0, |c| c.circle_hl_y),
            trail_enabled: if cursor_state.map_or(false, |c| c.trail_count > 0.0) {
                1.0
            } else {
                0.0
            },
            trail_count: cursor_state.map_or(0.0, |c| c.trail_count),
            trail_opacity: cursor_state.map_or(0.5, |_| {
                // trail_opacity is baked into trail_points alpha, just pass 1.0 as multiplier
                1.0
            }),
            window_mode: if self.config.window_mode { 1.0 } else { 0.0 },
        };

        // Upload trail points
        if let Some(cs) = cursor_state {
            let trail_gpu: Vec<TrailPointGpu> = cs
                .trail_points
                .iter()
                .map(|tp| TrailPointGpu {
                    x: tp.x,
                    y: tp.y,
                    alpha: tp.alpha,
                    size: tp.size,
                })
                .collect();
            self.queue
                .write_buffer(&self.trail_buffer, 0, bytemuck::cast_slice(&trail_gpu));
        }

        // Upload uniforms + source frame (the only per-frame GPU writes)
        self.queue
            .write_buffer(&self.uniform_buffer, 0, bytemuck::bytes_of(&uniforms));
        self.queue.write_texture(
            wgpu::ImageCopyTexture {
                texture: &self.source_texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            source_frame,
            wgpu::ImageDataLayout {
                offset: 0,
                bytes_per_row: Some(in_w * 4),
                rows_per_image: Some(in_h),
            },
            wgpu::Extent3d {
                width: in_w,
                height: in_h,
                depth_or_array_layers: 1,
            },
        );

        // Ensure bind group is built (cached, only rebuilt when textures change)
        self.ensure_bind_group();
        let bind_group = self.bind_group.as_ref().unwrap();

        // Encode and dispatch
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("Composite Encoder"),
            });

        {
            let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: Some("Composite Pass"),
                timestamp_writes: None,
            });
            pass.set_pipeline(&self.composite_pipeline);
            pass.set_bind_group(0, bind_group, &[]);
            pass.dispatch_workgroups((out_w + 15) / 16, (out_h + 15) / 16, 1);
        }

        // Copy output texture to download buffer
        encoder.copy_texture_to_buffer(
            wgpu::ImageCopyTexture {
                texture: &self.output_texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            wgpu::ImageCopyBuffer {
                buffer: &self.download_buffer,
                layout: wgpu::ImageDataLayout {
                    offset: 0,
                    bytes_per_row: Some(self.padded_bytes_per_row),
                    rows_per_image: Some(out_h),
                },
            },
            wgpu::Extent3d {
                width: out_w,
                height: out_h,
                depth_or_array_layers: 1,
            },
        );

        self.queue.submit(std::iter::once(encoder.finish()));

        // Read back the result
        let buffer_slice = self.download_buffer.slice(..);
        let (sender, receiver) = std::sync::mpsc::channel();
        buffer_slice.map_async(wgpu::MapMode::Read, move |result| {
            let _ = sender.send(result);
        });
        self.device.poll(wgpu::Maintain::Wait);
        receiver
            .recv()
            .map_err(|e| anyhow!("GPU readback channel error: {}", e))?
            .map_err(|e| anyhow!("GPU readback map error: {:?}", e))?;

        let data = buffer_slice.get_mapped_range();
        let unpadded_bytes_per_row = (out_w * 4) as usize;
        let padded = self.padded_bytes_per_row as usize;
        let result = if padded != unpadded_bytes_per_row {
            let mut result = Vec::with_capacity(unpadded_bytes_per_row * out_h as usize);
            for row in 0..out_h as usize {
                let start = row * padded;
                result.extend_from_slice(&data[start..start + unpadded_bytes_per_row]);
            }
            result
        } else {
            data.to_vec()
        };
        drop(data);
        self.download_buffer.unmap();

        Ok(result)
    }
}
