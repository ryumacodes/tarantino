struct Lens {
    center: vec2<f32>,
    radius: f32,
    zoom: f32,
    presence: f32,
    bevel: f32,
    shadow_alpha: f32,
    shadow_reach: f32,
    light_direction: vec2<f32>,
    light_tilt: f32,
    aberration: f32,
    reflection: f32,
    source_pixel_scale: f32,
    lighting: f32,
    shine_exponent: f32,
    separate_layers: f32,
    pad0: f32,
    pad1: f32,
    pad2: f32,
};
@group(0) @binding(0) var source: texture_2d<f32>;
@group(0) @binding(1) var linear_sampler: sampler;
@group(0) @binding(2) var output: texture_storage_2d<rgba8unorm, write>;
@group(0) @binding(3) var<uniform> lens: Lens;
@group(0) @binding(4) var filtered_scene: texture_2d<f32>;
@group(0) @binding(5) var foreground: texture_2d<f32>;
@group(0) @binding(6) var background: texture_2d<f32>;

fn ordered_threshold(pixel: vec2<u32>) -> f32 {
    var value = 0u;
    for (var bit = 0u; bit < 3u; bit += 1u) {
        let x = (pixel.x >> bit) & 1u;
        let y = (pixel.y >> bit) & 1u;
        value += ((x ^ y)*2u+y) << (4u-2u*bit);
    }
    return f32(value)/64.0;
}

fn spectrum_integral(value: f32) -> f32 {
    let x = clamp(value,-1.0,1.0);
    let squared = x*x;
    return x*(1.0-squared*(2.0/3.0)+squared*squared*0.2);
}
fn band_integral(start: f32, end: f32, center: f32, width: f32) -> f32 {
    return width*(spectrum_integral((end-center)/width)-spectrum_integral((start-center)/width));
}
fn refracted_position(position: vec2<f32>) -> vec2<f32> {
    let relative = position-lens.center;
    let distance = length(relative);
    var sampled = lens.center+relative/lens.zoom;
    let band = max(lens.bevel,lens.source_pixel_scale);
    if (lens.bevel > 0.0 && distance > 0.001*lens.source_pixel_scale && distance < lens.radius && distance > lens.radius-band) {
        let t = clamp((distance-lens.radius+band)/band,0.0,1.0);
        let squared = t*t;
        let slope = min(squared*inverseSqrt(1.0-min(squared,0.99)),6.0);
        sampled -= relative/distance*slope*(1.0-1.0/1.5)*lens.bevel/lens.zoom;
    }
    return sampled;
}
fn full_refracted_position(position: vec2<f32>) -> vec2<f32> {
    let ordinary = lens.center+(position-lens.center)/lens.zoom;
    return ordinary-(ordinary-refracted_position(position))*lens.zoom;
}
fn rim_blur_radius(position: vec2<f32>, sampled: vec2<f32>) -> f32 {
    let relative = position-lens.center;
    let distance = length(relative);
    if (lens.bevel <= 0.0 || distance >= lens.radius) { return 0.0; }
    var scale = 1.0/max(lens.zoom,0.05);
    if (distance >= lens.source_pixel_scale && distance > lens.radius-max(lens.bevel,lens.source_pixel_scale)) {
        let full_sample = full_refracted_position(position);
        let inward = full_refracted_position(position-relative/distance*lens.source_pixel_scale);
        let radial = length(full_sample-inward)/lens.source_pixel_scale;
        let tangential = length(full_sample-lens.center)/distance;
        scale = max(radial,tangential);
    }
    let bias = clamp(log2(max(scale,0.01)),-2.0,4.0);
    let lod = clamp(bias+log2(max(lens.zoom,0.05)),0.0,4.0);
    if (lod < 0.05) { return 0.0; }
    // The input stage precedes magnification; inverse-map the filter radius.
    return 0.58*(exp2(lod)-1.0)/lens.zoom;
}
fn filtered_read(position: vec2<f32>, radius: f32, pixel: vec2<u32>) -> vec4<f32> {
    let uv = position/vec2<f32>(textureDimensions(source));
    if (radius <= 0.0) { return textureSampleLevel(foreground,linear_sampler,uv,0.0); }
    return mip_sample_blurred(filtered_scene,linear_sampler,uv,radius,0.0,
        vec2<f32>(blue_noise(pixel),blue_noise(pixel+vec2<u32>(37u))));
}

fn scene_read(position: vec2<f32>, backdrop_position: vec2<f32>, radius: f32, pixel: vec2<u32>) -> vec4<f32> {
    let front = filtered_read(position,radius,pixel);
    if (lens.separate_layers <= 0.0 || front.a >= 1.0) { return front; }
    let back = textureSampleLevel(background,linear_sampler,backdrop_position/vec2<f32>(textureDimensions(background)),0.0);
    return front+back*(1.0-front.a);
}
fn spectral_sample(position: vec2<f32>, backdrop_position: vec2<f32>, split: vec2<f32>, split_pixels: f32, pixel: vec2<u32>, blur_radius: f32) -> vec4<f32> {
    let size = vec2<f32>(textureDimensions(source));
    if (split_pixels < 0.75) {
        return scene_read(position,backdrop_position,blur_radius,pixel);
    }
    let reads = u32(clamp(ceil(split_pixels),3.0,16.0));
    let interval = 2.0/f32(reads);
    var color = vec3<f32>(0.0);
    var weight_sum = vec3<f32>(0.0);
    var alpha = 0.0;
    for (var index = 0u; index < reads; index += 1u) {
        let end = 1.0-f32(index)*interval;
        let start = end-interval;
        let phase = fract(ordered_threshold(pixel)+f32(index)*0.61803398875);
        let sample_position = position+split*(end-phase*interval);
        let sample_color = scene_read(sample_position,backdrop_position+split*lens.zoom*(end-phase*interval),blur_radius,pixel);
        let rgb = clamp(sample_color.rgb,vec3<f32>(0.0),vec3<f32>(sample_color.a));
        let linear = rgb*rgb/max(sample_color.a,0.00001);
        let weights = vec3<f32>(
            band_integral(start,end,0.85,0.7)+0.3*band_integral(start,end,-0.95,0.35),
            band_integral(start,end,0.05,0.65),band_integral(start,end,-0.75,0.65));
        color += linear*weights;
        weight_sum += weights;
        alpha += sample_color.a;
    }
    alpha /= f32(reads);
    return vec4<f32>(sqrt(max(color/weight_sum*alpha,vec3<f32>(0.0))),alpha);
}

