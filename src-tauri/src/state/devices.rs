//! Device enumeration methods for UnifiedAppState
//!
//! Handles enumeration of displays, windows, cameras, and audio devices.

use anyhow::Result;
use std::sync::atomic::Ordering;

use super::{AudioDevice, AudioDevices, Display, UnifiedAppState, Window};

impl UnifiedAppState {
    pub fn cached_displays(&self) -> Vec<Display> {
        self.app.read().displays.clone()
    }

    pub fn cached_windows(&self) -> Vec<Window> {
        self.app.read().windows.clone()
    }

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

        #[cfg(not(target_os = "macos"))]
        use crate::capture::backends::CaptureBackendFactory;
        use crate::capture::backends::CaptureSourceType;
        println!("=== UNIFIED_STATE: get_displays called ===");

        #[cfg(target_os = "macos")]
        let sources_result =
            crate::capture::backends::macos::ScreenCaptureKitBackend::enumerate_displays_only();

        #[cfg(not(target_os = "macos"))]
        let sources_result = CaptureBackendFactory::create_backend().and_then(|backend| {
            futures::executor::block_on(async move {
                let sources = backend.enumerate_sources().await?;
                Ok::<_, anyhow::Error>(sources)
            })
        });

        let displays: Vec<Display> = match sources_result {
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
        if !displays.is_empty() {
            self.app.update_displays(displays.clone());
        }

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

    /// Get cached windows. Use refresh_windows() to perform native enumeration.
    pub async fn get_windows(&self) -> Result<Vec<Window>> {
        Ok(self.cached_windows())
    }

    /// Refresh windows from the native backend, coalescing overlapping requests.
    pub async fn refresh_windows(&self) -> Result<Vec<Window>> {
        use crate::capture::backends::CaptureSourceType;
        println!("=== UNIFIED_STATE: refresh_windows called ===");

        if self
            .source_refreshing
            .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
            .is_err()
        {
            println!("=== UNIFIED_STATE: refresh already active, returning cached windows ===");
            return Ok(self.cached_windows());
        }
        struct RefreshGuard<'a>(&'a std::sync::atomic::AtomicBool);
        impl Drop for RefreshGuard<'_> {
            fn drop(&mut self) {
                self.0.store(false, Ordering::SeqCst);
            }
        }
        let _guard = RefreshGuard(&self.source_refreshing);

        #[cfg(target_os = "macos")]
        let sources_result =
            crate::capture::backends::macos::ScreenCaptureKitBackend::enumerate_windows_only();

        #[cfg(not(target_os = "macos"))]
        let sources_result = {
            use crate::capture::backends::CaptureBackendFactory;
            CaptureBackendFactory::create_backend().and_then(|backend| {
                futures::executor::block_on(async move {
                    let sources = backend.enumerate_sources().await?;
                    Ok::<_, anyhow::Error>(sources)
                })
            })
        };

        let windows: Vec<Window> = match sources_result {
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
                        app_name: w.owner_name,
                        x: w.x,
                        y: w.y,
                        width: w.width,
                        height: w.height,
                        is_minimized: false,
                    }
                })
                .collect(),
            Err(err) => {
                println!("get_windows: enumeration failed: {}", err);
                let cached = self.app.read().windows.clone();
                if !cached.is_empty() {
                    println!(
                        "=== UNIFIED_STATE: Returning {} cached windows after enumeration failure ===",
                        cached.len()
                    );
                }
                cached
            }
        };
        if !windows.is_empty() {
            self.app.update_windows(windows.clone());
        }

        println!("=== UNIFIED_STATE: Refreshed {} windows ===", windows.len());
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
        #[cfg(not(target_os = "macos"))]
        use crate::capture::backends::{CaptureBackendFactory, CaptureSourceType};

        // Try native enumeration; fall back to empty lists on failure
        #[cfg(target_os = "macos")]
        let (displays, windows) = {
            let displays_out =
                crate::capture::backends::macos::ScreenCaptureKitBackend::enumerate_displays_only()
                    .map(|sources| {
                        sources
                            .into_iter()
                            .map(|src| super::app::Display {
                                id: src.id.to_string(),
                                name: src.name,
                                width: src.width,
                                height: src.height,
                                scale_factor: src.scale_factor as f32,
                                refresh_rate: 60,
                                is_primary: src.is_primary,
                                thumbnail: None,
                                cg_display_id: src.id as u32,
                            })
                            .collect()
                    })
                    .unwrap_or_else(|err| {
                        println!("Display enumeration failed, using empty list: {}", err);
                        Vec::new()
                    });
            // Window enumeration is intentionally deferred until the frontend is ready.
            // SCK window discovery is the slowest source path and should not block boot.
            (displays_out, self.cached_windows())
        };

        #[cfg(not(target_os = "macos"))]
        let (displays, windows) =
            match CaptureBackendFactory::create_backend().and_then(|backend| {
                futures::executor::block_on(async move {
                    let sources = backend.enumerate_sources().await?;
                    Ok::<_, anyhow::Error>(sources)
                })
            }) {
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
                                    x: src.x,
                                    y: src.y,
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
