//! GPU-accelerated video compositing using wgpu.
//!
//! Replaces all CPU-based per-pixel operations (zoom, cursor blend, rounded corners,
//! shadow, webcam mask, motion blur, background compositing) with GPU compute shaders.
//! Matches CapCut-style GPU rendering for near-instant per-frame compositing.

use anyhow::{Result, anyhow};
use image::RgbaImage;

/// Per-frame zoom state (re-exported from zoom_trajectory)
use super::zoom_trajectory::ZoomFrameState;

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
}

#[derive(Debug, Clone)]
pub struct WebcamConfig {
    pub pos_x: f32,
    pub pos_y: f32,
    pub size: f32,
    pub shape: String, // "circle" or "rounded"
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
    cursor_style: f32,    // 0=pointer,1=circle,2=filled,3=outline,4=dotted
    is_clicking: f32,

    click_effect: f32,    // 0=none,1=circle,2=ripple
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
/// Cursor rendering uses SDF (no texture) — matches preview GLSL shader exactly.
const COMPOSITE_SHADER: &str = r#"
struct Uniforms {
    output_width: f32,
    output_height: f32,
    content_width: f32,
    content_height: f32,
    content_offset_x: f32,
    content_offset_y: f32,
    input_width: f32,
    input_height: f32,
    input_offset_x: f32,
    input_offset_y: f32,
    zoom_scale: f32,
    zoom_center_x: f32,
    zoom_center_y: f32,
    corner_radius: f32,
    bg_r: f32,
    bg_g: f32,
    bg_b: f32,
    bg_a: f32,
    shadow_enabled: f32,
    shadow_blur: f32,
    shadow_intensity: f32,
    shadow_offset_x: f32,
    shadow_offset_y: f32,
    motion_blur_enabled: f32,
    motion_blur_pan_intensity: f32,
    motion_blur_zoom_intensity: f32,
    velocity_x: f32,
    velocity_y: f32,
    velocity_scale: f32,
    webcam_enabled: f32,
    webcam_pos_x: f32,
    webcam_pos_y: f32,
    webcam_size: f32,
    webcam_shape: f32,
    device_frame_enabled: f32,
    device_frame_bezel: f32,
    device_frame_corner_radius: f32,
    device_frame_r: f32,
    device_frame_g: f32,
    device_frame_b: f32,
    // Cursor SDF fields
    cursor_enabled: f32,
    cursor_x: f32,
    cursor_y: f32,
    cursor_size: f32,
    cursor_opacity: f32,
    cursor_rotation: f32,
    cursor_style: f32,
    is_clicking: f32,
    click_effect: f32,
    cursor_color_r: f32,
    cursor_color_g: f32,
    cursor_color_b: f32,
    cursor_highlight_r: f32,
    cursor_highlight_g: f32,
    cursor_highlight_b: f32,
    cursor_shadow_intensity: f32,
    ripple_progress: f32,
    ripple_x: f32,
    ripple_y: f32,
    ripple_r: f32,
    ripple_g: f32,
    ripple_b: f32,
    circle_hl_progress: f32,
    circle_hl_x: f32,
    circle_hl_y: f32,
    trail_enabled: f32,
    trail_count: f32,
    trail_opacity: f32,
};

struct TrailPoint {
    x: f32,
    y: f32,
    alpha: f32,
    size: f32,
};

@group(0) @binding(0) var<uniform> u: Uniforms;
@group(0) @binding(1) var source_tex: texture_2d<f32>;
@group(0) @binding(2) var output_tex: texture_storage_2d<rgba8unorm, write>;
@group(0) @binding(3) var corner_mask_tex: texture_2d<f32>;
@group(0) @binding(4) var shadow_tex: texture_2d<f32>;
@group(0) @binding(5) var cursor_tex: texture_2d<f32>;
@group(0) @binding(6) var webcam_tex: texture_2d<f32>;
@group(0) @binding(7) var bilinear_sampler: sampler;
@group(0) @binding(8) var<storage, read> trail_points: array<TrailPoint, 30>;

// Sample source texture with bilinear zoom applied
fn sample_zoomed(uv: vec2<f32>) -> vec4<f32> {
    let scale = u.zoom_scale;
    if (scale <= 1.001) {
        return textureSampleLevel(source_tex, bilinear_sampler, uv, 0.0);
    }
    let vis_size = 1.0 / scale;
    let half_vis = vis_size / 2.0;
    let src_x = clamp(u.zoom_center_x - half_vis, 0.0, 1.0 - vis_size);
    let src_y = clamp(u.zoom_center_y - half_vis, 0.0, 1.0 - vis_size);
    let src_uv = vec2<f32>(src_x + uv.x * vis_size, src_y + uv.y * vis_size);
    return textureSampleLevel(source_tex, bilinear_sampler, src_uv, 0.0);
}

// Sample with motion blur (directional + radial blur)
fn sample_with_motion_blur(uv: vec2<f32>) -> vec4<f32> {
    if (u.motion_blur_enabled < 0.5) {
        return sample_zoomed(uv);
    }
    let pan_speed = sqrt(u.velocity_x * u.velocity_x + u.velocity_y * u.velocity_y);
    let pan_blur = min(pan_speed * u.motion_blur_pan_intensity * 0.008, 0.012);
    let zoom_speed = abs(u.velocity_scale);
    let zoom_blur = min(zoom_speed * u.motion_blur_zoom_intensity * 0.006, 0.008);
    if (pan_blur < 0.001 && zoom_blur < 0.001) {
        return sample_zoomed(uv);
    }
    var pan_dir = vec2<f32>(u.velocity_x, u.velocity_y);
    let dir_mag = length(pan_dir);
    if (dir_mag > 0.001) { pan_dir = pan_dir / dir_mag; } else { pan_dir = vec2<f32>(0.0); }
    var color = vec4<f32>(0.0);
    var total_weight = 0.0;
    let samples = 12;
    for (var i = 0; i < samples; i = i + 1) {
        let t = f32(i) / f32(samples - 1) - 0.5;
        let weight = 1.0 - abs(t) * 0.5;
        var offset = pan_dir * pan_blur * t;
        let from_center = uv - vec2<f32>(0.5);
        offset = offset + from_center * zoom_blur * t;
        let sample_uv = clamp(uv + offset, vec2<f32>(0.0), vec2<f32>(1.0));
        color = color + sample_zoomed(sample_uv) * weight;
        total_weight = total_weight + weight;
    }
    return color / total_weight;
}

fn is_inside_rounded_rect(pos: vec2<f32>, size: vec2<f32>, radius: f32) -> f32 {
    if (radius <= 0.0) { return 1.0; }
    let r = min(radius, min(size.x, size.y) / 2.0);
    let q = abs(pos - size / 2.0) - size / 2.0 + vec2<f32>(r);
    let d = length(max(q, vec2<f32>(0.0))) - r;
    return 1.0 - smoothstep(-0.5, 0.5, d);
}

fn alpha_blend(dst: vec4<f32>, src: vec4<f32>) -> vec4<f32> {
    let out_a = src.a + dst.a * (1.0 - src.a);
    if (out_a < 0.001) { return vec4<f32>(0.0); }
    let out_rgb = (src.rgb * src.a + dst.rgb * dst.a * (1.0 - src.a)) / out_a;
    return vec4<f32>(out_rgb, out_a);
}

// --- SDF cursor functions (matching preview GLSL shader) ---

fn sd_segment(p: vec2<f32>, a: vec2<f32>, b: vec2<f32>) -> f32 {
    let pa = p - a;
    let ba = b - a;
    let h = clamp(dot(pa, ba) / dot(ba, ba), 0.0, 1.0);
    return length(pa - ba * h);
}

fn rotate_2d(p: vec2<f32>, angle: f32) -> vec2<f32> {
    let c = cos(angle);
    let s = sin(angle);
    return vec2<f32>(c * p.x - s * p.y, s * p.x + c * p.y);
}

// Arrow polygon vertices
fn arrow_v(i: i32, s: f32) -> vec2<f32> {
    switch(i) {
        case 0: { return vec2<f32>(0.0, 0.0); }
        case 1: { return vec2<f32>(0.0, 16.0 * s); }
        case 2: { return vec2<f32>(4.0 * s, 12.0 * s); }
        case 3: { return vec2<f32>(7.0 * s, 18.0 * s); }
        case 4: { return vec2<f32>(9.5 * s, 17.0 * s); }
        case 5: { return vec2<f32>(6.5 * s, 11.0 * s); }
        case 6: { return vec2<f32>(12.0 * s, 11.0 * s); }
        default: { return vec2<f32>(0.0); }
    }
}

// Winding number test for arrow polygon
fn arrow_mask(p: vec2<f32>, s: f32) -> f32 {
    var wn: i32 = 0;
    for (var i = 0; i < 7; i = i + 1) {
        let j = (i + 1) % 7;
        let vi = arrow_v(i, s);
        let vj = arrow_v(j, s);
        if (vi.y <= p.y) {
            if (vj.y > p.y) {
                let cross_val = (vj.x - vi.x) * (p.y - vi.y) - (p.x - vi.x) * (vj.y - vi.y);
                if (cross_val > 0.0) { wn = wn + 1; }
            }
        } else {
            if (vj.y <= p.y) {
                let cross_val = (vj.x - vi.x) * (p.y - vi.y) - (p.x - vi.x) * (vj.y - vi.y);
                if (cross_val < 0.0) { wn = wn - 1; }
            }
        }
    }
    // Min edge distance for AA
    var min_dist = sd_segment(p, arrow_v(0, s), arrow_v(1, s));
    for (var i = 1; i < 7; i = i + 1) {
        let j = (i + 1) % 7;
        min_dist = min(min_dist, sd_segment(p, arrow_v(i, s), arrow_v(j, s)));
    }
    if (wn != 0) {
        return smoothstep(0.0, 0.5, min_dist);
    }
    return 0.0;
}

// Min distance to any arrow edge (for stroke rendering)
fn arrow_edge_dist(p: vec2<f32>, s: f32) -> f32 {
    var min_dist = sd_segment(p, arrow_v(0, s), arrow_v(1, s));
    for (var i = 1; i < 7; i = i + 1) {
        let j = (i + 1) % 7;
        min_dist = min(min_dist, sd_segment(p, arrow_v(i, s), arrow_v(j, s)));
    }
    return min_dist;
}

@compute @workgroup_size(16, 16)
fn main(@builtin(global_invocation_id) gid: vec3<u32>) {
    let x = gid.x;
    let y = gid.y;
    let out_w = u32(u.output_width);
    let out_h = u32(u.output_height);
    if (x >= out_w || y >= out_h) { return; }

    let pixel = vec2<i32>(i32(x), i32(y));
    let px = vec2<f32>(f32(x), f32(y));

    // 1. Background color
    var result = vec4<f32>(u.bg_r, u.bg_g, u.bg_b, u.bg_a);

    // 2. Shadow
    if (u.shadow_enabled > 0.5) {
        let shadow_uv = vec2<f32>(f32(x) / u.output_width, f32(y) / u.output_height);
        let shadow = textureSampleLevel(shadow_tex, bilinear_sampler, shadow_uv, 0.0);
        result = alpha_blend(result, shadow);
    }

    // 3. Content (zoomed video + motion blur + rounded corners)
    let content_x = f32(x) - u.content_offset_x;
    let content_y = f32(y) - u.content_offset_y;
    if (content_x >= 0.0 && content_x < u.content_width && content_y >= 0.0 && content_y < u.content_height) {
        // Map content pixel to input area (centered within content)
        let input_x = content_x - u.input_offset_x;
        let input_y = content_y - u.input_offset_y;
        if (input_x >= 0.0 && input_x < u.input_width && input_y >= 0.0 && input_y < u.input_height) {
            let content_uv = vec2<f32>(input_x / u.input_width, input_y / u.input_height);
            var content_color = sample_with_motion_blur(content_uv);
            if (u.corner_radius > 0.0) {
                let mask_uv = vec2<f32>(content_x / u.content_width, content_y / u.content_height);
                let mask = textureSampleLevel(corner_mask_tex, bilinear_sampler, mask_uv, 0.0);
                content_color.a = content_color.a * mask.a;
            }
            result = alpha_blend(result, content_color);
        }
    }

    // 4. Cursor SDF rendering
    if (u.cursor_enabled > 0.5) {
        let s = u.cursor_size;
        let cursor_px = vec2<f32>(
            u.content_offset_x + u.input_offset_x + u.cursor_x * u.input_width,
            u.content_offset_y + u.input_offset_y + u.cursor_y * u.input_height
        );
        let max_radius = 60.0 * s;
        let dist_to_cursor = length(px - cursor_px);

        let active_color = select(
            vec3<f32>(u.cursor_color_r, u.cursor_color_g, u.cursor_color_b),
            vec3<f32>(u.cursor_highlight_r, u.cursor_highlight_g, u.cursor_highlight_b),
            u.is_clicking > 0.5
        );
        let ripple_color = vec3<f32>(u.ripple_r, u.ripple_g, u.ripple_b);
        let hl_color = vec3<f32>(u.cursor_highlight_r, u.cursor_highlight_g, u.cursor_highlight_b);
        let opacity = u.cursor_opacity;

        // 4a. Trail
        if (u.trail_enabled > 0.5) {
            let tc = i32(u.trail_count);
            for (var i = 0; i < 30; i = i + 1) {
                if (i >= tc - 1) { break; }
                let tp = trail_points[i];
                let trail_px = vec2<f32>(
                    u.content_offset_x + u.input_offset_x + tp.x * u.input_width,
                    u.content_offset_y + u.input_offset_y + tp.y * u.input_height
                );
                let dist = length(px - trail_px);
                let mask = 1.0 - smoothstep(tp.size - 0.5, tp.size + 0.5, dist);
                if (mask * tp.alpha > 0.001) {
                    let tc_color = vec3<f32>(u.cursor_color_r, u.cursor_color_g, u.cursor_color_b);
                    result = alpha_blend(result, vec4<f32>(tc_color, mask * tp.alpha));
                }
            }
        }

        // 4b. Ripple effect (click_effect == 2)
        if (u.ripple_progress > 0.001 && u.ripple_progress < 1.0 && u.click_effect > 1.5) {
            let rpx = vec2<f32>(
                u.content_offset_x + u.input_offset_x + u.ripple_x * u.input_width,
                u.content_offset_y + u.input_offset_y + u.ripple_y * u.input_height
            );
            let dist = length(px - rpx);
            let p = u.ripple_progress;
            // Outer ring
            let outer_r = (8.0 + p * 40.0) * s;
            let ring = abs(dist - outer_r);
            let ring_alpha = 1.0 - smoothstep(0.0, 2.5, ring);
            let fade = (1.0 - p) * 0.6;
            if (ring_alpha * fade > 0.001) {
                result = alpha_blend(result, vec4<f32>(ripple_color, ring_alpha * fade));
            }
            // Inner fill (first 50%)
            if (p < 0.5) {
                let inner_r = (4.0 + p * 20.0) * s;
                let inner_alpha = (0.5 - p) * 0.4;
                let inner_mask = 1.0 - smoothstep(inner_r - 0.5, inner_r + 0.5, dist);
                if (inner_mask * inner_alpha > 0.001) {
                    result = alpha_blend(result, vec4<f32>(ripple_color, inner_mask * inner_alpha));
                }
            }
        }

        // 4c. Circle highlight (click_effect == 1)
        if (u.circle_hl_progress > 0.001 && u.circle_hl_progress < 1.0 && u.click_effect > 0.5 && u.click_effect < 1.5) {
            let hl_px = vec2<f32>(
                u.content_offset_x + u.input_offset_x + u.circle_hl_x * u.input_width,
                u.content_offset_y + u.input_offset_y + u.circle_hl_y * u.input_height
            );
            let dist = length(px - hl_px);
            let p = u.circle_hl_progress;
            let radius = 20.0 * s;
            let alpha = 1.0 - p;
            // Fill
            let fill_mask = 1.0 - smoothstep(radius - 0.5, radius + 0.5, dist);
            if (fill_mask * alpha * 0.24 > 0.001) {
                result = alpha_blend(result, vec4<f32>(hl_color, fill_mask * alpha * 0.24));
            }
            // Stroke
            let stroke_ring = abs(dist - radius);
            let stroke_mask = 1.0 - smoothstep(0.0, 2.0, stroke_ring);
            if (stroke_mask * alpha * 0.8 > 0.001) {
                result = alpha_blend(result, vec4<f32>(hl_color, stroke_mask * alpha * 0.8));
            }
        }

        // 4d-f. Cursor body (shadow + SDF shape)
        if (dist_to_cursor < max_radius && opacity > 0.01) {
            var local = px - cursor_px;
            // Apply rotation
            let rot_rad = u.cursor_rotation * 3.14159265 / 180.0;
            if (abs(rot_rad) > 0.001) {
                local = rotate_2d(local, -rot_rad);
            }

            let shadow_alpha_base = (u.cursor_shadow_intensity / 100.0) * 0.5;

            if (u.cursor_style < 0.5) {
                // Pointer style
                if (shadow_alpha_base > 0.001) {
                    let sl = local - vec2<f32>(2.0 * s, 2.0 * s);
                    let sf = arrow_mask(sl, s);
                    if (sf * shadow_alpha_base * opacity > 0.001) {
                        result = alpha_blend(result, vec4<f32>(0.0, 0.0, 0.0, sf * shadow_alpha_base * opacity));
                    }
                }
                let fill = arrow_mask(local, s);
                if (fill > 0.001) {
                    result = alpha_blend(result, vec4<f32>(active_color, fill * opacity));
                }
                let edge_d = arrow_edge_dist(local, s);
                let stroke_w = 1.5 * s;
                let stroke_m = 1.0 - smoothstep(0.0, stroke_w, edge_d);
                let stroke_a = stroke_m * (1.0 - fill * 0.5);
                if (stroke_a * opacity > 0.001) {
                    result = alpha_blend(result, vec4<f32>(0.0, 0.0, 0.0, stroke_a * opacity));
                }
            } else if (u.cursor_style < 1.5) {
                // Circle style
                if (shadow_alpha_base > 0.001) {
                    let sl = local - vec2<f32>(2.0 * s, 2.0 * s);
                    let sd = length(sl);
                    let sm = 1.0 - smoothstep(10.0 * s - 0.5, 10.0 * s + 0.5, sd);
                    if (sm * shadow_alpha_base * opacity > 0.001) {
                        result = alpha_blend(result, vec4<f32>(0.0, 0.0, 0.0, sm * shadow_alpha_base * opacity));
                    }
                }
                let dist = length(local);
                let mask = 1.0 - smoothstep(10.0 * s - 0.5, 10.0 * s + 0.5, dist);
                if (mask * 0.8 * opacity > 0.001) {
                    result = alpha_blend(result, vec4<f32>(0.5, 0.5, 0.5, mask * 0.8 * opacity));
                }
            } else if (u.cursor_style < 2.5) {
                // Filled style (black fill, white stroke)
                if (shadow_alpha_base > 0.001) {
                    let sl = local - vec2<f32>(2.0 * s, 2.0 * s);
                    let sf = arrow_mask(sl, s);
                    if (sf * shadow_alpha_base * opacity > 0.001) {
                        result = alpha_blend(result, vec4<f32>(0.0, 0.0, 0.0, sf * shadow_alpha_base * opacity));
                    }
                }
                let fill = arrow_mask(local, s);
                if (fill > 0.001) {
                    result = alpha_blend(result, vec4<f32>(0.0, 0.0, 0.0, fill * opacity));
                }
                let edge_d = arrow_edge_dist(local, s);
                let stroke_m = 1.0 - smoothstep(0.0, 2.0 * s, edge_d);
                let stroke_a = stroke_m * (1.0 - fill * 0.5);
                if (stroke_a * opacity > 0.001) {
                    result = alpha_blend(result, vec4<f32>(1.0, 1.0, 1.0, stroke_a * opacity));
                }
            } else if (u.cursor_style < 3.5) {
                // Outline style (white stroke only)
                if (shadow_alpha_base > 0.001) {
                    let sl = local - vec2<f32>(2.0 * s, 2.0 * s);
                    let se = arrow_edge_dist(sl, s);
                    let ss = 1.0 - smoothstep(0.0, 2.0 * s, se);
                    if (ss * shadow_alpha_base * opacity > 0.001) {
                        result = alpha_blend(result, vec4<f32>(0.0, 0.0, 0.0, ss * shadow_alpha_base * opacity));
                    }
                }
                let edge_d = arrow_edge_dist(local, s);
                let stroke_m = 1.0 - smoothstep(0.0, 2.0 * s, edge_d);
                if (stroke_m * opacity > 0.001) {
                    result = alpha_blend(result, vec4<f32>(1.0, 1.0, 1.0, stroke_m * opacity));
                }
            } else {
                // Dotted style (fill + dashed stroke)
                if (shadow_alpha_base > 0.001) {
                    let sl = local - vec2<f32>(2.0 * s, 2.0 * s);
                    let sf = arrow_mask(sl, s);
                    if (sf * shadow_alpha_base * opacity > 0.001) {
                        result = alpha_blend(result, vec4<f32>(0.0, 0.0, 0.0, sf * shadow_alpha_base * opacity));
                    }
                }
                let fill = arrow_mask(local, s);
                if (fill > 0.001) {
                    result = alpha_blend(result, vec4<f32>(active_color, fill * opacity));
                }
                let edge_d = arrow_edge_dist(local, s);
                let stroke_m = 1.0 - smoothstep(0.0, 1.5 * s, edge_d);
                let dash_period = 4.0 * s;
                let dash_on = step(0.5, fract((local.x + local.y) / dash_period));
                let stroke_a = stroke_m * dash_on * (1.0 - fill * 0.5);
                if (stroke_a * opacity > 0.001) {
                    result = alpha_blend(result, vec4<f32>(0.0, 0.0, 0.0, stroke_a * opacity));
                }
            }

            // clickEffect=none means no click visual at all
        }
    }

    // 5. Webcam overlay
    if (u.webcam_enabled > 0.5) {
        let webcam_pixel_size = u.webcam_size * u.output_width;
        let webcam_cx = u.webcam_pos_x * u.output_width;
        let webcam_cy = u.webcam_pos_y * u.output_height;
        let webcam_left = webcam_cx - webcam_pixel_size / 2.0;
        let webcam_top = webcam_cy - webcam_pixel_size / 2.0;
        let wx = f32(x) - webcam_left;
        let wy = f32(y) - webcam_top;
        if (wx >= 0.0 && wx < webcam_pixel_size && wy >= 0.0 && wy < webcam_pixel_size) {
            let webcam_uv = vec2<f32>(wx / webcam_pixel_size, wy / webcam_pixel_size);
            var webcam_color = textureSampleLevel(webcam_tex, bilinear_sampler, webcam_uv, 0.0);
            if (u.webcam_shape < 0.5) {
                let dist = length(webcam_uv - vec2<f32>(0.5));
                webcam_color.a = webcam_color.a * (1.0 - smoothstep(0.49, 0.5, dist));
            } else {
                webcam_color.a = webcam_color.a * is_inside_rounded_rect(
                    vec2<f32>(wx, wy),
                    vec2<f32>(webcam_pixel_size, webcam_pixel_size),
                    webcam_pixel_size * 0.15
                );
            }
            if (webcam_color.a > 0.001) {
                result = alpha_blend(result, webcam_color);
            }
        }
    }

    // 6. Device frame overlay
    if (u.device_frame_enabled > 0.5) {
        let bezel = u.device_frame_bezel;
        let df_radius = u.device_frame_corner_radius;
        let in_bezel_x = f32(x) < bezel || f32(x) >= u.output_width - bezel;
        let in_bezel_y = f32(y) < bezel || f32(y) >= u.output_height - bezel;
        if (in_bezel_x || in_bezel_y) {
            result = vec4<f32>(u.device_frame_r, u.device_frame_g, u.device_frame_b, 1.0);
        }
        if (df_radius > 0.0) {
            let size = vec2<f32>(u.output_width, u.output_height);
            result.a = result.a * is_inside_rounded_rect(vec2<f32>(f32(x), f32(y)), size, df_radius);
        }
    }

    textureStore(output_tex, pixel, result);
}
"#;

impl GpuCompositor {
    /// Create a new GPU compositor with all pipelines and textures initialized.
    ///
    /// Cursor is now SDF-rendered, so `cursor_shape` is only used as a placeholder
    /// to keep the bind group layout stable. All cursor rendering is procedural in the shader.
    pub fn new(
        config: GpuCompositorConfig,
        cursor_shape: Option<&RgbaImage>,
        cursor_size: f32,
        ripple_color: [f32; 3],
        cursor_config: Option<&crate::video_processing::CursorSettings>,
    ) -> Result<Self> {
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
            backends: wgpu::Backends::all(),
            ..Default::default()
        });

