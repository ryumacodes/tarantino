// Allow dead code in this module - it's a complete state management API
// where not all methods are currently used but provide consistent interface
#![allow(dead_code)]

use parking_lot::RwLock;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;

/// UI state management for windows, tray, and interface elements
///
/// This module handles all UI-related state separate from recording logic,
/// providing clean separation of concerns.
#[derive(Debug, Clone)]
pub struct UIStateManager {
    /// Current window states
    window_states: Arc<RwLock<HashMap<String, WindowState>>>,
    
    /// Tray state
    tray_state: Arc<RwLock<TrayState>>,
    
    /// Current interface mode
    interface_mode: Arc<RwLock<InterfaceMode>>,
    
    /// Dialog states
    dialog_states: Arc<RwLock<HashMap<String, DialogState>>>,
    
    /// Loading states for different operations
    loading_states: Arc<RwLock<HashMap<String, LoadingState>>>,
}

/// State of individual windows
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WindowState {
    pub id: String,
    pub visible: bool,
    pub position: Option<WindowPosition>,
    pub size: Option<WindowSize>,
    pub minimized: bool,
    pub focused: bool,
    pub loading: bool,
}

/// Window position
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WindowPosition {
    pub x: i32,
    pub y: i32,
}

/// Window size
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WindowSize {
    pub width: u32,
    pub height: u32,
}

/// System tray state
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrayState {
    pub mode: TrayMode,
    pub status_text: String,
    pub recording_duration: Option<String>,
    pub menu_items: Vec<TrayMenuItem>,
}

/// Tray operating mode
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum TrayMode {
    Idle,
    Recording,
    Processing,
    Error(String),
}

/// Tray menu item
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrayMenuItem {
    pub id: String,
    pub label: String,
    pub enabled: bool,
    pub visible: bool,
}

/// Current interface mode
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum InterfaceMode {
    /// Initial setup and source selection
    Setup,
    /// Ready to record
    Ready,
    /// Currently recording
    Recording,
    /// Processing recording
    Processing,
    /// In editor mode
    Editor,
    /// Error state
    Error(String),
}

/// Dialog state
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DialogState {
    pub id: String,
    pub visible: bool,
    pub dialog_type: DialogType,
    pub data: serde_json::Value,
}

/// Types of dialogs
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum DialogType {
    Settings,
    Export,
    Preferences,
    About,
    Error,
    Confirmation,
}

/// Loading state for operations
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoadingState {
    pub operation: String,
    pub progress: f64,
    pub message: String,
    pub cancellable: bool,
}

impl UIStateManager {
    /// Create new UI state manager
    pub fn new() -> Self {
        Self {
            window_states: Arc::new(RwLock::new(HashMap::new())),
            tray_state: Arc::new(RwLock::new(TrayState::default())),
            interface_mode: Arc::new(RwLock::new(InterfaceMode::Setup)),
            dialog_states: Arc::new(RwLock::new(HashMap::new())),
            loading_states: Arc::new(RwLock::new(HashMap::new())),
        }
    }
    
    /// Set window state
    pub fn set_window_state(&self, window_id: String, state: WindowState) {
        let mut windows = self.window_states.write();
        windows.insert(window_id, state);
    }
    
    /// Get window state
    pub fn get_window_state(&self, window_id: &str) -> Option<WindowState> {
        let windows = self.window_states.read();
        windows.get(window_id).cloned()
    }
    
    /// Show window
    pub fn show_window(&self, window_id: &str) {
        let mut windows = self.window_states.write();
        if let Some(state) = windows.get_mut(window_id) {
            state.visible = true;
            state.minimized = false;
        } else {
            windows.insert(window_id.to_string(), WindowState {
                id: window_id.to_string(),
                visible: true,
                position: None,
                size: None,
                minimized: false,
                focused: true,
                loading: false,
            });
        }
    }
    
    /// Hide window
    pub fn hide_window(&self, window_id: &str) {
        let mut windows = self.window_states.write();
        if let Some(state) = windows.get_mut(window_id) {
            state.visible = false;
        }
    }
    
    /// Set window loading state
    pub fn set_window_loading(&self, window_id: &str, loading: bool) {
        let mut windows = self.window_states.write();
        if let Some(state) = windows.get_mut(window_id) {
            state.loading = loading;
        }
    }
    
    /// Update tray state
    pub fn update_tray_state(&self, tray_state: TrayState) {
        let mut state = self.tray_state.write();
        *state = tray_state;
    }
    
    /// Set tray to idle mode
    pub fn set_tray_idle(&self) {
        let mut state = self.tray_state.write();
        state.mode = TrayMode::Idle;
        state.status_text = "Ready to Record".to_string();
        state.recording_duration = None;
        state.menu_items = self.get_idle_menu_items();
    }
    
    /// Set tray to recording mode
    pub fn set_tray_recording(&self, duration: Option<String>) {
        let mut state = self.tray_state.write();
        state.mode = TrayMode::Recording;
        state.status_text = "Recording".to_string();
        state.recording_duration = duration;
        state.menu_items = self.get_recording_menu_items();
    }
    
