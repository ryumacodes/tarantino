// Tarantino GPU Compositor Shader
// Pipeline: Background → Display → Camera PIP → Cursor → Effects

struct CameraUniforms {
    position: vec2<f32>,
    size: vec2<f32>,
    roundness: f32,
    shadow_blur: f32,
    shadow_offset: vec2<f32>,
    shadow_opacity: f32,
}

struct ZoomUniforms {
    focus_point: vec2<f32>,
    zoom_factor: f32,
    progress: f32,
}

struct BackgroundUniforms {
    blur_radius: f32,
    background_color: vec4<f32>,
}

struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) uv: vec2<f32>,
}

// Bindings
@group(0) @binding(0) var display_texture: texture_2d<f32>;
@group(0) @binding(1) var display_sampler: sampler;
@group(0) @binding(2) var camera_texture: texture_2d<f32>;
@group(0) @binding(3) var camera_sampler: sampler;
@group(0) @binding(4) var<uniform> camera_uniforms: CameraUniforms;
@group(0) @binding(5) var<uniform> zoom_uniforms: ZoomUniforms;
@group(0) @binding(6) var<uniform> background_uniforms: BackgroundUniforms;

// Vertex shader - generates full-screen quad
@vertex
fn vs_main(@builtin(vertex_index) vertex_index: u32) -> VertexOutput {
    var positions = array<vec2<f32>, 4>(
        vec2<f32>(-1.0, -1.0), // Bottom-left
        vec2<f32>( 1.0, -1.0), // Bottom-right
        vec2<f32>(-1.0,  1.0), // Top-left
        vec2<f32>( 1.0,  1.0), // Top-right
    );
    
    var uvs = array<vec2<f32>, 4>(
        vec2<f32>(0.0, 1.0), // Bottom-left
        vec2<f32>(1.0, 1.0), // Bottom-right
        vec2<f32>(0.0, 0.0), // Top-left
        vec2<f32>(1.0, 0.0), // Top-right
    );
    
    var out: VertexOutput;
    out.position = vec4<f32>(positions[vertex_index], 0.0, 1.0);
    out.uv = uvs[vertex_index];
    return out;
}

// Fragment shader - composites all layers
@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    let uv = in.uv;
    
    // Apply zoom transformation if active
    var sample_uv = uv;
    if (zoom_uniforms.zoom_factor > 1.0) {
        sample_uv = apply_zoom_transform(uv, zoom_uniforms.focus_point, zoom_uniforms.zoom_factor, zoom_uniforms.progress);
    }
    
    // Sample display texture (main video)
    var display_color = textureSample(display_texture, display_sampler, sample_uv);
    
    // Apply background if display is transparent or out of bounds
    if (is_out_of_bounds(sample_uv) || display_color.a < 0.99) {
        let bg_color = background_uniforms.background_color;
        display_color = mix(bg_color, display_color, display_color.a);
    }
    
    // Apply background blur if enabled
    if (background_uniforms.blur_radius > 0.0) {
        display_color = apply_background_blur(display_color, sample_uv, background_uniforms.blur_radius);
    }
    
    var final_color = display_color;
    
    // Composite camera PIP if enabled
    if (camera_uniforms.size.x > 0.0 && camera_uniforms.size.y > 0.0) {
        final_color = composite_camera_pip(final_color, uv);
    }
    
    // TODO: Composite cursor (will be added in Sprint 3)
    
    return final_color;
}

// Apply zoom transformation with easing
fn apply_zoom_transform(uv: vec2<f32>, focus: vec2<f32>, zoom_factor: f32, progress: f32) -> vec2<f32> {
    // Ease-in-out curve for smooth zoom animation
    let eased_progress = ease_in_out_cubic(progress);
    let current_zoom = mix(1.0, zoom_factor, eased_progress);
    
    // Transform UV coordinates to zoom around focus point
    let centered_uv = (uv - focus) / current_zoom + focus;
    return centered_uv;
}

// Ease-in-out cubic function for smooth animations
fn ease_in_out_cubic(t: f32) -> f32 {
    let clamped_t = clamp(t, 0.0, 1.0);
    if (clamped_t < 0.5) {
        return 4.0 * clamped_t * clamped_t * clamped_t;
    } else {
        let shifted = clamped_t - 1.0;
        return 1.0 + 4.0 * shifted * shifted * shifted;
    }
}

// Check if UV coordinates are out of texture bounds
fn is_out_of_bounds(uv: vec2<f32>) -> bool {
    return uv.x < 0.0 || uv.x > 1.0 || uv.y < 0.0 || uv.y > 1.0;
}