fn outer_shadow(position: vec2<f32>) -> f32 {
    if (lens.shadow_alpha <= 0.001) { return 0.0; }
    let relative = position-lens.center;
    let reach = max(lens.shadow_reach,lens.source_pixel_scale);
    let hover_height = min(reach,lens.radius*0.25);
    let throw_distance = min(lens.light_tilt*hover_height,reach*0.45);
    let distance = length(relative+lens.light_direction*throw_distance);
    let effective_reach = max(reach-throw_distance,reach*0.4);
    let falloff = clamp(1.0-(distance-lens.radius)/effective_reach,0.0,1.0);
    let facing = dot(relative,lens.light_direction)/max(length(relative),0.001*lens.source_pixel_scale);
    let side_strength = mix(1.0,0.6,(facing+1.0)*0.5);
    return lens.shadow_alpha*side_strength*falloff*falloff;
}

fn rim_reflection(position: vec2<f32>) -> vec3<f32> {
    let strength = lens.reflection*min(lens.bevel/lens.source_pixel_scale*0.5,1.0)*lens.presence;
    let relative = position-lens.center;
    let distance = length(relative);
    let band = max(min(lens.bevel*1.5,lens.radius),lens.source_pixel_scale);
    if (strength <= 0.001 || distance >= lens.radius || distance < 0.001*lens.source_pixel_scale || distance <= lens.radius-band) {
        return vec3<f32>(0.0);
    }
    let t = clamp((distance-lens.radius+band)/band,0.0,1.0);
    let squared = t*t;
    let slope = min(squared*inverseSqrt(1.0-min(squared,0.99)),6.0);
    let cosine = inverseSqrt(1.0+slope*slope);
    let fresnel = 0.04+0.96*pow(1.0-cosine,5.0);
    let weight = clamp(fresnel*smoothstep(1.0,1.5,slope)*strength,0.0,1.0);
    if (weight <= 0.004) { return vec3<f32>(0.0); }
    let outward = min(lens.radius*0.5*slope/max(slope*slope-1.0,0.05),lens.radius);
    let read_radius = max(distance+outward,lens.radius+lens.source_pixel_scale);
    return vec3<f32>(lens.center+relative/distance*read_radius,weight);
}

fn write_output(pixel: vec2<i32>, color: vec4<f32>) {
    if (lens.separate_layers > 1.5) {
        textureStore(output,pixel,vec4<f32>(select(vec3<f32>(0.0),color.rgb/max(color.a,0.00001),color.a>0.0),color.a));
    } else { textureStore(output,pixel,color); }
}

@compute @workgroup_size(8,8)
fn main(@builtin(global_invocation_id) id: vec3<u32>) {
    let size = textureDimensions(output);
    if (id.x >= size.x || id.y >= size.y) { return; }
    let pixel = vec2<i32>(id.xy);
    let base = textureLoad(source,pixel,0);
    if (lens.presence <= 0.0 || lens.radius <= 0.0) {
        write_output(pixel,base);
        return;
    }
    let position = vec2<f32>(id.xy) + vec2<f32>(0.5);
    let distance = length(position - lens.center);
    let coverage = (1.0 - smoothstep(lens.radius - 0.5,lens.radius + 0.5,distance)) * lens.presence;
    if (coverage <= 0.0) {
        if (lens.lighting > 0.0 && lens.bevel > 0.0) {
            write_output(pixel,shade_pool(base,position,0.0));
            return;
        }
        let shadow = outer_shadow(position);
        let noise = ordered_threshold(id.xy+vec2<u32>(4u))/255.0*min(shadow*64.0,1.0);
        let opacity = clamp(shadow+(ordered_threshold(id.xy)-0.5)*(1.5/255.0)*shadow,0.0,1.0);
        write_output(pixel,vec4<f32>(base.rgb*(1.0-opacity)+vec3<f32>(noise),base.a));
        return;
    }
    let sampled_position = refracted_position(position);
    let ordinary = lens.center+(position-lens.center)/lens.zoom;
    let split = (ordinary-sampled_position)*lens.zoom*0.35*lens.aberration;
    let blur_radius = rim_blur_radius(position,sampled_position);
    var magnified = spectral_sample(sampled_position,full_refracted_position(position),split/lens.zoom,length(split),id.xy,blur_radius);
    let reflection = rim_reflection(position);
    if (reflection.z > 0.0) {
        let plain = textureSampleLevel(source,linear_sampler,reflection.xy/vec2<f32>(size),0.0);
        let reflected = vec4<f32>(plain.rgb*(1.0-outer_shadow(reflection.xy)),plain.a);
        magnified = mix(magnified,reflected,reflection.z);
    }
    var composed = mix(base,magnified,coverage);
    if (lens.lighting > 0.0 && lens.bevel > 0.0) { composed = shade_pool(composed,position,coverage); }
    write_output(pixel,shade_rim(composed,position,coverage,id.xy));
}
