fn cursor_body_radius() -> f32 {
    if(u.cursor_image_width<=0.0) {return 32.0;}
    let origin=vec2<f32>(u.cursor_image_left,u.cursor_image_top);
    let end=origin+vec2<f32>(u.cursor_image_width,u.cursor_image_height);
    return length(max(abs(origin),abs(end)));
}
fn cursor_shutter_start() -> vec4<f32> {
    return vec4<f32>(u.cursor_start_x,u.cursor_start_y,u.cursor_start_size,u.cursor_start_angle);
}
fn cursor_shutter_now() -> vec4<f32> {
    return vec4<f32>(u.cursor_now_x,u.cursor_now_y,u.cursor_now_size,u.cursor_now_angle);
}
fn cursor_temporal_active() -> bool {
    return u.motion_blur_enabled>0.5 && u.cursor_opacity>0.01 && any(abs(cursor_shutter_now()-cursor_shutter_start())>vec4<f32>(0.01));
}
fn cursor_temporal_body(pixel:vec2<f32>) -> vec4<f32> {
    let before=cursor_shutter_start();let now=cursor_shutter_now();
    let extent=cursor_body_radius();
    let radius=extent*max(before.z,now.z);
    if (any(pixel<min(before.xy,now.xy)-vec2<f32>(radius+3.0*u.cursor_fade_blur)) || any(pixel>max(before.xy,now.xy)+vec2<f32>(radius+3.0*u.cursor_fade_blur))) {return vec4<f32>(0.0);}
    let travel=length(now.xy-before.xy)+abs(now.w-before.w)*0.01745329252*radius+extent*abs(now.z-before.z);
    // Bound integration cost while resolving ordinary motion at roughly one output pixel per sample.
    // Analytic rotating-edge integration is still needed for very long shutter sweeps.
    let count=i32(clamp(ceil(travel),2.0,64.0));
    let color=select(vec3<f32>(u.cursor_color_r,u.cursor_color_g,u.cursor_color_b),
        vec3<f32>(u.cursor_highlight_r,u.cursor_highlight_g,u.cursor_highlight_b),u.is_clicking>0.5);
    var accumulated=vec4<f32>(0.0);
    for (var i=0;i<count;i=i+1) {
        let t=(f32(i)+0.5)/f32(count);let pose=mix(before,now,t);
        if (u.clip_cursor_to_video>0.5) {
            var rect=vec4<f32>(u.content_offset_x+u.input_offset_x+u.input_width*0.5,
                u.content_offset_y+u.input_offset_y+u.input_height*0.5,u.input_width,u.input_height);
            if (u.window_mode>0.5) {rect=mix(window_shutter_start(),window_shutter_now(),t);}
            if (any(abs(pixel-rect.xy)>=rect.zw*0.5)) {continue;}
        }
        var sample=vec4<f32>(0.0);var unused_layer=vec4<f32>(0.0);
        draw_cursor_body(pixel,pose.xy,pose.z,pose.w,u.cursor_opacity,color,&sample,&unused_layer);
        accumulated=accumulated+vec4<f32>(sample.rgb*sample.rgb*sample.a,sample.a)/f32(count);
    }
    if (accumulated.a<=0.0) {return vec4<f32>(0.0);}
    return vec4<f32>(sqrt(max(accumulated.rgb/accumulated.a,vec3<f32>(0.0))),accumulated.a);
}
