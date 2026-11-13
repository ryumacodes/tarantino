use super::detail_region;

#[derive(Clone, Copy)]
pub(crate) enum Quality { Playback, PausedLow, PausedHigh, Export }

pub(crate) struct DetailPolicy {
    pub quality: Quality,
    pub motion_pixels: f64,
    pub blur_strength: f64,
    pub projected_scale: f64,
    pub input_resolution: f64,
    pub magnifier_presence: f64,
    pub magnifier: bool,
}

fn quantize(value: f64) -> f64 {
    ((value * 16.0).round() / 16.0).clamp(0.0, 1.0)
}

pub(crate) fn slowest_motion(now: [f64; 4], before: [f64; 4]) -> f64 {
    let mut displacement = [0.0; 2];
    for axis in 0..2 {
        let near = now[axis] - before[axis];
        let far = now[axis] + now[axis + 2] - (before[axis] + before[axis + 2]);
        if near * far > 0.0 { displacement[axis] = near.abs().min(far.abs()); }
    }
    displacement[0].hypot(displacement[1])
}

impl DetailPolicy {
    pub fn strength(&self) -> f64 {
        if matches!(self.quality, Quality::Playback | Quality::PausedLow) { return 0.0; }
        let motion = quantize(1.0 - ((self.motion_pixels - 4.0) / 24.0).clamp(0.0, 1.0));
        let trail = ((self.motion_pixels * self.blur_strength - 4.0) / 4.0).clamp(0.0, 1.0);
        let blur = quantize(1.0 - trail * trail * (3.0 - 2.0 * trail));
        let oversampling = ((self.projected_scale / self.input_resolution.max(0.000001) - 1.02)
            / (1.2 - 1.02)).clamp(0.0, 1.0);
        let presence = if self.magnifier {
            ((self.magnifier_presence - 0.7) / (1.0 - 0.7)).clamp(0.0, 1.0)
        } else { (1.0 - self.magnifier_presence).clamp(0.0, 1.0) };
        quantize(motion * blur * oversampling * presence)
    }

    pub fn plan(&self, decoded: [u32; 2], logical: [f64; 2], window: [f64; 4],
        device_scale: f64, origin: [f64; 2], visible: Option<[f64; 4]>,
    ) -> Option<(f64, detail_region::Plan)> {
        let strength = self.strength();
        if strength < 0.2 { return None; }
        detail_region::plan(decoded, logical, window, device_scale, origin, visible)
            .map(|region| (strength, region))
    }
}

#[cfg(test)]
mod tests;
