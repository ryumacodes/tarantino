//! Validated keycap atlases and timelines for GPU or legacy CPU composition.
use anyhow::{Result,ensure};
use base64::{Engine as _,engine::general_purpose::STANDARD};
use serde::{Deserialize,Serialize};
use super::shortcut_motion::{self,MotionSpan};

#[derive(Debug,Clone,Serialize,Deserialize)]
pub struct ShortcutOverlay {
    pub width:u32,pub height:u32,pub frames:u64,
    pub tiles:Vec<Tile>,pub spans:Vec<Span>,
    #[serde(default)]
    pub motion:Vec<MotionSpan>,
}
#[derive(Debug,Clone,Serialize,Deserialize)]
pub struct Tile {pub x:u32,pub y:u32,pub png:String}
#[derive(Debug,Clone,Serialize,Deserialize)]
pub struct Span {pub start_frame:u64,pub end_frame:u64,pub tile:usize}
pub(super) struct Raster {pub x:u32,pub y:u32,pub width:u32,pub height:u32,pub pixels:Vec<u8>}
pub struct Prepared {width:u32,height:u32,tiles:Vec<Raster>,spans:Vec<Span>,motion:Vec<MotionSpan>}
impl Prepared {
    pub(super) fn animated(&self)->bool {!self.motion.is_empty()}
    pub(super) fn rasters(&self)->&[Raster] {&self.tiles}
    pub(super) fn draws(&self,frame:u64)->&[shortcut_motion::Draw] {
        let index=self.motion.partition_point(|span|span.start_frame<=frame);
        index.checked_sub(1).and_then(|i|self.motion.get(i)).filter(|s|frame<s.end_frame)
            .map_or(&[],|span|span.draws.as_slice())
    }
    pub fn new(input:Option<&ShortcutOverlay>,width:u32,height:u32,frames:u64)->Result<Option<Self>> {
        let Some(input)=input else{return Ok(None)};
        ensure!(input.width==width&&input.height==height&&input.frames==frames,"Shortcut overlay does not match export dimensions or duration");
        ensure!(input.motion.is_empty()||input.spans.is_empty(),"Conflicting shortcut animation plans");
        shortcut_motion::validate(&input.motion,input.tiles.len(),frames,width,height)?;
        let mut previous=0;
        for span in &input.spans {
            ensure!(span.start_frame>=previous&&span.start_frame<span.end_frame&&span.end_frame<=frames
                &&span.tile<input.tiles.len(),"Invalid shortcut display range");
            previous=span.end_frame;
        }
        const LIMIT:usize=128*1024*1024;
        let mut allocated=0usize;
        let mut tiles=Vec::with_capacity(input.tiles.len());
        for tile in &input.tiles {
            ensure!(tile.png.len()<=LIMIT,"Shortcut image is too large");
            let png=STANDARD.decode(&tile.png)?;
            let reader=image::ImageReader::with_format(std::io::Cursor::new(&png),image::ImageFormat::Png);
            let (w,h)=reader.into_dimensions()?;
            ensure!(w>0&&h>0&&tile.x.checked_add(w).is_some_and(|v|v<=width)
                &&tile.y.checked_add(h).is_some_and(|v|v<=height),"Shortcut tile exceeds output bounds");
            let bytes=(w as usize).checked_mul(h as usize).and_then(|v|v.checked_mul(4));
            allocated=allocated.checked_add(bytes.ok_or_else(||anyhow::anyhow!("Shortcut image size overflow"))?)
                .ok_or_else(||anyhow::anyhow!("Shortcut image size overflow"))?;
            ensure!(allocated<=LIMIT,"Shortcut images exceed the export memory limit");
            let pixels=image::load_from_memory_with_format(&png,image::ImageFormat::Png)?.into_rgba8().into_raw();
            tiles.push(Raster{x:tile.x,y:tile.y,width:w,height:h,pixels});
        }
        Ok(Some(Self{width,height,tiles,spans:input.spans.clone(),motion:input.motion.clone()}))
    }
    pub fn composite(&self,frame:u64,output:&mut[u8])->Result<()> {
        ensure!(output.len()==self.width as usize*self.height as usize*4,"Invalid shortcut destination buffer");
        if !self.motion.is_empty() {
            let index=self.motion.partition_point(|span|span.start_frame<=frame);
            if let Some(span)=index.checked_sub(1).and_then(|i|self.motion.get(i)).filter(|s|frame<s.end_frame) {
                for draw in &span.draws {shortcut_motion::composite(draw,&self.tiles[draw.tile],self.width,self.height,output);}
            }
            return Ok(());
        }
        let index=self.spans.partition_point(|span|span.start_frame<=frame);
        let Some(span)=index.checked_sub(1).and_then(|i|self.spans.get(i)).filter(|s|frame<s.end_frame) else{return Ok(())};
        let tile=&self.tiles[span.tile];
        for row in 0..tile.height as usize {
            let start=((tile.y as usize+row)*self.width as usize+tile.x as usize)*4;
            let source=&tile.pixels[row*tile.width as usize*4..(row+1)*tile.width as usize*4];
            for (dst,src) in output[start..start+source.len()].chunks_exact_mut(4).zip(source.chunks_exact(4)) {
                let alpha=src[3] as u32;
                if alpha==0{continue;}
                if alpha==255{dst.copy_from_slice(src);continue;}
                if dst[3]==255 {
                    for channel in 0..3 {dst[channel]=((src[channel] as u32*alpha+dst[channel] as u32*(255-alpha)+127)/255) as u8;}
                } else {
                    let denominator=alpha*255+dst[3] as u32*(255-alpha);
                    for channel in 0..3 {dst[channel]=((src[channel] as u32*alpha*255
                        +dst[channel] as u32*dst[3] as u32*(255-alpha)+denominator/2)/denominator) as u8;}
                    dst[3]=((denominator+127)/255) as u8;
                }
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn fixture()->ShortcutOverlay {
        let image=image::RgbaImage::from_raw(2,1,vec![240,180,120,128,10,20,30,255]).unwrap();
        let mut png=std::io::Cursor::new(Vec::new());
        image.write_to(&mut png,image::ImageFormat::Png).unwrap();
        ShortcutOverlay{width:4,height:2,frames:10,tiles:vec![Tile{x:1,y:1,png:STANDARD.encode(png.into_inner())}],
            motion:vec![],spans:vec![Span{start_frame:2,end_frame:4,tile:0},Span{start_frame:7,end_frame:9,tile:0}]}
    }
    #[test]
    fn composites_only_tile_pixels_at_half_open_ranges_including_repeated_clips() {
        let overlay=Prepared::new(Some(&fixture()),4,2,10).unwrap().unwrap();
        for frame in 0..10 {
            let mut image=vec![100u8;4*2*4];for p in image.chunks_exact_mut(4){p[3]=255;}
            let original=image.clone();overlay.composite(frame,&mut image).unwrap();
            if (2..4).contains(&frame)||(7..9).contains(&frame) {
                assert_eq!(&image[20..24],&[170,140,110,255]);
                assert_eq!(&image[24..28],&[10,20,30,255]);
                assert_eq!(&image[..20],&original[..20]);assert_eq!(&image[28..],&original[28..]);
            } else {assert_eq!(image,original);}
        }
        let mut transparent=vec![0;4*2*4];overlay.composite(2,&mut transparent).unwrap();
        assert_eq!(&transparent[20..24],&[240,180,120,128]);
    }
    #[test]
    fn dispatches_animated_ranges_and_preserves_static_payload_compatibility() {
        let mut input=fixture();
        let serialized=serde_json::to_value(&input).unwrap();
        let mut legacy=serialized.clone();legacy.as_object_mut().unwrap().remove("motion");
        assert!(serde_json::from_value::<ShortcutOverlay>(legacy).unwrap().motion.is_empty());
        input.motion=vec![MotionSpan{start_frame:3,end_frame:5,draws:vec![super::shortcut_motion::Draw{
            tile:0,x:1.0,y:1.0,width:2.0,height:1.0,opacity:0.5}]}];
        assert!(Prepared::new(Some(&input),4,2,10).is_err());
        input.spans.clear();
        let overlay=Prepared::new(Some(&input),4,2,10).unwrap().unwrap();
        for frame in [2,3,4,5] {
            let mut image=vec![0;32];overlay.composite(frame,&mut image).unwrap();
            if frame==3||frame==4 {
                assert_eq!(&image[20..24],&[240,180,120,64]);
                assert_eq!(&image[24..28],&[10,20,30,128]);
            } else {assert!(image.iter().all(|v|*v==0));}
        }
    }
    #[test]
    fn rejects_mismatched_or_invalid_plans_before_encoding() {
        let input=fixture();
        assert!(Prepared::new(Some(&input),4,2,11).is_err());
        let mut bad=input.clone();bad.tiles[0].x=4;
        assert!(Prepared::new(Some(&bad),4,2,10).is_err());
        let mut bad=input.clone();bad.spans[1].start_frame=3;
        assert!(Prepared::new(Some(&bad),4,2,10).is_err());
        let mut bad=input;bad.spans[0].tile=1;
        assert!(Prepared::new(Some(&bad),4,2,10).is_err());
    }
}
