// Broad colored light pools use smooth spectral modulation rather than
// displaced texture reads; the underlying content stays spatially unchanged.
fn pool_light(t: f32) -> vec3<f32> {
    let spectral = clamp(t,0.0,1.0);
    let hue = (1.0-pow(spectral,0.6))*6.283185307179586;
    let ramp = smoothstep(0.0,0.4,spectral);
    let amplitude = 0.5*0.35*ramp;
    let spectrum = vec3<f32>(cos(hue),cos(hue-2.0943951),cos(hue-4.1887902));
    let warmth = vec3<f32>(1.0)+0.125*vec3<f32>(0.878,-0.023,-0.851);
    return (vec3<f32>(1.0)+amplitude*spectrum)*warmth*mix(0.5,1.0,ramp);
}
fn shade_pool(content: vec4<f32>, position: vec2<f32>, coverage: f32) -> vec4<f32> {
    let relative = position-lens.center;
    let pool_presence = min(lens.bevel/lens.source_pixel_scale*0.5,1.0)*lens.presence*lens.lighting;
    var pool = vec3<f32>(0.0);
    if (pool_presence > 0.001) {
        if (coverage > 0.001) {
            let inner_u = clamp((length(relative-lens.light_direction*lens.radius)-lens.radius)/lens.radius,0.0,1.0);
            pool += pool_light(inner_u)*inner_u*inner_u*0.25*pool_presence*coverage;
        }
        if (coverage < 1.0) {
            let shift = min(lens.radius*0.5,max(lens.shadow_reach,lens.source_pixel_scale)*0.45);
            let outer_u = clamp((lens.radius-length(relative+lens.light_direction*shift))/shift,0.0,1.0);
            pool += pool_light(1.0-outer_u)*outer_u*outer_u*0.3*pool_presence*(1.0-coverage);
        }
    }
    let shadow = outer_shadow(position)*(1.0-coverage);
    let light = pool*vec3<f32>(1.0,0.98,0.95);
    let sheen = light*0.35;
    let lit = content.rgb*(vec3<f32>(1.0-shadow)+light-sheen)+sheen;
    let peak = max(lit.r,max(lit.g,lit.b));
    return vec4<f32>(lit/max(peak,1.0),min(content.a+(1.0-content.a)*shadow+max(sheen.r,max(sheen.g,sheen.b)),1.0));
}
