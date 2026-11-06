use std::path::Path;

use super::super::types::ExportSettings;
use crate::cursor_renderer::{CursorEvent, SpringConfig, parse_cursor_events};

/// Find the cursor sidecar, including the original recording fallback used by
/// post-processed media.
pub(crate) fn resolve_mouse_events_path(input_path: &Path) -> std::path::PathBuf {
    let direct = crate::recording::artifacts::RecordingArtifacts::new(input_path).mouse();
    if direct.exists() {
        return direct;
    }

    let Some(file_name) = input_path.file_name().and_then(|name| name.to_str()) else {
        return direct;
    };
    if !file_name.starts_with("processed_") {
        return direct;
    }

    let original = input_path.with_file_name(file_name.replacen("processed_", "", 1));
    let fallback = crate::recording::artifacts::RecordingArtifacts::new(original).mouse();
    if fallback.exists() { fallback } else { direct }
}

/// Load raw cursor events from sidecar for zoom trajectory simulation.
pub fn load_raw_cursor_events(
    mouse_events_path: &Path,
    source_width: u32,
    source_height: u32,
) -> Vec<CursorEvent> {
    load_pointer_events(mouse_events_path, source_width, source_height, false, 0.0)
}

pub fn load_filtered_cursor_events(path: &Path, width: u32, height: u32, threshold: f64) -> Vec<CursorEvent> {
    load_pointer_events(path, width, height, false, threshold)
}

pub fn load_camera_events(
    mouse_events_path: &Path,
    source_width: u32,
    source_height: u32,
    threshold: f64,
) -> Vec<CursorEvent> {
    load_pointer_events(mouse_events_path, source_width, source_height, true, threshold)
}


/// Recording-space positions retain off-crop travel and do not depend on output size.
pub(crate) fn load_rotation_points(path: &Path, threshold: f64) -> Vec<crate::cursor_renderer::rotation::MotionPoint> {
    let Ok(content) = std::fs::read_to_string(path) else { return Vec::new(); };
    let Ok(sidecar) = serde_json::from_str::<serde_json::Value>(&content) else { return Vec::new(); };
    let Some(events) = sidecar.get("mouse_events").unwrap_or(&sidecar).as_array() else { return Vec::new(); };
    let mut points = super::camera_samples::remove_shake(events.clone(), threshold).iter().filter_map(|event| {
        let base=event.get("base").unwrap_or(event);
        let x=base.get("x")?.as_f64()?;
        let y=base.get("y")?.as_f64()?;
        if !x.is_finite() || !y.is_finite() { return None; }
        Some(crate::cursor_renderer::rotation::MotionPoint {
            time_ms:base.get("timestamp")?.as_u64()? as f64,x,y,
        })
    }).collect::<Vec<_>>();
    points.sort_by(|a,b|a.time_ms.total_cmp(&b.time_ms));
    points
}

fn load_pointer_events(
    mouse_events_path: &Path,
    source_width: u32,
    source_height: u32,
    camera: bool,
    threshold: f64,
) -> Vec<CursorEvent> {
    load_pointer_tracks(mouse_events_path,source_width,source_height,camera,false,threshold).0
}

pub(crate) fn load_camera_tracks(path: &Path,width:u32,height:u32,threshold:f64)->(Vec<CursorEvent>,Vec<CursorEvent>) {
    let (grouped,raw,_) = load_pointer_tracks(path,width,height,true,true,threshold);
    (grouped,raw)
}

fn load_pointer_tracks(
    mouse_events_path: &Path,
    source_width: u32,
    source_height: u32,
    camera: bool,
    include_raw: bool,
    threshold: f64,
) -> (Vec<CursorEvent>,Vec<CursorEvent>,(f64,f64)) {
    let content = match std::fs::read_to_string(mouse_events_path) {
        Ok(c) => c,
        Err(_) => return (Vec::new(),Vec::new(),(source_width as f64,source_height as f64)),
    };
    let sidecar: serde_json::Value = match serde_json::from_str(&content) {
        Ok(s) => s,
        Err(_) => return (Vec::new(),Vec::new(),(source_width as f64,source_height as f64)),
    };

    parse_pointer_tracks(&sidecar, source_width, source_height, camera, include_raw, threshold)
}

pub(crate) fn parse_filtered_cursor_events(
    sidecar: &serde_json::Value, width: u32, height: u32, threshold: f64,
) -> Vec<CursorEvent> {
    parse_pointer_tracks(sidecar, width, height, false, false, threshold).0
}

