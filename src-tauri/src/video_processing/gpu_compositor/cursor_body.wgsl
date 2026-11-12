// Cursor body and its shadow only. Trails and click indicators are separate layers.
fn draw_cursor_procedural(pixel:vec2<f32>,position:vec2<f32>,size:f32,rotation:f32,
    opacity:f32,active_color:vec3<f32>,result:ptr<function,vec4<f32>>,front:ptr<function,vec4<f32>>) {
    if (length(pixel-position)>=60.0*size || opacity<=0.01) {return;}
    let s=size;
    var local=pixel-position;
    let radians=rotation*3.14159265/180.0;
    if (abs(radians)>0.001) {local=rotate_2d(local,-radians);}
    let shadow_alpha_base = (u.cursor_shadow_intensity / 100.0) * 0.5;

    if (u.cursor_style < 0.5) {
        // Pointer style
        if (shadow_alpha_base > 0.001) {
            let sl = local - vec2<f32>(2.0 * s, 2.0 * s);
            let sf = arrow_mask(sl, s);
            if (sf * shadow_alpha_base * opacity > 0.001) {
                blend_with_layer(result,front, vec4<f32>(0.0, 0.0, 0.0, sf * shadow_alpha_base * opacity));
            }
        }
        let fill = arrow_mask(local, s);
        if (fill > 0.001) {
            blend_with_layer(result,front, vec4<f32>(active_color, fill * opacity));
        }
        let edge_d = arrow_edge_dist(local, s);
        let stroke_w = 1.5 * s;
        let stroke_m = 1.0 - smoothstep(0.0, stroke_w, edge_d);
        let stroke_a = stroke_m * (1.0 - fill * 0.5);
        if (stroke_a * opacity > 0.001) {
            blend_with_layer(result,front, vec4<f32>(0.0, 0.0, 0.0, stroke_a * opacity));
        }
    } else if (u.cursor_style < 1.5) {
        // Circle style
        if (shadow_alpha_base > 0.001) {
            let sl = local - vec2<f32>(2.0 * s, 2.0 * s);
            let sd = length(sl);
            let sm = 1.0 - smoothstep(10.0 * s - 0.5, 10.0 * s + 0.5, sd);
            if (sm * shadow_alpha_base * opacity > 0.001) {
                blend_with_layer(result,front, vec4<f32>(0.0, 0.0, 0.0, sm * shadow_alpha_base * opacity));
            }
        }
        let dist = length(local);
        let mask = 1.0 - smoothstep(10.0 * s - 0.5, 10.0 * s + 0.5, dist);
        if (mask * 0.8 * opacity > 0.001) {
            blend_with_layer(result,front, vec4<f32>(0.5, 0.5, 0.5, mask * 0.8 * opacity));
        }
    } else if (u.cursor_style < 2.5) {
        // Filled style (black fill, white stroke)
        if (shadow_alpha_base > 0.001) {
            let sl = local - vec2<f32>(2.0 * s, 2.0 * s);
            let sf = arrow_mask(sl, s);
            if (sf * shadow_alpha_base * opacity > 0.001) {
                blend_with_layer(result,front, vec4<f32>(0.0, 0.0, 0.0, sf * shadow_alpha_base * opacity));
            }
        }
        let fill = arrow_mask(local, s);
        if (fill > 0.001) {
            blend_with_layer(result,front, vec4<f32>(0.0, 0.0, 0.0, fill * opacity));
        }
        let edge_d = arrow_edge_dist(local, s);
        let stroke_m = 1.0 - smoothstep(0.0, 2.0 * s, edge_d);
        let stroke_a = stroke_m * (1.0 - fill * 0.5);
        if (stroke_a * opacity > 0.001) {
            blend_with_layer(result,front, vec4<f32>(1.0, 1.0, 1.0, stroke_a * opacity));
        }
    } else if (u.cursor_style < 3.5) {
        // Outline style (white stroke only)
        if (shadow_alpha_base > 0.001) {
            let sl = local - vec2<f32>(2.0 * s, 2.0 * s);
            let se = arrow_edge_dist(sl, s);
            let ss = 1.0 - smoothstep(0.0, 2.0 * s, se);
            if (ss * shadow_alpha_base * opacity > 0.001) {
                blend_with_layer(result,front, vec4<f32>(0.0, 0.0, 0.0, ss * shadow_alpha_base * opacity));
            }
        }
        let edge_d = arrow_edge_dist(local, s);
        let stroke_m = 1.0 - smoothstep(0.0, 2.0 * s, edge_d);
        if (stroke_m * opacity > 0.001) {
            blend_with_layer(result,front, vec4<f32>(1.0, 1.0, 1.0, stroke_m * opacity));
        }
    } else {
        // Dotted style (fill + dashed stroke)
        if (shadow_alpha_base > 0.001) {
            let sl = local - vec2<f32>(2.0 * s, 2.0 * s);
            let sf = arrow_mask(sl, s);
            if (sf * shadow_alpha_base * opacity > 0.001) {
                blend_with_layer(result,front, vec4<f32>(0.0, 0.0, 0.0, sf * shadow_alpha_base * opacity));
            }
        }
        let fill = arrow_mask(local, s);
        if (fill > 0.001) {
            blend_with_layer(result,front, vec4<f32>(active_color, fill * opacity));
        }
        let edge_d = arrow_edge_dist(local, s);
        let stroke_m = 1.0 - smoothstep(0.0, 1.5 * s, edge_d);
        let dash_period = 4.0 * s;
        let dash_on = step(0.5, fract((local.x + local.y) / dash_period));
        let stroke_a = stroke_m * dash_on * (1.0 - fill * 0.5);
        if (stroke_a * opacity > 0.001) {
            blend_with_layer(result,front, vec4<f32>(0.0, 0.0, 0.0, stroke_a * opacity));
        }
    }
}

