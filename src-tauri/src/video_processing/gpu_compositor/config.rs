use super::*;
use crate::video_processing::{types, visual_effects};
use base64::Engine;

pub fn build_gpu_config(
    settings: &types::ExportSettings,
    source_width: Option<u32>,
    source_height: Option<u32>,
) -> GpuCompositorConfig {
    let (out_w, out_h) = visual_effects::get_output_dimensions(settings);

    let visual = settings.visual_settings.as_ref();

    let padding_pct = visual.and_then(|v| v.padding).unwrap_or(0.0) / 100.0;
    let padding_x = (out_w as f64 * padding_pct) as u32;
    let padding_y = (out_h as f64 * padding_pct) as u32;
    let content_w = out_w - 2 * padding_x;
    let content_h = out_h - 2 * padding_y;

    let bg_hex = visual
        .and_then(resolve_background_color)
        .unwrap_or("#282a36");
    let background_color = parse_hex_color(bg_hex);
    let background_gradient = build_gradient_config(visual);
    let background_image = visual.and_then(decode_custom_background_image);

    let corner_radius_pct = visual.and_then(|v| v.corner_radius).unwrap_or(0.0);
    let min_dim = content_w.min(content_h) as f32;
    let corner_radius = if corner_radius_pct > 0.0 {
        (min_dim * corner_radius_pct as f32 / 100.0 * 0.5).max(1.0)
    } else {
        0.0
    };

    let shadow_enabled = visual.and_then(|v| v.shadow_enabled).unwrap_or(false);
    let shadow_blur = visual.and_then(|v| v.shadow_blur).unwrap_or(20.0) as f32;
    let shadow_intensity = visual.and_then(|v| v.shadow_intensity).unwrap_or(0.5) as f32;
    let shadow_offset_x = visual.and_then(|v| v.shadow_offset_x).unwrap_or(0.0) as f32;
    let shadow_offset_y = visual.and_then(|v| v.shadow_offset_y).unwrap_or(10.0) as f32;

    let motion_blur_enabled = visual.and_then(|v| v.motion_blur_enabled).unwrap_or(false);
    let motion_blur_pan_intensity = visual
        .and_then(|v| v.motion_blur_pan_intensity)
        .unwrap_or(0.2) as f32;
    let motion_blur_zoom_intensity = visual
        .and_then(|v| v.motion_blur_zoom_intensity)
        .unwrap_or(0.0) as f32;

    // Webcam config
    let webcam = settings.visual_settings.as_ref().and_then(|_| {
        // Webcam info is passed separately — this is just the config shape.
        // Actual webcam data is set via set_webcam_texture + upload_webcam_frame.
        None::<WebcamConfig>
    });

    // Device frame config
    let device_frame = visual
        .and_then(|v| v.device_frame.as_deref())
        .and_then(|df| {
            if df == "none" {
                return None;
            }
            let frame_color = visual
                .and_then(|v| v.device_frame_color.as_deref())
                .unwrap_or("black");
            let color = match frame_color {
                "silver" => parse_hex_color("#c4c4c4"),
                "gold" => parse_hex_color("#d4af37"),
                "blue" => parse_hex_color("#2563eb"),
                _ => parse_hex_color("#1a1a1a"),
            };
            let (bezel, corner_radius) = match df {
                "iphone-15-pro" | "iphone-15" => (20, 55),
                "ipad-pro" => (30, 25),
                "macbook-pro" => (15, 10),
                "browser" => (40, 10),
                _ => (20, 20),
            };
            Some(DeviceFrameConfig {
                bezel,
                corner_radius,
                color,
            })
        });

    // Compute input dimensions
    // Display mode: aspect-fit source to fill content area (existing behavior).
    // Window Focus uses the original source aspect; Window Desktop stages that
    // source inside the selected canvas ratio.
    let is_window_mode = settings.capture_mode.as_deref() == Some("window");
    let is_focus_window = visual
        .and_then(|v| v.window_layout_mode.as_deref())
        .unwrap_or("focus")
        == "focus";
    let (input_w, input_h, input_off_x, input_off_y) =
        if let (Some(sw), Some(sh)) = (source_width, source_height) {
            if sw > 0 && sh > 0 {
                if is_window_mode && is_focus_window {
                    let src_aspect = sw as f64 / sh as f64;
                    let cnt_aspect = content_w as f64 / content_h as f64;
                    let (mut iw, mut ih) = if (src_aspect - cnt_aspect).abs() < 0.01 {
                        (content_w, content_h)
                    } else if src_aspect > cnt_aspect {
                        (content_w, (content_w as f64 / src_aspect).round() as u32)
                    } else {
                        ((content_h as f64 * src_aspect).round() as u32, content_h)
                    };
                    iw = iw - (iw % 2);
                    ih = ih - (ih % 2);
                    let ox = (content_w - iw) / 2;
                    let oy = (content_h - ih) / 2;
                    (iw, ih, ox, oy)
                } else {
                    // Display mode (or window mode with missing screen dims): aspect-fit to fill content
                    let src_aspect = sw as f64 / sh as f64;
                    let cnt_aspect = content_w as f64 / content_h as f64;
                    let (mut iw, mut ih) = if (src_aspect - cnt_aspect).abs() < 0.01 {
                        // Same aspect ratio — input fills content
                        (content_w, content_h)
                    } else if src_aspect > cnt_aspect {
                        // Source wider — fit to content width, letterbox vertically
                        let h = (content_w as f64 / src_aspect).round() as u32;
                        (content_w, h)
                    } else {
                        // Source taller — fit to content height, pillarbox horizontally
                        let w = (content_h as f64 * src_aspect).round() as u32;
                        (w, content_h)
                    };
                    // Ensure even dims
                    iw = iw - (iw % 2);
                    ih = ih - (ih % 2);
                    // Clamp to content
                    iw = iw.min(content_w);
                    ih = ih.min(content_h);
                    let ox = (content_w - iw) / 2;
                    let oy = (content_h - ih) / 2;
                    (iw, ih, ox, oy)
                }
            } else {
                (content_w, content_h, 0, 0)
            }
        } else {
            // No source dims — fallback: input == content
            (content_w, content_h, 0, 0)
        };

    println!(
        "[GPU Config] output={}x{}, content={}x{}, input={}x{} offset=({},{})",
        out_w, out_h, content_w, content_h, input_w, input_h, input_off_x, input_off_y
    );

    GpuCompositorConfig {
        output_width: out_w,
        output_height: out_h,
        content_width: content_w,
        content_height: content_h,
        content_offset_x: padding_x,
        content_offset_y: padding_y,
        input_width: input_w,
        input_height: input_h,
        input_offset_x: input_off_x,
        input_offset_y: input_off_y,
        background_color,
        background_gradient,
        background_image,
        corner_radius,
        shadow_enabled,
        shadow_blur,
        shadow_intensity,
        shadow_offset_x,
        shadow_offset_y,
        webcam: webcam,
        device_frame,
        motion_blur_enabled,
        motion_blur_pan_intensity,
        motion_blur_zoom_intensity,
        window_mode: settings.capture_mode.as_deref() == Some("window"),
    }
}

