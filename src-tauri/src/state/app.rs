use anyhow::Result;
use parking_lot::RwLock;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

/// Core application state (non-recording related)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppState {
    /// General application settings
    pub settings: AppSettings,
    
    /// Display and window management
    pub displays: Vec<Display>,
    pub selected_display_id: Option<String>,
    pub windows: Vec<Window>,
    
    /// Audio devices
    pub audio_devices: AudioDevices,
    
    /// Input device configurations
    pub webcam_config: WebcamConfig,
    pub microphone_config: MicrophoneConfig,
    pub system_audio_config: SystemAudioConfig,
}

/// Application settings
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppSettings {
    /// Default output directory for recordings
    pub default_output_dir: String,
    
    /// Default recording quality
    pub default_quality: String,
    
    /// Automatically open editor after recording
    pub auto_open_editor: bool,
    
    /// Show system notifications
    pub show_notifications: bool,
    
    /// Keyboard shortcuts
    pub shortcuts: KeyboardShortcuts,
    
    /// UI theme
    pub theme: AppTheme,
}

/// Keyboard shortcuts configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KeyboardShortcuts {
    pub start_recording: String,
    pub stop_recording: String,
    pub pause_resume: String,
    pub take_screenshot: String,
}

/// Application theme
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AppTheme {
    Light,
    Dark,
    System,
}

/// Display information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Display {
    pub id: String,
    pub name: String,
    pub width: u32,
    pub height: u32,
    pub scale_factor: f32,
    pub refresh_rate: u32,
    pub is_primary: bool,
    pub thumbnail: Option<String>,
    #[cfg(target_os = "macos")]
    pub cg_display_id: u32,
}

/// Window information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Window {
    pub id: String,
    pub title: String,
    pub app_name: String,
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
    pub is_minimized: bool,
}

/// Audio devices
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AudioDevices {
    pub microphones: Vec<AudioDevice>,
    pub system_sources: Vec<AudioDevice>,
}

/// Audio device information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AudioDevice {
    pub id: String,
    pub name: String,
    pub is_default: bool,
}

/// Webcam configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WebcamConfig {
    pub enabled: bool,
    pub device_id: Option<String>,
    pub position: WebcamPosition,
    pub size: WebcamSize,
    pub shape: WebcamShape,
    pub auto_dodge: bool,
}

/// Webcam position on screen
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WebcamPosition {
    pub x_percent: f32,
    pub y_percent: f32,
}

/// Webcam size
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum WebcamSize {
    Small,
    Medium,
    Large,
    Custom { width: u32, height: u32 },
}

/// Webcam shape
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum WebcamShape {
    Circle,
    Square,
    Rounded,
}

/// Microphone configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MicrophoneConfig {
    pub enabled: bool,
    pub device_id: Option<String>,
    pub gain: f32,
    pub noise_reduction: bool,
}

/// System audio configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemAudioConfig {
    pub enabled: bool,
    pub source_id: Option<String>,
    pub volume: f32,
}

/// Thread-safe app state container
pub struct AppStateContainer {
    inner: Arc<RwLock<AppState>>,
}

#[allow(dead_code)]
impl AppStateContainer {
    /// Create new app state container with defaults
    pub fn new() -> Self {
        Self {
            inner: Arc::new(RwLock::new(AppState::default())),
        }
    }
    
