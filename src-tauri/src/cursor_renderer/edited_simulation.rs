//! Cursor simulation in output time for edited exports.
use super::{CursorEvent,CursorFrameState,SpringConfig};
use super::{edited_path::{map_cursor,map_recording},rotation::{MotionPoint,horizontal_tilt}};
use crate::video_processing::{timeline::EditTimeline,CursorSettings,zoom_trajectory::{ZoomFrameState,CAMERA_SIMULATION_FPS}};

pub(crate) fn simulate(
    events:&[CursorEvent],physical:&[MotionPoint],timeline:&EditTimeline,recording:&str,
    spring:&SpringConfig,settings:&CursorSettings,source_zoom:&Option<Vec<ZoomFrameState>>,fps:f64,
)->Vec<CursorFrameState> {
    simulate_with_activity(events, physical, timeline, recording, spring, settings, source_zoom, fps, None)
}

pub(crate) fn simulate_with_activity(
    events:&[CursorEvent],physical:&[MotionPoint],timeline:&EditTimeline,recording:&str,
    spring:&SpringConfig,settings:&CursorSettings,source_zoom:&Option<Vec<ZoomFrameState>>,fps:f64,
    activity: Option<&super::typing_activity::TypingActivity>,
)->Vec<CursorFrameState> {
    simulate_with_planned_path(events,physical,timeline,recording,spring,settings,source_zoom,fps,activity,None,fps)
}

