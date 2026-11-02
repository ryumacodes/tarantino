//! Motion/rest windows and synchronization groups for candidate evaluation.
use super::Point;
use anyhow::{bail, Result};

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Interval {
    pub start: f64,
    pub end: f64,
}

pub struct MotionIntervals {
    moving: Vec<Interval>,
}
impl MotionIntervals {
    pub fn new(points: &[Point]) -> Self {
        let mut moving: Vec<Interval> = Vec::new();
        for pair in points.windows(2) {
            let (a, b) = (pair[0], pair[1]);
            if a.x == b.x && a.y == b.y {
                continue;
            }
            // A discontinuous jump occupies a positive interval so the
            // optimizer cannot treat that exact instant as a resting point.
            let start = if b.time_ms - a.time_ms <= 40.0 {
                a.time_ms
            } else {
                b.time_ms
            };
            if let Some(last) = moving.last_mut().filter(|r| r.end >= start) {
                last.end = last.end.max(b.time_ms);
            } else {
                moving.push(Interval {
                    start,
                    end: b.time_ms.max(start + 0.000001),
                });
            }
        }
        Self { moving }
    }

    pub fn moving_at(&self, time: f64) -> bool {
        let count = self.moving.partition_point(|r| r.start <= time);
        count > 0 && self.moving[count - 1].end > time
    }

    pub fn still_between(&self, a: f64, b: f64) -> bool {
        let (start, end) = (a.min(b), a.max(b));
        let count = self.moving.partition_point(|r| r.start <= end);
        count == 0 || self.moving[count - 1].end <= start
    }

    pub fn rest_at(&self, time: f64) -> Option<Interval> {
        if self.moving_at(time) {
            return None;
        }
        let count = self.moving.partition_point(|r| r.start <= time);
        Some(Interval {
            start: count
                .checked_sub(1)
                .map_or(f64::NEG_INFINITY, |i| self.moving[i].end),
            end: self.moving.get(count).map_or(f64::INFINITY, |r| r.start),
        })
    }

    pub fn last_rest_ending_by(&self, time: f64) -> Option<Interval> {
        let count = self.moving.partition_point(|r| r.start <= time);
        let index = count.checked_sub(1)?;
        Some(Interval {
            start: index
                .checked_sub(1)
                .map_or(f64::NEG_INFINITY, |i| self.moving[i].end),
            end: self.moving[index].start,
        })
    }
}

#[derive(Clone, Copy, Debug, serde::Deserialize, PartialEq)]
pub struct SyncPoint {
    pub time_ms: f64,
    pub tolerance: f64,
}

#[derive(Clone, Debug)]
pub struct SyncGroup {
    pub start: f64,
    pub end: f64,
    pub points: Vec<SyncPoint>,
}
impl SyncGroup {
    pub fn merge(self, next: Self) -> Self {
        let mut points = self.points;
        points.extend(next.points);
        Self {
            start: self.start,
            end: next.end,
            points,
        }
    }
}

/// Keep all constraints (including repeated-time click/drag points); a group
/// extends while each consecutive constraint is within the allowed gap.
pub fn group_sync_points(
    mut points: Vec<SyncPoint>,
    gap: f64,
    instant: &[(f64, f64)],
) -> Result<Vec<SyncGroup>> {
    if !gap.is_finite()
        || gap < 0.0
        || points
            .iter()
            .any(|p| !p.time_ms.is_finite() || !p.tolerance.is_finite() || p.tolerance <= 0.0)
    {
        bail!("Invalid cursor synchronization constraints");
    }
    points.retain(|p| {
        !instant
            .iter()
            .any(|&(a, b)| p.time_ms >= a && p.time_ms < b)
    });
    points.sort_by(|a, b| a.time_ms.total_cmp(&b.time_ms));
    let mut groups: Vec<SyncGroup> = Vec::new();
    for point in points {
        if let Some(group) = groups.last_mut().filter(|g| point.time_ms - g.end <= gap) {
            group.end = point.time_ms;
            group.points.push(point);
        } else {
            groups.push(SyncGroup {
                start: point.time_ms,
                end: point.time_ms,
                points: vec![point],
            });
        }
    }
    Ok(groups)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn grouping_uses_consecutive_gaps_and_excludes_only_instant_interiors() {
        let points = [201.0, 100.0, 0.0, 300.0, 400.0, 400.0].map(|time_ms| SyncPoint {
            time_ms,
            tolerance: 2.0,
        });
        let groups = group_sync_points(points.to_vec(), 100.0, &[(201.0, 300.0)]).unwrap();
        assert_eq!(
            groups
                .iter()
                .map(|g| (g.start, g.end, g.points.len()))
                .collect::<Vec<_>>(),
            vec![(0.0, 100.0, 2), (300.0, 400.0, 3)]
        );
        let merged = groups[0].clone().merge(groups[1].clone());
        assert_eq!(
            (merged.start, merged.end, merged.points.len()),
            (0.0, 400.0, 5)
        );
    }

    #[test]
    fn jumps_do_not_become_long_motion_intervals() {
        let points = [(0.0, 0.0), (20.0, 10.0), (200.0, 30.0), (250.0, 30.0)]
            .map(|(time_ms, x)| Point { time_ms, x, y: 0.0 });
        let activity = MotionIntervals::new(&points);
        assert!(activity.moving_at(10.0));
        assert!(!activity.moving_at(20.0));
        assert!(activity.still_between(20.0, 199.0));
        assert!(!activity.still_between(199.0, 200.0));
        assert!(activity.moving_at(200.0));
        assert_eq!(
            activity.rest_at(100.0),
            Some(Interval {
                start: 20.0,
                end: 200.0
            })
        );
        assert_eq!(
            activity.last_rest_ending_by(250.0),
            Some(Interval {
                start: 20.0,
                end: 200.0
            })
        );
    }

    #[test]
    #[ignore = "opt-in activity fixture; set TARANTINO_CURSOR_ACTIVITY_FIXTURE"]
    fn activity_matches_external_fixture() {
        let input = std::env::var("TARANTINO_CURSOR_ACTIVITY_FIXTURE").unwrap();
        let cases: serde_json::Value =
            serde_json::from_slice(&std::fs::read(input).unwrap()).unwrap();
        let mut count = 0;
        for case in cases.as_array().unwrap() {
            let path = super::super::SourcePath::new(
                serde_json::from_value(case["points"].clone()).unwrap(),
            )
            .unwrap();
            let activity = path.activity();
            let interval = |value: Option<Interval>| {
                value.map(|r| [r.start, r.end].map(|v| v.is_finite().then_some(v)))
            };
            for sample in case["samples"].as_array().unwrap() {
                let t = sample["time"].as_f64().unwrap();
                assert_eq!(activity.moving_at(t), sample["moving"].as_bool().unwrap());
                assert_eq!(
                    interval(activity.rest_at(t)),
                    serde_json::from_value::<Option<[Option<f64>; 2]>>(sample["rest"].clone())
                        .unwrap()
                );
                assert_eq!(
                    interval(activity.last_rest_ending_by(t)),
                    serde_json::from_value::<Option<[Option<f64>; 2]>>(sample["lastRest"].clone())
                        .unwrap()
                );
                assert_eq!(
                    activity.still_between(t, t + 39.0),
                    sample["still"].as_bool().unwrap()
                );
                assert_eq!(
                    activity.still_between(t + 39.0, t),
                    sample["still"].as_bool().unwrap()
                );
                count += 1;
            }
        }
        assert!(count > 1000);
        println!("Compared {count} activity samples");
    }
}
