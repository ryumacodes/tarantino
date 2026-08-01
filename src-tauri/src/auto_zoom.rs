use crate::event_capture::{CaptureSession, EnhancedMouseEvent};
use crate::input::KeyEvent;
use anyhow::Result;
use serde::{Deserialize, Serialize};
mod typing_zoom;

pub use typing_zoom::validate_zoom_blocks;
use typing_zoom::{TypingZoomConfig, create_typing_zoom_blocks, detect_typing_sessions};

/// Click-based zoom configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ZoomConfig {
    pub enabled: bool,
    pub zoom_factor: f32,
    pub zoom_duration: u64,     // Default: 5000ms (5s: 1s in, 3s hold, 1s out)
    pub min_click_spacing: u64, // Minimum time between click zooms (500ms)
}

impl Default for ZoomConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            zoom_factor: 2.0,
            zoom_duration: 5000, // 5 seconds total (1s zoom in, 3s hold, 1s zoom out)
            min_click_spacing: 500,
        }
    }
}

/// A re-center point within a zoom block (when user clicks a new location while zoomed)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ZoomCenter {
    pub x: f64,    // Normalized center X (0-1)
    pub y: f64,    // Normalized center Y (0-1)
    pub time: u64, // When to start panning to this center (ms)
}

/// Click-based zoom block.
/// Consecutive clicks merge into one block with multiple centers.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ZoomBlock {
    pub id: String,
    pub click_x: f64,     // First click position X
    pub click_y: f64,     // First click position Y
    pub center_x: f64,    // Current/initial zoom center X
    pub center_y: f64,    // Current/initial zoom center Y
    pub start_time: u64,  // When zoom starts
    pub end_time: u64,    // When zoom ends
    pub zoom_factor: f32, // Zoom level (default from config)
    pub is_manual: bool,  // True if user manually adjusted the zoom area
    #[serde(default)]
    pub centers: Vec<ZoomCenter>, // Re-center points from merged clicks
    #[serde(default = "default_kind")]
    pub kind: String, // "click" or "typing"
    #[serde(default)]
    pub zoom_in_speed: Option<String>,
    #[serde(default)]
    pub zoom_out_speed: Option<String>,
    #[serde(default)]
    pub timing_adjusted: bool,
}

fn default_kind() -> String {
    "click".into()
}

/// Zoom analysis result.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ZoomAnalysis {
    pub zoom_blocks: Vec<ZoomBlock>,
    pub total_clicks: usize,
    pub session_duration: u64,
    pub config: ZoomConfig,
}

/// A detected typing session (consecutive keystrokes with small gaps)
#[derive(Debug, Clone)]
/// Click-based zoom processor.
pub struct ZoomProcessor {
    config: ZoomConfig,
}

impl ZoomProcessor {
    pub fn new(config: ZoomConfig) -> Self {
        Self { config }
    }

    pub fn with_default_config() -> Self {
        Self::new(ZoomConfig::default())
    }

    /// Analyze capture session and create zoom blocks for clicks and typing
    pub fn analyze_session(
        &self,
        session: &CaptureSession,
        key_events: &[KeyEvent],
    ) -> Result<ZoomAnalysis> {
        println!("🔍 [ZOOM] analyze_session called");
        println!("🔍 [ZOOM] Session ID: {}", session.session_id);
        println!(
            "🔍 [ZOOM] Total mouse events in session: {}",
            session.mouse_events.len()
        );
        println!("🔍 [ZOOM] Total key events: {}", key_events.len());
        println!("🔍 [ZOOM] Session start_time: {}", session.start_time);
        println!("🔍 [ZOOM] Session end_time: {:?}", session.end_time);
        println!(
            "🔍 [ZOOM] Display resolution: {:?}",
            session.metadata.display_resolution
        );

        if !self.config.enabled {
            println!("⚠️ [ZOOM] Zoom creation disabled in config");
            return Ok(ZoomAnalysis {
                zoom_blocks: vec![],
                total_clicks: 0,
                session_duration: session.end_time.unwrap_or(session.start_time)
                    - session.start_time,
                config: self.config.clone(),
            });
        }

        // Extract mouse click events (button presses only)
        let click_events = self.extract_click_events(session);
        println!(
            "🖱️ [ZOOM] Extracted {} click events (ButtonPress only)",
            click_events.len()
        );

        // Log first few clicks for debugging
        for (i, event) in click_events.iter().take(5).enumerate() {
            println!(
                "   Click {}: time={}ms, pos=({:.1}, {:.1})",
                i, event.base.timestamp, event.base.x, event.base.y
            );
        }
        if click_events.len() > 5 {
            println!("   ... and {} more clicks", click_events.len() - 5);
        }

        // Create zoom blocks for clicks.
        let mut zoom_blocks = self.create_zoom_blocks(&click_events, session)?;
        println!(
            "✅ [ZOOM] Created {} zoom blocks from {} clicks",
            zoom_blocks.len(),
            click_events.len()
        );

        // Detect typing sessions and create additional zoom blocks
        let typing_config = TypingZoomConfig::default();
        let typing_sessions = detect_typing_sessions(
            key_events,
            &session.mouse_events,
            &session.metadata,
            &typing_config,
        );
        if !typing_sessions.is_empty() {
            let session_duration =
                session.end_time.unwrap_or(session.start_time) - session.start_time;
            let typing_blocks = create_typing_zoom_blocks(
                &typing_sessions,
                &typing_config,
                session_duration,
                zoom_blocks.len(),
            );
            println!(
                "⌨️ [ZOOM] Created {} typing zoom blocks from {} typing sessions",
                typing_blocks.len(),
                typing_sessions.len()
            );
            zoom_blocks.extend(typing_blocks);
        }

        let session_duration = session.end_time.unwrap_or(session.start_time) - session.start_time;
        // Re-validate after merging click + typing blocks (handles overlaps)
        validate_zoom_blocks(&mut zoom_blocks, session_duration);
        println!(
            "📊 [ZOOM] Final: {} zoom blocks, session duration: {}ms",
            zoom_blocks.len(),
            session_duration
        );

        Ok(ZoomAnalysis {
            zoom_blocks,
            total_clicks: click_events.len(),
            session_duration,
            config: self.config.clone(),
        })
    }