pub(crate) fn simulate_with_planned_path(
    events:&[CursorEvent],physical:&[MotionPoint],timeline:&EditTimeline,recording:&str,
    spring:&SpringConfig,settings:&CursorSettings,source_zoom:&Option<Vec<ZoomFrameState>>,fps:f64,
    activity:Option<&super::typing_activity::TypingActivity>,planned:Option<&super::planner::runtime::Prepared>,animation_fps:f64,
)->Vec<CursorFrameState> {
    let press_animation=planned.and_then(|p|super::edited_press::build(timeline,recording,animation_fps,
        spring,settings,|source|p.pressed_at_source(source,spring.mass!=0.0)));
    let segments=map_cursor(events,timeline,recording);
    let physical_segments=map_recording(physical,timeline,recording);
    let count=(timeline.duration_ms*fps/1000.0).ceil() as usize;
    let mut output=vec![CursorFrameState::default();count];
    let opening=segments.first().and_then(|segment|segment.motion.points.first()).copied();
    let physical_opening=physical_segments.first().and_then(|segment|segment.points.first()).copied();
    let loop_cutoff=timeline.duration_ms-settings.loop_duration_ms.unwrap_or(500) as f64;
    for segment in segments {
        let motion=segment.motion;
        let start=motion.output_start_ms;
        let end=motion.output_end_ms;
        let duration=(end-start).ceil() as u64;
        let mut local=settings.clone();
        // The closing behavior belongs to the edited output, not every cut.
        local.loop_to_start=Some(false);
        local.stop_at_end=Some(settings.stop_at_end.unwrap_or(false) && end==timeline.duration_ms);
        let loop_active=settings.loop_to_start.unwrap_or(false);
        let prepare=|points:&[MotionPoint],initial:Option<MotionPoint>| {
            let mut points=points.to_vec();
            if loop_active && end>loop_cutoff {
                points.retain(|point|point.time_ms<=loop_cutoff);
                if let Some(initial)=initial {
                    let return_time=points.last().map_or(start,|point|point.time_ms+1.0);
                    points.push(MotionPoint{time_ms:return_time,..initial});
                }
            }
            for point in &mut points {point.time_ms-=start;}
            points
        };
        let path=prepare(&motion.points,opening);
        let mut local_events=path.iter().map(|point|CursorEvent { is_release: false,
            timestamp_ms:point.time_ms.max(0.0).floor() as u64,x:point.x,y:point.y,is_click:false,
        }).collect::<Vec<_>>();
        for click in segment.clicks.iter().filter(|point|!loop_active || point.time_ms<=loop_cutoff) {
            // Quantize to the first simulation tick at or after the click.
            let tick=((click.time_ms-start)*CAMERA_SIMULATION_FPS/1000.0-1e-9).ceil();
            let timestamp_ms=(tick*1000.0/CAMERA_SIMULATION_FPS+1e-7).floor().max(0.0) as u64;
            local_events.push(CursorEvent{ is_release: false,timestamp_ms,x:click.x,y:click.y,is_click:true});
        }
        local_events.sort_by_key(|event|event.timestamp_ms);
        let physical_path=physical_segments.iter().find(|part|part.output_start_ms==start)
            .map(|part|prepare(&part.points,physical_opening)).unwrap_or_default();
        let keys=horizontal_tilt(&physical_path,settings.rotation_intensity.unwrap_or(30.0)/100.0);
        let local_zoom=source_zoom.as_ref().filter(|frames|!frames.is_empty()).map(|frames| {
            (0..(duration as f64*CAMERA_SIMULATION_FPS/1000.0).ceil() as usize).map(|index| {
                let source=motion.source_start_ms+index as f64*1000.0/CAMERA_SIMULATION_FPS*motion.rate;
                frames[((source*CAMERA_SIMULATION_FPS/1000.0+1e-7).floor() as usize).min(frames.len()-1)].clone()
            }).collect()
        });
        let mut local_spring=*spring;
        let segment_index=timeline.segments.partition_point(|segment|segment.output_start_ms<start);
        if timeline.segments.get(segment_index).is_some_and(|segment|segment.disable_cursor_smoothing) {
            local_spring.mass=0.0;
        }
        let typing = |time| activity.is_some_and(|a| a.active(motion.source_start_ms + time * motion.rate));
        let position=|time|planned.unwrap().at(start+time);
        let frames=super::simulate_cursor_frames_from(&local_events,&local_spring,CAMERA_SIMULATION_FPS,duration,
            &local_zoom,&local,0,CAMERA_SIMULATION_FPS,Some(&keys),activity.map(|_| &typing as &dyn Fn(f64)->bool),
            planned.map(|_|&position as &dyn Fn(f64)->[f64;2]),None,Clone::clone);
        let first=(start*fps/1000.0-1e-7).ceil().max(0.0) as usize;
        let last=(end*fps/1000.0-1e-7).ceil().max(0.0) as usize;
        for index in first..last.min(output.len()) {
            let local_time=index as f64*1000.0/fps-start;
            let sample=(local_time*CAMERA_SIMULATION_FPS/1000.0+1e-7).floor().max(0.0) as usize;
            if let Some(frame)=frames.get(sample) {
                output[index]=frame.clone();
                output[index].press_amount=press_animation.as_ref().map_or(0.0,|a|a.at(index as f64*1000.0/fps) as f32);
            }
        }
    }
    output
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::video_processing::timeline::EditClip;
    fn clip(id:&str,start:f64,source:f64,duration:f64,rate:f64)->EditClip {
        EditClip{id:id.into(),start_time:start,duration,source_in:source,source_out:source+duration*rate,
            playback_rate:rate,source_file_path:None,track_order:0,mic_volume: 1.0, system_volume: 1.0, muted:false,hide_cursor:false,disable_cursor_smoothing:false}
    }
    fn run(clips:&[EditClip],events:&[CursorEvent],settings:serde_json::Value,end:f64)->Vec<CursorFrameState> {
        let plan=EditTimeline::build(clips,0.0,end).unwrap();
        let physical=events.iter().map(|event|MotionPoint{time_ms:event.timestamp_ms as f64,x:event.x*1000.0,y:event.y*1000.0}).collect::<Vec<_>>();
        simulate(events,&physical,&plan,"recording.mp4",&SpringConfig{tension:170.0,friction:30.0,mass:0.0},
            &serde_json::from_value(settings).unwrap(),&None,120.0)
    }
    #[test]
    fn repeated_source_runs_at_output_speed_with_hidden_gap_and_reset_trails() {
        let events=[CursorEvent{ is_release: false,timestamp_ms:0,x:0.0,y:0.5,is_click:false},CursorEvent{ is_release: false,timestamp_ms:1000,x:1.0,y:0.5,is_click:false}];
        let output=run(&[clip("a",0.0,0.0,500.0,2.0),clip("b",750.0,0.0,2000.0,0.5)],&events,
            serde_json::json!({"trail_enabled":true}),2750.0);
        assert!((output[30].x-0.5).abs()<0.001);
        assert!((output[120].x-0.125).abs()<0.001);
        assert!(output[60..90].iter().all(|frame|frame.opacity==0.0 && frame.trail_count==0.0));
        assert!(output[90].trail_count<=1.0);
        assert_eq!(output[90].x,0.0);
    }
    #[test]
    fn fractional_speed_adjusted_click_is_not_emitted_before_its_tick() {
        let events=[CursorEvent{ is_release: false,timestamp_ms:0,x:0.2,y:0.2,is_click:false},CursorEvent{ is_release: false,timestamp_ms:17,x:0.2,y:0.2,is_click:true}];
        let frames=run(&[clip("a",0.0,0.0,500.0,2.0)],&events,serde_json::json!({"click_effect":"ripple"}),500.0);
        assert_eq!(frames[1].is_clicking,0.0); // 8.333ms precedes the 8.5ms click.
        assert_eq!(frames[2].is_clicking,1.0);
        assert!(frames[3].ripple_progress>0.0);
    }
    #[test]
    fn final_loop_returns_to_first_edited_opening_not_last_clip_opening() {
        let events=[CursorEvent{ is_release: false,timestamp_ms:0,x:0.1,y:0.5,is_click:false},CursorEvent{ is_release: false,timestamp_ms:1000,x:0.8,y:0.5,is_click:false},
            CursorEvent{ is_release: false,timestamp_ms:2000,x:0.9,y:0.5,is_click:false}];
        let frames=run(&[clip("a",0.0,0.0,500.0,1.0),clip("b",500.0,1000.0,1000.0,1.0)],&events,
            serde_json::json!({"loop_to_start":true,"loop_duration_ms":500}),1500.0);
        assert!((frames[60].x-0.8).abs()<0.001);
        assert!((frames.last().unwrap().x-0.1).abs()<0.001);
        assert!(frames.iter().all(|frame|frame.is_clicking==0.0));
    }
    #[test]
    fn display_camera_is_sampled_at_mapped_source_time() {
        let events=[CursorEvent{ is_release: false,timestamp_ms:0,x:0.5,y:0.5,is_click:false}];
        let plan=EditTimeline::build(&[clip("fast",0.0,500.0,500.0,2.0)],0.0,500.0).unwrap();
        let camera=(0..240).map(|index|ZoomFrameState{rect:None,scale:if index<120 {2.0}else{1.0},center_x:0.25,center_y:0.5}).collect();
        let frames=simulate(&events,&[],&plan,"recording.mp4",&SpringConfig{tension:170.0,friction:30.0,mass:0.0},
            &serde_json::from_value(serde_json::json!({})).unwrap(),&Some(camera),120.0);
        assert_eq!(frames[0].x,1.0);
        assert_eq!(frames[30].x,0.5);
        assert_eq!(frames[0].y,0.5);
    }
    #[test]
    fn clip_smoothing_override_is_local_and_preserves_the_recorded_path() {
        let events=[CursorEvent{ is_release: false,timestamp_ms:0,x:0.0,y:0.5,is_click:false},
            CursorEvent{ is_release: false,timestamp_ms:500,x:1.0,y:0.5,is_click:false},
            CursorEvent{ is_release: false,timestamp_ms:1000,x:1.0,y:0.5,is_click:false}];
        let mut raw=clip("raw",1000.0,0.0,1000.0,1.0);
        raw.disable_cursor_smoothing=true;
        let plan=EditTimeline::build(&[clip("smooth",0.0,0.0,1000.0,1.0),raw,
            clip("smooth-again",2000.0,0.0,1000.0,1.0)],0.0,3000.0).unwrap();
        let settings=serde_json::from_value(serde_json::json!({})).unwrap();
        let frames=simulate(&events,&[],&plan,"recording.mp4",
            &SpringConfig{tension:170.0,friction:30.0,mass:1.0},&settings,&None,120.0);
        assert!((frames[150].x-0.5).abs()<0.001);
        assert!(frames[30].x<frames[150].x-0.1);
        for index in 0..120 {
            assert!((frames[index].x-frames[index+240].x).abs()<1e-9);
        }
        assert_eq!(frames[120].x,0.0);
        assert_eq!(frames[240].x,0.0);
    }

}
