//! Cursor simulation in output time for edited exports.
use super::{CursorEvent,CursorFrameState,SpringConfig};
use super::{edited_path::{map_cursor,map_recording},rotation::MotionPoint};
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
    let mut motion_timeline=timeline.clone();
    for segment in &mut motion_timeline.segments {segment.hide_cursor=false;}
    let segments=map_cursor(events,&motion_timeline,recording);
    let physical_segments=map_recording(physical,&motion_timeline,recording);
    let count=(timeline.duration_ms*fps/1000.0).ceil() as usize;
    let mut output=vec![CursorFrameState::default();count];
    let opening=segments.first().and_then(|segment|segment.motion.points.first()).copied();
    let mut rotation=super::edited_rotation::EditedRotation::new(physical,&physical_segments,planned,settings,timeline.duration_ms);
    let loop_cutoff=timeline.duration_ms-settings.loop_duration_ms.unwrap_or(500) as f64;
    let freeze_time=if settings.stop_at_end.unwrap_or(false) {
        (timeline.duration_ms-settings.stop_duration_ms.unwrap_or(0) as f64).max(0.0)
    } else {timeline.duration_ms};
    let visibility_targets=|time:f64| {
        let mapped_time=time.min(freeze_time);
        let index=segments.partition_point(|segment|segment.motion.output_end_ms<=mapped_time);
        let segment=segments.get(index).or_else(||
            (mapped_time>=timeline.duration_ms).then(||segments.last()).flatten());
        let Some(segment)=segment else {return (false,false,false);};
        let motion=&segment.motion;
        if mapped_time<motion.output_start_ms {return (false,false,false);}
        let source=motion.source_start_ms+(mapped_time.min(motion.output_end_ms)-motion.output_start_ms)*motion.rate;
        let count=events.partition_point(|event|event.timestamp_ms as f64<=source);
        let latest=count.checked_sub(1).map(|index|&events[index]);
        let idle=settings.hide_when_idle.unwrap_or(false) && latest.is_some_and(|event|
            source-event.timestamp_ms as f64+(time-freeze_time).max(0.0)*motion.rate>settings.idle_timeout.unwrap_or(2000) as f64);
        (latest.is_some(),idle,activity.is_some_and(|a|a.active(source)))
    };
    let mut visibility=super::simulation::visibility::VisibilityTimeline::new();
    for segment in &segments {
        let motion=&segment.motion;
        if timeline.source_at(motion.output_start_ms).is_some_and(|(segment,_)|segment.hide_cursor) {continue;}
        let start=motion.output_start_ms;
        let end=motion.output_end_ms;
        let duration=(end-start).ceil() as u64;
        let mut local=settings.clone();
        // The closing behavior belongs to the edited output, not every cut.
        local.loop_to_start=Some(false);
        local.rotate_while_moving=Some(false);
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
        let local_zoom=source_zoom.as_ref().filter(|frames|!frames.is_empty()).map(|frames| {
            (0..(duration as f64*CAMERA_SIMULATION_FPS/1000.0).ceil() as usize).map(|index| {
                let source=motion.source_start_ms+index as f64*1000.0/CAMERA_SIMULATION_FPS*motion.rate;
                crate::video_processing::zoom_trajectory::sample_camera_frame(frames, source).unwrap()
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
            &local_zoom,&local,0,CAMERA_SIMULATION_FPS,None,activity.map(|_| &typing as &dyn Fn(f64)->bool),
            planned.map(|_|&position as &dyn Fn(f64)->[f64;2]),None,Clone::clone);
        let first=(start*fps/1000.0-1e-7).ceil().max(0.0) as usize;
        let last=(end*fps/1000.0-1e-7).ceil().max(0.0) as usize;
        for index in first..last.min(output.len()) {
            let local_time=index as f64*1000.0/fps-start;
            let position = (local_time * CAMERA_SIMULATION_FPS / 1000.0).max(0.0);
            let position = if (position-position.round()).abs()<1e-7 {position.round()} else {position};
            let sample = position.floor() as usize;
            if let Some(frame)=frames.get(sample) {
                output[index]=frames.get(sample+1).map_or_else(||frame.clone(),
                    |next|frame.interpolate(next,(position-sample as f64) as f32));
                if let Some(planned) = planned {
                    let mut position = planned.at(index as f64*1000.0/fps);
                    let source_time = motion.source_start_ms+local_time*motion.rate;
                    if let Some(zoom) = source_zoom.as_ref().and_then(|frames|
                        crate::video_processing::zoom_trajectory::sample_camera_frame(frames,source_time)) {
                        if zoom.scale>1.0 {
                            position[0]=(position[0]-zoom.center_x)*zoom.scale+0.5;
                            position[1]=(position[1]-zoom.center_y)*zoom.scale+0.5;
                        }
                    }
                    output[index].x=position[0] as f32;
                    output[index].y=position[1] as f32;
                }
                let time=index as f64*1000.0/fps;
                if let Some(rotation)=rotation.as_mut() {output[index].rotation=rotation.at(time);}
                let [opacity,blur]=visibility.sample(time,&visibility_targets);
                let has_pointer=visibility_targets(time).0;
                output[index].set_visibility(if has_pointer {opacity as f32} else {0.0},
                    if has_pointer {blur as f32} else {0.0},settings.trail_opacity.unwrap_or(0.5) as f32);
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
    fn splitting_a_moving_clip_preserves_rotation_target_and_momentum() {
        let physical=(0..=200).map(|i|MotionPoint{time_ms:i as f64*10.0,
            x:if i<60 {i as f64*10.0} else {600.0-(i-60) as f64*4.0},y:100.0}).collect::<Vec<_>>();
        let events=physical.iter().map(|p|CursorEvent{timestamp_ms:p.time_ms as u64,x:p.x/1000.0,y:0.1,
            is_click:false,is_release:false}).collect::<Vec<_>>();
        let whole=EditTimeline::build(&[clip("whole",0.0,0.0,2000.0,1.0)],0.0,2000.0).unwrap();
        let split=EditTimeline::build(&[clip("a",0.0,0.0,737.25,1.0),
            clip("b",737.25,737.25,1262.75,1.0)],0.0,2000.0).unwrap();
        let settings=serde_json::from_value(serde_json::json!({"rotate_while_moving":true,"rotation_intensity":100,"rotation":7})).unwrap();
        let spring=SpringConfig{tension:170.0,friction:30.0,mass:0.0};
        for fps in [24.0,60.0,120.0,144.0] {
            let run=|plan:&EditTimeline|simulate(&events,&physical,plan,"recording.mp4",&spring,&settings,&None,fps);
            if fps==24.0 {
                assert!((run(&split)[18].rotation as f64-13.899027884665369).abs()<1e-5);
            }
            for (index,(a,b)) in run(&whole).iter().zip(run(&split)).enumerate() {
                assert!((a.rotation-b.rotation).abs()<1e-5,"{fps}fps frame{index}: {} != {}",a.rotation,b.rotation);
            }
        }
    }

    #[test]
    fn splitting_during_typing_keeps_fade_and_blur_progress() {
        let path=std::env::temp_dir().join(format!("cursor-fade-cut-{}.json",uuid::Uuid::new_v4()));
        std::fs::write(&path,serde_json::json!({"mouse_events":[{"timestamp":0},{"timestamp":700}],
            "key_events":[{"timestamp":100},{"timestamp":200},{"timestamp":300}]}).to_string()).unwrap();
        let activity=super::super::typing_activity::TypingActivity::load(&path);
        let events=[CursorEvent{is_release:false,timestamp_ms:0,x:0.2,y:0.3,is_click:false},
            CursorEvent{is_release:false,timestamp_ms:700,x:0.4,y:0.3,is_click:false}];
        let settings=serde_json::from_value(serde_json::json!({})).unwrap();
        let spring=SpringConfig{tension:170.0,friction:30.0,mass:0.0};
        let whole=EditTimeline::build(&[clip("whole",0.0,0.0,1000.0,1.0)],0.0,1000.0).unwrap();
        let split=EditTimeline::build(&[clip("a",0.0,0.0,337.25,1.0),
            clip("b",337.25,337.25,662.75,1.0)],0.0,1000.0).unwrap();
        for fps in [24.0,60.0,120.0,144.0] {
            let run=|timeline:&EditTimeline|simulate_with_activity(&events,&[],timeline,"recording.mp4",
                &spring,&settings,&None,fps,Some(&activity));
            if fps==24.0 {
                let frames=run(&split);
                assert!((frames[9].opacity as f64-0.36043692705320135).abs()<1e-6);
                assert!((frames[9].fade_blur_radius as f64-2.585170331737275).abs()<1e-6);
            }
            for (index,(a,b)) in run(&whole).iter().zip(run(&split)).enumerate() {
                assert!((a.opacity-b.opacity).abs()<1e-6,"{fps}fps frame{index}: {} != {}",a.opacity,b.opacity);
                assert!((a.fade_blur_radius-b.fade_blur_radius).abs()<1e-6);
            }
        }
        std::fs::remove_file(path).unwrap();
    }

    #[test]
    fn fractional_clip_start_keeps_planned_position_in_output_time() {
        let sidecar = std::env::temp_dir().join(format!("cursor-fractional-{}.json",uuid::Uuid::new_v4()));
        let events = (0..=120).map(|i| CursorEvent { timestamp_ms:i*10,
            x:0.1+(i.min(120-i) as f64)*0.005,y:0.3,is_click:false,is_release:false }).collect::<Vec<_>>();
        let data = events.iter().map(|e|serde_json::json!({"timestamp":e.timestamp_ms,
            "x":e.x*1000.0,"y":e.y*1000.0,"event_type":"Move"})).collect::<Vec<_>>();
        std::fs::write(&sidecar,serde_json::to_vec(&data).unwrap()).unwrap();
        let timeline = EditTimeline::build(&[clip("offset",3.25,0.0,500.0,1.5)],0.0,503.25).unwrap();
        let settings = serde_json::from_value(serde_json::json!({"follow_hover":false})).unwrap();
        let spring = SpringConfig { tension:530.0,friction:40.0,mass:1.0 };
        let planned = super::super::planner::runtime::prepare(&sidecar,(1000,1000),None,Some(&timeline),
            &sidecar,1200.0,&settings,spring,None).unwrap().unwrap();
        for zoomed in [false,true] {
        let zoom = zoomed.then(|| (0..145).map(|index| ZoomFrameState { rect:None,
            scale:1.0+index as f64*0.003,center_x:0.6,center_y:0.4 }).collect::<Vec<_>>());
        for fps in [24.0,60.0,120.0,144.0] {
            let output = simulate_with_planned_path(&events,&[],&timeline,sidecar.to_str().unwrap(),
                &spring,&settings,&zoom,fps,None,Some(&planned),60.0);
            for (index,frame) in output.iter().enumerate() {
                let time = index as f64*1000.0/fps;
                if time<3.25 { assert_eq!(frame.opacity,0.0); continue; }
                let mut expected = planned.at(time);
                if zoomed {
                    let source_time = (time-3.25)*1.5;
                    let scale = 1.0+source_time*120.0/1000.0*0.003;
                    expected=[(expected[0]-0.6)*scale+0.5,(expected[1]-0.4)*scale+0.5];
                }
                assert!((frame.x as f64-expected[0]).abs()<1e-7,"{fps}fps {time}ms: {} != {}",frame.x,expected[0]);
                assert!((frame.y as f64-expected[1]).abs()<1e-7);
            }
        }
        }
        std::fs::remove_file(sidecar).unwrap();
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