    /// Extract click events from enhanced mouse events (button presses only)
    fn extract_click_events(&self, session: &CaptureSession) -> Vec<EnhancedMouseEvent> {
        session
            .mouse_events
            .iter()
            .filter(|event| {
                matches!(
                    event.base.event_type,
                    crate::input::MouseEventType::ButtonPress { .. }
                )
            })
            .cloned()
            .collect()
    }

    /// Create zoom blocks for clicks.
    /// Consecutive clicks that fall within an active zoom are MERGED into one
    /// continuous block with multiple re-center points — no zoom-out between them.
    fn create_zoom_blocks(
        &self,
        click_events: &[EnhancedMouseEvent],
        session: &CaptureSession,
    ) -> Result<Vec<ZoomBlock>> {
        let mut zoom_blocks: Vec<ZoomBlock> = Vec::new();
        let mut last_zoom_time = 0u64;

        // Timing constants
        let zoom_in_duration = 1000u64; // 1 second to zoom in before click
        let hold_duration = 3000u64; // 3 seconds hold at peak after click
        let session_duration = session.end_time.unwrap_or(session.start_time) - session.start_time;

        for (i, event) in click_events.iter().enumerate() {
            // Skip clicks that are too close in time to the previous zoom
            if event.base.timestamp.saturating_sub(last_zoom_time) < self.config.min_click_spacing {
                continue;
            }

            let (norm_x, norm_y) =
                self.normalize_coordinates(event.base.x, event.base.y, &session.metadata);

            // Check if this click falls within (or near) the previous block's active range.
            // If so, merge it into that block instead of creating a new one.
            let merged = if let Some(prev_block) = zoom_blocks.last_mut() {
                if event.base.timestamp <= prev_block.end_time + 500 {
                    // Merge: add a re-center point and extend the block
                    prev_block.centers.push(ZoomCenter {
                        x: norm_x,
                        y: norm_y,
                        time: event.base.timestamp,
                    });
                    // Extend end_time from this click
                    let new_end = (event.base.timestamp + hold_duration).min(session_duration);
                    prev_block.end_time = new_end;
                    true
                } else {
                    false
                }
            } else {
                false
            };

            if !merged {
                let start_time = event.base.timestamp.saturating_sub(zoom_in_duration);
                let end_time = (event.base.timestamp + hold_duration).min(session_duration);

                if end_time <= start_time + zoom_in_duration + 500 {
                    continue;
                }

                zoom_blocks.push(ZoomBlock {
                    id: format!("zoom_{}", i),
                    click_x: norm_x,
                    click_y: norm_y,
                    center_x: norm_x,
                    center_y: norm_y,
                    start_time,
                    end_time,
                    zoom_factor: self.config.zoom_factor,
                    is_manual: false,
                    centers: vec![ZoomCenter {
                        x: norm_x,
                        y: norm_y,
                        time: event.base.timestamp,
                    }],
                    kind: "click".to_string(),
                    zoom_in_speed: None,
                    zoom_out_speed: None,
                    timing_adjusted: false,
                });
            }

            last_zoom_time = event.base.timestamp;
        }

        validate_zoom_blocks(&mut zoom_blocks, session_duration);
        Ok(zoom_blocks)
    }

    /// Normalize screen coordinates to 0-1 range
    /// Uses capture_region for partial recordings to match cursor coordinate system
    fn normalize_coordinates(
        &self,
        x: f64,
        y: f64,
        metadata: &crate::event_capture::SessionMetadata,
    ) -> (f64, f64) {
        // Use capture_region for partial recordings, fall back to full display
        let (eff_x, eff_y, eff_w, eff_h) = if let Some((rx, ry, rw, rh)) = metadata.capture_region {
            (rx as f64, ry as f64, rw as f64, rh as f64)
        } else {
            (
                0.0,
                0.0,
                metadata.display_resolution.0 as f64,
                metadata.display_resolution.1 as f64,
            )
        };

        // Adjust coordinates relative to recording area (matches cursor normalization in cursor_renderer.rs)
        let adjusted_x = x - eff_x;
        let adjusted_y = y - eff_y;
        let norm_x = (adjusted_x / eff_w).clamp(0.0, 1.0);
        let norm_y = (adjusted_y / eff_h).clamp(0.0, 1.0);

        (norm_x, norm_y)
    }
}

/// Save zoom analysis to file
pub fn save_analysis(analysis: &ZoomAnalysis, path: impl AsRef<std::path::Path>) -> Result<()> {
    let path = path.as_ref();
    let json = serde_json::to_string_pretty(analysis)?;
    std::fs::write(path, json)?;
    println!("Zoom analysis saved to: {}", path.display());
    Ok(())
}

/// Load zoom analysis from file
pub fn load_analysis(path: impl AsRef<std::path::Path>) -> Result<ZoomAnalysis> {
    let json = std::fs::read_to_string(path)?;
    let analysis: ZoomAnalysis = serde_json::from_str(&json)?;
    Ok(analysis)
}

#[cfg(test)]
mod tests;