    /// Set tray to processing mode
    pub fn set_tray_processing(&self, message: &str) {
        let mut state = self.tray_state.write();
        state.mode = TrayMode::Processing;
        state.status_text = message.to_string();
        state.recording_duration = None;
        state.menu_items = self.get_processing_menu_items();
    }
    
    /// Set tray error state
    pub fn set_tray_error(&self, error: &str) {
        let mut state = self.tray_state.write();
        state.mode = TrayMode::Error(error.to_string());
        state.status_text = "Error".to_string();
        state.recording_duration = None;
        state.menu_items = self.get_error_menu_items();
    }
    
    /// Get current tray state
    pub fn get_tray_state(&self) -> TrayState {
        let state = self.tray_state.read();
        state.clone()
    }
    
    /// Set interface mode
    pub fn set_interface_mode(&self, mode: InterfaceMode) {
        let mut current_mode = self.interface_mode.write();
        *current_mode = mode;
    }
    
    /// Get current interface mode
    pub fn get_interface_mode(&self) -> InterfaceMode {
        let mode = self.interface_mode.read();
        mode.clone()
    }
    
    /// Show dialog
    pub fn show_dialog(&self, dialog_id: String, dialog_type: DialogType, data: serde_json::Value) {
        let mut dialogs = self.dialog_states.write();
        dialogs.insert(dialog_id.clone(), DialogState {
            id: dialog_id,
            visible: true,
            dialog_type,
            data,
        });
    }
    
    /// Hide dialog
    pub fn hide_dialog(&self, dialog_id: &str) {
        let mut dialogs = self.dialog_states.write();
        if let Some(dialog) = dialogs.get_mut(dialog_id) {
            dialog.visible = false;
        }
    }
    
    /// Get dialog state
    pub fn get_dialog_state(&self, dialog_id: &str) -> Option<DialogState> {
        let dialogs = self.dialog_states.read();
        dialogs.get(dialog_id).cloned()
    }
    
    /// Set loading state for an operation
    pub fn set_loading_state(&self, operation: String, state: LoadingState) {
        let mut loading = self.loading_states.write();
        loading.insert(operation, state);
    }
    
    /// Clear loading state
    pub fn clear_loading_state(&self, operation: &str) {
        let mut loading = self.loading_states.write();
        loading.remove(operation);
    }
    
    /// Get loading state
    pub fn get_loading_state(&self, operation: &str) -> Option<LoadingState> {
        let loading = self.loading_states.read();
        loading.get(operation).cloned()
    }
    
    /// Get all window states
    pub fn get_all_window_states(&self) -> HashMap<String, WindowState> {
        let windows = self.window_states.read();
        windows.clone()
    }
    
    /// Clear all UI state (for reset/cleanup)
    pub fn clear_all_state(&self) {
        {
            let mut windows = self.window_states.write();
            windows.clear();
        }
        
        {
            let mut dialogs = self.dialog_states.write();
            dialogs.clear();
        }
        
        {
            let mut loading = self.loading_states.write();
            loading.clear();
        }
        
        self.set_tray_idle();
        self.set_interface_mode(InterfaceMode::Setup);
    }
    
    /// Get idle menu items
    fn get_idle_menu_items(&self) -> Vec<TrayMenuItem> {
        vec![
            TrayMenuItem {
                id: "start_recording".to_string(),
                label: "Start Recording".to_string(),
                enabled: true,
                visible: true,
            },
            TrayMenuItem {
                id: "separator1".to_string(),
                label: "-".to_string(),
                enabled: false,
                visible: true,
            },
            TrayMenuItem {
                id: "settings".to_string(),
                label: "Settings".to_string(),
                enabled: true,
                visible: true,
            },
            TrayMenuItem {
                id: "quit".to_string(),
                label: "Quit Tarantino".to_string(),
                enabled: true,
                visible: true,
            },
        ]
    }
    
    /// Get recording menu items
    fn get_recording_menu_items(&self) -> Vec<TrayMenuItem> {
        vec![
            TrayMenuItem {
                id: "recording_status".to_string(),
                label: "Recording...".to_string(),
                enabled: false,
                visible: true,
            },
            TrayMenuItem {
                id: "separator1".to_string(),
                label: "-".to_string(),
                enabled: false,
                visible: true,
            },
            TrayMenuItem {
                id: "pause_recording".to_string(),
                label: "Pause Recording".to_string(),
                enabled: true,
                visible: true,
            },
            TrayMenuItem {
                id: "stop_recording".to_string(),
                label: "Stop Recording".to_string(),
                enabled: true,
                visible: true,
            },
            TrayMenuItem {
                id: "separator2".to_string(),
                label: "-".to_string(),
                enabled: false,
                visible: true,
            },
            TrayMenuItem {
                id: "quit".to_string(),
                label: "Quit Tarantino".to_string(),
                enabled: true,
                visible: true,
            },
        ]
    }
    
    /// Get processing menu items
    fn get_processing_menu_items(&self) -> Vec<TrayMenuItem> {
        vec![
            TrayMenuItem {
                id: "processing_status".to_string(),
                label: "Processing...".to_string(),
                enabled: false,
                visible: true,
            },
            TrayMenuItem {
                id: "separator1".to_string(),
                label: "-".to_string(),
                enabled: false,
                visible: true,
            },
            TrayMenuItem {
                id: "quit".to_string(),
                label: "Quit Tarantino".to_string(),
                enabled: true,
                visible: true,
            },
        ]
    }
    
