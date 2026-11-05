use crate::video_processing::{gpu_compositor::input_layout, types::ExportSettings, visual_effects};
use super::rectangle::ScreenRect;

/// Source plane and zoom bounds in a stage whose height is 1080 units.
#[derive(Clone, Copy, Debug)]
pub(crate) struct CameraViewport {
    pub(super) source: ScreenRect,
    pub(super) zoom_area: ScreenRect,
}

impl Default for CameraViewport {
    fn default() -> Self {
        let rect = ScreenRect { x: 0.0, y: 0.0, width: 1080.0, height: 1080.0 };
        Self { source: rect, zoom_area: rect }
    }
}

impl CameraViewport {
    pub(crate) fn from_settings(settings: &ExportSettings, source: (u32, u32)) -> Self {
        let (width, height) = visual_effects::get_output_dimensions(settings);
        let padding = settings.visual_settings.as_ref().and_then(|v| v.padding)
            .filter(|value| value.is_finite()).unwrap_or(0.0).clamp(0.0, 49.0) / 100.0;
        let padding_x = (width as f64 * padding) as u32;
        let padding_y = (height as f64 * padding) as u32;
        let content_width = width.saturating_sub(2 * padding_x).max(2);
        let content_height = height.saturating_sub(2 * padding_y).max(2);
        let (plane_width, plane_height, offset_x, offset_y) = input_layout(
            settings, Some(source.0), Some(source.1), content_width, content_height);
        let units = 1080.0 / height as f64;
        let source = ScreenRect {
            x: (padding_x + offset_x) as f64 * units,
            y: (padding_y + offset_y) as f64 * units,
            width: plane_width.max(1) as f64 * units,
            height: plane_height.max(1) as f64 * units,
        };
        let zoom_area = if settings.capture_mode.as_deref() == Some("window") {
            ScreenRect { x: padding_x as f64 * units, y: padding_y as f64 * units,
                width: content_width as f64 * units, height: content_height as f64 * units }
        } else {
            // Display capture samples a crop inside its fixed source plane.
            source
        };
        Self { source, zoom_area }
    }

    pub(super) fn activity_extent(self, zoom: f64) -> (f64, f64) {
        let zoom = if zoom.is_finite() { zoom.max(1.0) } else { 1.0 };
        (0.5 * (self.zoom_area.width / self.source.width / zoom).min(1.0),
         0.5 * (self.zoom_area.height / self.source.height / zoom).min(1.0))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn portrait_stage_keeps_more_vertical_source_visible() {
        let settings: ExportSettings = serde_json::from_value(serde_json::json!({
            "capture_mode":"window", "resolution":{"width":608, "height":1080}
        })).unwrap();
        let viewport = CameraViewport::from_settings(&settings, (1920,1080));
        assert_eq!(viewport.activity_extent(2.0), (0.25,0.5));
    }

    #[test]
    fn padding_stays_outside_the_zoom_activity_region() {
        let settings: ExportSettings = serde_json::from_value(serde_json::json!({
            "capture_mode":"window", "resolution":{"width":1000, "height":1000},
            "visual_settings":{"padding":10}
        })).unwrap();
        let viewport = CameraViewport::from_settings(&settings,(1000,1000));
        assert_eq!(viewport.source, ScreenRect { x:108.0,y:108.0,width:864.0,height:864.0 });
        assert_eq!(viewport.zoom_area, viewport.source);
        assert_eq!(viewport.activity_extent(2.0), (0.25,0.25));
        assert_eq!(CameraViewport::from_settings(&settings,(1000,1000)).activity_extent(1.0), (0.5,0.5));
    }

    #[test]
    fn stage_geometry_preserves_letterboxing_and_inset_bounds() {
        let settings: ExportSettings = serde_json::from_value(serde_json::json!({
            "capture_mode":"window", "resolution":{"width":608,"height":1080},
            "visual_settings":{"padding":10}
        })).unwrap();
        let viewport = CameraViewport::from_settings(&settings,(1920,1080));
        assert_eq!(viewport.zoom_area, ScreenRect { x:60.0,y:108.0,width:488.0,height:864.0 });
        assert_eq!(viewport.source, ScreenRect { x:60.0,y:403.0,width:488.0,height:274.0 });
        let zoomed = viewport.source.zoom_to(viewport.zoom_area,(0.8,0.9),2.0,None);
        assert_eq!(zoomed.y,266.0);
        assert_eq!(zoomed.height,548.0);
        assert_eq!(zoomed.x,-428.0);
    }
}
