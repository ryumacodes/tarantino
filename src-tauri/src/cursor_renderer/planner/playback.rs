//! Map recording events into edited playback without rounding event times.
use super::{activity::SyncPoint, input::RecordingInput, Point};
use crate::video_processing::timeline::EditTimeline;
use std::path::Path;

pub struct PlaybackInput {
    pub points: Vec<Point>,
    pub rotation_events: Option<Vec<Point>>,
    pub sync: Vec<SyncPoint>,
    pub cuts: Vec<f64>,
    pub instant: Vec<(f64, f64)>,
    pub duration: f64,
    pub width: f64,
    pub height: f64,
}

pub fn map(input: &RecordingInput, timeline: &EditTimeline, recording: &Path) -> PlaybackInput {
    let mut seeds = Vec::new();
    let mut points = Vec::new();
    let mut sync = Vec::new();
    let mut cuts = if timeline.starts_after_cut { vec![0.0] } else { Vec::new() };
    let mut instant = Vec::new();
    let canonical = std::fs::canonicalize(recording).ok();
    let matches = |path: &Option<String>| {
        path.as_ref().is_none_or(|p| {
            Path::new(p) == recording
                || canonical
                    .as_ref()
                    .is_some_and(|c| std::fs::canonicalize(p).ok().as_ref() == Some(c))
        })
    };
    let mut previous: Option<(f64, f64)> = None;
    for segment in &timeline.segments {
        let Some(source_start) = segment
            .source_start_ms
            .filter(|_| matches(&segment.source_file_path))
        else {
            if previous.take().is_some() {
                cuts.push(segment.output_start_ms);
            }
            continue;
        };
        let start = segment.output_start_ms;
        let end = segment.output_end_ms;
        let source_end = segment.source_end_ms.unwrap();
        if previous.is_some_and(|(output, source)| (output - start).abs() > 1e-7 || (source - source_start).abs() > 1e-7)
            || (previous.is_none() && start > 0.0)
        {
            cuts.push(start);
        }
        previous = Some((end, source_end));
        // A seed carries the last observed position into the new clip; it
        // does not invent a click, a drag, or motion during missing samples.
        let count = input.points.partition_point(|p| p.time_ms <= source_start);
        if let Some(point) = input.points.get(count.saturating_sub(1)) {
            seeds.push(Point {
                time_ms: start,
                ..*point
            });
        }
        let output_time = |time: f64| {
            if time == source_end { end } else { start + (time - source_start) / segment.rate }
        };
        let from = input.points.partition_point(|p| p.time_ms < source_start);
        let to = input.points.partition_point(|p| p.time_ms <= source_end);
        points.extend(input.points[from..to].iter().map(|p| Point {
            time_ms: output_time(p.time_ms),
            ..*p
        }));
        let from = input.sync.partition_point(|p| p.time_ms < source_start);
        let to = input.sync.partition_point(|p| p.time_ms <= source_end);
        sync.extend(input.sync[from..to].iter().map(|p| SyncPoint {
            time_ms: output_time(p.time_ms),
            ..*p
        }));
        if segment.disable_cursor_smoothing {
            instant.push((start, end));
        }
    }
    points.sort_by(|a,b|a.time_ms.total_cmp(&b.time_ms));
    seeds.extend(points.iter().copied());
    seeds.sort_by(|a, b| a.time_ms.total_cmp(&b.time_ms));
    sync.sort_by(|a, b| a.time_ms.total_cmp(&b.time_ms));
    cuts.sort_by(f64::total_cmp);
    cuts.dedup();
    PlaybackInput {
        points: seeds,
        rotation_events: Some(points),
        sync,
        cuts,
        instant,
        duration: timeline.duration_ms,
        width: input.width,
        height: input.height,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::video_processing::timeline::EditClip;
    #[test]
    #[ignore = "opt-in mapping fixture; set TARANTINO_CURSOR_MAPPING_FIXTURE"]
    fn mapping_matches_external_fixture() {
        let path = std::env::var("TARANTINO_CURSOR_MAPPING_FIXTURE").unwrap();
        let cases: serde_json::Value =
            serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap();
        for case in cases.as_array().unwrap() {
            let clips: Vec<EditClip> = serde_json::from_value(case["clips"].clone()).unwrap();
            let timeline = EditTimeline::build(
                &clips,
                case["trimStart"].as_f64().unwrap(),
                case["trimEnd"].as_f64().unwrap(),
            )
            .unwrap();
            let input = RecordingInput {
                press_events: Vec::new(),
                points: serde_json::from_value(case["points"].clone()).unwrap(),
                sync: serde_json::from_value(case["sync"].clone()).unwrap(),
                width: 1000.0,
                height: 500.0,
            };
            let output = map(&input, &timeline, Path::new("recording.mp4"));
            let expected = &case["expected"];
            assert_eq!(
                output.points.len(),
                expected["points"].as_array().unwrap().len(),
                "trim={} clips={}", case["trimStart"], case["clips"]
            );
            for (actual, wanted) in output
                .points
                .iter()
                .zip(expected["points"].as_array().unwrap())
            {
                for (key, value) in [
                    ("time_ms", actual.time_ms),
                    ("x", actual.x),
                    ("y", actual.y),
                ] {
                    assert!(
                        (value - wanted[key].as_f64().unwrap()).abs() < 1e-9,
                        "{key}: actual={value} expected={} trim={} clips={}", wanted[key], case["trimStart"], case["clips"]
                    );
                }
            }
            let sync: Vec<SyncPoint> = serde_json::from_value(expected["sync"].clone()).unwrap();
            assert_eq!(output.sync.len(), sync.len());
            for (actual, expected) in output.sync.iter().zip(sync) {
                assert!((actual.time_ms - expected.time_ms).abs() < 1e-9);
                assert_eq!(actual.tolerance, expected.tolerance);
            }
            assert_eq!(
                output.cuts,
                serde_json::from_value::<Vec<f64>>(expected["cuts"].clone()).unwrap()
            );
            assert_eq!(
                output.instant,
                serde_json::from_value::<Vec<(f64, f64)>>(expected["instant"].clone()).unwrap()
            );
        }
    }
    fn clip(start: f64, source: f64, duration: f64, rate: f64) -> EditClip {
        serde_json::from_value(
            serde_json::json!({"id":format!("{start}"),"startTime":start,
            "duration":duration,"sourceIn":source,"sourceOut":source+duration*rate,
            "playbackRate":rate,"trackOrder":0,"muted":false}),
        )
        .unwrap()
    }
    fn recording() -> RecordingInput {
        RecordingInput {
            press_events: Vec::new(),
            points: [0.0, 100.0, 200.0, 250.0, 300.0, 500.0]
                .map(|time_ms| Point {
                    time_ms,
                    x: time_ms,
                    y: 20.0,
                })
                .to_vec(),
            sync: [100.0, 250.0, 300.0]
                .map(|time_ms| SyncPoint {
                    time_ms,
                    tolerance: 2.0,
                })
                .to_vec(),
            width: 1000.0,
            height: 500.0,
        }
    }
    #[test]
    fn repeated_source_preserves_fractional_constraints_and_gap_cuts() {
        let mut slow = clip(200.0, 100.0, 400.0, 0.5);
        slow.disable_cursor_smoothing = true;
        let timeline =
            EditTimeline::build(&[clip(0.0, 100.0, 100.0, 2.0), slow], 0.0, 600.0).unwrap();
        let output = map(&recording(), &timeline, Path::new("recording.mp4"));
        assert_eq!(
            output.sync.iter().map(|p| p.time_ms).collect::<Vec<_>>(),
            vec![0.0, 75.0, 100.0, 200.0, 500.0, 600.0]
        );
        assert!(output.sync.iter().all(|p| p.tolerance == 2.0));
        assert_eq!(output.cuts, vec![100.0, 200.0]);
        assert_eq!(output.instant, vec![(200.0, 600.0)]);
        assert_eq!((output.width, output.height), (1000.0, 500.0));
    }
    #[test]
    fn contiguous_speed_change_is_not_a_cut_and_seed_is_held() {
        let mut next = clip(100.0, 200.0, 200.0, 0.5);
        next.hide_cursor = true;
        let timeline =
            EditTimeline::build(&[clip(0.0, 100.0, 100.0, 1.0), next], 25.0, 300.0).unwrap();
        let output = map(&recording(), &timeline, Path::new("recording.mp4"));
        assert!(output.cuts.is_empty());
        assert_eq!(output.points[0].time_ms, 0.0);
        assert_eq!(output.rotation_events.as_ref().unwrap()[0].time_ms,75.0,
            "clip seed belongs to position planning, not recorded rotation events");
        assert_eq!(
            output.points[0].x, 100.0,
            "seed must hold the preceding sample, not interpolate to125"
        );
        assert!(
            output.sync.iter().any(|p| p.time_ms == 175.0),
            "hidden cursor still plans its source motion"
        );
    }
    #[test]
    fn fractional_trim_keeps_boundary_samples_in_stable_order() {
        let timeline = EditTimeline::build(&[
            clip(0.0, 0.0, 300.0, 0.3),
            clip(300.0, 100.0, 300.0, 0.5),
        ], 299.999, 600.0).unwrap();
        let mut input = recording();
        input.points.insert(1, Point {time_ms:90.0, x:90.0, y:20.0});
        let output = map(&input, &timeline, Path::new("recording.mp4"));
        let boundary = timeline.segments[0].output_end_ms;
        let positions: Vec<_> = output.points.iter().filter(|p|p.time_ms == boundary).map(|p|p.x).collect();
        assert_eq!(positions, vec![100.0, 90.0, 100.0]);
    }

    #[test]
    fn fractional_trim_keeps_the_final_cursor_sample() {
        let timeline = EditTimeline::build(&[
            clip(0.0, 0.0, 300.0, 0.5),
            clip(300.0, 100.0, 300.0, 0.5),
            clip(600.0, 250.0, 300.0, 1.0),
        ], 25.2, 850.0).unwrap();
        assert_eq!(timeline.segments.last().unwrap().source_end_ms, Some(500.0));
        let output = map(&recording(), &timeline, Path::new("recording.mp4"));
        assert_eq!(output.points.last().unwrap().x, 500.0);
        assert!((output.points.last().unwrap().time_ms - 824.8).abs() < 1e-9);
    }

    #[test]
    fn trimming_at_a_cut_keeps_its_opening_anticipation_barrier() {
        for (source, expected) in [(100.0, vec![0.0]), (150.0, vec![])] {
            let clips = [clip(0.0, 0.0, 300.0, 0.5), clip(300.0, source, 300.0, 0.5)];
            let timeline = EditTimeline::build(&clips, 300.0, 600.0).unwrap();
            assert_eq!(map(&recording(), &timeline, Path::new("recording.mp4")).cuts, expected);
            let inside = EditTimeline::build(&clips, 301.0, 600.0).unwrap();
            assert!(!inside.starts_after_cut);
        }
    }

    #[test]
    fn fractional_speed_boundary_preserves_continuity() {
        let mut first = clip(0.0, 1000.1, 1000.2, 1.0);
        first.source_out = 2000.3;
        let next = clip(1000.2, 2000.3, 500.0, 2.0);
        let timeline = EditTimeline::build(&[first.clone(), next.clone()], 0.0, 1500.2).unwrap();
        assert!(map(&recording(), &timeline, Path::new("recording.mp4")).cuts.is_empty());
        for offset in [-0.01, 0.01] {
            let mut jumped = next.clone();
            jumped.source_in += offset;
            jumped.source_out += offset;
            let timeline = EditTimeline::build(&[first.clone(), jumped], 0.0, 1500.2).unwrap();
            assert_eq!(map(&recording(), &timeline, Path::new("recording.mp4")).cuts, vec![1000.2]);
        }
    }
    #[test]
    fn source_jump_marks_cut_and_other_recording_is_excluded() {
        let mut other = clip(100.0, 100.0, 100.0, 1.0);
        other.source_file_path = Some("other.mp4".into());
        let timeline = EditTimeline::build(
            &[
                clip(0.0, 0.0, 100.0, 1.0),
                other,
                clip(200.0, 300.0, 100.0, 1.0),
            ],
            0.0,
            300.0,
        )
        .unwrap();
        let output = map(&recording(), &timeline, Path::new("recording.mp4"));
        assert_eq!(output.cuts, vec![100.0, 200.0]);
        assert!(!output
            .points
            .iter()
            .any(|p| p.time_ms > 100.0 && p.time_ms < 200.0));
    }
}