        let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::HighPerformance,
            compatible_surface: None,
            force_fallback_adapter: false,
        }))
        .ok_or_else(|| anyhow!("Failed to find a suitable GPU adapter"))?;

        println!("[GPU] Using adapter: {:?}", adapter.get_info().name);

        let (device, queue) = pollster::block_on(adapter.request_device(
            &wgpu::DeviceDescriptor {
                label: Some("Export Compositor"),
                required_features: wgpu::Features::empty(),
                required_limits: wgpu::Limits::default(),
            },
            None,
        ))
        .map_err(|e| anyhow!("Failed to create GPU device: {}", e))?;

        let out_w = config.output_width;
        let out_h = config.output_height;
        let in_w = config.input_width;
        let in_h = config.input_height;
        let unpadded_bytes_per_row = out_w * 4;
        let align = wgpu::COPY_BYTES_PER_ROW_ALIGNMENT;
        let padded_bytes_per_row = (unpadded_bytes_per_row + align - 1) / align * align;
        let frame_size = (padded_bytes_per_row * out_h) as u64;

        // Source texture (video frame input — sized to input, not output)
        let source_texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("Source Video"),
            size: wgpu::Extent3d { width: in_w, height: in_h, depth_or_array_layers: 1 },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8Unorm,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });

        // Output texture (composited frame)
        let output_texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("Composited Output"),
            size: wgpu::Extent3d { width: out_w, height: out_h, depth_or_array_layers: 1 },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8Unorm,
            usage: wgpu::TextureUsages::STORAGE_BINDING | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        });

        // Staging buffers
        let upload_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Upload Staging"),
            size: frame_size,
            usage: wgpu::BufferUsages::COPY_SRC | wgpu::BufferUsages::MAP_WRITE,
            mapped_at_creation: false,
        });

        let download_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Download Staging"),
            size: frame_size,
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });

        // Uniform buffer
        let uniform_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Uniforms"),
            size: std::mem::size_of::<CompositeUniforms>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        // Trail buffer (30 trail points × 4 floats × 4 bytes = 480 bytes)
        let trail_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Trail Points"),
            size: (30 * std::mem::size_of::<TrailPointGpu>()) as u64,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        // Create cursor shape texture (1x1 placeholder — SDF rendering, no texture needed)
        let (cursor_texture, cursor_has_shape) = if let Some(shape) = cursor_shape {
            let cw = shape.width();
            let ch = shape.height();
            let tex = device.create_texture(&wgpu::TextureDescriptor {
                label: Some("Cursor Shape"),
                size: wgpu::Extent3d { width: cw, height: ch, depth_or_array_layers: 1 },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: wgpu::TextureFormat::Rgba8Unorm,
                usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
                view_formats: &[],
            });
            queue.write_texture(
                wgpu::ImageCopyTexture {
                    texture: &tex,
                    mip_level: 0,
                    origin: wgpu::Origin3d::ZERO,
                    aspect: wgpu::TextureAspect::All,
                },
                shape.as_raw(),
                wgpu::ImageDataLayout {
                    offset: 0,
                    bytes_per_row: Some(cw * 4),
                    rows_per_image: Some(ch),
                },
                wgpu::Extent3d { width: cw, height: ch, depth_or_array_layers: 1 },
            );
            println!("[GPU] Cursor shape texture: {}x{} ({} bytes)", cw, ch, shape.as_raw().len());
            (tex, true)
        } else {
            // 1x1 placeholder
            let tex = device.create_texture(&wgpu::TextureDescriptor {
                label: Some("Cursor Placeholder"),
                size: wgpu::Extent3d { width: 1, height: 1, depth_or_array_layers: 1 },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: wgpu::TextureFormat::Rgba8Unorm,
                usage: wgpu::TextureUsages::TEXTURE_BINDING,
                view_formats: &[],
            });
            (tex, false)
        };

        // Pre-bake corner mask texture
        let corner_mask_texture = Self::create_corner_mask_texture(
            &device, &queue,
            config.content_width, config.content_height,
            config.corner_radius,
        );

        // Pre-bake shadow texture
        let shadow_texture = Self::create_shadow_texture(
            &device, &queue, &config,
        );

        // Placeholder webcam texture (1x1 transparent)
        let webcam_texture = None;

        // Create shader module
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Composite Shader"),
            source: wgpu::ShaderSource::Wgsl(COMPOSITE_SHADER.into()),
        });

        // Bind group layout
        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("Composite BGL"),
            entries: &[
                // 0: Uniforms
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                // 1: Source texture
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                // 2: Output texture (storage)
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::StorageTexture {
                        access: wgpu::StorageTextureAccess::WriteOnly,
                        format: wgpu::TextureFormat::Rgba8Unorm,
                        view_dimension: wgpu::TextureViewDimension::D2,
                    },
                    count: None,
                },
                // 3: Corner mask texture
                wgpu::BindGroupLayoutEntry {
                    binding: 3,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                // 4: Shadow texture
                wgpu::BindGroupLayoutEntry {
                    binding: 4,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                // 5: Cursor texture
                wgpu::BindGroupLayoutEntry {
                    binding: 5,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                // 6: Webcam texture
                wgpu::BindGroupLayoutEntry {
                    binding: 6,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                // 7: Bilinear sampler
                wgpu::BindGroupLayoutEntry {
                    binding: 7,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
                // 8: Trail points (storage buffer)
                wgpu::BindGroupLayoutEntry {
                    binding: 8,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Storage { read_only: true },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
            ],
        });

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Composite Pipeline Layout"),
            bind_group_layouts: &[&bind_group_layout],
            push_constant_ranges: &[],
        });

        let composite_pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: Some("Composite Pipeline"),
            layout: Some(&pipeline_layout),
            module: &shader,
            entry_point: "main",
        });

        // Create cached sampler and placeholder texture (reused every frame)
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("Bilinear Sampler"),
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });

        let placeholder_texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("Placeholder"),
            size: wgpu::Extent3d { width: 1, height: 1, depth_or_array_layers: 1 },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8Unorm,
            usage: wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        });

        // Extract cursor visual config for SDF rendering
        let cc = cursor_config;
        let cursor_style_str = cc.and_then(|c| c.style.as_deref()).unwrap_or("pointer");
        let cursor_style_val = match cursor_style_str {
            "pointer" => 0.0,
            "circle" => 1.0,
            "filled" => 2.0,
            "outline" => 3.0,
            "dotted" => 4.0,
            _ => 0.0,
        };
        let always_pointer = cc.and_then(|c| c.always_use_pointer).unwrap_or(false);
        let cursor_style_final = if always_pointer { 0.0 } else { cursor_style_val };

        let color_hex = cc.and_then(|c| c.color.as_deref()).unwrap_or("#ffffff");
        let (cr, cg, cb) = crate::cursor_renderer::parse_hex_rgb(color_hex);
        let cursor_color = [cr as f32 / 255.0, cg as f32 / 255.0, cb as f32 / 255.0];

        let hl_hex = cc.and_then(|c| c.highlight_color.as_deref()).unwrap_or("#ff6b6b");
        let (hr, hg, hb) = crate::cursor_renderer::parse_hex_rgb(hl_hex);
        let cursor_highlight_color = [hr as f32 / 255.0, hg as f32 / 255.0, hb as f32 / 255.0];

        let cursor_shadow_intensity = cc.and_then(|c| c.shadow_intensity).unwrap_or(30.0) as f32;

        let click_effect_str = cc.and_then(|c| c.click_effect.as_deref()).unwrap_or("ripple");
        let click_effect_val = match click_effect_str {
            "none" => 0.0,
            "circle" => 1.0,
            "ripple" => 2.0,
            _ => 2.0,
        };

        println!("[GPU] Compositor initialized: {}x{}, cursor={}, corner_radius={}, shadow={}, cursor_style={}",
            out_w, out_h, cursor_has_shape, config.corner_radius, config.shadow_enabled, cursor_style_str);

        Ok(Self {
            device,
            queue,
            composite_pipeline,
            bind_group_layout,
            source_texture,
            output_texture,
            upload_buffer,
            download_buffer,
            uniform_buffer,
            trail_buffer,
            cursor_texture,
            cursor_enabled: cursor_has_shape,
            cursor_size,
            ripple_color,
            cursor_style: cursor_style_final,
            cursor_color,
            cursor_highlight_color,
            cursor_shadow_intensity,
            click_effect: click_effect_val,
            webcam_texture,
            corner_mask_texture,
            shadow_texture,
            sampler,
            placeholder_texture,
            bind_group: None,
            bind_group_dirty: true,
            padded_bytes_per_row,
            config,
            prev_zoom_state: None,
        })
    }

    /// Set webcam video frames source. Call once after decoding first webcam frame.
    #[allow(dead_code)]
    pub fn set_webcam_texture(&mut self, width: u32, height: u32) {
        self.webcam_texture = Some(self.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("Webcam"),
            size: wgpu::Extent3d { width, height, depth_or_array_layers: 1 },
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
    fn ensure_bind_group(&mut self) {
        if !self.bind_group_dirty && self.bind_group.is_some() {
            return;
        }

        let source_view = self.source_texture.create_view(&wgpu::TextureViewDescriptor::default());
        let output_view = self.output_texture.create_view(&wgpu::TextureViewDescriptor::default());
        let corner_mask_view = self.corner_mask_texture.create_view(&wgpu::TextureViewDescriptor::default());
        let shadow_view = self.shadow_texture.create_view(&wgpu::TextureViewDescriptor::default());
        let placeholder_view = self.placeholder_texture.create_view(&wgpu::TextureViewDescriptor::default());

        let cursor_view = self.cursor_texture.create_view(&wgpu::TextureViewDescriptor::default());

        let webcam_view = self.webcam_texture.as_ref()
            .map(|t| t.create_view(&wgpu::TextureViewDescriptor::default()))
            .unwrap_or_else(|| placeholder_view);

        self.bind_group = Some(self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Composite BG"),
            layout: &self.bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry { binding: 0, resource: self.uniform_buffer.as_entire_binding() },
                wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::TextureView(&source_view) },
                wgpu::BindGroupEntry { binding: 2, resource: wgpu::BindingResource::TextureView(&output_view) },
                wgpu::BindGroupEntry { binding: 3, resource: wgpu::BindingResource::TextureView(&corner_mask_view) },
                wgpu::BindGroupEntry { binding: 4, resource: wgpu::BindingResource::TextureView(&shadow_view) },
                wgpu::BindGroupEntry { binding: 5, resource: wgpu::BindingResource::TextureView(&cursor_view) },
                wgpu::BindGroupEntry { binding: 6, resource: wgpu::BindingResource::TextureView(&webcam_view) },
                wgpu::BindGroupEntry { binding: 7, resource: wgpu::BindingResource::Sampler(&self.sampler) },
                wgpu::BindGroupEntry { binding: 8, resource: self.trail_buffer.as_entire_binding() },
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
                wgpu::Extent3d { width, height, depth_or_array_layers: 1 },
            );
        }
    }

    /// Composite a single frame on the GPU.
    ///
    /// Takes raw RGBA source frame, applies zoom, motion blur, rounded corners,
    /// shadow, cursor, webcam, device frame — returns composited RGBA output.
    ///
    /// Bind group and sampler are cached across frames to avoid per-frame allocation.
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
                source_frame.len(), input_frame_size, in_w, in_h
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
            shadow_enabled: if self.config.shadow_enabled { 1.0 } else { 0.0 },
            shadow_blur: self.config.shadow_blur,
            shadow_intensity: self.config.shadow_intensity,
            shadow_offset_x: self.config.shadow_offset_x,
            shadow_offset_y: self.config.shadow_offset_y,
            motion_blur_enabled: if self.config.motion_blur_enabled { 1.0 } else { 0.0 },
            motion_blur_pan_intensity: self.config.motion_blur_pan_intensity,
            motion_blur_zoom_intensity: self.config.motion_blur_zoom_intensity,
            velocity_x: vel_x,
            velocity_y: vel_y,
            velocity_scale: vel_scale,
            webcam_enabled: if webcam_cfg.is_some() && self.webcam_texture.is_some() { 1.0 } else { 0.0 },
            webcam_pos_x: webcam_cfg.as_ref().map_or(0.0, |w| w.pos_x),
            webcam_pos_y: webcam_cfg.as_ref().map_or(0.0, |w| w.pos_y),
            webcam_size: webcam_cfg.as_ref().map_or(0.0, |w| w.size),
            webcam_shape: webcam_cfg.as_ref().map_or(0.0, |w| if w.shape == "circle" { 0.0 } else { 1.0 }),
            device_frame_enabled: if df_cfg.is_some() { 1.0 } else { 0.0 },
            device_frame_bezel: df_cfg.as_ref().map_or(0.0, |d| d.bezel as f32),
            device_frame_corner_radius: df_cfg.as_ref().map_or(0.0, |d| d.corner_radius as f32),
            device_frame_r: df_cfg.as_ref().map_or(0.0, |d| d.color[0]),
            device_frame_g: df_cfg.as_ref().map_or(0.0, |d| d.color[1]),
            device_frame_b: df_cfg.as_ref().map_or(0.0, |d| d.color[2]),
            cursor_enabled: if self.cursor_enabled && cursor_state.is_some() { 1.0 } else { 0.0 },
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
            trail_enabled: if cursor_state.map_or(false, |c| c.trail_count > 0.0) { 1.0 } else { 0.0 },
            trail_count: cursor_state.map_or(0.0, |c| c.trail_count),
            trail_opacity: cursor_state.map_or(0.5, |_| {
                // trail_opacity is baked into trail_points alpha, just pass 1.0 as multiplier
                1.0
            }),
        };

        // Upload trail points
        if let Some(cs) = cursor_state {
            let trail_gpu: Vec<TrailPointGpu> = cs.trail_points.iter().map(|tp| TrailPointGpu {
                x: tp.x,
                y: tp.y,
                alpha: tp.alpha,
                size: tp.size,
            }).collect();
            self.queue.write_buffer(&self.trail_buffer, 0, bytemuck::cast_slice(&trail_gpu));
        }

        // Upload uniforms + source frame (the only per-frame GPU writes)
        self.queue.write_buffer(&self.uniform_buffer, 0, bytemuck::bytes_of(&uniforms));
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
            wgpu::Extent3d { width: in_w, height: in_h, depth_or_array_layers: 1 },
        );

        // Ensure bind group is built (cached, only rebuilt when textures change)
        self.ensure_bind_group();
        let bind_group = self.bind_group.as_ref().unwrap();

        // Encode and dispatch
        let mut encoder = self.device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("Composite Encoder"),
        });

        {
            let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: Some("Composite Pass"),
                timestamp_writes: None,
            });
            pass.set_pipeline(&self.composite_pipeline);
            pass.set_bind_group(0, bind_group, &[]);
            pass.dispatch_workgroups(
                (out_w + 15) / 16,
                (out_h + 15) / 16,
                1,
            );
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
            wgpu::Extent3d { width: out_w, height: out_h, depth_or_array_layers: 1 },
        );

        self.queue.submit(std::iter::once(encoder.finish()));

        // Read back the result
        let buffer_slice = self.download_buffer.slice(..);
        let (sender, receiver) = std::sync::mpsc::channel();
        buffer_slice.map_async(wgpu::MapMode::Read, move |result| {
            let _ = sender.send(result);
        });
        self.device.poll(wgpu::Maintain::Wait);
        receiver.recv()
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

    /// Pre-bake a rounded corner alpha mask texture.
    fn create_corner_mask_texture(
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
                    let alpha = Self::rounded_corner_alpha(x as f32, y as f32, width as f32, height as f32, r);
                    let a = (alpha * 255.0).clamp(0.0, 255.0) as u8;
                    data[idx] = 255;     // R
                    data[idx + 1] = 255; // G
                    data[idx + 2] = 255; // B
                    data[idx + 3] = a;   // A
                }
            }
        }

        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("Corner Mask"),
            size: wgpu::Extent3d { width, height, depth_or_array_layers: 1 },
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
            wgpu::Extent3d { width, height, depth_or_array_layers: 1 },
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
    fn create_shadow_texture(
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
                            lx as f32, ly as f32,
                            content_w as f32, content_h as f32,
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
                    data[idx] = 0;   // R
                    data[idx + 1] = 0; // G
                    data[idx + 2] = 0; // B
                    data[idx + 3] = (alpha * 255.0).clamp(0.0, 255.0) as u8;
                }
            }
        }

        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("Shadow"),
            size: wgpu::Extent3d { width: out_w, height: out_h, depth_or_array_layers: 1 },
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
            wgpu::Extent3d { width: out_w, height: out_h, depth_or_array_layers: 1 },
        );

        texture
    }
}

