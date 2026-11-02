use super::*;

fn path(points: &[(f64, f64)]) -> SourcePath {
    SourcePath::new(
        points
            .iter()
            .map(|&(time_ms, x)| Point {
                time_ms,
                x,
                y: x * 0.5,
            })
            .collect(),
    )
    .unwrap()
}

#[test]
fn sparse_input_holds_and_continuous_input_interpolates() {
    for gap in [39.0, 40.0, 41.0, 1000.0] {
        let source = path(&[(0.0, 0.0), (gap, 100.0)]);
        assert_eq!(
            source.at(gap / 2.0),
            if gap <= 40.0 {
                [50.0, 25.0]
            } else {
                [0.0, 0.0]
            }
        );
        assert_eq!(source.at(gap), [100.0, 50.0]);
        assert_eq!(source.at(-100.0), [0.0, 0.0]);
        assert_eq!(source.at(gap + 100.0), [100.0, 50.0]);
    }
    assert!(SourcePath::new(vec![Point {
        time_ms: 0.0,
        x: f64::NAN,
        y: 0.0
    }])
    .is_err());
    assert_eq!(path(&[]).at(10.0), [0.0; 2]);
}

#[test]
fn integer_partitioning_preserves_position_and_velocity() {
    let source = path(&[(0.0, 0.0), (20.0, 100.0), (40.0, -80.0), (60.0, 30.0)]);
    let solver = Solver::new(
        &source,
        SpringConfig {
            tension: 470.0,
            friction: 70.0,
            mass: 3.0,
        },
        &[],
    )
    .unwrap();
    let initial = State::resting([0.0; 2]);
    let full = solver.advance(
        0.0,
        1000.0,
        initial,
        &|_| 0.0,
        &|t| if t < 32.0 { 1.0 } else { 2.5 },
        |_, _| {},
    );
    let mut chunked = initial;
    for t in 0..1000 {
        chunked = solver.advance(
            t as f64,
            (t + 1) as f64,
            chunked,
            &|_| 0.0,
            &|t| if t < 32.0 { 1.0 } else { 2.5 },
            |_, _| {},
        );
    }
    assert_eq!(full, chunked);
    assert_eq!(full, State::resting([30.0, 15.0]));
}

#[test]
fn instant_intervals_are_half_open_and_reset_velocity() {
    let source = path(&[(0.0, 0.0), (20.0, 100.0), (40.0, 200.0)]);
    let ranges = [(10.0, 20.0)];
    let solver = Solver::new(
        &source,
        SpringConfig {
            tension: 470.0,
            friction: 70.0,
            mass: 3.0,
        },
        &ranges,
    )
    .unwrap();
    let at19 = solver.advance(
        0.0,
        19.0,
        State::resting([0.0; 2]),
        &|_| 0.0,
        &|_| 1.0,
        |_, _| {},
    );
    assert_eq!(at19, State::resting([95.0, 47.5]));
    let at20 = solver.advance(19.0, 20.0, at19, &|_| 0.0, &|_| 1.0, |_, _| {});
    assert!(at20.position[0] < 100.0 && at20.velocity[0] > 0.0);
}

#[test]
#[ignore = "opt-in numeric motion fixture; set TARANTINO_CURSOR_PLANNER_FIXTURE"]
fn cursor_planning_matches_external_numeric_fixtures() {
    let input = std::env::var("TARANTINO_CURSOR_PLANNER_FIXTURE").expect("fixture path required");
    let fixtures: serde_json::Value =
        serde_json::from_slice(&std::fs::read(input).unwrap()).unwrap();
    let mut compared = 0;
    for case in fixtures["cases"].as_array().unwrap() {
        let points: Vec<Point> = serde_json::from_value(case["points"].clone()).unwrap();
        let source = SourcePath::new(points).unwrap();
        let config = &case["spring"];
        let instant: Vec<(f64, f64)> = serde_json::from_value(case["instant"].clone()).unwrap();
        let solver = Solver::new(
            &source,
            SpringConfig {
                tension: config["stiffness"].as_f64().unwrap(),
                friction: config["damping"].as_f64().unwrap(),
                mass: config["mass"].as_f64().unwrap(),
            },
            &instant,
        )
        .unwrap();
        let channel = |name: &str, time: f64, default: f64| {
            case[name]
                .as_array()
                .and_then(|values| values.get(time.max(0.0).floor() as usize))
                .and_then(|value| value.as_f64())
                .unwrap_or(default)
        };
        let track = solver.trajectory(2000.0, &|t| channel("lead", t, 0.0), &|t| {
            channel("speed", t, 1.0)
        });
        for sample in case["samples"].as_array().unwrap() {
            let time = sample["timeMs"].as_f64().unwrap();
            let actual = track.at(time);
            for (axis, key) in ["x", "y"].iter().enumerate() {
                let expected = sample[key].as_f64().unwrap();
                assert!(
                    (actual[axis] - expected).abs() < 1e-5,
                    "{} t={time} {key}: {} vs {expected}",
                    case["name"],
                    actual[axis]
                );
            }
            compared += 1;
        }
    }
    assert!(compared > 10000);
    println!("Compared {compared} cursor positions");
}
