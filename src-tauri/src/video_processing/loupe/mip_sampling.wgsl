// Filtered-pyramid reads. Noise is supplied by the caller so the sampling
// pattern can remain stable in screen space across moving scene content.
fn mip_stretch(size: vec2<f32>, level: f32) -> vec2<f32> {
    let span = exp2(level);
    return size / (max(floor(size / span), vec2<f32>(1.0)) * span);
}

// Transparent extension is used for isolated cursor tiles. Scene textures keep
// edge clamping. Each integer mip needs its own half-texel border coverage.
fn mip_read_border(scene:texture_2d<f32>,filtering:sampler,uv:vec2<f32>,lod:f32) -> vec4<f32> {
    let size=vec2<f32>(textureDimensions(scene,u32(lod)));
    let p=uv*size;
    let coverage=clamp(min(p+vec2<f32>(0.5),size+vec2<f32>(0.5)-p),vec2<f32>(0.0),vec2<f32>(1.0));
    return textureSampleLevel(scene,filtering,uv,lod)*coverage.x*coverage.y;
}
fn mip_sample_at_border(scene: texture_2d<f32>, filtering: sampler, uv: vec2<f32>, lod: f32, offset: vec2<f32>, transparent:bool) -> vec4<f32> {
    let size = vec2<f32>(textureDimensions(scene));
    let level = floor(lod);
    let fraction = fract(lod);
    let scale0 = mip_stretch(size, level);
    let scale1 = mip_stretch(size, level + 1.0);
    let position = uv + offset / size;
    if(transparent) {
        let upper=min(level+1.0,f32(textureNumLevels(scene)-1u));
        return mix(mip_read_border(scene,filtering,position*scale0,level),
            mip_read_border(scene,filtering,position*mip_stretch(size,upper),upper),fraction);
    }
    // Hardware mip sizes round down. Blending separately avoids stretching
    // odd dimensions differently on either side of a fractional LOD.
    if (any(abs(scale1 - scale0) > vec2<f32>(0.01))) {
        return mix(textureSampleLevel(scene, filtering, position * scale0, level),
            textureSampleLevel(scene, filtering, position * scale1, level + 1.0), fraction);
    }
    return textureSampleLevel(scene, filtering, position * mix(scale0, scale1, fraction), lod);
}

fn mip_sample_blurred_border(scene: texture_2d<f32>, filtering: sampler, uv: vec2<f32>, radius: f32, min_lod: f32, noise: vec2<f32>,transparent:bool) -> vec4<f32> {
    let sigma = max(radius, 0.0) * 0.5;
    if (sigma <= 0.02) {
        if(!transparent) {return textureSampleLevel(scene,filtering,uv,min_lod);}
        return mip_sample_at_border(scene,filtering,uv,min_lod,vec2<f32>(0.0),transparent);
    }
    let spread = 0.6 * clamp(1.0 - (log2(sigma) - 1.0) / 3.0, 0.0, 1.0);
    let ring = 0.5 + 0.4 * spread;
    let jitter = 1.6 * spread;
    let variance = 1.0 / 6.0 + ring * ring * (1.0 + jitter * jitter / 12.0) * 0.5;
    // Variance of the first box level and subsequent binomial levels.
    var span_squared = (sigma * sigma + 0.75) / (0.25 + variance);
    if (span_squared < 4.0) {
        span_squared = (sigma * sigma + 1.0 / 12.0) / (1.0 / 12.0 + variance);
    }
    let span = sqrt(span_squared);
    let maximum_lod = f32(textureNumLevels(scene) - 1u);
    let lod = clamp(max(log2(span), min_lod), 0.0, maximum_lod);
    let ring_radius = ring * span;
    if (ring_radius < 0.05) {
        return mip_sample_at_border(scene, filtering, uv, lod, vec2<f32>(0.0),transparent);
    }
    let radius_px = ring_radius * (1.0 + jitter * (fract(noise.y) - 0.5));
    var result = vec4<f32>(0.0);
    for (var index = 0u; index < 3u; index += 1u) {
        let angle = (noise.x + f32(index) / 3.0) * 6.283185307179586;
        result += mip_sample_at_border(scene, filtering, uv, lod, vec2<f32>(cos(angle), sin(angle)) * radius_px,transparent);
    }
    return result / 3.0;
}

fn mip_sample_at(scene:texture_2d<f32>,filtering:sampler,uv:vec2<f32>,lod:f32,offset:vec2<f32>) -> vec4<f32> {
    return mip_sample_at_border(scene,filtering,uv,lod,offset,false);
}
fn mip_sample_blurred(scene:texture_2d<f32>,filtering:sampler,uv:vec2<f32>,radius:f32,min_lod:f32,noise:vec2<f32>) -> vec4<f32> {
    return mip_sample_blurred_border(scene,filtering,uv,radius,min_lod,noise,false);
}
