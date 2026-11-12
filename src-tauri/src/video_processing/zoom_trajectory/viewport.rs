use crate::video_processing::{gpu_compositor::input_layout, types::ExportSettings, visual_effects};
use super::rectangle::ScreenRect;

/// Source plane and zoom bounds in a stage whose height is 1080 units.
#[derive(Clone, Copy, Debug)]
pub(crate) struct CameraViewport {
    pub(super) source: ScreenRect,
    pub(super) zoom_area: ScreenRect,
    pub(super) stage_size: (f64,f64),
    pub(super) source_size: (f64,f64),
}

impl Default for CameraViewport {
    fn default() -> Self {
        let rect = ScreenRect { x: 0.0, y: 0.0, width: 1080.0, height: 1080.0 };
        Self { source: rect, zoom_area: rect, stage_size:(1080.0,1080.0),source_size:(1080.0,1080.0) }
    }
}

impl CameraViewport {
    pub(crate) fn from_source_settings(settings: &ExportSettings, source: (u32, u32)) -> anyhow::Result<Self> {
        let crop = settings.source_crop.map(|crop| crop.geometry(source.0, source.1)).transpose()?;
        let mut layout = settings.clone();
        if let Some(crop) = crop {
            crop.apply_window_dimensions(&mut layout);
        }
        Ok(Self::from_settings(&layout, crop.map(|crop| (crop.width, crop.height)).unwrap_or((source.0 as f64,source.1 as f64))))
    }

    pub(crate) fn from_settings<T: Into<f64>>(settings: &ExportSettings, source: (T, T)) -> Self {
        let source_size=(source.0.into(),source.1.into());
        let (width, height) = visual_effects::get_output_dimensions(settings);
        let padding_x = visual_effects::content_inset(width,height,
            settings.visual_settings.as_ref().and_then(|v|v.padding).unwrap_or(0.0));
        let padding_y = padding_x;
        let content_width = width.saturating_sub(2 * padding_x).max(2);
        let content_height = height.saturating_sub(2 * padding_y).max(2);
        let (plane_width, plane_height, offset_x, offset_y) = input_layout(
            settings, Some(source_size.0), Some(source_size.1), content_width, content_height);
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
        Self { source, zoom_area, source_size, stage_size:(width as f64*units,1080.0) }
    }

    pub(super) fn activity_extent(self, zoom: f64, logical_size: Option<(f64,f64)>) -> (f64, f64) {
        let zoom = if zoom.is_finite() { zoom.max(1.0) } else { 1.0 };
        let size=logical_size.unwrap_or(self.source_size);
        (0.5 * (self.stage_size.0 / size.0 / zoom).min(1.0),
         0.5 * (self.stage_size.1 / size.1 / zoom).min(1.0))
    }

    pub(super) fn source_activity_extent(self, zoom:f64, size:(f64,f64)) -> (f64,f64) {
        let zoom=if zoom.is_finite() {zoom.max(1.0)} else {1.0};
        ((self.stage_size.0/(size.0*zoom)).min(1.0)*size.0*0.5,
         (self.stage_size.1/(size.1*zoom)).min(1.0)*size.1*0.5)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn portrait_stage_groups_in_original_source_coordinates() {
        let settings: ExportSettings = serde_json::from_value(serde_json::json!({
            "capture_mode":"window", "resolution":{"width":608, "height":1080}
        })).unwrap();
        let viewport = CameraViewport::from_settings(&settings, (1920,1080));
        assert_eq!(viewport.activity_extent(2.0,None), (608.0/1920.0/4.0,0.25));
    }

    #[test]
    fn grouping_uses_full_stage_while_rendering_keeps_padding() {
        let settings: ExportSettings = serde_json::from_value(serde_json::json!({
            "capture_mode":"window", "resolution":{"width":1000, "height":1000},
            "visual_settings":{"padding":10}
        })).unwrap();
        let viewport = CameraViewport::from_settings(&settings,(1000,1000));
        assert_eq!(viewport.source, ScreenRect { x:83.0*1.08,y:83.0*1.08,width:834.0*1.08,height:834.0*1.08 });
        assert_eq!(viewport.zoom_area, viewport.source);
        assert_eq!(viewport.activity_extent(2.0,None), (0.27,0.27));
        assert_eq!(CameraViewport::from_settings(&settings,(1000,1000)).activity_extent(1.0,None), (0.5,0.5));
    }

    #[test]
    fn stage_geometry_preserves_letterboxing_and_inset_bounds() {
        let settings: ExportSettings = serde_json::from_value(serde_json::json!({
            "capture_mode":"window", "resolution":{"width":608,"height":1080},
            "visual_settings":{"padding":10}
        })).unwrap();
        let viewport = CameraViewport::from_settings(&settings,(1920,1080));
        assert_eq!(viewport.zoom_area, ScreenRect { x:50.0,y:50.0,width:508.0,height:980.0 });
        assert_eq!(viewport.source, ScreenRect { x:50.0,y:397.0,width:508.0,height:286.0 });
        let zoomed = viewport.source.zoom_to(viewport.zoom_area,(0.8,0.9),2.0,None);
        assert_eq!(zoomed.y,254.0);
        assert_eq!(zoomed.height,572.0);
        assert_eq!(zoomed.x,-458.0);
    }
}
