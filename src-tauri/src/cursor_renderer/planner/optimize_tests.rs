use super::*;
use crate::cursor_renderer::SpringConfig;

fn check(actual: Option<Choice>, expected: &serde_json::Value) {
    if expected.is_null() {
        assert!(actual.is_none());
        return;
    }
    let actual = actual.expect("missing choice");
    for (key, value) in [
        ("leadMs", actual.lead),
        ("speed", actual.speed),
        ("errorRatio", actual.error),
    ] {
        let wanted = expected[key].as_f64().unwrap();
        assert!((value - wanted).abs() < 1e-8, "{key}: {value} vs {wanted}");
    }
    for (key, value) in [
        ("x", actual.end_state.position[0]),
        ("y", actual.end_state.position[1]),
        ("velocityX", actual.end_state.velocity[0]),
        ("velocityY", actual.end_state.velocity[1]),
    ] {
        let wanted = expected["endState"][key].as_f64().unwrap();
        assert!((value - wanted).abs() < 1e-7, "{key}: {value} vs {wanted}");
    }
    let knots = expected["leadKnots"].as_array().unwrap();
    assert_eq!(actual.knots.len(), knots.len());
    for ((t, v), knot) in actual.knots.iter().zip(knots) {
        assert!((t - knot[0].as_f64().unwrap()).abs() < 1e-8);
        assert!((v - knot[1].as_f64().unwrap()).abs() < 1e-8);
    }
}

#[test]
#[ignore = "opt-in optimizer fixture; set TARANTINO_CURSOR_OPTIMIZER_FIXTURE"]
fn optimizer_matches_external_fixture() {
    let input = std::env::var("TARANTINO_CURSOR_OPTIMIZER_FIXTURE").unwrap();
    let cases: serde_json::Value = serde_json::from_slice(&std::fs::read(input).unwrap()).unwrap();
    let mut count = 0;
    for (index, case) in cases.as_array().unwrap().iter().enumerate() {
        println!("optimizer case {index}");
        let path =
            SourcePath::new(serde_json::from_value(case["points"].clone()).unwrap()).unwrap();
        let config = &case["spring"];
        let spring = SpringConfig {
            tension: config["stiffness"].as_f64().unwrap(),
            friction: config["damping"].as_f64().unwrap(),
            mass: config["mass"].as_f64().unwrap(),
        };
        let solver = Solver::new(&path, spring, &[]).unwrap();
        let anchor = case["anchor"].as_f64().unwrap();
        let lead =
            LeadSchedule::new(serde_json::from_value(case["cuts"].clone()).unwrap()).unwrap();
        let speed = SpeedSchedule::default();
        let points = serde_json::from_value(case["sync"].clone()).unwrap();
        let group = SyncGroup {
            start: 1000.0,
            end: 1100.0,
            points,
        };
        let mut evaluator = Evaluator::new(
            &path,
            &solver,
            &group,
            Context {
                anchor,
                anchor_lead: case["anchorLead"].as_f64().unwrap(),
                state: State::resting(path.at(anchor)),
                lead: &lead,
                speed: &speed,
            },
        );
        for trial in case["trials"].as_array().unwrap() {
            let choice = evaluator
                .score(
                    trial["lead"].as_f64().unwrap(),
                    trial["speed"].as_f64().unwrap(),
                    f64::INFINITY,
                )
                .unwrap();
            check(choice, &trial["expected"]);
            count += 1;
        }
        check(Some(evaluator.choose().unwrap()), &case["choice"]);
        count += 1;
    }
    assert!(count > 100);
    println!("Compared {count} scored/selected approaches");
}

#[test]
fn zero_damping_search_terminates() {
    let path = SourcePath::new(vec![
        super::super::Point {
            time_ms: 0.0,
            x: 0.0,
            y: 0.0,
        },
        super::super::Point {
            time_ms: 1000.0,
            x: 100.0,
            y: 0.0,
        },
    ])
    .unwrap();
    let solver = Solver::new(
        &path,
        SpringConfig {
            tension: 470.0,
            friction: 0.0,
            mass: 3.0,
        },
        &[],
    )
    .unwrap();
    let group = SyncGroup {
        start: 1000.0,
        end: 1000.0,
        points: vec![super::super::activity::SyncPoint {
            time_ms: 1000.0,
            tolerance: 2.0,
        }],
    };
    let lead = LeadSchedule::default();
    let speed = SpeedSchedule::default();
    let choice = Evaluator::new(
        &path,
        &solver,
        &group,
        Context {
            anchor: 0.0,
            anchor_lead: 0.0,
            state: State::resting([0.0; 2]),
            lead: &lead,
            speed: &speed,
        },
    )
    .choose()
    .unwrap();
    assert!(choice.error.is_finite() && choice.speed <= 32.0);
}
