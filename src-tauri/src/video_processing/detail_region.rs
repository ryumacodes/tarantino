#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Plan {
    pub region: [u32; 4],
    pub scale: u32,
    pub output_size: [u32; 2],
    pub output_window: [f64; 4],
}

/// Plan an enhancement tile in decoded pixels, retaining its source UV window.
pub(crate) fn plan(
    decoded_size: [u32; 2],
    logical_size: [f64; 2],
    window: [f64; 4],
    device_scale: f64,
    device_origin: [f64; 2],
    visible: Option<[f64; 4]>,
) -> Option<Plan> {
    if device_scale <= 0.0 || !device_scale.is_finite()
        || decoded_size.contains(&0)
        || logical_size.iter().any(|v| !v.is_finite() || *v <= 0.0)
        || window.iter().any(|v| !v.is_finite())
        || window[2] <= 0.0 || window[3] <= 0.0
    {
        return None;
    }
    let mut region = [0, 0, decoded_size[0], decoded_size[1]];
    if let Some(visible) = visible {
        if visible.iter().chain(device_origin.iter()).any(|v| !v.is_finite()) {
            return None;
        }
        for axis in 0..2 {
            let to_pixel = |position: f64| {
                ((position - device_origin[axis]) / device_scale / logical_size[axis]
                    - window[axis]) / window[axis + 2] * decoded_size[axis] as f64
            };
            let start = (to_pixel(visible[axis]) + 0.0001).floor().max(0.0);
            let end = (to_pixel(visible[axis] + visible[axis + 2]) - 0.0001)
                .ceil().min(decoded_size[axis] as f64);
            if end <= start { return None; }
            region[axis] = start as u32;
            region[axis + 2] = (end - start) as u32;
        }
    }
    let fits = |scale: u32| {
        region[2].max(region[3]) as u64 * scale as u64 <= 8192
            && region[2] as u64 * region[3] as u64 * (scale * scale) as u64 <= 24_000_000
    };
    let projected_scale = device_scale * window[2] * logical_size[0] / decoded_size[0] as f64;
    let scale = if projected_scale >= 2.5 && fits(4) { 4 } else { 2 };
    if !fits(scale) { return None; }
    let mut output_window = window;
    for axis in 0..2 {
        output_window[axis] += region[axis] as f64 / decoded_size[axis] as f64 * window[axis + 2];
        output_window[axis + 2] = region[axis + 2] as f64 / decoded_size[axis] as f64 * window[axis + 2];
    }
    Some(Plan { region, scale, output_size: [region[2] * scale, region[3] * scale], output_window })
}

#[cfg(test)]
mod tests;
