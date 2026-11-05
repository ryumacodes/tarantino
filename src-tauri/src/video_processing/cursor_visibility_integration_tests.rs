//! Rendered coverage of cursor visibility across edited clip boundaries.
use super::{export::export_video, types::{ExportSettings, VideoInfo}};
use std::{path::Path, process::Command};

fn ffmpeg(args: &[&str]) -> Vec<u8> {
    let output = Command::new(crate::ffmpeg::binary("ffmpeg")).args(args).output().unwrap();
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
    output.stdout
}

#[tokio::test]
#[ignore = "requires local FFmpeg and GPU; cursor must leave and re-enter captured window"]
async fn off_window_cursor_travel_does_not_pin_to_the_capture_edge() {
    let root = std::env::temp_dir().join(format!("cursor-edge-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&root).unwrap();
    let input = root.join("black.mkv");
    ffmpeg(&["-v", "error", "-f", "lavfi", "-i", "color=c=black:s=128x128:r=60:d=4",
        "-c:v", "ffv1", input.to_str().unwrap()]);
    // Logical coordinates on a monitor left of the primary display, at Retina scale.
    // Leaving through the left edge exposes the old clamp: the arrow points inward.
    std::fs::write(input.with_extension("mouse.json"), serde_json::to_vec(&serde_json::json!({
        "display_width":1470,"display_height":956,"scale_factor":2,
        "recording_area":{"x":-300,"y":50,"width":128,"height":128},
        "mouse_events":[
            {"timestamp":0,"x":-236,"y":114},
            {"timestamp":500,"x":-236,"y":114},
            {"timestamp":1000,"x":-428,"y":114},
            {"timestamp":2500,"x":-428,"y":114},
            {"timestamp":3000,"x":-236,"y":114},
            {"timestamp":3999,"x":-236,"y":114}
        ]
    })).unwrap()).unwrap();
    let info = VideoInfo { duration_ms:4000,width:128,height:128,fps:60.0,
        bitrate:0,format:"mkv".into(),size_bytes:0,frame_count:Some(240) };
    for fps in [24,60] { for smooth in [false,true] {
        let output = root.join(format!("{fps}-{smooth}.mp4"));
        let settings: ExportSettings = serde_json::from_value(serde_json::json!({
            "output_path":output,"resolution":{"width":128,"height":128},
            "capture_mode":"window","source_width":128,"source_height":128,
            "frame_rate":fps,"format":"mp4","codec":"h264","zoom_blocks":[],
            "cursor_settings":{"enabled":true,"size":1,"color":"#ffffff","click_effect":"none",
                "spring_tension":470,"spring_friction":70,"spring_mass":if smooth {3}else{0}},
            "visual_settings":{"padding":0,"corner_radius":0,"shadow_enabled":false}
        })).unwrap();
        export_video(&root,&input,settings,&info,None).await.unwrap();
        let decoded = ffmpeg(&["-v","error","-i",output.to_str().unwrap(),"-fps_mode","passthrough",
            "-f","rawvideo","-pix_fmt","rgb24","-"]);
        let frames = decoded.chunks_exact(128*128*3).collect::<Vec<_>>();
        assert_eq!(frames.len(),fps*4);
        for (second,visible) in [(0.25,true),(2.25,false),(3.75,true)] {
            let frame = frames[(second*fps as f64) as usize];
            // Inspect the cursor's horizontal band; window corner antialiasing
            // is visible even on a black source and is unrelated to pointer travel.
            let bright = frame.chunks_exact(3).enumerate()
                .filter(|(i,rgb)|(32..96).contains(&(i/128)) && rgb.iter().any(|v|*v>32)).count();
            if visible { assert!(bright>20,"missing cursor at {second}s; {}",output.display()); }
            else { assert_eq!(bright,0,"edge-pinned cursor at {second}s; {}",output.display()); }
        }
    }}
    std::fs::remove_dir_all(root).unwrap();
}

#[tokio::test]
#[ignore = "requires local FFmpeg and GPU; rendered per-clip cursor visibility"]
async fn hidden_clips_remove_cursor_and_click_effects_after_speed_and_trim() {
    let root = std::env::temp_dir().join(format!("cursor-visibility-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&root).unwrap();
    verify(&root, false).await;
    std::fs::remove_dir_all(root).unwrap();
}

#[tokio::test]
#[ignore = "requires local FFmpeg and GPU; contiguous clips must retain cursor visibility"]
async fn contiguous_hidden_clips_keep_visibility_mapping() {
    let root=std::env::temp_dir().join(format!("cursor-contiguous-{}",uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&root).unwrap();
    verify(&root,true).await;
    std::fs::remove_dir_all(root).unwrap();
}

async fn verify(root: &Path, contiguous: bool) {
    let input = root.join("black.mkv");
    ffmpeg(&["-v", "error", "-f", "lavfi", "-i", "color=c=black:s=128x128:r=60:d=1",
        "-c:v", "ffv1", input.to_str().unwrap()]);
    std::fs::write(input.with_extension("mouse.json"), serde_json::to_vec(&serde_json::json!([
        {"timestamp":0,"x":40,"y":40,"event_type":{"ButtonPress":"Left"}},
        {"timestamp":20,"x":40,"y":40,"event_type":{"ButtonRelease":"Left"}},
        {"timestamp":150,"x":40,"y":40,"event_type":{"ButtonPress":"Left"}},
        {"timestamp":170,"x":40,"y":40,"event_type":{"ButtonRelease":"Left"}},
        {"timestamp":250,"x":40,"y":40,"event_type":{"ButtonPress":"Left"}},
        {"timestamp":270,"x":40,"y":40,"event_type":{"ButtonRelease":"Left"}},
        {"timestamp":999,"x":40,"y":40}
    ])).unwrap()).unwrap();
    let info = VideoInfo { duration_ms:1000, width:128, height:128, fps:60.0,
        bitrate:0, format:"mkv".into(), size_bytes:0, frame_count:Some(60) };
    // Each clip repeats the same source interval at 2x. Thus hidden output cannot
    // be explained by an empty sidecar, an idle source interval or source-time masking.
    let mut baseline = Vec::new();
    for effect in ["none", "ripple", "circle"] {
        let output = root.join(format!("{effect}.mp4"));
        let mut value = serde_json::json!({
            "output_path":output,"resolution":{"width":128,"height":128},
            "source_width":128,"source_height":128,"frame_rate":60,"format":"mp4","codec":"h264",
            "trim_start":125,"trim_end":625,"zoom_blocks":[],
            "cursor_settings":{"enabled":true,"size":1,"color":"#ffffff",
                "click_effect":effect,"trail_enabled":true,"trail_length":15},
            "visual_settings":{"padding":0,"corner_radius":0,"shadow_enabled":false},
            "timeline_clips":[
                {"id":"before","startTime":0,"duration":250,"sourceIn":0,"sourceOut":500,"playbackRate":2,"trackOrder":0,"muted":false},
                {"id":"hidden","startTime":250,"duration":250,"sourceIn":0,"sourceOut":500,"playbackRate":2,"trackOrder":0,"muted":false,"hideCursor":true},
                {"id":"after","startTime":500,"duration":250,"sourceIn":0,"sourceOut":500,"playbackRate":2,"trackOrder":0,"muted":false}
            ]
        });
        if contiguous {
            value["trim_end"]=875.into();
            value["timeline_clips"]=serde_json::json!([
                {"id":"before","startTime":0,"duration":250,"sourceIn":0,"sourceOut":250,"playbackRate":1,"trackOrder":0,"muted":false},
                {"id":"hidden","startTime":250,"duration":500,"sourceIn":250,"sourceOut":750,"playbackRate":1,"trackOrder":0,"muted":false,"hideCursor":true},
                {"id":"after","startTime":750,"duration":250,"sourceIn":750,"sourceOut":1000,"playbackRate":1,"trackOrder":0,"muted":false}
            ]);
        }
        let settings:ExportSettings=serde_json::from_value(value).unwrap();
        export_video(root, &input, settings, &info, None).await.unwrap();
        let decoded = ffmpeg(&["-v","error","-i",output.to_str().unwrap(),"-map","0:v",
            "-fps_mode","passthrough","-f","rawvideo","-pix_fmt","rgb24","-"]);
        assert_eq!(decoded.len(), (if contiguous {45} else {30}) * 128 * 128 * 3);
        if effect == "none" {
            baseline = decoded.clone();
        } else {
            let differing_channels = decoded.iter().zip(&baseline)
                .filter(|(a, b)| a.abs_diff(**b) > 32).count();
            assert!(differing_channels > 100, "{effect}: fixture must actually render the click effect");
        }
        for (index, frame) in decoded.chunks_exact(128 * 128 * 3).enumerate() {
            let time = index as f64 * 1000.0 / 60.0;
            let bright_pixels = frame.chunks_exact(3).filter(|rgb| rgb.iter().any(|v| *v > 32)).count();
            if (125.0..if contiguous {625.0} else {375.0}).contains(&time) {
                assert_eq!(bright_pixels, 0, "{effect}: stale cursor/effect in hidden frame {index}; artifacts at {}", root.display());
            } else {
                assert!(bright_pixels > 20, "{effect}: cursor absent from visible frame {index}; artifacts at {}", root.display());
            }
        }
    }
}

#[tokio::test]
#[ignore="requires local FFmpeg and GPU; trimmed loop return position"]
async fn contiguous_trim_loop_renders_the_edited_opening_at_the_end() {
    let root=std::env::temp_dir().join(format!("cursor-trim-loop-{}",uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&root).unwrap();
    let input=root.join("black.mkv");
    ffmpeg(&["-v","error","-f","lavfi","-i","color=c=black:s=128x128:r=60:d=1","-c:v","ffv1",input.to_str().unwrap()]);
    std::fs::write(input.with_extension("mouse.json"),r#"[{"timestamp":0,"x":20,"y":40},{"timestamp":500,"x":100,"y":40},{"timestamp":999,"x":20,"y":40}]"#).unwrap();
    let output=root.join("loop.mp4");
    let settings:ExportSettings=serde_json::from_value(serde_json::json!({
        "output_path":output,"resolution":{"width":128,"height":128},"frame_rate":60,"format":"mp4","codec":"h264",
        "trim_start":250,"trim_end":750,"zoom_blocks":[],
        "cursor_settings":{"enabled":true,"size":1,"color":"#ffffff","click_effect":"none","spring_mass":0,
            "spring_tension":170,"spring_friction":30,"loop_to_start":true,"loop_duration_ms":100},
        "visual_settings":{"padding":0,"corner_radius":0,"shadow_enabled":false},
        "timeline_clips":[{"id":"one","startTime":0,"duration":1000,"sourceIn":0,"sourceOut":1000,"playbackRate":1,"trackOrder":0,"muted":false}]
    })).unwrap();
    let info=VideoInfo{duration_ms:1000,width:128,height:128,fps:60.0,bitrate:0,format:"mkv".into(),size_bytes:0,frame_count:Some(60)};
    export_video(&root,&input,settings,&info,None).await.unwrap();
    let decoded=ffmpeg(&["-v","error","-i",output.to_str().unwrap(),"-fps_mode","passthrough","-f","rawvideo","-pix_fmt","rgb24","-"]);
    let frames=decoded.chunks_exact(128*128*3).collect::<Vec<_>>();
    assert_eq!(frames.len(),30);
    let center=|frame:&[u8]| {
        let pixels=frame.chunks_exact(3).enumerate().filter(|(_,rgb)|rgb.iter().any(|v|*v>160))
            .map(|(index,_)|(index%128) as f64).collect::<Vec<_>>();
        assert!(pixels.len()>20,"cursor must be visible; artifacts {}",root.display());
        pixels.iter().sum::<f64>()/pixels.len() as f64
    };
    let first=center(frames[0]);let middle=center(frames[14]);let last=center(frames[29]);
    assert!((last-first).abs()<1.0,"loop ends at wrong position: {first} -> {last}; {}",root.display());
    assert!(middle-first>20.0,"fixture must move away before returning");
    std::fs::remove_dir_all(root).unwrap();
}

#[tokio::test]
#[ignore="requires local FFmpeg and GPU; rendered per-clip smoothing override"]
async fn clip_smoothing_override_matches_raw_cursor_pixels_only_in_selected_clips() {
    let root=std::env::temp_dir().join(format!("cursor-clip-smoothing-{}",uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&root).unwrap();
    let input=root.join("black.mkv");
    ffmpeg(&["-v","error","-f","lavfi","-i","color=c=black:s=128x128:r=60:d=1","-c:v","ffv1",input.to_str().unwrap()]);
    std::fs::write(input.with_extension("mouse.json"),r#"[{"timestamp":0,"x":20,"y":40},{"timestamp":500,"x":100,"y":40},{"timestamp":999,"x":100,"y":40}]"#).unwrap();
    let info=VideoInfo{duration_ms:1000,width:128,height:128,fps:60.0,bitrate:0,format:"mkv".into(),size_bytes:0,frame_count:Some(60)};
    for repeated in [false,true] {
        let count=if repeated {3}else{1};
        let mut centers=Vec::new();
        for variant in ["override","smooth","raw"] {
            let output=root.join(format!("{repeated}-{variant}.mp4"));
            let clips=(0..count).map(|i|serde_json::json!({
                "id":format!("clip-{i}"),"startTime":i*1000,"duration":1000,"sourceIn":0,"sourceOut":1000,
                "playbackRate":1,"trackOrder":0,"muted":false,
                "disableCursorSmoothing":variant=="override" && (!repeated || i==1)
            })).collect::<Vec<_>>();
            let settings:ExportSettings=serde_json::from_value(serde_json::json!({
                "output_path":output,"resolution":{"width":128,"height":128},"frame_rate":60,"format":"mp4","codec":"h264",
                "trim_start":0,"trim_end":count*1000,"zoom_blocks":[],
                "cursor_settings":{"enabled":true,"size":1,"color":"#ffffff","click_effect":"none",
                    "spring_mass":if variant=="raw" {0}else{1},"spring_tension":170,"spring_friction":30},
                "visual_settings":{"padding":0,"corner_radius":0,"shadow_enabled":false},"timeline_clips":clips
            })).unwrap();
            export_video(&root,&input,settings,&info,None).await.unwrap();
            let decoded=ffmpeg(&["-v","error","-i",output.to_str().unwrap(),"-fps_mode","passthrough","-f","rawvideo","-pix_fmt","rgb24","-"]);
            let frames=decoded.chunks_exact(128*128*3).collect::<Vec<_>>();
            assert_eq!(frames.len(),count*60);
            let sample=(0..count).map(|i| {
                let pixels=frames[i*60+15].chunks_exact(3).enumerate()
                    .filter(|(_,rgb)|rgb.iter().any(|v|*v>160))
                    .map(|(index,_)|(index%128) as f64).collect::<Vec<_>>();
                assert!(pixels.len()>20,"cursor must be visible; artifacts {}",root.display());
                pixels.iter().sum::<f64>()/pixels.len() as f64
            }).collect::<Vec<_>>();
            centers.push(sample);
        }
        for i in 0..count {
            let raw=!repeated || i==1;
            let expected=centers[if raw {2}else{1}][i];
            assert!((centers[0][i]-expected).abs()<1.0,"repeated={repeated}, clip={i}, centers={centers:?}; {}",root.display());
            assert!(centers[2][i]-centers[1][i]>10.0,"fixture must distinguish raw and spring positions: {centers:?}");
        }
    }
    std::fs::remove_dir_all(root).unwrap();
}

#[tokio::test]
#[ignore = "requires local FFmpeg and GPU; typing fade through actual video encoding"]
async fn typing_fade_survives_rendered_trim_and_repeated_speed_changes() {
    let root=std::env::temp_dir().join(format!("cursor-typing-render-{}",uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&root).unwrap();
    let input=root.join("black.mkv");
    ffmpeg(&["-v","error","-f","lavfi","-i","color=c=black:s=128x128:r=60:d=3",
        "-c:v","ffv1",input.to_str().unwrap()]);
    let info=VideoInfo{duration_ms:3000,width:128,height:128,fps:60.0,bitrate:0,
        format:"mkv".into(),size_bytes:0,frame_count:Some(180)};
    for edited in [false,true] {
        for typing in [false,true] {
            let mut sidecar=serde_json::json!({"display_width":128,"display_height":128,
                "mouse_events":[{"timestamp":0,"x":40,"y":40},{"timestamp":2000,"x":40,"y":40}],
                "key_events":[]});
            if typing { sidecar["key_events"]=serde_json::json!([
                {"timestamp":100,"is_typing":true},{"timestamp":200,"is_typing":true},
                {"timestamp":300,"is_typing":true}]); }
            std::fs::write(input.with_extension("mouse.json"),sidecar.to_string()).unwrap();
            let output=root.join(format!("edited-{edited}-typing-{typing}.mp4"));
            let mut value=serde_json::json!({
                "output_path":output,"resolution":{"width":128,"height":128},
                "frame_rate":60,"format":"mp4","codec":"h264","trim_start":500,"trim_end":3000,
                "zoom_blocks":[],"cursor_settings":{"enabled":true,"size":1,"color":"#ffffff",
                    "click_effect":"none","trail_enabled":true,"trail_length":15},
                "visual_settings":{"padding":0,"corner_radius":0,"shadow_enabled":false}
            });
            let clips=serde_json::json!([
                {"id":"fast","startTime":0,"duration":1500,"sourceIn":0,"sourceOut":3000,"playbackRate":2,"trackOrder":0,"muted":false},
                {"id":"slow","startTime":1500,"duration":6000,"sourceIn":0,"sourceOut":3000,"playbackRate":0.5,"trackOrder":0,"muted":false}
            ]);
            if edited {
                value["trim_start"]=250.into(); value["trim_end"]=7250.into();
                value["timeline_clips"]=clips.clone();
            }
            let settings:ExportSettings=serde_json::from_value(value).unwrap();
            export_video(&root,&input,settings,&info,None).await.unwrap();
            let decoded=ffmpeg(&["-v","error","-i",output.to_str().unwrap(),"-fps_mode","passthrough",
                "-f","rawvideo","-pix_fmt","rgb24","-"]);
            let frames=decoded.chunks_exact(128*128*3).collect::<Vec<_>>();
            assert_eq!(frames.len(),if edited {420}else{150});
            let plan=super::timeline::EditTimeline::build(&serde_json::from_value::<Vec<super::timeline::EditClip>>(clips).unwrap(),250.0,7250.0).unwrap();
            let mut hidden=0; let mut visible=0;
            for (index,frame) in frames.iter().enumerate() {
                let output_time=index as f64*1000.0/60.0;
                let source=if edited {plan.source_at(output_time).unwrap().1}else{500.0+output_time};
                let bright=frame.chunks_exact(3).filter(|rgb|rgb.iter().any(|v|*v>32)).count();
                if typing && (1000.0..1800.0).contains(&source) {
                    assert_eq!(bright,0,"cursor/trail visible while typing at source {source}; {}",root.display());
                    hidden+=1;
                } else if !typing || source>2700.0 {
                    assert!(bright>20,"cursor missing after typing or without keys at source {source}; {}",root.display());
                    visible+=1;
                }
            }
            assert!(visible>5);
            if typing { assert!(hidden>20); }
        }
    }
    std::fs::remove_dir_all(root).unwrap();
}

#[tokio::test]
#[ignore = "requires local FFmpeg and GPU; validates encoded cursor press scale"]
async fn encoded_cursor_press_shrinks_and_returns_without_moving_hotspot() {
    let root = std::env::temp_dir().join(format!("cursor-press-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&root).unwrap();
    let input = root.join("black.mkv");
    ffmpeg(&["-v","error","-f","lavfi","-i","color=c=black:s=256x256:r=60:d=4",
        "-c:v","ffv1",input.to_str().unwrap()]);
    std::fs::write(input.with_extension("mouse.json"),serde_json::to_vec(&serde_json::json!({
        "display_width":256,"display_height":256,"mouse_events":[
            {"timestamp":0,"x":96,"y":96,"event_type":"Move"},
            {"timestamp":1000,"x":96,"y":96,"event_type":{"ButtonPress":"Left"}},
            {"timestamp":1500,"x":96,"y":96,"event_type":"Move"},
            {"timestamp":3000,"x":96,"y":96,"event_type":{"ButtonRelease":"Left"}},
            {"timestamp":3999,"x":96,"y":96,"event_type":"Move"}
        ]})).unwrap()).unwrap();
    let info=VideoInfo{duration_ms:4000,width:256,height:256,fps:60.0,bitrate:0,
        format:"mkv".into(),size_bytes:0,frame_count:Some(240)};
    for fps in [24,60] { for smooth in [false,true] {
      let mut reference_area=Vec::new();
      for mode in ["normal","loop","edited","split"] {
        let output=root.join(format!("press-{fps}-{smooth}-{mode}.mp4"));
        let mut value=serde_json::json!({
            "output_path":output,"resolution":{"width":256,"height":256},
            "capture_mode":"window","source_width":256,"source_height":256,
            "frame_rate":fps,"format":"mp4","codec":"h264","zoom_blocks":[],
            "cursor_settings":{"enabled":true,"size":2,"color":"#ffffff","highlight_color":"#ffffff",
                "click_effect":"none","shadow_intensity":0,"spring_tension":470,"spring_friction":70,
                "spring_mass":if smooth {3}else{0}},
            "visual_settings":{"padding":0,"corner_radius":0,"shadow_enabled":false}
        });
        if mode=="loop" {
            value["cursor_settings"]["loop_to_start"]=true.into();
            value["cursor_settings"]["loop_duration_ms"]=3500.into();
        }
        if mode=="edited" {
            value["trim_start"]=0.into();value["trim_end"]=1500.into();
            value["timeline_clips"]=serde_json::json!([{ "id":"slow-trim", "startTime":0,
                "duration":1500,"sourceIn":0,"sourceOut":750,"playbackRate":0.5,
                "trackOrder":0,"muted":true }]);
        }
        if mode=="split" {
            value["trim_start"]=0.into();value["trim_end"]=4000.into();
            value["timeline_clips"]=serde_json::json!([
                {"id":"first","startTime":0,"duration":650.25,"sourceIn":0,"sourceOut":650.25,"playbackRate":1,"trackOrder":0,"muted":true},
                {"id":"second","startTime":650.25,"duration":3349.75,"sourceIn":650.25,"sourceOut":4000,"playbackRate":1,"trackOrder":0,"muted":true}]);
        }
        let settings:ExportSettings=serde_json::from_value(value).unwrap();
        export_video(&root,&input,settings,&info,None).await.unwrap();
        let decoded=ffmpeg(&["-v","error","-i",output.to_str().unwrap(),"-fps_mode","passthrough",
            "-f","rawvideo","-pix_fmt","rgb24","-"]);
        let frames=decoded.chunks_exact(256*256*3).collect::<Vec<_>>();
        assert_eq!(frames.len(),if mode=="edited" {fps*3/2}else{fps*4});
        let areas=frames.iter().map(|frame|frame.chunks_exact(3).enumerate()
            .filter(|(i,p)|(64..192).contains(&(i/256)) && p.iter().all(|v|*v>160)).count()).collect::<Vec<_>>();
        if mode=="normal" {reference_area=areas;}
        else if mode=="split" {
            assert_eq!(areas.len(),reference_area.len());
            for (index,(actual,expected)) in areas.iter().zip(&reference_area).enumerate() {
                assert!(actual.abs_diff(*expected)<=3,"split changed cursor coverage at frame{index}: {actual} != {expected}");
            }
            continue;
        }
        let measure=|second:f64| {
            let pixels=frames[(second*fps as f64) as usize].chunks_exact(3).enumerate()
                .filter(|(i,p)|(64..192).contains(&(i/256)) && p.iter().all(|v|*v>160))
                .map(|(i,_)|(i%256,i/256)).collect::<Vec<_>>();
            assert!(!pixels.is_empty(),"cursor missing: {} at {second}",output.display());
            (pixels.len(),pixels.iter().map(|p|p.0).min().unwrap(),pixels.iter().map(|p|p.1).min().unwrap())
        };
        let normal=measure(0.25);
        if mode!="normal" {
            let closing=measure(if mode=="edited" {1.3}else{2.0});
            assert!((closing.0 as f64/normal.0 as f64-1.0).abs()<0.03,
                "excluded click changed size in {mode}: {normal:?} -> {closing:?}");
            assert!(normal.1.abs_diff(closing.1)<=1 && normal.2.abs_diff(closing.2)<=1);
            continue;
        }
        let pressed=measure(2.0);let released=measure(3.75);
        let ratio=pressed.0 as f64/normal.0 as f64;
        assert!((0.72..0.90).contains(&ratio),"press area ratio {ratio}: {}",output.display());
        assert!((released.0 as f64/normal.0 as f64-1.0).abs()<0.03,"release size did not return");
        assert!(normal.1.abs_diff(pressed.1)<=1 && normal.2.abs_diff(pressed.2)<=1,
            "press changed cursor hotspot: {normal:?} -> {pressed:?}");
    }}}
    std::fs::remove_dir_all(root).unwrap();
}
