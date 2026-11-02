struct Shape { rect:vec4<f32>, color:vec4<f32>, resolution:vec2<f32>, radius:f32, opacity:f32, curvature:array<vec4<f32>,4> };
@group(0) @binding(0) var<uniform> shape:Shape;
@vertex fn vertex(@builtin(vertex_index) index:u32)->@builtin(position) vec4<f32> {
    let uv=vec2<f32>(f32(index%2u),f32(index/2u));
    return vec4<f32>(uv.x*2.0-1.0,1.0-uv.y*2.0,0.0,1.0);
}
fn ratio(q:vec2<f32>)->f32 {
    if(q.x<=0.0 || q.y<=0.0){return 1.0;}
    let angle=atan2(min(q.x,q.y),max(q.x,q.y));
    let sample=clamp(angle*19.098593,0.0,15.0);
    let low=u32(sample); let high=min(low+1u,15u);
    return mix(shape.curvature[low/4u][low%4u],shape.curvature[high/4u][high%4u],fract(sample));
}
fn distance(q:vec2<f32>)->f32 {
    return length(max(q,vec2<f32>(0.0)))+min(max(q.x,q.y),0.0)-shape.radius*ratio(q);
}
@fragment fn fragment(@builtin(position) position:vec4<f32>)->@location(0) vec4<f32> {
    let half_size=shape.rect.zw*0.5;
    let q=abs(position.xy-shape.rect.xy-half_size)-half_size+vec2<f32>(shape.radius);
    let d=distance(q);
    let coverage=clamp(0.5-d/max(fwidth(d),0.001),0.0,1.0);
    // The centered outline expands the corner field before its outer threshold.
    let outer=distance(q-vec2<f32>(5.0))-5.0;
    let outline=clamp(0.5-outer/max(fwidth(outer),0.001),0.0,1.0)*0.1;
    let fill_alpha=shape.color.a*coverage;
    let behind=outline*(1.0-fill_alpha);
    let rgb=shape.color.rgb*fill_alpha+(vec3<f32>(1.0)-shape.color.rgb)*behind;
    return vec4<f32>(rgb,fill_alpha+behind)*shape.opacity;
}
