use anyhow::Result;
use parking_lot::RwLock;
use serde::Serialize;
use std::sync::Arc;
use std::time::Instant;
use tokio::sync::Mutex;

use crate::recording::{RecordingAPI, RecordingState as CoreRecordingState};
use crate::recording::types::RecordingConfig;

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
    
    /// Tray timer handle for UI updates
    tray_timer_handle: Arc<Mutex<Option<tokio::task::JoinHandle<()>>>>,
}

#[allow(dead_code)]
impl RecordingStateManager {
    /// Create new recording state manager
    pub fn new() -> Result<Self> {
        let recording_api = RecordingAPI::new()?;
        
        Ok(Self {
            recording_api: Arc::new(Mutex::new(recording_api)),
            cached_state: Arc::new(RwLock::new(CoreRecordingState::Idle)),
            current_config: Arc::new(RwLock::new(None)),
            tray_timer_handle: Arc::new(Mutex::new(None)),
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
            api.start_recording(config).await?;
        }
        
        // Update cached state
        {
            let mut cached = self.cached_state.write();
            *cached = CoreRecordingState::Starting;
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
            *cached = CoreRecordingState::Stopping {
                temp_path: temp_path.clone(),
            };
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
            *cached = CoreRecordingState::Completed {
                final_path: final_path.clone(),
            };
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
    
    /// Get current recording state (fast access via cache)
    pub fn get_cached_state(&self) -> CoreRecordingState {
        let cached = self.cached_state.read();
        cached.clone()
    }
    
    /// Get current recording state from API (slower but authoritative)
    pub async fn get_current_state(&self) -> CoreRecordingState {
        let api = self.recording_api.lock().await;
        let state = api.get_state();

        // Update cache
        {
            let mut cached = self.cached_state.write();
            *cached = state.clone();
        }

        state
    }
    
    /// Check if recording is active (fast check via cache)
    pub fn is_recording(&self) -> bool {
        let cached = self.cached_state.read();
        matches!(*cached, CoreRecordingState::Recording { .. } | 
                          CoreRecordingState::Paused { .. })
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
    
    /// Start tray timer for recording duration updates
    async fn start_tray_timer(&self) {
        let cached_state = Arc::clone(&self.cached_state);
        
        let handle = tokio::spawn(async move {
            let mut last_update = Instant::now();
            
            while last_update.elapsed() < std::time::Duration::from_secs(8 * 60 * 60) { // Max 8 hour recording
                tokio::time::sleep(std::time::Duration::from_secs(1)).await;
                
                // Check if still recording
                let state = {
                    let cached = cached_state.read();
                    cached.clone()
                };
                
                match state {
                    CoreRecordingState::Recording => {
                        println!("Recording in progress...");
                    },
                    CoreRecordingState::Paused => {
                        println!("Recording paused");
                    },
                    _ => {
                        // Recording stopped, exit timer
                        break;
                    }
                }
                
                last_update = Instant::now();
            }
            
            println!("Tray timer ended");
        });
        
        let mut timer_handle = self.tray_timer_handle.lock().await;
        *timer_handle = Some(handle);
    }
    
    /// Stop tray timer
    async fn stop_tray_timer(&self) {
        let mut timer_handle = self.tray_timer_handle.lock().await;
        if let Some(handle) = timer_handle.take() {
            handle.abort();
            println!("Tray timer stopped");
        }
    }
    
    /// Get recording statistics and information
    pub async fn get_recording_info(&self) -> Result<RecordingInfo> {
        let state = self.get_cached_state();
        let config = self.get_current_config();
        
        Ok(RecordingInfo {
            state,
            config,
            started_at: self.get_recording_start_time(),
            duration: self.get_recording_duration(),
        })
    }
    
    /// Get recording start time
    fn get_recording_start_time(&self) -> Option<Instant> {
        // Simplified: we don't carry start timestamps in RecordingState
        None
    }

    /// Get current recording duration
    fn get_recording_duration(&self) -> Option<std::time::Duration> {
        None
    }
}

/// Recording information for UI display
#[derive(Debug, Clone, Serialize)]
#[allow(dead_code)]
pub struct RecordingInfo {
    #[serde(skip_serializing)]
    pub state: CoreRecordingState,
    pub config: Option<RecordingConfig>,
    #[allow(dead_code)] // Reserved for future use - duration tracking feature
    #[serde(skip_serializing)]
    pub started_at: Option<Instant>,
    #[allow(dead_code)] // Reserved for future use - duration tracking feature
    #[serde(skip_serializing)]
    pub duration: Option<std::time::Duration>,
}

/// Format duration for display
#[allow(dead_code)] // Reserved for future use - duration tracking feature
fn format_duration(duration: std::time::Duration) -> String {
    let total_seconds = duration.as_secs();
    let hours = total_seconds / 3600;
    let minutes = (total_seconds % 3600) / 60;
    let seconds = total_seconds % 60;
    
    if hours > 0 {
        format!("{:02}:{:02}:{:02}", hours, minutes, seconds)
    } else {
        format!("{:02}:{:02}", minutes, seconds)
    }
}

impl Drop for RecordingStateManager {
    fn drop(&mut self) {
        // Best effort cleanup
        if let Ok(handle) = self.tray_timer_handle.try_lock() {
            if let Some(timer) = handle.as_ref() {
                timer.abort();
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::recording::types::*;
    
    #[tokio::test]
    async fn test_recording_state_manager_creation() {
        let manager = RecordingStateManager::new();
        assert!(manager.is_ok(), "Should be able to create recording state manager");
        
        let manager = manager.unwrap();
        assert!(!manager.is_recording(), "Should not be recording initially");
        
        let state = manager.get_cached_state();
        assert!(matches!(state, CoreRecordingState::Idle), "Should start in idle state");
    }
    
    #[tokio::test]
    async fn test_recording_state_caching() {
        let manager = RecordingStateManager::new().unwrap();
        
        // Cached state should match API state initially
        let cached_state = manager.get_cached_state();
        let current_state = manager.get_current_state().await;
        
        assert!(matches!(cached_state, CoreRecordingState::Idle));
        assert!(matches!(current_state, CoreRecordingState::Idle));
    }
    
    #[test]
    fn test_duration_formatting() {
        let duration1 = std::time::Duration::from_secs(65); // 1:05
        let duration2 = std::time::Duration::from_secs(3665); // 1:01:05
        
        assert_eq!(format_duration(duration1), "01:05");
        assert_eq!(format_duration(duration2), "01:01:05");
    }
    
    #[tokio::test]
    async fn test_recording_config_storage() {
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
        
        // Store config (this will fail to start recording but should store config)
        let _result = manager.start_recording(config.clone()).await;
        
        let stored_config = manager.get_current_config();
        assert!(stored_config.is_some(), "Should have stored config");
        
        let stored = stored_config.unwrap();
        assert_eq!(stored.output_path, "/tmp/test.mp4");
        assert!(matches!(stored.quality, QualityPreset::High));
    }
}