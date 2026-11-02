//! Bounded-input decode graphs for timeline segments. Each source range is sought
//! independently so a reorder does not buffer the entire original recording.
use super::timeline::EditTimeline;
use std::path::Path;

fn text(args: &[&str]) -> Vec<String> {
    args.iter().map(|s| s.to_string()).collect()
}
fn seconds(ms: f64) -> String {
    format!("{:.9}", ms / 1000.0)
}

pub fn video_decode_args(
    plan: &EditTimeline,
    default_source: &Path,
    width: u32,
    height: u32,
    fps: u32,
) -> Vec<String> {
    video_decode_args_with_crop(plan, default_source, width, height, fps, None)
}

pub fn video_decode_args_with_crop(
    plan: &EditTimeline, default_source: &Path, width: u32, height: u32, fps: u32,
    crop: Option<super::source_crop::PixelCrop>,
) -> Vec<String> {
    let mut args = text(&["-v", "error"]);
    let mut graph = Vec::new();
    let sampled: Vec<_> = plan
        .segments
        .iter()
        .filter_map(|segment| {
            let first = (segment.output_start_ms * fps as f64 / 1000.0 - 1e-7)
                .ceil()
                .max(0.0) as u64;
            let last = (segment.output_end_ms * fps as f64 / 1000.0 - 1e-7)
                .ceil()
                .max(0.0) as u64;
            (last > first).then_some((segment, first, last - first))
        })
        .collect();
    for (index, (segment, first, count)) in sampled.iter().enumerate() {
        let duration = *count as f64 * 1000.0 / fps as f64;
        let offset = *first as f64 * 1000.0 / fps as f64 - segment.output_start_ms;
        if let Some(start) = segment.source_start_ms {
            args.extend([
                "-ss".into(),
                seconds(start + offset * segment.rate),
                "-t".into(),
                seconds((segment.output_end_ms - segment.output_start_ms - offset) * segment.rate),
                "-i".into(),
                segment
                    .source_file_path
                    .clone()
                    .unwrap_or_else(|| default_source.to_string_lossy().into_owned()),
            ]);
        } else {
            args.extend([
                "-f".into(),
                "lavfi".into(),
                "-i".into(),
                format!(
                    "color=c=black:s={width}x{height}:r={fps}:d={}",
                    seconds(duration)
                ),
            ]);
        }
        let crop_filter = crop.filter(|_| segment.source_start_ms.is_some()).map_or(String::new(), |rect|
            rect.decode_filter());
        graph.push(format!("[{index}:v]setpts=(PTS-STARTPTS)/{},{crop_filter}scale={width}:{height},setsar=1,fps={fps},tpad=stop_mode=clone:stop_duration={},trim=end_frame={count},setpts=PTS-STARTPTS[v{index}]", segment.rate, seconds(duration)));
    }
    graph.push(format!(
        "{}concat=n={}:v=1:a=0[out]",
        (0..sampled.len())
            .map(|i| format!("[v{i}]"))
            .collect::<String>(),
        sampled.len()
    ));

    args.extend([
        "-filter_complex".into(),
        graph.join(";"),
        "-map".into(),
        "[out]".into(),
        "-f".into(),
        "rawvideo".into(),
        "-pix_fmt".into(),
        "rgba".into(),
        "-".into(),
    ]);
    args
}

/// Decode one linked source audio track. Muted clips and timeline gaps become silence.
pub fn audio_decode_args(plan: &EditTimeline, source: &Path) -> Vec<String> {
    audio_decode_args_for_channel(plan, source, false)
}

pub fn audio_decode_args_for_channel(plan: &EditTimeline, source: &Path, microphone: bool) -> Vec<String> {
    let mut args = text(&["-v", "error"]);
    let mut graph = Vec::new();
    for (index, segment) in plan.segments.iter().enumerate() {
        let duration = segment.output_end_ms - segment.output_start_ms;
        if let Some(start) = segment.source_start_ms.filter(|_| !segment.muted) {
            args.extend([
                "-ss".into(),
                seconds(start),
                "-t".into(),
                seconds(duration * segment.rate),
                "-i".into(),
                source.to_string_lossy().into_owned(),
            ]);
        } else {
            args.extend([
                "-f".into(),
                "lavfi".into(),
                "-t".into(),
                seconds(duration),
                "-i".into(),
                "anullsrc=r=48000:cl=stereo".into(),
            ]);
        }
        let mut remaining = segment.rate;
        let mut tempo = Vec::new();
        while remaining > 2.0 {
            tempo.push("atempo=2".to_string());
            remaining /= 2.0;
        }
        while remaining < 0.5 {
            tempo.push("atempo=0.5".to_string());
            remaining /= 0.5;
        }
        tempo.push(format!("atempo={remaining}"));
        tempo.push(format!("volume={}", if microphone { segment.mic_volume } else { segment.system_volume }));
        graph.push(format!("[{index}:a]asetpts=PTS-STARTPTS,aformat=sample_fmts=fltp:sample_rates=48000:channel_layouts=stereo,{},apad,atrim=duration={},asetpts=PTS-STARTPTS[a{index}]", tempo.join(","), seconds(duration)));
    }
    graph.push(format!(
        "{}concat=n={}:v=0:a=1[out]",
        (0..plan.segments.len())
            .map(|i| format!("[a{i}]"))
            .collect::<String>(),
        plan.segments.len()
    ));
    args.extend([
        "-filter_complex".into(),
        graph.join(";"),
        "-map".into(),
        "[out]".into(),
        "-f".into(),
        "f32le".into(),
        "-ac".into(),
        "1".into(),
        "-ar".into(),
        "48000".into(),
        "-".into(),
    ]);
    args
}

