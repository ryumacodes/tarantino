use crate::cursor_renderer::{CursorEvent,SpringConfig,rotation::MotionPoint};
use crate::video_processing::{CursorSettings,ExportSettings,timeline::EditClip};

#[derive(serde::Deserialize)]
#[serde(rename_all="camelCase")]
pub struct EditedCursorRequest {
    pub recording_path:String,
    pub clips:Vec<EditClip>,
    pub trim_start:u64,
    pub trim_end:u64,
}

pub(super) fn prepare(request:&EditedCursorRequest,events:&[CursorEvent],physical:&[MotionPoint],
    settings:&CursorSettings,spring:&SpringConfig,duration:u64,fps:u32, activity: Option<&crate::cursor_renderer::typing_activity::TypingActivity>)->Result<Option<String>,String> {
    prepare_with_input(request,events,|| physical.to_vec(),settings,spring,duration,fps,activity,None)
}

pub(super) fn prepare_with_input(request:&EditedCursorRequest,events:&[CursorEvent],physical:impl FnOnce() -> Vec<MotionPoint>,
    settings:&CursorSettings,spring:&SpringConfig,duration:u64,fps:u32,activity:Option<&crate::cursor_renderer::typing_activity::TypingActivity>,
    input:Option<(&std::path::Path,(u32,u32),Option<crate::video_processing::source_crop::SourceCrop>)>)->Result<Option<String>,String> {
    let export:ExportSettings=serde_json::from_value(serde_json::json!({
        "timeline_clips":request.clips,"trim_start":request.trim_start,"trim_end":request.trim_end,"cursor_settings":settings
    })).map_err(|error|error.to_string())?;
    let plan=crate::video_processing::export::timeline_support::resolve_plan(&export,
        std::path::Path::new(&request.recording_path),duration).map_err(|error|error.to_string())?;
    let plan=crate::video_processing::export::timeline_support::cursor_plan(&export,plan.as_ref()).map_err(|error|error.to_string())?;
    let Some(plan)=plan else {return Ok(None);};
    let planned=input.map(|(path,size,crop)|crate::cursor_renderer::planner::runtime::prepare(path,size,crop,Some(&plan),
        std::path::Path::new(&request.recording_path),duration as f64,settings,*spring,None)).transpose().map_err(|e|e.to_string())?.flatten();
    let physical = if settings.rotate_while_moving.unwrap_or(false)
        && planned.as_ref().and_then(|path| path.rotation_keys()).is_none() {
        physical()
    } else { Vec::new() };
    let frames=crate::cursor_renderer::edited_simulation::simulate_with_planned_path(events,&physical,&plan,&request.recording_path,
        spring,settings,&None,fps as f64,activity,planned.as_ref(),60.0);
    let response = EditedTrajectory {
        frames: &frames,
        origin_ms: request.trim_start,
        output_time: true,
        segments: plan.segments.iter().map(|segment| [segment.output_start_ms, segment.output_end_ms]).collect(),
    };
    serde_json::to_string(&response).map(Some).map_err(|error| error.to_string())
}

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct EditedTrajectory<'a> {
    #[serde(serialize_with = "serialize_frames")]
    frames: &'a [crate::cursor_renderer::CursorFrameState],
    origin_ms: u64,
    output_time: bool,
    segments: Vec<[f64; 2]>,
}

