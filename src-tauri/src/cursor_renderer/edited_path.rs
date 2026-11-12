//! Recording-to-output motion mapping. Each visible segment keeps a separate
//! path so callers can reset simulation at cuts instead of bridging a gap.
use super::rotation::MotionPoint;
use crate::video_processing::timeline::EditTimeline;

#[derive(Debug)]
pub(crate) struct MotionSegment {
    pub output_start_ms:f64,
    pub output_end_ms:f64,
    pub source_start_ms:f64,
    pub source_end_ms:f64,
    pub rate:f64,
    pub points:Vec<MotionPoint>,
}

#[derive(Debug)]
pub(crate) struct CursorSegment {
    pub motion:MotionSegment,
    /// Exact click times are kept as floating point until simulation sampling.
    pub clicks:Vec<MotionPoint>,
}

/// Preserve click events independently from interpolated boundary targets.
/// A boundary position never fabricates a click or repeats the preceding one.
pub(crate) fn map_cursor(events:&[super::CursorEvent],timeline:&EditTimeline,recording_path:&str)->Vec<CursorSegment> {
    let points=events.iter().map(|event|MotionPoint{time_ms:event.timestamp_ms as f64,
        x:event.x,y:event.y}).collect::<Vec<_>>();
    map_recording(&points,timeline,recording_path).into_iter().map(|motion| {
        let start=motion.source_start_ms;
        let end=motion.source_end_ms;
        let from=events.partition_point(|event|(event.timestamp_ms as f64)<start);
        let to=events.partition_point(|event|(event.timestamp_ms as f64)<end);
        let clicks=events[from..to].iter().filter(|event|event.is_click).map(|event|MotionPoint {
            time_ms:motion.output_start_ms+(event.timestamp_ms as f64-start)/motion.rate,
            x:event.x,y:event.y,
        }).collect();
        CursorSegment{motion,clicks}
    }).collect()
}

fn at(points:&[MotionPoint],time:f64)->Option<MotionPoint> {
    let next=points.partition_point(|point|point.time_ms<=time);
    let first=points.first()?;
    if next==0 {return Some(MotionPoint{time_ms:time,..*first});}
    let previous=points[next-1];
    let Some(after)=points.get(next) else {return Some(MotionPoint{time_ms:time,..previous});};
    let t=(time-previous.time_ms)/(after.time_ms-previous.time_ms);
    Some(MotionPoint{time_ms:time,x:previous.x+(after.x-previous.x)*t,
        y:previous.y+(after.y-previous.y)*t})
}

