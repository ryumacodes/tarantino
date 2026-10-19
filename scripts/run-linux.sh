#!/bin/sh
set -eu

# Rustup installs per user; desktop launchers may not load shell profiles.
tarantino_js_root=${XDG_DATA_HOME:-"$HOME/.local/share"}/tarantino/toolchain
PATH="$tarantino_js_root/bin:$tarantino_js_root/global/bin:${CARGO_HOME:-$HOME/.cargo}/bin:$PATH"
export PATH

# Use the environment the user launched from, including on SteamOS.
# Entering a development container is an explicit user choice.
for command in cargo pnpm cc pkg-config gst-inspect-1.0 ffmpeg ffprobe; do
  if ! command -v "$command" >/dev/null 2>&1; then
    echo "Linux development dependency is missing in this environment: $command" >&2
    echo "Install the native build dependencies here, or launch a built Tarantino AppImage." >&2
    exit 1
  fi
done

# Desktop backend selection happens in Linux native startup for both
# development runs and packaged applications.

# A previous development run may have left the Vite interface available after
# the native process exited. Reuse that live server instead of failing because
# its fixed port is occupied.
if curl -fsS --max-time 2 http://127.0.0.1:2703/ 2>/dev/null | grep -qi '<title>Tarantino</title>'; then
  echo "Reusing the existing Tarantino development interface on port 2703."
  cd src-tauri
  exec cargo run --no-default-features
fi

exec pnpm tauri dev
