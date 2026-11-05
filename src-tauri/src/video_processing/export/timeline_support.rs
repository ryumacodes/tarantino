//! Preparation of linked recording tracks for edited exports.
use super::super::{
    audio_export::AudioTrackFiles, timeline::EditTimeline, timeline_decode::audio_decode_args_for_channel,
    types::ExportSettings,
};
use anyhow::{Result, bail};
use std::{
    path::{Path, PathBuf},
    process::Command,
};

pub fn resolve_plan(
    settings: &ExportSettings,
    input: &Path,
    source_duration: u64,
) -> Result<Option<EditTimeline>> {
    let Some(clips) = settings.timeline_clips.as_ref() else {
        return Ok(None);
    };
    // Sidecars currently belong to the primary recording. Reject unrelated media
    // rather than silently applying its cursor, camera, or audio to another file.
    for clip in clips {
        if clip.source_out > source_duration as f64 + 1.0 {
            bail!("Clip {} extends beyond the recording", clip.id);
        }
        if let Some(path) = &clip.source_file_path {
            if Path::new(path) != input
                && Path::new(path).canonicalize()? != input.canonicalize()?
            {
                bail!("Exporting clips from multiple recordings is not yet supported");
            }
        }
    }
    let extent = clips
        .iter()
        .map(|c| c.start_time + c.duration)
        .fold(0.0, f64::max);
    let end = settings.trim_end.map(|n| n as f64).unwrap_or(extent);
    let start = settings.trim_start.unwrap_or(0) as f64;
    let plan = EditTimeline::build(clips, start, end)?;
    // Preserve the accelerated contiguous path when edits do not change media
    // timing or mute state. Avoid unnecessary PCM staging for ordinary exports.
    if (settings.trim_end.is_some() || end == source_duration as f64)
        && end <= source_duration as f64
        && plan.segments.iter().all(|s| {
            !s.muted
                && s.mic_volume == 1.0 && s.system_volume == 1.0
                && !s.hide_cursor
                && s.rate == 1.0
                && s.source_start_ms
                    .is_some_and(|source| (source - s.output_start_ms - start).abs() < 1e-7)
        })
    {
        return Ok(None);
    }
    Ok(Some(plan))
}

/// Audio-only changes retain their own preparation plan while the video decoder
/// can keep its contiguous source range and hardware path.
pub(crate) fn video_plan(settings: &ExportSettings, plan: Option<&EditTimeline>, source_duration: u64) -> Option<EditTimeline> {
    let plan = plan?;
    let start = settings.trim_start.unwrap_or(0) as f64;
    let end = start + plan.duration_ms;
    let contiguous = (settings.trim_end.is_some() || end == source_duration as f64)
        && end <= source_duration as f64
        && plan.segments.iter().all(|segment| !segment.hide_cursor && segment.rate == 1.0
            && segment.source_start_ms.is_some_and(|source|
                (source - segment.output_start_ms - start).abs() < 1e-7));
    if contiguous { None } else { Some(plan.clone()) }
}

/// Cursor closing behavior uses edited bounds even when media can stream
/// contiguously. Call after resolve_plan has validated the source clips.
pub(crate) fn cursor_plan(settings:&ExportSettings,media:Option<&EditTimeline>)->Result<Option<EditTimeline>> {
    if let Some(plan)=media {return Ok(Some(plan.clone()));}
    let closing=settings.cursor_settings.as_ref().is_some_and(|cursor|
        cursor.loop_to_start.unwrap_or(false) || cursor.stop_at_end.unwrap_or(false));
    let overrides=settings.timeline_clips.as_ref().is_some_and(|clips|
        clips.iter().any(|clip|clip.disable_cursor_smoothing));
    if !closing && !overrides {return Ok(None);}
    let Some(clips)=settings.timeline_clips.as_ref() else {return Ok(None);};
    let end=settings.trim_end.map(|time|time as f64).unwrap_or_else(||clips.iter()
        .map(|clip|clip.start_time+clip.duration).fold(0.0,f64::max));
    EditTimeline::build(clips,settings.trim_start.unwrap_or(0) as f64,end).map(Some)
}

