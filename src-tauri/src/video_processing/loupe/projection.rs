//! Convert original source lens geometry into the compositor's stage pixels.
use super::{content_pass::ContentLens, LoupeTarget, SourceCrop};
use crate::video_processing::zoom_trajectory::ZoomFrameState;
use anyhow::{bail, Result};

pub fn project(
    lens: LoupeTarget,
    source: (u32, u32),
    crop: Option<SourceCrop>,
    video_rect: [f64; 4],
    camera: &ZoomFrameState,
    window_mode: bool,
) -> Result<ContentLens> {
    let crop = crop
        .unwrap_or(SourceCrop {
            x: 0.0,
            y: 0.0,
            width: 1.0,
            height: 1.0,
        })
        .geometry(source.0, source.1)?
        .normalized();
    if !video_rect.iter().all(|value| value.is_finite())
        || video_rect[2] <= 0.0
        || video_rect[3] <= 0.0
        || ![camera.scale, camera.center_x, camera.center_y]
            .iter()
            .all(|value| value.is_finite())
        || camera.scale <= 0.0
    {
        bail!("Invalid magnifier stage projection");
    }
    let mut center = [
        (lens.center_x - crop.x) / crop.width,
        (lens.center_y - crop.y) / crop.height,
    ];
    let rect = camera.rect.filter(|_| window_mode);
    let scale = if let Some(rect) = rect {
        rect[2]
    } else if window_mode || camera.scale > 1.001 {
        camera.scale
    } else {
        1.0
    };
    for (axis, focus) in [camera.center_x, camera.center_y].into_iter().enumerate() {
        center[axis] = if let Some(rect) = rect {
            rect[axis] + center[axis] * rect[axis+2]
        } else if window_mode {
            let t = (scale - 1.0).clamp(0.0, 1.0);
            let anchor = focus + (focus.clamp(0.22, 0.78) - focus) * t * t * (3.0 - 2.0 * t);
            anchor + (center[axis] - focus) * scale
        } else {
            let visible = 1.0 / scale;
            let left = (focus - visible / 2.0).clamp(0.0, 1.0 - visible);
            (center[axis] - left) / visible
        };
    }
    let to_light = [(0.5-lens.center_x)*source.0 as f64,
        (-0.5-lens.center_y)*source.1 as f64];
    let light_distance = to_light[0].hypot(to_light[1]);
    let direction = if light_distance < 0.001 { [0.0,-1.0] }
        else { [to_light[0]/light_distance,to_light[1]/light_distance] };
    Ok(ContentLens {
        center: [
            (video_rect[0] + center[0] * video_rect[2]) as f32,
            (video_rect[1] + center[1] * video_rect[3]) as f32,
        ],
        radius: (lens.radius / crop.width * video_rect[2] * scale) as f32,
        bevel: (lens.radius / crop.width * video_rect[2] * scale * lens.bevel.clamp(0.0, 1.0)) as f32,
        zoom: lens.zoom as f32,
        presence: lens.presence as f32,
        aberration: lens.chromatic_aberration.clamp(0.0,1.0) as f32,
        reflection: 1.0,
        lighting: 1.0,
        shine_exponent: (light_distance/(0.1*source.0 as f64)).clamp(1.5,24.0) as f32,
        source_pixel_scale: (video_rect[2]*scale/(crop.width*source.0 as f64)) as f32,
        shadow_alpha: lens.shadow_alpha.clamp(0.0,1.0) as f32,
        shadow_reach: (0.9 / crop.width * video_rect[2] * scale) as f32,
        light_direction: [direction[0] as f32,direction[1] as f32],
        light_tilt: (light_distance/(0.8*source.0 as f64)).min(2.0) as f32,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn explicit_rectangle_places_lens_using_both_dimensions() {
        let camera=ZoomFrameState {scale:3.0,center_x:0.9,center_y:0.9,
            rect:Some([0.125,0.25,0.5,0.25])};
        let projected=project(lens(),(128,128),None,[0.0,0.0,128.0,128.0],&camera,true).unwrap();
        assert_eq!(projected.center,[64.0,48.0]);
        assert_eq!(projected.radius,8.0);
    }
    fn lens() -> LoupeTarget {
        LoupeTarget {
            center_x: 0.75,
            center_y: 0.5,
            radius: 0.125,
            zoom: 2.0,
            presence: 1.0,
            bevel: 0.0,
            chromatic_aberration: 0.0,
            shadow_alpha: 0.0,
        }
    }
    #[test]
    fn cropped_source_maps_to_padded_stage_then_camera_transform() {
        let crop = Some(SourceCrop {
            x: 0.5,
            y: 0.0,
            width: 0.5,
            height: 1.0,
        });
        let rect = [20.0, 40.0, 200.0, 200.0];
        let identity = ZoomFrameState { rect: None,
            scale: 1.0,
            center_x: 0.5,
            center_y: 0.5,
        };
        let base = project(lens(), (1000, 500), crop, rect, &identity, false).unwrap();
        assert_eq!(base.center, [120.0, 140.0]);
        assert_eq!(base.radius, 50.0);
        assert!((base.source_pixel_scale-0.4).abs()<0.0001);
        assert!((base.shadow_reach-360.0).abs()<0.001);
        assert!((base.light_direction[0]+0.4472136).abs()<0.0001);
        assert!((base.light_direction[1]+0.8944272).abs()<0.0001);
        assert!((base.light_tilt-0.69877124).abs()<0.0001);
        let zoom = ZoomFrameState { rect: None,
            scale: 2.0,
            center_x: 0.75,
            center_y: 0.25,
        };
        let display = project(lens(), (1000, 500), crop, rect, &zoom, false).unwrap();
        assert_eq!(display.center, [20.0, 240.0]);
        assert_eq!(display.radius, 100.0);
        assert!((display.source_pixel_scale-0.8).abs()<0.0001);
        assert_eq!(display.light_direction,base.light_direction);
        assert_eq!(display.light_tilt,base.light_tilt);
        assert!((display.shadow_reach-720.0).abs()<0.001);
        let window = project(lens(), (1000, 500), crop, rect, &zoom, true).unwrap();
        assert_eq!(window.center, [70.0, 190.0]);
        assert_eq!(window.radius, 100.0);
        let glass = LoupeTarget { bevel: 0.35, ..lens() };
        let projected_glass = project(glass, (1000, 500), crop, rect, &zoom, false).unwrap();
        assert!((projected_glass.bevel - 35.0).abs() < 0.0001);
        let oversized = project(LoupeTarget { bevel: 2.0, ..glass }, (1000, 500), crop, rect, &identity, false).unwrap();
        assert_eq!(oversized.bevel, oversized.radius);
    }
    #[test]
    fn window_edge_anchor_matches_compositor_and_keeps_outside_centers_outside() {
        let zoom = ZoomFrameState { rect: None,
            scale: 2.0,
            center_x: 0.0,
            center_y: 1.0,
        };
        let target = LoupeTarget {
            center_x: 0.0,
            center_y: 1.0,
            ..lens()
        };
        let window = project(
            target,
            (100, 100),
            None,
            [0.0, 0.0, 100.0, 100.0],
            &zoom,
            true,
        )
        .unwrap();
        assert_eq!(window.center, [22.0, 78.0]);
        let target = LoupeTarget {
            center_x: -0.1,
            ..target
        };
        let display = project(
            target,
            (100, 100),
            None,
            [0.0, 0.0, 100.0, 100.0],
            &zoom,
            false,
        )
        .unwrap();
        assert_eq!(display.center[0], -20.0);
    }
}