/// Build a GpuCompositorConfig from ExportSettings (mirrors what visual_effects.rs used to compute).
pub fn build_gpu_config(settings: &super::types::ExportSettings, source_width: Option<u32>, source_height: Option<u32>) -> GpuCompositorConfig {
    let (out_w, out_h) = super::visual_effects::get_output_dimensions(settings);

    let visual = settings.visual_settings.as_ref();

    let padding_pct = visual.and_then(|v| v.padding).unwrap_or(0.0) / 100.0;
    let padding_x = (out_w as f64 * padding_pct) as u32;
    let padding_y = (out_h as f64 * padding_pct) as u32;
    let content_w = out_w - 2 * padding_x;
    let content_h = out_h - 2 * padding_y;

    let bg_hex = visual.and_then(|v| v.background_color.as_deref()).unwrap_or("#282a36");
    let background_color = parse_hex_color(bg_hex);

    let corner_radius_pct = visual.and_then(|v| v.corner_radius).unwrap_or(0.0);
    let min_dim = content_w.min(content_h) as f32;
    let corner_radius = if corner_radius_pct > 0.0 {
        (min_dim * corner_radius_pct as f32 / 100.0 * 0.5).max(1.0)
    } else {
        0.0
    };

    let shadow_enabled = visual.and_then(|v| v.shadow_enabled).unwrap_or(false);
    let shadow_blur = visual.and_then(|v| v.shadow_blur).unwrap_or(20.0) as f32;
    let shadow_intensity = visual.and_then(|v| v.shadow_intensity).unwrap_or(0.5) as f32;
    let shadow_offset_x = visual.and_then(|v| v.shadow_offset_x).unwrap_or(0.0) as f32;
    let shadow_offset_y = visual.and_then(|v| v.shadow_offset_y).unwrap_or(10.0) as f32;

    let motion_blur_enabled = visual.and_then(|v| v.motion_blur_enabled).unwrap_or(false);
    let motion_blur_pan_intensity = visual.and_then(|v| v.motion_blur_pan_intensity).unwrap_or(0.2) as f32;
    let motion_blur_zoom_intensity = visual.and_then(|v| v.motion_blur_zoom_intensity).unwrap_or(0.0) as f32;

    // Webcam config
    let webcam = settings.visual_settings.as_ref().and_then(|_| {
        // Webcam info is passed separately — this is just the config shape.
        // Actual webcam data is set via set_webcam_texture + upload_webcam_frame.
        None::<WebcamConfig>
    });

    // Device frame config
    let device_frame = visual
        .and_then(|v| v.device_frame.as_deref())
        .and_then(|df| {
            if df == "none" { return None; }
            let frame_color = visual.and_then(|v| v.device_frame_color.as_deref()).unwrap_or("black");
            let color = match frame_color {
                "silver" => parse_hex_color("#c4c4c4"),
                "gold" => parse_hex_color("#d4af37"),
                "blue" => parse_hex_color("#2563eb"),
                _ => parse_hex_color("#1a1a1a"),
            };
            let (bezel, corner_radius) = match df {
                "iphone-15-pro" | "iphone-15" => (20, 55),
                "ipad-pro" => (30, 25),
                "macbook-pro" => (15, 10),
                "browser" => (40, 10),
                _ => (20, 20),
            };
            Some(DeviceFrameConfig { bezel, corner_radius, color })
        });

    // Compute input dimensions (aspect-fit source within content area)
    let (input_w, input_h, input_off_x, input_off_y) = if let (Some(sw), Some(sh)) = (source_width, source_height) {
        if sw > 0 && sh > 0 {
            let src_aspect = sw as f64 / sh as f64;
            let cnt_aspect = content_w as f64 / content_h as f64;
            let (mut iw, mut ih) = if (src_aspect - cnt_aspect).abs() < 0.01 {
                // Same aspect ratio — input fills content
                (content_w, content_h)
            } else if src_aspect > cnt_aspect {
                // Source wider — fit to content width, letterbox vertically
                let h = (content_w as f64 / src_aspect).round() as u32;
                (content_w, h)
            } else {
                // Source taller — fit to content height, pillarbox horizontally
                let w = (content_h as f64 * src_aspect).round() as u32;
                (w, content_h)
            };
            // Ensure even dims
            iw = iw - (iw % 2);
            ih = ih - (ih % 2);
            // Clamp to content
            iw = iw.min(content_w);
            ih = ih.min(content_h);
            let ox = (content_w - iw) / 2;
            let oy = (content_h - ih) / 2;
            (iw, ih, ox, oy)
        } else {
            (content_w, content_h, 0, 0)
        }
    } else {
        // No source dims — fallback: input == content
        (content_w, content_h, 0, 0)
    };

    println!("[GPU Config] output={}x{}, content={}x{}, input={}x{} offset=({},{})",
        out_w, out_h, content_w, content_h, input_w, input_h, input_off_x, input_off_y);

    GpuCompositorConfig {
        output_width: out_w,
        output_height: out_h,
        content_width: content_w,
        content_height: content_h,
        content_offset_x: padding_x,
        content_offset_y: padding_y,
        input_width: input_w,
        input_height: input_h,
        input_offset_x: input_off_x,
        input_offset_y: input_off_y,
        background_color,
        corner_radius,
        shadow_enabled,
        shadow_blur,
        shadow_intensity,
        shadow_offset_x,
        shadow_offset_y,
        webcam: webcam,
        device_frame,
        motion_blur_enabled,
        motion_blur_pan_intensity,
        motion_blur_zoom_intensity,
    }
}

