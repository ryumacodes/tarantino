@group(0) @binding(12) var cursor_motion_sampler:sampler;
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
    bg_r: f32, bg_g: f32, bg_b: f32, bg_a: f32,
    bg_type: f32, bg_gradient_direction: f32, bg_gradient_count: f32, bg_pos_0: f32,
    bg_pos_1: f32, bg_pos_2: f32, bg_pos_3: f32, bg_color_0_r: f32,
    bg_color_0_g: f32, bg_color_0_b: f32, bg_color_0_a: f32, bg_color_1_r: f32,
    bg_color_1_g: f32, bg_color_1_b: f32, bg_color_1_a: f32, bg_color_2_r: f32,
    bg_color_2_g: f32, bg_color_2_b: f32, bg_color_2_a: f32, bg_color_3_r: f32,
    bg_color_3_g: f32, bg_color_3_b: f32, bg_color_3_a: f32,
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
    webcam_opacity: f32,
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
    window_mode: f32,
    clip_cursor_to_video: f32,
    layer_outputs: f32,
    shutter_start_x:f32, shutter_start_y:f32, shutter_start_w:f32, shutter_start_h:f32,
    shutter_now_x:f32, shutter_now_y:f32, shutter_now_w:f32, shutter_now_h:f32,
    cursor_start_x:f32,cursor_start_y:f32,cursor_start_size:f32,cursor_start_angle:f32,
    cursor_now_x:f32,cursor_now_y:f32,cursor_now_size:f32,cursor_now_angle:f32,
    lens_camera_opacity:f32, webcam_mirrored:f32, cursor_fade_blur:f32, lens_camera_pad3:f32,
    camera_left:f32, camera_top:f32, camera_width:f32, camera_height:f32,
    cursor_image_left:f32, cursor_image_top:f32, cursor_image_width:f32, cursor_image_height:f32,
    source_left:f32, source_top:f32, source_width:f32, source_height:f32,
    mask_left:f32, mask_top:f32, mask_width:f32, mask_height:f32,
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
@group(0) @binding(9) var background_tex: texture_2d<f32>;
@group(0) @binding(10) var foreground_layer: texture_storage_2d<rgba8unorm, write>;
@group(0) @binding(11) var background_layer: texture_storage_2d<rgba8unorm, write>;

fn sample_window_mask(uv: vec2<f32>) -> f32 {
    let offset=vec2<f32>(u.mask_left,u.mask_top);
    let extent=vec2<f32>(u.mask_width,u.mask_height);
    return textureSampleLevel(corner_mask_tex,bilinear_sampler,offset+uv*extent,0.0).r;
}

fn source_view() -> vec4<f32> {
    return vec4<f32>(u.source_left,u.source_top,u.source_width,u.source_height);
}

fn sample_source_texture(uv: vec2<f32>) -> vec4<f32> {
    return textureSampleLevel(source_tex, bilinear_sampler, source_view().xy + uv * source_view().zw, source_sampling_lod());
}

fn sample_source_segment(uv:vec2<f32>,delta:vec2<f32>,count:i32) -> vec4<f32> {
    let view=source_view();
    let size=vec2<f32>(textureDimensions(source_tex))*view.zw;
    let texels=delta*size;
    let along=delta/f32(count);
    let across=vec2<f32>(-texels.y,texels.x)/max(length(texels),0.000001)
        *exp2(source_sampling_lod())/size;
    return textureSampleGrad(source_tex,cursor_motion_sampler,view.xy+uv*view.zw,
        along*view.zw,across*view.zw);
}

fn sample_zoomed(uv: vec2<f32>) -> vec4<f32> {
    let scale = u.zoom_scale;
    if (scale <= 1.001) {
        return sample_source_texture(uv);
    }
    let vis_size = 1.0 / scale;
    let half_vis = vis_size / 2.0;
    let src_x = clamp(u.zoom_center_x - half_vis, 0.0, 1.0 - vis_size);
    let src_y = clamp(u.zoom_center_y - half_vis, 0.0, 1.0 - vis_size);
    let src_uv = vec2<f32>(src_x + uv.x * vis_size, src_y + uv.y * vis_size);
    return sample_source_texture(src_uv);
}

