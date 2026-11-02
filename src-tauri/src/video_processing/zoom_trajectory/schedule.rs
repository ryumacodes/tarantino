//! Sweep source time once instead of scanning every range at every frame.
use super::ZoomBlock;
use std::{
    cmp::Reverse,
    collections::{BTreeSet, BinaryHeap},
};

pub(super) struct ZoomSchedule<'a> {
    blocks: &'a [ZoomBlock],
    starts: Vec<usize>,
    screen_starts: Vec<usize>,
    next: usize,
    active: BTreeSet<usize>,
    ends: BinaryHeap<Reverse<(u64, usize)>>,
    instant_ends: BinaryHeap<Reverse<u64>>,
}
impl<'a> ZoomSchedule<'a> {
    pub fn new(blocks: &'a [ZoomBlock]) -> Self {
        let mut starts: Vec<_> = (0..blocks.len()).filter(|&i| !blocks[i].is_disabled).collect();
        starts.sort_by_key(|&i| (blocks[i].start_time_ms, i));
        let screen_starts = starts.iter().copied().filter(|&i| blocks[i].loupe.is_none()).collect();
        Self {
            blocks,
            starts,
            screen_starts,
            next: 0,
            active: BTreeSet::new(),
            ends: BinaryHeap::new(),
            instant_ends: BinaryHeap::new(),
        }
    }

    /// Calls must use nondecreasing time, as the fixed-step simulator does.
    pub fn advance(&mut self, time: f64) -> (Option<usize>, bool) {
        while let Some(&index) = self.starts.get(self.next) {
            let block = &self.blocks[index];
            if block.start_time_ms as f64 > time {
                break;
            }
            self.active.insert(index);
            self.ends.push(Reverse((block.end_time_ms, index)));
            if block.instant_animation {
                self.instant_ends
                    .push(Reverse(block.end_time_ms.saturating_add(50)));
            }
            self.next += 1;
        }
        while self.ends.peek().is_some_and(|end| (end.0.0 as f64) < time) {
            let Reverse((_, index)) = self.ends.pop().unwrap();
            self.active.remove(&index);
        }
        while self
            .instant_ends
            .peek()
            .is_some_and(|end| (end.0 as f64) < time)
        {
            self.instant_ends.pop();
        }
        // Preserve input-order precedence for overlapping ranges.
        let selected = self.active.iter().copied().find(|&i| self.blocks[i].loupe.is_none()).or_else(|| {
            // An active magnifier prevents anticipating a screen zoom, but
            // does not suppress an already active screen range.
            if !self.active.is_empty() { return None; }
            let next_screen = self.screen_starts.partition_point(|&i| self.blocks[i].start_time_ms as f64 <= time);
            let index = *self.screen_starts.get(next_screen)?;
            let block = &self.blocks[index];
            (!block.instant_animation
                && time
                    >= block
                        .start_time_ms
                        .saturating_sub(block.animation_lead_in_ms.min(2000))
                        as f64)
                .then_some(index)
        });
        (selected, !self.instant_ends.is_empty())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn sweep_matches_scan_for_overlaps_ties_gaps_and_instant_guards() {
        let mut blocks: Vec<ZoomBlock> = (0..137)
            .map(|i| {
                let start = (i * 173) % 3000;
                serde_json::from_value(serde_json::json!({
                    "start_time_ms": start, "end_time_ms": start + (i * 71) % 500,
                    "zoom_level": 2, "center_x": 0.5, "center_y": 0.5,
                    "instant_animation": i % 4 == 0, "animation_lead_in_ms": (i * 41) % 2200,
                    "is_disabled": i % 11 == 0,
                    "loupe": if i % 3 == 0 { serde_json::json!({"radius":0.2,"glass":false,
                        "bevel":0,"chromatic_aberration":0,"shadow_alpha":0}) } else { serde_json::Value::Null }
                }))
                .unwrap()
            })
            .collect();
        blocks.reverse();
        blocks.push(blocks[0].clone());
        let mut times: Vec<f64> = (0..500).map(|i| i as f64 * 1000.0 / 120.0).collect();
        for block in &blocks {
            for boundary in [
                block.start_time_ms,
                block.end_time_ms,
                block.end_time_ms + 50,
            ] {
                times.extend([boundary as f64, boundary as f64 + 0.001]);
            }
        }
        times.sort_by(f64::total_cmp);
        let mut schedule = ZoomSchedule::new(&blocks);
        for time in times {
            let scan = blocks
                .iter()
                .enumerate()
                .find(|(_, b)| !b.is_disabled && b.loupe.is_none()
                    && time >= b.start_time_ms as f64 && time <= b.end_time_ms as f64)
                .or_else(|| {
                    if blocks.iter().any(|b| !b.is_disabled && time >= b.start_time_ms as f64
                        && time <= b.end_time_ms as f64) { return None; }
                    blocks
                        .iter()
                        .enumerate()
                        .filter(|(_, b)| !b.is_disabled && b.loupe.is_none() && b.start_time_ms as f64 > time)
                        .min_by_key(|(_, b)| b.start_time_ms)
                        .filter(|(_, b)| {
                            !b.instant_animation
                                && time
                                    >= b.start_time_ms
                                        .saturating_sub(b.animation_lead_in_ms.min(2000))
                                        as f64
                        })
                })
                .map(|(index, _)| index);
            let instant = blocks.iter().any(|b| {
                !b.is_disabled && b.instant_animation
                    && time >= b.start_time_ms as f64
                    && time <= b.end_time_ms.saturating_add(50) as f64
            });
            assert_eq!(schedule.advance(time), (scan, instant), "time={time}");
        }
        assert_eq!(ZoomSchedule::new(&[]).advance(0.0), (None, false));
    }
}
