//! Source bounds for cursor state selection, including the inclusive final
//! endpoint. Media-frame lookup remains half-open in EditTimeline::source_at.
use crate::video_processing::timeline::EditTimeline;
use std::path::Path;

pub fn boundary(timeline: &EditTimeline, time: f64, recording: &Path) -> Option<f64> {
    if !time.is_finite() {
        return None;
    }
    let index = timeline
        .segments
        .partition_point(|s| s.output_end_ms < time)
        .min(timeline.segments.len().saturating_sub(1));
    let segment = timeline.segments.get(index)?;
    if let Some(path) = &segment.source_file_path {
        if Path::new(path) != recording {
            let expected = std::fs::canonicalize(recording).ok()?;
            if std::fs::canonicalize(path).ok()? != expected {
                return None;
            }
        }
    }
    let source = segment.source_start_ms?;
    Some(
        source
            + (time.clamp(segment.output_start_ms, segment.output_end_ms)
                - segment.output_start_ms)
                * segment.rate,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::video_processing::timeline::EditClip;

    #[test]
    #[ignore = "requires external source-boundary fixtures"]
    fn source_boundaries_match_external_resolver() {
        let path = std::env::var("TARANTINO_PRESS_BOUNDARY_FIXTURES").unwrap();
        let cases: serde_json::Value =
            serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap();
        for case in cases.as_array().unwrap() {
            let clips=case["links"].as_array().unwrap().iter().enumerate().map(|(i,l)| {
                let number=|key:&str|l[key].as_f64().unwrap();
                let start=number("playbackStartMs");let end=number("playbackEndMs");
                serde_json::from_value::<EditClip>(serde_json::json!({"id":i.to_string(),"startTime":start,
                    "duration":end-start,"sourceIn":number("sourceStartMs"),"sourceOut":number("sourceEndMs"),
                    "playbackRate":(number("sourceEndMs")-number("sourceStartMs"))/(end-start),
                    "trackOrder":0,"muted":false})).unwrap()
            }).collect::<Vec<_>>();
            let duration = clips.last().unwrap().start_time + clips.last().unwrap().duration;
            let timeline = EditTimeline::build(&clips, 0.0, duration).unwrap();
            let actual = boundary(
                &timeline,
                case["time"].as_f64().unwrap(),
                Path::new("video.mp4"),
            )
            .unwrap();
            assert!((actual - case["expected"].as_f64().unwrap()).abs() < 1e-9);
        }
    }
}
