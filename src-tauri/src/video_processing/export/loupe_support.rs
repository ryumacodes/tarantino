use super::*;
use crate::video_processing::loupe::{self, LoupeTarget};

pub(super) fn prepare(
    blocks: &[ZoomBlock],
    settings: &ExportSettings,
    activity: &crate::video_processing::camera_activity::Tracks,
    source: (u32, u32),
    duration_ms: u64,
) -> Result<Option<Vec<LoupeTarget>>> {
    let styles: Vec<_> = blocks
        .iter()
        .filter(|block| !block.is_disabled)
        .filter_map(|block| block.loupe)
        .collect();
    if styles.is_empty() {
        return Ok(None);
    }
    Ok(Some(loupe::prepare::trajectory_with_settings(
        blocks,
        &[],
        source,
        duration_ms,
        CAMERA_SIMULATION_FPS,
        get_zoom_spring_config(settings),
        settings,
     Some(activity))?))
}
