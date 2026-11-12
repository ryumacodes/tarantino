use super::{
    angular_spring::AngularTimeline,
    edited_path::MotionSegment,
    planner::runtime::Prepared,
    rotation::{sample, targets, MotionPoint, RotationKey},
};
use crate::video_processing::CursorSettings;

pub(super) struct EditedRotation {
    keys: Vec<RotationKey>,
    animation: AngularTimeline,
    cutoff: f64,
    base: f64,
    directional: bool,
}
impl EditedRotation {
    pub(super) fn new(
        points: &[MotionPoint],
        segments: &[MotionSegment],
        planned: Option<&Prepared>,
        settings: &CursorSettings,
        duration: f64,
    ) -> Option<Self> {
        if !settings.rotate_while_moving.unwrap_or(false) {
            return None;
        }
        let keys = planned.and_then(|plan| plan.rotation_keys()).map_or_else(
            || {
                let mut drawn = Vec::new();
                if let Some(first) = segments.first().and_then(|segment| segment.points.first()) {
                    drawn.push(*first);
                }
                for segment in segments {
                    let source_end = segment.source_end_ms;
                    let from =
                        points.partition_point(|point| point.time_ms < segment.source_start_ms);
                    let to = points.partition_point(|point| point.time_ms < source_end);
                    drawn.extend(points[from..to].iter().map(|point| {
                        let time = segment.output_start_ms
                            + (point.time_ms - segment.source_start_ms) / segment.rate;
                        MotionPoint {
                            time_ms: planned.map_or(time, |plan| plan.target_time_of(time)),
                            ..*point
                        }
                    }));
                }
                drawn.sort_by(|a, b| a.time_ms.total_cmp(&b.time_ms));
                if settings.loop_to_start.unwrap_or(false) {
                    drawn = super::loop_path::rotation_points(
                        &drawn,
                        duration.ceil() as u64,
                        settings.loop_duration_ms.unwrap_or(500),
                    );
                }
                targets(&drawn, settings)
            },
            |keys| keys.to_vec(),
        );
        let animation =
            AngularTimeline::for_mode(sample(&keys, 0.0), settings.rotation_mode.as_deref());
        let cutoff = if settings.stop_at_end.unwrap_or(false) {
            (duration - settings.stop_duration_ms.unwrap_or(0) as f64).max(0.0)
        } else {
            duration
        };
        Some(Self {
            keys,
            animation,
            cutoff,
            base: settings.rotation.unwrap_or(0.0),
            directional: settings.rotation_mode.as_deref() == Some("directional"),
        })
    }

    pub(super) fn at(&mut self, time: f64) -> f32 {
        let keys = &self.keys;
        let cutoff = self.cutoff;
        (self.base
            + self.animation.sample(time, |time| {
                let rest = (time - cutoff).max(0.0);
                sample(keys, time.min(cutoff))
                    * if self.directional {
                        1.0
                    } else if rest >= 720.0 {
                        0.0
                    } else {
                        (-rest / 120.0).exp()
                    }
            })) as f32
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::video_processing::timeline::{EditClip, EditTimeline};

    #[test]
    fn fractional_trim_excludes_rotation_from_the_next_source_segment() {
        let clip: EditClip = serde_json::from_value(serde_json::json!({
            "id":"clip", "startTime":300.0, "duration":300.0,
            "sourceIn":100.0, "sourceOut":250.0, "playbackRate":0.5,
            "trackOrder":0, "muted":false
        })).unwrap();
        let timeline = EditTimeline::build(&[clip],25.3,600.0).unwrap();
        for mode in ["horizontal", "directional"] {
            let settings: CursorSettings = serde_json::from_value(serde_json::json!({
                "rotate_while_moving":true, "rotation_mode":mode
            })).unwrap();
            let make = |outside_x| {
                let points = [
                    MotionPoint {time_ms:100.0,x:10.0,y:20.0},
                    MotionPoint {time_ms:249.0,x:100.0,y:30.0},
                    MotionPoint {time_ms:250.0,x:outside_x,y:200.0},
                ];
                let segments = super::super::edited_path::map_recording(&points,&timeline,"recording.mp4");
                EditedRotation::new(&points,&segments,None,&settings,timeline.duration_ms).unwrap()
            };
            let mut left = make(-10000.0);
            let mut right = make(10000.0);
            for time in 0..575 {
                assert_eq!(left.at(time as f64),right.at(time as f64),"{mode} at {time}ms");
            }
        }
    }
}
