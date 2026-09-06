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

Check that the chosen source is correct, cursor visibility matches the setting,
the output dimensions are sensible, timestamps increase normally, and stopping
produces a playable finalized file. System-audio capture is not supported yet
and should produce a clear error when enabled.
