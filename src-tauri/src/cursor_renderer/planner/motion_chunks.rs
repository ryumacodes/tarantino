use super::{plan, playback::PlaybackInput, MotionTrack, Point, SourcePath, SpringConfig};
use crate::cursor_renderer::rotation::{
    directional, horizontal_tilt, sample, MotionPoint, RotationKey,
};
use anyhow::Result;

const CHUNK_MS: f64 = 10_000.0;
const LOOKAHEAD_MS: f64 = 2100.0;

struct Chunk {
    motion: MotionTrack,
    lead: super::timing::LeadSchedule,
}

pub(super) struct PlannedMotion {
    chunks: Vec<Chunk>,
    pub rotation: Option<Vec<RotationKey>>,
}

impl PlannedMotion {
    fn chunk(&self, time: f64) -> (&Chunk, f64) {
        let index = ((time / CHUNK_MS).floor() as usize).min(self.chunks.len() - 1);
        (&self.chunks[index], time - index as f64 * CHUNK_MS)
    }

    pub fn at(&self, time: f64) -> [f64; 2] {
        let (chunk, local) = self.chunk(time);
        chunk.motion.at(local)
    }

    pub fn target_time_of(&self, time: f64) -> f64 {
        let (chunk, local) = self.chunk(time);
        time - local + chunk.lead.target_time_of(local)
    }
}

fn settling_time(spring: SpringConfig) -> f64 {
    let step = if (spring.tension / spring.mass)
        .sqrt()
        .max(spring.friction / spring.mass)
        / 60.0
        <= 0.5
    {
        1000.0 / 60.0
    } else {
        1.0
    };
    let (mut value, mut velocity, mut time) = (0.0_f64, 0.000001_f64, 0.0);
    while value != 1000.0 && velocity != 0.0 && time <= 20_000.0 {
        velocity += ((1000.0 - value) * spring.tension - velocity * spring.friction) / spring.mass
            * step
            / 1000.0;
        value += velocity * step / 1000.0;
        if velocity.abs() < 2.0 && (value - 1000.0).abs() < 2.0 {
            value = 1000.0;
            velocity = 0.0;
        }
        time += step;
    }
    time
}

