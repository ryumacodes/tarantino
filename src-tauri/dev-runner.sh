#!/bin/sh
set -eu

exe="$1"
shift

launch_mode="${TARANTINO_DEV_LAUNCH_MODE:-app}"
export TARANTINO_DEV_LAUNCH_MODE="$launch_mode"

if [ "$(uname -s)" = "Darwin" ] && [ "$launch_mode" = "raw" ]; then
  cat >&2 <<'INFO'
Tarantino dev: using raw launch mode.
Run from Terminal or iTerm2 when testing camera/screen permissions.
INFO
fi

if [ "$(uname -s)" = "Darwin" ] && [ "$launch_mode" = "app" ]; then
  app_dir="$(dirname "$exe")/Tarantino Dev.app"
  app_exe="$app_dir/Contents/MacOS/$(basename "$exe")"

  if [ -x "$app_exe" ]; then
    cp "$exe" "$app_exe"
    codesign --force --sign - "$app_exe" >/dev/null 2>&1 || true
    codesign --force --sign - --deep "$app_dir" >/dev/null 2>&1 || true
    echo "Tarantino dev: launching app bundle at $app_dir" >&2
    exec "$app_exe" "$@"
  fi

  cat >&2 <<INFO
Tarantino dev: app bundle launch requested, but $app_exe was not found.
Falling back to raw binary launch.
INFO
fi

exec "$exe" "$@"
