//! Prepare one planned position channel shared by renderer entry points.
use super::{input::RecordingInput, playback, Point, SourcePath, SpringConfig};
use crate::video_processing::{source_crop::SourceCrop, timeline::EditTimeline, CursorSettings};
use anyhow::Result;
use std::path::Path;

pub struct Prepared {
    motion: super::motion_chunks::PlannedMotion,
    source_press: super::press::PressTrack,
    width: f64,
    height: f64,
}
impl Prepared {
    pub fn rotation_keys(&self) -> Option<&[crate::cursor_renderer::rotation::RotationKey]> {
        self.motion.rotation.as_deref()
    }
    /// Sample source time, so a speed change does not stretch the lookahead
    /// window. A caller applies its clip's source-time mapping first.
    pub fn pressed_at_source(&self, time: f64, smooth: bool) -> bool {
        self.source_press.pressed(time, smooth)
    }
    pub fn target_time_of(&self, time: f64) -> f64 {
        self.motion.target_time_of(time)
    }

    pub fn at(&self, time: f64) -> [f64; 2] {
        let position = time * 60.0 / 1000.0;
        let position = if (position - position.round()).abs() < 1e-7 {
            position.round()
        } else {
            position
        };
        let frame = position.floor();
        let lower = self.motion.at(frame * 1000.0 / 60.0);
        let fraction = position - frame;
        let point = if fraction == 0.0 {
            lower
        } else {
            let upper = self.motion.at((frame + 1.0) * 1000.0 / 60.0);
            std::array::from_fn(|i| lower[i] + (upper[i] - lower[i]) * fraction)
        };
        [point[0] / self.width, point[1] / self.height]
    }
}