pub(super) fn build(
    mapped: &PlaybackInput,
    source: Option<&[Point]>,
    spring: SpringConfig,
    horizontal_ratio: Option<f64>,
    source_time: impl Fn(f64) -> Option<f64>,
    check: &dyn Fn() -> Result<()>,
) -> Result<PlannedMotion> {
    let motion = |p: &Point| MotionPoint {
        time_ms: p.time_ms,
        x: p.x,
        y: p.y,
    };
    let history = source
        .filter(|_| horizontal_ratio.is_none())
        .map_or_else(Vec::new, |source| {
            directional::targets(&source.iter().map(motion).collect::<Vec<_>>(), None)
        });
    let normalized = if spring.mass == 0.0 {
        SpringConfig {
            tension: 125.0,
            friction: 12.0,
            mass: 1.5,
        }
    } else {
        spring
    };
    let history_ms = settling_time(normalized) + LOOKAHEAD_MS;
    let mut chunks = Vec::new();
    let mut rotation = source.map(|_| Vec::new());
    for index in 0..=(mapped.duration / CHUNK_MS).floor() as usize {
        check()?;
        let origin = index as f64 * CHUNK_MS;
        let planned_from = origin - history_ms;
        let from = planned_from.max(0.0);
        let to = (origin + CHUNK_MS + LOOKAHEAD_MS).min(mapped.duration);
        let first = mapped.points.partition_point(|p| p.time_ms < from);
        let last = mapped.points.partition_point(|p| p.time_ms <= to);
        let seed_index = mapped
            .points
            .partition_point(|p| p.time_ms <= from)
            .saturating_sub(1);
        let seed = Point {
            time_ms: from - origin,
            ..mapped.points[seed_index]
        };
        let mut points = vec![seed];
        points.extend(mapped.points[first..last].iter().map(|p| Point {
            time_ms: p.time_ms - origin,
            ..*p
        }));
        let sync = mapped
            .sync
            .iter()
            .filter(|p| p.time_ms >= from && p.time_ms <= to)
            .map(|p| super::activity::SyncPoint {
                time_ms: p.time_ms - origin,
                ..*p
            })
            .collect();
        let cuts = mapped
            .cuts
            .iter()
            .filter(|t| **t >= from && **t <= to)
            .map(|t| t - origin)
            .collect();
        let instant = if spring.mass == 0.0 {
            vec![(f64::NEG_INFINITY, f64::INFINITY)]
        } else {
            mapped
                .instant
                .iter()
                .filter(|(a, b)| *b > from && *a < to)
                .map(|(a, b)| (a.max(from) - origin, b.min(to) - origin))
                .collect::<Vec<_>>()
        };
        let planned = plan::build(
            &SourcePath::new(points)?,
            normalized,
            sync,
            cuts,
            &instant,
            to - origin,
        )?;
        chunks.push(Chunk {
            motion: planned
                .motion
                .into_playback_window(CHUNK_MS.min(mapped.duration - origin)),
            lead: planned.lead,
        });
        let Some(result) = rotation.as_mut() else {
            continue;
        };
        let lead = &chunks.last().unwrap().lead;
        let mut drawn = vec![motion(&seed)];
        let events = mapped.rotation_events.as_deref().unwrap_or(&mapped.points);
        let first_event = events.partition_point(|p| p.time_ms < from);
        let last_event = events.partition_point(|p| p.time_ms <= to);
        drawn.extend(events[first_event..last_event].iter().map(|p| MotionPoint {
            time_ms: lead.target_time_of(p.time_ms - origin),
            x: p.x,
            y: p.y,
        }));
        drawn.sort_by(|a, b| a.time_ms.total_cmp(&b.time_ms));
        let opening = if planned_from > 0.0 {
            source_time(planned_from).map(|time| sample(&history, time).to_radians())
        } else {
            None
        };
        let keys = horizontal_ratio.map_or_else(
            || directional::targets(&drawn, opening),
            |ratio| horizontal_tilt(&drawn, ratio),
        );
        let end = CHUNK_MS.min(mapped.duration - origin);
        result.push(RotationKey {
            time_ms: origin,
            degrees: sample(&keys, 0.0),
        });
        result.extend(
            keys.iter()
                .filter(|k| k.time_ms > 0.0 && k.time_ms < end)
                .map(|k| RotationKey {
                    time_ms: origin + k.time_ms,
                    ..*k
                }),
        );
        result.push(RotationKey {
            time_ms: origin + end,
            degrees: sample(&keys, end),
        });
    }
    Ok(PlannedMotion { chunks, rotation })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn long_drag_positions_use_local_plans_with_or_without_rotation() {
        let source = (0..=1200)
            .map(|i| Point {
                time_ms: i as f64 * 20.0,
                x: 500.0 + 240.0 * (i as f64 * 0.013).sin(),
                y: 300.0 + 160.0 * (i as f64 * 0.023).sin(),
            })
            .collect::<Vec<_>>();
        let sync = source
            .iter()
            .filter(|p| (8000.0..=16000.0).contains(&p.time_ms))
            .map(|p| super::super::activity::SyncPoint {
                time_ms: p.time_ms,
                tolerance: 2.0,
            })
            .collect();
        let mapped = PlaybackInput {
            points: source.clone(),
            rotation_events: None,
            sync,
            cuts: Vec::new(),
            instant: Vec::new(),
            duration: 24000.0,
            width: 1000.0,
            height: 600.0,
        };
        let spring = SpringConfig {
            tension: 530.0,
            friction: 40.0,
            mass: 1.0,
        };
        let plain = build(&mapped, None, spring, None, Some, &|| Ok(())).unwrap();
        let rotated = build(&mapped, Some(&source), spring, None, Some, &|| Ok(())).unwrap();
        for (time, expected) in [
            (24000.0, [537.3594970703125, 410.0735168457031]),
            (7816.666666666667, [273.7863972981771, 374.8765767415364]),
            (10016.666666666666, [555.0820312499999, 162.1280161539713]),
            (9983.333333333334, [550.1116739908855, 159.25713602701828]),
            (10000.0, [552.5490112304688, 160.6027374267578]),
            (16000.0, [300.8428649902344, 231.55087280273438]),
        ] {
            for plan in [&plain, &rotated] {
                let actual = plan.at(time);
                assert!(
                    (actual[0] - expected[0]).hypot(actual[1] - expected[1]) < 1e-5,
                    "position at {time}"
                );
            }
        }
    }

    #[test]
    fn drag_constraints_retime_rotation_across_segment_boundaries() {
        let source = (0..=1200)
            .map(|i| Point {
                time_ms: i as f64 * 20.0,
                x: 500.0 + 240.0 * (i as f64 * 0.013).sin(),
                y: 300.0 + 160.0 * (i as f64 * 0.023).sin(),
            })
            .collect::<Vec<_>>();
        let sync = source
            .iter()
            .filter(|p| {
                (9600.0..=10400.0).contains(&p.time_ms) || (19600.0..=20400.0).contains(&p.time_ms)
            })
            .map(|p| super::super::activity::SyncPoint {
                time_ms: p.time_ms,
                tolerance: 2.0,
            })
            .collect();
        let mapped = PlaybackInput {
            points: source.clone(),
            rotation_events: None,
            sync,
            cuts: Vec::new(),
            instant: Vec::new(),
            duration: 24000.0,
            width: 1000.0,
            height: 600.0,
        };
        let keys = build(
            &mapped,
            Some(&source),
            SpringConfig {
                tension: 530.0,
                friction: 40.0,
                mass: 1.0,
            },
            None,
            Some,
            &|| Ok(()),
        )
        .unwrap()
        .rotation
        .unwrap();
        for (time, expected) in [
            (9575.0, 88.58726609903457),
            (9975.0, 93.96798345337656),
            (10000.0, 94.85377010755454),
            (10025.0, 95.28173800922453),
            (10400.0, 105.37846132584538),
            (19975.0, 66.05658891627075),
            (20000.0, 66.03465878982061),
            (20025.0, 66.14766053531606),
        ] {
            assert!(
                (sample(&keys, time) - expected).abs() < 1e-8,
                "rotation at {time}ms"
            );
        }
    }

    #[test]
    #[ignore = "requires external segmented rotation fixtures"]
    fn segmented_rotation_matches_external_timeline() {
        let path = std::env::var("TARANTINO_ROTATION_CHUNKS").unwrap();
        let data: serde_json::Value =
            serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap();
        for case in data["results"].as_array().unwrap() {
            let source = case["points"]
                .as_array()
                .unwrap()
                .iter()
                .map(|p| Point {
                    time_ms: p["recordingTimeMs"].as_f64().unwrap(),
                    x: p["x"].as_f64().unwrap(),
                    y: p["y"].as_f64().unwrap(),
                })
                .collect::<Vec<_>>();
            let direct = case["style"] == "none";
            let mapped = PlaybackInput {
                points: source.clone(),
                rotation_events: None,
                sync: case["sync"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|p| super::super::activity::SyncPoint {
                        time_ms: p["timeMs"].as_f64().unwrap(),
                        tolerance: p["tolerancePt"].as_f64().unwrap(),
                    })
                    .collect(),
                cuts: Vec::new(),
                instant: if direct {
                    vec![(f64::NEG_INFINITY, f64::INFINITY)]
                } else {
                    Vec::new()
                },
                duration: 24000.0,
                width: 1000.0,
                height: 600.0,
            };
            let spring = SpringConfig {
                tension: case["spring"]["stiffness"].as_f64().unwrap(),
                friction: case["spring"]["damping"].as_f64().unwrap(),
                mass: if direct {
                    0.0
                } else {
                    case["spring"]["mass"].as_f64().unwrap()
                },
            };
            let keys = build(&mapped, Some(&source), spring, None, Some, &|| Ok(()))
                .unwrap()
                .rotation
                .unwrap();
            let sidecar = std::env::temp_dir()
                .join(format!("segmented-rotation-{}.json", uuid::Uuid::new_v4()));
            let events = source.iter().zip(case["points"].as_array().unwrap()).map(|(p,raw)| {
                let kind = match raw["type"].as_str().unwrap() {
                    "mouseDown"=>serde_json::json!({"ButtonPress":{"button":"Left"}}),
                    "mouseUp"=>serde_json::json!({"ButtonRelease":{"button":"Left"}}),
                    _=>serde_json::json!("Move"),
                };
                serde_json::json!({"timestamp":p.time_ms as u64,"x":p.x,"y":p.y,"event_type":kind})
            }).collect::<Vec<_>>();
            std::fs::write(&sidecar,serde_json::to_vec(&serde_json::json!({"display_width":1000,"display_height":600,"mouse_events":events})).unwrap()).unwrap();
            let settings = serde_json::from_value(serde_json::json!({"rotate_while_moving":true,"rotation_mode":"directional","follow_hover":false})).unwrap();
            let prepared = super::super::runtime::prepare(
                &sidecar,
                (1000, 600),
                None,
                None,
                &sidecar,
                24000.0,
                &settings,
                spring,
                None,
            )
            .unwrap()
            .unwrap();
            std::fs::remove_file(sidecar).unwrap();
            if let Some(samples) = case["positionSamples"].as_array() {
                for expected in samples {
                    let time = expected["time_ms"].as_f64().unwrap();
                    let actual = prepared.at(time);
                    let error = (actual[0] * 1000.0 - expected["x"].as_f64().unwrap())
                        .hypot(actual[1] * 600.0 - expected["y"].as_f64().unwrap());
                    assert!(
                        error < 1e-5,
                        "{} {} position at {time}ms differs by {error}px",
                        case["scenario"],
                        case["style"]
                    );
                }
            }
            for expected in case["samples"].as_array().unwrap() {
                let time = expected["time_ms"].as_f64().unwrap();
                let angle = sample(&keys, time);
                assert!(
                    (sample(prepared.rotation_keys().unwrap(), time) - angle).abs() < 1e-8,
                    "prepared rotation at {time}"
                );
                assert!(
                    (angle - expected["degrees"].as_f64().unwrap()).abs() < 1e-8,
                    "{} {time}ms: {angle} vs {}",
                    case["style"],
                    expected["degrees"]
                );
            }
            let mut animation = crate::cursor_renderer::angular_spring::AngularTimeline::for_mode(
                sample(&keys, 0.0),
                Some("directional"),
            );
            for expected in case["animated"].as_array().unwrap() {
                let time = expected["time_ms"].as_f64().unwrap();
                let actual = animation.sample(time, |t| sample(&keys, t));
                assert!(
                    (actual - expected["chunkedDegrees"].as_f64().unwrap()).abs() < 1e-8,
                    "{} animated {time}ms: {actual} vs {}",
                    case["style"],
                    expected["chunkedDegrees"]
                );
            }
        }
    }
}

