use anyhow::{ensure,Result};
use serde::{Deserialize,Serialize};
use super::mask_composite::MaskPlacement;
#[derive(Clone,Debug,Deserialize,Serialize)]
#[serde(rename_all="camelCase")]
pub struct MaskSpan {
    start:u64,end:u64,rect:[f64;4],radius:f64,sensitive:bool,highlight_opacity:f64,
}
#[derive(Clone,Debug,Deserialize,Serialize)]
pub struct MaskExport {frames:u64,spans:Vec<MaskSpan>}
pub(super) struct Prepared {spans:Vec<MaskSpan>,source_rect:[f64;4]}
impl Prepared {
    pub fn new(input:MaskExport,frames:u64,size:[u32;2],view:[f32;4])->Result<Self> {
        ensure!(input.frames==frames&&size.iter().all(|v|*v>0)&&input.spans.len()<=100000,"Invalid mask export size");
        let mut end=0;
        for span in &input.spans {
            ensure!(span.start>=end&&span.end>span.start&&span.end<=frames
                &&span.rect.iter().all(|v|v.is_finite()&&v.abs()<=1000000.0)&&span.rect[2]>0.0&&span.rect[3]>0.0
                &&span.radius.is_finite()&&(0.0..=1000000.0).contains(&span.radius)
                &&span.highlight_opacity.is_finite()&&(0.1..=0.9).contains(&span.highlight_opacity),"Invalid mask export span");
            end=span.end;
        }
        let [width,height]=size.map(f64::from);
        let view=view.map(f64::from);
        Ok(Self{spans:input.spans,source_rect:[view[0]*width,view[1]*height,view[2]*width,view[3]*height]})
    }
    pub fn sample(&self,frame:u64)->Option<MaskPlacement> {
        let index=self.spans.partition_point(|span|span.start<=frame).checked_sub(1)?;
        let span=&self.spans[index];if frame>=span.end{return None;}
        let [x,y,w,h]=self.source_rect;
        Some(MaskPlacement{rect:[(x+span.rect[0]*w) as f32,(y+span.rect[1]*h) as f32,(span.rect[2]*w) as f32,(span.rect[3]*h) as f32],
            radius:(span.radius*w) as f32,opacity:1.0,highlight_opacity:span.highlight_opacity as f32,sensitive:span.sensitive})
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    fn input()->MaskExport {serde_json::from_value(serde_json::json!({"frames":60,"spans":[{
        "start":10,"end":20,"rect":[0.25,0.1,0.5,0.4],"radius":0.02,"sensitive":true,"highlightOpacity":0.5}]})).unwrap()}
    #[test]
    fn maps_to_decoder_pixels_and_excludes_gap_and_end_frames(){
        let prepared=Prepared::new(input(),60,[400,200],[0.0,0.0,1.0,1.0]).unwrap();
        assert!(prepared.sample(9).is_none());assert!(prepared.sample(20).is_none());
        let p=prepared.sample(10).unwrap();assert_eq!(p.rect,[100.0,20.0,200.0,80.0]);assert_eq!(p.radius,8.0);
        assert_eq!(p.opacity,1.0);
    }
    #[test]
    fn maps_crop_relative_bounds_and_radius_into_the_decode_view() {
        let prepared=Prepared::new(input(),60,[400,200],[0.25,0.125,0.5,0.75]).unwrap();
        let placement=prepared.sample(10).unwrap();
        assert_eq!(placement.rect,[150.0,40.0,100.0,60.0]);
        assert_eq!(placement.radius,4.0);
        assert!(prepared.sample(20).is_none());
    }

    #[test]
    fn rejects_ambiguous_or_invalid_coverage(){
        let mut value=input();value.spans.push(value.spans[0].clone());assert!(Prepared::new(value,60,[400,200],[0.0,0.0,1.0,1.0]).is_err());
        assert!(Prepared::new(input(),59,[400,200],[0.0,0.0,1.0,1.0]).is_err());
        let mut value=input();value.spans[0].rect[2]=f64::NAN;assert!(Prepared::new(value,60,[400,200],[0.0,0.0,1.0,1.0]).is_err());
    }
}
