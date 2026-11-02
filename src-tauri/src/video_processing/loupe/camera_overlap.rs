//! Stage-space camera coverage and deterministic overlap opacity.
use crate::cursor_renderer::{spring_step_with_precision, SpringConfig, SpringState, TargetTolerance};

/// Rectangle coordinates and lens coordinates must use the same stage units.
pub fn coverage(center: (f64, f64), radius: f64, rect: [f64; 4]) -> f64 {
    let [x, y, width, height] = rect;
    if ![center.0,center.1,radius,x,y,width,height].iter().all(|v|v.is_finite())
        || radius <= 0.0 || width <= 0.0 || height <= 0.0 { return 0.0; }
    let (x0,x1)=((x-center.0)/radius,(x+width-center.0)/radius);
    let (y0,y1)=((y-center.1)/radius,(y+height-center.1)/radius);
    if x0>=1.0 || x1<=-1.0 || y0>=1.0 || y1<=-1.0 { return 0.0; }
    fn primitive(x:f64)->f64 { (x*(1.0-x*x).max(0.0).sqrt()+x.asin())/2.0 }
    fn quadrant(x:f64,y:f64)->f64 {
        let sign=x.signum()*y.signum();
        let (x,y)=(x.abs().min(1.0),y.abs().min(1.0));
        let flat_end=x.min((1.0-y*y).max(0.0).sqrt());
        sign*(flat_end*y+primitive(x)-primitive(flat_end))
    }
    let area=quadrant(x1,y1)-quadrant(x0,y1)-quadrant(x1,y0)+quadrant(x0,y0);
    (area/((width/radius)*(height/radius))).clamp(0.0,1.0)
}

/// Feed consecutive fixed-rate source-time samples, then retain the resulting
/// values for seekable playback. Do not advance this state from render callbacks.
pub struct CameraOverlapFade {
    state: SpringState,
    tolerance: TargetTolerance,
}
impl Default for CameraOverlapFade {
    fn default()->Self { Self { state:SpringState{value:1.0,velocity:0.0},tolerance:TargetTolerance::default() } }
}
impl CameraOverlapFade {
    pub fn step(&mut self, covered_fraction:f64, spring:&SpringConfig, dt:f64)->f64 {
        let target=if covered_fraction>=0.5 {0.0} else {1.0};
        let precision=self.tolerance.update(self.state.value,target);
        self.state=spring_step_with_precision(self.state.value,target,self.state.velocity,spring,dt,precision);
        self.state.value.clamp(0.0,1.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn circle_area_and_false_bounding_box_overlap() {
        assert!((coverage((0.0,0.0),1.0,[-1.0,-1.0,2.0,2.0])-std::f64::consts::PI/4.0).abs()<1e-12);
        assert_eq!(coverage((0.0,0.0),1.0,[0.8,0.8,0.2,0.2]),0.0);
        assert!((coverage((0.0,0.0),1.0,[-0.1,-0.1,0.2,0.2])-1.0).abs()<1e-12);
        assert_eq!(coverage((0.0,0.0),0.0,[0.0,0.0,1.0,1.0]),0.0);
    }
    #[test]
    fn partial_coverage_matches_independent_area_sampling() {
        for x in [-1.2,-0.9,-0.5,0.0,0.3,0.7] {
            let mut inside=0;
            for j in 0..500 { for i in 0..500 {
                let px=x+(i as f64+0.5)*0.8/500.0;
                let py=-0.4+(j as f64+0.5)*0.8/500.0;
                if px*px+py*py<=1.0 { inside+=1; }
            }}
            assert!((coverage((0.0,0.0),1.0,[x,-0.4,0.8,0.8])-inside as f64/250000.0).abs()<0.0002);
        }
    }
    #[test]
    fn overlap_fades_and_recovers_at_exact_half_threshold() {
        let spring=SpringConfig{tension:470.0,friction:70.0,mass:3.0};
        let mut fade=CameraOverlapFade::default();
        assert_eq!(fade.step(0.4999,&spring,1.0/120.0),1.0);
        let first=fade.step(0.5,&spring,1.0/120.0);
        assert!(first>0.0 && first<1.0);
        for _ in 0..240 {fade.step(0.5,&spring,1.0/120.0);}
        assert_eq!(fade.state.value,0.0);
        let first=fade.step(0.0,&spring,1.0/120.0);
        assert!(first>0.0 && first<1.0);
        for _ in 0..240 {fade.step(0.0,&spring,1.0/120.0);}
        assert_eq!(fade.state.value,1.0);
    }
}
