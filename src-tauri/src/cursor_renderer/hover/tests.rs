use super::*;

fn frame(time: f64, level: u8) -> Frame {
    let mut y = vec![80; 128 * 128];
    for row in 56..=72 {
        for x in 56..=72 {
            y[row * 128 + x] = level;
        }
    }
    Frame::new(time, 128, 128, y, vec![128; 128 * 64]).unwrap()
}

fn samples() -> Vec<Sample> {
    [
        (0.0, 40.0),
        (400.0, 40.0),
        (450.0, 48.0),
        (500.0, 56.0),
        (550.0, 64.0),
        (600.0, 64.0),
        (2000.0, 64.0),
    ]
    .map(|(time_ms, x)| Sample {
        time_ms,
        x,
        y: 64.0,
        free: true,
    })
    .to_vec()
}

#[test]
fn settled_changes_respect_clicks_and_keep_bounded_history() {
    for (click, expected) in [
        (None, vec![600.0]),
        (Some(300.0), vec![610.0]),
        (Some(680.0), vec![]),
        (Some(681.0), vec![600.0]),
    ] {
        let mut detector = Detector::new(samples(), click.into_iter().collect(), 1.0).unwrap();
        let mut times = Vec::new();
        for n in 0..200 {
            if let Some(time) = detector
                .push(frame(n as f64 * 10.0, if n < 60 { 80 } else { 180 }))
                .unwrap()
            {
                times.push(time);
            }
            assert!(detector.history.len() <= 28);
        }
        assert_eq!(times, expected);
    }
}

#[test]
fn movement_counts_travel_and_excludes_old_predecessors_and_drags() {
    let path = [(430.0, 48.0), (500.0, 50.0), (550.0, 48.0)].map(|(time_ms, x)| Sample {
        time_ms,
        x,
        y: 64.0,
        free: true,
    });
    assert_eq!(input::free_movement(&path, 600.0), 4.0);
    assert_eq!(input::free_movement(&path, 750.0), 0.0);
    let drag = path.map(|s| Sample { free: false, ..s });
    assert_eq!(input::free_movement(&drag, 600.0), 0.0);
    assert_eq!(input::position(&path, 535.0), Some([48.6, 64.0]));
    assert_eq!(input::position(&path, 400.0), None);
}

#[test]
fn frame_boundary_rejects_invalid_planes_and_geometry_changes() {
    assert!(Frame::new(0.0, 2, 2, vec![0; 4], vec![0; 1]).is_err());
    let mut detector = Detector::new(samples(), vec![], 1.0).unwrap();
    detector.push(frame(100.0, 80)).unwrap();
    assert!(detector.push(frame(90.0, 80)).is_err());
    assert!(
        detector
            .push(Frame::new(110.0, 2, 2, vec![0; 4], vec![128; 2]).unwrap())
            .is_err()
    );
}

#[test]
#[ignore = "requires external decoded hover fixtures"]
fn decoded_hover_matches_external_fixtures() {
    #[derive(serde::Deserialize)]
    struct Fixture {
        name: String,
        width: usize,
        height: usize,
        before: Vec<[u8; 3]>,
        after: Vec<[u8; 3]>,
        samples: Vec<Sample>,
        expected: Vec<f64>,
    }
    let path = std::env::var("TARANTINO_HOVER_FIXTURE").unwrap();
    let cases: Vec<Fixture> = serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap();
    let count = cases.len();
    for case in cases {
        let make_frame = |time: f64, pixels: &[[u8; 3]]| {
            let y = pixels.iter().map(|p| p[0]).collect();
            let mut uv = Vec::new();
            for row in (0..case.height).step_by(2) {
                for x in (0..case.width).step_by(2) {
                    uv.extend_from_slice(&pixels[row * case.width + x][1..]);
                }
            }
            Frame::new(time, case.width, case.height, y, uv).unwrap()
        };
        let mut detector = Detector::new(case.samples, vec![], 1.0).unwrap();
        let mut actual = Vec::new();
        for n in 0..120 {
            let time = n as f64 * 1000.0 / 60.0;
            if let Some(time) = detector
                .push(make_frame(
                    time,
                    if n < 36 { &case.before } else { &case.after },
                ))
                .unwrap()
            {
                actual.push(time);
            }
        }
        assert_eq!(actual, case.expected, "{}", case.name);
    }
    assert!(count >= 19);
    println!("Compared {count} decoded hover cases");
}

#[test]
fn fractional_scroll_step_includes_the_maximum_displacement() {
    let width = 256;
    let y = (0..width * width)
        .map(|i| {
            let (x, y) = (i % width, i / width);
            (20 + (x * 31 + y * 47 + x * y * 13) % 193) as u8
        })
        .collect::<Vec<_>>();
    let shifted = (0..width * width)
        .map(|i| if i % width >= 80 { y[i - 80] } else { 20 })
        .collect();
    let before = Frame::new(0.0, width, width, y, vec![128; width * width / 2]).unwrap();
    let after = Frame::new(600.0, width, width, shifted, vec![128; width * width / 2]).unwrap();
    assert!(pixels::explained_by_scroll(
        &before,
        &after,
        [128.0, 128.0],
        2.5
    ));
}
