// Scene input/output are premultiplied linear colors. Cover colors are the
// premultiplied encoded quadrant averages produced by the sampling pass.
struct Mask { rect:vec4<f32>, controls:vec4<f32>, curvature:array<vec4<f32>,4> };
@group(0) @binding(0) var scene:texture_2d<f32>;
@group(0) @binding(1) var colors:texture_2d<f32>;
@group(0) @binding(2) var filtering:sampler;
@group(0) @binding(3) var<uniform> mask:Mask;
@vertex fn vertex(@builtin(vertex_index) index:u32)->@builtin(position) vec4<f32> {
    let uv=vec2<f32>(f32(index%2u),f32(index/2u));
    return vec4<f32>(uv.x*2.0-1.0,1.0-uv.y*2.0,0.0,1.0);
}
fn radial_ratio(q:vec2<f32>)->f32 {
    if(q.x<=0.0 || q.y<=0.0){return 1.0;}
    let sample=clamp(atan2(min(q.x,q.y),max(q.x,q.y))*19.098593,0.0,15.0);
    let low=u32(sample);let high=min(low+1u,15u);
    return mix(mask.curvature[low/4u][low%4u],mask.curvature[high/4u][high%4u],fract(sample));
}
fn apply_mask(position:vec2<f32>,linear:vec4<f32>)->vec4<f32> {
    let radius=mask.controls.x;
    let half_size=mask.rect.zw*0.5;
    let q=abs(position-mask.rect.xy-half_size)-half_size+radius;
    let distance=length(max(q,vec2<f32>(0.0)))+min(max(q.x,q.y),0.0)-radius*radial_ratio(q);
    let aa=max(fwidth(distance),0.00001);
    let inside=1.0-smoothstep(-aa,aa,distance);
    let uv=clamp((position-mask.rect.xy)/max(mask.rect.zw,vec2<f32>(0.000001)),vec2<f32>(0.0),vec2<f32>(1.0));
    let texel=textureSampleLevel(colors,filtering,uv*0.5+0.25,0.0);
    let straight=texel.rgb/max(texel.a,0.00001);
    let cover=vec4<f32>(straight*straight,1.0);
    let covered=mix(linear,cover,inside*mask.controls.y);
    let dimmed=mix(linear,vec4<f32>(0.0,0.0,0.0,1.0),mask.controls.z*(1.0-inside)*mask.controls.y);
    return mix(dimmed,covered,mask.controls.w);
}

@fragment fn fragment(@builtin(position) position:vec4<f32>)->@location(0) vec4<f32> {
    return apply_mask(position.xy,textureLoad(scene,vec2<i32>(position.xy),0));
}
// Recording textures use straight encoded RGBA. Keep conversion within this
// pass so it adds no full-resolution intermediate texture or extra dispatch.
@fragment fn fragment_encoded(@builtin(position) position:vec4<f32>)->@location(0) vec4<f32> {
    let encoded=textureLoad(scene,vec2<i32>(position.xy),0);
    if(mask.controls.y==0.0){return encoded;}
    let linear=vec4<f32>(encoded.rgb*encoded.rgb*encoded.a,encoded.a);
    let result=apply_mask(position.xy,linear);
    if(all(result==linear)){return encoded;}
    return vec4<f32>(sqrt(max(result.rgb,vec3<f32>(0.0))/max(result.a,0.00001)),result.a);
}