fn decode_custom_background_image(visual: &types::VisualSettings) -> Option<RgbaImage> {
    if visual.background_type.as_deref() != Some("wallpaper") {
        println!(
            "[Wallpaper Image] export skipped: background_type={:?}",
            visual.background_type
        );
        return None;
    }
    if visual.wallpaper_id.is_some() {
        println!(
            "[Wallpaper Image] export skipped: preset wallpaper_id={:?}",
            visual.wallpaper_id
        );
        return None;
    }

    let image_src = match visual.custom_background_image.as_deref() {
        Some(src) => src,
        None => {
            println!("[Wallpaper Image] export skipped: no custom_background_image");
            return None;
        }
    };
    println!(
        "[Wallpaper Image] export received custom image data url: length={}, prefix={}",
        image_src.len(),
        &image_src[..image_src.len().min(48)]
    );

    let (_, encoded) = match image_src.split_once("base64,") {
        Some(parts) => parts,
        None => {
            println!("[Wallpaper Image] export decode failed: data URL missing base64 marker");
            return None;
        }
    };
    let bytes = match base64::engine::general_purpose::STANDARD.decode(encoded) {
        Ok(bytes) => {
            println!(
                "[Wallpaper Image] export base64 decoded: {} bytes",
                bytes.len()
            );
            bytes
        }
        Err(error) => {
            println!("[Wallpaper Image] export base64 decode failed: {}", error);
            return None;
        }
    };
    match image::load_from_memory(&bytes) {
        Ok(image) => {
            println!(
                "[Wallpaper Image] export image decoded: {}x{}",
                image.width(),
                image.height()
            );
            Some(image.to_rgba8())
        }
        Err(error) => {
            println!("[Wallpaper Image] export image decode failed: {}", error);
            None
        }
    }
}

