//! Seekable range selection shared by magnifier preview and export.
use super::{target_at, LoupeOptions, LoupeTarget, SourceCrop};
use anyhow::{bail, Result};

#[derive(Clone, Copy, Debug)]
pub struct LoupeRange {
    pub start_ms: f64,
    pub end_ms: f64,
    pub disabled: bool,
    pub instant: bool,
    /// None denotes an ordinary whole-screen zoom, which can block early entry.
    pub options: Option<LoupeOptions>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Selection {
    Active { index: usize },
    Inactive { index: usize, boundary_ms: f64 },
    Empty,
}

pub struct LoupeTimeline<'a> {
    ranges: &'a [LoupeRange],
    starts: Vec<usize>,
    ends: Vec<usize>,
    loupe_starts: Vec<usize>,
    prefix_end: Vec<f64>,
    instant_starts: Vec<f64>,
    instant_end_prefix: Vec<f64>,
}

impl<'a> LoupeTimeline<'a> {
    pub fn new(ranges: &'a [LoupeRange]) -> Result<Self> {
        if ranges.iter().any(|range| !range.start_ms.is_finite() || !range.end_ms.is_finite() || range.start_ms < 0.0 || range.end_ms <= range.start_ms) {
            bail!("Magnifier ranges must have positive duration");
        }
        let mut starts: Vec<_> = (0..ranges.len()).collect();
        starts.sort_by(|&a,&b| ranges[a].start_ms.total_cmp(&ranges[b].start_ms).then(a.cmp(&b)));
        let mut ends = starts.clone();
        ends.sort_by(|&a,&b| ranges[a].end_ms.total_cmp(&ranges[b].end_ms).then(a.cmp(&b)));
        let loupe_starts = starts
            .iter()
            .copied()
            .filter(|&index| !ranges[index].disabled && ranges[index].options.is_some())
            .collect();
        let mut longest = 0.0_f64;
        let prefix_end = starts
            .iter()
            .map(|&index| {
                longest = longest.max(ranges[index].end_ms);
                longest
            })
            .collect();
        let instant_ranges: Vec<_> = starts
            .iter()
            .map(|&i| ranges[i])
            .filter(|range| range.instant && !range.disabled)
            .collect();
        let instant_starts = instant_ranges.iter().map(|range| range.start_ms).collect();
        let mut instant_end = 0.0_f64;
        let instant_end_prefix = instant_ranges
            .iter()
            .map(|range| {
                instant_end = instant_end.max(range.end_ms + 50.0);
                instant_end
            })
            .collect();
        Ok(Self {
            ranges,
            starts,
            ends,
            loupe_starts,
            prefix_end,
            instant_starts,
            instant_end_prefix,
        })
    }

    /// Snap through the short release window as well as the active range.
    pub fn snaps_at(&self, time_ms: f64) -> bool {
        if !time_ms.is_finite() || time_ms < 0.0 {
            return false;
        }
        let count = self
            .instant_starts
            .partition_point(|&start| start as f64 <= time_ms);
        count > 0 && self.instant_end_prefix[count - 1] as f64 >= time_ms
    }

    pub fn select(&self, time_ms: f64, lead_in_ms: u64) -> Selection {
        if !time_ms.is_finite() || time_ms < 0.0 {
            return Selection::Empty;
        }
        let count = self
            .starts
            .partition_point(|&i| self.ranges[i].start_ms <= time_ms);
        let mut active_loupe: Option<usize> = None;
        let mut active_other = false;
        // Prefix maxima let ordinary nonoverlapping timelines stop after one
        // interval, while overlapping imported ranges retain input precedence.
        for position in (0..count).rev() {
            if (self.prefix_end[position] as f64) < time_ms {
                break;
            }
            let index = self.starts[position];
            let range = self.ranges[index];
            if range.disabled || (range.end_ms) < time_ms {
                continue;
            }
            if range.options.is_some() {
                active_loupe = Some(active_loupe.map_or(index, |old| old.min(index)));
            } else {
                active_other = true;
            }
        }
        if let Some(index) = active_loupe {
            return Selection::Active { index };
        }
        if !active_other && lead_in_ms > 0 {
            let next = self
                .loupe_starts
                .partition_point(|&i| self.ranges[i].start_ms <= time_ms);
            if let Some(&index) = self.loupe_starts.get(next) {
                let range = self.ranges[index];
                if !range.instant && time_ms >= (range.start_ms - lead_in_ms as f64).max(0.0) {
                    return Selection::Active { index };
                }
            }
        }
        self.inactive_at(time_ms)
    }

    fn inactive_at(&self, time_ms: f64) -> Selection {
        let previous = self
            .ends
            .partition_point(|&i| (self.ranges[i].end_ms) < time_ms);
        let previous = previous.checked_sub(1).map(|i| self.ends[i]);
        let next = self
            .starts
            .partition_point(|&i| self.ranges[i].start_ms <= time_ms);
        let next = self.starts.get(next).copied();
        let chosen = match (previous, next) {
            (Some(a), Some(b)) => {
                if time_ms - self.ranges[a].end_ms
                    <= self.ranges[b].start_ms - time_ms
                {
                    Some((a, self.ranges[a].end_ms))
                } else {
                    Some((b, self.ranges[b].start_ms))
                }
            }
            (Some(a), None) => Some((a, self.ranges[a].end_ms)),
            (None, Some(b)) => Some((b, self.ranges[b].start_ms)),
            _ => None,
        };
        match chosen {
            Some((index, boundary_ms))
                if !self.ranges[index].disabled && self.ranges[index].options.is_some() =>
            {
                Selection::Inactive { index, boundary_ms }
            }
            _ => Selection::Empty,
        }
    }

