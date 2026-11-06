//! Plan a complete playback path, retrying close constraints together when
//! separate approaches cannot meet interaction accuracy.
use super::{
    activity::{group_sync_points, SyncGroup, SyncPoint},
    approach,
    optimize::{Choice, Context, Evaluator},
    timing::{LeadSchedule, SpeedSchedule},
    MotionTrack, Solver, SourcePath, SpringConfig, State,
};
use anyhow::{bail, Result};

struct Committed {
    group: SyncGroup,
    choice: Choice,
    previous_lead: LeadSchedule,
    previous_speed: SpeedSchedule,
}

pub struct GroupResult {
    pub start: f64,
    pub end: f64,
    pub lead: f64,
    pub speed: f64,
    pub error: f64,
}
pub struct Plan {
    pub motion: MotionTrack,
    pub lead: LeadSchedule,
    pub speed: SpeedSchedule,
    pub groups: Vec<GroupResult>,
}

pub struct TimingPlan {
    pub lead: LeadSchedule,
    pub speed: SpeedSchedule,
    pub groups: Vec<GroupResult>,
}

pub fn build(
    path: &SourcePath,
    spring: SpringConfig,
    sync: Vec<SyncPoint>,
    cuts: Vec<f64>,
    instant: &[(f64, f64)],
    duration: f64,
) -> Result<Plan> {
    let timing = build_timing(path, spring, sync, cuts, instant, duration)?;
    let solver = Solver::new(path, spring, instant)?;
    let motion = solver.trajectory(duration, &|t| timing.lead.at(t), &|t| timing.speed.at(t));
    Ok(Plan {
        motion,
        lead: timing.lead,
        speed: timing.speed,
        groups: timing.groups,
    })
}

