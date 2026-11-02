//! Animated atlas placement over the encoder's existing RGBA readback buffer.
use anyhow::{ensure,Result};
use serde::{Deserialize,Serialize};
use super::shortcut_overlay::Raster;

#[derive(Debug,Clone,Serialize,Deserialize)]
pub struct Draw {
    pub tile:usize,pub x:f64,pub y:f64,pub width:f64,pub height:f64,pub opacity:f64,
}
#[derive(Debug,Clone,Serialize,Deserialize)]
pub struct MotionSpan {pub start_frame:u64,pub end_frame:u64,pub draws:Vec<Draw>}

pub(super) fn validate(spans:&[MotionSpan],tiles:usize,frames:u64,width:u32,height:u32)->Result<()> {
    ensure!(spans.len()<=1_000_000,"Shortcut animation exceeds the export memory limit");
    let mut previous=0;let mut count=0usize;
    for span in spans {
        ensure!(span.start_frame>=previous&&span.start_frame<span.end_frame&&span.end_frame<=frames,
            "Invalid shortcut animation range");
        previous=span.end_frame;
        count=count.checked_add(span.draws.len()).ok_or_else(||anyhow::anyhow!("Shortcut animation overflow"))?;
        ensure!(count<=1_000_000,"Shortcut animation exceeds the export memory limit");
        validate_draws(&span.draws,tiles,width,height)?;
    }
    Ok(())
}

// Shared by plan validation and the frame renderer without building a temporary plan.
pub(super) fn validate_draws(draws:&[Draw],tiles:usize,width:u32,height:u32)->Result<()> {
    ensure!(draws.len()<=1_000_000,"Shortcut animation exceeds the export memory limit");
    for draw in draws {
        ensure!([draw.x,draw.y,draw.width,draw.height,draw.opacity].iter().all(|v|v.is_finite())
            &&draw.tile<tiles&&draw.width>0.0&&draw.height>0.0
            &&draw.width<=width as f64*4.0&&draw.height<=height as f64*4.0
            &&draw.x.abs()<=width as f64*4.0&&draw.y.abs()<=height as f64*4.0
            &&(0.0..=1.0).contains(&draw.opacity),"Invalid shortcut animation placement");
    }
    Ok(())
}

pub(super) fn composite(draw:&Draw,tile:&Raster,width:u32,height:u32,output:&mut[u8]) {
    if draw.opacity==0.0{return;}
    let left=draw.x.floor().max(0.0).min(width as f64) as usize;
    let top=draw.y.floor().max(0.0).min(height as f64) as usize;
    let right=(draw.x+draw.width).ceil().max(0.0).min(width as f64) as usize;
    let bottom=(draw.y+draw.height).ceil().max(0.0).min(height as f64) as usize;
    let sx=tile.width as f64/draw.width;let sy=tile.height as f64/draw.height;
    for y in top..bottom {
        for x in left..right {
            let source_x=(x as f64+0.5-draw.x)*sx-0.5;
            let source_y=(y as f64+0.5-draw.y)*sy-0.5;
            let ix=source_x.floor() as i64;let iy=source_y.floor() as i64;
            let fx=source_x-ix as f64;let fy=source_y-iy as f64;
            let mut rgba=[0.0;4];
            for (dy,wy) in [(0,1.0-fy),(1,fy)] {
                for (dx,wx) in [(0,1.0-fx),(1,fx)] {
                    let px=ix+dx;let py=iy+dy;
                    if px<0||py<0||px>=tile.width as i64||py>=tile.height as i64{continue;}
                    let offset=(py as usize*tile.width as usize+px as usize)*4;
                    let p=&tile.pixels[offset..offset+4];
                    let alpha=p[3] as f64/255.0*wx*wy*draw.opacity;
                    for c in 0..3 {rgba[c]+=p[c] as f64*alpha;}
                    rgba[3]+=alpha;
                }
            }
            let alpha=rgba[3];if alpha<=0.0{continue;}
            let offset=(y*width as usize+x)*4;let dst=&mut output[offset..offset+4];
            let retained=dst[3] as f64/255.0*(1.0-alpha);let total=alpha+retained;
            for c in 0..3 {dst[c]=((rgba[c]+dst[c] as f64*retained)/total).round().clamp(0.0,255.0) as u8;}
            dst[3]=(total*255.0).round().clamp(0.0,255.0) as u8;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn tile()->Raster {Raster{x:0,y:0,width:2,height:1,pixels:vec![255,0,0,255,0,255,0,0]}}
    fn draw()->Draw {Draw{tile:0,x:0.5,y:0.0,width:2.0,height:1.0,opacity:0.5}}
    #[test]
    fn fractional_sampling_uses_premultiplied_color_and_opacity() {
        let mut output=vec![0;3*4];composite(&draw(),&tile(),3,1,&mut output);
        assert_eq!(output,vec![255,0,0,64,255,0,0,64,0,0,0,0]);
        let mut opaque=vec![0,0,255,255,0,0,255,255,0,0,255,255];
        composite(&draw(),&tile(),3,1,&mut opaque);
        assert_eq!(opaque,vec![64,0,191,255,64,0,191,255,0,0,255,255]);
    }
    #[test]
    fn clips_negative_placements_without_wrapping_rows() {
        let mut d=draw();d.x=-1.0;d.opacity=1.0;
        let raster=Raster{x:0,y:0,width:2,height:1,pixels:vec![10,20,30,255,40,50,60,255]};
        let mut output=vec![0;2*2*4];composite(&d,&raster,2,2,&mut output);
        assert_eq!(&output[..4],&[40,50,60,255]);assert!(output[4..].iter().all(|v|*v==0));
    }
    #[test]
    fn rejects_invalid_transform_and_overlapping_spans() {
        let mut spans=vec![MotionSpan{start_frame:0,end_frame:2,draws:vec![draw()]}];
        assert!(validate(&spans,1,10,10,10).is_ok());
        spans[0].draws[0].opacity=f64::NAN;assert!(validate(&spans,1,10,10,10).is_err());
        spans[0].draws[0]=draw();spans.push(spans[0].clone());
        assert!(validate(&spans,1,10,10,10).is_err());
    }
}
