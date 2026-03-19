use anyhow::Result;
use parking_lot::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::SystemTime;

// Re-export all state modules
pub mod app;
mod devices;
mod mouse_state;
pub mod persistence;
pub mod recording; // TODO: migrate to native backend state or remove
pub mod ui;

pub use persistence::save_zoom_sidecar;

pub use app::{AppStateContainer, AudioDevice, AudioDevices, Display, Window};
pub use recording::{RecordingInfo, RecordingStateManager};
pub use ui::{InterfaceMode, TrayState, UIStateManager};

// Type alias for devices (audio devices for now)
pub type Device = AudioDevice;

/// Unified application state container
///
/// This provides a single entry point for all application state management,
/// with proper separation of concerns between app settings, recording state,
/// and UI state.
pub struct UnifiedAppState {
    /// Core application state (settings, devices, etc.)
    pub app: AppStateContainer,

    /// Recording state management
    pub recording: RecordingStateManager,

    /// UI state management
    pub ui: UIStateManager,

    /// Tray timer handle and its associated cancellation flag
    tray_timer: Arc<Mutex<Option<(tokio::task::JoinHandle<()>, Arc<AtomicBool>)>>>,
}

impl UnifiedAppState {
    /// Create new unified app state
    pub fn new() -> Result<Self> {
        Ok(Self {
            app: AppStateContainer::new(),
            recording: RecordingStateManager::new()?,
            ui: UIStateManager::new(),
            tray_timer: Arc::new(Mutex::new(None)),
        })
    }

    /// Initialize the application state
    pub async fn initialize(&self) -> Result<()> {
        // Load saved app settings if they exist
        if let Err(e) = self.load_app_settings().await {
            println!("No saved settings found, using defaults: {}", e);
        }

        // Set initial UI state
        self.ui.set_interface_mode(InterfaceMode::Setup);
        self.ui.set_tray_idle();

        // Refresh device lists
        self.refresh_devices().await?;

        println!("Unified app state initialized");
        Ok(())
    }

    // Note: load_app_settings, save_app_settings, get_config_directory are in persistence.rs

    /// Start recording with configuration
    pub async fn start_recording(
        &self,
        config: crate::recording::types::RecordingConfig,
    ) -> Result<()> {
        // Update UI state
        self.ui.set_interface_mode(InterfaceMode::Recording);
        self.ui.set_tray_recording(None);

        // Start recording through recording state manager
        self.recording.start_recording(config).await?;

        // Start mouse tracking best-effort so zoom analysis has events
        if let Err(e) = self.start_mouse_tracking().await {
            println!("Warning: Failed to start mouse tracking: {}", e);
        }

        println!("Recording started through unified state");
        Ok(())
    }

    /// Signal stop recording (instant response)
    /// Returns (temp_path, recording_start_time) tuple
    pub async fn signal_stop_recording(&self) -> Result<(String, Option<SystemTime>)> {
        println!("=== UNIFIED_STATE: signal_stop_recording called ===");
        println!("=== UNIFIED_STATE: About to call recording.signal_stop_recording() ===");
        let temp_path = self.recording.signal_stop_recording().await?;
        println!(
            "=== UNIFIED_STATE: recording.signal_stop_recording() completed, temp_path: {} ===",
            temp_path
        );

        // Cancel the tray timer when recording stops
        println!("=== UNIFIED_STATE: About to cancel tray timer ===");
        self.cancel_tray_timer();
        println!("=== UNIFIED_STATE: Tray timer cancellation completed ===");

        // CRITICAL: Capture recording_start_time BEFORE stop_mouse_tracking() resets it to None
        // This fixes the race condition where background processing would see all events at time=0
        let recording_start_time = {
            let tracker = self.get_mouse_tracker();
            let guard = tracker.lock();
            println!("=== UNIFIED_STATE: Captured recording_start_time: {:?} ===", guard.recording_start_time);
            guard.recording_start_time
        };

        // Stop mouse tracking to finalize event collection (this resets recording_start_time to None)
        if let Err(e) = self.stop_mouse_tracking().await {
            println!("Warning: Failed to stop mouse tracking: {}", e);
        }

        // Generate zoom analysis from collected mouse events
        // This creates .auto_zoom.json and .mouse.json sidecar files
        if let Err(e) = self.generate_zoom_analysis(&temp_path).await {
            println!("Warning: Failed to generate zoom analysis: {}", e);
        }

        // Update UI state
        self.ui.set_interface_mode(InterfaceMode::Processing);
        self.ui.set_tray_processing("Finalizing recording...");

        println!("Recording stop signaled through unified state");
        Ok((temp_path, recording_start_time))
    }

