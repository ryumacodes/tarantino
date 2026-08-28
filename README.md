# Tarantino

A native macOS and Linux screen recorder and editor for polished product demos.

> Early-stage software — under active development. Expect bugs and breaking changes.

## Features

- **Display and window capture** — Record a full display or a single app window
- **Audio and webcam** — Capture system audio, a microphone, and a positioned camera overlay
- **Automatic zooms** — Generate zooms from clicks and typing, then adjust them in the timeline
- **Cursor effects** — Smooth cursor movement and render clicks, trails, and motion effects
- **Presentation framing** — Add padding, rounded corners, shadows, backgrounds, and device frames
- **GPU export** — Render edits and effects through one accelerated compositor

## Built With

- [Tauri](https://tauri.app/) and Rust — desktop shell and native application code
- React and Zustand — editor interface and state management
- ScreenCaptureKit and VideoToolbox — macOS capture and hardware video encoding
- xdg-desktop-portal, PipeWire, and VA-API — Wayland capture and hardware video encoding on Linux
- wgpu, Metal, and Vulkan — preview and export rendering
- FFmpeg — media inspection and processing

## Platform Support

Tarantino supports macOS 12.3+ and modern Linux desktops with xdg-desktop-portal and PipeWire. Linux media processing works with both FFmpeg 8 and FFmpeg 9. Windows capture is not ready yet.

## Installation

### Developers (Fresh Clone)

You will need Node.js, pnpm, Rust, and the Xcode Command Line Tools.

```bash
git clone https://github.com/ryumacodes/tarantino.git
cd tarantino
pnpm install
pnpm tauri:dev
```

The first recording may prompt for Screen Recording, Microphone, or Camera access. If you change a permission in macOS System Settings, restart Tarantino before testing it again.

## Development

Common development tasks:

```bash
pnpm tauri:dev       # Run the development app
pnpm test:unit       # Run frontend unit tests
pnpm test:macos      # Run the complete macOS verification suite
pnpm tauri:build     # Build the packaged app
```

The standard `pnpm tauri:dev` command detects the host platform: macOS runs
the native macOS toolchain, while Linux and SteamOS run through the native
Linux Distrobox environment.

For permission debugging, Tarantino can also run as a raw binary. In this mode, macOS associates capture permissions with the terminal that launched it:

```bash
pnpm tauri:dev:raw
```

Use the regular development command unless you specifically need raw mode.

### Linux / SteamOS

SteamOS is immutable and does not ship development headers. Tarantino uses a Distrobox environment for compilation while the app, GPU, PipeWire, desktop portal, camera, and audio remain connected directly to the host session.

```bash
pnpm setup:linux       # one-time setup
pnpm tauri:dev         # build and run the native Linux app
pnpm test:linux        # complete Linux verification
```

`pnpm tauri:dev:linux` remains available as an explicit Linux-only alias.

When recording starts, KDE's native sharing dialog asks which display or window to capture. This is required by Wayland's security model.

## Permissions

Tarantino needs macOS permission for the sources you choose to record:

- Screen Recording for displays and windows
- Microphone for voice capture
- Camera for webcam overlays
- Accessibility for native cursor and keyboard event tracking

On Linux, screen/window access is granted through the desktop portal picker. Camera and microphone permission prompts are provided by WebKit and PipeWire. Grant only the permissions needed for the recording you are making.

## License

[PolyForm Noncommercial License 1.0.0](./LICENSE). Commercial use requires prior written permission from the copyright holder.
