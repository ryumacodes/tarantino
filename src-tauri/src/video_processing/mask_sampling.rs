//! Geometry planning for the bounded quadrant color sampler.
use anyhow::{ensure,Result};
use super::mask_cover::CoverPlan;
pub(super) fn plan(bounds:[f64;4],uv:[f64;4],size:[u32;2],levels:u32)->Result<Option<CoverPlan>> {
    ensure!(bounds.iter().chain(uv.iter()).all(|v|v.is_finite())
        &&bounds[2]>0.0&&bounds[3]>0.0&&uv[2]>0.0&&uv[3]>0.0
        &&size.iter().all(|v|*v>0)&&(1..=32).contains(&levels),"Invalid mask sampling geometry");
    let left=((bounds[0]-uv[0])/uv[2]*f64::from(size[0])).max(0.0);
    let top=((bounds[1]-uv[1])/uv[3]*f64::from(size[1])).max(0.0);
    let right=((bounds[0]+bounds[2]-uv[0])/uv[2]*f64::from(size[0])).min(f64::from(size[0]));
    let bottom=((bounds[1]+bounds[3]-uv[1])/uv[3]*f64::from(size[1])).min(f64::from(size[1]));
    ensure!([left,top,right,bottom].iter().all(|v|v.is_finite()),"Mask coordinate overflow");
    if right<=left||bottom<=top{return Ok(None);}
    let width=right-left;let height=bottom-top;
    let max_level=(levels-1).min((width.min(height)/8.0).log2().floor().max(0.0) as u32);
    for level in 0..=max_level {
        let scale=2f64.powi(level as i32);
        let support=if level==0 {0.5}else{1.5*scale-2.0};
        let ix=(scale+support).min(width/2.0);let iy=(scale+support).min(height/2.0);
        let lattice=[left+ix,top+iy,width-2.0*ix,height-2.0*iy];
        let mut x=(lattice[2]/(2.0*scale)).ceil().max(1.0) as u64;
        let mut y=(lattice[3]/(2.0*scale)).ceil().max(1.0) as u64;
        if x*y>256&&level<max_level {continue;}
        if x*y>256 {
            if x>=y {x=(256/y).max(1);y=y.min(256);}
            else {y=(256/x).max(1);x=x.min(256);}
        }
        return Ok(Some(CoverPlan {lattice:lattice.map(|v|v as f32),taps:[x as u32,y as u32],level,
            level_span:size.map(|v|((f64::from(v)/scale).floor().max(1.0)*scale) as f32)}));
    }
    Ok(None)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn rejects_invalid_geometry_and_omits_offscreen_regions() {
        assert!(plan([0.0,0.0,1.0,1.0],[0.0,0.0,0.0,1.0],[64,64],1).is_err());
        assert!(plan([2.0,2.0,1.0,1.0],[0.0,0.0,1.0,1.0],[64,64],1).unwrap().is_none());
    }
    #[test]
    #[ignore="requires independently validated sampling fixtures"]
    fn matches_validated_sampling_plans() {
        let path=std::env::var("TARANTINO_MASK_SAMPLING_FIXTURES").unwrap();
        let fixtures:serde_json::Value=serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap();
        for case in fixtures.as_array().unwrap() {
            let rect=|key:&str|["x","y","width","height"].map(|field|case[key][field].as_f64().unwrap());
            let size=["width","height"].map(|field|case["size"][field].as_u64().unwrap() as u32);
            let actual=plan(rect("bounds"),rect("uvRect"),size,case["levels"].as_u64().unwrap() as u32).unwrap().unwrap();
            let expected=&case["plan"];
            assert_eq!(actual.level,expected["level"].as_u64().unwrap() as u32);
            assert_eq!(actual.taps,["x","y"].map(|field|expected["tapsPerQuadrant"][field].as_u64().unwrap() as u32));
            for (index,field) in ["x","y","width","height"].iter().enumerate(){
                let value=expected["lattice"][field].as_f64().unwrap() as f32;
                assert_eq!(actual.lattice[index],value);
            }
            assert_eq!(actual.level_span,["width","height"].map(|field|expected["levelSpan"][field].as_f64().unwrap() as f32));
        }
    }
}
