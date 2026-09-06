#!/bin/sh
set -eu

backend=${1:-}
if [ -z "$backend" ]; then
  echo "usage: $0 <gnome|kde|wlr>" >&2
  exit 2
fi

case "$backend" in
  gnome|kde|wlr) ;;
  *)
    echo "error: unsupported portal backend: $backend" >&2
    exit 2
    ;;
esac

portal_file=""
for directory in /usr/share/xdg-desktop-portal/portals /usr/local/share/xdg-desktop-portal/portals; do
  candidate="$directory/$backend.portal"
  if [ -f "$candidate" ]; then
    portal_file=$candidate
    break
  fi
done

if [ -z "$portal_file" ]; then
  echo "error: $backend desktop portal descriptor is not installed" >&2
  exit 1
fi

if ! grep -Eq '^Interfaces=.*org\.freedesktop\.impl\.portal\.ScreenCast' "$portal_file"; then
  echo "error: $portal_file does not advertise the ScreenCast interface" >&2
  exit 1
fi

for command in gst-inspect-1.0 dbus-run-session; do
  if ! command -v "$command" >/dev/null 2>&1; then
    echo "error: required portal runtime command is missing: $command" >&2
    exit 1
  fi
done

for plugin in pipewiresrc h264parse mp4mux; do
  if ! gst-inspect-1.0 "$plugin" >/dev/null 2>&1; then
    echo "error: required portal recording plugin is missing: $plugin" >&2
    exit 1
  fi
done

echo "$backend portal advertises ScreenCast and the PipeWire recording runtime is installed."
