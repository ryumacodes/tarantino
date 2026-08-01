# Tarantino

A macOS screen recorder and editor for polished product demos.

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
- wgpu and Metal — preview and export rendering
- FFmpeg — media inspection and processing

## Platform Support

Tarantino currently supports macOS. Windows and Linux capture backends are not ready yet.

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

For permission debugging, Tarantino can also run as a raw binary. In this mode, macOS associates capture permissions with the terminal that launched it:

```bash
pnpm tauri:dev:raw
```

Use the regular development command unless you specifically need raw mode.

## Permissions

Tarantino needs macOS permission for the sources you choose to record:

- Screen Recording for displays and windows
- Microphone for voice capture
- Camera for webcam overlays
- Accessibility for native cursor and keyboard event tracking

Grant only the permissions needed for the recording you are making.

## License

[PolyForm Noncommercial License 1.0.0](./LICENSE). Commercial use requires prior written permission from the copyright holder.
