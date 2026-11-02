//! Compare edited camera pixels to a continuous source-time export, including fades.
use super::{run,strings,ExportSettings};
use crate::video_processing::{export::export_video,types::VideoInfo};
use std::path::Path;

pub async fn render(root:&Path,input:&Path,settings:&ExportSettings,info:&VideoInfo)->Vec<u8> {
    let mut continuous=settings.clone();
    continuous.timeline_clips=None;
    continuous.frame_rate=Some(120);
    let path=root.join("camera-continuous.mp4");
    continuous.output_path=Some(path.to_string_lossy().into_owned());
    export_video(root,input,continuous,info,None).await.unwrap();
    let frames=run(&strings(&["-v","error","-i",path.to_str().unwrap(),"-map","0:v",
        "-fps_mode","passthrough","-f","rawvideo","-pix_fmt","rgba","-"]));
    assert_eq!(frames.len(),240*64*64*4);
    frames
}

pub fn compare(reference:&[u8],frame:&[u8],index:usize) {
    let source_frame=if index<30 {Some(120+index*4)} else if index>=45 {Some(index-45)} else {None};
    for (x,y) in [(48,48),(5,5),(32,32)] {
        let offset=(y*64+x)*4;
        let pixel=&frame[offset..offset+3];
        if let Some(source_frame)=source_frame {
            let expected=&reference[source_frame*64*64*4+offset..][..3];
            assert!(pixel.iter().zip(expected).all(|(a,b)|(*a as i32-*b as i32).abs()<=20),
                "camera source-time mismatch output {index}, source {source_frame}, {x},{y}: {pixel:?} != {expected:?}");
        } else {assert!(pixel.iter().all(|v|*v<15),"gap {index}: {pixel:?}");}
    }
}
