//! Image-space measurements shared by recorded cursor renderers.
use image::RgbaImage;

#[derive(Clone, Copy, Debug, PartialEq, serde::Serialize)]
pub struct Bounds {
    pub x: u32,
    pub y: u32,
    pub width: u32,
    pub height: u32,
}

/// Measure the cursor body independently of transparent padding and faint shadows.
pub fn visible_bounds(image: &RgbaImage) -> Option<Bounds> {
    let mut total = 0.0;
    let mut count = 0;
    for pixel in image.pixels().filter(|pixel| pixel[3] > 0) {
        total += pixel[3] as f64 / 255.0;
        count += 1;
    }
    if count == 0 { return None; }
    let threshold = total / count as f64;
    let (mut left, mut top, mut right, mut bottom) = (image.width(), image.height(), 0, 0);
    for (x, y, pixel) in image.enumerate_pixels() {
        if pixel[3] > 0 && pixel[3] as f64 / 255.0 >= threshold {
            left = left.min(x); top = top.min(y); right = right.max(x); bottom = bottom.max(y);
        }
    }
    (left < image.width() && top < image.height()).then(|| Bounds {
        x: left, y: top, width: right - left + 1, height: bottom - top + 1,
    })
}

#[derive(Clone, Copy, Debug, serde::Serialize)]
pub struct Geometry {
    /// Full image size per unit of requested visible cursor height.
    pub size_per_height: [f64; 2],
    /// Native hotspot normalized to the full bitmap, including its padding.
    pub anchor: [f64; 2],
}

impl Geometry {
    pub fn new(image: &RgbaImage, logical_size: [f64; 2], hotspot: [f64; 2]) -> Option<Self> {
        if !logical_size.iter().chain(hotspot.iter()).all(|v| v.is_finite())
            || logical_size.iter().any(|v| *v <= 0.0) { return None; }
        let body = visible_bounds(image)?;
        Some(Self {
            size_per_height: [image.width() as f64 / body.height as f64,
                image.height() as f64 / body.height as f64],
            anchor: [hotspot[0] / logical_size[0], hotspot[1] / logical_size[1]],
        })
    }

    /// Unrotated image rectangle; rotation must use the supplied hotspot as pivot.
    pub fn rect(&self, hotspot: [f64; 2], visible_height: f64) -> [f64; 4] {
        let width = self.size_per_height[0] * visible_height;
        let height = self.size_per_height[1] * visible_height;
        [hotspot[0] - self.anchor[0] * width, hotspot[1] - self.anchor[1] * height, width, height]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn body_scale_ignores_padding_and_keeps_hotspot_stationary_when_pressed() {
        let mut image = RgbaImage::from_pixel(40,60,image::Rgba([0,0,0,8]));
        for y in 10..50 { for x in 12..20 { image.put_pixel(x,y,image::Rgba([255,255,255,255])); } }
        let geometry = Geometry::new(&image,[20.0,30.0],[7.0,6.0]).unwrap();
        assert_eq!(visible_bounds(&image),Some(Bounds{x:12,y:10,width:8,height:40}));
        for height in [17.0,15.3,34.0] {
            let rect = geometry.rect([123.0,77.0],height);
            assert!((rect[0]+rect[2]*geometry.anchor[0]-123.0).abs()<1e-12);
            assert!((rect[1]+rect[3]*geometry.anchor[1]-77.0).abs()<1e-12);
            assert!((rect[3]*40.0/60.0-height).abs()<1e-12);
        }
        assert!(Geometry::new(&RgbaImage::new(10,10),[10.0,10.0],[0.0,0.0]).is_none());
        assert!(Geometry::new(&image,[0.0,30.0],[0.0,0.0]).is_none());
    }

    #[test]
    #[ignore = "external cursor image geometry fixtures"]
    fn geometry_matches_external_samples() {
        let path=std::env::var("TARANTINO_CURSOR_IMAGE_GEOMETRY").unwrap();
        let cases:serde_json::Value=serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap();
        for case in cases.as_array().unwrap() {
            let (w,h)=(case["width"].as_u64().unwrap() as u32,case["height"].as_u64().unwrap() as u32);
            let bytes=case["rgba"].as_array().unwrap().iter().map(|v|v.as_u64().unwrap() as u8).collect();
            let image=RgbaImage::from_raw(w,h,bytes).unwrap();
            let bounds=visible_bounds(&image);
            assert_eq!(serde_json::to_value(bounds).unwrap(),case["bounds"]);
            if bounds.is_none() { continue; }
            let anchor:[f64;2]=serde_json::from_value(case["anchor"].clone()).unwrap();
            let geometry=Geometry::new(&image,[w as f64,h as f64],[anchor[0]*w as f64,anchor[1]*h as f64]).unwrap();
            for sample in case["samples"].as_array().unwrap() {
                let point=serde_json::from_value(sample["point"].clone()).unwrap();
                let actual=geometry.rect(point,sample["size"].as_f64().unwrap());
                let expected:[f64;4]=serde_json::from_value(sample["rect"].clone()).unwrap();
                for (a,b) in actual.into_iter().zip(expected) { assert!((a-b).abs()<1e-9,"{a} != {b}"); }
            }
        }
    }
}
