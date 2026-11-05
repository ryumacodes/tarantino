//! Full encoded-output coverage of cursor shape selection in source time.
use super::{export::export_video,types::{ExportSettings,VideoInfo}};
use crate::input::cursor_images::{CursorImage,CursorImages,SystemCursor};
use base64::Engine;

fn ffmpeg(args:&[&str])->Vec<u8> {
    let result=std::process::Command::new(crate::ffmpeg::binary("ffmpeg")).args(args).output().unwrap();
    assert!(result.status.success(),"{}",String::from_utf8_lossy(&result.stderr));
    result.stdout
}

#[tokio::test]
#[ignore="native GPU and FFmpeg; recorded cursor shapes through full export"]
async fn recorded_cursor_shapes_follow_trim_speed_repeat_and_fixed_pointer() {
    let root=std::env::temp_dir().join(format!("recorded-cursor-export-{}",uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&root).unwrap();
    let input=root.join("black.mkv");
    ffmpeg(&["-v","error","-f","lavfi","-i","color=c=black:s=128x128:r=60:d=2",
        "-c:v","ffv1",input.to_str().unwrap()]);
    let mut images=CursorImages::default();
    for (time,color) in [(0,[255,0,0,255]),(750,[0,255,0,255]),(1250,[0,0,255,255])] {
        let pixels=image::RgbaImage::from_pixel(8,12,image::Rgba(color));
        let mut png=std::io::Cursor::new(Vec::new());
        pixels.write_to(&mut png,image::ImageFormat::Png).unwrap();
        images.record(time,CursorImage{system_cursor:Some(if time==0 {SystemCursor::Arrow} else if time==750 {SystemCursor::Text} else {SystemCursor::Hand}),width:8.0,height:12.0,hotspot_x:0.0,hotspot_y:0.0,
            png_base64:base64::engine::general_purpose::STANDARD.encode(png.into_inner())});
    }
    let sidecar=serde_json::json!({
        "display_width":128,"display_height":128,"cursor_images":images,
        "mouse_events":[{"timestamp":0,"x":40,"y":40,"event_type":"Move"},
            {"timestamp":900,"x":40,"y":40,"event_type":{"ButtonPress":{"button":"Left"}}},
            {"timestamp":1000,"x":40,"y":40,"event_type":{"ButtonRelease":{"button":"Left"}}},
            {"timestamp":1510,"x":40,"y":40,"event_type":"Move"},
            {"timestamp":1600,"x":40,"y":40,"event_type":{"ButtonPress":{"button":"Left"}}},
            {"timestamp":1700,"x":40,"y":40,"event_type":{"ButtonRelease":{"button":"Left"}}},
            {"timestamp":1999,"x":40,"y":40,"event_type":"Move"}]
    });
    let info=VideoInfo{duration_ms:2000,width:128,height:128,fps:60.0,bitrate:0,
        format:"mkv".into(),size_bytes:0,frame_count:Some(120)};
    let mut total=0;
    for fps in [24,60] { for mode in ["normal","trim","edited","fixed","fixed-legacy","loop","stop","filtered","filtered-trim","filtered-edited"] {
        let mut recording=sidecar.clone();
        if mode=="fixed-legacy" {
            for asset in recording["cursor_images"]["images"].as_array_mut().unwrap() {
                asset.as_object_mut().unwrap().remove("system_cursor");
            }
        }
        std::fs::write(input.with_extension("mouse.json"),serde_json::to_vec(&recording).unwrap()).unwrap();
        let output=root.join(format!("{fps}-{mode}.mp4"));
        let mut value=serde_json::json!({
            "output_path":output,"resolution":{"width":128,"height":128},
            "capture_mode":"window","source_width":128,"source_height":128,
            "frame_rate":fps,"format":"mp4","codec":"h264","zoom_blocks":[],
            "cursor_settings":{"enabled":true,"size":2,"style":"pointer","color":"#ffffff","highlight_color":"#ffffff",
                "click_effect":"none","shadow_intensity":0,"spring_mass":0,"optimize_cursor_types":mode.starts_with("filtered"),"always_use_pointer":mode.starts_with("fixed")},
            "visual_settings":{"padding":0,"corner_radius":0,"shadow_enabled":false}
        });
        // Shape lookup keeps following source time even when position closes early.
        if mode=="loop" {
            value["cursor_settings"]["loop_to_start"]=true.into();
            value["cursor_settings"]["loop_duration_ms"]=1500.into();
        }
        if mode=="stop" {
            value["cursor_settings"]["stop_at_end"]=true.into();
            value["cursor_settings"]["stop_duration_ms"]=1500.into();
        }
        let duration=match mode {
            "trim"|"filtered-trim"=>{value["trim_start"]=250.into();value["trim_end"]=1750.into();1500},
            "edited"|"filtered-edited"=>{
                value["trim_start"]=0.into();value["trim_end"]=2500.into();
                value["timeline_clips"]=serde_json::json!([
                    {"id":"fast","startTime":0,"duration":500,"sourceIn":500,"sourceOut":1500,"playbackRate":2,"trackOrder":0,"muted":true},
                    {"id":"repeat","startTime":500,"duration":2000,"sourceIn":0,"sourceOut":1000,"playbackRate":0.5,"trackOrder":0,"muted":true}
                ]);2500
            },_=>2000
        };
        let settings:ExportSettings=serde_json::from_value(value).unwrap();
        export_video(&root,&input,settings,&info,None).await.unwrap();
        let bytes=ffmpeg(&["-v","error","-i",output.to_str().unwrap(),"-fps_mode","passthrough",
            "-f","rawvideo","-pix_fmt","rgb24","-"]);
        let frames=bytes.chunks_exact(128*128*3).collect::<Vec<_>>();
        assert_eq!(frames.len(),duration*fps/1000);
        for (index,frame) in frames.iter().enumerate() {
            let time=index as f64*1000.0/fps as f64;
            let source=match mode {"trim"|"filtered-trim"=>time+250.0,"edited"|"filtered-edited" if time<500.0=>500.0+2.0*time,
                "edited"|"filtered-edited"=>(time-500.0)*0.5,_=>time};
            let mut counts=[0usize;3];let mut white=0;
            for rgb in frame.chunks_exact(3) {
                for c in 0..3 {if rgb[c]>120 && rgb[(c+1)%3]<50 && rgb[(c+2)%3]<50 {counts[c]+=1;}}
                if rgb.iter().all(|v|*v>150) {white+=1;}
            }
            if mode=="fixed-legacy" {assert_eq!(counts,[0;3]);assert!(white>50);}
            else {
                let expected=if mode=="fixed" {0} else if mode.starts_with("filtered") {
                    if source>1510.0 {2} else if source==1510.0 || source<750.0 {0} else {1}
                } else if source>=1250.0 {2}else if source>=750.0 {1}else{0};
                assert!(counts[expected]>100,"{fps}fps {mode} frame{index} source{source}: {counts:?}");
                for c in 0..3 {if c!=expected {assert_eq!(counts[c],0,"wrong shape at {fps}fps {mode} frame{index}");}}
            }
        }
        total+=frames.len();
    }}
    println!("Verified 20 recorded-cursor exports / {total} decoded frames");
    std::fs::remove_dir_all(root).unwrap();
}
