//! Interaction-aware shape selection. This preserves observation timestamps and
//! output order; a click may promote an earlier appearance after a later reset.
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Kind { Move, Drag, Down, Up, Other }

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct Event {
    pub time: u64,
    pub kind: Kind,
    pub shape: usize,
}

pub fn filter(events: &[Event], arrow: usize) -> Vec<Event> {
    let mut result: Vec<Event> = Vec::new();
    let mut pending: Option<Event> = None;
    for &event in events {
        if pending.is_none_or(|previous| previous.shape != event.shape) {
            pending = Some(event);
        }
        let last = result.last().copied();
        match event.kind {
            Kind::Drag => {
                if last.is_none_or(|previous| previous.shape != event.shape) {
                    result.push(event);
                }
            }
            Kind::Move => {
                if last.is_some_and(|previous| previous.kind == Kind::Up
                    && previous.shape != event.shape
                    && event.time.saturating_sub(previous.time) > 250) {
                    result.push(Event { shape: arrow, ..event });
                }
            }
            Kind::Down => {
                if let Some(appearance) = pending.take() { result.push(appearance); }
            }
            Kind::Up => result.push(event),
            Kind::Other => {}
        }
    }
    result
}

/// Lower-bound selection keeps the first exact-time observation. Deliberately
/// uses the filter's output order, including promoted appearances.
pub fn index_at(events: &[Event], time: u64) -> Option<usize> {
    let (mut low, mut high) = (0, events.len());
    while low < high {
        let middle = low + (high - low) / 2;
        if events[middle].time < time { low = middle + 1; }
        else { high = middle; }
    }
    if events.get(low).is_some_and(|event| event.time == time) { Some(low) }
    else { low.checked_sub(1) }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn event(time: u64, kind: Kind, shape: usize) -> Event { Event { time, kind, shape } }

    #[test]
    fn click_promotes_the_first_observation_without_reordering_resets() {
        use Kind::*;
        let input = [event(0,Down,1),event(100,Up,1),event(200,Move,2),
            event(350,Move,2),event(400,Move,2),event(500,Down,2)];
        assert_eq!(filter(&input,0), [input[0],input[1],event(400,Move,0),input[2]]);
        assert!(filter(&[event(0,Move,0),event(100,Move,1)],0).is_empty());
    }

    #[test]
    fn drag_retains_changes_and_reuses_shapes_without_a_click() {
        use Kind::*;
        let input = [event(0,Drag,1),event(10,Drag,1),event(20,Drag,2),
            event(30,Drag,1),event(40,Up,1)];
        assert_eq!(filter(&input,0),[input[0],input[2],input[3],input[4]]);
    }

    #[test]
    #[ignore = "requires external cursor shape fixtures"]
    fn cursor_shape_filter_matches_external_fixtures() {
        #[derive(Deserialize)]
        struct Sample { time: u64, index: Option<usize> }
        #[derive(Deserialize)]
        struct Fixture { name: String, input: Vec<Event>, output: Vec<Event>, samples: Vec<Sample> }
        let path = std::env::var("TARANTINO_CURSOR_SHAPE_FIXTURES").unwrap();
        let fixtures: Vec<Fixture> = serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap();
        assert!(!fixtures.is_empty());
        for fixture in fixtures {
            assert_eq!(filter(&fixture.input,0),fixture.output,"{}",fixture.name);
            for sample in fixture.samples {
                assert_eq!(index_at(&fixture.output,sample.time),sample.index,
                    "{} at {}",fixture.name,sample.time);
            }
        }
    }
}
