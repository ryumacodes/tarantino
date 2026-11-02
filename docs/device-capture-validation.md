# Device capture validation

The macOS Device picker enumerates USB muxed screen sources separately from webcam inputs. Discovery uses CoreMediaIO's screen-device opt-in and AVFoundation. Recording feeds BGRA frames and their source timestamps into the existing VideoToolbox/MP4 pipeline. Reopen the Device picker to refresh discovery after connecting/unlocking a device.

## Current limits

- A connected, unlocked, trusted iPhone/iPad is required for hardware validation. No device was available for the initial implementation.
- USB device audio is not implemented; the system-audio control is unavailable in Device mode. Microphone recording remains a separate input.
- Keep orientation fixed during a take. A source dimension change is detected and fails the recording rather than feeding mismatched frames into the encoder; dynamic rotation is not implemented.
- The selected frame rate reaches the recording configuration. USB screen sources control their actual delivery cadence; a requested 120 FPS does not guarantee 120 captured frames per second.
- Device touch tracking, phone auto-zoom, device disconnect notifications, and hardware-tested color/orientation fidelity remain outstanding.

## Hardware checklist

1. With no device attached, open Device: expect connection/unlock/trust guidance and no webcam entries.
2. Connect and trust a phone, reopen Device, select it, then record a moving clock and scrolling text for 30 seconds.
3. Stop and verify output dimensions, duration, frame timestamps, readable text and editor playback. Confirm no Mac cursor events were collected.
4. Repeat at requested 30/60/120 FPS; measure delivered frames, dropped frames, CPU/GPU use, memory, and stop latency. Compare with the display-capture baseline using the same workload.
5. Record with an external microphone and camera overlay; inspect source alignment at the beginning and end.
6. Disconnect before start: expect an actionable error. Disconnect or rotate during a take: inspect finalization and error recovery; these paths are not certified yet.
7. Switch back to display/window recording and verify capture, audio, automatic zoom and cursor behavior.

## Automated checks

New unit coverage verifies explicit frame-rate overrides, legacy quality defaults, and speed-aware clip splitting. Frontend build, Rust compile, formatting, repository hygiene and existing unit suites must pass. These checks do not certify USB hardware behavior or perceptual smoothness.

API reference: [Apple CoreMediaIO screen-device property](https://developer.apple.com/documentation/coremediaio/kcmiohardwarepropertyallowscreencapturedevices).