    /// Wait for recording completion (background processing)
    pub async fn wait_for_recording_completion(&self) -> Result<String> {
        let final_path = self.recording.wait_for_completion().await?;

        // Update UI state
        self.ui.set_interface_mode(InterfaceMode::Editor);
        self.ui.set_tray_idle();

        println!("Recording completed through unified state");
        Ok(final_path)
    }

    /// Pause recording
    pub async fn pause_recording(&self) -> Result<()> {
        self.recording.pause_recording().await?;

        // Update tray to show paused state
        self.ui.set_tray_recording(Some("Paused".to_string()));

        Ok(())
    }

    /// Resume recording
    pub async fn resume_recording(&self) -> Result<()> {
        self.recording.resume_recording().await?;

        // Update tray to show recording state
        self.ui.set_tray_recording(None);

        Ok(())
    }

    /// Get comprehensive app status
    pub async fn get_app_status(&self) -> AppStatus {
        let recording_info = self.recording.get_recording_info().await.unwrap_or_else(
            |_| RecordingInfo {
                state: crate::recording::RecordingState::Idle,
                config: None,
                started_at: None,
                duration: None,
            },
        );

        AppStatus {
            interface_mode: self.ui.get_interface_mode(),
            recording_info,
            tray_state: self.ui.get_tray_state(),
            is_recording: self.recording.is_recording(),
        }
    }

    // UI control methods (temporary - these should be moved to proper state management)
    pub async fn set_capture_mode(&self, _mode: crate::CaptureMode) -> Result<()> {
        // TODO: Implement capture mode setting
        Ok(())
    }

    pub async fn select_display(&self, id: String) -> Result<()> {
        let mut app = self.app.write();
        // Only allow selecting known display IDs
        if app.displays.iter().any(|d| d.id == id) {
            app.selected_display_id = Some(id);
            println!("Selected display updated");
        } else {
            println!("Attempted to select unknown display id");
        }
        Ok(())
    }

    pub async fn select_window(&self, _id: String) -> Result<()> {
        // TODO: Implement window selection
        Ok(())
    }

    pub async fn select_area(
        &self,
        _x: f64,
        _y: f64,
        _width: f64,
        _height: f64,
    ) -> Result<()> {
        // TODO: Implement area selection
        Ok(())
    }

    pub async fn select_device(&self, _id: String) -> Result<()> {
        // TODO: Implement device selection
        Ok(())
    }

    pub async fn set_camera_input(
        &self,
        _enabled: bool,
        _device_id: Option<String>,
        _shape: String,
    ) -> Result<()> {
        // TODO: Implement camera input settings
        Ok(())
    }

    pub async fn set_mic_input(&self, _enabled: bool, _device_id: Option<String>) -> Result<()> {
        // TODO: Implement microphone input settings
        Ok(())
    }

    pub async fn set_system_audio(
        &self,
        _enabled: bool,
        _source_id: Option<String>,
    ) -> Result<()> {
        // TODO: Implement system audio settings
        Ok(())
    }

    pub async fn set_webcam_transform(
        &self,
        _x_norm: f32,
        _y_norm: f32,
        _size_norm: f32,
        _shape: String,
    ) -> Result<()> {
        // TODO: Implement webcam transform settings
        Ok(())
    }

    pub async fn set_webcam_autododge(
        &self,
        _enabled: bool,
        _radius_norm: f32,
        _strength: f32,
    ) -> Result<()> {
        // TODO: Implement webcam autododge settings
        Ok(())
    }

    // Note: get_current_sidecar_path is in persistence.rs

    pub fn set_tray_timer_handle_with_flag(
        &self,
        handle: tokio::task::JoinHandle<()>,
        cancel_flag: Arc<AtomicBool>,
    ) {
        let mut timer = self.tray_timer.lock();

        // Cancel existing timer if there is one
        if let Some((existing_handle, existing_flag)) = timer.take() {
            // Set the existing timer's cancel flag and abort the task
            existing_flag.store(true, Ordering::Relaxed);
            existing_handle.abort();
            println!("Cancelled existing tray timer");
        }

        // Store the new timer handle and its cancel flag
        *timer = Some((handle, cancel_flag));
        println!("Stored new tray timer handle with cancel flag");
    }

    pub fn cancel_tray_timer(&self) {
        println!("=== CANCEL_TIMER: Cancelling tray timer ===");

        let mut timer = self.tray_timer.lock();
        if let Some((handle, cancel_flag)) = timer.take() {
            println!("=== CANCEL_TIMER: Setting cancellation flag with Release ordering ===");
            cancel_flag.store(true, Ordering::Release);
            println!("=== CANCEL_TIMER: Flag set, now aborting handle ===");
            handle.abort();
            println!("=== CANCEL_TIMER: Tray timer cancelled successfully ===");
        } else {
            println!("=== CANCEL_TIMER: No timer to cancel ===");
        }
    }

