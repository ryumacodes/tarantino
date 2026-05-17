# Tarantino

Recording good product demos should be fast and accessible. Tarantino was built for that.

Tarantino is a screen recording and editing app for making polished product videos without dragging every detail around by hand. It is built around a simple idea: record quickly, make the result feel intentional, and export fast enough that iteration does not become a chore.

## What It Does

- Records your screen, a window, or a selected capture source
- Gives you a small floating capture bar that stays out of the way
- Supports camera, microphone, and system audio controls
- Lets you place and resize a webcam overlay before recording
- Opens recordings into an editor for review and polishing
- Adds smart zooms around clicks, typing, and cursor movement
- Supports manual zoom blocks when you want full control
- Renders clean cursor effects, click highlights, trails, and motion
- Adds presentation-style framing with padding, rounded corners, shadows, backgrounds, and device frames

## Fast Exports Matter

Tarantino uses a GPU compositor for export.

That is a core part of the app, not an optional extra. The GPU compositor keeps visual effects like zoom, motion blur, cursor rendering, shadows, rounded corners, webcam overlays, and device frames moving through one fast export path.

The goal is simple: you should be able to try an edit, export it, notice something, adjust it, and export again without losing momentum.

## The Editing Feel

Tarantino is designed for product demos, walkthroughs, tutorials, and short clips where the recording should feel guided but not over-produced.

The editor focuses on the things that usually make a screen recording feel better:

- Smooth zooms
- Clear cursor motion
- Good framing
- Clean audio/video output
- Fast preview and export loops

## Platform Support

Tarantino currently works on macOS.

Linux and Windows support are planned next.

## Current Focus

The priority is to keep improving:

- GPU-accelerated exporting
- Faster compile and development feedback
- Reliable macOS capture
- Better editing flow
- Cleaner project structure
- More polished final videos

## Development

Install dependencies:

```bash
pnpm install
```

Run the app in development:

```bash
pnpm tauri:dev
```

On macOS, development runs in raw mode so camera and screen permissions belong to the terminal that launched Tarantino. Run this from Terminal or iTerm2 when testing camera or screen access.

Packaged builds use the normal app permission flow.

Build the app:

```bash
pnpm tauri:build
```

## License

PolyForm Noncommercial License 1.0.0.

Commercial use is not permitted without prior written permission from the copyright holder.