fn build_gradient_config(visual: Option<&types::VisualSettings>) -> Option<GradientConfig> {
    let visual = visual?;
    let (colors_src, direction) = if visual.background_type.as_deref() == Some("wallpaper") {
        wallpaper_gradient(visual.wallpaper_id.as_deref()?)?
    } else if visual.background_type.as_deref() == Some("gradient") {
        let stops = visual.gradient_stops.as_ref()?;
        if stops.is_empty() {
            return None;
        }
        let direction = match visual.gradient_direction.as_deref() {
            Some("radial") => 3,
            Some("to-right") => 1,
            Some("to-bottom") => 2,
            _ => 0,
        };
        let owned: Vec<(String, f32)> = stops
            .iter()
            .map(|stop| {
                (
                    stop.color.clone(),
                    (stop.position as f32 / 100.0).clamp(0.0, 1.0),
                )
            })
            .collect();
        return gradient_from_stops(&owned, direction);
    } else {
        return None;
    };

    let owned: Vec<(String, f32)> = colors_src
        .iter()
        .enumerate()
        .map(|(i, color)| {
            let position = if colors_src.len() <= 1 {
                0.0
            } else {
                i as f32 / (colors_src.len() - 1) as f32
            };
            ((*color).to_string(), position)
        })
        .collect();
    gradient_from_stops(&owned, direction)
}

fn wallpaper_gradient(id: &str) -> Option<(&'static [&'static str], u32)> {
    match id {
        "gradient-purple" => Some((&["#667eea", "#764ba2"], 0)),
        "gradient-blue" => Some((&["#2193b0", "#6dd5ed"], 0)),
        "gradient-sunset" => Some((&["#ff6b6b", "#feca57", "#ff9ff3"], 0)),
        "gradient-ocean" => Some((&["#0f0c29", "#302b63", "#24243e"], 0)),
        "gradient-mint" => Some((&["#11998e", "#38ef7d"], 0)),
        "gradient-peach" => Some((&["#ee9ca7", "#ffdde1"], 0)),
        _ => None,
    }
}

fn gradient_from_stops(stops: &[(String, f32)], direction: u32) -> Option<GradientConfig> {
    let mut colors = [[0.0, 0.0, 0.0, 1.0]; 4];
    let mut positions = [0.0; 4];
    for (i, stop) in stops.iter().take(4).enumerate() {
        colors[i] = parse_hex_color(&stop.0);
        positions[i] = stop.1;
    }

    Some(GradientConfig {
        direction,
        colors,
        positions,
        count: stops.len().min(4) as u32,
    })
}

/// Build a GpuCompositorConfig with webcam info populated.
pub fn build_gpu_config_with_webcam(
    settings: &types::ExportSettings,
    webcam_info: &Option<(std::path::PathBuf, f64, f64, f64, String)>,
    source_width: Option<u32>,
    source_height: Option<u32>,
) -> GpuCompositorConfig {
    let mut config = build_gpu_config(settings, source_width, source_height);
    if webcam_info.is_some() {
        // Map corner name to normalized center position
        let size = settings.webcam_size.unwrap_or(0.15) as f32;
        let margin = 0.03_f32;
        let half = size / 2.0;
        let shape = settings
            .webcam_shape
            .clone()
            .unwrap_or_else(|| "circle".to_string());
        let corner = settings.webcam_corner.as_deref().unwrap_or("bottom-right");
        let (corner_x, corner_y) = match corner {
            "top-left" => (margin + half, margin + half),
            "top-right" => (1.0 - margin - half, margin + half),
            "bottom-left" => (margin + half, 1.0 - margin - half),
            _ => (1.0 - margin - half, 1.0 - margin - half), // bottom-right default
        };
        let min_center = half;
        let max_center = 1.0 - half;
        let pos_x = settings
            .webcam_x
            .map(|x| x as f32)
            .unwrap_or(corner_x)
            .clamp(min_center, max_center);
        let pos_y = settings
            .webcam_y
            .map(|y| y as f32)
            .unwrap_or(corner_y)
            .clamp(min_center, max_center);
        config.webcam = Some(WebcamConfig {
            pos_x,
            pos_y,
            size,
            shape,
        });
    }
    config
}

fn parse_hex_color(hex: &str) -> [f32; 4] {
    let hex = hex.trim_start_matches('#');
    if hex.len() >= 6 {
        let r = u8::from_str_radix(&hex[0..2], 16).unwrap_or(0) as f32 / 255.0;
        let g = u8::from_str_radix(&hex[2..4], 16).unwrap_or(0) as f32 / 255.0;
        let b = u8::from_str_radix(&hex[4..6], 16).unwrap_or(0) as f32 / 255.0;
        [r, g, b, 1.0]
    } else {
        [0.157, 0.165, 0.212, 1.0] // default #282a36
    }
}

fn resolve_background_color(visual: &types::VisualSettings) -> Option<&str> {
    if visual.background_type.as_deref() == Some("wallpaper") {
        match visual.wallpaper_id.as_deref()? {
            "solid-dark" => return Some("#1a1a2e"),
            "solid-light" => return Some("#f5f5f5"),
            "solid-blue" => return Some("#0a192f"),
            _ => {}
        }
    }
    visual.background_color.as_deref()
}
