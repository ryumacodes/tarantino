use crate::video_processing::{gpu_compositor::input_layout, types::ExportSettings, visual_effects};

/// Stage dimensions relative to the unzoomed source plane.
#[derive(Clone, Copy, Debug)]
pub(crate) struct CameraViewport {
    width: f64,
    height: f64,
}

impl Default for CameraViewport {
    fn default() -> Self { Self { width: 1.0, height: 1.0 } }
}

impl CameraViewport {
    pub(crate) fn from_settings(settings: &ExportSettings, source: (u32, u32)) -> Self {
        // Display capture zooms within the source texture. Window capture moves
        // the whole source plane across the stage, revealing different extents.
        if settings.capture_mode.as_deref() != Some("window") { return Self::default(); }
        let (width, height) = visual_effects::get_output_dimensions(settings);
        let padding = settings.visual_settings.as_ref().and_then(|v| v.padding)
            .filter(|value| value.is_finite()).unwrap_or(0.0).clamp(0.0, 49.0) / 100.0;
        let content_width = width.saturating_sub(2 * (width as f64 * padding) as u32).max(2);
        let content_height = height.saturating_sub(2 * (height as f64 * padding) as u32).max(2);
        let (plane_width, plane_height, _, _) = input_layout(settings, Some(source.0), Some(source.1), content_width, content_height);
        Self { width: width as f64 / plane_width.max(1) as f64,
            height: height as f64 / plane_height.max(1) as f64 }
    }

    pub(super) fn activity_extent(self, zoom: f64) -> (f64, f64) {
        let zoom = if zoom.is_finite() { zoom.max(1.0) } else { 1.0 };
        (0.5 * (self.width / zoom).min(1.0), 0.5 * (self.height / zoom).min(1.0))
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
    fn padding_expands_activity_region_without_exceeding_source() {
        let settings: ExportSettings = serde_json::from_value(serde_json::json!({
            "capture_mode":"window", "resolution":{"width":1000, "height":1000},
            "visual_settings":{"padding":10}
        })).unwrap();
        assert_eq!(CameraViewport::from_settings(&settings,(1000,1000)).activity_extent(2.0), (0.3125,0.3125));
        assert_eq!(CameraViewport::from_settings(&settings,(1000,1000)).activity_extent(1.0), (0.5,0.5));
    }
}
