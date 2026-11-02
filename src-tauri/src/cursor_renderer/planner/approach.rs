//! Candidate lead envelopes for approaching an interaction group.
use super::{timing::LeadSchedule, SourcePath};

#[derive(Debug)]
pub struct LeadCandidate {
    pub knots: Vec<(f64, f64)>,
    pub changes_from: f64,
}

pub fn decay(path: &SourcePath, anchor: f64, lead: f64) -> Vec<(f64, f64)> {
    if lead <= 0.0 {
        return vec![(anchor, 0.0)];
    }
    let end = if path.activity().still_between(anchor, anchor + lead) {
        anchor + 0.001
    } else {
        anchor + lead / 0.35
    };
    vec![(anchor, lead), (end, 0.0)]
}

pub fn candidate(
    path: &SourcePath,
    anchor: f64,
    anchor_lead: f64,
    start: f64,
    end: f64,
    lead: f64,
) -> Option<LeadCandidate> {
    let baseline = decay(path, anchor, anchor_lead);
    let mut schedule = LeadSchedule::default();
    for &(t, value) in &baseline {
        schedule.add(t, value).ok()?;
    }
    if lead <= 0.0 {
        let mut knots = baseline
            .into_iter()
            .filter(|&(t, _)| t < end)
            .collect::<Vec<_>>();
        knots.push((end, schedule.planned_at(end)));
        return Some(LeadCandidate {
            knots,
            changes_from: start,
        });
    }
    let valid_rate = |time: f64, value: f64| {
        let rate = (start + lead - (time + value)) / (start - time);
        (0.65..=1.4).contains(&rate)
    };
    let earliest = anchor.max(start - 1500.0);
    let mut rest = path.activity().last_rest_ending_by(start + lead);
    let mut raise = None;
    while let Some(interval) = rest.filter(|r| r.end > earliest) {
        let at = (start - 1.0).min(earliest.max(interval.end - lead));
        let existing = schedule.planned_at(at);
        let jump = lead.min(interval.end - at);
        if at + jump.min(existing) >= interval.start
            && at + jump.max(existing) <= interval.end
            && valid_rate(at, jump)
        {
            raise = Some((at, jump));
            break;
        }
        rest = path.activity().last_rest_ending_by(interval.start);
    }
    let (at, jump) = raise.or_else(|| {
        let at = anchor.max(start - 400.0);
        if at >= start {
            return None;
        }
        let value = schedule.planned_at(at);
        valid_rate(at, value).then_some((at, value))
    })?;
    let mut knots = baseline
        .into_iter()
        .filter(|&(t, _)| t < at)
        .collect::<Vec<_>>();
    knots.extend([
        (at, schedule.planned_at(at)),
        (at + 0.001, jump),
        (start, lead),
        (end, lead),
    ]);
    Some(LeadCandidate {
        knots,
        changes_from: at,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    #[ignore = "opt-in lead-candidate fixture; set TARANTINO_CURSOR_APPROACH_FIXTURE"]
    fn approaches_match_external_fixture() {
        let input = std::env::var("TARANTINO_CURSOR_APPROACH_FIXTURE").unwrap();
        let cases: serde_json::Value =
            serde_json::from_slice(&std::fs::read(input).unwrap()).unwrap();
        let mut count = 0;
        for case in cases.as_array().unwrap() {
            let path =
                SourcePath::new(serde_json::from_value(case["points"].clone()).unwrap()).unwrap();
            let number = |key: &str| case[key].as_f64().unwrap();
            let actual = candidate(
                &path,
                number("anchor"),
                number("anchorLead"),
                number("start"),
                number("end"),
                number("lead"),
            );
            let expected = &case["expected"];
            if expected.is_null() {
                assert!(actual.is_none(), "unexpected candidate in {case}");
            } else {
                let actual = actual.expect("missing lead candidate");
                assert!(
                    (actual.changes_from - expected["changesFromMs"].as_f64().unwrap()).abs()
                        < 1e-9
                );
                let knots = expected["knots"].as_array().unwrap();
                assert_eq!(actual.knots.len(), knots.len());
                for ((time, value), expected) in actual.knots.into_iter().zip(knots) {
                    assert!((time - expected[0].as_f64().unwrap()).abs() < 1e-9);
                    assert!((value - expected[1].as_f64().unwrap()).abs() < 1e-9);
                }
            }
            count += 1;
        }
        assert!(count > 100);
        println!("Compared {count} lead candidates");
    }
}