fn serialize_frames<S: serde::Serializer>(
    frames: &&[crate::cursor_renderer::CursorFrameState],
    serializer: S,
) -> Result<S::Ok, S::Error> {
    serializer.collect_seq(frames.iter().map(|p| {
        [p.x, p.y, p.opacity, p.is_clicking, p.ripple_progress, p.ripple_x,
            p.ripple_y, p.rotation, p.fade_blur_radius, p.press_amount].map(f64::from)
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn edited_response_preserves_numeric_precision_and_frame_columns() {
        let mut frame = crate::cursor_renderer::CursorFrameState::default();
        frame.x = 0.12345679;
        frame.y = -0.2345679;
        frame.opacity = 0.3456789;
        frame.is_clicking = 1.0;
        frame.ripple_progress = 0.456789;
        frame.ripple_x = 0.56789;
        frame.ripple_y = 0.6789;
        frame.rotation = -112.56789;
        frame.fade_blur_radius = 2.345678;
        frame.press_amount = 0.0789123;
        let expected = serde_json::json!({
            "frames": [[frame.x, frame.y, frame.opacity, frame.is_clicking,
                frame.ripple_progress, frame.ripple_x, frame.ripple_y, frame.rotation,
                frame.fade_blur_radius, frame.press_amount]],
            "originMs": 125, "outputTime": true, "segments": [[0.0, 500.5]]
        });
        let frames = [frame];
        let response = EditedTrajectory {
            frames: &frames, origin_ms: 125, output_time: true, segments: vec![[0.0, 500.5]],
        };
        let actual: serde_json::Value = serde_json::from_str(&serde_json::to_string(&response).unwrap()).unwrap();
        assert_eq!(actual, expected);
    }

    #[test]
    fn planned_rotation_does_not_load_a_second_source_path() {
        let path = std::env::temp_dir().join(format!("cursor-plan-{}.json", uuid::Uuid::new_v4()));
        let input = serde_json::json!({"display_width":100,"display_height":100,"mouse_events":[
            {"timestamp":0,"x":10,"y":20,"event_type":"Move"},
            {"timestamp":500,"x":80,"y":60,"event_type":"Move"}
        ]});
        std::fs::write(&path, input.to_string()).unwrap();
        let request: EditedCursorRequest = serde_json::from_value(serde_json::json!({
            "recordingPath":"recording.mp4","trimStart":0,"trimEnd":500,"clips":[
                {"id":"fast","startTime":0,"duration":500,"sourceIn":0,"sourceOut":1000,
                 "playbackRate":2,"trackOrder":0,"muted":false}
            ]
        })).unwrap();
        let events = crate::video_processing::export::parse_filtered_cursor_events(&input,100,100,0.0);
        for mode in ["horizontal", "directional"] {
            let settings: CursorSettings = serde_json::from_value(serde_json::json!({
                "rotate_while_moving":true,"rotation_mode":mode
            })).unwrap();
            let response = prepare_with_input(&request,&events,
                || panic!("planned rotation must not re-read the sidecar"),
                &settings,&SpringConfig{tension:470.0,friction:70.0,mass:3.0},1000,120,None,
                Some((&path,(100,100),None))).unwrap().unwrap();
            let value: serde_json::Value = serde_json::from_str(&response).unwrap();
            assert_eq!(value["frames"].as_array().unwrap().len(),60);
        }
        std::fs::remove_file(path).unwrap();
    }

    #[tokio::test]
    #[ignore = "opt-in recorded preview command profile"]
    async fn recorded_preview_command_profile() {
        let path = std::env::var("TARANTINO_CURSOR_PROFILE_INPUT").unwrap();
        let duration = std::env::var("TARANTINO_CURSOR_PROFILE_DURATION").unwrap().parse().unwrap();
        let data: serde_json::Value = serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
        let width = data["display_width"].as_u64().unwrap() as u32;
        let height = data["display_height"].as_u64().unwrap() as u32;
        let start = std::time::Instant::now();
        let result = super::super::compute_cursor_trajectory(path, duration, 120, width, height,
            1.0, false, 2000, false, 0, false, None, None, Some(470.0), Some(70.0), Some(3.0),
            Some(0.0), Some(true), Some(30.0), Some("horizontal".into()), None, None, Some(false), None)
            .await.unwrap();
        let command_ms = start.elapsed().as_secs_f64() * 1000.0;
        let frames: Vec<[f64; 10]> = serde_json::from_str(&result).unwrap();
        assert_eq!(frames.len(), (duration as f64 * 120.0 / 1000.0).ceil() as usize);
        assert!(frames.iter().flatten().all(|value| value.is_finite()));
        println!("PREVIEW_PROFILE duration_ms={duration} frames={} bytes={} command_ms={command_ms:.3}",
            frames.len(), result.len());
        if let Ok(output) = std::env::var("TARANTINO_CURSOR_PROFILE_OUTPUT") {
            std::fs::write(output, result).unwrap();
        }
    }

    #[tokio::test]
    async fn preview_trajectory_is_independent_of_rendered_cursor_size() {
        let path=std::env::temp_dir().join(format!("cursor-size-{}.json",uuid::Uuid::new_v4()));
        let samples=(0..=100).map(|i|serde_json::json!({"timestamp":i*10,"x":20.0+i as f64*0.5,"y":20,
            "event_type":if i==50 {serde_json::json!({"ButtonPress":{"button":"Left"}})}
                else if i==55 {serde_json::json!({"ButtonRelease":{"button":"Left"}})}
                else {serde_json::json!("Move")}})).collect::<Vec<_>>();
        std::fs::write(&path,serde_json::to_vec(&samples).unwrap()).unwrap();
        let mut results=Vec::new();
        for size in [1.0,5.0] {
            results.push(super::super::compute_cursor_trajectory(path.to_string_lossy().into_owned(),1000,120,100,100,
                size,false,2000,false,0,false,None,None,Some(470.0),Some(70.0),Some(3.0),Some(0.0),Some(true),Some(30.0),
                Some("directional".into()),None,None,Some(false),None).await.unwrap());
        }
        assert_eq!(results[0],results[1]);
        let frames:Vec<[f64;10]>=serde_json::from_str(&results[0]).unwrap();
        assert_eq!(frames.len(),120);
        assert!(frames.iter().any(|frame|frame[4]>0.0));
        assert!(frames.last().unwrap()[0]>frames[0][0]);
        std::fs::remove_file(path).unwrap();
    }

    #[tokio::test]
    async fn command_returns_output_time_frames_for_repeated_speed_adjusted_clips() {
        let path=std::env::temp_dir().join(format!("edited-cursor-{}.json",uuid::Uuid::new_v4()));
        let samples=(0..=50).map(|i|serde_json::json!({"timestamp":i*10,"x":20.0+i as f64*1.2,"y":20})).collect::<Vec<_>>();
        std::fs::write(&path,serde_json::to_vec(&samples).unwrap()).unwrap();
        let request:EditedCursorRequest=serde_json::from_value(serde_json::json!({
            "recordingPath":"recording.mp4","trimStart":0,"trimEnd":1500,
            "clips":[
                {"id":"fast","startTime":0,"duration":250,"sourceIn":0,"sourceOut":500,"playbackRate":2,"trackOrder":0,"muted":false},
                {"id":"slow","startTime":500,"duration":1000,"sourceIn":0,"sourceOut":500,"playbackRate":0.5,"trackOrder":0,"muted":false}
            ]
        })).unwrap();
        let result=super::super::compute_cursor_trajectory(path.to_string_lossy().into_owned(),1000,120,100,100,
            1.0,false,2000,false,0,false,None,None,Some(170.0),Some(30.0),Some(0.0),Some(0.0),Some(false),Some(100.0), None,Some(request),None,Some(false), None).await.unwrap();
        let value:serde_json::Value=serde_json::from_str(&result).unwrap();
        assert_eq!(value["outputTime"],true);
        assert_eq!(value["originMs"],0);
        let frames=value["frames"].as_array().unwrap();
        assert_eq!(frames.len(),180);
        assert!((frames[15][0].as_f64().unwrap()-0.5).abs()<0.001);
        assert!(frames[30..60].iter().all(|frame|frame[2].as_f64()==Some(0.0) && frame[8].as_f64()==Some(0.0)));
        assert!(frames.iter().all(|frame|frame.as_array().unwrap().len()==10));
        assert!((frames[60][0].as_f64().unwrap()-0.2).abs()<0.001);
        assert!((frames[90][0].as_f64().unwrap()-0.35).abs()<0.001);
        std::fs::remove_file(path).unwrap();
    }
}

#[cfg(test)]
mod trim_tests {
    use super::*;
    #[test]
    fn contiguous_trim_loop_returns_to_the_edited_opening() {
        let request:EditedCursorRequest=serde_json::from_value(serde_json::json!({
            "recordingPath":"recording.mp4","trimStart":250,"trimEnd":750,"clips":[
                {"id":"one","startTime":0,"duration":1000,"sourceIn":0,"sourceOut":1000,"playbackRate":1,"trackOrder":0,"muted":false}
            ]
        })).unwrap();
        let events=[CursorEvent{ is_release: false,timestamp_ms:0,x:0.0,y:0.5,is_click:false},CursorEvent{ is_release: false,timestamp_ms:500,x:0.5,y:0.5,is_click:false},
            CursorEvent{ is_release: false,timestamp_ms:1000,x:1.0,y:0.5,is_click:false}];
        let settings=serde_json::from_value(serde_json::json!({"loop_to_start":true,"loop_duration_ms":100})).unwrap();
        let result=prepare(&request,&events,&[],&settings,&SpringConfig{tension:170.0,friction:30.0,mass:0.0},1000,120,None).unwrap().unwrap();
        let result:serde_json::Value=serde_json::from_str(&result).unwrap();
        assert_eq!(result["originMs"],250);
        let frames=result["frames"].as_array().unwrap();
        assert_eq!(frames.len(),60);
        assert_eq!(frames[0][0].as_f64(),Some(0.25));
        assert_eq!(frames.last().unwrap()[0].as_f64(),Some(0.25));
        assert!(frames.iter().any(|frame|frame[0].as_f64().unwrap()>0.4));
    }
}

#[cfg(test)]
mod smoothing_override_tests {
    use super::*;
    #[test]
    fn preview_request_uses_the_clip_override_even_without_media_edits() {
        let request:EditedCursorRequest=serde_json::from_value(serde_json::json!({
            "recordingPath":"recording.mp4","trimStart":0,"trimEnd":1000,"clips":[
                {"id":"one","startTime":0,"duration":1000,"sourceIn":0,"sourceOut":1000,
                 "playbackRate":1,"trackOrder":0,"muted":false,"disableCursorSmoothing":true}
            ]
        })).unwrap();
        let events=[CursorEvent{ is_release: false,timestamp_ms:0,x:0.0,y:0.5,is_click:false},
            CursorEvent{ is_release: false,timestamp_ms:500,x:1.0,y:0.5,is_click:false}];
        let settings=serde_json::from_value(serde_json::json!({})).unwrap();
        let result=prepare(&request,&events,&[],&settings,
            &SpringConfig{tension:170.0,friction:30.0,mass:1.0},1000,120,None).unwrap().unwrap();
        let result:serde_json::Value=serde_json::from_str(&result).unwrap();
        assert_eq!(result["outputTime"],true);
        assert!((result["frames"][30][0].as_f64().unwrap()-0.5).abs()<0.001);
    }
}

#[cfg(test)]
mod typing_tests {
    use super::*;
    #[test]
    fn typing_visibility_follows_repeated_speed_adjusted_clips_and_empty_gaps() {
        let path=std::env::temp_dir().join(format!("edited-typing-{}.json",uuid::Uuid::new_v4()));
        std::fs::write(&path,serde_json::json!({"mouse_events":[{"timestamp":0},{"timestamp":1000}],
            "key_events":[{"timestamp":100,"is_typing":true},{"timestamp":200,"is_typing":true},
                {"timestamp":300,"is_typing":true}]}).to_string()).unwrap();
        let activity=crate::cursor_renderer::typing_activity::TypingActivity::load(&path);
        let request:EditedCursorRequest=serde_json::from_value(serde_json::json!({
            "recordingPath":"recording.mp4","trimStart":0,"trimEnd":5500,"clips":[
                {"id":"fast","startTime":0,"duration":1000,"sourceIn":0,"sourceOut":2000,"playbackRate":2,"trackOrder":0,"muted":false},
                {"id":"slow","startTime":1500,"duration":4000,"sourceIn":0,"sourceOut":2000,"playbackRate":0.5,"trackOrder":0,"muted":false}
            ]})).unwrap();
        let events=[CursorEvent{ is_release: false,timestamp_ms:0,x:0.2,y:0.2,is_click:false},
            CursorEvent{ is_release: false,timestamp_ms:1000,x:0.3,y:0.2,is_click:false}];
        let settings=serde_json::from_value(serde_json::json!({})).unwrap();
        let spring=SpringConfig{tension:170.0,friction:30.0,mass:1.0};
        let result=prepare(&request,&events,&[],&settings,&spring,2000,120,Some(&activity)).unwrap().unwrap();
        let result:serde_json::Value=serde_json::from_str(&result).unwrap();
        let frames=result["frames"].as_array().unwrap();
        let opacity=|time:usize| frames[time*120/1000][2].as_f64().unwrap();
        assert!(opacity(400)<0.05);
        assert!(opacity(900)>0.9);
        assert_eq!(opacity(1200),0.0);
        assert!((opacity(1500)-0.01446750122588123).abs()<1e-7);
        assert!(opacity(2600)<0.01);
        assert!(opacity(4500)>0.95);
        let plan=crate::video_processing::timeline::EditTimeline::build(&request.clips,0.0,5500.0).unwrap();
        let exported=crate::cursor_renderer::edited_simulation::simulate_with_activity(
            &events,&[],&plan,&request.recording_path,&spring,&settings,&None,120.0,Some(&activity));
        for (actual,expected) in frames.iter().zip(exported) {
            assert!((actual[2].as_f64().unwrap()-expected.opacity as f64).abs()<1e-7);
        }
        std::fs::remove_file(path).unwrap();
    }
}
