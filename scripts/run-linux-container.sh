#!/bin/sh
set -eu

# KDE Wayland intentionally ignores application-controlled keep-above and
# activation requests for ordinary Wayland surfaces. Use XWayland for the GTK
# shell only on that combination. Other desktops retain their native backend.
desktop=${XDG_CURRENT_DESKTOP:-${DESKTOP_SESSION:-}}
case "$desktop" in
  *KDE*|*kde*|*Plasma*|*plasma*)
    if [ -n "${WAYLAND_DISPLAY:-}" ]; then
      export GDK_BACKEND=x11
    fi
    ;;
esac

# A previous development run may have left the Vite interface available after
# the native process exited. Reuse that live server instead of failing because
# its fixed port is occupied.
if curl -fsS --max-time 2 http://127.0.0.1:2703/ 2>/dev/null | grep -qi '<title>Tarantino</title>'; then
  echo "Reusing the existing Tarantino development interface on port 2703."
  cd src-tauri
  exec cargo run --no-default-features
fi

exec pnpm tauri dev
