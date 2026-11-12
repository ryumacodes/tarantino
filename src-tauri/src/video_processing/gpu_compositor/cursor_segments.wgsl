fn cursor_segment_splits(p:vec3<f32>) -> vec2<f32> {
    if(abs(p.x)>0.00001*(abs(p.y)+abs(p.z)+0.000001)) {
        let discriminant=p.y*p.y-4.0*p.x*p.z;
        if(discriminant<=0.0) {return vec2<f32>(1.0);}
        let q=-0.5*(p.y+select(-1.0,1.0,p.y>=0.0)*sqrt(discriminant));
        let roots=vec2<f32>(q/p.x,p.z/q);
        return clamp(vec2<f32>(min(roots.x,roots.y),max(roots.x,roots.y)),vec2<f32>(0.0),vec2<f32>(1.0));
    }
    if(abs(p.y)>0.00000001) {return vec2<f32>(clamp(-p.z/p.y,0.0,1.0),1.0);}
    return vec2<f32>(1.0);
}
fn cursor_comb_phase(pixel:vec2<f32>) -> f32 {
    let p=vec2<u32>(pixel)%vec2<u32>(8u);
    var value=0u;
    for(var bit=0u;bit<3u;bit=bit+1u) {
        let x=(p.x>>bit)&1u;let y=(p.y>>bit)&1u;
        value=value+(((x^y)*2u+y)<<(4u-bit*2u));
    }
    return f32(value)/64.0;
}
fn cursor_segment_local(pixel:vec2<f32>,pose:vec4<f32>) -> vec2<f32> {
    return rotate_2d(pixel-pose.xy,-pose.w*0.01745329252)/pose.z;
}
fn cursor_segment_distance(pixel:vec2<f32>,pose:vec4<f32>,rect:vec4<f32>) -> f32 {
    let delta=(abs(cursor_segment_local(pixel,pose)-rect.xy-rect.zw*0.5)-rect.zw*0.5)*pose.z;
    return length(max(delta,vec2<f32>(0.0)))+min(max(delta.x,delta.y),0.0);
}
fn cursor_segments(pixel:vec2<f32>,before:vec4<f32>,now:vec4<f32>) -> vec4<f32> {
    let rect=select(vec4<f32>(-24.0,-24.0,64.0,64.0),
        vec4<f32>(u.cursor_image_left,u.cursor_image_top,u.cursor_image_width,u.cursor_image_height),u.cursor_image_width>0.0);
    let fit=motion_fit_distance(cursor_segment_distance(pixel,before,rect),
        cursor_segment_distance(pixel,mix(before,now,0.5),rect),cursor_segment_distance(pixel,now,rect));
    let coverage=motion_coverage_ramp(fit,0.0,1.5);
    let splits=cursor_segment_splits(fit);
    let edges=array<f32,4>(0.0,splits.x,splits.y,1.0);
    let dimensions=vec2<f32>(textureDimensions(cursor_tex));
    let footprint=dimensions/(rect.zw*before.z);
    let texels_per_pixel=max(footprint.x,footprint.y);
    let lod=max(0.0,log2(texels_per_pixel));
    let phase=cursor_comb_phase(pixel);
    var total=vec4<f32>(0.0);var weight=0.0;
    for(var segment=0;segment<3;segment=segment+1) {
        let span=edges[segment+1]-edges[segment];
        if(span<=0.0001) {continue;}
        weight=weight+span;
        let alpha=clamp(motion_integrate_coverage(coverage,edges[segment],edges[segment+1])/span,0.0,1.0);
        if(alpha<=0.001) {continue;}
        let start=(cursor_segment_local(pixel,mix(before,now,edges[segment]))-rect.xy)/rect.zw;
        let end=(cursor_segment_local(pixel,mix(before,now,edges[segment+1]))-rect.xy)/rect.zw;
        let delta=end-start;let texel_delta=delta*dimensions;
        let count=i32(clamp(ceil(length(texel_delta)/texels_per_pixel/4.0),1.0,16.0));
        let along=delta/f32(count);
        let across=vec2<f32>(-texel_delta.y,texel_delta.x)/max(length(texel_delta),0.000001)*exp2(lod)/dimensions;
        var color=vec4<f32>(0.0);
        for(var sample_index=0;sample_index<count;sample_index=sample_index+1) {
            let uv=start+delta*(f32(sample_index)+phase)/f32(count);
            var sample=vec4<f32>(0.0);
            if(u.cursor_fade_blur<=0.04) {
                sample=textureSampleGrad(cursor_tex,cursor_motion_sampler,uv,along,across);
            } else {
                let blur_lod=max(0.0,log2(max(min(length(along*dimensions),length(across*dimensions)),0.000001)));
                let noise_pixel=vec2<u32>(pixel);
                sample=mip_sample_blurred_border(cursor_tex,bilinear_sampler,uv,
                    u.cursor_fade_blur*dimensions.y/(rect.w*now.z),blur_lod,
                    vec2<f32>(blue_noise(noise_pixel),blue_noise(noise_pixel+vec2<u32>(37u))),true);
            }
            let rgb=clamp(sample.rgb,vec3<f32>(0.0),vec3<f32>(sample.a));
            color=color+vec4<f32>(rgb*rgb/max(sample.a,0.00001),sample.a)/f32(count);
        }
        total=total+color*(alpha*span);
    }
    total=total/weight;
    if(total.a<=0.0) {return vec4<f32>(0.0);}
    return vec4<f32>(sqrt(max(total.rgb/total.a,vec3<f32>(0.0))),total.a*u.cursor_opacity);
}