/// Owns temporary, uncompressed edited audio until the encoder has finished.
/// Drop also cleans up partial preparation after a failed subprocess.
pub struct PreparedAudio {
    pub tracks: AudioTrackFiles,
    directory: PathBuf,
}
impl PreparedAudio {
    #[cfg(test)]
    pub fn new(root: &Path, plan: &EditTimeline, tracks: &AudioTrackFiles) -> Result<Self> {
        Self::with_control(root, plan, tracks, &Default::default())
    }
    pub fn with_control(root: &Path, plan: &EditTimeline, tracks: &AudioTrackFiles,
        control: &super::super::export_control::ExportControl) -> Result<Self> {
        control.check()?;
        let directory = root.join(format!("timeline-audio-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&directory)?;
        let mut prepared = Self {
            tracks: AudioTrackFiles {
                background: None,
                mic: None,
                system: None,
            },
            directory,
        };
        for (name, source, target) in [
            ("mic", &tracks.mic, &mut prepared.tracks.mic),
            ("system", &tracks.system, &mut prepared.tracks.system),
        ] {
            let Some(source) = source else { continue };
            let output = prepared.directory.join(format!("{name}.wav"));
            let mut args = audio_decode_args_for_channel(plan, source, name == "mic");
            // Replace the raw mono test/stream output with a stereo PCM file.
            args.truncate(args.len() - 7);
            args.extend([
                "-c:a".into(),
                "pcm_f32le".into(),
                "-ac".into(),
                "2".into(),
                "-ar".into(),
                "48000".into(),
                output.to_string_lossy().into_owned(),
            ]);
            control.check()?;
            let child = super::resources::ExportChild::new(Command::new(crate::ffmpeg::binary("ffmpeg")).args(args)
                .stdout(std::process::Stdio::null()).stderr(std::process::Stdio::piped()).spawn()?, control);
            let mut stderr = child.stderr().unwrap();
            let drain = std::thread::spawn(move || {
                use std::io::Read;
                let mut bytes = Vec::new();
                let _ = stderr.read_to_end(&mut bytes);
                bytes
            });
            let status = child.wait()?;
            let stderr = drain.join().unwrap_or_default();
            control.check()?;
            if !status.success() { bail!("Could not prepare edited {name} audio: {}", String::from_utf8_lossy(&stderr)); }
            *target = Some(output);
        }
        Ok(prepared)
    }
}
impl Drop for PreparedAudio {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.directory);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn audio_only_edits_retain_contiguous_video_but_not_audio_trimming() {
        let mut value=serde_json::json!({"trim_start":250,"trim_end":1750,"timeline_clips":[
            {"id":"a","startTime":0,"duration":1000,"sourceIn":0,"sourceOut":1000,"playbackRate":1,"trackOrder":0,"muted":false,"micVolume":0.25},
            {"id":"b","startTime":1000,"duration":1000,"sourceIn":1000,"sourceOut":2000,"playbackRate":1,"trackOrder":0,"muted":true}
        ]});
        let verify=|value:serde_json::Value, needs_video:bool| {
            let settings:ExportSettings=serde_json::from_value(value).unwrap();
            let audio=resolve_plan(&settings,Path::new("recording.mp4"),2000).unwrap();
            assert!(audio.is_some());
            assert_eq!(video_plan(&settings,audio.as_ref(),2000).is_some(),needs_video);
            audio.unwrap()
        };
        let audio=verify(value.clone(),false);
        assert_eq!(audio.duration_ms,1500.0);
        assert_eq!(audio.source_at(0.0).unwrap().1,250.0);
        assert_eq!(audio.segments[0].mic_volume,0.25);
        assert!(audio.segments[1].muted);
        value["timeline_clips"][1]["hideCursor"]=true.into();
        verify(value.clone(),true);
        value["timeline_clips"][1]["hideCursor"]=false.into();
        value["timeline_clips"][1]["sourceIn"]=500.into();
        value["timeline_clips"][1]["sourceOut"]=1500.into();
        verify(value.clone(),true);
        value["timeline_clips"][1]["playbackRate"]=0.5.into();
        value["timeline_clips"][1]["sourceOut"]=1000.into();
        verify(value,true);
    }

    #[test]
    fn cursor_closing_bounds_do_not_disable_contiguous_media_fast_path() {
        for closing in ["loop_to_start","stop_at_end"] {
            let mut value=serde_json::json!({"trim_start":250,"trim_end":750,"cursor_settings":{},"timeline_clips":[
                {"id":"one","startTime":0,"duration":1000,"sourceIn":0,"sourceOut":1000,"playbackRate":1,"trackOrder":0,"muted":false}
            ]});
            value["cursor_settings"][closing]=true.into();
            let settings:ExportSettings=serde_json::from_value(value).unwrap();
            let media=resolve_plan(&settings,Path::new("recording.mp4"),1000).unwrap();
            assert!(media.is_none());
            let cursor=cursor_plan(&settings,media.as_ref()).unwrap().unwrap();
            assert_eq!(cursor.duration_ms,500.0);
            assert_eq!(cursor.source_at(0.0).unwrap().1,250.0);
            assert_eq!(cursor.source_at(499.0).unwrap().1,749.0);
        }
    }
    #[test]
    fn contiguous_edits_keep_fast_path_but_speed_and_mute_require_mapping() {
        let mut value = serde_json::json!({"trim_start":250,"trim_end":750,"timeline_clips":[
            {"id":"a","startTime":0,"duration":1000,"sourceIn":0,"sourceOut":1000,"playbackRate":1,"trackOrder":0,"muted":false}
        ]});
        let parse = |v| serde_json::from_value::<ExportSettings>(v).unwrap();
        assert!(
            resolve_plan(&parse(value.clone()), Path::new("recording.mp4"), 1000)
                .unwrap()
                .is_none()
        );
        value["timeline_clips"][0]["micVolume"] = 0.25.into();
        assert!(resolve_plan(&parse(value.clone()), Path::new("recording.mp4"), 1000).unwrap().is_some());
        value["timeline_clips"][0]["micVolume"] = 1.0.into();
        value["timeline_clips"][0]["systemVolume"] = 0.0.into();
        assert!(resolve_plan(&parse(value.clone()), Path::new("recording.mp4"), 1000).unwrap().is_some());
        value["timeline_clips"][0]["systemVolume"] = 1.0.into();
        value["timeline_clips"][0]["muted"] = true.into();
        assert!(
            resolve_plan(&parse(value.clone()), Path::new("recording.mp4"), 1000)
                .unwrap()
                .is_some()
        );
        value["timeline_clips"][0]["muted"] = false.into();
        value["timeline_clips"][0]["hideCursor"] = true.into();
        let hidden=resolve_plan(&parse(value.clone()),Path::new("recording.mp4"),1000).unwrap().unwrap();
        assert!(hidden.segments.iter().all(|segment|segment.hide_cursor));
        value["timeline_clips"][0]["hideCursor"] = false.into();
        value["timeline_clips"][0]["playbackRate"] = 0.5.into();
        value["timeline_clips"][0]["duration"] = 2000.into();
        let plan = resolve_plan(&parse(value.clone()), Path::new("recording.mp4"), 1000)
            .unwrap()
            .unwrap();
        assert_eq!(plan.source_at(0.0).unwrap().1, 125.0);
        value["timeline_clips"][0]["sourceOut"] = 2000.into();
        assert!(resolve_plan(&parse(value), Path::new("recording.mp4"), 1000).is_err());
    }
}


#[cfg(test)]
mod smoothing_override_tests {
    use super::*;
    #[test]
    fn cursor_smoothing_override_keeps_contiguous_media_fast_path() {
        let settings:ExportSettings=serde_json::from_value(serde_json::json!({
            "trim_start":0,"trim_end":1000,"timeline_clips":[
                {"id":"raw","startTime":0,"duration":1000,"sourceIn":0,"sourceOut":1000,
                 "playbackRate":1,"trackOrder":0,"muted":false,"disableCursorSmoothing":true}
            ]
        })).unwrap();
        let media=resolve_plan(&settings,Path::new("recording.mp4"),1000).unwrap();
        assert!(media.is_none());
        let cursor=cursor_plan(&settings,media.as_ref()).unwrap().unwrap();
        assert!(cursor.segments[0].disable_cursor_smoothing);
        assert!(video_plan(&settings,Some(&cursor),1000).is_none());
        let old:super::super::super::timeline::EditClip=serde_json::from_value(serde_json::json!({
            "id":"old","startTime":0,"duration":1000,"sourceIn":0,"sourceOut":1000,
            "playbackRate":1,"trackOrder":0,"muted":false
        })).unwrap();
        assert!(!old.disable_cursor_smoothing);
    }
}
