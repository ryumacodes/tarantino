use super::{image_geometry::Geometry, recorded_images::Image};
use image::{Rgba, RgbaImage};
use std::sync::OnceLock;

const OUTLINE: &[[f64; 2]] = &[
    [8.0,28.0],[6.0,23.0],[2.0,17.0],[2.0,15.0],[3.5,14.0],
    [7.0,17.0],[7.0,4.0],[8.0,2.0],[10.0,2.0],[11.0,4.0],
    [11.0,11.0],[12.0,10.0],[14.0,10.0],[15.0,12.0],
    [17.0,11.0],[19.0,12.0],[20.0,14.0],[22.0,14.0],
    [23.0,16.0],[23.0,22.0],[21.0,28.0],
];

pub fn image(outline: bool) -> Image {
    static FILLED: OnceLock<Image> = OnceLock::new();
    static HOLLOW: OnceLock<Image> = OnceLock::new();
    (if outline { &HOLLOW } else { &FILLED }).get_or_init(|| rasterize(outline)).clone()
}

fn distance(point: [f64;2], a: [f64;2], b: [f64;2]) -> f64 {
    let delta=[b[0]-a[0],b[1]-a[1]];
    let t=(((point[0]-a[0])*delta[0]+(point[1]-a[1])*delta[1])
        /(delta[0]*delta[0]+delta[1]*delta[1])).clamp(0.0,1.0);
    (point[0]-a[0]-t*delta[0]).hypot(point[1]-a[1]-t*delta[1])
}

fn rasterize(outline: bool) -> Image {
    const SCALE: f64 = 8.0;
    let pixels=RgbaImage::from_fn(200,248,|x,y| {
        let point=[(x as f64+0.5)/SCALE,(y as f64+0.5)/SCALE];
        let mut inside=false;
        let mut edge=f64::INFINITY;
        for (a,b) in OUTLINE.iter().zip(OUTLINE.iter().cycle().skip(1)) {
            edge=edge.min(distance(point,*a,*b));
            if (a[1]>point[1]) != (b[1]>point[1])
                && point[0] < (b[0]-a[0])*(point[1]-a[1])/(b[1]-a[1])+a[0] { inside=!inside; }
        }
        let stroke=((0.7-edge)*SCALE+0.5).clamp(0.0,1.0);
        let alpha=if inside && !outline {1.0} else {stroke};
        let white=if alpha>0.0 {stroke/alpha} else {0.0};
        Rgba([(white*255.0).round() as u8,(white*255.0).round() as u8,
            (white*255.0).round() as u8,(alpha*255.0).round() as u8])
    });
    let geometry=Geometry::new(&pixels,[25.0,31.0],[9.0,2.0]).unwrap();
    Image {pixels,geometry}
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn hand_styles_share_hotspot_and_keep_distinct_interiors() {
        let filled=image(false);
        let outline=image(true);
        assert_eq!(filled.geometry.anchor,outline.geometry.anchor);
        assert_eq!(filled.pixels.get_pixel(120,176),&Rgba([0,0,0,255]));
        assert_eq!(outline.pixels.get_pixel(120,176)[3],0);
        assert!(outline.pixels.pixels().any(|p|p[3]>200));
        if let Ok(folder)=std::env::var("TARANTINO_HAND_PREVIEW") {
            filled.pixels.save(std::path::Path::new(&folder).join("filled-hand.png")).unwrap();
            outline.pixels.save(std::path::Path::new(&folder).join("outline-hand.png")).unwrap();
        }
    }
}