fn parse_pointer_tracks(
    sidecar: &serde_json::Value, source_width: u32, source_height: u32,
    camera: bool, include_raw: bool, threshold: f64,
) -> (Vec<CursorEvent>, Vec<CursorEvent>, (f64, f64)) {
    let (events, _sf, ex, ey, ew, eh) = if let Some(mouse_events) = sidecar.get("mouse_events") {
        let dw = sidecar
            .get("display_width")
            .and_then(|v| v.as_f64())
            .unwrap_or(source_width as f64);
        let dh = sidecar
            .get("display_height")
            .and_then(|v| v.as_f64())
            .unwrap_or(source_height as f64);
        let sf = sidecar
            .get("scale_factor")
            .and_then(|v| v.as_f64())
            .unwrap_or(1.0);
        let recording_area = sidecar.get("recording_area");
        let (ex, ey, ew, eh) = if let Some(area) = recording_area {
            (
                area.get("x").and_then(|v| v.as_f64()).unwrap_or(0.0),
                area.get("y").and_then(|v| v.as_f64()).unwrap_or(0.0),
                area.get("width").and_then(|v| v.as_f64()).unwrap_or(dw),
                area.get("height").and_then(|v| v.as_f64()).unwrap_or(dh),
            )
        } else {
            (0.0, 0.0, dw, dh)
        };
        (
            mouse_events.as_array().cloned().unwrap_or_default(),
            sf,
            ex,
            ey,
            ew,
            eh,
        )
    } else if let Some(arr) = sidecar.as_array() {
        (
            arr.clone(),
            1.0,
            0.0,
            0.0,
            source_width as f64,
            source_height as f64,
        )
    } else {
        return (Vec::new(),Vec::new(),(source_width as f64,source_height as f64));
    };

    let events = super::camera_samples::remove_shake(events, threshold);
    let raw = if include_raw { parse_cursor_events(&events,1.0,ex,ey,ew,eh) } else { Vec::new() };
    let events = if camera { super::camera_samples::filter(events) } else { events };
    let mut parsed=parse_cursor_events(&events, 1.0, ex, ey, ew, eh);
    // Camera targeting retains its existing edge policy independently of cursor travel.
    if camera { for event in &mut parsed { event.x=event.x.clamp(0.0,1.0);event.y=event.y.clamp(0.0,1.0); } }
    (parsed,raw,(ew,eh))
}

pub(crate) fn get_zoom_spring_config(settings: &ExportSettings) -> SpringConfig {
    if let (Some(t), Some(f), Some(m)) = (
        settings.zoom_spring_tension,
        settings.zoom_spring_friction,
        settings.zoom_spring_mass,
    ) {
        return SpringConfig {
            tension: t,
            friction: f,
            mass: m,
        };
    }
    let name = settings.animation_speed.as_deref().unwrap_or("mellow");
    resolve_spring_preset(name)
}

pub(crate) fn get_cursor_spring_config(settings: &ExportSettings) -> SpringConfig {
    if let Some(ref cursor) = settings.cursor_settings {
        if let (Some(t), Some(f), Some(m)) = (
            cursor.spring_tension,
            cursor.spring_friction,
            cursor.spring_mass,
        ) {
            return SpringConfig {
                tension: t,
                friction: f,
                mass: m,
            };
        }
        if let Some(ref preset) = cursor.speed_preset {
            return resolve_spring_preset(preset);
        }
    }
    SpringConfig {
        tension: 170.0,
        friction: 30.0,
        mass: 1.0,
    }
}

