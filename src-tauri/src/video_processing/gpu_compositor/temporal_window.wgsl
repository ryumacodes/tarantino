fn source_sampling_lod() -> f32 {
    let extent=select(window_shutter_now().zw,window_shutter_start().zw,window_temporal_active());
    let footprint=vec2<f32>(textureDimensions(source_tex))*source_view().zw/extent;
    return max(0.0,log2(max(footprint.x,footprint.y)));
}
fn window_sample_phase(pixel:vec2<f32>) -> f32 {
    var cell=vec2<u32>(pixel);var rank=0u;
    for (var bit=0;bit<3;bit=bit+1) {
        rank=rank*4u+2u*((cell.x^cell.y)&1u)+(cell.y&1u);
        cell=cell>>vec2<u32>(1u);
    }
    return f32(rank)/64.0;
}
fn window_fetch_count(delta:vec2<f32>) -> i32 {
    let size=vec2<f32>(textureDimensions(source_tex))*source_view().zw;
    let texels_per_pixel=size/window_shutter_start().zw;
    let pixels_per_texel=inverseSqrt(max(max(texels_per_pixel.x*texels_per_pixel.x,texels_per_pixel.y*texels_per_pixel.y),1e-8));
    return i32(clamp(ceil(length(delta*size)*pixels_per_texel/4.0),1.0,16.0));
}
fn window_shutter_start() -> vec4<f32> {
    return vec4<f32>(u.shutter_start_x,u.shutter_start_y,u.shutter_start_w,u.shutter_start_h);
}
fn window_shutter_now() -> vec4<f32> {
    return vec4<f32>(u.shutter_now_x,u.shutter_now_y,u.shutter_now_w,u.shutter_now_h);
}
fn window_temporal_active() -> bool {
    return u.motion_blur_enabled>0.5 && any(abs(window_shutter_now()-window_shutter_start())>vec4<f32>(0.01));
}
fn window_temporal_distance(pixel:vec2<f32>,rect:vec4<f32>) -> f32 {
    let has_mask=textureDimensions(corner_mask_tex).x>1u;
    let fallback=select(min(u.input_width,u.input_height)*0.022,0.0,has_mask);
    let radius=min(max(u.corner_radius,fallback)*rect.z/u.input_width,min(rect.z,rect.w)*0.5);
    let q=abs(pixel-rect.xy)-rect.zw*0.5+vec2<f32>(radius);
    return length(max(q,vec2<f32>(0.0)))+min(max(q.x,q.y),0.0)-radius;
}
fn window_temporal_shadow(pixel:vec2<f32>,rect:vec4<f32>) -> vec4<f32> {
    let input_size=vec2<f32>(u.input_width,u.input_height);
    let base_center=vec2<f32>(u.content_offset_x+u.input_offset_x,u.content_offset_y+u.input_offset_y)+input_size*0.5;
    let local=(pixel-rect.xy)/rect.zw*input_size+base_center;
    let uv=local/vec2<f32>(u.output_width,u.output_height);
    if (any(uv<vec2<f32>(0.0))||any(uv>vec2<f32>(1.0))) {return vec4<f32>(0.0);}
    let shadow=textureSampleLevel(shadow_tex,bilinear_sampler,uv,0.0);
    return vec4<f32>(shadow.rgb*shadow.rgb*shadow.a,shadow.a);
}
fn window_temporal_content(pixel:vec2<f32>) -> vec4<f32> {
    let before=window_shutter_start();let now=window_shutter_now();
    var low=min(before.xy-before.zw*0.5,now.xy-now.zw*0.5)-vec2<f32>(1.0);
    var high=max(before.xy+before.zw*0.5,now.xy+now.zw*0.5)+vec2<f32>(1.0);
    if (u.shadow_enabled>0.5) {
        let input_size=vec2<f32>(u.input_width,u.input_height);
        let base_center=vec2<f32>(u.content_offset_x+u.input_offset_x,u.content_offset_y+u.input_offset_y)+input_size*0.5;
        let start_uv=-base_center/input_size;
        let end_uv=(vec2<f32>(u.output_width,u.output_height)-base_center)/input_size;
        low=min(low,min(before.xy+start_uv*before.zw,now.xy+start_uv*now.zw));
        high=max(high,max(before.xy+end_uv*before.zw,now.xy+end_uv*now.zw));
    }
    if (any(pixel<low)||any(pixel>high)) {return vec4<f32>(0.0);}
    let uv_start=(pixel-before.xy)/before.zw+vec2<f32>(0.5);
    let uv_end=(pixel-now.xy)/now.zw+vec2<f32>(0.5);
    if (length((uv_end-uv_start)*before.zw)<1.0) {
        let rect=(before+now)*0.5;
        let uv=clamp((uv_start+uv_end)*0.5,vec2<f32>(0.0),vec2<f32>(1.0));
        let color=sample_source_texture(uv);
        let mask=sample_window_mask(uv);
        let alpha=color.a*mask*(1.0-smoothstep(-0.5,0.5,window_temporal_distance(pixel,rect)));
        var content=vec4<f32>(color.rgb*color.rgb*alpha,alpha);
        if (u.shadow_enabled>0.5) {content=content+window_temporal_shadow(pixel,rect)*(1.0-content.a);}
        if (content.a<=0.0) {return vec4<f32>(0.0);}
        return vec4<f32>(sqrt(max(content.rgb/content.a,vec3<f32>(0.0))),content.a);
    }
    let distance=motion_fit_distance(window_temporal_distance(pixel,before),
        window_temporal_distance(pixel,(before+now)*0.5),window_temporal_distance(pixel,now));
    let coverage=motion_coverage_ramp(distance,0.0,0.5);
    let roots=motion_level_roots(distance,0.0,0.0,1.0);
    let edges=array<f32,4>(0.0,min(roots.x,roots.y),max(roots.x,roots.y),1.0);
    var accumulated=vec4<f32>(0.0);var total_weight=0.0;
    for (var segment=0;segment<3;segment=segment+1) {
        let start=edges[segment];let end=edges[segment+1];let span=end-start;
        if (span<0.0001) {continue;}
        let alpha=motion_integrate_coverage(coverage,start,end)/span;
        var content=vec4<f32>(0.0);
        if (alpha>0.001) {
            let a=mix(uv_start,uv_end,start);let b=mix(uv_start,uv_end,end);
            let count=window_fetch_count(b-a);
            for (var tap=0;tap<count;tap=tap+1) {
                let uv=clamp(mix(a,b,(f32(tap)+window_sample_phase(pixel))/f32(count)),vec2<f32>(0.0),vec2<f32>(1.0));
                let color=sample_source_segment(uv,b-a,count);
                let mask=sample_window_mask(uv);
                let opacity=color.a*mask;
                content=content+vec4<f32>(color.rgb*color.rgb*opacity,opacity)/f32(count);
            }
            content=content*alpha;
        }
        if (u.shadow_enabled>0.5 && content.a<0.999) {
            let shadow=window_temporal_shadow(pixel,mix(before,now,(start+end)*0.5));
            content=content+shadow*(1.0-content.a);
        }
        let weight=span*(1.0+2.0*smoothstep(0.0,1.0,1.0-content.a));
        accumulated=accumulated+content*weight;total_weight=total_weight+weight;
    }
    if (total_weight<=0.0 || accumulated.a<=0.0) {return vec4<f32>(0.0);}
    return vec4<f32>(sqrt(max(accumulated.rgb/accumulated.a,vec3<f32>(0.0))),accumulated.a/total_weight);
}