    /// Resolve focus at the boundary for inactive stand-ins, not at the seek
    /// position. This keeps their geometry stable as the effect fades away.
    pub fn target(
        &self,
        source: (u32, u32),
        crop: SourceCrop,
        time_ms: f64,
        lead_in_ms: u64,
        mut focus_at: impl FnMut(usize, f64) -> (f64, f64),
    ) -> Result<Option<LoupeTarget>> {
        let (index, sample_time, inactive) = match self.select(time_ms, lead_in_ms) {
            Selection::Active { index } => (index, time_ms, false),
            Selection::Inactive { index, boundary_ms } => (index, boundary_ms, true),
            Selection::Empty => return Ok(None),
        };
        let target = target_at(
            source,
            crop,
            focus_at(index, sample_time),
            sample_time,
            self.ranges[index]
                .options
                .expect("selected range has magnifier options"),
        )?;
        Ok(Some(if inactive { target.inactive() } else { target }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn fractional_boundaries_preserve_selection_and_inactive_sampling() {
        let mut item=range(100,200);
        item.start_ms=100.125;item.end_ms=200.875;
        let ranges=[item];
        let timeline=LoupeTimeline::new(&ranges).unwrap();
        assert_eq!(timeline.select(100.0,0),Selection::Inactive{index:0,boundary_ms:100.125});
        assert_eq!(timeline.select(100.125,0),Selection::Active{index:0});
        assert_eq!(timeline.select(200.875,0),Selection::Active{index:0});
        assert_eq!(timeline.select(200.876,0),Selection::Inactive{index:0,boundary_ms:200.875});
    }
    fn range(start_ms: u64, end_ms: u64) -> LoupeRange {
        LoupeRange {
            start_ms: start_ms as f64,
            end_ms: end_ms as f64,
            disabled: false,
            instant: false,
            options: Some(LoupeOptions {
                radius: 0.25,
                zoom: 2.0,
                glass: true,
                bevel: 0.1,
                chromatic_aberration: 0.01,
                shadow_alpha: 0.35,
            }),
        }
    }

    #[test]
    fn early_entry_is_blocked_by_classic_zoom_but_not_disabled_zoom() {
        let mut ranges = [
            LoupeRange {
                options: None,
                ..range(100, 900)
            },
            range(1000, 2000),
        ];
        let timeline = LoupeTimeline::new(&ranges).unwrap();
        assert!(!matches!(
            timeline.select(800.0, 500),
            Selection::Active { .. }
        ));
        assert_eq!(timeline.select(950.0, 500), Selection::Active { index: 1 });
        ranges[0].disabled = true;
        assert_eq!(
            LoupeTimeline::new(&ranges).unwrap().select(800.0, 500),
            Selection::Active { index: 1 }
        );
        ranges[1].instant = true;
        let timeline = LoupeTimeline::new(&ranges).unwrap();
        assert!(!matches!(
            timeline.select(950.0, 500),
            Selection::Active { .. }
        ));
        assert_eq!(timeline.select(1000.0, 500), Selection::Active { index: 1 });
    }

    #[test]
    fn nearest_boundary_includes_disabled_and_classic_ranges_and_previous_wins_ties() {
        let mut ranges = [range(100, 200), range(400, 500)];
        assert_eq!(
            LoupeTimeline::new(&ranges).unwrap().select(300.0, 0),
            Selection::Inactive {
                index: 0,
                boundary_ms: 200.0
            }
        );
        ranges[0].disabled = true;
        assert_eq!(
            LoupeTimeline::new(&ranges).unwrap().select(300.0, 0),
            Selection::Empty
        );
        ranges[0].disabled = false;
        ranges[1].options = None;
        assert_eq!(
            LoupeTimeline::new(&ranges).unwrap().select(350.0, 0),
            Selection::Empty
        );
    }

    #[test]
    fn arbitrary_seek_order_is_stable_and_inactive_focus_uses_boundary_time() {
        let ranges = [range(1000, 2000), range(3000, 4000)];
        let timeline = LoupeTimeline::new(&ranges).unwrap();
        let crop = SourceCrop {
            x: 0.0,
            y: 0.0,
            width: 1.0,
            height: 1.0,
        };
        for time in [2500.0, 50.0, 3500.0, 2500.0] {
            let selection = timeline.select(time, 0);
            let expected = match selection {
                Selection::Active { .. } => time,
                Selection::Inactive { boundary_ms, .. } => boundary_ms,
                _ => panic!("expected a magnifier target"),
            };
            let target = timeline
                .target((1000, 500), crop, time, 0, |_, sampled| {
                    assert_eq!(sampled, expected);
                    (0.3, 0.7)
                })
                .unwrap()
                .unwrap();
            assert_eq!(
                target.presence,
                if matches!(selection, Selection::Active { .. }) {
                    1.0
                } else {
                    0.0
                }
            );
        }
    }

    #[test]
    fn unsorted_overlapping_ranges_keep_input_precedence_and_invalid_time_is_empty() {
        let ranges = [range(500, 1000), range(0, 2000)];
        let timeline = LoupeTimeline::new(&ranges).unwrap();
        assert_eq!(timeline.select(750.0, 0), Selection::Active { index: 0 });
        assert_eq!(timeline.select(1001.0, 0), Selection::Active { index: 1 });
        for time in [f64::NAN, f64::INFINITY, -1.0] {
            assert_eq!(timeline.select(time, 500), Selection::Empty);
        }
        assert!(LoupeTimeline::new(&[range(100, 100)]).is_err());
    }
}
