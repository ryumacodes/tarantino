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

pub fn build(
    path: &SourcePath,
    spring: SpringConfig,
    sync: Vec<SyncPoint>,
    cuts: Vec<f64>,
    instant: &[(f64, f64)],
    duration: f64,
) -> Result<Plan> {
    if !duration.is_finite() || duration < 0.0 {
        bail!("Invalid cursor plan duration");
    }
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
    let motion = solver.trajectory(duration, &|t| lead.at(t), &|t| speed.at(t));
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
    Ok(Plan {
        motion,
        lead,
        speed,
        groups,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
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
