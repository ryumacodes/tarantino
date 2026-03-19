//! Device enumeration methods for UnifiedAppState
//!
//! Handles enumeration of displays, windows, cameras, and audio devices.

use anyhow::Result;

use super::{AudioDevice, AudioDevices, Display, UnifiedAppState, Window};

impl UnifiedAppState {
    /// Get all available displays
    pub async fn get_displays(&self) -> Result<Vec<Display>> {
        // If no selected display yet and we have a primary, auto-select it
        {
            let mut app = self.app.write();
            if app.selected_display_id.is_none() {
                if let Some(primary) = app.displays.iter().find(|d| d.is_primary) {
                    app.selected_display_id = Some(primary.id.clone());
                } else if let Some(first) = app.displays.first() {
                    app.selected_display_id = Some(first.id.clone());
                }
            }
        }

        use crate::capture::backends::{CaptureBackendFactory, CaptureSourceType};
        println!("=== UNIFIED_STATE: get_displays called ===");

        let displays = match CaptureBackendFactory::create_backend().and_then(|backend| {
            futures::executor::block_on(async move {
                let sources = backend.enumerate_sources().await?;
                Ok::<_, anyhow::Error>(sources)
            })
        }) {
            Ok(sources) => sources
                .into_iter()
                .filter(|s| matches!(s.source_type, CaptureSourceType::Display))
                .map(|d| {
                    println!(
                        "=== UNIFIED_STATE: Converting display {} ({}) ===",
                        d.id, d.name
                    );
                    Display {
                        id: d.id.to_string(),
                        name: d.name,
                        width: d.width,
                        height: d.height,
                        scale_factor: d.scale_factor as f32,
                        refresh_rate: 60,
                        is_primary: d.is_primary,
                        thumbnail: None,
                        #[cfg(target_os = "macos")]
                        cg_display_id: d.id as u32,
                    }
                })
                .collect(),
            Err(err) => {
                println!("get_displays: enumeration failed: {}", err);
                Vec::new()
            }
        };

        println!(
            "=== UNIFIED_STATE: Returning {} displays to frontend ===",
            displays.len()
        );
        Ok(displays)
    }

    /// Get all displays with thumbnails
    pub async fn get_displays_with_thumbnails(&self) -> Result<Vec<Display>> {
        println!("=== UNIFIED_STATE: get_displays_with_thumbnails called ===");

        // For now, just call get_displays() without thumbnails
        // TODO: Add thumbnail generation later
        self.get_displays().await
    }

    /// Get all available windows
    pub async fn get_windows(&self) -> Result<Vec<Window>> {
        use crate::capture::backends::{CaptureBackendFactory, CaptureSourceType};
        println!("=== UNIFIED_STATE: get_windows called ===");

        let windows = match CaptureBackendFactory::create_backend().and_then(|backend| {
            futures::executor::block_on(async move {
                let sources = backend.enumerate_sources().await?;
                Ok::<_, anyhow::Error>(sources)
            })
        }) {
            Ok(sources) => sources
                .into_iter()
                .filter(|s| matches!(s.source_type, CaptureSourceType::Window))
                .map(|w| {
                    println!(
                        "=== UNIFIED_STATE: Converting window {} ({}) ===",
                        w.id, w.name
                    );
                    Window {
                        id: w.id.to_string(),
                        title: w.name,
                        app_name: String::new(),
                        x: 0,
                        y: 0,
                        width: w.width,
                        height: w.height,
                        is_minimized: false,
                    }
                })
                .collect(),
            Err(err) => {
                println!("get_windows: enumeration failed: {}", err);
                Vec::new()
            }
        };

        println!(
            "=== UNIFIED_STATE: Returning {} windows to frontend ===",
            windows.len()
        );
        Ok(windows)
    }

    /// Get all available capture devices (cameras, etc.)
    pub async fn get_capture_devices(&self) -> Result<Vec<AudioDevice>> {
        println!("=== UNIFIED_STATE: get_capture_devices called ===");
        // TODO: Get capture devices (cameras, etc.)
        println!("=== UNIFIED_STATE: Returning 0 capture devices (not implemented yet) ===");
        Ok(vec![])
    }

    /// Get all audio devices
    pub async fn get_audio_devices(&self) -> Result<AudioDevices> {
        println!("=== UNIFIED_STATE: get_audio_devices called ===");
        // TODO: Get audio devices
        println!("=== UNIFIED_STATE: Returning default audio devices (not implemented yet) ===");
        Ok(AudioDevices::default())
    }

    /// Refresh all device lists (displays, windows, audio devices)
    pub async fn refresh_devices(&self) -> Result<()> {
        use crate::capture::backends::{CaptureBackendFactory, CaptureSourceType};

        // Try native enumeration; fall back to empty lists on failure
        let (displays, windows) = match CaptureBackendFactory::create_backend().and_then(
            |backend| {
                futures::executor::block_on(async move {
                    let sources = backend.enumerate_sources().await?;
                    Ok::<_, anyhow::Error>(sources)
                })
            },
        ) {
            Ok(sources) => {
                let mut displays_out = Vec::new();
                let mut windows_out = Vec::new();
                for src in sources {
                    match src.source_type {
                        CaptureSourceType::Display => {
                            displays_out.push(super::app::Display {
                                id: src.id.to_string(),
                                name: src.name,
                                width: src.width,
                                height: src.height,
                                scale_factor: src.scale_factor as f32,
                                refresh_rate: 60, // Backend does not expose; placeholder
                                is_primary: src.is_primary,
                                thumbnail: None,
                                #[cfg(target_os = "macos")]
                                cg_display_id: src.id as u32,
                            });
                        }
                        CaptureSourceType::Window => {
                            windows_out.push(super::app::Window {
                                id: src.id.to_string(),
                                title: src.name,
                                app_name: String::new(),
                                x: 0,
                                y: 0,
                                width: src.width,
                                height: src.height,
                                is_minimized: false,
                            });
                        }
                    }
                }
                (displays_out, windows_out)
            }
            Err(err) => {
                println!("Device enumeration failed, using empty lists: {}", err);
                (Vec::new(), Vec::new())
            }
        };

        self.app.update_displays(displays);
        self.app.update_windows(windows);

        // Audio devices: keep defaults until native audio enumeration is added
        println!("Device lists refreshed from native backends");
        Ok(())
    }
}
