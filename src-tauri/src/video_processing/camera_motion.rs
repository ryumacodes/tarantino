//! Seek-independent camera rectangle and opacity animation in stage pixels.
use crate::cursor_renderer::{spring_step_with_precision, SpringConfig, SpringState, TargetTolerance};
use anyhow::{bail, Result};
use super::webcam_timeline::{contain_webcam, WebcamTimeline};

pub const CAMERA_MOTION_FPS: f64 = 120.0;
const VISIBILITY_SPRING: SpringConfig = SpringConfig { tension:530.0, friction:40.0, mass:1.0 };

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CameraMotionTarget {
    /// Top-left x/y and width/height after layout, crop and hidden-state scaling.
    pub rect: [f64;4],
    pub opacity: f64,
    /// Layout/silence hiding, distinct from magnifier overlap opacity.
    pub hidden: bool,
}
impl WebcamTimeline {
    /// Resolve layout before animation. Magnifier coverage changes opacity but
    /// must not trigger the visibility spring or the hidden-state shrink.
    pub fn motion_target(&self, source_ms: f64, width: u32, height: u32,
        covered: bool) -> Option<CameraMotionTarget> {
        if width == 0 || height == 0 { return None; }
        let (camera, hidden) = self.sample_target(source_ms)?;
        let bounds = contain_webcam(camera, width, height);
        let (w, h) = (width as f64, height as f64);
        let mut rect = if camera.shape == "fullscreen" { [0.0, 0.0, w, h] } else {
            let side = bounds.size as f64 * w;
            [bounds.pos_x as f64 * w - side / 2.0,
                bounds.pos_y as f64 * h - side / 2.0, side, side]
        };
        if hidden {
            let anchor_x = ((rect[0] + rect[2] / 2.0) / w).clamp(0.0, 1.0);
            let anchor_y = ((rect[1] + rect[3] / 2.0) / h).clamp(0.0, 1.0);
            rect[0] += rect[2] * 0.1 * anchor_x;
            rect[1] += rect[3] * 0.1 * anchor_y;
            rect[2] *= 0.9;
            rect[3] *= 0.9;
        }
        Some(CameraMotionTarget { rect, opacity: if hidden || covered { 0.0 } else { 1.0 }, hidden })
    }
}
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CameraMotionFrame {
    pub rect: [f64;4],
    pub opacity: f64,
}
pub struct CameraMotionTimeline {
    frames: Vec<CameraMotionFrame>,
}
impl CameraMotionTimeline {
    /// Targets are consecutive source samples at 120 Hz. Simulation is performed
    /// once; frame lookup never advances state or depends on output cadence.
    pub fn new(targets:&[CameraMotionTarget], screen:SpringConfig)->Result<Self> {
        if ![screen.mass,screen.tension,screen.friction].iter().all(|v|v.is_finite() && *v>=0.0) {
            bail!("Invalid camera motion spring");
        }
        for target in targets {
            if !target.rect.iter().chain(std::iter::once(&target.opacity)).all(|v|v.is_finite())
                || target.rect[2]<0.0 || target.rect[3]<0.0 || !(0.0..=1.0).contains(&target.opacity) {
                bail!("Invalid camera motion target");
            }
        }
        let Some(first)=targets.first() else {return Ok(Self{frames:Vec::new()});};
        let channels=|t:&CameraMotionTarget|[t.rect[0],t.rect[1],t.rect[2],t.rect[3],t.opacity];
        let mut states=channels(first).map(|value|SpringState{value,velocity:0.0});
        let mut precision=std::array::from_fn::<_,5,_>(|_|TargetTolerance::default());
        let mut frames=Vec::with_capacity(targets.len());
        for (index,target) in targets.iter().enumerate() {
            // Visibility changes use a fixed fast spring. Overlap fades retain
            // the screen spring because they do not change the layout's hidden state.
            let spring=if target.hidden || targets[index.saturating_sub(24)].hidden {
                &VISIBILITY_SPRING
            } else {&screen};
            for (channel,value) in channels(target).into_iter().enumerate() {
                let state=&mut states[channel];
                let tolerance=precision[channel].update(state.value,value);
                if index>0 {
                    *state=spring_step_with_precision(state.value,value,state.velocity,spring,
                        1.0/CAMERA_MOTION_FPS,tolerance);
                }
            }
            frames.push(CameraMotionFrame{rect:[states[0].value,states[1].value,
                states[2].value.max(0.0),states[3].value.max(0.0)],opacity:states[4].value.clamp(0.0,1.0)});
        }
        Ok(Self{frames})
    }
    pub fn sample(&self,source_ms:f64)->Option<CameraMotionFrame> {
        if !source_ms.is_finite() || source_ms<0.0 {return None;}
        self.frames.get((source_ms*CAMERA_MOTION_FPS/1000.0+1e-7).floor() as usize).copied()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn spring()->SpringConfig{SpringConfig{tension:170.0,friction:50.0,mass:3.0}}
    fn target(x:f64)->CameraMotionTarget{CameraMotionTarget{rect:[x,10.0,100.0,80.0],opacity:1.0,hidden:false}}
    #[test]
    fn layout_targets_preserve_hidden_geometry_and_separate_overlap() {
        use crate::video_processing::{types::ExportSettings, gpu_compositor::WebcamConfig};
        let settings:ExportSettings=serde_json::from_value(serde_json::json!({
            "webcam_silence_ranges":[{"start_ms":500,"end_ms":800}],
            "webcam_keyframes":[
                {"id":"hidden","time":1000,"x":0.8,"y":0.8,"size":0.2,"shape":"circle","visible":true,"layout":"hidden"},
                {"id":"full","time":1500,"x":0.2,"y":0.3,"size":0.2,"shape":"roundrect","visible":true,"layout":"fullscreen"}
            ]
        })).unwrap();
        let timeline=WebcamTimeline::new(&settings,Some(WebcamConfig{
            pos_x:0.8,pos_y:0.8,size:0.2,shape:"circle".into(),
        })).unwrap();
        let visible=timeline.motion_target(0.0,1000,1000,false).unwrap();
        let covered=timeline.motion_target(0.0,1000,1000,true).unwrap();
        assert_eq!(covered.rect,visible.rect);
        assert_eq!(covered.opacity,0.0);
        assert!(!covered.hidden);
        let silent=timeline.motion_target(500.0,1000,1000,false).unwrap();
        let hidden=timeline.motion_target(1000.0,1000,1000,false).unwrap();
        assert_eq!(silent,hidden);
        assert!(hidden.hidden);
        assert_eq!(hidden.opacity,0.0);
        assert!((hidden.rect[2]-180.0).abs()<0.0001);
        // Shrink toward the camera's stage-relative anchor, preserving its corner bias.
        assert!((hidden.rect[0]-visible.rect[0]-16.0).abs()<0.0001);
        assert_eq!(timeline.motion_target(800.0,1000,1000,false).unwrap(),visible);
        assert_eq!(timeline.motion_target(1500.0,1920,1080,false).unwrap().rect,[0.0,0.0,1920.0,1080.0]);
        assert!(timeline.motion_target(f64::NAN,1000,1000,false).is_none());
        assert!(timeline.motion_target(-1.0,1000,1000,false).is_none());
        assert!(timeline.motion_target(0.0,0,1000,false).is_none());
        assert!(timeline.sample(1000.0).is_none());
        // A seek back to an earlier state must not retain the fullscreen geometry.
        assert_eq!(timeline.motion_target(0.0,1000,1000,false).unwrap(),visible);
    }
    #[test]
    fn rectangle_moves_continuously_and_seek_order_cannot_change_it() {
        let targets:Vec<_>=(0..480).map(|i|target(if i<60 {10.0} else {200.0})).collect();
        let motion=CameraMotionTimeline::new(&targets,spring()).unwrap();
        assert_eq!(motion.sample(0.0).unwrap().rect,targets[0].rect);
        let moving=motion.sample(600.0).unwrap();
        assert!(moving.rect[0]>10.0 && moving.rect[0]<200.0);
        assert_eq!(motion.sample(3900.0).unwrap().rect,targets[479].rect);
        assert_eq!(motion.sample(600.0).unwrap(),moving);
        for fps in [24.0,30.0,60.0] {
            for i in 0..(fps*3.0) as usize {
                let time=i as f64*1000.0/fps;
                let fixed=(time*120.0/1000.0+1e-7).floor() as usize;
                assert_eq!(motion.sample(time),Some(motion.frames[fixed]));
            }
        }
    }
    #[test]
    fn hiding_and_reappearance_use_fast_spring_but_overlap_does_not() {
        let mut targets=vec![target(10.0);240];
        for t in &mut targets[60..120] {t.opacity=0.0;t.hidden=true;}
        let hidden=CameraMotionTimeline::new(&targets,spring()).unwrap();
        for t in &mut targets {t.hidden=false;}
        let overlap=CameraMotionTimeline::new(&targets,spring()).unwrap();
        assert!(hidden.sample(600.0).unwrap().opacity<overlap.sample(600.0).unwrap().opacity);
        assert!(hidden.sample(1100.0).unwrap().opacity>overlap.sample(1100.0).unwrap().opacity);
        assert_eq!(hidden.sample(1900.0).unwrap().opacity,1.0);
    }
    #[test]
    fn visibility_spring_window_ends_exactly_200_ms_after_reappearance() {
        let first_step=|change:usize| {
            let mut targets=vec![target(0.0);160];
            for t in &mut targets[60..120] {t.hidden=true;}
            for t in &mut targets[change..] {t.rect[0]=100.0;}
            let motion=CameraMotionTimeline::new(&targets,spring()).unwrap();
            motion.frames[change].rect[0]
        };
        let fast=first_step(143);
        let normal=first_step(144);
        assert!(fast>normal*2.0,"fast={fast}, normal={normal}");
        assert_eq!(normal,first_step(145));
    }

    #[test]
    fn invalid_targets_and_out_of_range_lookups_are_rejected() {
        let mut bad=target(0.0);bad.rect[2]=-1.0;
        assert!(CameraMotionTimeline::new(&[bad],spring()).is_err());
        let motion=CameraMotionTimeline::new(&[target(0.0)],spring()).unwrap();
        assert_eq!(motion.sample(f64::NAN),None);
        assert_eq!(motion.sample(-1.0),None);
        assert_eq!(motion.sample(9.0),None);
    }
}
