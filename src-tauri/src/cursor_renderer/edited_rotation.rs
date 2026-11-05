use super::{angular_spring::AngularTimeline, edited_path::MotionSegment,
    planner::runtime::Prepared, rotation::{horizontal_tilt, sample, MotionPoint, RotationKey}};
use crate::video_processing::CursorSettings;

pub(super) struct EditedRotation {
    keys: Vec<RotationKey>,
    animation: AngularTimeline,
    cutoff: f64,
    base: f64,
}
impl EditedRotation {
    pub(super) fn new(points:&[MotionPoint],segments:&[MotionSegment],planned:Option<&Prepared>,
        settings:&CursorSettings,duration:f64) -> Option<Self> {
        if !settings.rotate_while_moving.unwrap_or(false) {return None;}
        let mut drawn=Vec::new();
        if let Some(first)=segments.first().and_then(|segment|segment.points.first()) {
            drawn.push(*first);
        }
        for segment in segments {
            let source_end=segment.source_start_ms+(segment.output_end_ms-segment.output_start_ms)*segment.rate;
            let from=points.partition_point(|point|point.time_ms<segment.source_start_ms);
            let to=points.partition_point(|point|point.time_ms<source_end);
            drawn.extend(points[from..to].iter().map(|point| {
                let time=segment.output_start_ms+(point.time_ms-segment.source_start_ms)/segment.rate;
                MotionPoint{time_ms:planned.map_or(time,|plan|plan.target_time_of(time)),..*point}
            }));
        }
        drawn.sort_by(|a,b|a.time_ms.total_cmp(&b.time_ms));
        if settings.loop_to_start.unwrap_or(false) {
            drawn=super::loop_path::rotation_points(&drawn,duration.ceil() as u64,settings.loop_duration_ms.unwrap_or(500));
        }
        let keys=horizontal_tilt(&drawn,settings.rotation_intensity.unwrap_or(30.0)/100.0);
        let animation=AngularTimeline::new(sample(&keys,0.0));
        let cutoff=if settings.stop_at_end.unwrap_or(false) {
            (duration-settings.stop_duration_ms.unwrap_or(0) as f64).max(0.0)
        } else {duration};
        Some(Self{keys,animation,cutoff,base:settings.rotation.unwrap_or(0.0)})
    }

    pub(super) fn at(&mut self,time:f64)->f32 {
        let keys=&self.keys;
        let cutoff=self.cutoff;
        (self.base+self.animation.sample(time,|time| {
            let rest=(time-cutoff).max(0.0);
            sample(keys,time.min(cutoff))*if rest>=720.0 {0.0} else {(-rest/120.0).exp()}
        })) as f32
    }
}
