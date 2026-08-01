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
                id: "restart_recording".to_string(),
                label: "Restart Recording".to_string(),
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
