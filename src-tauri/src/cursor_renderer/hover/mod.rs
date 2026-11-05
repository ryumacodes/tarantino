//! Detect local visual changes that should stay synchronized with cursor motion.
#[cfg(target_os = "macos")]
pub mod analysis;
mod input;
mod pixels;
mod spans;
#[cfg(target_os = "macos")]
pub mod reader;
use anyhow::{Result, ensure};
pub use input::Sample;
pub use pixels::Frame;
use std::collections::VecDeque;

pub struct Detector {
    samples: Vec<Sample>,
    clicks: Vec<f64>,
    scale: f64,
    history: VecDeque<Frame>,
    recycled: Option<Frame>,
    last_change: f64,
}

impl Detector {
    pub fn new(samples: Vec<Sample>, mut clicks: Vec<f64>, pixels_per_point: f64) -> Result<Self> {
        ensure!(
            pixels_per_point.is_finite() && pixels_per_point > 0.0,
            "Invalid hover scale"
        );
        ensure!(
            samples
                .iter()
                .all(|s| s.time_ms.is_finite() && s.x.is_finite() && s.y.is_finite())
                && samples.windows(2).all(|p| p[0].time_ms <= p[1].time_ms),
            "Invalid hover cursor samples"
        );
        ensure!(
            clicks.iter().all(|t| t.is_finite()),
            "Invalid hover click times"
        );
        clicks.sort_by(f64::total_cmp);
        Ok(Self {
            samples,
            clicks,
            scale: pixels_per_point,
            history: VecDeque::new(),
            recycled: None,
            last_change: f64::NEG_INFINITY,
        })
    }

    pub fn push(&mut self, frame: Frame) -> Result<Option<f64>> {
        if let Some(previous) = self.history.back() {
            ensure!(
                frame.time_ms >= previous.time_ms
                    && frame.width == previous.width
                    && frame.height == previous.height,
                "Hover frames must preserve geometry and timestamp order"
            );
        }
        let changed = self.is_change(&frame);
        let time = frame.time_ms;
        if changed {
            self.last_change = time;
        }
        self.history.push_back(frame);
        // Keep the predecessor of the settled-history boundary for irregular frame times.
        while self.history.len() > 1 && self.history[1].time_ms <= time - 270.0 {
            self.recycled = self.history.pop_front();
        }
        Ok(changed.then_some(time))
    }

    pub fn take_recycled(&mut self) -> Option<Frame> {
        self.recycled.take()
    }

    fn is_change(&self, frame: &Frame) -> bool {
        let time = frame.time_ms;
        if time - self.last_change < 120.0 || input::free_movement(&self.samples, time) < 3.0 {
            return false;
        }
        let first_click = self.clicks.partition_point(|&click| click < time - 300.0);
        if self
            .clicks
            .get(first_click)
            .is_some_and(|&click| click <= time + 80.0)
        {
            return false;
        }
        let Some(center) = input::position(&self.samples, time) else {
            return false;
        };
        let center = center.map(|v| v * self.scale);
        let Some(reference) = self
            .history
            .iter()
            .rev()
            .find(|f| f.time_ms <= time - 120.0)
        else {
            return false;
        };
        let Some(settled) = self
            .history
            .iter()
            .rev()
            .find(|f| f.time_ms <= time - 270.0)
        else {
            return false;
        };
        pixels::compare(reference, frame, center, self.scale).is_highlight()
            && pixels::compare(settled, reference, center, self.scale).changed <= 0.08
            && !pixels::explained_by_scroll(reference, frame, center, self.scale)
    }
}

#[cfg(test)]
mod tests;

#[cfg(all(test, target_os = "macos"))]
mod reader_tests;
