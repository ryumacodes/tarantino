//! Playback-time controls for cursor anticipation and temporary spring speed.
use anyhow::{bail, Result};

#[derive(Clone, Default)]
pub struct LeadSchedule {
    knots: Vec<(f64, f64)>,
    cuts: Vec<f64>,
}

impl LeadSchedule {
    pub fn new(mut cuts: Vec<f64>) -> Result<Self> {
        if cuts.iter().any(|t| !t.is_finite()) {
            bail!("Invalid cursor cut time");
        }
        cuts.sort_by(f64::total_cmp);
        Ok(Self {
            knots: Vec::new(),
            cuts,
        })
    }

    pub fn add(&mut self, time: f64, lead: f64) -> Result<()> {
        if !time.is_finite()
            || !lead.is_finite()
            || self
                .knots
                .last()
                .is_some_and(|&(previous, _)| time < previous)
        {
            bail!("Cursor lead knots must have finite values and ascending times");
        }
        self.knots.push((time, lead));
        Ok(())
    }

    /// Unconstrained plan, used when choosing a candidate approach.
    pub fn planned_at(&self, time: f64) -> f64 {
        let count = self.knots.partition_point(|&(t, _)| t <= time);
        if count == 0 {
            return 0.0;
        }
        let (start, value) = self.knots[count - 1];
        let Some(&(end, next)) = self.knots.get(count) else {
            return value;
        };
        if start == end {
            return value;
        }
        value + (next - value) * (time - start) / (end - start)
    }

    /// Do not sample a future clip before its cut. At the cut itself, allow
    /// the new clip's lead; only the next cut limits it.
    pub fn at(&self, time: f64) -> f64 {
        let lead = self.planned_at(time).max(0.0);
        if lead == 0.0 {
            return 0.0;
        }
        let next = self.cuts.partition_point(|&cut| cut < time + 1e-9);
        self.cuts
            .get(next)
            .map_or(lead, |&cut| lead.min(cut - time))
    }
}

#[derive(Clone, Copy, Debug, serde::Deserialize)]
pub struct SpeedBump {
    pub ramp_in_start: f64,
    pub full_start: f64,
    pub full_end: f64,
    pub ramp_out_end: f64,
    pub speed: f64,
}

#[derive(Clone, Default)]
pub struct SpeedSchedule {
    bumps: Vec<(SpeedBump, f64)>,
    longest: f64,
}

impl SpeedSchedule {
    pub fn add(&mut self, bump: SpeedBump) -> Result<()> {
        if ![
            bump.ramp_in_start,
            bump.full_start,
            bump.full_end,
            bump.ramp_out_end,
            bump.speed,
        ]
        .iter()
        .all(|v| v.is_finite())
            || bump.ramp_in_start > bump.full_start
            || bump.full_start > bump.full_end
            || bump.full_end > bump.ramp_out_end
            || bump.speed <= 0.0
        {
            bail!("Invalid cursor speed envelope");
        }
        if bump.speed <= 1.0 {
            return Ok(());
        }
        self.longest = self.longest.max(bump.ramp_out_end - bump.ramp_in_start);
        self.bumps.push((bump, bump.speed.ln()));
        self.bumps
            .sort_by(|(a, _), (b, _)| a.ramp_in_start.total_cmp(&b.ramp_in_start));
        Ok(())
    }

    /// Blend in logarithmic speed. Overlaps choose the stronger envelope;
    /// multiplying them would introduce an unintended acceleration spike.
    pub fn at(&self, time: f64) -> f64 {
        let end = self.bumps.partition_point(|(b, _)| b.ramp_in_start <= time);
        let mut strongest: f64 = 0.0;
        for &(b, log_speed) in self.bumps[..end].iter().rev() {
            if b.ramp_in_start < time - self.longest {
                break;
            }
            if time >= b.ramp_out_end {
                continue;
            }
            let weight = if time < b.full_start {
                (time - b.ramp_in_start) / (b.full_start - b.ramp_in_start)
            } else if time <= b.full_end {
                1.0
            } else {
                (b.ramp_out_end - time) / (b.ramp_out_end - b.full_end)
            };
            strongest = strongest.max(log_speed * weight);
        }
        strongest.exp()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lead_respects_cuts_and_clone_independence() {
        let mut schedule = LeadSchedule::new(vec![500.0, 250.0]).unwrap();
        schedule.add(0.0, 100.0).unwrap();
        schedule.add(400.0, 300.0).unwrap();
        assert_eq!(schedule.planned_at(200.0), 200.0);
        assert_eq!(schedule.at(200.0), 50.0);
        assert_eq!(schedule.at(250.0), 225.0);
        assert_eq!(schedule.at(500.0), 300.0);
        let mut clone = schedule.clone();
        clone.add(500.0, 0.0).unwrap();
        assert_eq!(clone.at(600.0), 0.0);
        assert_eq!(schedule.at(600.0), 300.0);
        assert!(schedule.add(300.0, 100.0).is_err());
    }

    #[test]
    fn speed_blends_geometrically_and_overlaps_do_not_multiply() {
        let mut schedule = SpeedSchedule::default();
        let bump = SpeedBump {
            ramp_in_start: 0.0,
            full_start: 100.0,
            full_end: 200.0,
            ramp_out_end: 300.0,
            speed: 4.0,
        };
        schedule.add(bump).unwrap();
        schedule.add(bump).unwrap();
        for (time, expected) in [
            (-1.0, 1.0),
            (0.0, 1.0),
            (50.0, 2.0),
            (150.0, 4.0),
            (250.0, 2.0),
            (300.0, 1.0),
        ] {
            assert!((schedule.at(time) - expected).abs() < 1e-12);
        }
        let mut clone = schedule.clone();
        clone.add(SpeedBump { speed: 9.0, ..bump }).unwrap();
        assert!((clone.at(50.0) - 3.0).abs() < 1e-12);
        assert_eq!(schedule.at(50.0), 2.0);
    }

    #[test]
    #[ignore = "opt-in timing fixture; set TARANTINO_CURSOR_TIMING_FIXTURE"]
    fn timing_matches_external_fixture() {
        let path = std::env::var("TARANTINO_CURSOR_TIMING_FIXTURE").unwrap();
        let cases: serde_json::Value =
            serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap();
        let mut count = 0;
        for case in cases.as_array().unwrap() {
            let mut lead =
                LeadSchedule::new(serde_json::from_value(case["cuts"].clone()).unwrap()).unwrap();
            for knot in case["knots"].as_array().unwrap() {
                lead.add(knot[0].as_f64().unwrap(), knot[1].as_f64().unwrap())
                    .unwrap();
            }
            let mut speed = SpeedSchedule::default();
            for bump in case["bumps"].as_array().unwrap() {
                speed
                    .add(serde_json::from_value(bump.clone()).unwrap())
                    .unwrap();
            }
            for sample in case["samples"].as_array().unwrap() {
                let t = sample[0].as_f64().unwrap();
                for (actual, index) in [(lead.planned_at(t), 1), (lead.at(t), 2), (speed.at(t), 3)]
                {
                    let expected = sample[index].as_f64().unwrap();
                    assert!(
                        (actual - expected).abs() < 1e-9,
                        "t={t} channel={index}: {actual} vs {expected}"
                    );
                }
                count += 1;
            }
        }
        println!("Compared {count} cursor timing samples");
        assert!(count > 1000);
    }
}
