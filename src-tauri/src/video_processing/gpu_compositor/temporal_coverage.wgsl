// Fit a signed-distance trajectory at shutter start, midpoint and end.
fn motion_fit_distance(start: f32, middle: f32, end: f32) -> vec3<f32> {
    return vec3<f32>(2.0*start-4.0*middle+2.0*end, -3.0*start+4.0*middle-end, start);
}

fn motion_coverage_ramp(distance: vec3<f32>, inset: f32, antialias: f32) -> vec3<f32> {
    let inverse_width = 1.0/max(2.0*antialias, 0.000001);
    return vec3<f32>(-distance.xy*inverse_width, 0.5-(distance.z-inset)*inverse_width);
}

fn motion_level_roots(p: vec3<f32>, level: f32, lo: f32, hi: f32) -> vec2<f32> {
    let a=p.x; let b=p.y; let c=p.z-level;
    if (abs(a)<0.0000001) {
        if (abs(b)<0.0000001) { return vec2<f32>(hi); }
        return vec2<f32>(clamp(-c/b,lo,hi),hi);
    }
    let discriminant=b*b-4.0*a*c;
    if (discriminant<0.0) { return vec2<f32>(hi); }
    // Stable quadratic roots avoid subtracting nearly equal numbers.
    let sign_b=select(-1.0,1.0,b>=0.0);
    let q=-0.5*(b+sign_b*sqrt(discriminant));
    if (abs(q)<0.0000001) { return vec2<f32>(clamp(-b/(2.0*a),lo,hi)); }
    return clamp(vec2<f32>(q/a,c/q),vec2<f32>(lo),vec2<f32>(hi));
}

// Exact integral of clamp(a*t*t+b*t+c,0,1), without subtracting large excess integrals.
fn motion_integrate_coverage(p: vec3<f32>, start: f32, end: f32) -> f32 {
    if (end<=start) { return 0.0; }
    let zero=motion_level_roots(p,0.0,start,end);
    let one=motion_level_roots(p,1.0,start,end);
    var edges=array<f32,6>(start,end,zero.x,zero.y,one.x,one.y);
    for (var i=1; i<6; i=i+1) {
        for (var j=i; j>0; j=j-1) {
            if (edges[j]>=edges[j-1]) { break; }
            let swap=edges[j]; edges[j]=edges[j-1]; edges[j-1]=swap;
        }
    }
    var area=0.0;
    for (var i=0; i<5; i=i+1) {
        let span=edges[i+1]-edges[i];
        let middle=(edges[i+1]+edges[i])*0.5;
        let value=(p.x*middle+p.y)*middle+p.z;
        if (value>=1.0) { area=area+span; }
        else if (value>0.0) {
            area=area+clamp(span*(value+p.x*span*span/12.0),0.0,span);
        }
    }
    return clamp(area,0.0,end-start);
}