    pub async fn stop_recording(&self) -> Result<String> {
        let (temp_path, _recording_start_time) = self.signal_stop_recording().await?;
        Ok(temp_path)
    }

    /// Handle application shutdown
    #[allow(dead_code)]
    pub async fn shutdown(&self) -> Result<()> {
        println!("Shutting down unified app state...");

        // Cancel any active tray timer
        self.cancel_tray_timer();

        // Save settings
        if let Err(e) = self.save_app_settings().await {
            println!("Warning: Failed to save settings during shutdown: {}", e);
        }

        // Stop any active recordings
        if self.recording.is_recording() {
            println!("Stopping active recording during shutdown...");
            if let Err(e) = self.signal_stop_recording().await {
                println!("Warning: Failed to stop recording during shutdown: {}", e);
            }
        }

        // Clear UI state
        self.ui.clear_all_state();

        println!("Unified app state shutdown complete");
        Ok(())
    }

    /// Update tray with recording duration
    pub async fn update_recording_duration(&self, duration: String) {
        if self.recording.is_recording() {
            self.ui.set_tray_recording(Some(duration));
        }
    }

    /// Show error in UI
    pub fn show_error(&self, error: &str) {
        self.ui
            .set_interface_mode(InterfaceMode::Error(error.to_string()));
        self.ui.set_tray_error(error);
    }

    /// Clear error state
    #[allow(dead_code)]
    pub fn clear_error(&self) {
        self.ui.set_interface_mode(InterfaceMode::Ready);
        self.ui.set_tray_idle();
    }

    /// Open editor window
    #[allow(dead_code)]
    pub fn open_editor(&self, media_path: &str) {
        self.ui.set_interface_mode(InterfaceMode::Editor);
        self.ui.show_window("editor");

        // Set editor as loading while it processes the media
        self.ui.set_window_loading("editor", true);

        println!("Editor opened for media: {}", media_path);
    }

    /// Notify editor ready
    pub fn notify_editor_ready(&self) {
        self.ui.set_window_loading("editor", false);
        println!("Editor ready");
    }

    /// Get current recording audio flags (mic, system)
    pub fn get_recording_audio_flags(&self) -> (bool, bool) {
        let cfg = self.recording.get_current_config();
        if let Some(cfg) = cfg {
            (cfg.include_microphone, cfg.include_system_audio)
        } else {
            (false, false)
        }
    }
}

/// Comprehensive application status
#[derive(Debug, Clone)]
pub struct AppStatus {
    pub interface_mode: InterfaceMode,
    pub recording_info: RecordingInfo,
    pub tray_state: TrayState,
    pub is_recording: bool,
}

impl Default for UnifiedAppState {
    fn default() -> Self {
        Self::new().expect("Failed to create unified app state")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_unified_state_creation() {
        let state = UnifiedAppState::new();
        assert!(state.is_ok(), "Should be able to create unified state");

        let state = state.unwrap();
        assert!(
            !state.recording.is_recording(),
            "Should not be recording initially"
        );

        let interface_mode = state.ui.get_interface_mode();
        assert!(
            matches!(interface_mode, InterfaceMode::Setup),
            "Should start in setup mode"
        );
    }

    #[tokio::test]
    async fn test_state_initialization() {
        let state = UnifiedAppState::new().unwrap();
        let result = state.initialize().await;
        assert!(result.is_ok(), "Should be able to initialize state");

        // Check that devices were populated
        let app_state = state.app.read();
        assert!(
            !app_state.displays.is_empty(),
            "Should have displays after initialization"
        );
    }

    #[tokio::test]
    async fn test_app_status() {
        let state = UnifiedAppState::new().unwrap();
        state.initialize().await.unwrap();

        let status = state.get_app_status().await;
        assert!(!status.is_recording);
        assert!(matches!(status.interface_mode, InterfaceMode::Setup));
    }

    #[tokio::test]
    async fn test_error_handling() {
        let state = UnifiedAppState::new().unwrap();

        // Show error
        state.show_error("Test error");
        let interface_mode = state.ui.get_interface_mode();
        assert!(matches!(interface_mode, InterfaceMode::Error(_)));

        // Clear error
        state.clear_error();
        let interface_mode = state.ui.get_interface_mode();
        assert!(matches!(interface_mode, InterfaceMode::Ready));
    }

    #[tokio::test]
    async fn test_editor_operations() {
        let state = UnifiedAppState::new().unwrap();

        // Open editor
        state.open_editor("/tmp/test.mp4");

        let window_state = state.ui.get_window_state("editor").unwrap();
        assert!(window_state.visible);
        assert!(window_state.loading);

        // Notify ready
        state.notify_editor_ready();

        let window_state = state.ui.get_window_state("editor").unwrap();
        assert!(!window_state.loading);
    }
}
