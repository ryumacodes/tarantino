# Linux recording smoke test

Run this checklist in a real logged-in desktop session. Container CI can verify
the application, portal descriptors, and media plugins, but the ScreenCast
portal intentionally requires a user to approve its source picker.

## Session matrix

Test each available session:

| Desktop | Session | Expected portal backend |
| --- | --- | --- |
| GNOME | Wayland | `xdg-desktop-portal-gnome` |
| KDE Plasma | Wayland | `xdg-desktop-portal-kde` |
| Sway or Hyprland | Wayland | `xdg-desktop-portal-wlr` |
| GNOME or KDE Plasma | X11 | Matching GNOME or KDE backend |

## Preflight

1. Confirm PipeWire and the correct desktop portal are running.
2. Run `pnpm test:linux`.
3. Run `scripts/check-linux-portal-backend.sh gnome`, `kde`, or `wlr` for the
   active desktop.
4. Start the development application with `pnpm tauri:dev`.

## Recording cases

For every session in the matrix:

1. Record an entire display for at least ten seconds, with the cursor enabled.
2. Record one window for at least ten seconds.
3. Cancel the source picker and confirm the application returns to idle without
   leaving a recording process or an empty output file.
4. Record twice consecutively to verify the portal and PipeWire session are
   released after stopping.
5. Enable microphone capture and confirm the microphone sidecar is created.
6. Verify the MP4 with `ffprobe` and play it through to the final frame.
7. With a readable mouse device, record several clicks without enabling any
   additional app option. Confirm no automatic-zoom opt-in appears, cursor and
   click data are saved, and the editor and export both show generated zooms.
8. In a Wayland session without mouse-device access, confirm accessibility is
   reported as unavailable and screen recording still works with the compositor's
   cursor when cursor capture is enabled. A failed input listener must not leave
   tracking marked active. Restore access and record again to confirm tracking
   can start on the next recording. In a full X11 session with the RECORD
   extension, confirm click tracking remains available without raw-device access
   using the isolated X11 regression below.
9. With a readable keyboard, type in the captured source and confirm generated
   typing zooms follow the caret when accessible geometry can be matched, and
   otherwise use the captured cursor position. Shift+letters should count as
   typing; Ctrl/Alt/Meta shortcuts should not. Stop recording, type outside a
   recording, then record again and verify those earlier keystrokes are absent.
   Reconnect a keyboard during recording and confirm detection resumes.

Check that the chosen source is correct, cursor visibility matches the setting,
the output dimensions are sensible, timestamps increase normally, and stopping
produces a playable finalized file.

## System audio

Enable system audio, play a recognizable sound through the default output, and
record it with a display and then a window. Confirm a `.system.wav` sidecar is
created, it contains the output sound rather than microphone input, and its
onset matches the video in both editor playback and export. Repeat with the
microphone enabled and confirm the two tracks remain separate and editable.
Stop and record again to check that both WAV headers are finalized. Also test
startup with the PulseAudio-compatible service unavailable: recording must fail
cleanly without leaving a capture pipeline running. The automated audio fixture
validates the production conversion/WAV branch; a live monitor test is still
required on each desktop.

## Native package acceptance

Run the downloaded AppImage on the host, outside the development container.

- Run `--linux-runtime-check` and save its output. Then launch normally with no
  Node.js, Rust, FFmpeg or GStreamer development environment added to PATH.
- On SteamOS, verify Desktop Mode first; test Gaming Mode separately and report
  portal or window-management limitations rather than assuming Desktop Mode results apply.
- Record the GPU, driver, desktop/session, resolution, frame rate and selected
  encoder. Confirm hardware encoding is actually selected on supported hardware.
- Measure capture CPU/GPU usage, dropped frames, preview responsiveness, export
  duration and peak memory using the same clip and settings for comparisons.
- Repeat capture, system audio, cursor/click/typing zoom and export checks from
  this checklist. A headless package check alone is not a parity result.

### GPU selection and input reconnect regressions

- On a machine with multiple render nodes, verify export selects a working
  VA-API device even when the first node cannot encode the requested codec.
  `TARANTINO_VAAPI_DEVICE` explicitly pins a device; automatic discovery tests
  each node. Each native encoder probe has a five-second deadline.
- During compositor-metadata recording, unplug and reconnect the mouse, then
  click again. Click zoom should resume after device discovery (up to one second).
- Stop recording, click while idle, and start another recording. Idle clicks
  must not appear at the start of the new timeline. Input devices are closed
  between recordings; host device permissions are still required.

### Native caret tracking

Typing zoom uses AT-SPI caret geometry when the focused field and capture source
can be matched. The accessibility bridge is enabled automatically; this does not
enable a screen reader. No text contents are requested or saved. Unavailable or
ambiguous geometry retains the existing pointer-based fallback.

On a graphical Linux desktop with `kdialog` and XWayland, run the optional native
fixture test from `src-tauri`:

```sh
cargo test native_text_field_geometry -- --ignored --nocapture
cargo test native_gtk_text_field_geometry -- --ignored --nocapture
```

