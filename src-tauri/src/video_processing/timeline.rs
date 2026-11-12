//! Timeline-to-source mapping shared by decoded media and motion sampling.
use anyhow::{Result, bail};
use serde::{Deserialize, Serialize};

fn default_volume() -> f64 { 1.0 }

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EditClip {
    pub id: String,
    pub start_time: f64,
    pub duration: f64,
    pub source_in: f64,
    pub source_out: f64,
    pub playback_rate: f64,
    pub source_file_path: Option<String>,
    pub track_order: i32,
    pub muted: bool,
    #[serde(default = "default_volume")]
    pub mic_volume: f64,
    #[serde(default = "default_volume")]
    pub system_volume: f64,
    #[serde(default)]
    pub hide_cursor: bool,
    #[serde(default)]
    pub disable_cursor_smoothing: bool,
}
#[derive(Clone, Debug, PartialEq)]
pub struct EditSegment {
    pub output_start_ms: f64,
    pub output_end_ms: f64,
    pub source_start_ms: Option<f64>,
    pub source_end_ms: Option<f64>,
    pub source_file_path: Option<String>,
    pub rate: f64,
    pub muted: bool,
    pub mic_volume: f64,
    pub system_volume: f64,
    pub hide_cursor: bool,
    pub disable_cursor_smoothing: bool,
}
#[derive(Clone, Debug)]
pub struct EditTimeline {
    pub segments: Vec<EditSegment>,
    pub duration_ms: f64,
    pub starts_after_cut: bool,
}
impl EditTimeline {
    pub fn build(clips: &[EditClip], trim_start_ms: f64, trim_end_ms: f64) -> Result<Self> {
        if !trim_start_ms.is_finite()
            || !trim_end_ms.is_finite()
            || trim_start_ms < 0.0
            || trim_end_ms <= trim_start_ms
        {
            bail!("Invalid timeline range");
        }
        let mut boundaries = vec![trim_start_ms, trim_end_ms];
        for clip in clips {
            if ![
                clip.start_time,
                clip.duration,
                clip.source_in,
                clip.source_out,
                clip.playback_rate,
            ]
            .iter()
            .all(|n| n.is_finite())
                || clip.start_time < 0.0
                || clip.duration <= 0.0
                || clip.source_in < 0.0
                || clip.source_out <= clip.source_in
                || !(0.25..=24.0).contains(&clip.playback_rate)
            {
                bail!("Invalid timing for clip {}", clip.id);
            }
            if (clip.duration * clip.playback_rate - (clip.source_out - clip.source_in)).abs() > 1.0
            {
                bail!(
                    "Clip {} has inconsistent source range and playback speed",
                    clip.id
                );
            }
            if !clip.mic_volume.is_finite() || !clip.system_volume.is_finite()
                || !(0.0..=1.0).contains(&clip.mic_volume) || !(0.0..=1.0).contains(&clip.system_volume) {
                bail!("Invalid audio volume for clip {}", clip.id);
            }
            boundaries.push(clip.start_time.clamp(trim_start_ms, trim_end_ms));
            boundaries.push((clip.start_time + clip.duration).clamp(trim_start_ms, trim_end_ms));
        }
        boundaries.sort_by(f64::total_cmp);
        boundaries.dedup();
        let mut segments = Vec::new();
        for interval in boundaries.windows(2) {
            let (start, end) = (interval[0], interval[1]);
            // Lower track order is visually above higher track order. Input order
            // breaks ties deterministically for overlapping clips on one track.
            let active = clips
                .iter()
                .enumerate()
                .filter(|(_, c)| c.start_time <= start && c.start_time + c.duration > start)
                .min_by_key(|(index, c)| (c.track_order, *index))
                .map(|(_, c)| c);
            segments.push(EditSegment {
                output_start_ms: start - trim_start_ms,
                output_end_ms: end - trim_start_ms,
                source_start_ms: active
                    .map(|c| c.source_in + (start - c.start_time) * c.playback_rate),
                source_end_ms: active
                    .map(|c| c.source_in + (end - c.start_time) * c.playback_rate),
                source_file_path: active.and_then(|c| c.source_file_path.clone()),
                rate: active.map_or(1.0, |c| c.playback_rate),
                mic_volume: active.map_or(1.0, |c| c.mic_volume),
                system_volume: active.map_or(1.0, |c| c.system_volume),
                muted: active.is_none_or(|c| c.muted),
                hide_cursor: active.is_none_or(|c| c.hide_cursor),
                disable_cursor_smoothing: active.is_some_and(|c| c.disable_cursor_smoothing),
            });
        }
        let preceding = clips.iter().enumerate()
            .filter(|(_, clip)| clip.start_time < trim_start_ms
                && clip.start_time + clip.duration >= trim_start_ms)
            .min_by_key(|(index, clip)| (clip.track_order, *index))
            .map(|(_, clip)| clip);
        let starts_after_cut = preceding.zip(segments.first()).is_some_and(|(clip, first)| {
            first.source_start_ms.is_some_and(|source| {
                let previous = clip.source_in + (trim_start_ms - clip.start_time) * clip.playback_rate;
                (previous - source).abs() > 1e-7 || clip.source_file_path != first.source_file_path
            })
        });
        Ok(Self {
            starts_after_cut,
            segments,
            duration_ms: trim_end_ms - trim_start_ms,
        })
    }
    pub fn source_at(&self, output_ms: f64) -> Option<(&EditSegment, f64)> {
        if !output_ms.is_finite() || output_ms < 0.0 || output_ms >= self.duration_ms {
            return None;
        }
        let index = self
            .segments
            .partition_point(|s| s.output_end_ms <= output_ms);
        let segment = self.segments.get(index)?;
        let source =
            segment.source_start_ms? + (output_ms - segment.output_start_ms) * segment.rate;
        Some((segment, source))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn clip(start: f64, source: f64, length: f64, rate: f64) -> EditClip {
        EditClip {
            id: format!("{start}"),
            start_time: start,
            duration: length,
            source_in: source,
            source_out: source + length * rate,
            playback_rate: rate,
            source_file_path: None,
            track_order: 0,
            mic_volume: 1.0, system_volume: 1.0,
            muted: false,
            hide_cursor: false, disable_cursor_smoothing: false,
        }
    }
    #[test]
    fn cursor_visibility_follows_the_selected_clip_after_trim_and_speed_edits() {
        let mut hidden = clip(0.0, 4000.0, 1000.0, 2.0);
        hidden.hide_cursor = true;
        let plan = EditTimeline::build(&[hidden, clip(1000.0, 0.0, 1000.0, 0.5)], 500.0, 1800.0).unwrap();
        let (segment, source) = plan.source_at(499.0).unwrap();
        assert!(segment.hide_cursor);
        assert_eq!(source, 5998.0);
        let (segment, source) = plan.source_at(500.0).unwrap();
        assert!(!segment.hide_cursor);
        assert_eq!(source, 0.0);
        let mut legacy = serde_json::to_value(clip(0.0, 0.0, 1000.0, 1.0)).unwrap();
        legacy.as_object_mut().unwrap().remove("hideCursor");
        assert!(!serde_json::from_value::<EditClip>(legacy).unwrap().hide_cursor);
    }

    #[test]
    fn clip_audio_defaults_to_unity_and_rejects_invalid_gains() {
        let mut legacy = serde_json::to_value(clip(0.0, 0.0, 1000.0, 1.0)).unwrap();
        legacy.as_object_mut().unwrap().remove("micVolume");
        legacy.as_object_mut().unwrap().remove("systemVolume");
        let restored: EditClip = serde_json::from_value(legacy).unwrap();
        assert_eq!((restored.mic_volume, restored.system_volume), (1.0, 1.0));
        for invalid in [f64::NAN, f64::INFINITY, -0.1, 1.1] {
            let mut changed = restored.clone();
            changed.mic_volume = invalid;
            assert!(EditTimeline::build(&[changed], 0.0, 1000.0).is_err());
            let mut changed = restored.clone();
            changed.system_volume = invalid;
            assert!(EditTimeline::build(&[changed], 0.0, 1000.0).is_err());
        }
    }

    #[test]
    fn maps_reordered_fast_and_slow_clips_with_a_deleted_middle_gap() {
        let plan = EditTimeline::build(
            &[
                clip(0.0, 4000.0, 1000.0, 2.0),
                clip(1500.0, 0.0, 2000.0, 0.5),
            ],
            250.0,
            3000.0,
        )
        .unwrap();
        assert_eq!(plan.duration_ms, 2750.0);
        assert_eq!(plan.source_at(0.0).unwrap().1, 4500.0);
        assert_eq!(plan.source_at(500.0).unwrap().1, 5500.0);
        assert!(plan.source_at(750.0).is_none());
        assert!(plan.source_at(1249.0).is_none());
        assert_eq!(plan.source_at(1250.0).unwrap().1, 0.0);
        assert_eq!(plan.source_at(2250.0).unwrap().1, 500.0);
        assert!(plan.source_at(2750.0).is_none());
    }
    #[test]
    fn resolves_overlaps_by_track_order_and_preserves_mute() {
        let mut bottom = clip(0.0, 0.0, 2000.0, 1.0);
        bottom.track_order = 1;
        let mut top = clip(500.0, 5000.0, 500.0, 1.0);
        top.muted = true;
        let plan = EditTimeline::build(&[bottom, top], 0.0, 2000.0).unwrap();
        assert_eq!(plan.source_at(600.0).unwrap().1, 5100.0);
        assert!(plan.source_at(600.0).unwrap().0.muted);
        assert_eq!(plan.source_at(1000.0).unwrap().1, 1000.0);
    }
    #[test]
    fn rejects_inconsistent_or_nonfinite_clip_timing() {
        let mut invalid = clip(0.0, 0.0, 1000.0, 2.0);
        invalid.source_out = 1000.0;
        assert!(EditTimeline::build(&[invalid], 0.0, 2000.0).is_err());
        assert!(EditTimeline::build(&[clip(0.0, 0.0, 1000.0, f64::NAN)], 0.0, 2000.0).is_err());
        assert!(EditTimeline::build(&[], 2000.0, 1000.0).is_err());
    }
}
