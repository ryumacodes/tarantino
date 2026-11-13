@group(0) @binding(0) var source:texture_2d<f32>;
@group(0) @binding(1) var output:texture_storage_2d<rgba8unorm,write>;
@group(0) @binding(3) var<uniform> region:vec4<u32>;
fn original(p:vec2<i32>)->vec4<f32> {
    let pixel=clamp(p+vec2<i32>(region.xy),vec2<i32>(0),vec2<i32>(textureDimensions(source))-vec2<i32>(1));
    return textureLoad(source,pixel,0);
}
fn write_output(p:vec2<u32>,residual:f32,scale:u32) {
    let position=(vec2<f32>(p)+vec2<f32>(0.5))/f32(scale)-vec2<f32>(0.5);
    let origin=vec2<i32>(floor(position));let weight=fract(position);
    let a=original(origin);let b=original(origin+vec2<i32>(1,0));
    let c=original(origin+vec2<i32>(0,1));let d=original(origin+vec2<i32>(1,1));
    let base=mix(mix(a,b,weight.x),mix(c,d,weight.x),weight.y);
    let low=min(min(a.rgb,b.rgb),min(c.rgb,d.rgb));
    let high=max(max(a.rgb,b.rgb),max(c.rgb,d.rgb));
    textureStore(output,vec2<i32>(p),vec4<f32>(clamp(base.rgb+vec3<f32>(residual),low,high),base.a));
}
