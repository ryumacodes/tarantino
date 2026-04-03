# Tarantino

A GPU-accelerated screen recording and editing app with a minimal floating capture bar, draggable webcam overlay, and a powerful video editor with real-time preview.

## Features

- **Floating Capture Bar**: Minimal, always-on-top UI with capture modes (Display, Window, Area)
- **Input Controls**: Camera, Microphone, and System Audio toggles with device selection
- **Pre-record Webcam Overlay**: Draggable, resizable webcam preview with circle/rounded rectangle shapes
- **GPU Compositor**: wgpu compute shaders for zoom, cursor, corners, shadow, motion blur, webcam overlay, and device frames
- **Hardware Encoding**: VideoToolbox on macOS
- **Video Editor**: Three.js real-time preview with timeline, spring-physics zoom blocks, SDF cursor rendering, and post-processing effects
- **Smart Zoom**: Auto-detected and manual zoom blocks with per-block spring physics (slow/mellow/quick/rapid), cursor-follow phase, configurable in/out speeds
- **Cursor Rendering**: SDF-based with 5 styles (pointer, circle, filled, outline, dotted), click effects (ripple, circle highlight), trail, idle fade, rotation
- **Visual Effects**: Padding, corner radius, shadows, background (solid/gradient/wallpaper), device frames, motion blur (pan + zoom channels)
- **Export Pipeline**: GPU-accelerated with FFmpeg decode/encode, wgpu compute compositing, audio muxing

## Tech Stack

- **Backend**: Rust + Tauri 2.9
- **GPU Pipeline**: wgpu 0.19 (Metal/D3D12/Vulkan) with WGSL compute shaders
- **Frontend**: React 19 + TypeScript 5.7 + Vite 5
- **3D Preview**: Three.js 0.180 + React Three Fiber 9 + postprocessing 6
- **State Management**: Zustand 4 + Immer
- **Styling**: Tailwind CSS 3
- **Package Manager**: pnpm 8

## Development

### Prerequisites

- Rust 1.75+
- Node.js 18+
- pnpm 8+
- FFmpeg (runtime dependency for decode/encode)
- Platform-specific:
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

### Capture Pipeline

Native backend per OS with a thin cross-platform abstraction.

- Core trait: `NativeCaptureBackend` with `enumerate_sources`, `start_capture`, `frame_receiver`, `stop_capture`, and `capabilities`.
- Backends:
  - macOS: ScreenCaptureKit (complete) — display, window, and area capture
  - Windows: DXGI Desktop Duplication (planned)
  - Linux: PipeWire via xdg-desktop-portal (planned)

### Recording Modes

- **Display/Screen**: Full display capture at native resolution
- **Window**: Individual window capture, aspect-fitted inside export canvas with background fill, padding, and letterbox/pillarbox

### GPU Export Pipeline

All per-frame effects run on the GPU via wgpu compute shaders:

1. FFmpeg decodes source video to raw RGBA frames
2. wgpu compute shader composites each frame: background → shadow → zoom/pan → motion blur → rounded corners → video → SDF cursor → webcam → device frame
3. FFmpeg encodes composited RGBA frames to output format with optional audio mux

### Video Editor Preview

Real-time Three.js preview matching the export pipeline:

- Spring-physics zoom with per-block speed presets and cursor-follow
- SDF cursor via GLSL post-processing (same shapes as export WGSL)
- Motion blur via directional + radial sampling
- Video plane aspect-fitting and padding matching export layout
- Window mode: entire canvas (video + background + shadow) zooms as one unit

### Mouse Event Sidecar

Mouse events captured via rdev during recording, stored as `.mouse.json` sidecar files. Coordinates normalized to [0,1] using recording area bounds. Same sidecar drives both preview and export cursor simulation.

### State Management

- **Recording State**: Idle → PreRecord → Recording → Review
- **Capture Modes**: Display, Window, Area
- **Editor Store**: Zustand + Immer with action slices (zoom, settings, playback)

### IPC Commands

Tauri IPC for UI-to-native communication:

- `capture_set_mode` / `capture_select_*`: Capture configuration
- `input_set_*`: Camera/mic/system audio
- `webcam_set_transform`: Overlay position/size
- `record_start_native` / `record_stop_native`: Recording control
- `export_video`: GPU-accelerated export with full visual settings
- `compute_cursor_trajectory`: Pre-compute cursor positions for preview

### macOS Notes (ScreenCaptureKit)

- Requires macOS 12.3+
- Grant System Settings → Privacy & Security → Screen Recording
- BGRA pixel format; window capture supported
- HiDPI/Retina: coordinates from rdev are in logical pixels (points)

## Roadmap

- [ ] Windows capture backend (DXGI Desktop Duplication)
- [ ] Linux capture backend (PipeWire / xdg-desktop-portal)
- [ ] Windows hardware encoding (Media Foundation)
- [ ] Linux hardware encoding (VAAPI/NVENC)
- [ ] Audio DSP (noise gate, EQ, gain normalization)
- [ ] Auto-dodge cursor avoidance for zoom
- [ ] Export presets (1080p60, 1440p60, 4K60)
- [ ] Hotkey / global shortcut support
- [ ] Multi-monitor DPI handling
- [ ] Area capture selection UI
- [ ] Gradient and image background export support
- [ ] Trim / split in timeline editor
- [ ] Undo/redo for editor actions

## License

MIT
