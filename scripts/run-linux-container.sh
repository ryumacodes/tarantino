#!/bin/sh
set -eu

# KDE Wayland intentionally ignores application-controlled keep-above and
# activation requests for ordinary Wayland surfaces. The capture bar needs
# those window-manager semantics, so run only Tarantino's GTK shell through
# XWayland. Capture and encoding remain native PipeWire/VA-API.
export GDK_BACKEND=x11

# A previous development run may have left the Vite interface available after
# the native process exited. Reuse that live server instead of failing because
# its fixed port is occupied.
if curl -fsS --max-time 2 http://127.0.0.1:2703/ 2>/dev/null | grep -qi '<title>Tarantino</title>'; then
  echo "Reusing the existing Tarantino development interface on port 2703."
  cd src-tauri
  exec cargo run --no-default-features
fi

exec pnpm tauri dev
