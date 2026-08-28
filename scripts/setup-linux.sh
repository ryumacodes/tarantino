#!/bin/sh
set -eu

container_name="tarantino-dev"

if ! command -v distrobox >/dev/null 2>&1; then
  echo "Tarantino Linux setup requires Distrobox." >&2
  exit 1
fi

if ! distrobox list 2>/dev/null | rg -q "[[:space:]]${container_name}[[:space:]]"; then
  distrobox create \
    --name "$container_name" \
    --image docker.io/library/archlinux:latest \
    --yes
fi

distrobox enter "$container_name" -- sudo pacman -Syu --noconfirm --needed \
  webkit2gtk-4.1 \
  base-devel \
  curl \
  wget \
  file \
  openssl \
  appmenu-gtk-module \
  libappindicator-gtk3 \
  librsvg \
  xdotool \
  xorg-xrandr \
  rustup \
  nodejs \
  npm \
  pnpm \
  ffmpeg \
  gstreamer \
  gst-plugin-pipewire \
  gst-plugins-base \
  gst-plugins-good \
  gst-plugins-bad \
  pipewire \
  libpulse

echo "Linux environment ready. Run: pnpm tauri:dev:linux"
