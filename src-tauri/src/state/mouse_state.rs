//! Mouse tracking and zoom analysis methods for UnifiedAppState
//!
//! Handles mouse event capture and auto-zoom analysis generation.

use anyhow::Result;
use std::sync::Arc;

use super::{save_zoom_sidecar, UnifiedAppState};

impl UnifiedAppState {
    /// Get the mouse tracker instance
    pub fn get_mouse_tracker(
        &self,
    ) -> Arc<parking_lot::Mutex<crate::mouse_tracking::MouseTracker>> {
        static MOUSE_TRACKER: once_cell::sync::Lazy<
            Arc<parking_lot::Mutex<crate::mouse_tracking::MouseTracker>>,
        > = once_cell::sync::Lazy::new(|| {
            Arc::new(parking_lot::Mutex::new(
                crate::mouse_tracking::MouseTracker::new(),
            ))
        });
        Arc::clone(&MOUSE_TRACKER)
    }

    /// Start mouse tracking
    pub async fn start_mouse_tracking(&self) -> Result<()> {
        use crate::mouse_tracking::create_mouse_listener;
        use std::sync::atomic::{AtomicBool, Ordering};
        static LISTENER_STARTED: once_cell::sync::Lazy<AtomicBool> =
            once_cell::sync::Lazy::new(|| AtomicBool::new(false));

        let tracker = self.get_mouse_tracker();
        {
            let mut guard = tracker.lock();
            guard.start_tracking()?;
        }

        if !LISTENER_STARTED.load(Ordering::Acquire) {
            // Spawn global listener thread once
            create_mouse_listener(tracker.clone())?;
            LISTENER_STARTED.store(true, Ordering::Release);
        }
        Ok(())
    }

    /// Stop mouse tracking
    pub async fn stop_mouse_tracking(&self) -> Result<()> {
        let tracker = self.get_mouse_tracker();
        {
            let mut guard = tracker.lock();
            guard.stop_tracking();
        }
        Ok(())
    }

    /// Get all captured mouse events
    pub async fn get_mouse_events(&self) -> Result<Vec<crate::mouse_tracking::MouseEvent>> {
        let tracker = self.get_mouse_tracker();
        let events = {
            let guard = tracker.lock();
            guard.get_events()
        };
        Ok(events)
    }

    /// Get all captured key events
    pub async fn get_key_events(&self) -> Result<Vec<crate::mouse_tracking::KeyEvent>> {
        let tracker = self.get_mouse_tracker();
        let events = {
            let guard = tracker.lock();
            guard.get_key_events()
        };
        Ok(events)
    }

    /// Get mouse tracking statistics
    pub async fn get_mouse_tracking_stats(
        &self,
    ) -> Result<crate::mouse_tracking::MouseTrackingStats> {
        let tracker = self.get_mouse_tracker();
        let stats = {
            let guard = tracker.lock();
            guard.get_stats()
        };
        Ok(stats)
    }

    /// Generate zoom analysis from captured mouse events
    /// This creates .auto_zoom.json and .mouse.json sidecar files for the video
    pub async fn generate_zoom_analysis(&self, video_path: &str) -> Result<()> {
        use crate::auto_zoom::ZoomProcessor;
        use crate::event_capture::{CaptureSession, EnhancedMouseEvent};
        use std::time::{SystemTime, UNIX_EPOCH};

        println!(
            "=== ZOOM_ANALYSIS: Generating zoom analysis for {} ===",
            video_path
        );

        // Get mouse events and key events from tracker
        let events = self.get_mouse_events().await?;
        let key_events = self.get_key_events().await?;

        if events.is_empty() && key_events.is_empty() {
            println!("=== ZOOM_ANALYSIS: No mouse/key events captured, skipping zoom analysis ===");
            return Ok(());
        }

        println!("=== ZOOM_ANALYSIS: Got {} mouse events, {} key events ===", events.len(), key_events.len());

        // Get display resolution, scale factor, and recording area from current config
        let (width, height, scale_factor, recording_area) = self
            .get_recording_info()
            .unwrap_or((1920, 1080, 1.0, None));
        println!(
            "=== ZOOM_ANALYSIS: Using display resolution {}x{}, scale_factor: {} ===",
            width, height, scale_factor
        );

        // Find recording start time (minimum timestamp across mouse + key events) to normalize to 0-based
        let mouse_min = events.iter().map(|e| e.timestamp).min();
        let key_min = key_events.iter().map(|e| e.timestamp).min();
        let start_time = match (mouse_min, key_min) {
            (Some(m), Some(k)) => m.min(k),
            (Some(m), None) => m,
            (None, Some(k)) => k,
            (None, None) => 0,
        };
        println!(
            "=== ZOOM_ANALYSIS: Recording start time: {} ===",
            start_time
        );

        // Create CaptureSession with mouse events
        let mut session = CaptureSession::new();
        session.metadata.display_resolution = (width, height);
        session.start_time = 0; // Normalized to 0

        // Convert MouseEvent to EnhancedMouseEvent with normalized timestamps (0-based)
        session.mouse_events = events
            .into_iter()
            .map(|e| {
                let mut normalized = e.clone();
                // Normalize timestamp to be relative to recording start (video timeline compatible)
                normalized.timestamp = e.timestamp.saturating_sub(start_time);
                EnhancedMouseEvent {
                    base: normalized,
                    window_id: None,
                    app_name: None,
                    is_double_click: false,
                    cluster_id: None,
                }
            })
            .collect();

        // Set end time to now
        session.end_time = Some(
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_millis() as u64,
        );

        // Normalize key event timestamps to 0-based (same as mouse events)
        let normalized_key_events: Vec<crate::mouse_tracking::KeyEvent> = key_events
            .into_iter()
            .map(|mut k| {
                k.timestamp = k.timestamp.saturating_sub(start_time);
                k
            })
            .collect();

        println!(
            "=== ZOOM_ANALYSIS: Created CaptureSession with {} mouse events, {} key events ===",
            session.mouse_events.len(),
            normalized_key_events.len()
        );

        // Run zoom analysis
        let processor = ZoomProcessor::with_default_config();
        let analysis = processor.analyze_session(&session, &normalized_key_events)?;

        println!(
            "=== ZOOM_ANALYSIS: {} clicks → {} zoom blocks ===",
            analysis.total_clicks,
            analysis.zoom_blocks.len()
        );

        // Save zoom analysis and mouse events to sidecar files using persistence helper
        save_zoom_sidecar(
            video_path,
            &analysis,
            &session.mouse_events,
            (width, height, scale_factor, recording_area),
        )?;

        println!("=== ZOOM_ANALYSIS: Complete! ===");
        Ok(())
    }

    /// Get recording resolution and scale factor from current config
    /// Returns (width, height, scale_factor, recording_area)
    pub(crate) fn get_recording_info(
        &self,
    ) -> Option<(
        u32,
        u32,
        f32,
        Option<crate::recording::types::RecordingArea>,
    )> {
        if let Some(config) = self.recording.get_current_config() {
            match &config.target {
                crate::recording::types::RecordingTarget::Desktop { display_id, area } => {
                    // Look up display resolution and scale factor from app state
                    let app = self.app.read();
                    if let Some(display) =
                        app.displays.iter().find(|d| d.id == display_id.to_string())
                    {
                        return Some((
                            display.width,
                            display.height,
                            display.scale_factor,
                            area.clone(),
                        ));
                    }
                }
                _ => {}
            }
        }
        None
    }
}
