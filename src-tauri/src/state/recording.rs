use anyhow::Result;
use parking_lot::RwLock;
use serde::Serialize;
use std::sync::Arc;
use tokio::sync::Mutex;

use crate::recording::types::RecordingConfig;
use crate::recording::{RecordingAPI, RecordingState as CoreRecordingState};

/// Recording state management for the application
///
/// This module provides a clean interface between the UI and the recording engine,
/// ensuring instant responses and proper state management.
pub struct RecordingStateManager {
    /// The core recording API
    recording_api: Arc<Mutex<RecordingAPI>>,

    /// Current recording state (cached for quick access)
    cached_state: Arc<RwLock<CoreRecordingState>>,

    /// Current recording configuration
    current_config: Arc<RwLock<Option<RecordingConfig>>>,
}
impl RecordingStateManager {
    /// Create new recording state manager
    pub fn new() -> Result<Self> {
        let recording_api = RecordingAPI::new()?;

        Ok(Self {
            recording_api: Arc::new(Mutex::new(recording_api)),
            cached_state: Arc::new(RwLock::new(CoreRecordingState::Idle)),
            current_config: Arc::new(RwLock::new(None)),
        })
    }

    /// Start recording with the given configuration
    /// Returns immediately after starting the recording process
    pub async fn start_recording(&self, config: RecordingConfig) -> Result<()> {
        // Store configuration
        {
            let mut current_config = self.current_config.write();
            *current_config = Some(config.clone());
        }

        // Start recording through API
        {
            let mut api = self.recording_api.lock().await;
            if let Err(err) = api.start_recording(config).await {
                let mut current_config = self.current_config.write();
                *current_config = None;
                return Err(err);
            }
        }

        // Update cached state after the backend confirms capture started.
        {
            let mut cached = self.cached_state.write();
            *cached = CoreRecordingState::Recording;
        }

        // Start tray timer for UI updates
        // NOTE: Disabled - tray timer is now handled by main.rs with proper AppHandle access
        // self.start_tray_timer().await;

        println!("Recording started through state manager");
        Ok(())
    }

    /// Signal recording to stop and return temp path immediately
    /// Background processing continues after this returns
    pub async fn signal_stop_recording(&self) -> Result<String> {
        // Signal stop through API
        let temp_path = {
            let mut api = self.recording_api.lock().await;
            api.signal_stop().await?
        };

        // Update cached state
        {
            let mut cached = self.cached_state.write();
            *cached = CoreRecordingState::Stopping;
        }

        // Stop tray timer
        // NOTE: Disabled - tray timer is now handled by main.rs
        // self.stop_tray_timer().await;

        println!("Recording stop signaled through state manager");
        Ok(temp_path)
    }

    /// Wait for recording completion and get final path
    /// This should be called in the background after signal_stop_recording
    pub async fn wait_for_completion(&self) -> Result<String> {
        let final_path = {
            let mut api = self.recording_api.lock().await;
            api.wait_for_completion().await?
        };

        // Update cached state
        {
            let mut cached = self.cached_state.write();
            *cached = CoreRecordingState::Completed;
        }

        // Clear current config
        {
            let mut current_config = self.current_config.write();
            *current_config = None;
        }

        println!("Recording completed through state manager");
        Ok(final_path)
    }

    /// Pause current recording
    pub async fn pause_recording(&self) -> Result<()> {
        let mut api = self.recording_api.lock().await;
        api.pause().await?;

        // Update cached state
        self.update_cached_state().await;

        println!("Recording paused through state manager");
        Ok(())
    }

    /// Resume current recording
    pub async fn resume_recording(&self) -> Result<()> {
        let mut api = self.recording_api.lock().await;
        api.resume().await?;

        // Update cached state
        self.update_cached_state().await;

        println!("Recording resumed through state manager");
        Ok(())
    }

    /// Check if recording is active (fast check via cache)
    pub fn is_recording(&self) -> bool {
        let cached = self.cached_state.read();
        matches!(
            *cached,
            CoreRecordingState::Recording
                | CoreRecordingState::Paused
                | CoreRecordingState::Stopping
        )
    }

    /// Get current recording configuration
    pub fn get_current_config(&self) -> Option<RecordingConfig> {
        let config = self.current_config.read();
        config.clone()
    }

    /// Update cached state from API
    async fn update_cached_state(&self) {
        let api = self.recording_api.lock().await;
        let state = api.get_state();

        let mut cached = self.cached_state.write();
        *cached = state;
    }

    /// Get recording statistics and information
    pub async fn get_recording_info(&self) -> Result<RecordingInfo> {
        let config = self.get_current_config();

        Ok(RecordingInfo { config })
    }
}

/// Recording information for UI display
#[derive(Debug, Clone, Serialize)]
pub struct RecordingInfo {
    pub config: Option<RecordingConfig>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::recording::types::*;

    #[tokio::test]
    async fn test_recording_state_manager_creation() {
        let manager = RecordingStateManager::new();
        assert!(
            manager.is_ok(),
            "Should be able to create recording state manager"
        );

        let manager = manager.unwrap();
        assert!(!manager.is_recording(), "Should not be recording initially");

        assert!(matches!(
            *manager.cached_state.read(),
            CoreRecordingState::Idle
        ));
    }

    #[tokio::test]
    async fn test_recording_config_access() {
        let manager = RecordingStateManager::new().unwrap();

        let config = RecordingConfig {
            target: RecordingTarget::Desktop {
                display_id: 0,
                area: None,
            },
            quality: QualityPreset::High,
            output_path: "/tmp/test.mp4".to_string(),
            ..Default::default()
        };

        // Keep this unit test independent of live ScreenCaptureKit hardware.
        // start_recording intentionally clears the config if backend startup fails.
        *manager.current_config.write() = Some(config);

        let stored_config = manager.get_current_config();
        assert!(stored_config.is_some(), "Should have stored config");

        let stored = stored_config.unwrap();
        assert_eq!(stored.output_path, "/tmp/test.mp4");
        assert!(matches!(stored.quality, QualityPreset::High));
    }
}
