struct Placement { rect:vec4<f32>, resolution:vec2<f32>, opacity:f32, blur_radius:f32 };
@group(0) @binding(0) var atlas:texture_2d<f32>;
@group(0) @binding(1) var filtering:sampler;
@group(0) @binding(2) var<uniform> placement:Placement;
struct Vertex { @builtin(position) position:vec4<f32>, @location(0) uv:vec2<f32> };
@vertex fn vertex(@builtin(vertex_index) index:u32)->Vertex {
    let uv=vec2<f32>(f32(index%2u),f32(index/2u));
    let p=(placement.rect.xy+uv*placement.rect.zw)/placement.resolution;
    return Vertex(vec4<f32>(p.x*2.0-1.0,1.0-p.y*2.0,0.0,1.0),uv);
}
@fragment fn fragment(input:Vertex)->@location(0) vec4<f32> {
    let pixel=vec2<u32>(floor(input.position.xy));
    let noise=vec2<f32>(blue_noise(pixel),blue_noise(pixel+vec2<u32>(37u)));
    let size=vec2<f32>(textureDimensions(atlas));
    let lod=max(0.0,log2(min(size.x/placement.rect.z,size.y/placement.rect.w)));
    return mip_sample_blurred_border(atlas,filtering,input.uv,
        placement.blur_radius*size.y/placement.rect.w,lod,noise,true)*placement.opacity;
}