pub fn prepare(
    sidecar: &Path,
    size: (u32, u32),
    crop: Option<SourceCrop>,
    timeline: Option<&EditTimeline>,
    recording: &Path,
    duration: f64,
    settings: &CursorSettings,
    spring: SpringConfig,
    check: Option<&dyn Fn() -> Result<()>>,
) -> Result<Option<Prepared>> {
    let check = || check.map_or(Ok(()), |check| check());
    check()?;
    if !sidecar.exists() {
        return Ok(None);
    }
    let mut input = RecordingInput::load(
        sidecar,
        size,
        settings.shake_threshold.unwrap_or(0.0),
        crop,
        !settings.always_use_pointer.unwrap_or(false),
        settings.optimize_cursor_types.unwrap_or(true),
    )?;
    #[cfg(target_os = "macos")]
    if spring.mass > 0.0 && settings.follow_hover.unwrap_or(false) {
        let times = crate::cursor_renderer::hover::analysis::analyze_with_check(
            recording, sidecar, size, &check,
        )?;
        input
            .sync
            .extend(times.iter().map(|&time_ms| super::activity::SyncPoint {
                time_ms,
                tolerance: 4.0,
            }));
        input.sync.sort_by(|a, b| a.time_ms.total_cmp(&b.time_ms));
    }
    let mut source_press = super::press::PressTrack::new(std::mem::take(&mut input.press_events));
    let looped = settings.loop_to_start.unwrap_or(false);
    let playback_duration = timeline.map_or(duration, |t| t.duration_ms);
    let cutoff = if looped {
        (playback_duration - settings.loop_duration_ms.unwrap_or(500) as f64).max(0.0)
    } else {
        playback_duration
    };
    let bounds = timeline.map_or(Some((0.0, cutoff)), |t| {
        Some((
            super::press_window::boundary(t, 0.0, recording)?,
            super::press_window::boundary(t, cutoff, recording)?,
        ))
    });
    if let Some((start, end)) = bounds {
        source_press = source_press.window(start, end, looped);
    }
    let rotating = settings.rotate_while_moving.unwrap_or(false);
    let rotation_source = rotating.then(|| input.points.clone());
    if let Some((start, end)) = bounds.filter(|_| looped) {
        input.points = super::source_window::select(
            &input.points,
            start,
            end,
            true,
            |p| p.time_ms,
            |p, time_ms| Point { time_ms, ..p },
        );
        input
            .sync
            .retain(|p| p.time_ms >= start && p.time_ms <= end);
    }
    let mut mapped = if let Some(timeline) = timeline {
        playback::map(&input, timeline, recording)
    } else {
        playback::PlaybackInput {
            points: input.points,
            rotation_events: None,
            sync: input.sync,
            cuts: Vec::new(),
            instant: Vec::new(),
            duration,
            width: input.width,
            height: input.height,
        }
    };
    if mapped.points.is_empty() {
        return Ok(None);
    }
    if settings.stop_at_end.unwrap_or(false) && settings.stop_duration_ms.unwrap_or(0) > 0 {
        let cutoff = (mapped.duration - settings.stop_duration_ms.unwrap_or(0) as f64).max(0.0);
        let position = SourcePath::new(mapped.points.clone())?.at(cutoff);
        mapped.points.retain(|p| p.time_ms <= cutoff);
        mapped.sync.retain(|p| p.time_ms <= cutoff);
        let stopped = Point {
            time_ms: cutoff,
            x: position[0],
            y: position[1],
        };
        mapped.points.push(stopped);
        if let Some(events) = mapped.rotation_events.as_mut() {
            events.retain(|p| p.time_ms <= cutoff);
            events.push(stopped);
        }
    }
    if spring.mass == 0.0 {
        mapped.instant = vec![(f64::NEG_INFINITY, f64::INFINITY)];
    }
    let motion = super::motion_chunks::build(
        &mapped,
        rotation_source.as_deref(),
        spring,
        (settings.rotation_mode.as_deref() != Some("directional"))
            .then(|| settings.rotation_intensity.unwrap_or(30.0) / 100.0),
        |time| {
            timeline.map_or(Some(time), |timeline| {
                super::press_window::boundary(timeline, time, recording)
            })
        },
        &check,
    )?;
    Ok(Some(Prepared {
        motion,
        source_press,
        width: mapped.width,
        height: mapped.height,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    #[ignore = "opt-in recorded cursor planning profile"]
    fn recorded_cursor_planning_profile() {
        let sidecar = std::path::PathBuf::from(
            std::env::var("TARANTINO_CURSOR_PROFILE_INPUT").unwrap(),
        );
        let duration: f64 = std::env::var("TARANTINO_CURSOR_PROFILE_DURATION")
            .unwrap().parse().unwrap();
        let data: serde_json::Value =
            serde_json::from_slice(&std::fs::read(&sidecar).unwrap()).unwrap();
        let size = (data["display_width"].as_u64().unwrap() as u32,
            data["display_height"].as_u64().unwrap() as u32);
        for mode in ["off", "horizontal", "directional"] {
            let settings = serde_json::from_value(serde_json::json!({
                "follow_hover":false,"rotate_while_moving":mode != "off",
                "rotation_mode":mode,
            })).unwrap();
            let start = std::time::Instant::now();
            let planned = prepare(&sidecar,size,None,None,&sidecar,duration,&settings,
                SpringConfig { tension:470.0,friction:70.0,mass:3.0 },None)
                .unwrap().unwrap();
            let preparation_ms = start.elapsed().as_secs_f64() * 1000.0;
            let start = std::time::Instant::now();
            let mut checksum = 0.0;
            let frames = (duration * 60.0 / 1000.0).ceil() as usize;
            for frame in 0..frames {
                let point = planned.at(frame as f64 * 1000.0 / 60.0);
                assert!(point.iter().all(|value| value.is_finite()));
                checksum += point[0] + point[1];
            }
            println!("CURSOR_PROFILE mode={mode} duration_ms={duration} frames={frames} prepare_ms={preparation_ms:.3} sample_ms={:.3} checksum={checksum}",
                start.elapsed().as_secs_f64() * 1000.0);
        }
    }

    #[test]
    #[ignore = "requires external cursor position timeline fixtures"]
    fn prepared_positions_match_external_style_timeline() {
        let path = std::env::var("TARANTINO_CURSOR_POSITION_TIMELINE").unwrap();
        let cases: serde_json::Value =
            serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap();
        let cases = cases.as_array().unwrap();
        assert!(!cases.is_empty());
        for case in cases {
            let sidecar =
                std::env::temp_dir().join(format!("cursor-position-{}.json", uuid::Uuid::new_v4()));
            let events = case["points"]
                .as_array()
                .unwrap()
                .iter()
                .map(|p| {
                    serde_json::json!({
                        "timestamp":p["timeMs"],"x":p["x"],"y":p["y"],"event_type":"Move",
                    })
                })
                .collect::<Vec<_>>();
            std::fs::write(
                &sidecar,
                serde_json::to_vec(&serde_json::json!({
                    "display_width":1000,"display_height":500,"mouse_events":events,
                }))
                .unwrap(),
            )
            .unwrap();
            let spring = SpringConfig {
                tension: case["spring"]["stiffness"].as_f64().unwrap(),
                friction: case["spring"]["damping"].as_f64().unwrap(),
                mass: case["spring"]["mass"].as_f64().unwrap(),
            };
            let settings =
                serde_json::from_value(serde_json::json!({"follow_hover":false})).unwrap();
            let planned = prepare(
                &sidecar,
                (1000, 500),
                None,
                None,
                &sidecar,
                1200.0,
                &settings,
                spring,
                None,
            )
            .unwrap()
            .unwrap();
            for sample in case["samples"].as_array().unwrap() {
                let time = sample["time"].as_f64().unwrap();
                let actual = planned.at(time);
                for (axis, key, extent) in [(0, "x", 1000.0), (1, "y", 500.0)] {
                    let expected = sample["expected"][key].as_f64().unwrap();
                    assert!(
                        (actual[axis] * extent - expected).abs() < 1e-5,
                        "{time}ms {key}: {} != {expected}",
                        actual[axis] * extent
                    );
                }
            }
            std::fs::remove_file(sidecar).unwrap();
        }
    }

    #[test]
    fn planned_positions_reach_clicks_in_logical_pixels_and_feed_render_state() {
        let sidecar =
            std::env::temp_dir().join(format!("cursor-planning-{}.json", uuid::Uuid::new_v4()));
        let events=(0..=100).map(|i|serde_json::json!({"timestamp":i*10,"x":i*2,"y":100,
            "event_type":if i==50 {serde_json::json!({"ButtonPress":{"button":"Left"}})}
                else if i==51 {serde_json::json!({"ButtonRelease":{"button":"Left"}})} else {serde_json::json!("Move")}
        })).collect::<Vec<_>>();
        std::fs::write(
            &sidecar,
            serde_json::to_vec(
                &serde_json::json!({"display_width":1000,"display_height":500,
            "scale_factor":2,"mouse_events":events}),
            )
            .unwrap(),
        )
        .unwrap();
        let settings: CursorSettings =
            serde_json::from_value(serde_json::json!({"enabled":true,"click_effect":"none"}))
                .unwrap();
        let spring = SpringConfig {
            tension: 470.0,
            friction: 70.0,
            mass: 3.0,
        };
        let planned = prepare(
            &sidecar,
            (2000, 1000),
            None,
            None,
            &sidecar,
            1000.0,
            &settings,
            spring,
            None,
        )
        .unwrap()
        .unwrap();
        assert!((planned.at(500.0)[0] * 1000.0 - 100.0).abs() < 1.5);
        assert!(planned.pressed_at_source(500.0, false));
        assert!(!planned.pressed_at_source(510.0, false));
        assert!(planned.pressed_at_source(0.0, true));
        assert!(!planned.pressed_at_source(500.001, true));
        let raw =
            crate::video_processing::export::load_filtered_cursor_events(&sidecar, 2000, 1000, 0.0);
        for fps in [24.0, 30.0, 60.0, 120.0] {
            let position = |t| planned.at(t);
            let frames = crate::cursor_renderer::simulate_cursor_frames_from(
                &raw,
                &spring,
                fps,
                1000,
                &None,
                &settings,
                0,
                120.0,
                None,
                None,
                Some(&position),
                Some((&|t| planned.pressed_at_source(t, true), 120.0)),
                Clone::clone,
            );
            for (index, frame) in frames.iter().enumerate() {
                let time = (index as f64 * 120.0 / fps + 1e-7).floor() * 1000.0 / 120.0;
                assert!((frame.x as f64 - planned.at(time)[0]).abs() < 1e-7);
                assert!(frame.opacity > 0.99);
                assert!(frame.press_amount.is_finite());
            }
        }
        let mut loop_settings = settings.clone();
        loop_settings.loop_to_start = Some(true);
        loop_settings.loop_duration_ms = Some(600);
        let looped = prepare(
            &sidecar,
            (2000, 1000),
            None,
            None,
            &sidecar,
            1000.0,
            &loop_settings,
            spring,
            None,
        )
        .unwrap()
        .unwrap();
        assert!(!looped.pressed_at_source(0.0, true));
        assert!(!looped.pressed_at_source(500.0, false));
        let clip = serde_json::from_value(serde_json::json!({"id":"trimmed","startTime":0,
            "duration":200,"sourceIn":0,"sourceOut":400,"playbackRate":2,
            "trackOrder":0,"muted":false}))
        .unwrap();
        let timeline = EditTimeline::build(&[clip], 0.0, 200.0).unwrap();
        let trimmed = prepare(
            &sidecar,
            (2000, 1000),
            None,
            Some(&timeline),
            &sidecar,
            1000.0,
            &settings,
            spring,
            None,
        )
        .unwrap()
        .unwrap();
        assert!(!trimmed.pressed_at_source(0.0, true));
        assert!(!trimmed.pressed_at_source(399.0, true));
        std::fs::remove_file(sidecar).unwrap();
    }
}

#[cfg(test)]
mod press_render_tests {
    use crate::cursor_renderer::{simulate_cursor_frames_from, CursorEvent, SpringConfig};
    use crate::video_processing::CursorSettings;

    #[test]
    fn configured_press_clock_is_independent_of_transport_cadence() {
        let settings: CursorSettings =
            serde_json::from_value(serde_json::json!({"enabled":true})).unwrap();
        let events = [CursorEvent {
            is_release: false,
            timestamp_ms: 0,
            x: 0.5,
            y: 0.5,
            is_click: false,
        }];
        let pressed = |t| (100.0..600.0).contains(&t);
        let smooth = SpringConfig {
            tension: 470.0,
            friction: 70.0,
            mass: 3.0,
        };
        let run = |fps, spring: &SpringConfig| {
            simulate_cursor_frames_from(
                &events,
                spring,
                fps,
                2000,
                &None,
                &settings,
                0,
                120.0,
                None,
                None,
                None,
                Some((&pressed, 120.0)),
                |f| f.press_amount,
            )
        };
        let opening = simulate_cursor_frames_from(
            &events,
            &smooth,
            120.0,
            100,
            &None,
            &settings,
            0,
            120.0,
            None,
            None,
            None,
            Some((&|_| true, 120.0)),
            |f| f.press_amount,
        );
        assert!(
            opening.iter().all(|amount| *amount == 0.1),
            "opening target must start at rest"
        );
        let reference = run(120.0, &smooth);
        assert!(reference[60] > 0.09);
        assert!(reference[72] > 0.09); // Release begins a spring return, not a jump.
        assert!(reference[230].abs() < 0.001);
        for fps in [24.0, 30.0, 60.0] {
            for (i, amount) in run(fps, &smooth).iter().enumerate() {
                let index = (i as f64 * 120.0 / fps + 1e-7).floor() as usize;
                assert_eq!(*amount, reference[index]);
            }
        }
        let raw = run(
            120.0,
            &SpringConfig {
                mass: 0.0,
                ..smooth
            },
        );
        assert_eq!(raw[11], 0.0);
        assert_eq!(raw[12], 0.1);
        assert_eq!(raw[71], 0.1);
        assert_eq!(raw[72], 0.0);
    }
}