/// Map one recording's sorted pixel path into every matching edited occurrence.
/// Coordinates stay in recording units; speed changes affect time, not distance.
/// The final boundary point is a target for interpolation, not a visible frame.
pub(crate) fn map_recording(points:&[MotionPoint],timeline:&EditTimeline,recording_path:&str)->Vec<MotionSegment> {
    timeline.segments.iter().filter_map(|segment| {
        let matches_recording=segment.source_file_path.as_deref().is_none_or(|path| {
            if path==recording_path {return true;}
            match (std::fs::canonicalize(path),std::fs::canonicalize(recording_path)) {
                (Ok(source),Ok(recording))=>source==recording,
                _=>false,
            }
        });
        if segment.hide_cursor || !matches_recording {
            return None;
        }
        let start=segment.source_start_ms?;
        let end=segment.source_end_ms?;
        // Do not introduce a cursor before its first recorded event.
        let first_time=start.max(points.first()?.time_ms);
        if first_time>=end {return None;}
        let first=at(points,first_time)?;
        let last=at(points,end)?;
        let mapped=|point:MotionPoint|MotionPoint {
            time_ms:segment.output_start_ms+(point.time_ms-start)/segment.rate,..point
        };
        let mut output=vec![mapped(first)];
        let from=points.partition_point(|point|point.time_ms<=first_time);
        let to=points.partition_point(|point|point.time_ms<end);
        output.extend(points[from..to].iter().copied().map(mapped));
        output.push(mapped(last));
        Some(MotionSegment{output_start_ms:segment.output_start_ms,
            output_end_ms:segment.output_end_ms,source_start_ms:start,source_end_ms:end,rate:segment.rate,points:output})
    }).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::video_processing::timeline::EditClip;
    fn clip(id:&str,start:f64,source:f64,length:f64,rate:f64)->EditClip {
        EditClip{id:id.into(),start_time:start,duration:length,source_in:source,
            source_out:source+length*rate,playback_rate:rate,source_file_path:Some("recording.mp4".into()),
            track_order:0,mic_volume: 1.0, system_volume: 1.0, muted:false,hide_cursor:false,disable_cursor_smoothing:false}
    }
    fn points()->Vec<MotionPoint> {
        (0..=8).map(|i|MotionPoint{time_ms:i as f64*250.0,x:i as f64*100.0,y:20.0}).collect()
    }
    #[test]
    fn repeated_source_has_separate_output_paths_and_speed() {
        let timeline=EditTimeline::build(&[clip("fast",0.0,500.0,500.0,2.0),
            clip("slow",750.0,500.0,1000.0,0.5)],0.0,1750.0).unwrap();
        let segments=map_recording(&points(),&timeline,"recording.mp4");
        assert_eq!(segments.len(),2);
        assert_eq!((segments[0].output_start_ms,segments[0].output_end_ms),(0.0,500.0));
        assert_eq!((segments[1].output_start_ms,segments[1].output_end_ms),(750.0,1750.0));
        assert_eq!(segments[0].points.iter().map(|p|p.time_ms).collect::<Vec<_>>(),vec![0.0,125.0,250.0,375.0,500.0]);
        assert_eq!(segments[1].points.iter().map(|p|p.time_ms).collect::<Vec<_>>(),vec![750.0,1250.0,1750.0]);
        assert_eq!(segments[0].points[0].x,segments[1].points[0].x);
        let fast_speed=(segments[0].points[1].x-segments[0].points[0].x)/125.0;
        let slow_speed=(segments[1].points[1].x-segments[1].points[0].x)/500.0;
        assert_eq!(fast_speed,4.0*slow_speed);
    }
    #[test]
    fn trim_uses_interpolated_boundary_positions_without_neighboring_events() {
        let timeline=EditTimeline::build(&[clip("one",0.0,0.0,2000.0,1.0)],125.0,875.0).unwrap();
        let segments=map_recording(&points(),&timeline,"recording.mp4");
        let path=&segments[0].points;
        assert_eq!((path[0].time_ms,path[0].x),(0.0,50.0));
        assert_eq!((path.last().unwrap().time_ms,path.last().unwrap().x),(750.0,350.0));
        assert_eq!(path.len(),5);
    }
    #[test]
    fn gaps_hidden_segments_and_other_recordings_never_borrow_motion() {
        let mut hidden=clip("hidden",500.0,0.0,500.0,1.0);hidden.hide_cursor=true;
        let mut other=clip("other",1000.0,0.0,500.0,1.0);other.source_file_path=Some("other.mp4".into());
        let timeline=EditTimeline::build(&[hidden,other,clip("ours",1500.0,0.0,500.0,1.0)],0.0,2000.0).unwrap();
        let segments=map_recording(&points(),&timeline,"recording.mp4");
        assert_eq!(segments.len(),1);
        assert_eq!(segments[0].output_start_ms,1500.0);
        assert!(map_recording(&[],&timeline,"recording.mp4").is_empty());
    }
    #[test]
    fn first_event_is_not_pulled_back_into_an_empty_opening() {
        let timeline=EditTimeline::build(&[clip("one",0.0,0.0,1000.0,1.0)],0.0,1000.0).unwrap();
        let delayed=[MotionPoint{time_ms:600.0,x:10.0,y:20.0}];
        let mapped=map_recording(&delayed,&timeline,"recording.mp4");
        assert_eq!(mapped[0].points[0].time_ms,600.0);
        let empty=EditTimeline::build(&[clip("one",0.0,0.0,500.0,1.0)],0.0,500.0).unwrap();
        assert!(map_recording(&delayed,&empty,"recording.mp4").is_empty());
    }
    fn clicks(times:&[u64])->Vec<super::super::CursorEvent> {
        times.iter().map(|time|super::super::CursorEvent{ is_release: false,timestamp_ms:*time,x:*time as f64/1000.0,y:0.4,is_click:true}).collect()
    }
    #[test]
    fn click_on_a_contiguous_cut_belongs_only_to_the_next_segment() {
        let timeline=EditTimeline::build(&[clip("a",0.0,0.0,500.0,1.0),clip("b",500.0,500.0,500.0,1.0)],0.0,1000.0).unwrap();
        let segments=map_cursor(&clicks(&[0,499,500,999,1000]),&timeline,"recording.mp4");
        assert_eq!(segments[0].clicks.iter().map(|p|p.time_ms).collect::<Vec<_>>(),vec![0.0,499.0]);
        assert_eq!(segments[1].clicks.iter().map(|p|p.time_ms).collect::<Vec<_>>(),vec![500.0,999.0]);
        assert_eq!(segments[1].clicks[0].x,0.5);
    }
    #[test]
    fn fractional_trim_does_not_duplicate_a_boundary_click() {
        let timeline=EditTimeline::build(&[
            clip("a",0.0,0.0,300.0,0.5),
            clip("b",300.0,100.0,300.0,0.5),
            clip("c",600.0,250.0,300.0,1.0),
        ],25.3,850.0).unwrap();
        let segments=map_cursor(&clicks(&[0,249,250,499,500]),&timeline,"recording.mp4");
        assert_eq!(segments[1].clicks.iter().map(|p|p.x).collect::<Vec<_>>(),vec![0.249]);
        assert_eq!(segments[2].clicks.iter().map(|p|p.x).collect::<Vec<_>>(),vec![0.250,0.499]);
    }
    #[test]
    fn repeated_clicks_retime_without_rounding_fractional_output_times() {
        let timeline=EditTimeline::build(&[clip("fast",0.0,0.0,250.0,2.0),clip("slow",500.0,0.0,1000.0,0.5)],0.0,1500.0).unwrap();
        let segments=map_cursor(&clicks(&[0,101,400]),&timeline,"recording.mp4");
        assert_eq!(segments[0].clicks.iter().map(|p|p.time_ms).collect::<Vec<_>>(),vec![0.0,50.5,200.0]);
        assert_eq!(segments[1].clicks.iter().map(|p|p.time_ms).collect::<Vec<_>>(),vec![500.0,702.0,1300.0]);
    }
    #[test]
    fn interpolated_trim_boundary_does_not_replay_an_earlier_click() {
        let timeline=EditTimeline::build(&[clip("one",0.0,0.0,1000.0,1.0)],250.0,750.0).unwrap();
        let segments=map_cursor(&clicks(&[0,500,1000]),&timeline,"recording.mp4");
        assert_eq!(segments[0].motion.points[0].time_ms,0.0);
        assert_eq!(segments[0].clicks.len(),1);
        assert_eq!(segments[0].clicks[0].time_ms,250.0);
    }
}
