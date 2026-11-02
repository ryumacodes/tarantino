//! Button-state timing is independent of cursor position and click ripples.
#[derive(Clone, Copy, Debug, PartialEq, serde::Deserialize)]
pub enum Kind {
    #[serde(rename = "mouseMoved")]
    Move,
    #[serde(rename = "mouseDown")]
    Down,
    #[serde(rename = "mouseUp")]
    Up,
    #[serde(rename = "mouseDragged")]
    Drag,
}

#[derive(Clone, Copy, Debug, serde::Deserialize)]
pub struct Event {
    #[serde(rename = "recordingTimeMs")]
    pub time: f64,
    #[serde(rename = "type")]
    pub kind: Kind,
}

pub struct PressTrack {
    events: Vec<Event>,
    buttons: Vec<Event>,
}

impl PressTrack {
    pub fn new(mut events: Vec<Event>) -> Self {
        events.retain(|e| e.time.is_finite());
        events.sort_by(|a, b| a.time.total_cmp(&b.time));
        let buttons = events
            .iter()
            .copied()
            .filter(|e| matches!(e.kind, Kind::Down | Kind::Up))
            .collect();
        Self { events, buttons }
    }

    pub fn effective(&self, time: f64, smooth: bool) -> Option<Event> {
        if !time.is_finite() {
            return None;
        }
        if smooth {
            let next = self.buttons.partition_point(|e| e.time < time);
            if let Some(event) = self.buttons.get(next) {
                if event.time - time <= 500.0 {
                    return Some(*event);
                }
            }
        }
        // At equal timestamps choose the first event; between timestamps use
        // the final preceding event. Before recording use the first sample.
        let next = self.events.partition_point(|e| e.time < time);
        let index = match self.events.get(next) {
            Some(event) if event.time == time => next,
            _ => next.saturating_sub(1),
        };
        self.events.get(index).copied()
    }

    /// Restrict source events before lookahead. The opening event is seeded at
    /// the source start; loop closure retains its kind, not a fabricated release.
    pub fn window(self, start: f64, end: f64, looped: bool) -> Self {
        let events = super::source_window::select(
            &self.events, start, end, looped, |event| event.time,
            |event, time| Event { time, ..event },
        );
        let buttons = events
            .iter()
            .copied()
            .filter(|e| matches!(e.kind, Kind::Down | Kind::Up))
            .collect();
        Self { events, buttons }
    }

    pub fn pressed(&self, time: f64, smooth: bool) -> bool {
        self.effective(time, smooth)
            .is_some_and(|e| matches!(e.kind, Kind::Down | Kind::Drag))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn excluded_future_press_cannot_anticipate_into_the_closing_window() {
        let events = vec![
            Event {
                time: 0.0,
                kind: Kind::Move,
            },
            Event {
                time: 900.0,
                kind: Kind::Down,
            },
        ];
        let full = PressTrack::new(events.clone());
        assert!(full.pressed(500.0, true));
        let window = PressTrack::new(events).window(0.0, 600.0, true);
        assert!(!window.pressed(500.0, true));
        assert!(!window.pressed(950.0, false));
    }

    #[test]
    #[ignore = "requires external source-window fixtures"]
    fn source_window_matches_external_event_sequence() {
        #[derive(serde::Deserialize)]
        struct Case {
            events: Vec<Event>,
            start: f64,
            end: f64,
            looped: bool,
            expected: Vec<Event>,
        }
        let path = std::env::var("TARANTINO_PRESS_WINDOW_FIXTURES").unwrap();
        let cases: Vec<Case> = serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap();
        for case in cases {
            let actual = PressTrack::new(case.events).window(case.start, case.end, case.looped);
            assert_eq!(actual.events.len(), case.expected.len());
            for (a, b) in actual.events.iter().zip(case.expected) {
                assert_eq!(a.time, b.time);
                assert_eq!(a.kind, b.kind);
            }
        }
    }

    #[test]
    fn press_anticipation_and_drag_release_are_separate_from_click_pulses() {
        let track = PressTrack::new(vec![
            Event {
                time: 0.0,
                kind: Kind::Move,
            },
            Event {
                time: 1000.0,
                kind: Kind::Down,
            },
            Event {
                time: 1100.0,
                kind: Kind::Drag,
            },
            Event {
                time: 2000.0,
                kind: Kind::Up,
            },
        ]);
        assert!(!track.pressed(499.999, true));
        assert!(track.pressed(500.0, true));
        assert!(!track.pressed(500.0, false));
        assert!(track.pressed(1499.0, true));
        assert!(!track.pressed(1500.0, true));
        assert!(track.pressed(1500.0, false));
        assert!(!track.pressed(2000.0, false));
        assert!(!track.pressed(f64::NAN, true));
    }

    #[test]
    #[ignore = "requires external timing fixtures"]
    fn matches_external_press_timing() {
        #[derive(serde::Deserialize)]
        struct Sample {
            time: f64,
            event: Option<Kind>,
            #[serde(rename = "eventTime")]
            event_time: Option<f64>,
            pressed: bool,
        }
        #[derive(serde::Deserialize)]
        struct Case {
            events: Vec<Event>,
            smooth: bool,
            samples: Vec<Sample>,
        }
        let file = std::env::var("TARANTINO_PRESS_FIXTURES").unwrap();
        let cases: Vec<Case> = serde_json::from_slice(&std::fs::read(file).unwrap()).unwrap();
        for case in cases {
            let track = PressTrack::new(case.events);
            for sample in case.samples {
                let actual = track.effective(sample.time, case.smooth);
                assert_eq!(actual.map(|e| e.kind), sample.event);
                assert_eq!(actual.map(|e| e.time), sample.event_time);
                assert_eq!(track.pressed(sample.time, case.smooth), sample.pressed);
            }
        }
    }
}
