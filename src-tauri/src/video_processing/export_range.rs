use super::types::ExportSettings;
use anyhow::{Result, bail};

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ExportRange {
    pub start_ms: u64,
    pub end_ms: u64,
}
impl ExportRange {
    pub fn resolve(settings: &ExportSettings, source_duration_ms: u64) -> Result<Self> {
        let start_ms = settings.trim_start.unwrap_or(0);
        let end_ms = settings
            .trim_end
            .unwrap_or(source_duration_ms)
            .min(source_duration_ms);
        if start_ms >= end_ms {
            bail!("Export trim must select a nonempty source range");
        }
        Ok(Self { start_ms, end_ms })
    }
    pub fn duration_ms(self) -> u64 {
        self.end_ms - self.start_ms
    }
    pub fn source_sample(self, output_frame: usize, output_fps: u32, simulation_fps: f64) -> usize {
        let time_ms = self.start_ms as f64 + output_frame as f64 * 1000.0 / output_fps as f64;
        (time_ms * simulation_fps / 1000.0 + 1e-7).floor() as usize
    }
}

/// Number of output samples in [0, duration), including a final partial frame.
/// Preserve fractional timeline durations instead of rounding milliseconds first.
pub(crate) fn output_frame_count(duration_ms:f64,fps:u32)->u64 {
    (duration_ms*f64::from(fps)/1000.0-1e-7).ceil().max(1.0) as u64
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn output_samples_respect_exclusive_end_and_fractional_timeline_durations() {
        for fps in [24,30,60,120] {
            assert_eq!(output_frame_count(4000.0,fps),4*u64::from(fps));
            assert_eq!(output_frame_count(1000.0/f64::from(fps),fps),1);
            assert_eq!(output_frame_count(1000.0/f64::from(fps)+0.1,fps),2);
            assert_eq!(output_frame_count(0.5,fps),1);
        }
    }
    #[test]
    fn resolves_partial_ranges_and_rejects_reversed_or_empty_ranges() {
        let settings = |value| serde_json::from_value::<ExportSettings>(value).unwrap();
        assert_eq!(
            ExportRange::resolve(&settings(serde_json::json!({"trim_end":500})), 2000).unwrap(),
            ExportRange {
                start_ms: 0,
                end_ms: 500
            }
        );
        assert_eq!(
            ExportRange::resolve(&settings(serde_json::json!({"trim_start":500})), 2000)
                .unwrap()
                .duration_ms(),
            1500
        );
        for (start, end) in [(1500, 500), (2000, 2000), (3000, 4000)] {
            assert!(
                ExportRange::resolve(
                    &settings(serde_json::json!({"trim_start":start,"trim_end":end})),
                    2000
                )
                .is_err()
            );
        }
    }
    #[test]
    fn maps_trimmed_output_to_source_motion_at_all_frame_rates() {
        let range = ExportRange {
            start_ms: 1250,
            end_ms: 2250,
        };
        for fps in [24, 30, 60, 120] {
            assert_eq!(range.source_sample(0, fps, 120.0), 150);
            assert_eq!(range.source_sample(fps as usize / 2, fps, 120.0), 210);
        }
    }
}