/// Build a GpuCompositorConfig with webcam info populated.
pub fn build_gpu_config_with_webcam(
    settings: &super::types::ExportSettings,
    webcam_info: &Option<(std::path::PathBuf, f64, f64, f64, String)>,
    source_width: Option<u32>,
    source_height: Option<u32>,
) -> GpuCompositorConfig {
    let mut config = build_gpu_config(settings, source_width, source_height);
    if let Some((_, pos_x, pos_y, size, shape)) = webcam_info {
        config.webcam = Some(WebcamConfig {
            pos_x: *pos_x as f32,
            pos_y: *pos_y as f32,
            size: *size as f32,
            shape: shape.clone(),
        });
    }
    config
}

fn parse_hex_color(hex: &str) -> [f32; 4] {
    let hex = hex.trim_start_matches('#');
    if hex.len() >= 6 {
        let r = u8::from_str_radix(&hex[0..2], 16).unwrap_or(0) as f32 / 255.0;
        let g = u8::from_str_radix(&hex[2..4], 16).unwrap_or(0) as f32 / 255.0;
        let b = u8::from_str_radix(&hex[4..6], 16).unwrap_or(0) as f32 / 255.0;
        [r, g, b, 1.0]
    } else {
        [0.157, 0.165, 0.212, 1.0] // default #282a36
    }
}
