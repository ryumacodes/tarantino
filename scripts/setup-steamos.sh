#!/bin/sh
set -eu

if [ -f /.flatpak-info ]; then
  echo "Run native SteamOS setup from Konsole in the project directory." >&2
  exit 1
fi
. /etc/os-release
if [ "${ID:-}" != steamos ]; then
  echo "This installer is for SteamOS. Use scripts/setup-linux.sh." >&2
  exit 1
fi

run_as_root() {
  if [ "$(id -u)" -eq 0 ]; then "$@"; else sudo "$@"; fi
}

# Restore the original filesystem mode on success, failure or interruption.
readonly_status=$(steamos-readonly status)
restore_readonly=0
case "$readonly_status" in
  *disabled*) ;;
  *enabled*) restore_readonly=1 ;;
  *) echo "Unrecognized read-only status: $readonly_status" >&2; exit 1 ;;
esac
cleanup() {
  setup_result=$?
  trap - EXIT
  if [ "$restore_readonly" -eq 1 ]; then
    if ! run_as_root steamos-readonly enable; then
      echo "Could not restore read-only mode. Run sudo steamos-readonly enable." >&2
      exit 1
    fi
  fi
  exit "$setup_result"
}
trap cleanup EXIT
trap 'exit 130' INT
trap 'exit 143' TERM
if [ "$restore_readonly" -eq 1 ]; then
  run_as_root steamos-readonly disable
fi

# Use the installed SteamOS repositories and signing keys. Do not replace
# repository configuration or perform a rolling Arch system upgrade.
run_as_root pacman-key --init
run_as_root pacman-key --populate
run_as_root pacman -S --needed \
  base-devel gcc clang pkgconf git curl wget file patchelf nodejs pnpm python \
  openssl webkit2gtk-4.1 libayatana-appindicator librsvg xdotool alsa-lib \
  ffmpeg gstreamer gst-plugins-base gst-plugins-good gst-plugins-bad \
  gst-plugins-ugly gst-plugin-pipewire gst-plugin-va libva-utils \
  pipewire xdg-desktop-portal

# SteamOS can retain package records while omitting development headers.
# Reinstall these only when the C toolchain's fundamental headers are absent.
if [ ! -f /usr/include/stdio.h ] || [ ! -f /usr/include/linux/types.h ]; then
  run_as_root pacman -S glibc linux-api-headers
fi

. "$(dirname "$0")/setup-linux-rust.sh"
. "$(dirname "$0")/setup-linux-js.sh"
cc --version
python3 "$(dirname "$0")/repair-steamos-dev.py"
echo "Native SteamOS dependencies found. Run pnpm install --frozen-lockfile, then pnpm tauri dev."
echo "SteamOS updates may require rerunning this setup."