These tests create and close test-only input windows, read their caret bounds,
and verify that actual accessibility caret signals refine recording key events.
The Qt fixture checks twice the logical display scale. The GTK fixture requires
Python GTK3 and checks decorated XWayland pixel coordinates against a 1.45-scale
portal geometry. Also verify real recording with both monitor
and window sources: click a text field, type, move the pointer away, keep typing,
and inspect the editor and exported zoom. Switch to another window and confirm
its typing does not steer the captured window's zoom. Repeat with fractional
scaling and after resizing the captured window; the fixture alone does not prove
those end-to-end cases.

### Mouse, trackpad, and Steam Controller clicks

Linux uses physical evdev clicks together with desktop-generated X11/XWayland
clicks. Steam Input and remote-pointer emulation can generate desktop clicks
without an evdev mouse event. Both paths use the captured stream's coordinates;
overlapping reports from the two backends are deduplicated automatically.

The optional native regression below creates a test-only GTK window, positions
a temporary virtual pointer over it, and generates a desktop click through
XTest. It asserts that the production tracker and zoom processor create exactly
one automatic zoom block. It requires Python GTK3, `xdotool`, X11/XWayland, and
writable `/dev/uinput`; it does not require a physical click or typed input.

```sh
cargo test native_desktop_click_creates_zoom -- --ignored --nocapture
```

For the full recording check, test a physical mouse and the Steam Controller's
desktop trackpad separately. Each click should appear once in the mouse sidecar,
produce an automatic zoom block, and zoom in preview and export. Pure Wayland
sessions without XWayland still rely on the physical evdev path.

### Camera discovery and recording

The Linux camera picker uses the GStreamer V4L2 provider's capability probe,
not an unfiltered list of `/dev/video*` nodes. Metadata and output nodes must
not appear as cameras. Discovery runs on a blocking worker and does not start
camera streaming. `pnpm test:linux` checks provider availability; an empty list
on a machine without a camera does not verify webcam recording.

With a physical camera connected, refresh the camera menu, enable its preview,
record a short screen capture with the webcam enabled, and verify the webcam
in both the editor and exported video. Repeat with two cameras, switching the
selected camera, then unplug the selected camera and confirm an actionable
error rather than silently recording a different camera. These hardware checks
remain required before claiming webcam parity.

The native GTK and Qt caret tests also feed the accessibility-refined key event
through the production zoom processor with its default configuration. They
assert that a typing block is created at the normalized caret position rather
than the pointer. This covers the accessibility-to-zoom connection; it does not
replace recording, editor-preview and export checks with actual typing.

For KDE window capture, also test a stream whose portal response omits logical
size and a GTK entry that hides its pointer during typing. The recorder must
match the accessible client bounds to the actual stream pixels, retain the
recent captured click for initial field binding, and store changing caret
coordinates in `.mouse.json`. The typing block in `.auto_zoom.json`, editor
preview, and export must follow those caret coordinates. The GTK native test
covers missing portal size and a hidden pointer; the Qt test covers logical
coordinates at 2× scale.

### X11 without raw pointer access

On a full X11 session, click zoom uses the desktop RECORD service when the
extension is available, without requiring readable `/dev/input` devices. This
fallback is not used to claim full-desktop coverage on Wayland/XWayland.
Keyboard activity still uses the existing keyboard-access path.

CI runs the production click listener on an isolated Xvfb display:

```bash
xvfb-run -a env XDG_SESSION_TYPE=x11 cargo test --locked \
  --manifest-path src-tauri/Cargo.toml native_x11_click_without_raw_devices \
  -- --ignored --nocapture
```

The test must receive exactly one captured click without starting evdev readers.

## Local acceptance status — 20 October 2026

The current implementation was exercised on SteamOS Desktop Mode, KDE Plasma
Wayland with XWayland and AMD graphics. The working branch is
`linux-parity-defaults`; both `origin/linux-native-parity` and
`origin/linux-support` are ancestors of its base. No macOS-only source files were
changed. The following records distinguish actual checks from the remaining
session and hardware matrix.

| Requirement | Evidence and scope |
| --- | --- |
| Native SteamOS setup | Interactive setup exited successfully; Cargo 1.88, GCC 15.1, WebKitGTK, GStreamer, dbusmenu metadata and patchelf were available afterward. SteamOS read-only protection was restored. |
| Native development launch | `pnpm tauri dev` built and started the app using host dependencies, without Distrobox or temporary pkg-config paths. An app-only screenshot showed the complete recording toolbar. |
| Linux verification suite | `pnpm test:linux` passed after native setup with temporary dependency paths removed. |
| Default click and typing zoom | Recording starts tracking without an additional opt-in. Real window recordings produced click and caret-based typing zoom sidecars, editor previews and exported videos. Desktop-generated and virtual-device click paths were exercised. |
| Caret zoom export | The recorded typing fixture exported as H.264, 2026×1080, 60 fps, 1,236 frames and 20.6 seconds. Saved caret coordinates changed while the pointer remained stationary; preview and export followed the caret. |
| Full X11 click fallback | An isolated Xvfb test exercised the production listener and received exactly one click without starting raw-device readers. This is not a full X11 portal recording test. |
| Packaged media runtime | AppImage media checks passed on SteamOS and in Ubuntu 24.04 and Fedora containers with desktop runtime libraries. Ubuntu/Fedora checks preceded the latest X11 listener change; the latest AppImage also passed the SteamOS media check. Container checks do not prove desktop capture or performance parity. |
| Camera discovery | The native V4L2 provider check passed and found zero cameras. Webcam recording has not been verified. |

