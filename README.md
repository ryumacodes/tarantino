# Tarantino

A cross-platform screen recorder and editor for polished product demos.

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
- XDG Desktop Portal, PipeWire, and in-process GStreamer — Linux capture and encoding
- wgpu and Metal — preview and export rendering
- FFmpeg — media inspection and processing

## Platform Support

Tarantino supports macOS. Linux support is under active development for modern
PipeWire desktops on both Wayland and X11. Windows capture is not ready yet.

On Linux, the desktop portal presents the authoritative screen or window picker
when recording starts. This avoids compositor-specific enumeration and works
with GNOME, KDE Plasma, and other portal-capable desktop environments.

Linux cursor tracking and automatic click zoom are enabled by default. Tracking
uses the desktop RECORD service in a full X11 session. Wayland tracking still
requires session permission to read a compatible mouse device; the app reports
that capability separately from screen capture. The desktop's source
picker remains authoritative for screen-capture access. Typing-triggered zoom
also requires a readable keyboard; it records activity and editing categories,
not typed text. AT-SPI accessibility is enabled automatically for caret tracking
in supported applications. GTK and Qt caret events are mapped into recording
coordinates and drive typing zoom; when a reliable caret position is unavailable,
zoom uses the captured pointer position. Desktop-generated clicks (including
Steam Input emulation under XWayland) supplement physical input, with duplicate
reports filtered out. Pure Wayland sessions still depend on readable input devices.
System audio records the default output monitor to a separate WAV track through
PulseAudio or PipeWire's PulseAudio service.

## Installation

### Native Linux packages

Linux users should launch a packaged release directly; Node.js, Rust and
Distrobox are development tools, not application requirements. The Linux CI
build is configured to upload an x86_64 AppImage and Debian installer as the
`tarantino-linux-x86_64` artifact. The AppImage includes FFmpeg, ffprobe and the
GStreamer media framework, including the matching cryptography and media
helper libraries. It still uses the host's GTK desktop runtime, desktop portal,
PipeWire services and GPU drivers. Minimal server/container images need those
desktop libraries before the package can start.

After downloading and extracting a successful CI artifact on SteamOS:

```bash
chmod +x Tarantino_*.AppImage
./Tarantino_*.AppImage
```

If FUSE is unavailable, use `APPIMAGE_EXTRACT_AND_RUN=1 ./Tarantino_*.AppImage`.
Builds use Ubuntu 24.04 as their library baseline; compatibility with older
systems and SteamOS must be checked on those systems. A passing build does not
prove capture permissions, hardware acceleration or performance parity.

To build both packages in a Linux development environment:

```bash
pnpm build:linux
```

To check a package's media dependencies without opening a window:

```bash
APPIMAGE_EXTRACT_AND_RUN=1 ./Tarantino_*.AppImage --linux-runtime-check
```

This check exercises bundled media tools and synthetic video/audio encoding.
It does not capture the desktop or test GPU performance. CI runs it before
uploading packages; desktop validation follows the Linux recording checklist.

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

Keep handwritten source files at 700 lines or fewer. Repository checks enforce
this limit; necessary exceptions require a reason and a ceiling in
`scripts/source-size-exceptions.txt`. Generated schemas and dependency lockfiles
remain intact. Keep Linux fixes scoped to Linux and preserve macOS behavior and
performance; verify performance claims with macOS measurements.

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

### Linux development

Linux requires the normal Tauri dependencies plus FFmpeg, PipeWire (including
development headers for its cursor metadata reader), the desktop
portal for your desktop environment, and these GStreamer elements:
`pipewiresrc`, `h264parse`, `mp4mux`, and either `x264enc` or `openh264enc`.

The setup command detects the host from `/etc/os-release` and installs the
correct native packages on Arch/Manjaro, Debian/Ubuntu/Mint, Fedora/RHEL, and
openSUSE Tumbleweed. Leap requires manual dependency setup. SteamOS setup installs
native development packages from the configured SteamOS repositories, initializes
the package signing keyring and restores the original filesystem read-only mode
when it exits. SteamOS updates may require rerunning setup.

Run from a host terminal in the project directory:

```bash
sh scripts/setup-linux.sh
tarantino_tools="${XDG_DATA_HOME:-$HOME/.local/share}/tarantino/toolchain"
export PATH="$tarantino_tools/bin:$tarantino_tools/global/bin:${CARGO_HOME:-$HOME/.cargo}/bin:$PATH"
pnpm install --frozen-lockfile
pnpm tauri dev
```

Setup installs Rust 1.88 and Cargo with the official rustup installer if no
suitable compiler is available, and preserves an existing newer compiler. The
PATH line above makes a newly installed Cargo available in the current terminal;
`pnpm tauri:dev` also finds the user Cargo directory automatically. `pnpm install`
installs JavaScript dependencies; native toolchains are installed by Linux setup.
Setup also installs the pinned pnpm version. If Node is missing or too old, it
installs the official Node 24 LTS binary after checking its SHA-256 checksum.
These JavaScript tools live under the user data directory, leaving distro
packages in place. Existing compatible Node installations are preserved.
The SteamOS installer uses Pacman file records to reinstall packages with missing
development headers or pkg-config metadata, following the build dependency chain.
If repair fails, setup stops with the remaining error. Both setup and launch use the host directly and do
not create or enter a development container.

The capture implementation itself is distro-independent: XDG Desktop Portal
selects displays and windows, PipeWire supplies frames, and GStreamer chooses
an encoder available on that machine. Desktop-specific behavior is kept at the
edges—for example, only KDE Wayland uses the XWayland keep-above workaround.
Install the matching portal backend for the active desktop (`gnome`, `kde`, or
`wlr`); most desktop distributions include it by default.

Run the distro-independent verification suite with:

```bash
pnpm test:linux
```

The automated Linux matrix runs this suite on Ubuntu 24.04, Debian Stable,
Fedora, openSUSE Tumbleweed, and current Arch Linux, with the same build also
verified in the repository's Nix development environment. Linux Mint 22 uses
the Ubuntu 24.04 package base covered by the Ubuntu build; LMDE is represented
by Debian Stable. NixOS developers can enter the environment with `nix develop`.

CI also validates the GNOME, KDE Plasma, and wlroots ScreenCast portal
descriptors and recording plugins. A final recording smoke test still requires
a logged-in graphical session because the portal deliberately requires user
interaction; follow [the Linux recording checklist](docs/linux-recording-smoke-test.md).

## Permissions

Tarantino needs macOS permission for the sources you choose to record:

- Screen Recording for displays and windows
- Microphone for voice capture
- Camera for webcam overlays
- Accessibility for native cursor and keyboard event tracking

Grant only the permissions needed for the recording you are making.

## License

[PolyForm Noncommercial License 1.0.0](./LICENSE). Commercial use requires prior written permission from the copyright holder.
