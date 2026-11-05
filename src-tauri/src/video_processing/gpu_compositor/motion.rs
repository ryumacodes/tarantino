use super::ZoomFrameState;

// Normalized camera geometry, matching the shader's window anchors/display clamps.
fn pose(camera: &ZoomFrameState, window: bool) -> (f64, f64, f64) {
    let scale = camera.scale;
    if window {
        if let Some([x,y,width,height]) = camera.rect {
            return (width,x+width/2.0-0.5,0.5-y-height/2.0);
        }
        let t = (scale - 1.0).clamp(0.0, 1.0);
        let strength = t*t*(3.0-2.0*t);
        let offset = |center: f64| {
            let anchor = center + (center.clamp(0.22,0.78)-center)*strength;
            (anchor-0.5)-(center-0.5)*scale
        };
        return (scale, offset(camera.center_x), -offset(camera.center_y));
    }
    if scale <= 1.001 { return (1.0,0.0,0.0); }
    let width = 1.0/scale;
    let offset = |center: f64| {
        let start = (center-width/2.0).clamp(0.0,1.0-width);
        (0.5-start-width/2.0)/width
    };
    (scale,offset(camera.center_x),-offset(camera.center_y))
}

pub(super) fn velocity(now: &ZoomFrameState, previous: Option<&ZoomFrameState>,
    window: bool, fps: f64) -> (f32,f32,f32) {
    let Some(previous) = previous else { return (0.0,0.0,0.0); };
    let (scale,x,y) = pose(now,window);
    let (old_scale,old_x,old_y) = pose(previous,window);
    let vx = -(x-old_x)*fps;
    let vy = (y-old_y)*fps;
    let zoom = (scale-old_scale)/scale.max(0.1)*fps;
    if vx.hypot(vy) <= 0.15 { return (0.0,0.0,zoom as f32); }
    ((vx*10.0) as f32,(vy*10.0) as f32,zoom as f32)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn camera(scale:f64,x:f64,y:f64)->ZoomFrameState {ZoomFrameState { rect: None,scale,center_x:x,center_y:y}}
    #[test]
    fn export_velocity_is_frame_rate_independent_and_resets() {
        let now=camera(2.0,0.5,0.5);
        for fps in [24.0,30.0,60.0,120.0] {
            let previous=camera(2.0-2.0/fps,0.5,0.5);
            assert_eq!(velocity(&now,Some(&previous),true,fps),(0.0,0.0,1.0));
            assert_eq!(velocity(&now,None,true,fps),(0.0,0.0,0.0));
        }
    }
    #[test]
    fn clamped_display_has_no_false_pan() {
        assert_eq!(velocity(&camera(2.0,0.1,0.5),Some(&camera(2.0,0.05,0.5)),false,60.0),(0.0,0.0,0.0));
    }
    #[test]
    fn window_anchor_motion_matches_preview_geometry() {
        let value=velocity(&camera(1.51,0.1,0.8),Some(&camera(1.5,0.1,0.8)),true,120.0);
        assert!((value.0-(-6.959712)).abs()<0.0001);
        assert!((value.1-3.959952).abs()<0.0001);
        assert!((value.2-(0.01/1.51*120.0)).abs()<0.0001);
    }
}

pub(super) fn window_rect(camera:&ZoomFrameState, config:&super::GpuCompositorConfig)->[f32;4] {
    if let Some([x,y,width,height]) = camera.rect {
        return [
            (config.content_offset_x+config.input_offset_x) as f32
                + ((x+width/2.0)*config.input_width as f64) as f32,
            (config.content_offset_y+config.input_offset_y) as f32
                + ((y+height/2.0)*config.input_height as f64) as f32,
            (width*config.input_width as f64) as f32,
            (height*config.input_height as f64) as f32,
        ];
    }
    let (scale,x,y)=pose(camera,true);
    let w=config.input_width as f64;let h=config.input_height as f64;
    [(config.content_offset_x+config.input_offset_x) as f32+((0.5+x)*w) as f32,
     (config.content_offset_y+config.input_offset_y) as f32+((0.5-y)*h) as f32,
     (scale*w) as f32,(scale*h) as f32]
}

pub(super) fn shutter_rect(now:&ZoomFrameState,previous:Option<&ZoomFrameState>,
    config:&super::GpuCompositorConfig)->([f32;4],[f32;4]) {
    let current=window_rect(now,config);
    let Some(previous)=previous.filter(|_|config.window_mode && config.motion_blur_enabled) else {
        return (current,current);
    };
    let old=window_rect(previous,config);
    let mut start=current;
    for i in 0..4 {
        let strength=if i<2 {config.motion_blur_pan_intensity} else {config.motion_blur_zoom_intensity};
        start[i]+=(old[i]-current[i])*strength*(config.frame_rate as f32/60.0);
    }
    start[2]=start[2].max(0.001);start[3]=start[3].max(0.001);
    (start,current)
}