// Apply background blur (simple box blur for now)
fn apply_background_blur(base_color: vec4<f32>, uv: vec2<f32>, blur_radius: f32) -> vec4<f32> {
    if (blur_radius <= 0.0) {
        return base_color;
    }
    
    // Simple 9-tap box blur
    let texel_size = 1.0 / vec2<f32>(textureDimensions(display_texture));
    let blur_offset = blur_radius * texel_size;
    
    var blurred_color = vec4<f32>(0.0);
    var weight_sum = 0.0;
    
    for (var x = -1; x <= 1; x++) {
        for (var y = -1; y <= 1; y++) {
            let sample_uv = uv + vec2<f32>(f32(x), f32(y)) * blur_offset;
            let sample_color = textureSample(display_texture, display_sampler, sample_uv);
            let weight = 1.0; // Uniform weight for box blur
            
            blurred_color += sample_color * weight;
            weight_sum += weight;
        }
    }
    
    return blurred_color / weight_sum;
}

// Composite camera PIP with rounded corners and shadow
fn composite_camera_pip(background: vec4<f32>, uv: vec2<f32>) -> vec4<f32> {
    let pip_pos = camera_uniforms.position;
    let pip_size = camera_uniforms.size;
    let pip_bounds = vec4<f32>(pip_pos.x, pip_pos.y, pip_pos.x + pip_size.x, pip_pos.y + pip_size.y);
    
    // Check if we're inside the PIP bounds
    if (uv.x >= pip_bounds.x && uv.x <= pip_bounds.z && uv.y >= pip_bounds.y && uv.y <= pip_bounds.w) {
        // Calculate local UV within the PIP
        let local_uv = (uv - pip_pos) / pip_size;
        
        // Apply rounded corner mask
        let corner_mask = rounded_rect_mask(local_uv, camera_uniforms.roundness);
        
        if (corner_mask > 0.0) {
            // Sample camera texture
            let camera_color = textureSample(camera_texture, camera_sampler, local_uv);
            
            // Apply shadow if enabled
            var shadow_contribution = 0.0;
            if (camera_uniforms.shadow_opacity > 0.0) {
                shadow_contribution = calculate_shadow(uv, pip_bounds);
            }
            
            // Blend camera over background with corner mask and shadow
            let alpha = camera_color.a * corner_mask;
            let shadowed_bg = mix(background, vec4<f32>(0.0, 0.0, 0.0, 1.0), shadow_contribution);
            return mix(shadowed_bg, camera_color, alpha);
        }
    }
    
    // Check if we need to draw shadow outside PIP bounds
    if (camera_uniforms.shadow_opacity > 0.0) {
        let shadow_contribution = calculate_shadow(uv, pip_bounds);
        if (shadow_contribution > 0.0) {
            return mix(background, vec4<f32>(0.0, 0.0, 0.0, 1.0), shadow_contribution);
        }
    }
    
    return background;
}

// Calculate rounded rectangle mask
fn rounded_rect_mask(uv: vec2<f32>, roundness: f32) -> f32 {
    if (roundness <= 0.0) {
        return 1.0;
    }
    
    // Distance from edges
    let edge_dist = min(min(uv.x, 1.0 - uv.x), min(uv.y, 1.0 - uv.y));
    let corner_radius = roundness * 0.5;
    
    // Calculate distance to nearest corner in corner regions
    let corner_uv = abs(uv - 0.5) - (0.5 - corner_radius);
    let corner_dist = length(max(corner_uv, 0.0)) - corner_radius;
    
    // Smooth edge for anti-aliasing
    return 1.0 - smoothstep(-0.001, 0.001, corner_dist);
}

// Calculate shadow contribution for PIP
fn calculate_shadow(uv: vec2<f32>, pip_bounds: vec4<f32>) -> f32 {
    let shadow_offset = camera_uniforms.shadow_offset;
    let shadow_blur = camera_uniforms.shadow_blur;
    let shadow_opacity = camera_uniforms.shadow_opacity;
    
    if (shadow_opacity <= 0.0 || shadow_blur <= 0.0) {
        return 0.0;
    }
    
    // Offset bounds for shadow
    let shadow_bounds = vec4<f32>(
        pip_bounds.x + shadow_offset.x,
        pip_bounds.y + shadow_offset.y,
        pip_bounds.z + shadow_offset.x,
        pip_bounds.w + shadow_offset.y
    );
    
    // Calculate distance to shadow bounds
    let shadow_center = (shadow_bounds.xy + shadow_bounds.zw) * 0.5;
    let shadow_size = shadow_bounds.zw - shadow_bounds.xy;
    let dist_to_shadow = abs(uv - shadow_center) - shadow_size * 0.5;
    let shadow_dist = length(max(dist_to_shadow, 0.0));
    
    // Gaussian-like falloff for shadow
    let shadow_strength = exp(-shadow_dist / shadow_blur);
    return shadow_strength * shadow_opacity;
}