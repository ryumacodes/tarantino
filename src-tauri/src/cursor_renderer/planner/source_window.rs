//! Inclusive source activity bounds shared by motion and button-state planning.
//! Input must be sorted. Keep fractional boundaries and loop seed ordering.
pub(crate) fn select<T: Copy>(
    events: &[T], start: f64, end: f64, looped: bool,
    time: impl Fn(&T) -> f64, with_time: impl Fn(T, f64) -> T,
) -> Vec<T> {
    if !start.is_finite() || !end.is_finite() { return Vec::new(); }
    let first = events.partition_point(|event| time(event) < start);
    let last = events.partition_point(|event| time(event) <= end);
    let mut selected = if first < last { events[first..last].to_vec() } else { Vec::new() };
    if selected.is_empty() {
        let index = match events.get(first) {
            Some(event) if time(event) == start => first,
            _ => first.saturating_sub(1),
        };
        selected.extend(events.get(index).copied());
    }
    if let (Some(opening), Some(closing)) = (selected.first().copied(), selected.last().copied()) {
        selected[0] = with_time(opening, start);
        if looped { selected.push(with_time(opening, time(&closing) + 1.0)); }
    }
    selected
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Clone, Copy, Debug, PartialEq, serde::Deserialize)]
    struct Sample {
        #[serde(rename = "recordingTimeMs")]
        time: f64,
        x: f64,
        y: f64,
        #[serde(rename = "type")]
        kind: super::super::press::Kind,
    }

    #[test]
    fn fractional_window_keeps_positions_and_original_loop_seed_time() {
        let events = [Sample { time:100.0,x:0.2,y:0.3,kind:super::super::press::Kind::Down }];
        let result = select(&events,650.25,650.75,true,|e|e.time,|e,time|Sample{time,..e});
        assert_eq!(result,vec![Sample{time:650.25,..events[0]},Sample{time:101.0,..events[0]}]);
    }

    #[test]
    #[ignore = "requires external source activity window fixtures"]
    fn activity_windows_match_external_positions_and_times() {
        #[derive(serde::Deserialize)]
        struct Case { events:Vec<Sample>,start:f64,end:f64,looped:bool,expected:Vec<Sample> }
        let path=std::env::var("TARANTINO_ACTIVITY_WINDOW_FIXTURES").unwrap();
        let cases:Vec<Case>=serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap();
        for case in cases {
            let actual=select(&case.events,case.start,case.end,case.looped,|e|e.time,|e,time|Sample{time,..e});
            assert_eq!(actual,case.expected,"window {}..{}, loop={}",case.start,case.end,case.looped);
        }
    }
}
