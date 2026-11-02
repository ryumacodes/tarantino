fn compose_rim_light(content: vec4<f32>, light: vec3<f32>) -> vec4<f32> {
    let sheen = light*0.55;
    let lit = content.rgb*(vec3<f32>(1.0)+light*0.9-sheen)+sheen;
    let peak = max(lit.r,max(lit.g,lit.b));
    return vec4<f32>(lit/max(peak,1.0),min(content.a+max(sheen.r,max(sheen.g,sheen.b)),1.0));
}
fn highlight_arcs(facing: f32, width: f32, edge: f32, softness: f32) -> vec2<f32> {
    let start = 1.0-width;
    let feather = max(mix(width,0.02,edge)*(1.0+softness*0.5),0.01);
    return vec2<f32>(smoothstep(start,start+feather,max(facing,0.0)),
        0.28*smoothstep(start+width*0.25,start+feather+width*0.25,max(-facing,0.0)));
}
fn shade_rim(content: vec4<f32>, position: vec2<f32>, coverage: f32, pixel: vec2<u32>) -> vec4<f32> {
    let relative = position-lens.center;
    let distance = length(relative);
    if (lens.lighting <= 0.0 || lens.bevel <= 0.0 || coverage <= 0.0 || distance >= lens.radius || distance < 0.001*lens.source_pixel_scale) {
        return content;
    }
    let bevel_band = max(lens.bevel,lens.source_pixel_scale);
    let shine_band = max(lens.radius*0.036*pow(3.0,(0.5-0.9)*2.0),lens.source_pixel_scale);
    let inner_band_width = shine_band*50.0;
    if (distance < lens.radius-max(bevel_band,inner_band_width)) { return content; }
    let t = clamp((distance-lens.radius+bevel_band)/bevel_band,0.0,1.0);
    let bevel_presence = min(lens.bevel/lens.source_pixel_scale*0.5,1.0)*lens.lighting;
    let facing = dot(relative/distance,lens.light_direction);
    let edge_distance = lens.radius-distance;
    let core = pow(clamp(1.0-edge_distance/shine_band,0.0,1.0),2.5);
    let inner_u = clamp(1.0-edge_distance/inner_band_width,0.0,1.0);
    let inner = inner_u*inner_u*(3.0-2.0*inner_u)*0.1;
    let softness = clamp(4.0/max(lens.shine_exponent,1.0),0.0,1.0);
    let arcs = highlight_arcs(facing,0.3,0.25,softness)*core
        +highlight_arcs(facing,0.55,0.125,softness)*inner;
    let tint = vec3<f32>(1.0,0.98,0.95);
    let light = (tint*arcs.x+tint*vec3<f32>(0.9,0.96,1.05)*arcs.y+tint*0.05*(core+inner))
        *bevel_presence*coverage*lens.presence*1.05*(1.0+(ordered_threshold(pixel)-0.5)*0.8);
    let darken = 0.14*t*t*bevel_presence*coverage;
    return compose_rim_light(vec4<f32>(content.rgb*(1.0-darken),content.a),light);
}
