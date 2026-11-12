use super::{compute_cursor_trajectory, edited_cursor::EditedCursorRequest};
use serde_json::{Value, json};

#[tokio::test]
async fn bounded_preview_preserves_full_motion_across_cuts_loops_and_chunk_edges() {
    let path = std::env::temp_dir().join(format!("cursor-prefix-{}.json", uuid::Uuid::new_v4()));
    let events: Vec<_> = (0..=750).map(|i| json!({
        "timestamp":i*40,"x":50.0+(i as f64/13.0).sin()*45.0,
        "y":50.0+(i as f64/19.0).cos()*40.0,
        "event_type": if i%90==0 {json!({"ButtonPress":{"button":"Left"}})}
            else if i%90==50 {json!({"ButtonRelease":{"button":"Left"}})} else {json!("Move")}
    })).collect();
    std::fs::write(&path,json!({"display_width":100,"display_height":100,"mouse_events":events,
        "key_events":[{"timestamp":2000,"is_typing":true},{"timestamp":2100,"is_typing":true}],
        "cursor_images":{"images":[{},{}],"changes":[{"timestamp":0,"image_id":0},{"timestamp":4990,"image_id":1}]}
    }).to_string()).unwrap();
    for edited in [false,true] {
        for mode in ["horizontal","directional"] {
            for looped in [false,true] {
                let run = |until, job| {
                    let request: Option<EditedCursorRequest> = edited.then(|| serde_json::from_value(json!({
                        "recordingPath":"recording.mp4","trimStart":0,"trimEnd":22000,
                        "clips":[
                            {"id":"a","startTime":0,"duration":8000.25,"sourceIn":500.5,"sourceOut":8500.75,"playbackRate":1,"trackOrder":0,"muted":false},
                            {"id":"b","startTime":8500.5,"duration":6000,"sourceIn":10000,"sourceOut":22000,"playbackRate":2,"trackOrder":0,"muted":false},
                            {"id":"c","startTime":14500.5,"duration":7499.5,"sourceIn":7000,"sourceOut":10749.75,"playbackRate":0.5,"trackOrder":0,"muted":false}
                        ]
                    })).unwrap());
                    compute_cursor_trajectory(path.to_string_lossy().into_owned(),30000,120,100,100,
                        1.0,true,2000,true,2000,looped,None,None,Some(470.0),Some(70.0),Some(3.0),
                        Some(7.0),Some(true),Some(30.0),Some(mode.into()),request,None,Some(false),None,until, job)
                };
                let full: Value = serde_json::from_str(&run(None, None).await.unwrap()).unwrap();
                let full = full.get("frames").unwrap_or(&full).as_array().unwrap();
                let job = crate::commands::cursor_preview_jobs::begin(&uuid::Uuid::new_v4().to_string());
                for until in [4999,9999,10000,12501,21900,29900] {
                    let prefix: Value = serde_json::from_str(&run(Some(until), Some(job.clone())).await.unwrap()).unwrap();
                    let prefix = prefix.get("frames").unwrap_or(&prefix).as_array().unwrap();
                    assert_eq!(prefix.len(),((until as f64*0.12).ceil() as usize).min(full.len()));
                    for (i,frame) in prefix.iter().enumerate() {
                        assert_eq!(frame,&full[i],"edited={edited} mode={mode} loop={looped} until={until} frame={i}");
                    }
                }
                crate::commands::cursor_preview_jobs::finish(&job);
            }
        }
    }
    std::fs::remove_file(path).unwrap();
}

#[tokio::test]
#[ignore = "opt-in recorded bounded preview performance comparison"]
async fn recorded_bounded_preview_profile() {
    let path = std::env::var("TARANTINO_CURSOR_PROFILE_INPUT").unwrap();
    let duration: u64 = std::env::var("TARANTINO_CURSOR_PROFILE_DURATION").unwrap().parse().unwrap();
    let data: Value = serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
    let width = data["display_width"].as_u64().unwrap() as u32;
    let height = data["display_height"].as_u64().unwrap() as u32;
    drop(data);
    for edited in [false,true] {
      for cached in [false,true] {
        let job = cached.then(|| crate::commands::cursor_preview_jobs::begin(&uuid::Uuid::new_v4().to_string()));
        let mut results = Vec::new();
        for until in [Some(5000),None] {
            let edit = edited.then(|| serde_json::from_value(json!({
                "recordingPath":"recording.mp4","trimStart":0,"trimEnd":duration/2,
                "clips":[{"id":"whole","startTime":0,"duration":duration as f64/2.0,"sourceIn":0,
                    "sourceOut":duration,"playbackRate":2,"trackOrder":0,"muted":false}]
            })).unwrap());
            let start = std::time::Instant::now();
            let response = compute_cursor_trajectory(path.clone(),duration,120,width,height,
                1.0,false,2000,false,0,false,None,None,Some(470.0),Some(70.0),Some(3.0),
                Some(0.0),Some(true),Some(30.0),Some("horizontal".into()),edit,None,Some(false),None,until, job.clone())
                .await.unwrap();
            println!("BOUNDED_PREVIEW cached={cached} edited={edited} until={until:?} ms={:.3} bytes={}",
                start.elapsed().as_secs_f64()*1000.0,response.len());
            let mut value: Value = serde_json::from_str(&response).unwrap();
            let frames = if edited { value["frames"].take() } else { value };
            results.push(frames.as_array().unwrap().to_vec());
        }
        assert_eq!(results[0].len(),600);
        assert_eq!(results[0],results[1][..600]);
      }
    }
}

