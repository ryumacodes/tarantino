# Tarantino

A GPU-accelerated screen recording app with a minimal floating capture bar, draggable webcam overlay, and powerful editing features.

## Features

- **Floating Capture Bar**: Minimal, always-on-top UI with four capture modes (Display, Window, Area, Device)
- **Input Controls**: Camera, Microphone, and System Audio toggles with device selection
- **Pre-record Webcam Overlay**: Draggable, resizable webcam preview with circle/rounded rectangle shapes
- **GPU Compositor**: wgpu-based real-time compositing with zero-copy pipeline
- **Hardware Encoding**: Platform-native encoders (VideoToolbox, Media Foundation, NVENC/VAAPI)
- **Editor UI**: Three.js-powered viewer with timeline, zoom keyframes, and overlay tools
- **Cross-platform**: macOS 13+, Windows 11+, Linux (Wayland/X11)

## Tech Stack

- **Backend**: Rust + Tauri 2.0
- **GPU Pipeline**: wgpu (Metal/D3D12/Vulkan)
- **Frontend**: React + TypeScript + Vite
- **3D Viewer**: Three.js + React Three Fiber
- **Styling**: Tailwind CSS

## Development

### Prerequisites

- Rust 1.75+
- Node.js 18+
- pnpm
- Platform-specific requirements:
  - macOS: Xcode Command Line Tools, Homebrew
  - Windows: Visual Studio 2022 with C++ tools
  - Linux: Development libraries for your distro

### Setup

```bash
# Install system dependencies (macOS)
brew install pkgconf ffmpeg

# Install dependencies
pnpm install

# Run in development mode
pnpm tauri:dev

# Build for production
pnpm tauri:build
```

## Architecture

### Capture Architecture (v0.2.0)

We use a thin cross‑platform abstraction with the best native backend per OS.

- Core trait: `NativeCaptureBackend` with `enumerate_sources`, `start_capture`, `frame_receiver`, `stop_capture`, and `capabilities`.
- Backends:
  - macOS: ScreenCaptureKit (✅ complete)
  - Windows: DXGI Desktop Duplication (stub)
  - Linux: PipeWire via xdg-desktop-portal (stub)

Why: native backends deliver higher FPS, lower latency, cursor/window inclusion, HDR/HiDPI awareness, and are maintained by platform vendors. The old FFmpeg/xcap-based implementations have been fully removed.

### Using the Backend (Low-Level)

```rust
use crate::capture::backends::{CaptureBackendFactory, CaptureConfig, CaptureSourceType};

let mut backend = CaptureBackendFactory::create_backend()?;
let perms = backend.check_permissions().await?;
if !perms.screen_recording { backend.request_permissions().await?; }

let sources = backend.enumerate_sources().await?;
let primary = sources.iter().find(|s| s.is_primary).unwrap();

let handle = backend.start_capture(CaptureConfig {
  source_id: primary.id,
  source_type: CaptureSourceType::Display,
  fps: 60,
  include_cursor: true,
  include_audio: false,
  region: None,
}).await?;

if let Some(mut rx) = backend.frame_receiver() {
  while let Ok(frame) = rx.recv().await {
    // frame.data (Bytes), frame.width, frame.height
  }
}

backend.stop_capture().await?;
```

### macOS Notes (SCK)
- Requires macOS 12.3+
- Grant System Settings → Privacy & Security → Screen Recording
- Use BGRA pixel format; window capture supported

### State + IPC
- Commands: `record_start_native`, `record_stop_native`, `project_open`, editor controls
- The recording session manager is being reworked to sit on top of the new backends.

### Encoding
- macOS: VideoToolbox (planned wiring after capture migration)
- Windows: Media Foundation (planned)
- Linux: Pipeline with VAAPI/NVENC (planned)

### State Management

- **Recording State**: Idle → PreRecord → Recording → Review
- **Capture Modes**: Display, Window, Area, Device
- **Input States**: Camera, Mic, System Audio (all default off)

### IPC Commands

The app uses Tauri's IPC system for UI-to-native communication:

- `capture_set_mode`: Switch capture mode
- `capture_select_*`: Select display/window/area/device
- `input_set_*`: Configure camera/mic/system audio
- `webcam_set_transform`: Update overlay position/size
- `record_start/stop/pause/resume`: Control recording
- `export_start`: Begin export with preset

## Roadmap

- [ ] Complete platform capture implementations
- [ ] Hardware encoder integration
- [ ] Audio DSP (noise gate, EQ)
- [ ] Auto-dodge cursor avoidance
- [ ] Export presets (1080p60, 1440p60, 4K60)
- [ ] Sidecar JSON for non-destructive edits
- [ ] Hotkey support
- [ ] Multi-monitor DPI handling

## License

MIT