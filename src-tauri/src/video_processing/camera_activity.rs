//! Source-time pointer activity after edited playback bounds and loop closure.
use super::{ExportSettings, timeline::EditTimeline};
use crate::cursor_renderer::{CursorEvent, planner::{source_window, press_window}};
use anyhow::Result;
use std::path::Path;

#[derive(Clone, Copy, Debug)]
pub(crate) struct Sample {
    pub time: f64,
    pub x: f64,
    pub y: f64,
    pub button: bool,
}

#[derive(Default)]
pub(crate) struct Tracks {
    pub raw: Vec<Sample>,
    pub grouped: Vec<Sample>,
    buttons: Vec<Sample>,
}

impl Tracks {
    pub fn prepare(events: &[CursorEvent], logical: (f64,f64), source: (u32,u32),
        settings: &ExportSettings, duration: u64, recording: &Path) -> Result<Self> {
        let cursor=settings.cursor_settings.as_ref();
        let looped=cursor.is_some_and(|c|c.loop_to_start.unwrap_or(false));
        let loop_ms=if looped {cursor.and_then(|c|c.loop_duration_ms).unwrap_or(500) as f64} else {0.0};
        let start=settings.trim_start.unwrap_or(0) as f64;
        let bounds=if let Some(clips)=&settings.timeline_clips {
            let extent=clips.iter().map(|c|c.start_time+c.duration).fold(0.0,f64::max);
            let end=settings.trim_end.map_or(extent,|t|t as f64);
            let timeline=EditTimeline::build(clips,start,end)?;
            press_window::boundary(&timeline,0.0,recording).zip(
                press_window::boundary(&timeline,(timeline.duration_ms-loop_ms).max(0.0),recording))
        } else {
            let end=settings.trim_end.unwrap_or(duration) as f64;
            Some((start,(end-loop_ms).max(start)))
        };
        let crop=settings.source_crop.map(|c|c.resolve(source.0,source.1)).transpose()?;
        let size=crop.map_or(logical,|c| {
            let rect=c.normalized(); (logical.0*rect.width,logical.1*rect.height)
        });
        let mut raw:Vec<_>=events.iter().map(|e| {
            let (x,y)=crop.map_or((e.x,e.y),|c|c.source_point(e.x,e.y));
            Sample {time:e.timestamp_ms as f64,x,y,button:e.is_click||e.is_release}
        }).collect();
        raw.sort_by(|a,b|a.time.total_cmp(&b.time));
        Ok(Self::select(&raw,size,bounds,looped))
    }

    fn select(raw:&[Sample],size:(f64,f64),bounds:Option<(f64,f64)>,looped:bool)->Self {
        let raw=bounds.map_or_else(Vec::new,|(start,end)|source_window::select(
            raw,start,end,looped,|e|e.time,|e,time|Sample{time,..e}));
        // Window selection resets the distance anchor. Never sort the loop seed.
        let mut grouped:Vec<Sample>=Vec::new();
        for &event in &raw {
            if event.button || grouped.last().is_none_or(|previous|
                ((event.x-previous.x)*size.0).hypot((event.y-previous.y)*size.1)>=10.0) {
                grouped.push(event);
            }
        }
        let buttons=raw.iter().copied().filter(|e|e.button).collect();
        Self{raw,grouped,buttons}
    }

    pub fn fallback(&self,time:f64,smooth:bool)->Option<Sample> {
        if smooth {
            // Binary lookup must operate on the button-only sequence, including
            // the original ordering of a synthetic loop closure.
            let index=self.buttons.partition_point(|e|e.time<time);
            if let Some(event)=self.buttons.get(index).filter(|e|e.time-time<=500.0) {return Some(*event);}
        }
        let first=self.raw.partition_point(|e|e.time<time);
        let index=if self.raw.get(first).is_some_and(|e|e.time==time) {first} else {first.saturating_sub(1)};
        self.raw.get(index).copied()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn speed_and_loop_use_edited_source_bounds_before_thinning() {
        let settings:ExportSettings=serde_json::from_value(serde_json::json!({
            "timeline_clips":[{"id":"clip","startTime":0,"duration":500,
                "sourceIn":300.25,"sourceOut":1300.25,"playbackRate":2,"trackOrder":0,"muted":false}],
            "cursor_settings":{"loop_to_start":true,"loop_duration_ms":100},
            "source_crop":{"x":0.5,"y":0,"width":0.5,"height":1}
        })).unwrap();
        let events=[(0,0.0),(400,0.6),(600,0.65),(1000,0.8),(1200,0.95)].map(|(timestamp_ms,x)|
            CursorEvent{timestamp_ms,x,y:0.5,is_click:false,is_release:false});
        let tracks=Tracks::prepare(&events,(100.0,100.0),(100,100),&settings,1500,Path::new("source.mp4")).unwrap();
        assert_eq!(tracks.raw.iter().map(|e|e.time).collect::<Vec<_>>(),[300.25,600.0,1000.0,1001.0]);
        assert_eq!(tracks.grouped.iter().map(|e|e.time).collect::<Vec<_>>(),[300.25,1000.0,1001.0]);
        assert!((tracks.raw[0].x-0.2).abs()<1e-12);
        assert!((tracks.raw[3].x-0.2).abs()<1e-12);
        assert!(tracks.raw.iter().all(|e|e.time<1200.0));
    }

    #[test]
    fn excluded_release_does_not_anticipate_into_trimmed_camera_range() {
        let settings:ExportSettings=serde_json::from_value(serde_json::json!({"trim_start":200,"trim_end":700})).unwrap();
        let events=[(0,0.1,false),(300,0.55,false),(800,0.9,true)].map(|(timestamp_ms,x,is_release)|
            CursorEvent{timestamp_ms,x,y:0.5,is_click:false,is_release});
        let tracks=Tracks::prepare(&events,(100.0,100.0),(100,100),&settings,1000,Path::new("source.mp4")).unwrap();
        assert_eq!(tracks.fallback(500.0,true).unwrap().x,0.55);
        assert_eq!(tracks.raw[0].time,200.0);
    }

    #[test]
    #[ignore = "requires external camera source window fixtures"]
    fn prepared_camera_windows_match_external_samples() {
        #[derive(serde::Deserialize)]
        struct Point { #[serde(rename="recordingTimeMs")] time:f64,x:f64,y:f64,#[serde(rename="type")]kind:String }
        #[derive(serde::Deserialize)]
        struct Case {events:Vec<Point>,start:f64,end:f64,looped:bool,expected:Vec<Point>,grouped:Vec<Point>}
        let path=std::env::var("TARANTINO_ACTIVITY_WINDOW_FIXTURES").unwrap();
        let cases:Vec<Case>=serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap();
        for case in cases {
            let raw=case.events.iter().map(|e|Sample{time:e.time,x:e.x,y:e.y,button:matches!(e.kind.as_str(),"mouseDown"|"mouseUp")}).collect::<Vec<_>>();
            let tracks=Tracks::select(&raw,(100.0,100.0),Some((case.start,case.end)),case.looped);
            for (actual,expected) in [(&tracks.raw,&case.expected),(&tracks.grouped,&case.grouped)] {
                assert_eq!(actual.len(),expected.len());
                for (a,e) in actual.iter().zip(expected.iter()) {
                    assert_eq!((a.time,a.x,a.y),(e.time,e.x,e.y));
                    assert_eq!(a.button,matches!(e.kind.as_str(),"mouseDown"|"mouseUp"));
                }
            }
        }
    }
}
