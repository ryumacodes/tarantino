#[derive(Clone, Copy, serde::Deserialize)]
pub struct Sample {
    pub time_ms: f64,
    pub x: f64,
    pub y: f64,
    pub free: bool,
}

pub fn position(samples: &[Sample], time: f64) -> Option<[f64; 2]> {
    let next = samples.partition_point(|sample| sample.time_ms <= time);
    let a = samples.get(next.checked_sub(1)?)?;
    let Some(b) = samples.get(next) else {
        return Some([a.x, a.y]);
    };
    let amount = (time - a.time_ms) / (b.time_ms - a.time_ms).max(0.001);
    Some([a.x + (b.x - a.x) * amount, a.y + (b.y - a.y) * amount])
}

pub fn free_movement(samples: &[Sample], time: f64) -> f64 {
    let first = samples.partition_point(|sample| sample.time_ms <= time - 200.0);
    let end = samples.partition_point(|sample| sample.time_ms <= time);
    samples[first..end]
        .windows(2)
        .filter(|pair| pair[1].free && pair[1].time_ms > time - 150.0)
        .map(|pair| (pair[1].x - pair[0].x).hypot(pair[1].y - pair[0].y))
        .sum()
}
