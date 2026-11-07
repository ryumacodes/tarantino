//! Reanalyze recorded input without modifying the source or its saved analysis.
use crate::{auto_zoom::{ZoomAnalysis,ZoomProcessor},event_capture::{CaptureSession,EnhancedMouseEvent},input::KeyEvent};
use serde_json::Value;

#[tauri::command]
pub async fn regenerate_auto_zoom_data(video_path:String)->Result<ZoomAnalysis,String> {
    let info=crate::video_processing::VideoProcessor::new().map_err(|e|e.to_string())?
        .get_video_info(&video_path).await.map_err(|e|e.to_string())?;
    let path=crate::recording::artifacts::RecordingArtifacts::new(&video_path).mouse();
    tauri::async_runtime::spawn_blocking(move || {
        let data=std::fs::read_to_string(path).map_err(|e|format!("Cannot read recorded input: {e}"))?;
        analyze(&data,info.duration_ms,(info.width,info.height))
    }).await.map_err(|e|e.to_string())?
}

fn analyze(data:&str,duration:u64,source:(u32,u32))->Result<ZoomAnalysis,String> {
    let value:Value=serde_json::from_str(data).map_err(|e|format!("Invalid recorded input: {e}"))?;
    let events=value.get("mouse_events").unwrap_or(&value).as_array().ok_or("Missing recorded pointer events")?;
    let mut session=CaptureSession::new();
    session.start_time=0;
    session.end_time=Some(duration);
    let dimension=|key:&str,fallback:u32| -> Result<u32,String> {
        let raw=match value.get(key) {Some(v)=>v.as_u64().ok_or_else(||format!("Invalid {key}"))?,None=>fallback as u64};
        u32::try_from(raw).ok().filter(|v|*v>0).ok_or_else(||format!("Invalid {key}"))
    };
    session.metadata.display_resolution=(dimension("display_width",source.0)?,dimension("display_height",source.1)?);
    if let Some(area)=value.get("recording_area").filter(|a|!a.is_null()) {
        let number=|key:&str|area[key].as_f64().filter(|v|v.is_finite()).ok_or_else(||format!("Invalid capture area {key}"));
        let (x,y,w,h)=(number("x")?,number("y")?,number("width")?,number("height")?);
        if x<i32::MIN as f64 || x>i32::MAX as f64 || y<i32::MIN as f64 || y>i32::MAX as f64
            || w<1.0 || h<1.0 || w>u32::MAX as f64 || h>u32::MAX as f64 {
            return Err("Invalid capture area dimensions".into());
        }
        session.metadata.capture_region=Some((x as i32,y as i32,w as u32,h as u32));
    }
    for event in events {
        let base=serde_json::from_value(event.get("base").unwrap_or(event).clone())
            .map_err(|e|format!("Invalid pointer event: {e}"))?;
        session.mouse_events.push(EnhancedMouseEvent{base,window_id:None,app_name:None,is_double_click:false,cluster_id:None});
    }
    session.mouse_events.sort_by_key(|e|e.base.timestamp);
    let mut keys:Vec<KeyEvent>=serde_json::from_value(value.get("key_events").cloned().unwrap_or_else(||serde_json::json!([])))
        .map_err(|e|format!("Invalid keyboard events: {e}"))?;
    keys.sort_by_key(|e|e.timestamp);
    let focus: Vec<crate::input::focus_samples::FocusSample> = serde_json::from_value(value.get("focus_samples").cloned().unwrap_or_else(||serde_json::json!([])))
        .map_err(|e|format!("Invalid focus samples: {e}"))?;
    ZoomProcessor::with_default_config().analyze_session_with_focus(&session,&keys,&focus).map_err(|e|e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    #[test]
    fn saved_drag_reanalysis_uses_capture_geometry_and_final_duration() {
        let input=json!({"display_width":1000,"display_height":500,"scale_factor":2,
            "recording_area":{"x":-100,"y":50,"width":1000,"height":500},
            "mouse_events":[
                {"base":{"timestamp":1000,"x":0,"y":100,"event_type":{"ButtonPress":{"button":"Left"}}}},
                {"base":{"timestamp":6000,"x":400,"y":100,"event_type":"Move"}},
                {"base":{"timestamp":8000,"x":500,"y":100,"event_type":{"ButtonRelease":{"button":"Left"}}}}
            ]});
        let analysis=analyze(&input.to_string(),12000,(2000,1000)).unwrap();
        assert_eq!(analysis.zoom_blocks.len(),1);
        assert_eq!(analysis.zoom_blocks[0].end_time,11000);
        assert_eq!(analysis.zoom_blocks[0].click_x,0.1);
        assert_eq!(analysis.zoom_blocks[0].click_y,0.1);
    }
    #[test]
    fn typing_only_sidecar_can_generate_zoom_and_bad_input_is_reported() {
        let input=json!({"display_width":1000,"display_height":500,"mouse_events":[],
            "key_events":[{"timestamp":2000,"is_typing":true,"is_modifier":false,"caret_x":200,"caret_y":100}]});
        let analysis=analyze(&input.to_string(),10000,(1000,500)).unwrap();
        assert_eq!(analysis.total_clicks,0);
        assert_eq!(analysis.zoom_blocks[0].kind,"typing");
        assert!(analyze("{}",10000,(1000,500)).is_err());
    }
    #[test]
    fn delayed_typing_focus_keeps_observation_time_and_capture_geometry() {
        let mut input=json!({"display_width":1000,"display_height":500,"mouse_events":[],
            "recording_area":{"x":-100,"y":50,"width":1000,"height":500},
            "key_events":[{"timestamp":2000,"is_typing":true,"is_modifier":false,"cursor_x":1500,"cursor_y":100}],
            "focus_samples":[{"timestamp":2120,"trigger_timestamp":2000,"trigger":"key","caret":[100,150]}]});
        let analysis=analyze(&input.to_string(),10000,(1000,500)).unwrap();
        let block=&analysis.zoom_blocks[0];
        assert_eq!(block.kind,"typing");
        assert_eq!(block.centers.len(),1);
        assert_eq!(block.centers[0].time,2120);
        assert_eq!((block.centers[0].x,block.centers[0].y),(0.2,0.2));
        input["key_events"][0]["cursor_x"]=json!(0);
        input["focus_samples"][0]["caret"]=json!([1500,150]);
        assert!(analyze(&input.to_string(),10000,(1000,500)).unwrap().zoom_blocks.is_empty());
    }

    #[test]
    #[ignore = "requires external recorded-input range cases"]
    fn external_zoom_range_cases_match_recorded_input() {
        let path = std::env::var("TARANTINO_ZOOM_RANGE_CASES").unwrap();
        let cases: Value = serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap();
        for case in cases.as_array().unwrap() {
            let input = &case["input"];
            let duration = case["duration"].as_u64().unwrap();
            let result = analyze(&input.to_string(),duration,(1000,500)).unwrap();
            let actual: Vec<_> = result.zoom_blocks.iter()
                .map(|block|[block.start_time,block.end_time]).collect();
            assert_eq!(serde_json::to_value(actual).unwrap(),case["expected"],"case {}",case["name"]);
        }
    }

    #[tokio::test]
    async fn brief_recording_zoom_survives_preview_and_export_sampling() {
        use crate::video_processing::{export,types::ExportSettings,zoom_trajectory};
        let input = json!({"display_width":1000,"display_height":500,"mouse_events":[
            {"base":{"timestamp":0,"x":700,"y":300,"event_type":{"ButtonPress":{"button":"Left"}}}},
            {"base":{"timestamp":1,"x":700,"y":300,"event_type":{"ButtonRelease":{"button":"Left"}}}}
        ]});
        let analysis = analyze(&input.to_string(),1001,(1000,500)).unwrap();
        let block = &analysis.zoom_blocks[0];
        assert_eq!((block.start_time,block.end_time),(1,201));
        let video = std::env::temp_dir().join(format!("brief-zoom-{}.mp4",uuid::Uuid::new_v4()));
        let sidecar = video.with_extension("mouse.json");
        std::fs::write(&sidecar,input.to_string()).unwrap();
        let settings: ExportSettings = serde_json::from_value(json!({
            "source_width":1000,"source_height":500,"capture_mode":"window",
            "screen_motion_style":"focused","zoom_spring_tension":250,
            "zoom_spring_friction":40,"zoom_spring_mass":2.25,
            "zoom_blocks":[{"start_time_ms":block.start_time,"end_time_ms":block.end_time,
                "zoom_level":block.zoom_factor,"center_x":block.center_x,"center_y":block.center_y,
                "kind":block.kind,"centers":block.centers}]
        })).unwrap();
        let preview = super::super::motion::compute_zoom_trajectory(
            video.to_string_lossy().into_owned(),1001,settings.clone()).await.unwrap();
        assert_eq!(preview[0][0],1.0);
        assert!(preview[24][0]>1.2,"short range must produce visible zoom");
        assert!(preview.last().unwrap()[0]<preview[24][0],"camera must exit the short range");
        let activity = export::load_camera_activity(&sidecar,&video,(1000,500),&settings,1001).unwrap();
        let spring = export::get_zoom_spring_config(&settings);
        for fps in [24.0,30.0,60.0,120.0] {
            let frames = zoom_trajectory::simulate_zoom_trajectory_in_viewport(
                settings.zoom_blocks.as_ref().unwrap(),&[],&spring,true,"focused",fps,1001,true,
                zoom_trajectory::CameraViewport::from_settings(&settings,(1000,500)),
                Some(zoom_trajectory::targets::CameraActivity{events:&[],prepared:Some(&activity),smooth:true}));
            for (index,frame) in frames.iter().enumerate() {
                let source = (index as f64*120.0/fps).round() as usize;
                assert_eq!(preview[source],frame.sample());
            }
        }
        std::fs::remove_file(sidecar).unwrap();
    }

}
