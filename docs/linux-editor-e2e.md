# Linux editor end-to-end tests

Run from the repository root in a logged-in Linux desktop, or in its development
container with access to the display and GPU:

```sh
sh scripts/test-linux-e2e.sh
```

In addition to the normal Linux development dependencies, this requires
`tauri-driver`, `WebKitWebDriver`, Python 3 and Pillow. Install the Tauri driver
with `cargo install tauri-driver --locked`. On Arch, `WebKitWebDriver` is shipped
in `webkitgtk-6.0`, and Pillow in `python-pillow`. The driver operates the actual
application webview, following [Tauri's native WebDriver setup](https://v2.tauri.app/develop/tests/webdriver/).

The runner builds the frontend and native app, records six seconds of synthetic
SMPTE frames through the Linux GStreamer recording pipeline, and feeds a known
click into the actual recording finalization and zoom-analysis code. It opens
that MP4 in the real editor and checks:

- Timeline thumbnails have decoded successfully.
- The preview has visible colored pixels, not just a mounted canvas.
- The generated zoom changes the visible video framing during playback.
- Video frames keep changing with the zoom removed, so a moving zoom cannot
  conceal a frozen source frame.
- Seeking to the end displays the last decodable frame.
- Reopening the finalized recording restores thumbnails, zooms and preview.

All editor operations use native IPC and UI controls. No frontend store or IPC
responses are mocked. Screenshots and `results.json` are retained in a temporary
results directory printed by the runner. Failures save a screenshot and page
source. Set `TARANTINO_E2E_OUTPUT_DIR` to choose the output directory,
`TARANTINO_E2E_DRIVER` for a driver outside PATH, or `TARANTINO_E2E_PORT` to change
the default port 4446 (the native driver uses the following port).

The synthetic source makes the editor checks repeatable. It does not automate
the desktop's permission picker or physical mouse devices; those still require
[the live recording checklist](linux-recording-smoke-test.md).

## Regressions covered

The Linux preview fallback previously allocated a 2×2 canvas texture before a
decoded frame arrived. Resizing that canvas after GPU allocation caused a
rejected texture upload and a black preview. Texture allocation now waits for
the first frame and replaces storage when dimensions change.

The fallback also requested frames only when paused. When WebKit could not decode
the video, the editor's fallback clock advanced while the picture stayed frozen.
Frame requests now follow that clock during playback as well. End-of-video
requests are clamped to the last valid frame instead of seeking beyond EOF.

These runtime changes are confined to `LinuxNativeVideoOverlay`, which is mounted
only on Linux. The macOS video material and native macOS code are unchanged.

The packaged Linux build also avoids Vite's special `-legacy` chunk-name path,
which injected the main editor CSS inline instead of emitting a stylesheet.
Tauri's CSP rejected that injected CSS. The Linux preview loading state no
longer uses a font-backed Three.js label: its blocked font worker suspended the
canvas before the video-loading effect could run. Both changes retain the
existing macOS build and rendering paths.

## Real window verification (2026-09-05)

The WebDriver harness navigates the capture-bar webview to the editor URL.
That reuses the capture bar's native window flags, including always-on-top;
it must not be used to judge the production editor's stacking behavior.
For interactive testing, use the editor window created after recording stops.
If temporarily reusing the capture-bar window, clear its keep-above flag through
the window manager before leaving it visible. The production editor is a separate
normal window; capture controls retain their foreground behavior.

Follow-up windowed-editor checks are recorded in
[the windowed results](benchmarks/linux-windowed-editor-2026-09-05.json).
The real recording played at 1000×700, 1280×840 and 1600×1000, with decoded
thumbnails, visible controls and a resized preview. Native clicks entered and
exited fullscreen. Timeline zoom/fit/collapse/expand and all six settings tabs
were exercised. Focus and Desktop layout exports both decoded to 733 frames
over 12.217 seconds, at 1310×1080 and 1920×1080 respectively; rendered frames
were inspected for the real window, zoom, cursor and click ripple. Audio capture,
webcam and every individual effect setting are outside this follow-up's coverage.

The initial tab-opening check missed a cursor-panel styling failure: packaged
Linux CSP blocked its runtime React style element. The panel now loads the same
styles from an external CSS file on Linux; the Mac inline-style path is retained.
The E2E suite checks computed panel/toggle layout to catch this failure.
[Native cursor-control checks](benchmarks/linux-cursor-panel-2026-09-05.json)
also passed for all five styles, click effects, pointer/hide toggles, size/reset,
and Rotation/Advanced expansion.

The synthetic suite was followed by a real KDE Wayland window recording, using
the desktop portal's selected browser window. OS-level mouse input exercised the
normal input path; the test did not create mouse or zoom sidecars by hand.
The final 12-second run produced a 12.219-second, 1378×1137 recording with
338 decoded frames. The editor decoded 13 thumbnails, displayed the real window,
and visibly followed the generated zoom and cursor. Export through the editor's
Export button produced a 12.217-second video with 733 decoded frames at 60fps.
See [the real-window results](benchmarks/linux-real-window-2026-09-05.json).

This test exposed bugs the synthetic fixture could not detect:

- An absolute HID device could be a touchpad or controller, leaving every mouse
  click at the same position. Linux capture now consumes the compositor's
  stream-relative cursor metadata and reads button events from readable mice.
- GStreamer's image path did not reliably deliver cursor-only updates on static
  content. A dedicated PipeWire metadata consumer handles those updates without
  mapping or copying video pixels. It uses a separate portal connection, and
  stops its worker before releasing the recording session.
- Portal window placeholders reported zero dimensions and were incorrectly used
  as a crop rectangle. Stream-local mouse data now uses the captured dimensions
  with no desktop crop offset.
- Static-window buffers could reuse their original image timestamps. Linux now
  stamps live buffers with elapsed recording time, retains static frames with
  keepalives, and sends the final held frame on stop. Otherwise the saved video
  could collapse to one frame and its later zooms would be discarded.

The verified click positions were (1250, 400), (1250, 700), and (1000, 950),
relative to the 1378×1137 window. Both preview and export showed the cursor and
zoom. These capture changes and their native build dependencies are Linux-only;
macOS capture, encoding and rendering branches remain unchanged. This is live
coverage of one KDE/GPU combination, not proof of parity across all desktops.
