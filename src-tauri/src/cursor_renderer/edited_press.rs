//! Preserve the animation clock and velocity across edit boundaries.
use super::{press_animation::PressAnimation, SpringConfig};
use crate::video_processing::{timeline::EditTimeline, CursorSettings};
use std::path::Path;

pub fn build(
    timeline: &EditTimeline,
    recording: &str,
    fps: f64,
    spring: &SpringConfig,
    settings: &CursorSettings,
    output_end_ms: f64,
    pressed: impl Fn(f64) -> bool,
) -> Option<PressAnimation> {
    let canonical = std::fs::canonicalize(recording).ok();
    let sources = timeline
        .segments
        .iter()
        .map(|segment| {
            segment.source_start_ms.filter(|_| {
                segment.source_file_path.as_ref().is_none_or(|path| {
                    Path::new(path) == Path::new(recording)
                        || canonical
                            .as_ref()
                            .is_some_and(|c| std::fs::canonicalize(path).ok().as_ref() == Some(c))
                })
            })
        })
        .collect::<Vec<_>>();
    let cutoff = if settings.stop_at_end.unwrap_or(false) {
        (timeline.duration_ms - settings.stop_duration_ms.unwrap_or(0) as f64).max(0.0)
    } else {
        timeline.duration_ms
    };
    PressAnimation::build_with_config(output_end_ms, fps, |time| {
        let time = time.min(cutoff);
        let index = timeline
            .segments
            .partition_point(|s| s.output_end_ms < time)
            .min(timeline.segments.len().saturating_sub(1));
        let Some(segment) = timeline.segments.get(index) else {
            return (false, *spring);
        };
        let mut config = *spring;
        if segment.disable_cursor_smoothing {
            config.mass = 0.0;
        }
        let active = sources[index].is_some_and(|source| {
            pressed(
                source
                    + (time.clamp(segment.output_start_ms, segment.output_end_ms)
                        - segment.output_start_ms)
                        * segment.rate,
            )
        });
        (active, config)
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::video_processing::timeline::EditClip;

    fn clip(start: f64, duration: f64, source: f64, rate: f64, instant: bool) -> EditClip {
        serde_json::from_value(serde_json::json!({"id":start.to_string(),"startTime":start,
            "duration":duration,"sourceIn":source,"sourceOut":source+duration*rate,
            "playbackRate":rate,"trackOrder":0,"muted":false,"disableCursorSmoothing":instant}))
        .unwrap()
    }

    #[test]
    fn splitting_a_clip_does_not_reset_press_velocity_or_frame_phase() {
        let spring = SpringConfig {
            tension: 470.0,
            friction: 70.0,
            mass: 3.0,
        };
        let settings = serde_json::from_value(serde_json::json!({"enabled":true})).unwrap();
        let timeline = EditTimeline::build(
            &[
                clip(0.0, 337.25, 0.0, 1.0, false),
                clip(337.25, 662.75, 337.25, 1.0, false),
            ],
            0.0,
            1000.0,
        )
        .unwrap();
        for fps in [24.0, 30.0, 60.0, 120.0] {
            let edited = build(&timeline, "video.mp4", fps, &spring, &settings, timeline.duration_ms, |t| {
                (103.0..607.0).contains(&t)
            })
            .unwrap();
            let plain =
                PressAnimation::build(1000.0, fps, &spring, |t| (103.0..607.0).contains(&t))
                    .unwrap();
            for i in 0..2000 {
                let time = i as f64 * 0.5;
                assert_eq!(edited.at(time), plain.at(time));
            }
        }
    }

    #[test]
    fn speed_change_and_instant_slice_change_targets_without_restarting_clock() {
        let spring = SpringConfig {
            tension: 470.0,
            friction: 70.0,
            mass: 3.0,
        };
        let settings = serde_json::from_value(serde_json::json!({"enabled":true})).unwrap();
        let timeline = EditTimeline::build(
            &[
                clip(0.0, 300.0, 0.0, 1.0, false),
                clip(300.0, 300.0, 300.0, 2.0, true),
            ],
            0.0,
            600.0,
        )
        .unwrap();
        let edited = build(&timeline, "video.mp4", 60.0, &spring, &settings, timeline.duration_ms, |t| {
            t < 700.0
        })
        .unwrap();
        assert_eq!(edited.at(450.0), 0.1);
        assert_eq!(edited.at(550.0), 0.0);
    }

    #[test]
    fn bounded_press_animation_keeps_global_cutoff_and_only_samples_needed_frames() {
        let spring = SpringConfig { tension:470.0, friction:70.0, mass:3.0 };
        let timeline = EditTimeline::build(&[
            clip(0.0,337.25,0.0,1.0,false),
            clip(337.25,500.5,1000.0,2.0,true),
            clip(900.0,1100.0,300.0,0.5,false),
        ],0.0,2000.0).unwrap();
        for freeze in [0,700,1900] {
            let settings = serde_json::from_value(serde_json::json!({
                "stop_at_end":true,"stop_duration_ms":freeze
            })).unwrap();
            for fps in [24.0,60.0,120.0] {
                let pressed = |t| (100.0..400.0).contains(&t) || (1200.0..1500.0).contains(&t);
                let full = build(&timeline,"video.mp4",fps,&spring,&settings,timeline.duration_ms,pressed).unwrap();
                for end in [100.25,337.25,800.5,999.9,1500.0] {
                    let calls = std::cell::Cell::new(0);
                    let prefix = build(&timeline,"video.mp4",fps,&spring,&settings,end,|time| {
                        calls.set(calls.get()+1);
                        pressed(time)
                    }).unwrap();
                    assert!(calls.get() <= (end*fps/1000.0).ceil() as usize+2);
                    for sample in 0..=1000 {
                        let time = end*sample as f64/1000.0;
                        assert_eq!(prefix.at(time),full.at(time),"end={end} time={time} fps={fps} freeze={freeze}");
                    }
                }
            }
        }
    }

}