// The fade radius is in output pixels, independent of cursor size and rotation.
fn draw_cursor_body(pixel:vec2<f32>,position:vec2<f32>,size:f32,rotation:f32,
    opacity:f32,result:ptr<function,vec4<f32>>,front:ptr<function,vec4<f32>>) {
    if(opacity<=0.01 || size<=0.0) {return;}
    let local=rotate_2d(pixel-position,-rotation*0.01745329252)/size;
    let rect=select(vec4<f32>(-24.0,-24.0,64.0,64.0),
        vec4<f32>(u.cursor_image_left,u.cursor_image_top,u.cursor_image_width,u.cursor_image_height),u.cursor_image_width>0.0);
    let margin=3.0*u.cursor_fade_blur/size;
    if(any(local<rect.xy-vec2<f32>(margin)) || any(local>rect.xy+rect.zw+vec2<f32>(margin))) {return;}
    let uv=(local-rect.xy)/rect.zw;
    let texels_per_pixel=vec2<f32>(textureDimensions(cursor_tex))/(rect.zw*size);
    let sampling_lod=max(0.0,log2(max(texels_per_pixel.x,texels_per_pixel.y)));
    var sampled=vec4<f32>(0.0);
    if(u.cursor_fade_blur<=0.04) {
        if(all(uv>=vec2<f32>(0.0)) && all(uv<=vec2<f32>(1.0))) {
            sampled=textureSampleLevel(cursor_tex,bilinear_sampler,uv,sampling_lod);
        }
    } else {
        let noise_pixel=vec2<u32>(max(floor(pixel),vec2<f32>(0.0)));
        let texel_scale=f32(textureDimensions(cursor_tex).x)/(rect.z*size);
        sampled=mip_sample_blurred_border(cursor_tex,bilinear_sampler,uv,u.cursor_fade_blur*texel_scale,sampling_lod,
            vec2<f32>(blue_noise(noise_pixel),blue_noise(noise_pixel+vec2<u32>(37u))),true);
    }
    if(sampled.a>0.0) {blend_with_layer(result,front,vec4<f32>(sampled.rgb/sampled.a,sampled.a*opacity));}
}
