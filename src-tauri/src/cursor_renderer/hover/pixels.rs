use anyhow::{Result, ensure};

pub struct Frame {
    pub time_ms: f64,
    pub width: usize,
    pub height: usize,
    y: Vec<u8>,
    uv: Vec<u8>,
}

impl Frame {
    pub fn new(time_ms: f64, width: usize, height: usize, y: Vec<u8>, uv: Vec<u8>) -> Result<Self> {
        ensure!(
            time_ms.is_finite() && width > 0 && height > 0,
            "Invalid hover frame geometry or time"
        );
        ensure!(
            width.checked_mul(height) == Some(y.len()),
            "Invalid hover luma plane"
        );
        ensure!(
            width
                .div_ceil(2)
                .checked_mul(height.div_ceil(2))
                .and_then(|n| n.checked_mul(2))
                == Some(uv.len()),
            "Invalid hover chroma plane"
        );
        Ok(Self {
            time_ms,
            width,
            height,
            y,
            uv,
        })
    }

    fn pixel(&self, x: usize, y: usize) -> [i32; 3] {
        let chroma = (y / 2 * self.width.div_ceil(2) + x / 2) * 2;
        [
            self.y[y * self.width + x] as i32,
            self.uv[chroma] as i32,
            self.uv[chroma + 1] as i32,
        ]
    }
}

pub struct Comparison {
    pub changed: f64,
    coherent: f64,
    quarters: [f64; 4],
}

impl Comparison {
    pub fn is_highlight(&self) -> bool {
        self.changed >= 0.4
            && self.coherent >= 0.3
            && self
                .quarters
                .iter()
                .filter(|&&fraction| fraction >= 0.15)
                .count()
                >= 3
    }
}

pub fn compare(before: &Frame, after: &Frame, center: [f64; 2], scale: f64) -> Comparison {
    let radius = (8.0 * scale).round().max(2.0) as isize;
    let [cx, cy] = center.map(|v| v.round() as isize);
    let left = (cx - radius).max(0);
    let right = (cx + radius).min(after.width as isize - 1);
    let top = (cy - radius).max(0);
    let bottom = (cy + radius).min(after.height as isize - 1);
    let mut totals = [0usize; 4];
    let mut changed = Vec::new();
    for y in top..=bottom {
        for x in left..=right {
            let quarter = usize::from(x >= cx) + 2 * usize::from(y >= cy);
            totals[quarter] += 1;
            let a = before.pixel(x as usize, y as usize);
            let b = after.pixel(x as usize, y as usize);
            let delta = std::array::from_fn::<_, 3, _>(|i| b[i] - a[i]);
            if delta.iter().map(|v| v.abs()).sum::<i32>() >= 10 {
                changed.push((delta, quarter));
            }
        }
    }
    if changed.is_empty() {
        return Comparison {
            changed: 0.0,
            coherent: 0.0,
            quarters: [0.0; 4],
        };
    }
    let median = std::array::from_fn::<_, 3, _>(|channel| {
        let mut values: Vec<_> = changed.iter().map(|(d, _)| d[channel]).collect();
        let middle = values.len() / 2;
        *values.select_nth_unstable(middle).1
    });
    let tolerance = (0.35 * median.iter().map(|v| v.abs()).sum::<i32>() as f64)
        .round()
        .max(6.0) as i32;
    let mut coherent = [0usize; 4];
    for (delta, quarter) in &changed {
        if (0..3).map(|i| (delta[i] - median[i]).abs()).sum::<i32>() <= tolerance {
            coherent[*quarter] += 1;
        }
    }
    let total = totals.iter().sum::<usize>() as f64;
    Comparison {
        changed: changed.len() as f64 / total,
        coherent: coherent.iter().sum::<usize>() as f64 / total,
        quarters: std::array::from_fn(|q| {
            if totals[q] == 0 {
                0.0
            } else {
                coherent[q] as f64 / totals[q] as f64
            }
        }),
    }
}

pub fn explained_by_scroll(before: &Frame, after: &Frame, center: [f64; 2], scale: f64) -> bool {
    let radius = (24.0 * scale).round() as isize;
    let shift = (32.0 * scale).round() as isize;
    let step = scale.round().max(1.0) as usize;
    let [cx, cy] = center.map(|v| v.round() as isize);
    let left = (cx - radius).max(shift);
    let right = (cx + radius).min(after.width as isize - 1 - shift);
    let top = (cy - radius).max(shift);
    let bottom = (cy + radius).min(after.height as isize - 1 - shift);
    if left >= right || top >= bottom {
        return false;
    }
    let residual = |dx: isize, dy: isize| {
        let mut sum = 0u64;
        for y in (top..=bottom).step_by(step) {
            for x in (left..=right).step_by(step) {
                let a = before.y[(y - dy) as usize * before.width + (x - dx) as usize];
                let b = after.y[y as usize * after.width + x as usize];
                sum += a.abs_diff(b) as u64;
            }
        }
        sum
    };
    let baseline = residual(0, 0);
    if baseline == 0 {
        return false;
    }
    (-shift..shift)
        .step_by(step)
        .chain(std::iter::once(shift))
        .filter(|&offset| offset != 0)
        .any(|offset| {
            (residual(offset, 0) as f64) < baseline as f64 * 0.5
                || (residual(0, offset) as f64) < baseline as f64 * 0.5
        })
}
