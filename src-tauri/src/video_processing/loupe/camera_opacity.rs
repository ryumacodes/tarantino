//! Immutable source-time opacity samples shared by playback and export.
use super::camera_overlap::CameraOverlapFade;
use crate::cursor_renderer::SpringConfig;
use anyhow::{bail, Result};
use serde::Serialize;

const FPS: f64 = 120.0;
#[derive(Clone, Copy, Debug, PartialEq, Serialize)]
pub struct CameraOpacity {
    pub stage: f64,
    pub lens_content: f64,
    /// Normalized stage rectangle, shared across preview and export resolutions.
    pub rect: Option<[f64;4]>,
}
/// Coverage uses the target lens geometry. Presence uses its animated geometry.
/// Keeping them separate avoids applying the lens spring twice to the camera fade.
pub struct OverlapSample {
    pub coverage: f64,
    pub presence: f64,
}
pub struct CameraOpacityTimeline { frames: Vec<CameraOpacity>, duration_ms: u64 }
impl CameraOpacityTimeline {
    pub fn new(duration_ms:u64, spring:SpringConfig,
        mut sample:impl FnMut(f64)->Result<OverlapSample>)->Result<Self> {
        if ![spring.tension,spring.friction,spring.mass].iter().all(|v|v.is_finite() && *v>=0.0) {
            bail!("Invalid camera overlap spring");
        }
        let count=(duration_ms as f64*FPS/1000.0).ceil() as usize;
        let mut fade=CameraOverlapFade::default();
        let mut frames=Vec::with_capacity(count);
        for index in 0..count {
            let input=sample(index as f64*1000.0/FPS)?;
            if !input.coverage.is_finite() || !input.presence.is_finite() {
                bail!("Invalid camera overlap sample");
            }
            frames.push(CameraOpacity {
                rect:None,
                stage:fade.step(input.coverage.clamp(0.0,1.0),&spring,1.0/FPS),
                lens_content:(1.0-input.presence).clamp(0.0,1.0),
            });
        }
        Ok(Self{frames,duration_ms})
    }
    pub fn from_frames(duration_ms:u64, frames:Vec<CameraOpacity>) -> Self { Self {frames,duration_ms} }
    /// Outside the recorded range there is no visible camera. Sampling is read-only.
    pub fn sample(&self,source_ms:f64)->CameraOpacity {
        if source_ms.is_finite() && source_ms>=0.0 && source_ms < self.duration_ms as f64 {
            let index=(source_ms*FPS/1000.0+1e-7).floor() as usize;
            if let Some(frame)=self.frames.get(index) {return *frame;}
        }
        CameraOpacity{stage:0.0,lens_content:0.0,rect:None}
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    fn timeline()->CameraOpacityTimeline {
        CameraOpacityTimeline::new(3000,SpringConfig{tension:470.0,friction:70.0,mass:3.0},|t|
            Ok(OverlapSample{coverage:if (500.0..1500.0).contains(&t){0.75}else{0.0},
                presence:((t-500.0)/500.0).clamp(0.0,1.0)})).unwrap()
    }
    #[test]
    fn seeking_and_cadence_select_identical_source_samples() {
        let timeline=timeline();
        let expected=timeline.sample(1000.0);
        for time in [2500.0,0.0,1500.0,100.0,1000.0] {timeline.sample(time);}
        assert_eq!(timeline.sample(1000.0),expected);
        for fps in [24.0,30.0,60.0,120.0] {
            assert_eq!(timeline.sample((fps/2.0)*1000.0/fps),timeline.sample(500.0));
            assert_eq!(timeline.sample(fps*1000.0/fps),expected);
        }
        assert_eq!(timeline.sample(0.0).stage,1.0);
        assert!(expected.stage<0.1);
        assert_eq!(timeline.sample(2990.0).stage,1.0);
    }
    #[test]
    fn fractional_final_tick_does_not_extend_camera_past_source() {
        let spring=SpringConfig{tension:470.0,friction:70.0,mass:3.0};
        let timeline=CameraOpacityTimeline::new(10,spring,|_|Ok(OverlapSample{coverage:0.0,presence:0.0})).unwrap();
        assert_eq!(timeline.sample(9.9).stage,1.0);
        assert_eq!(timeline.sample(10.0).stage,0.0);
        assert_eq!(timeline.sample(f64::NAN).stage,0.0);
    }
    #[test]
    fn lens_content_factor_does_not_inherit_camera_spring() {
        let timeline=timeline();
        assert_eq!(timeline.sample(750.0).lens_content,0.5);
        assert_eq!(timeline.sample(1000.0).lens_content,0.0);
        assert!(timeline.sample(750.0).stage>0.0);
        assert_eq!(timeline.sample(-1.0).stage,0.0);
        assert_eq!(timeline.sample(3000.0).stage,0.0);
    }
}
