use super::*;

impl GpuCompositor {
    pub(super) fn cursor_shutter(&mut self, camera:&ZoomFrameState,
        cursor:Option<&crate::cursor_renderer::CursorFrameState>) -> ([f32;4],[f32;4]) {
        let Some(cursor)=cursor.filter(|c|self.cursor_enabled && c.opacity>0.01) else {
            self.prev_cursor_pose=None;
            return ([0.0;4],[0.0;4]);
        };
        let config=&self.config;
        let size=self.cursor_size*(1.0-cursor.press_amount);
        let now=if config.window_mode {
            let rect=motion::window_rect(camera,config);
            [rect[0]+(cursor.x-0.5)*rect[2],rect[1]+(cursor.y-0.5)*rect[3],
                size*camera.rect.map_or(camera.scale,|rect|rect[2]) as f32,cursor.rotation]
        } else {
            [(config.content_offset_x+config.input_offset_x) as f32+cursor.x*config.input_width as f32,
                (config.content_offset_y+config.input_offset_y) as f32+cursor.y*config.input_height as f32,
                size,cursor.rotation]
        };
        let previous=self.prev_cursor_pose.replace(now).unwrap_or(now);
        let mut start=now;
        if config.motion_blur_enabled {
            let pan=config.motion_blur_pan_intensity*config.frame_rate as f32/60.0;
            let zoom=config.motion_blur_zoom_intensity*config.frame_rate as f32/60.0;
            start[0]+=(previous[0]-now[0])*pan;
            start[1]+=(previous[1]-now[1])*pan;
            start[2]=(now[2]+(previous[2]-now[2])*zoom).max(0.001);
            start[3]+=((previous[3]-now[3]+180.0).rem_euclid(360.0)-180.0)*pan;
        }
        (start,now)
    }
}