### Deferred live checks

These checks are explicitly unverified pending suitable hardware or sessions:

- Physical webcam preview, recording, export, camera switching and unplug recovery.
- Physical Steam Controller/trackpad input. Equivalent desktop-generated and
  virtual-device inputs passed; these do not certify the physical controller.
- End-to-end capture and export on GNOME, wlroots desktops and a full X11 desktop,
  plus SteamOS Gaming Mode. Only KDE Wayland was tested end to end here.
- Multi-monitor source changes, captured-window resizing and the remaining
  fractional-scaling/application combinations in the checklist above.
- Comparative capture/export CPU, GPU, memory and responsiveness measurements
  against macOS using the same content, settings and comparable hardware.
- Remote execution of the updated CI matrix. Local passing checks do not imply
  a successful CI run or validation on every listed distribution.

Keep these items open when reporting parity. Automatic input tracking is enabled
by default, but Wayland still requires input-device access, and X11 typing still
requires keyboard-device access. AT-SPI caret matching has a pointer-position
fallback when a field cannot be matched to the captured source. Enabling the
accessibility bridge does not override desktop security permissions.

### Forwarded Steam Input clicks

XWayland can forward XTEST input to the Wayland compositor without sending a
matching X server button event. Linux now also observes XTEST mouse-button
requests and deduplicates them against desktop and device events. Keyboard
requests are ignored. Captured-stream coordinates are still required.

The isolated regression `native_xtest_requests_without_server_events` starts
only the request listener and verifies a click press and release. The existing
`native_x11_click_without_raw_devices` test exercises the combined listeners and
requires exactly one click. Both are included in Linux CI.

For hardware acceptance, record a native Wayland window and click inside it
using the Steam Controller's R2 mouse binding. Confirm button events appear in
`.mouse.json` and produce `.auto_zoom.json` blocks. Also record an XWayland
window and verify each physical click appears only once. A trigger with a gamepad
binding must not be inferred to be a mouse click merely from its axis value.

On 20 October, a physical R2 diagnostic confirmed XTEST left-button press/release
requests with no corresponding X server button events for most presses. The
request-only native regression and combined-listener deduplication regression
both passed after the fix. A new saved recording with physical R2 clicks still
needs preview/export acceptance; the earlier hardware take contained zero clicks.

### Performance measurement — 20 October 2026

See [the saved measurements](linux-performance-2026-10-20.json) for raw resource
samples and scope. A release build recorded an animated 1920×1080 OpenGL window
on KDE Wayland with AMD VA-API. With the source synchronized to the 60 Hz display,
source rendering stayed near 60 fps, but the recorded clip averaged 51.2 fps.
The uncapped stress run captured 53.7 fps and reduced source throughput by about
19%. Neither result proves sustained 60 fps capture. This is an open performance
gap. The 57.8-second stress clip exported in 45.6 seconds (76.1 output fps);
export resampling must not be mistaken for unique capture frames. Tomb Raider
stalled before its benchmark, so these are synthetic results, not game FPS.

The later physical-controller recording `/tmp/2026-10-19T14-00-07.mp4` contained
21 clicks and generated 7 automatic zoom blocks after the XTEST request fix.
This verifies physical R2 input reaches zoom generation; visual preview/export
acceptance of that particular recording remains separate.

### Portal portability and recording failure regressions

`pnpm test:linux` includes headless regressions for asynchronous pipeline errors,
normal versus unexpected EOS, monitor lifetime, portal source filtering, and
persistent/nonpersistent window selections. It also checks unavailable portals
using an isolated, nonexistent session-bus address. Frontend tests cover a failure
arriving before the start command returns and starting another take after failure.

On the SteamOS device in Desktop Mode, run `pnpm test:linux`, then verify:

1. Choose a window, record, stop, and export. Repeat with a different window to
   verify that the saved portal token follows the new selection.
2. Restart an active recording and stop normally. Neither action should show a
   recording-failed warning.
3. During capture, stop sharing through the desktop's sharing controls. If the
   portal ends the stream with an error or EOS, the capture bar should return,
   the tray timer should stop, and the failure details should be visible. A new
   recording should then start successfully.
4. On a portal without persistence, selecting a window must succeed; Record may
   ask for the window again. A previous saved window must not be reused.
5. On a monitor-only portal, Window must be unavailable while Display still
   records. Without a ScreenCast portal, the capture bar must show recording as
   unavailable; after restoring the service, “Check again” must refresh sources.

Container tests do not validate SteamOS's live portal, AMD acceleration, or input
permissions. Keep the live Desktop Mode checks separate from Gaming Mode.
