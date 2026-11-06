use crate::cursor_renderer::{SpringConfig, SpringState, TargetTolerance, step_frame_spring};

#[derive(Clone, Copy, Debug, PartialEq, serde::Deserialize)]
pub(super) struct ScreenRect {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

impl ScreenRect {
    pub fn zoom_to(self, area: Self, focus: (f64, f64), scale: f64, anchor: Option<(f64, f64)>) -> Self {
        let width = self.width * scale;
        let height = self.height * scale;
        let anchor = anchor.unwrap_or((area.x + area.width / 2.0, area.y + area.height / 2.0));
        let place = |origin: f64, extent: f64, size: f64, position: f64| {
            if size <= extent { origin + (extent - size) / 2.0 }
            else { position.clamp(origin + extent - size, origin) }
        };
        Self {
            x: place(area.x, area.width, width, anchor.0 - focus.0 * width),
            y: place(area.y, area.height, height, anchor.1 - focus.1 * height),
            width,
            height,
        }
    }

    pub fn interpolate(self, next: Self, fraction: f64) -> Self {
        Self {
            x: self.x + (next.x - self.x) * fraction,
            y: self.y + (next.y - self.y) * fraction,
            width: self.width + (next.width - self.width) * fraction,
            height: self.height + (next.height - self.height) * fraction,
        }
    }

    fn components(self) -> [f64; 4] { [self.x, self.y, self.width, self.height] }
}

pub(super) struct RectMotion {
    states: [SpringState; 4],
    tolerances: [TargetTolerance; 4],
}

impl RectMotion {
    pub fn new(initial: ScreenRect) -> Self {
        Self {
            states: initial.components().map(|value| SpringState { value, velocity: 0.0 }),
            tolerances: [TargetTolerance::default(); 4],
        }
    }

    pub fn advance(&mut self, target: ScreenRect, config: &SpringConfig, elapsed_ms: f64) -> ScreenRect {
        for (index, value) in target.components().into_iter().enumerate() {
            let state = self.states[index];
            let precision = self.tolerances[index].update(state.value, value);
            self.states[index] = step_frame_spring(state, value, config, elapsed_ms, precision);
        }
        let [x, y, width, height] = self.states.map(|state| state.value);
        ScreenRect { x, y, width, height }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[ignore = "requires external rectangle timeline fixtures"]
    fn window_camera_pipeline_matches_external_rectangles() {
        #[derive(serde::Deserialize)]
        struct Config { tension:f64,friction:f64,mass:f64 }
        #[derive(serde::Deserialize)]
        struct Case { config:Config,blocks:Vec<super::super::ZoomBlock>,samples:Vec<[f64;4]> }
        let path=std::env::var("TARANTINO_RECT_PIPELINE_CASES").unwrap();
        let cases:Vec<Case>=serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap();
        assert!(!cases.is_empty());
        for case in cases {
            let config=SpringConfig{tension:case.config.tension,friction:case.config.friction,mass:case.config.mass};
            for window in [true, false] {
                for fps in [24.0,60.0,120.0,144.0] {
                    let frames=super::super::simulate_zoom_trajectory(&case.blocks,&[],&config,true,"focused",fps,3000,window);
                    for (index,frame) in frames.iter().enumerate() {
                        let position=index as f64*60.0/fps;
                        let lower=position.floor() as usize;
                        let fraction=position-lower as f64;
                        let expected:[f64;4]=std::array::from_fn(|i|case.samples[lower][i]
                            +(case.samples[lower+1][i]-case.samples[lower][i])*fraction);
                        let rect=if window {frame.rect.unwrap()} else {
                            [0.5-frame.center_x*frame.scale,0.5-frame.center_y*frame.scale,frame.scale,frame.scale]
                        };
                        for (actual,expected) in rect.into_iter().zip(expected) {
                            assert!((actual-expected).abs()<1e-9,"window={window} {fps}fps frame {index}: {actual} != {expected}");
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn letterboxed_axis_stays_centered_and_large_axis_clamps() {
        let source = ScreenRect { x: 0.0, y: 369.0, width: 608.0, height: 342.0 };
        let area = ScreenRect { x: 0.0, y: 0.0, width: 608.0, height: 1080.0 };
        let target = source.zoom_to(area, (1.0, 1.0), 2.0, None);
        assert_eq!(target, ScreenRect { x: -608.0, y: 198.0, width: 1216.0, height: 684.0 });
        assert_eq!(source.interpolate(target, 0.5), ScreenRect {
            x: -304.0, y: 283.5, width: 912.0, height: 513.0,
        });
    }

    #[test]
    #[ignore = "requires external rectangle geometry fixtures"]
    fn rectangle_targets_match_external_geometry() {
        #[derive(serde::Deserialize)]
        struct Point { x: f64, y: f64 }
        #[derive(serde::Deserialize)]
        struct Case { initial: ScreenRect, area: ScreenRect, focus: Point, scale: f64, expected: ScreenRect }
        let path = std::env::var("TARANTINO_RECT_GEOMETRY_CASES").unwrap();
        let cases: Vec<Case> = serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap();
        assert!(!cases.is_empty());
        for case in cases {
            let actual = case.initial.zoom_to(case.area, (case.focus.x, case.focus.y), case.scale, None);
            for (actual, expected) in actual.components().into_iter().zip(case.expected.components()) {
                assert!((actual - expected).abs() < 1e-9);
            }
        }
    }

    #[test]
    #[ignore = "requires external rectangle animation fixtures"]
    fn rectangle_motion_matches_external_samples() {
        #[derive(serde::Deserialize)]
        struct Sample { time: f64, target: ScreenRect, expected: ScreenRect }
        #[derive(serde::Deserialize)]
        struct Config { tension: f64, friction: f64, mass: f64 }
        #[derive(serde::Deserialize)]
        struct Case { initial: ScreenRect, config: Config, samples: Vec<Sample> }
        let path = std::env::var("TARANTINO_RECT_MOTION_CASES").unwrap();
        let cases: Vec<Case> = serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap();
        assert!(!cases.is_empty());
        for case in cases {
            let mut motion = RectMotion::new(case.initial);
            let config = SpringConfig { tension: case.config.tension,
                friction: case.config.friction, mass: case.config.mass };
            let mut previous = 0.0;
            for sample in case.samples {
                let actual = motion.advance(sample.target, &config, sample.time - previous);
                previous = sample.time;
                for (actual, expected) in actual.components().into_iter().zip(sample.expected.components()) {
                    assert!((actual - expected).abs() < 1e-9,"time {}: {actual} != {expected}",sample.time);
                }
            }
        }
    }
}
