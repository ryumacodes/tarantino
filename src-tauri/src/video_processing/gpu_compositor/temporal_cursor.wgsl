fn cursor_body_radius() -> f32 {
    if(u.cursor_image_width<=0.0) {return length(vec2<f32>(40.0));}
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
    if(u.motion_blur_enabled<=0.5 || u.cursor_opacity<=0.01) {return false;}
    let before=cursor_shutter_start();let now=cursor_shutter_now();
    let rect=select(vec4<f32>(-24.0,-24.0,64.0,64.0),
        vec4<f32>(u.cursor_image_left,u.cursor_image_top,u.cursor_image_width,u.cursor_image_height),u.cursor_image_width>0.0);
    let old_box=vec4<f32>(before.xy+rect.xy*before.z,rect.zw*before.z);
    let new_box=vec4<f32>(now.xy+rect.xy*now.z,rect.zw*now.z);
    let tolerance=u.output_height/1080.0;
    let rotation_travel=abs(now.w-before.w)*0.01745329252*cursor_body_radius()*max(before.z,now.z);
    return any(abs(new_box-old_box)>=vec4<f32>(tolerance))
        || any(abs(now.xy-before.xy)>=vec2<f32>(tolerance)) || rotation_travel>=tolerance;
}
fn cursor_temporal_body(pixel:vec2<f32>) -> vec4<f32> {
    let before=cursor_shutter_start();let now=cursor_shutter_now();
    let extent=cursor_body_radius();
    let radius=extent*max(before.z,now.z);
    if (any(pixel<min(before.xy,now.xy)-vec2<f32>(radius+1.5+3.0*u.cursor_fade_blur)) || any(pixel>max(before.xy,now.xy)+vec2<f32>(radius+1.5+3.0*u.cursor_fade_blur))) {return vec4<f32>(0.0);}
    let travel=length(now.xy-before.xy)+abs(now.w-before.w)*0.01745329252*radius+extent*abs(now.z-before.z);
    // Edge-clipped motion still integrates the moving video bounds per pose.
    let count=i32(clamp(ceil(travel),2.0,64.0));
    if(u.clip_cursor_to_video<0.5) {
        let start_local=rotate_2d(pixel-before.xy,-before.w*0.01745329252)/before.z;
        let end_local=rotate_2d(pixel-now.xy,-now.w*0.01745329252)/now.z;
        if(length((end_local-start_local)*before.z)<1.0) {
            let pose=mix(before,now,0.5);
            var sample=vec4<f32>(0.0);var unused_layer=vec4<f32>(0.0);
            draw_cursor_body(pixel,pose.xy,pose.z,pose.w,u.cursor_opacity,before.z,now.z,&sample,&unused_layer);
            return sample;
        }
        return cursor_segments(pixel,before,now);
    }
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
        draw_cursor_body(pixel,pose.xy,pose.z,pose.w,u.cursor_opacity,pose.z,pose.z,&sample,&unused_layer);
        accumulated=accumulated+vec4<f32>(sample.rgb*sample.rgb*sample.a,sample.a)/f32(count);
    }
    if (accumulated.a<=0.0) {return vec4<f32>(0.0);}
    return vec4<f32>(sqrt(max(accumulated.rgb/accumulated.a,vec3<f32>(0.0))),accumulated.a);
}