pub fn build_timing(
    path: &SourcePath,
    spring: SpringConfig,
    sync: Vec<SyncPoint>,
    cuts: Vec<f64>,
    instant: &[(f64, f64)],
    duration: f64,
) -> Result<TimingPlan> {
    if !duration.is_finite() || duration < 0.0 {
        bail!("Invalid cursor plan duration");
    }
    path.sample_hint.set(0);
    let solver = Solver::new(path, spring, instant)?;
    let mut groups = group_sync_points(sync, 100.0, instant)?;
    let initial_time = path.start().min(0.0);
    let mut committed: Vec<Committed> = Vec::new();
    let mut lead = LeadSchedule::new(cuts)?;
    let mut speed = SpeedSchedule::default();
    let mut index = 0;
    while index < groups.len() {
        let group = &groups[index];
        let previous = committed.last();
        let context = Context {
            anchor: previous.map_or(initial_time, |p| p.group.end),
            anchor_lead: previous.map_or(0.0, |p| p.choice.end_lead()),
            state: previous.map_or_else(
                || State::resting(path.at(initial_time)),
                |p| p.choice.end_state,
            ),
            lead: &lead,
            speed: &speed,
        };
        let choice = Evaluator::new(path, &solver, group, context).choose()?;
        if choice.error > 1.0 && previous.is_some_and(|p| group.start - p.group.end <= 400.0) {
            let prior = committed.pop().unwrap();
            lead = prior.previous_lead;
            speed = prior.previous_speed;
            groups[index] = prior.group.merge(group.clone());
            continue;
        }
        let previous_lead = lead.clone();
        let previous_speed = speed.clone();
        choice.commit(&mut lead, &mut speed)?;
        committed.push(Committed {
            group: group.clone(),
            choice,
            previous_lead,
            previous_speed,
        });
        index += 1;
    }
    let last = committed.last();
    for (time, value) in approach::decay(
        path,
        last.map_or(initial_time, |p| p.group.end),
        last.map_or(0.0, |p| p.choice.end_lead()),
    ) {
        lead.add(time, value)?;
    }
    let groups = committed
        .into_iter()
        .map(|entry| GroupResult {
            start: entry.group.start,
            end: entry.group.end,
            lead: lead.at(entry.group.start),
            speed: entry.choice.speed,
            error: entry.choice.error,
        })
        .collect();
    Ok(TimingPlan {
        lead,
        speed,
        groups,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn drag_across_fractional_cut_preserves_timing_and_repeatable_playback() {
        let point = |source: f64, time_ms| super::super::Point {
            time_ms,
            x: 500.0 + 240.0 * (source / 20.0 * 0.013).sin(),
            y: 300.0 + 160.0 * (source / 20.0 * 0.023).sin(),
        };
        let mut points = vec![point(0.0, 0.0), point(15000.0, 9000.0)];
        points.extend((0..=450).map(|i| point(i as f64 * 20.0, i as f64 * 20.0)));
        points.extend((751..=905).map(|i| point(i as f64 * 20.0, i as f64 * 20.0 - 6007.5)));
        let path = SourcePath::new(points).unwrap();
        let sync = (430..=450)
            .map(|i| i as f64 * 20.0)
            .chain((751..=770).map(|i| i as f64 * 20.0 - 6007.5))
            .map(|time_ms| SyncPoint {
                time_ms,
                tolerance: 2.0,
            })
            .collect::<Vec<_>>();
        let spring = SpringConfig {
            tension: 470.0,
            friction: 70.0,
            mass: 3.0,
        };
        let first = build(&path, spring, sync.clone(), vec![9000.0], &[], 12100.0).unwrap();
        assert!((first.lead.target_time_of(8220.0) - 8219.651125729084).abs() < 1e-7);
        let expected = first.motion.at(9000.0);
        for time in [12000.0, 8999.0, 0.0, 9000.0] {
            first.motion.at(time);
            path.at(time);
            assert_eq!(first.motion.at(9000.0), expected);
        }
        let repeated = build(&path, spring, sync, vec![9000.0], &[], 12100.0).unwrap();
        assert_eq!(repeated.motion.at(9000.0), expected);
        assert_eq!(
            repeated.lead.target_time_of(8220.0),
            first.lead.target_time_of(8220.0)
        );
    }

    #[test]
    #[ignore = "requires external edited planning fixtures"]
    fn edited_constraint_timing_matches_external_plan() {
        let path = std::env::var("TARANTINO_EDITED_PLAN").unwrap();
        let data: serde_json::Value =
            serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap();
        for case in data["results"].as_array().unwrap() {
            for (index, reference) in case["plans"].as_array().unwrap().iter().enumerate() {
                let options = &reference["options"];
                let points = options["points"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|p| super::super::Point {
                        time_ms: p["timeMs"].as_f64().unwrap(),
                        x: p["x"].as_f64().unwrap(),
                        y: p["y"].as_f64().unwrap(),
                    })
                    .collect();
                let path = SourcePath::new(points).unwrap();
                let spring = &options["spring"];
                let sync = options["syncPoints"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|p| SyncPoint {
                        time_ms: p["timeMs"].as_f64().unwrap(),
                        tolerance: p["tolerancePt"].as_f64().unwrap(),
                    })
                    .collect();
                let plan = build_timing(
                    &path,
                    SpringConfig {
                        tension: spring["stiffness"].as_f64().unwrap(),
                        friction: spring["damping"].as_f64().unwrap(),
                        mass: spring["mass"].as_f64().unwrap(),
                    },
                    sync,
                    serde_json::from_value(options["cutTimesMs"].clone()).unwrap(),
                    &[],
                    options["durationMs"].as_f64().unwrap(),
                )
                .unwrap();
                for expected in reference["targetTimes"].as_array().unwrap() {
                    let time = expected["time"].as_f64().unwrap();
                    let actual = plan.lead.target_time_of(time);
                    let target = expected["target"].as_f64().unwrap();
                    assert!(
                        (actual - target).abs() < 1e-7,
                        "chunk {index} at {time}: {actual} vs {target}"
                    );
                }
            }
        }
    }

    #[test]
    #[ignore = "opt-in full planner fixture; set TARANTINO_CURSOR_PLAN_FIXTURE"]
    fn complete_plans_match_external_fixture() {
        let input = std::env::var("TARANTINO_CURSOR_PLAN_FIXTURE").unwrap();
        let cases: serde_json::Value =
            serde_json::from_slice(&std::fs::read(input).unwrap()).unwrap();
        let mut count = 0;
        for (index, case) in cases.as_array().unwrap().iter().enumerate() {
            println!("plan case {index}");
            let path =
                SourcePath::new(serde_json::from_value(case["points"].clone()).unwrap()).unwrap();
            let config = &case["spring"];
            let spring = SpringConfig {
                tension: config["stiffness"].as_f64().unwrap(),
                friction: config["damping"].as_f64().unwrap(),
                mass: config["mass"].as_f64().unwrap(),
            };
            let instant: Vec<(f64, f64)> = serde_json::from_value(case["instant"].clone()).unwrap();
            let plan = build(
                &path,
                spring,
                serde_json::from_value(case["sync"].clone()).unwrap(),
                serde_json::from_value(case["cuts"].clone()).unwrap(),
                &instant,
                2000.0,
            )
            .unwrap();
            let groups = case["groups"].as_array().unwrap();
            assert_eq!(plan.groups.len(), groups.len());
            for (actual, expected) in plan.groups.iter().zip(groups) {
                for (key, value) in [
                    ("startMs", actual.start),
                    ("endMs", actual.end),
                    ("leadMs", actual.lead),
                    ("speed", actual.speed),
                    ("errorRatio", actual.error),
                ] {
                    assert!(
                        (value - expected[key].as_f64().unwrap()).abs() < 1e-7,
                        "group {key}"
                    );
                }
            }
            for sample in case["samples"].as_array().unwrap() {
                let t = sample["time"].as_f64().unwrap();
                let p = plan.motion.at(t);
                for (key, value) in [
                    ("x", p[0]),
                    ("y", p[1]),
                    ("lead", plan.lead.at(t)),
                    ("speed", plan.speed.at(t)),
                ] {
                    let wanted = sample[key].as_f64().unwrap();
                    assert!(
                        (value - wanted).abs() < 1e-5,
                        "t={t} {key}: {value} vs {wanted}"
                    );
                }
                count += 1;
            }
        }
        assert!(count > 10000);
        println!("Compared {count} fully planned samples");
    }
}