pub fn resolve_spring_preset(name: &str) -> SpringConfig {
    match name {
        "slow" => SpringConfig {
            tension: 120.0,
            friction: 28.0,
            mass: 1.0,
        },
        "mellow" => SpringConfig {
            tension: 170.0,
            friction: 30.0,
            mass: 1.0,
        },
        "quick" => SpringConfig {
            tension: 280.0,
            friction: 38.0,
            mass: 1.0,
        },
        "rapid" => SpringConfig {
            tension: 400.0,
            friction: 44.0,
            mass: 1.0,
        },
        _ => SpringConfig {
            tension: 170.0,
            friction: 30.0,
            mass: 1.0,
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rotation_retains_recording_distance_outside_capture_bounds() {
        let path=std::env::temp_dir().join(format!("rotation-space-{}.json",uuid::Uuid::new_v4()));
        std::fs::write(&path,r#"{"scale_factor":2,"recording_area":{"x":100,"y":200,"width":100,"height":100},"mouse_events":[{"base":{"timestamp":1000,"x":500,"y":250}},{"base":{"timestamp":0,"x":300,"y":250}}]}"#).unwrap();
        let normalized=load_filtered_cursor_events(&path,200,200,0.0);
        assert_eq!(normalized.iter().map(|event|event.x).collect::<Vec<_>>(),vec![2.0,4.0]);
        assert!(load_camera_events(&path,200,200,0.0).iter().all(|event|event.x==1.0));
        let pixels=load_rotation_points(&path,0.0);
        assert_eq!(pixels.len(),2);
        assert_eq!((pixels[0].time_ms,pixels[0].x,pixels[1].x),(0.0,300.0,500.0));
        let keys=crate::cursor_renderer::rotation::horizontal_tilt(&pixels,1.0);
        assert!((crate::cursor_renderer::rotation::sample(&keys,500.0)-2.4).abs()<1e-10);
        std::fs::remove_file(path).unwrap();
    }

    #[test]
    fn cursor_normalization_keeps_off_window_positions_and_rejects_invalid_geometry() {
        let events=vec![serde_json::json!({"timestamp":0,"x":-140.0,"y":30.0}),
            serde_json::json!({"timestamp":100,"x":200.0,"y":170.0})];
        let cursor=parse_cursor_events(&events,2.0,-100.0,50.0,200.0,100.0);
        assert_eq!((cursor[0].x,cursor[0].y),(-0.2,-0.2));
        assert_eq!((cursor[1].x,cursor[1].y),(1.5,1.2));
        let settings=serde_json::from_value(serde_json::json!({"hide_when_idle":false})).unwrap();
        for fps in [24.0,30.0,60.0,120.0] {
            let frames=crate::cursor_renderer::simulate_cursor_frames(&cursor,
                &SpringConfig{tension:470.0,friction:70.0,mass:3.0},fps,2000,&None,&settings,
                |frame|(frame.x,frame.y));
            let last=frames.last().unwrap();
            assert!(last.0>1.4&&last.1>1.1,"off-window cursor pinned at {last:?} ({fps}fps)");
            assert!(frames.iter().all(|(x,y)|x.is_finite()&&y.is_finite()));
        }
        for width in [0.0,-1.0,f64::NAN,f64::INFINITY] {
            assert!(parse_cursor_events(&events,2.0,0.0,0.0,width,100.0).is_empty());
        }
    }

    #[test]
    fn explicit_screen_styles_reach_export_without_legacy_speed_overriding_them() {
        for (name, tension, friction, mass) in
            [("focused", 250.0, 40.0, 2.25), ("smooth", 170.0, 50.0, 3.0)]
        {
            let settings: ExportSettings = serde_json::from_value(serde_json::json!({
                "screen_motion_style": name, "animation_speed": "rapid",
                "zoom_spring_tension": tension, "zoom_spring_friction": friction,
                "zoom_spring_mass": mass
            }))
            .unwrap();
            let config = get_zoom_spring_config(&settings);
            assert_eq!(
                (config.tension, config.friction, config.mass),
                (tension, friction, mass)
            );
        }
    }

    #[test]
    fn explicit_cursor_styles_reach_export_without_legacy_speed_overriding_them() {
        for (tension, friction, mass) in [
            (470.0, 70.0, 3.0),
            (340.0, 60.0, 3.0),
            (530.0, 40.0, 1.0),
            (1.0, 0.0, 0.0),
        ] {
            let settings: ExportSettings = serde_json::from_value(serde_json::json!({
                "cursor_settings": { "speed_preset": "slow", "spring_tension": tension,
                    "spring_friction": friction, "spring_mass": mass }
            }))
            .unwrap();
            let config = get_cursor_spring_config(&settings);
            assert_eq!(
                (config.tension, config.friction, config.mass),
                (tension, friction, mass)
            );
            if mass == 0.0 {
                let state = crate::cursor_renderer::spring_step(0.2, 0.8, 5.0, &config, 1.0 / 60.0);
                assert_eq!((state.value, state.velocity), (0.8, 0.0));
            }
        }
    }
}

/// Prepare source-time camera input once, before decoder trim settings change.
pub(crate) fn load_camera_activity(mouse: &Path, recording: &Path, source: (u32,u32),
    settings: &ExportSettings, duration: u64) -> anyhow::Result<super::super::camera_activity::Tracks> {
    let (raw,_,logical)=load_pointer_tracks(mouse,source.0,source.1,false,false,
        settings.cursor_settings.as_ref().and_then(|c|c.shake_threshold).unwrap_or(0.0));
    super::super::camera_activity::Tracks::prepare(&raw,logical,source,settings,duration,recording)
}