#[cfg(test)]
mod edited_tests {
    use super::*;
    use crate::video_processing::timeline::{EditClip, EditTimeline};

    #[test]
    #[ignore = "requires external edited rotation fixtures"]
    fn edited_rotation_matches_external_timeline() {
        let path = std::env::var("TARANTINO_EDITED_ROTATION").unwrap();
        let data: serde_json::Value =
            serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap();
        let mut failures = Vec::new();
        for case in data["results"].as_array().unwrap() {
            let clips: Vec<EditClip> = serde_json::from_value(case["clips"].clone()).unwrap();
            let duration = case["duration"].as_f64().unwrap();
            let timeline = EditTimeline::build(&clips, 0.0, duration).unwrap();
            let sidecar =
                std::env::temp_dir().join(format!("edited-rotation-{}.json", uuid::Uuid::new_v4()));
            let events = case["points"]
                .as_array()
                .unwrap()
                .iter()
                .map(|p| {
                    let kind = match p["type"].as_str().unwrap() {
                        "mouseDown" => serde_json::json!({"ButtonPress":{"button":"Left"}}),
                        "mouseUp" => serde_json::json!({"ButtonRelease":{"button":"Left"}}),
                        _ => serde_json::json!("Move"),
                    };
                    serde_json::json!({"timestamp":p["recordingTimeMs"].as_u64().unwrap(),
                    "x":p["x"],"y":p["y"],"event_type":kind})
                })
                .collect::<Vec<_>>();
            std::fs::write(&sidecar,serde_json::to_vec(&serde_json::json!({"display_width":1000,"display_height":600,"mouse_events":events,"cursor_images":case["cursorImages"]})).unwrap()).unwrap();
            let spring = SpringConfig {
                tension: case["spring"]["stiffness"].as_f64().unwrap(),
                friction: case["spring"]["damping"].as_f64().unwrap(),
                mass: case["spring"]["mass"].as_f64().unwrap(),
            };
            let mode = case["mode"].as_str().unwrap_or("directional");
            let settings = serde_json::from_value(serde_json::json!({"rotate_while_moving":true,"rotation_mode":mode,"follow_hover":false,"optimize_cursor_types":case["optimizeCursorTypes"].as_bool().unwrap_or(true),"loop_to_start":!case["loopDuration"].is_null(),"loop_duration_ms":case["loopDuration"]})).unwrap();
            let prepared = super::super::runtime::prepare(
                &sidecar,
                (1000, 600),
                None,
                Some(&timeline),
                &sidecar,
                duration,
                &settings,
                spring,
                None,
            )
            .unwrap()
            .unwrap();
            std::fs::remove_file(sidecar).unwrap();
            let keys = prepared.rotation_keys().unwrap();
            let mut target_error = 0.0_f64;
            let mut animated_error = 0.0_f64;
            let mut position_error = 0.0_f64;
            if let Some(samples) = case["positionSamples"].as_array() {
                for expected in samples {
                    let actual = prepared.at(expected["time_ms"].as_f64().unwrap());
                    let error = (actual[0] * 1000.0 - expected["x"].as_f64().unwrap())
                        .hypot(actual[1] * 600.0 - expected["y"].as_f64().unwrap());
                    assert!(error.is_finite());
                    position_error = position_error.max(error);
                }
            }
            for expected in case["samples"].as_array().unwrap() {
                let time = expected["time_ms"].as_f64().unwrap();
                let error = (sample(keys, time) - expected["degrees"].as_f64().unwrap()).abs();
                assert!(error.is_finite());
                target_error = target_error.max(error);
            }
            let mut animation = crate::cursor_renderer::angular_spring::AngularTimeline::for_mode(
                sample(keys, 0.0),
                Some(mode),
            );
            for expected in case["animated"].as_array().unwrap() {
                let time = expected["time_ms"].as_f64().unwrap();
                let actual = animation.sample(time, |t| sample(keys, t));
                let error = (actual - expected["degrees"].as_f64().unwrap()).abs();
                assert!(error.is_finite());
                animated_error = animated_error.max(error);
            }
            if target_error >= 1e-8 || animated_error >= 1e-8 || position_error >= 1e-5 {
                failures.push(format!(
                    "{} {} loop={}: target={target_error}, animated={animated_error}, position={position_error}px",
                    case["scenario"], case["style"], case["loopDuration"]
                ));
            }
        }
        assert!(
            failures.is_empty(),
            "{} of {} cases differ:\n{}",
            failures.len(),
            data["results"].as_array().unwrap().len(),
            failures.join("\n")
        );
    }
}
