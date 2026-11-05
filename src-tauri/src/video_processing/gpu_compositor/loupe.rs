use super::*;
use crate::video_processing::{
    loupe::{projection, LoupeTarget},
    source_crop::SourceCrop,
};

impl GpuCompositor {
    /// Set per-frame content geometry. Clearing it bypasses the pass completely,
    /// including after timeline gaps or a previously visible magnifier.
    pub fn set_loupe_target(
        &mut self,
        target: Option<LoupeTarget>,
        source: (u32, u32),
        crop: Option<SourceCrop>,
        camera: &ZoomFrameState,
    ) -> Result<()> {
        self.loupe_content = None;
        if let Some(target) = target.filter(|target| target.presence > 0.0 && target.radius > 0.0) {
            let config = &self.config;
            self.loupe_content = Some(projection::project(
                target,
                source,
                crop,
                [
                    (config.content_offset_x + config.input_offset_x) as f64,
                    (config.content_offset_y + config.input_offset_y) as f64,
                    config.input_width as f64,
                    config.input_height as f64,
                ],
                camera,
                config.window_mode,
            )?);
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cursor_renderer::{simulate_cursor_frames, CursorEvent, SpringConfig};

    #[test]
    #[ignore = "requires native GPU; magnifier integrated with cursor composition"]
    fn composed_cursor_is_magnified_and_clearing_the_lens_restores_base_frame() {
        let settings: crate::video_processing::ExportSettings = serde_json::from_value(serde_json::json!({
            "resolution":{"width":128,"height":128},
            "cursor_settings":{"enabled":true,"size":1,"color":"#ffffff","click_effect":"none","shadow_intensity":0},
            "visual_settings":{"padding":0,"corner_radius":0,"shadow_enabled":false,"background_color":"#000000"}
        })).unwrap();
        let mut config = build_gpu_config_with_webcam(&settings, &None, Some(128), Some(128));
        config.motion_blur_enabled = false;
        let shape = RgbaImage::from_pixel(1, 1, image::Rgba([255, 255, 255, 255]));
        let mut compositor = GpuCompositor::new(
            config,
            Some(&shape),
            1.0,
            [1.0, 1.0, 1.0],
            settings.cursor_settings.as_ref(),
        )
        .unwrap();
        let cursor = simulate_cursor_frames(
            &[CursorEvent { is_release: false,
                timestamp_ms: 0,
                x: 0.5,
                y: 0.5,
                is_click: false,
            }],
            &SpringConfig {
                tension: 170.0,
                friction: 30.0,
                mass: 0.0,
            },
            30.0,
            100,
            &None,
            settings.cursor_settings.as_ref().unwrap(),
            Clone::clone,
        );
        let source: Vec<u8> = (0..128 * 128).flat_map(|_| [0, 0, 0, 255]).collect();
        let camera = ZoomFrameState { rect: None,
            scale: 1.0,
            center_x: 0.5,
            center_y: 0.5,
        };
        let baseline = compositor
            .composite_frame(&source, &camera, Some(&cursor[0]))
            .unwrap();
        let lens = LoupeTarget {
            center_x: 0.5,
            center_y: 0.5,
            radius: 0.3125,
            zoom: 2.0,
            presence: 1.0,
            bevel: 0.0,
            chromatic_aberration: 0.0,
            shadow_alpha: 0.0,
        };
        compositor
            .set_loupe_target(Some(lens), (128, 128), None, &camera)
            .unwrap();
        let magnified = compositor
            .composite_frame(&source, &camera, Some(&cursor[0]))
            .unwrap();
        let bright = |pixels: &[u8]| {
            pixels
                .chunks_exact(4)
                .filter(|pixel| pixel[0] > 180)
                .count()
        };
        assert!(
            bright(&baseline) > 20,
            "fixture must contain an actual rendered cursor"
        );
        assert!(
            bright(&magnified) > bright(&baseline) * 2,
            "pointer should grow inside lens: base={}, magnified={}, cursor={:?}", bright(&baseline), bright(&magnified), cursor[0]
        );
        for (index, (actual, base)) in magnified
            .chunks_exact(4)
            .zip(baseline.chunks_exact(4))
            .enumerate()
        {
            let x = (index % 128) as f32 + 0.5;
            let y = (index / 128) as f32 + 0.5;
            if (x - 64.0).hypot(y - 64.0) > 40.5 {
                assert_eq!(actual, base);
            }
        }
        compositor
            .set_loupe_target(None, (128, 128), None, &camera)
            .unwrap();
        let restored = compositor
            .composite_frame(&source, &camera, Some(&cursor[0]))
            .unwrap();
        assert_eq!(
            restored, baseline,
            "cleared lens must not leave stale pixels"
        );
        compositor
            .set_loupe_target(Some(lens), (128, 128), None, &camera)
            .unwrap();
        assert_eq!(
            compositor
                .composite_frame(&source, &camera, Some(&cursor[0]))
                .unwrap(),
            magnified,
            "cached pass must be reusable without accumulated magnification"
        );
    }
}