#[cfg(test)]
mod volume_tests {
    use super::*;
    use super::super::timeline::EditClip;
    #[test]
    #[ignore = "requires local FFmpeg; measures decoded clip audio"]
    fn decoded_clip_audio_preserves_independent_levels_through_speed_trim_and_gaps() {
        let dir = std::env::temp_dir().join(format!("clip-audio-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        let result = std::panic::catch_unwind(|| {
            let source = dir.join("tone.wav");
            let output = std::process::Command::new("ffmpeg").args(["-v", "error", "-f", "lavfi", "-i",
                "sine=frequency=440:sample_rate=48000:duration=3", "-c:a", "pcm_f32le"]).arg(&source).output().unwrap();
            assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
            let clips:Vec<EditClip> = serde_json::from_value(serde_json::json!([
                {"id":"fast","startTime":0,"duration":1000,"sourceIn":0,"sourceOut":2000,"playbackRate":2,"trackOrder":0,"muted":false,"micVolume":0.25,"systemVolume":0.7},
                {"id":"slow","startTime":1500,"duration":1000,"sourceIn":0,"sourceOut":500,"playbackRate":0.5,"trackOrder":0,"muted":false,"micVolume":0,"systemVolume":0.5},
                {"id":"muted","startTime":2500,"duration":500,"sourceIn":0,"sourceOut":500,"playbackRate":1,"trackOrder":0,"muted":true,"micVolume":1,"systemVolume":1}
            ])).unwrap();
            let plan = EditTimeline::build(&clips, 250.0, 3000.0).unwrap();
            for microphone in [true, false] {
                let output = std::process::Command::new("ffmpeg").args(audio_decode_args_for_channel(&plan,&source,microphone)).output().unwrap();
                assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
                let samples:Vec<f64> = output.stdout.chunks_exact(4).map(|s|f32::from_le_bytes(s.try_into().unwrap()) as f64).collect();
                assert!((samples.len() as i64 - 132000).abs() < 100);
                for (start,end,gain) in [(100,600,if microphone {0.25} else {0.7}),
                    (850,1150,0.0),(1400,2000,if microphone {0.0} else {0.5}),(2350,2650,0.0)] {
                    let section=&samples[start*48..end*48];
                    let rms=(section.iter().map(|s|s*s).sum::<f64>()/section.len() as f64).sqrt();
                    let expected=0.125/2_f64.sqrt()*gain;
                    assert!((rms-expected).abs() < 0.001, "mic={microphone} at {start}: {rms} expected {expected}");
                }
            }
        });
        std::fs::remove_dir_all(dir).unwrap();
        if let Err(error)=result {std::panic::resume_unwind(error);}
    }
}

#[cfg(test)]
mod fast_tests {
    use super::*;
    use super::super::timeline::EditClip;
    #[test]
    #[ignore = "requires local FFmpeg; checks high-speed video and audio timing"]
    fn fast_decode_preserves_frames_duration_and_audio_pitch() {
        let dir = std::env::temp_dir().join(format!("fast-decode-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        let result = std::panic::catch_unwind(|| {
            let source = dir.join("source.mkv");
            let output = std::process::Command::new("ffmpeg").args(["-v","error","-f","lavfi","-i",
                "color=red:s=16x16:r=30:d=12","-f","lavfi","-i","color=blue:s=16x16:r=30:d=12",
                "-f","lavfi","-i","sine=frequency=440:sample_rate=48000:duration=24",
                "-filter_complex","[0:v][1:v]concat=n=2:v=1:a=0[v]","-map","[v]","-map","2:a",
                "-c:v","ffv1","-c:a","pcm_f32le"]).arg(&source).output().unwrap();
            assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
            for rate in [8.0,16.0,24.0] {
                let duration = 24000.0/rate;
                let clip:EditClip = serde_json::from_value(serde_json::json!({
                    "id":"fast","startTime":0,"duration":duration,"sourceIn":0,"sourceOut":24000,
                    "playbackRate":rate,"trackOrder":0,"muted":false
                })).unwrap();
                let plan = EditTimeline::build(&[clip],0.0,duration).unwrap();
                let frames = std::process::Command::new("ffmpeg").args(video_decode_args(&plan,&source,16,16,30)).output().unwrap();
                assert!(frames.status.success(), "{}", String::from_utf8_lossy(&frames.stderr));
                let count=(duration*30.0/1000.0).round() as usize;
                assert_eq!(frames.stdout.len(),count*16*16*4,"rate={rate}");
                for (frame,red) in [(count/4,true),(count*3/4,false)] {
                    let pixel=&frames.stdout[frame*16*16*4..][..4];
                    assert!(if red {pixel[0]>200 && pixel[2]<20} else {pixel[2]>200 && pixel[0]<20});
                }
                let audio = std::process::Command::new("ffmpeg").args(audio_decode_args(&plan,&source)).output().unwrap();
                assert!(audio.status.success(),"{}",String::from_utf8_lossy(&audio.stderr));
                let samples:Vec<f32> = audio.stdout.chunks_exact(4).map(|s|f32::from_le_bytes(s.try_into().unwrap())).collect();
                assert_eq!(samples.len(),(duration*48.0).round() as usize);
                let middle=&samples[12000..36000];
                let crossings=middle.windows(2).filter(|w| w[0]<=0.0 && w[1]>0.0).count();
                assert!((crossings as i32-220).abs()<=3,"rate={rate}, crossings={crossings}");
                let rms=(middle.iter().map(|s|(*s as f64).powi(2)).sum::<f64>()/middle.len() as f64).sqrt();
                assert!(rms>0.07 && rms<0.1,"rate={rate}, rms={rms}");
            }
        });
        std::fs::remove_dir_all(dir).unwrap();
        if let Err(error)=result {std::panic::resume_unwind(error);}
    }
}
