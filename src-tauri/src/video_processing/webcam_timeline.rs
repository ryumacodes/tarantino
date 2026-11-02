//! Source-time camera layout states, evaluated identically on seek and playback.
use super::{gpu_compositor::WebcamConfig, types::ExportSettings};
use anyhow::{Result, bail};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WebcamKeyframe {
    pub id: String,
    pub time: f64,
    pub x: f64,
    pub y: f64,
    pub size: f64,
    pub shape: String,
    pub visible: bool,
    pub layout: Option<String>,
}
#[derive(Clone, Copy)]
pub struct CameraBounds {
    pub pos_x: f32,
    pub pos_y: f32,
    pub size: f32,
}
/// Match preview containment in normalized output coordinates without per-frame allocation.
pub fn contain_webcam(config: &WebcamConfig, width: u32, height: u32) -> CameraBounds {
    if config.shape == "fullscreen" { return CameraBounds { pos_x: 0.5, pos_y: 0.5, size: 1.0 }; }
    let aspect = width.max(1) as f32 / height.max(1) as f32;
    let size = config.size.clamp(0.0, 1.0_f32.min(1.0 / aspect));
    let half_x = size / 2.0;
    let half_y = size * aspect / 2.0;
    CameraBounds {
        pos_x: config.pos_x.clamp(half_x, 1.0 - half_x),
        pos_y: config.pos_y.clamp(half_y, 1.0 - half_y),
        size,
    }
}

#[derive(Debug,Clone,Serialize,Deserialize,PartialEq)]
pub struct SilenceRange {pub start_ms:f64,pub end_ms:f64}

pub struct WebcamTimeline {
    enabled: bool,
    silence:Vec<SilenceRange>,
    initial: Option<CameraLayoutState>,
    states: Vec<(f64, Option<CameraLayoutState>)>,
}
struct CameraLayoutState { config: WebcamConfig, hidden: bool }