    /// Get error menu items
    fn get_error_menu_items(&self) -> Vec<TrayMenuItem> {
        vec![
            TrayMenuItem {
                id: "error_status".to_string(),
                label: "Error occurred".to_string(),
                enabled: false,
                visible: true,
            },
            TrayMenuItem {
                id: "separator1".to_string(),
                label: "-".to_string(),
                enabled: false,
                visible: true,
            },
            TrayMenuItem {
                id: "retry".to_string(),
                label: "Retry".to_string(),
                enabled: true,
                visible: true,
            },
            TrayMenuItem {
                id: "settings".to_string(),
                label: "Settings".to_string(),
                enabled: true,
                visible: true,
            },
            TrayMenuItem {
                id: "quit".to_string(),
                label: "Quit Tarantino".to_string(),
                enabled: true,
                visible: true,
            },
        ]
    }
}

impl Default for TrayState {
    fn default() -> Self {
        Self {
            mode: TrayMode::Idle,
            status_text: "Ready to Record".to_string(),
            recording_duration: None,
            menu_items: Vec::new(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_ui_state_manager_creation() {
        let manager = UIStateManager::new();
        
        let mode = manager.get_interface_mode();
        assert!(matches!(mode, InterfaceMode::Setup));
        
        let tray_state = manager.get_tray_state();
        assert!(matches!(tray_state.mode, TrayMode::Idle));
    }
    
    #[test]
    fn test_window_state_management() {
        let manager = UIStateManager::new();
        
        // Show window
        manager.show_window("editor");
        let state = manager.get_window_state("editor").unwrap();
        assert!(state.visible);
        assert!(!state.minimized);
        
        // Hide window
        manager.hide_window("editor");
        let state = manager.get_window_state("editor").unwrap();
        assert!(!state.visible);
        
        // Set loading
        manager.set_window_loading("editor", true);
        let state = manager.get_window_state("editor").unwrap();
        assert!(state.loading);
    }
    
    #[test]
    fn test_tray_state_transitions() {
        let manager = UIStateManager::new();
        
        // Start with idle
        let state = manager.get_tray_state();
        assert!(matches!(state.mode, TrayMode::Idle));
        
        // Switch to recording
        manager.set_tray_recording(Some("00:30".to_string()));
        let state = manager.get_tray_state();
        assert!(matches!(state.mode, TrayMode::Recording));
        assert_eq!(state.recording_duration, Some("00:30".to_string()));
        
        // Switch to processing
        manager.set_tray_processing("Processing video...");
        let state = manager.get_tray_state();
        assert!(matches!(state.mode, TrayMode::Processing));
        
        // Switch to error
        manager.set_tray_error("Recording failed");
        let state = manager.get_tray_state();
        assert!(matches!(state.mode, TrayMode::Error(_)));
    }
    
    #[test]
    fn test_dialog_management() {
        let manager = UIStateManager::new();
        
        // Show dialog
        manager.show_dialog(
            "settings".to_string(),
            DialogType::Settings,
            serde_json::json!({"tab": "general"}),
        );
        
        let dialog = manager.get_dialog_state("settings").unwrap();
        assert!(dialog.visible);
        assert!(matches!(dialog.dialog_type, DialogType::Settings));
        
        // Hide dialog
        manager.hide_dialog("settings");
        let dialog = manager.get_dialog_state("settings").unwrap();
        assert!(!dialog.visible);
    }
    
    #[test]
    fn test_loading_state_management() {
        let manager = UIStateManager::new();
        
        // Set loading state
        let loading_state = LoadingState {
            operation: "encoding".to_string(),
            progress: 50.0,
            message: "Encoding video...".to_string(),
            cancellable: true,
        };
        
        manager.set_loading_state("export".to_string(), loading_state);
        
        let state = manager.get_loading_state("export").unwrap();
        assert_eq!(state.progress, 50.0);
        assert!(state.cancellable);
        
        // Clear loading state
        manager.clear_loading_state("export");
        let state = manager.get_loading_state("export");
        assert!(state.is_none());
    }
    
    #[test]
    fn test_interface_mode_changes() {
        let manager = UIStateManager::new();
        
        // Start in setup mode
        let mode = manager.get_interface_mode();
        assert!(matches!(mode, InterfaceMode::Setup));
        
        // Change to ready
        manager.set_interface_mode(InterfaceMode::Ready);
        let mode = manager.get_interface_mode();
        assert!(matches!(mode, InterfaceMode::Ready));
        
        // Change to recording
        manager.set_interface_mode(InterfaceMode::Recording);
        let mode = manager.get_interface_mode();
        assert!(matches!(mode, InterfaceMode::Recording));
        
        // Change to editor
        manager.set_interface_mode(InterfaceMode::Editor);
        let mode = manager.get_interface_mode();
        assert!(matches!(mode, InterfaceMode::Editor));
    }
}