# Performance verification

macOS runtime behavior must remain unchanged by Linux fixes. A Linux test pass
does not establish macOS performance parity. Compare baseline and candidate on
the same Mac, and separately on each Linux GPU/driver combination. Different
hardware cannot be expected to produce identical timings or encoded bytes.

## Repeatable export benchmark

The opt-in `export_performance_benchmark` test runs the actual application export
path: decode, GPU composition, and encode. It generates a five-second
1920×1080/60fps fixture, applies padding, rounded corners and shadow, performs
one untimed warm-up and five measured exports, and validates every output's
dimensions and 300 decoded frames. Fixture generation and validation are outside
the timed region; encoder selection and GPU initialization are included.

Run on an idle, plugged-in machine with the same power mode and tool versions:

```sh
cargo test --release --locked --manifest-path src-tauri/Cargo.toml \
  export_performance_benchmark -- --ignored --nocapture > benchmark.log 2>&1
```

Run the same harness on the baseline and candidate revisions. When the baseline
predates the harness, copy `src-tauri/src/video_processing/performance_tests.rs`
into a separate baseline checkout and add the same `#[cfg(test)]` module
declaration to `video_processing/mod.rs`; do not copy production changes.
This harness is excluded from production binaries.

Optionally set `TARANTINO_BENCH_INPUT` to one shared 300-frame, 1080p/60fps MP4
to keep the input identical across systems. `TARANTINO_BENCH_RUNS` controls the
number of measured runs (default five). Keep these settings identical between
comparisons. Debug runs are functional smoke tests, not release benchmarks.

Save the full log, revision, OS version, CPU/GPU, driver, FFmpeg version and
power mode. The log identifies the GPU and encoder and reports `BENCH_RESULT`
with all durations and their median. Repeat baseline/candidate runs in alternating
order. Investigate any reproducible slowdown on macOS; there is no accepted
regression allowance. Compare output quality as well as speed.

This fixture does not benchmark every effect, preview responsiveness, recording,
audio, or webcam. Also export the same real project on both revisions, with its
cursor, zoom, motion blur, audio and webcam settings intact, and compare preview
smoothness, export time and the resulting media.

## Recording and Linux variation

`pnpm test:linux` now encodes synthetic live frames through the production
conversion/encoder/muxer pipeline, validates two consecutive recordings with
FFprobe and FFmpeg, and exercises installed software encoders. It cannot replace
the real portal tests in [the recording checklist](linux-recording-smoke-test.md).

For hardware coverage, run on AMD/Intel VA-API, NVIDIA, and a machine with only
software encoding. Include a multi-GPU machine and a VM/container with GPU
passthrough. Confirm the log chooses a working encoder; plugin installation or
a device node alone does not establish support. Test GNOME, KDE and wlroots
source selection, cancellation, stop and repeated sessions as documented.

For before/after recording measurements, use the same source workload, display
resolution, refresh rate and quality for 60 seconds. Record elapsed stop time,
CPU/memory usage, file duration and decoded frame count. Inspect timestamps and
playback for drops or gaps. Take equivalent measurements on the Mac; retain
its ScreenCaptureKit/VideoToolbox path.

## Verified in this workspace

The initial Linux export probe used a 64×64 frame. On the available AMD Ryzen Z1
Extreme VA-API encoder, FFmpeg rejected that size because its minimum is
128×128, causing automatic software fallback. The Linux-only probe now uses
320×240. Recording additionally probes the actual encoder pipeline and tries a
system-memory conversion path when VA surface conversion fails.

The frontend source-size correction removes blank lines only; the production
frontend artifacts were verified byte-identical before and after that edit.
macOS-only files and production macOS code paths were not changed.

Release benchmark measurements from 2026-09-05 are saved in
[the results file](benchmarks/linux-2026-09-05.json). On this AMD/Arch container,
the baseline selected software encoding and had a 5.95-second median. Two
candidate batches selected VA-API and had 3.27- and 5.39-second medians. Each
batch had five measured runs after warm-up, and every output validated. This
variation prevents claiming a stable percentage improvement or macOS parity.
The hardware selection fix is verified independently of those timings.

The Linux preview fallback now decodes the next frame batch while cached frames
continue to render. In a single 4K preview comparison on this host, observed
texture uploads over 4.2 seconds increased from 32 to 118, and the longest gap
fell from 1638ms to 82ms. See [the preview measurements](benchmarks/linux-preview-2026-09-05.json)
for native batch timings and limitations. This is not a macOS comparison.

[Real window capture and export](benchmarks/linux-real-window-2026-09-05.json)
were subsequently verified, including stream-relative click positions, static
window duration, decoded thumbnails, cursor rendering and visible zooms. The
Linux cursor reader adds a PipeWire metadata consumer without mapping video
pixels. It is not compiled into macOS builds. Other Linux desktop backends and
hardware combinations still require equivalent live testing.

## Remaining verification gates

### Latest Lenovo Go S Z1E benchmark

The current working tree was benchmarked on 2026-09-05 on **Lenovo Go S Z1E running SteamOS** (Arch development container). The device
name was supplied by the user; firmware reports Lenovo 83N6 and an AMD Ryzen
Z1 Extreme processor. The machine was on battery,
discharging at 92%, using the `custom` platform profile.

Release export of the five-second 1080p/60fps fixture took a median **3.391s**
(**88.47 fps**), ranging from 3.336s to 4.046s across five measured runs after
one warm-up. VA-API encoding was selected; all six outputs validated at 300
frames and 1920×1080.

Packaged debug preview decoding of 30 frames at 1280px width had first/repeat
batch medians of **1074/318.5ms** for a 1080p source and **487.5/540ms** for 4K.
Both batches are retained: the first 1080p measurements were variable. These
are decode/IPC timings, not playback frame rates. No matched baseline or Mac
comparison was run for these measurements.

[Machine-readable results](benchmarks/steamos-handheld-2026-09-05.json) ·
[Full export log](benchmarks/steamos-handheld-2026-09-05.log)

| Requirement | Current evidence | Still required |
| --- | --- | --- |
| Real Linux window recording, clicks, zoom and export | Passed on KDE Wayland; linked real-window report above | Equivalent live runs on GNOME/wlroots and other GPU families |
| Linux preview and export performance | Measured locally; linked reports above | Repeated comparable workloads across supported hardware |
| No macOS performance regression | Linux-only native changes and runtime-gated preview changes; macOS-only code unchanged | Baseline/candidate measurements on the same Mac |
| 1:1 performance parity | Not established by Linux-only measurements | Matched workload and output-quality measurements on both platforms |

The Linux Debian package explicitly depends on `libpipewire-0.3-0`, matching the
new cursor reader's `libpipewire-0.3.so.0` linkage. Local dependency resolution
and Tauri configuration inspection passed. The static-window source properties
(`keepalive-time` and `resend-last`) also exist with the required types in
[PipeWire 0.3.65](https://github.com/PipeWire/pipewire/blob/0.3.65/src/gst/gstpipewiresrc.c).
This source compatibility check does not replace running an older distribution.
