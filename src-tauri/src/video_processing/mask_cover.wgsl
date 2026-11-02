struct Parameters {
    lattice: vec4<f32>,
    span_and_taps: vec4<f32>,
    level_and_padding: vec4<f32>,
}
@group(0) @binding(0) var source: texture_2d<f32>;
@group(0) @binding(1) var source_sampler: sampler;
@group(0) @binding(2) var<uniform> params: Parameters;
@group(0) @binding(3) var output: texture_storage_2d<rgba8unorm, write>;

@compute @workgroup_size(2, 2)
fn main(@builtin(global_invocation_id) id: vec3<u32>) {
    if (id.x >= 2u || id.y >= 2u) { return; }
    let taps = vec2<u32>(params.span_and_taps.zw);
    let extent = params.lattice.zw * 0.5;
    let origin = params.lattice.xy + vec2<f32>(id.xy) * extent;
    let step = extent / vec2<f32>(taps);
    var total = vec4<f32>(0.0);
    for (var y = 0u; y < taps.y; y++) {
        for (var x = 0u; x < taps.x; x++) {
            let position = origin + (vec2<f32>(f32(x), f32(y)) + 0.5) * step;
            total += textureSampleLevel(source, source_sampler, position / params.span_and_taps.xy, params.level_and_padding.x);
        }
    }
    textureStore(output, vec2<i32>(id.xy), total / f32(taps.x * taps.y));
}