fn layout(mut config: WebcamConfig, mode: &str, visible: bool) -> Result<Option<CameraLayoutState>> {
    match mode {
        "hidden" => {},
        "fullscreen" => {
            config.shape = "fullscreen".into();
            config.pos_x = 0.5;
            config.pos_y = 0.5;
            config.size = 1.0;
        }
        "overlay" => {}
        _ => bail!("Unknown camera layout"),
    }
    Ok(Some(CameraLayoutState { config, hidden: !visible || mode == "hidden" }))
}
impl WebcamTimeline {
    pub fn new(settings: &ExportSettings, base: Option<WebcamConfig>) -> Result<Self> {
        let initial = base
            .clone()
            .map(|c| {
                layout(
                    c,
                    settings.webcam_layout.as_deref().unwrap_or("overlay"),
                    true,
                )
            })
            .transpose()?
            .flatten();
        let mut states = Vec::new();
        for k in settings.webcam_keyframes.as_deref().unwrap_or(&[]) {
            if ![k.time, k.x, k.y, k.size].iter().all(|n| n.is_finite())
                || k.time < 0.0
                || !(0.0..=1.0).contains(&k.x)
                || !(0.0..=1.0).contains(&k.y)
                || !(0.01..=1.0).contains(&k.size)
                || !matches!(k.shape.as_str(), "circle" | "roundrect")
            {
                bail!("Invalid camera keyframe {}", k.id);
            }
            let half = k.size as f32 / 2.0;
            let config = WebcamConfig {
                pos_x: (k.x as f32).clamp(half, 1.0 - half),
                pos_y: (k.y as f32).clamp(half, 1.0 - half),
                size: k.size as f32,
                shape: k.shape.clone(),
            };
            let state = layout(config, k.layout.as_deref().unwrap_or("overlay"), k.visible)?;
            states.push((k.time, if base.is_some() { state } else { None }));
        }
        states.sort_by(|a, b| a.0.total_cmp(&b.0));
        let mut silence=settings.webcam_silence_ranges.clone().unwrap_or_default();
        for range in &silence {
            if !range.start_ms.is_finite() || !range.end_ms.is_finite() || range.start_ms<0.0 || range.end_ms<=range.start_ms {
                bail!("Invalid camera silence range");
            }
        }
        silence.sort_by(|a,b|a.start_ms.total_cmp(&b.start_ms));
        if silence.windows(2).any(|pair|pair[0].end_ms>pair[1].start_ms){bail!("Overlapping camera silence ranges");}
        Ok(Self { initial, states, silence, enabled: settings.webcam_enabled != Some(false) })
    }
    /// Retain hidden geometry for shrink/fade transitions. Legacy consumers can
    /// continue using sample(), which omits fully hidden target states.
    pub fn sample_target(&self, time: f64) -> Option<(&WebcamConfig, bool)> {
        if !self.enabled || !time.is_finite() || time < 0.0 { return None; }
        let index = self.states.partition_point(|(t, _)| *t <= time);
        let state=if index == 0 {self.initial.as_ref()} else {self.states[index - 1].1.as_ref()}?;
        let quiet=self.silence.partition_point(|range|range.start_ms<=time);
        let silent=state.config.shape!="fullscreen" && quiet>0 && time<self.silence[quiet-1].end_ms;
        Some((&state.config,state.hidden || silent))
    }
    pub fn sample(&self, time: f64) -> Option<&WebcamConfig> {
        self.sample_target(time).and_then(|(config,hidden)|(!hidden).then_some(config))
    }

}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn global_disable_overrides_fullscreen_keys_without_mutating_them() {
        let mut settings: ExportSettings = serde_json::from_value(serde_json::json!({
            "webcam_enabled":false,
            "webcam_keyframes":[{"id":"full","time":1000,"x":0.5,"y":0.5,"size":0.2,"shape":"circle","visible":true,"layout":"fullscreen"}]
        })).unwrap();
        let base=WebcamConfig{pos_x:0.8,pos_y:0.8,size:0.15,shape:"circle".into()};
        let disabled=WebcamTimeline::new(&settings,Some(base.clone())).unwrap();
        for time in [2000.0,0.0,1000.0,999.0] { assert!(disabled.sample(time).is_none()); }
        settings.webcam_enabled=Some(true);
        let enabled=WebcamTimeline::new(&settings,Some(base)).unwrap();
        assert_eq!(enabled.sample(0.0).unwrap().shape,"circle");
        assert_eq!(enabled.sample(1000.0).unwrap().shape,"fullscreen");
        assert_eq!(settings.webcam_keyframes.as_ref().unwrap().len(),1);
    }
    #[test]
    fn silence_hides_overlays_but_preserves_fullscreen_and_rejects_invalid_ranges(){
        let mut settings:ExportSettings=serde_json::from_value(serde_json::json!({
            "webcam_silence_ranges":[{"start_ms":1000,"end_ms":6000}],
            "webcam_keyframes":[{"id":"full","time":2000,"x":0.5,"y":0.5,"size":0.2,"shape":"circle","visible":true,"layout":"fullscreen"}]
        })).unwrap();
        let base=WebcamConfig{pos_x:0.8,pos_y:0.8,size:0.15,shape:"circle".into()};
        let timeline=WebcamTimeline::new(&settings,Some(base.clone())).unwrap();
        assert!(timeline.sample(999.0).is_some());assert!(timeline.sample(1000.0).is_none());
        assert_eq!(timeline.sample(2000.0).unwrap().shape,"fullscreen");
        settings.webcam_keyframes=None;
        let timeline=WebcamTimeline::new(&settings,Some(base.clone())).unwrap();
        assert!(timeline.sample(5999.0).is_none());assert!(timeline.sample(6000.0).is_some());
        settings.webcam_silence_ranges=Some(vec![SilenceRange{start_ms:2.0,end_ms:1.0}]);
        assert!(WebcamTimeline::new(&settings,Some(base.clone())).is_err());
        settings.webcam_silence_ranges=Some(vec![SilenceRange{start_ms:0.0,end_ms:10.0},SilenceRange{start_ms:5.0,end_ms:20.0}]);
        assert!(WebcamTimeline::new(&settings,Some(base)).is_err());
    }
    #[test]
    fn overlay_bounds_match_preview_for_wide_and_portrait_canvases() {
        for (width, height) in [(1920,1080),(1080,1920),(2560,1080),(1000,1000)] {
            for x in [0.0,0.5,1.0] { for y in [0.0,0.5,1.0] { for size in [0.15,0.5,1.0] {
                let c = contain_webcam(&WebcamConfig {pos_x:x,pos_y:y,size,shape:"circle".into()},width,height);
                let half = c.size * width as f32 / 2.0;
                assert!(c.pos_x * width as f32 - half >= -0.001);
                assert!(c.pos_y * height as f32 - half >= -0.001);
                assert!(c.pos_x * width as f32 + half <= width as f32 + 0.001);
                assert!(c.pos_y * height as f32 + half <= height as f32 + 0.001);
            }}}
        }
        let c = contain_webcam(&WebcamConfig {pos_x:0.895,pos_y:0.895,size:0.15,shape:"circle".into()},1920,1080);
        assert!((c.pos_y - 0.8666667).abs() < 0.00001);
    }
    #[test]
    fn camera_states_are_source_timed_and_seek_independent() {
        let settings=serde_json::from_value(serde_json::json!({"webcam_keyframes":[
            {"id":"hidden","time":1000,"x":0.2,"y":0.3,"size":0.2,"shape":"circle","visible":false},
            {"id":"full","time":1500,"x":0.2,"y":0.3,"size":0.2,"shape":"roundrect","visible":true,"layout":"fullscreen"}
        ]})).unwrap();
        let base = WebcamConfig {
            pos_x: 0.8,
            pos_y: 0.8,
            size: 0.15,
            shape: "circle".into(),
        };
        let timeline = WebcamTimeline::new(&settings, Some(base)).unwrap();
        for time in [1600.0, 200.0, 1200.0, 1500.0, 999.0] {
            if time < 1000.0 {
                assert_eq!(timeline.sample(time).unwrap().pos_x, 0.8);
            } else if time < 1500.0 {
                assert!(timeline.sample(time).is_none());
            } else {
                assert_eq!(timeline.sample(time).unwrap().shape, "fullscreen");
            }
        }
        assert!(
            WebcamTimeline::new(&settings, None)
                .unwrap()
                .sample(1600.0)
                .is_none()
        );
    }
}