fn bg_color(index: i32) -> vec4<f32> {
    if (index == 1) { return vec4<f32>(u.bg_color_1_r, u.bg_color_1_g, u.bg_color_1_b, u.bg_color_1_a); }
    if (index == 2) { return vec4<f32>(u.bg_color_2_r, u.bg_color_2_g, u.bg_color_2_b, u.bg_color_2_a); }
    if (index == 3) { return vec4<f32>(u.bg_color_3_r, u.bg_color_3_g, u.bg_color_3_b, u.bg_color_3_a); }
    return vec4<f32>(u.bg_color_0_r, u.bg_color_0_g, u.bg_color_0_b, u.bg_color_0_a);
}

fn bg_pos(index: i32) -> f32 {
    if (index == 1) { return u.bg_pos_1; } if (index == 2) { return u.bg_pos_2; } if (index == 3) { return u.bg_pos_3; }
    return u.bg_pos_0;
}

fn gradient_color(t_in: f32) -> vec4<f32> {
    let t = clamp(t_in, 0.0, 1.0);
    let count = i32(clamp(u.bg_gradient_count, 1.0, 4.0));
    var color = bg_color(0);
    for (var i = 1; i < 4; i = i + 1) {
        if (i < count && t >= bg_pos(i)) {
            let p0 = bg_pos(i - 1);
            let p1 = bg_pos(i);
            color = mix(bg_color(i - 1), bg_color(i), clamp((t - p0) / max(p1 - p0, 0.001), 0.0, 1.0));
        }
    }
    return color;
}

fn background_at(px: vec2<f32>) -> vec4<f32> {
    if (u.bg_type < 0.5) { return vec4<f32>(u.bg_r, u.bg_g, u.bg_b, u.bg_a); }
    let uv = clamp(px / vec2<f32>(u.output_width, u.output_height), vec2<f32>(0.0), vec2<f32>(1.0));
    if (u.bg_type > 1.5) {
        let dims = vec2<f32>(textureDimensions(background_tex));
        let image_aspect = dims.x / max(dims.y, 1.0); let output_aspect = u.output_width / max(u.output_height, 1.0);
        var image_uv = uv;
        if (image_aspect > output_aspect) { image_uv.x = 0.5 + (uv.x - 0.5) * (output_aspect / image_aspect); }
        else { image_uv.y = 0.5 + (uv.y - 0.5) * (image_aspect / output_aspect); }
        return textureSampleLevel(background_tex, bilinear_sampler, clamp(image_uv, vec2<f32>(0.0), vec2<f32>(1.0)), 0.0);
    }
    if (u.bg_gradient_direction == 1.0) { return gradient_color(uv.x); } if (u.bg_gradient_direction == 2.0) { return gradient_color(uv.y); }
    if (u.bg_gradient_direction == 3.0) { return gradient_color(distance(uv, vec2<f32>(0.5)) * 1.4142); }
    return gradient_color((uv.x + uv.y) * 0.5);
}

// Sample the source along the camera shutter while retaining the display clip.
fn sample_with_motion_blur(uv: vec2<f32>) -> vec4<f32> {
    if (!window_temporal_active()) { return sample_zoomed(uv); }
    let size=vec2<f32>(u.input_width,u.input_height);
    let pixel=vec2<f32>(u.content_offset_x+u.input_offset_x,u.content_offset_y+u.input_offset_y)+uv*size;
    let before=window_shutter_start();let now=window_shutter_now();
    let a=(pixel-before.xy)/before.zw+vec2<f32>(0.5);
    let b=(pixel-now.xy)/now.zw+vec2<f32>(0.5);
    if (length((b-a)*before.zw)<1.0) {
        return sample_source_texture(clamp((a+b)*0.5,vec2<f32>(0.0),vec2<f32>(1.0)));
    }
    let count=window_fetch_count(b-a);
    var color=vec4<f32>(0.0);
    for (var tap=0;tap<count;tap=tap+1) {
        let pos=clamp(mix(a,b,(f32(tap)+window_sample_phase(pixel))/f32(count)),vec2<f32>(0.0),vec2<f32>(1.0));
        let value=sample_source_segment(pos,b-a,count);
        color=color+vec4<f32>(value.rgb*value.rgb*value.a,value.a)/f32(count);
    }
    if (color.a<=0.0) {return vec4<f32>(0.0);}
    return vec4<f32>(sqrt(max(color.rgb/color.a,vec3<f32>(0.0))),color.a);
}