#[tokio::test]
async fn contiguous_trim_prefix_uses_the_returned_trajectory_clock() {
    let path = std::env::temp_dir().join(format!("cursor-trim-prefix-{}.json",uuid::Uuid::new_v4()));
    std::fs::write(&path,json!({"mouse_events":[
        {"timestamp":0,"x":20,"y":30,"event_type":"Move"},
        {"timestamp":12000,"x":80,"y":70,"event_type":"Move"},
        {"timestamp":30000,"x":30,"y":20,"event_type":"Move"}
    ]}).to_string()).unwrap();
    for looped in [false,true] {
        let run = |until| {
            let edit = serde_json::from_value(json!({
                "recordingPath":"recording.mp4","trimStart":10000,"trimEnd":20000,
                "clips":[{"id":"whole","startTime":0,"duration":30000,"sourceIn":0,
                    "sourceOut":30000,"playbackRate":1,"trackOrder":0,"muted":false}]
            })).unwrap();
            compute_cursor_trajectory(path.to_string_lossy().into_owned(),30000,120,100,100,
                1.0,false,2000,false,0,looped,None,None,Some(470.0),Some(70.0),Some(3.0),
                Some(0.0),Some(true),Some(30.0),Some("horizontal".into()),Some(edit),None,Some(false),None,until, None)
        };
        let full: Value = serde_json::from_str(&run(None).await.unwrap()).unwrap();
        let prefix: Value = serde_json::from_str(&run(Some(5000)).await.unwrap()).unwrap();
        assert_eq!(prefix.is_object(),looped);
        if looped {
            assert_eq!(prefix["originMs"],10000);
            assert_eq!(prefix["outputTime"],true);
        }
        let full = full.get("frames").unwrap_or(&full).as_array().unwrap();
        let prefix = prefix.get("frames").unwrap_or(&prefix).as_array().unwrap();
        let expected = if looped {600} else {1800};
        assert_eq!(prefix.len(),expected);
        assert_eq!(prefix,&full[..expected]);
    }
    std::fs::remove_file(path).unwrap();
}

#[tokio::test]
#[ignore = "opt-in recorded seek replacement command profile"]
async fn recorded_seek_replacement_preserves_frames_and_cancels_full_work() {
    let path = std::env::var("TARANTINO_CURSOR_PROFILE_INPUT").unwrap();
    let duration: u64 = std::env::var("TARANTINO_CURSOR_PROFILE_DURATION").unwrap().parse().unwrap();
    let data: Value = serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
    let width = data["display_width"].as_u64().unwrap() as u32;
    let height = data["display_height"].as_u64().unwrap() as u32;
    drop(data);
    assert!(duration > 37000);
    for edited in [false, true] {
        let run = |until, job| {
            let edit = edited.then(|| serde_json::from_value(json!({
                "recordingPath":"recording.mp4","trimStart":0,"trimEnd":duration/2,
                "clips":[{"id":"whole","startTime":0,"duration":duration as f64/2.0,
                    "sourceIn":0,"sourceOut":duration,"playbackRate":2,"trackOrder":0,"muted":false}]
            })).unwrap());
            compute_cursor_trajectory(path.clone(),duration,120,width,height,
                1.0,false,2000,false,0,false,None,None,Some(470.0),Some(70.0),Some(3.0),
                Some(0.0),Some(true),Some(30.0),Some("horizontal".into()),edit,None,Some(false),None,until,job)
        };
        let owner = uuid::Uuid::new_v4().to_string();
        let first = crate::commands::cursor_preview_jobs::begin(&owner);
        run(Some(5000),Some(first.clone())).await.unwrap();
        let full = tokio::spawn(run(None,Some(first)));
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
        assert!(!full.is_finished(),"full preparation finished before the seek");
        let start = std::time::Instant::now();
        let second = crate::commands::cursor_preview_jobs::begin(&owner);
        let error = full.await.unwrap().unwrap_err();
        assert!(error.contains("Cursor preview cancelled"),"{error}");
        let cancelled_ms = start.elapsed().as_secs_f64()*1000.0;
        let replacement = run(Some(37000),Some(second.clone())).await.unwrap();
        let ready_ms = start.elapsed().as_secs_f64()*1000.0;
        crate::commands::cursor_preview_jobs::finish(&second);
        let reference = run(Some(37000),None).await.unwrap();
        assert_eq!(replacement,reference,"edited={edited}");
        let value: Value = serde_json::from_str(&replacement).unwrap();
        let frames = value.get("frames").unwrap_or(&value).as_array().unwrap();
        assert_eq!(frames.len(),4440);
        println!("SEEK_REPLACEMENT edited={edited} cancelled_ms={cancelled_ms:.3} ready_ms={ready_ms:.3} frames={} bytes={}",frames.len(),replacement.len());
    }
}
