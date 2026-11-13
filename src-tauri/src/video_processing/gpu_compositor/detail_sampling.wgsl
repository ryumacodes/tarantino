fn detail_minor_lod(dx:vec2<f32>,dy:vec2<f32>,composite_size:vec2<f32>)->f32 {
    let footprint=min(length(dx*composite_size),length(dy*composite_size));
    return max(0.0,log2(max(footprint,0.000001)));
}

fn detail_source_lod(composite_lod:f32,crop_span:vec2<f32>,source_size:vec2<f32>,
    region_span:vec2<f32>,composite_size:vec2<f32>)->f32 {
    let density=crop_span*source_size/max(region_span*composite_size,vec2<f32>(0.000001));
    return max(0.0,composite_lod+log2(max(max(density.x,density.y),0.000001)));
}

fn sample_detail(base:vec4<f32>,enhanced:texture_2d<f32>,linear_sampler:sampler,
    source_uv:vec2<f32>,dx:vec2<f32>,dy:vec2<f32>,window:vec4<f32>,
    content_size:vec2<f32>,strength:f32,source_lod:f32)->vec4<f32> {
    if(!(strength>=0.2 && source_lod<0.05)){return base;}
    let texture_size=vec2<f32>(textureDimensions(enhanced));
    let local=(source_uv-window.xy)/window.zw;
    let uv=clamp(local*content_size,vec2<f32>(0.5),content_size-vec2<f32>(0.5))/texture_size;
    let gradient_scale=content_size/(window.zw*texture_size);
    let detail=textureSampleGrad(enhanced,linear_sampler,uv,dx*gradient_scale,dy*gradient_scale);
    if(detail.a>0.0){return mix(base,detail,strength);}
    return base;
}
