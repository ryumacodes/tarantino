//! Continuous caption corners, represented by a bounded radial table.
pub(super) fn profile(width: f64, height: f64, radius: f64) -> (f32, [f32; 16]) {
    let side = width.min(height);
    let r = radius.min(side / 2.0);
    if r <= 0.0 {
        return (0.0, [1.0; 16]);
    }
    let extent = (2.0 * r).min(side / 2.0);
    let turn = std::f64::consts::FRAC_PI_4 * (2.0 - 4.0 * r / side).clamp(0.0, 1.0);
    let tangent = r * (turn / 2.0).tan();
    let dx = tangent * turn.cos();
    let dy = tangent * turn.sin();
    let chord = r * std::f64::consts::SQRT_2 * (std::f64::consts::FRAC_PI_4 - turn).sin();
    let third = (extent - chord - dx - dy) / 3.0;
    let end_x = 3.0 * third + dx;
    let point = |t: f64| {
        (
            3.0 * (1.0 - t) * (1.0 - t) * t * (2.0 * third)
                + 3.0 * (1.0 - t) * t * t * (3.0 * third)
                + t * t * t * end_x,
            t * t * t * dy,
        )
    };
    let mut table = [1.0; 16];
    for (index, entry) in table.iter_mut().enumerate().skip(1) {
        let angle = index as f64 / 15.0 * std::f64::consts::FRAC_PI_4;
        let (sin, cos) = angle.sin_cos();
        let cross = |x: f64, y: f64| x * cos + (y - extent) * sin;
        let distance = if cross(end_x, dy) >= -1e-12 && third > 1e-12 {
            let (mut low, mut high) = (0.0, 1.0);
            for _ in 0..48 {
                let mid = (low + high) / 2.0;
                let (x, y) = point(mid);
                if cross(x, y) < 0.0 {
                    low = mid;
                } else {
                    high = mid;
                }
            }
            let (x, y) = point((low + high) / 2.0);
            x * sin + (extent - y) * cos
        } else {
            let (cx, cy) = (extent - r, r - extent);
            let projection = cx * sin - cy * cos;
            projection
                + (projection * projection - cx * cx - cy * cy + r * r)
                    .max(0.0)
                    .sqrt()
        };
        *entry = (distance / extent) as f32;
    }
    (extent as f32, table)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    #[ignore = "requires independently validated corner profile fixture"]
    fn matches_validated_corner_profiles() {
        let path = std::env::var("TARANTINO_CORNER_PROFILES").expect("Set profile fixture path");
        let cases: serde_json::Value =
            serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap();
        for case in cases.as_array().unwrap() {
            let (radius, table) = profile(
                case["width"].as_f64().unwrap(),
                case["height"].as_f64().unwrap(),
                case["inputRadius"].as_f64().unwrap(),
            );
            assert!((radius as f64 - case["radius"].as_f64().unwrap()).abs() < 1e-6);
            for (actual, expected) in table.iter().zip(case["curvature"].as_array().unwrap()) {
                assert!((*actual as f64 - expected.as_f64().unwrap()).abs() < 2e-6);
            }
        }
        println!(
            "Matched {} corner profiles",
            cases.as_array().unwrap().len()
        );
    }
}
