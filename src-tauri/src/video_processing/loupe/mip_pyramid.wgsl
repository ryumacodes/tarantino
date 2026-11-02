@group(0) @binding(0) var source: texture_2d<f32>;
@group(0) @binding(1) var output: texture_storage_2d<rgba8unorm,write>;

fn read_clamped(point: vec2<i32>) -> vec4<f32> {
    return textureLoad(source,clamp(point,vec2<i32>(0),vec2<i32>(textureDimensions(source))-1),0);
}

@compute @workgroup_size(8,8)
fn copy_level(@builtin(global_invocation_id) id: vec3<u32>) {
    if (any(id.xy >= textureDimensions(output))) { return; }
    textureStore(output,vec2<i32>(id.xy),textureLoad(source,vec2<i32>(id.xy),0));
}

@compute @workgroup_size(8,8)
fn box_level(@builtin(global_invocation_id) id: vec3<u32>) {
    if (any(id.xy >= textureDimensions(output))) { return; }
    let origin=vec2<i32>(id.xy)*2;
    let color=(read_clamped(origin)+read_clamped(origin+vec2<i32>(1,0))
        +read_clamped(origin+vec2<i32>(0,1))+read_clamped(origin+vec2<i32>(1,1)))*0.25;
    textureStore(output,vec2<i32>(id.xy),color);
}

@compute @workgroup_size(8,8)
fn binomial_level(@builtin(global_invocation_id) id: vec3<u32>) {
    if (any(id.xy >= textureDimensions(output))) { return; }
    let origin=vec2<i32>(id.xy)*2-1;
    let weights=array<f32,4>(0.125,0.375,0.375,0.125);
    var color=vec4<f32>(0.0);
    for(var y=0; y<4; y+=1) {
        for(var x=0; x<4; x+=1) {
            color+=read_clamped(origin+vec2<i32>(x,y))*weights[x]*weights[y];
        }
    }
    textureStore(output,vec2<i32>(id.xy),color);
}

@compute @workgroup_size(8,8)
fn premultiply_level(@builtin(global_invocation_id) id: vec3<u32>) {
    if (any(id.xy >= textureDimensions(output))) { return; }
    let color=textureLoad(source,vec2<i32>(id.xy),0);
    textureStore(output,vec2<i32>(id.xy),vec4<f32>(color.rgb*color.a,color.a));
}