    /// Get read access to app state
    pub fn read(&self) -> parking_lot::RwLockReadGuard<'_, AppState> {
        self.inner.read()
    }

    /// Get write access to app state
    pub fn write(&self) -> parking_lot::RwLockWriteGuard<'_, AppState> {
        self.inner.write()
    }
    
    /// Update app settings
    pub fn update_settings(&self, settings: AppSettings) {
        let mut state = self.write();
        state.settings = settings;
    }
    
    /// Update displays list and auto-select primary if none selected
    pub fn update_displays(&self, displays: Vec<Display>) {
        let mut state = self.write();
        let had_selection = state.selected_display_id.is_some();
        state.displays = displays;
        if !had_selection {
            // pick primary, else first
            if let Some(primary) = state.displays.iter().find(|d| d.is_primary) {
                state.selected_display_id = Some(primary.id.clone());
            } else if let Some(first) = state.displays.first() {
                state.selected_display_id = Some(first.id.clone());
            }
        } else if let Some(selected) = &state.selected_display_id {
            // If previously selected no longer exists, fallback
            let exists = state.displays.iter().any(|d| &d.id == selected);
            if !exists {
                if let Some(primary) = state.displays.iter().find(|d| d.is_primary) {
                    state.selected_display_id = Some(primary.id.clone());
                } else if let Some(first) = state.displays.first() {
                    state.selected_display_id = Some(first.id.clone());
                } else {
                    state.selected_display_id = None;
                }
            }
        }
    }
    
    /// Update windows list
    pub fn update_windows(&self, windows: Vec<Window>) {
        let mut state = self.write();
        state.windows = windows;
    }
    
    /// Update audio devices
    pub fn update_audio_devices(&self, devices: AudioDevices) {
        let mut state = self.write();
        state.audio_devices = devices;
    }
    
    /// Update webcam configuration
    pub fn update_webcam_config(&self, config: WebcamConfig) {
        let mut state = self.write();
        state.webcam_config = config;
    }
    
    /// Update microphone configuration
    pub fn update_microphone_config(&self, config: MicrophoneConfig) {
        let mut state = self.write();
        state.microphone_config = config;
    }
    
    /// Update system audio configuration
    pub fn update_system_audio_config(&self, config: SystemAudioConfig) {
        let mut state = self.write();
        state.system_audio_config = config;
    }
    
    /// Save state to file
    pub fn save_to_file(&self, path: &str) -> Result<()> {
        let state = self.read();
        let json = serde_json::to_string_pretty(&*state)?;
        std::fs::write(path, json)?;
        Ok(())
    }
    
    /// Load state from file
    pub fn load_from_file(&self, path: &str) -> Result<()> {
        let json = std::fs::read_to_string(path)?;
        let state: AppState = serde_json::from_str(&json)?;
        
        let mut current_state = self.write();
        *current_state = state;
        
        Ok(())
    }
}

impl Default for AppState {
    fn default() -> Self {
        Self {
            settings: AppSettings::default(),
            displays: Vec::new(),
            selected_display_id: None,
            windows: Vec::new(),
            audio_devices: AudioDevices::default(),
            webcam_config: WebcamConfig::default(),
            microphone_config: MicrophoneConfig::default(),
            system_audio_config: SystemAudioConfig::default(),
        }
    }
}

impl Default for AppSettings {
    fn default() -> Self {
        let home_dir = std::env::var("HOME").unwrap_or_else(|_| "/tmp".to_string());
        let default_output_dir = std::path::Path::new(&home_dir)
            .join("Movies")
            .join("Tarantino")
            .to_string_lossy()
            .to_string();
        
        Self {
            default_output_dir,
            default_quality: "High".to_string(),
            auto_open_editor: true,
            show_notifications: true,
            shortcuts: KeyboardShortcuts::default(),
            theme: AppTheme::System,
        }
    }
}

impl Default for KeyboardShortcuts {
    fn default() -> Self {
        Self {
            start_recording: "Cmd+Shift+R".to_string(),
            stop_recording: "Cmd+Shift+S".to_string(),
            pause_resume: "Cmd+Shift+P".to_string(),
            take_screenshot: "Cmd+Shift+3".to_string(),
        }
    }
}

impl Default for AudioDevices {
    fn default() -> Self {
        Self {
            microphones: Vec::new(),
            system_sources: Vec::new(),
        }
    }
}

impl Default for WebcamConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            device_id: None,
            position: WebcamPosition {
                x_percent: 85.0,
                y_percent: 15.0,
            },
            size: WebcamSize::Medium,
            shape: WebcamShape::Circle,
            auto_dodge: false,
        }
    }
}

impl Default for MicrophoneConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            device_id: None,
            gain: 1.0,
            noise_reduction: true,
        }
    }
}

impl Default for SystemAudioConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            source_id: None,
            volume: 1.0,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_app_state_creation() {
        let state = AppStateContainer::new();
        let app_state = state.read();
        
        assert!(!app_state.settings.default_output_dir.is_empty());
        assert_eq!(app_state.settings.default_quality, "High");
        assert!(app_state.settings.auto_open_editor);
    }
    
    #[test]
    fn test_app_state_serialization() {
        let state = AppState::default();
        let json = serde_json::to_string(&state).unwrap();
        let deserialized: AppState = serde_json::from_str(&json).unwrap();
        
        assert_eq!(state.settings.default_quality, deserialized.settings.default_quality);
        assert_eq!(state.webcam_config.enabled, deserialized.webcam_config.enabled);
    }
    
    #[test]
    fn test_settings_update() {
        let container = AppStateContainer::new();
        
        let new_settings = AppSettings {
            default_quality: "Low".to_string(),
            auto_open_editor: false,
            ..Default::default()
        };
        
        container.update_settings(new_settings);
        
        let state = container.read();
        assert_eq!(state.settings.default_quality, "Low");
        assert!(!state.settings.auto_open_editor);
    }
}