fn sample_window_motion_blur(uv: vec2<f32>) -> vec4<f32> {
    let base = sample_source_texture(uv);
    if (u.motion_blur_enabled < 0.5) {
        return base;
    }
    let pan_speed = sqrt(u.velocity_x * u.velocity_x + u.velocity_y * u.velocity_y);
    let pan_blur = min(pan_speed * u.motion_blur_pan_intensity * 0.008, 0.012);
    let zoom_blur = min(abs(u.velocity_scale) * u.motion_blur_zoom_intensity * 0.006, 0.008);
    if (pan_blur < 0.001 && zoom_blur < 0.001) {
        return base;
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
        let offset = (pan_dir * pan_blur + (uv - vec2<f32>(0.5)) * zoom_blur) * t;
        let sample_uv = clamp(uv + offset, vec2<f32>(0.0), vec2<f32>(1.0));
        color = color + sample_source_texture(sample_uv) * weight;
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

fn blend_with_layer(result: ptr<function,vec4<f32>>, foreground: ptr<function,vec4<f32>>, color: vec4<f32>) {
    *result = alpha_blend(*result,color);
    if (u.layer_outputs > 0.5) { *foreground = alpha_blend(*foreground,color); }
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

fn window_inverse_zoom(out_px: vec2<f32>) -> vec2<f32> {
    let source_origin = vec2<f32>(u.content_offset_x + u.input_offset_x,
        u.content_offset_y + u.input_offset_y);
    let rect = window_shutter_now();
    let rectangle_origin = rect.xy - rect.zw * 0.5;
    return source_origin + (out_px - rectangle_origin) / rect.zw
        * vec2<f32>(u.input_width,u.input_height);
}

@compute @workgroup_size(16, 16)
fn main(@builtin(global_invocation_id) gid: vec3<u32>) {
    let x = gid.x;
    let y = gid.y;
    let out_w = u32(u.output_width);
    let out_h = u32(u.output_height);
    if (x >= out_w || y >= out_h) { return; }

    let pixel = vec2<i32>(i32(x), i32(y));
    let out_px = vec2<f32>(f32(x), f32(y));

    let is_window = u.window_mode > 0.5;
    var px = out_px;
    if (is_window) { px = window_inverse_zoom(out_px); }

    // Keep wallpaper/gradient/solid backgrounds locked to the export canvas.
    // Window-mode zoom remaps only the captured window layer below.
    let backdrop = background_at(out_px);
    var result = backdrop;
    var front = vec4<f32>(0.0);

    if (u.shadow_enabled > 0.5 && !(is_window && window_temporal_active())) {
        let shadow_uv = vec2<f32>(px.x / u.output_width, px.y / u.output_height);
        let shadow = textureSampleLevel(shadow_tex, bilinear_sampler, shadow_uv, 0.0);
        blend_with_layer(&result,&front, shadow);
    }

    let content_x = px.x - u.content_offset_x;
    let content_y = px.y - u.content_offset_y;
    if (is_window && window_temporal_active()) {
        blend_with_layer(&result,&front,window_temporal_content(out_px));
    } else if (content_x >= 0.0 && content_x < u.content_width && content_y >= 0.0 && content_y < u.content_height) {
        let input_x = content_x - u.input_offset_x;
        let input_y = content_y - u.input_offset_y;
        if (input_x >= 0.0 && input_x < u.input_width && input_y >= 0.0 && input_y < u.input_height) {
            let content_uv = vec2<f32>(input_x / u.input_width, input_y / u.input_height);
            var content_color: vec4<f32>;
            if (is_window) {
                content_color = sample_window_motion_blur(content_uv);
            } else {
                content_color = sample_with_motion_blur(content_uv);
            }
            let has_native_window_mask = textureDimensions(corner_mask_tex).x > 1u;
            let native_window_alpha = sample_window_mask(content_uv);
            content_color.a = content_color.a * native_window_alpha;
            // Older recordings have no silhouette sidecar, so retain the
            // conservative compatibility radius only for those files.
            let needs_fallback_radius = is_window && !has_native_window_mask;
            let window_radius = select(
                0.0,
                min(u.input_width, u.input_height) * 0.022,
                needs_fallback_radius
            );
            let input_radius = max(u.corner_radius, window_radius);
            content_color.a = content_color.a * is_inside_rounded_rect(vec2<f32>(input_x, input_y), vec2<f32>(u.input_width, u.input_height), input_radius);
            blend_with_layer(&result,&front, content_color);
        }
    }

    let cursor_local = px - vec2<f32>(u.content_offset_x + u.input_offset_x, u.content_offset_y + u.input_offset_y);
    let cursor_in_video = all(cursor_local >= vec2<f32>(0.0)) && all(cursor_local < vec2<f32>(u.input_width,u.input_height));
    if (u.cursor_enabled > 0.5 && (u.clip_cursor_to_video < 0.5 || cursor_in_video)) {
        let s = u.cursor_size;
        let cursor_px = vec2<f32>(
            u.content_offset_x + u.input_offset_x + u.cursor_x * u.input_width,
            u.content_offset_y + u.input_offset_y + u.cursor_y * u.input_height
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
                var trail_px = vec2<f32>(
                    u.content_offset_x + u.input_offset_x + tp.x * u.input_width,
                    u.content_offset_y + u.input_offset_y + tp.y * u.input_height
                );
                var trail_size = tp.size;
                if (is_window) {
                    let rect = window_shutter_now();
                    trail_px = rect.xy + (vec2<f32>(tp.x,tp.y)-vec2<f32>(0.5))*rect.zw;
                    trail_size = trail_size * rect.z / u.input_width;
                }
                let dist = length(out_px - trail_px);
                let mask = 1.0 - smoothstep(trail_size - 0.5, trail_size + 0.5, dist);
                if (mask * tp.alpha > 0.001) {
                    let tc_color = vec3<f32>(u.cursor_color_r, u.cursor_color_g, u.cursor_color_b);
                    blend_with_layer(&result,&front, vec4<f32>(tc_color, mask * tp.alpha));
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
                blend_with_layer(&result,&front, vec4<f32>(ripple_color, ring_alpha * fade));
            }
            // Inner fill (first 50%)
            if (p < 0.5) {
                let inner_r = (4.0 + p * 20.0) * s;
                let inner_alpha = (0.5 - p) * 0.4;
                let inner_mask = 1.0 - smoothstep(inner_r - 0.5, inner_r + 0.5, dist);
                if (inner_mask * inner_alpha > 0.001) {
                    blend_with_layer(&result,&front, vec4<f32>(ripple_color, inner_mask * inner_alpha));
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
                blend_with_layer(&result,&front, vec4<f32>(hl_color, fill_mask * alpha * 0.24));
            }
            // Stroke
            let stroke_ring = abs(dist - radius);
            let stroke_mask = 1.0 - smoothstep(0.0, 2.0, stroke_ring);
            if (stroke_mask * alpha * 0.8 > 0.001) {
                blend_with_layer(&result,&front, vec4<f32>(hl_color, stroke_mask * alpha * 0.8));
            }
        }

        if (!cursor_temporal_active()) {
            // Keep edge filtering in output pixels, just like temporal samples.
            // Inverse window coordinates would scale the antialiasing width
            // when playback transitions between moving and stationary poses.
            let pose = cursor_shutter_now();
            draw_cursor_body(out_px,pose.xy,pose.z,pose.w,opacity,cursor_shutter_start().z,pose.z,&result,&front);
        }
    }

    if (u.cursor_enabled>0.5 && cursor_temporal_active()) {
        blend_with_layer(&result,&front,cursor_temporal_body(out_px));
    }

    // 5. Webcam overlay
    if (u.webcam_opacity > 0.0) {
        let fullscreen = u.webcam_shape > 1.5;
        let webcam_width = u.camera_width;
        let webcam_height = u.camera_height;
        let webcam_left = u.camera_left;
        let webcam_top = u.camera_top;
        let wx = f32(x) - webcam_left;
        let wy = f32(y) - webcam_top;
        if (wx >= 0.0 && wx < webcam_width && wy >= 0.0 && wy < webcam_height) {
            let webcam_uv = vec2<f32>(wx / webcam_width, wy / webcam_height);
            let webcam_dims = textureDimensions(webcam_tex);
            let webcam_aspect = (f32(webcam_dims.x) / max(f32(webcam_dims.y), 1.0)) / (webcam_width / webcam_height);
            var webcam_sample_uv = webcam_uv;
            if (webcam_aspect > 1.0) {
                webcam_sample_uv.x = 0.5 + (webcam_uv.x - 0.5) / webcam_aspect;
            } else {
                webcam_sample_uv.y = 0.5 + (webcam_uv.y - 0.5) * webcam_aspect;
            }
            if (u.webcam_mirrored > 0.5) { webcam_sample_uv.x = 1.0 - webcam_sample_uv.x; }
            var webcam_color = textureSampleLevel(webcam_tex, bilinear_sampler, webcam_sample_uv, 0.0);
            if (u.webcam_shape < 0.5) {
                let dist = length(webcam_uv - vec2<f32>(0.5));
                webcam_color.a = webcam_color.a * (1.0 - smoothstep(0.49, 0.5, dist));
            } else if (!fullscreen) {
                webcam_color.a = webcam_color.a * is_inside_rounded_rect(
                    vec2<f32>(wx, wy),
                    vec2<f32>(webcam_width, webcam_height),
                    webcam_width * 0.15
                );
            }
            webcam_color.a *= u.webcam_opacity;
            if (webcam_color.a > 0.001) {
                result = alpha_blend(result,webcam_color);
                if (u.layer_outputs > 0.5) {
                    front = alpha_blend(front,vec4<f32>(webcam_color.rgb,webcam_color.a*u.lens_camera_opacity));
                }
            }
        }
    }

    // 6. Device frame overlay
    if (u.device_frame_enabled > 0.5 && !(u.webcam_opacity > 0.0 && u.webcam_shape > 1.5)) {
        let bezel = u.device_frame_bezel;
        let df_radius = u.device_frame_corner_radius;
        let in_bezel_x = f32(x) < bezel || f32(x) >= u.output_width - bezel;
        let in_bezel_y = f32(y) < bezel || f32(y) >= u.output_height - bezel;
        if (in_bezel_x || in_bezel_y) {
            result = vec4<f32>(u.device_frame_r, u.device_frame_g, u.device_frame_b, 1.0);
            if (u.layer_outputs > 0.5) { front = result; }
        }
        if (df_radius > 0.0) {
            let size = vec2<f32>(u.output_width, u.output_height);
            let coverage = is_inside_rounded_rect(vec2<f32>(f32(x), f32(y)), size, df_radius);
            result.a *= coverage;
            front.a *= coverage;
        }
    }

    if (u.layer_outputs > 0.5) {
        textureStore(foreground_layer,pixel,vec4<f32>(front.rgb*front.a,front.a));
        textureStore(background_layer,pixel,vec4<f32>(backdrop.rgb*backdrop.a,backdrop.a));
        result = vec4<f32>(result.rgb*result.a,result.a);
    }
    textureStore(output_tex, pixel, result);
}
