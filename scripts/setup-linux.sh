#!/bin/sh
set -eu

container_name="${TARANTINO_CONTAINER_NAME:-tarantino-dev}"

if [ "$(uname -s)" != "Linux" ]; then
  echo "Tarantino's Linux setup can only run on Linux." >&2
  exit 1
fi

if [ ! -r /etc/os-release ]; then
  echo "Unable to identify this Linux distribution: /etc/os-release is missing." >&2
  exit 1
fi

# /etc/os-release is the freedesktop.org contract for distro identification.
. /etc/os-release
distro_id=${ID:-unknown}
distro_like=${ID_LIKE:-}

is_family() {
  [ "$distro_id" = "$1" ] || printf '%s\n' "$distro_like" | grep -Eq "(^|[[:space:]])$1([[:space:]]|$)"
}

run_as_root() {
  if [ "$(id -u)" -eq 0 ]; then
    "$@"
  elif command -v sudo >/dev/null 2>&1; then
    sudo "$@"
  else
    echo "This setup needs root access. Install sudo or run it as root." >&2
    exit 1
  fi
}

setup_steamos_container() {
  if ! command -v distrobox >/dev/null 2>&1; then
    echo "SteamOS is immutable, so Tarantino uses Distrobox for development." >&2
    echo "Install Distrobox, then run pnpm setup:linux again." >&2
    exit 1
  fi

  if ! distrobox list 2>/dev/null | grep -q "[[:space:]]${container_name}[[:space:]]"; then
    distrobox create --name "$container_name" --image docker.io/library/archlinux:latest --yes
  fi

  distrobox enter "$container_name" -- sudo pacman -Syu --noconfirm --needed \
    base-devel git curl wget file pkgconf openssl clang rustup nodejs pnpm \
    webkit2gtk-4.1 libayatana-appindicator librsvg xdotool alsa-lib \
    ffmpeg gstreamer gst-plugins-base gst-plugins-good gst-plugins-bad \
    gst-plugins-ugly gst-plugin-pipewire gst-plugin-va libva-utils \
    pipewire xdg-desktop-portal
}

if [ "$distro_id" = "steamos" ]; then
  setup_steamos_container
elif is_family arch; then
  run_as_root pacman -Syu --noconfirm --needed \
    base-devel git curl wget file pkgconf openssl clang rustup nodejs pnpm \
    webkit2gtk-4.1 libayatana-appindicator librsvg xdotool alsa-lib \
    ffmpeg gstreamer gst-plugins-base gst-plugins-good gst-plugins-bad \
    gst-plugins-ugly gst-plugin-pipewire gst-plugin-va libva-utils \
    pipewire xdg-desktop-portal
elif is_family debian || [ "$distro_id" = "ubuntu" ] || [ "$distro_id" = "linuxmint" ]; then
  run_as_root apt-get update
  run_as_root apt-get install -y \
    build-essential git curl wget file pkg-config libssl-dev clang nodejs npm \
    libwebkit2gtk-4.1-dev libayatana-appindicator3-dev librsvg2-dev \
    libxdo-dev libasound2-dev libgstreamer1.0-dev \
    libgstreamer-plugins-base1.0-dev libavcodec-dev libavformat-dev \
    libavutil-dev libavfilter-dev libavdevice-dev libswscale-dev \
    libswresample-dev ffmpeg gstreamer1.0-tools gstreamer1.0-pipewire \
    gstreamer1.0-plugins-base gstreamer1.0-plugins-good \
    gstreamer1.0-plugins-bad gstreamer1.0-plugins-ugly \
    gstreamer1.0-vaapi vainfo pipewire xdg-desktop-portal
elif is_family fedora || [ "$distro_id" = "rhel" ]; then
  run_as_root dnf install -y \
    gcc gcc-c++ make git curl wget file pkgconf-pkg-config openssl-devel \
    clang nodejs npm webkit2gtk4.1-devel libappindicator-gtk3-devel \
    librsvg2-devel libxdo-devel alsa-lib-devel gstreamer1-devel \
    gstreamer1-plugins-base-devel ffmpeg-free ffmpeg-free-devel \
    gstreamer1-plugins-base-tools gstreamer1-plugins-good \
    gstreamer1-plugins-bad-free gstreamer1-plugin-openh264 \
    gstreamer1-vaapi libva-utils pipewire-gstreamer pipewire \
    xdg-desktop-portal
elif is_family suse || [ "$distro_id" = "opensuse-tumbleweed" ] || [ "$distro_id" = "opensuse-leap" ]; then
  run_as_root zypper --non-interactive refresh
  run_as_root zypper --non-interactive install --no-recommends \
    gcc gcc-c++ make git curl wget file pkg-config clang nodejs npm \
    libopenssl-devel 'pkgconfig(webkit2gtk-4.1)' libappindicator3-1 \
    librsvg-devel xdotool alsa-devel gstreamer-devel \
    gstreamer-plugins-base-devel gstreamer-utils gstreamer-plugins-base \
    gstreamer-plugins-good gstreamer-plugins-bad gstreamer-plugins-ugly \
    gstreamer-plugin-pipewire ffmpeg-7 'pkgconfig(libavcodec)' \
    'pkgconfig(libavformat)' 'pkgconfig(libavutil)' 'pkgconfig(libavfilter)' \
    'pkgconfig(libavdevice)' 'pkgconfig(libswscale)' \
    'pkgconfig(libswresample)' gstreamer-plugins-vaapi libva-utils \
    pipewire xdg-desktop-portal
else
  echo "Unsupported automatic setup for ${PRETTY_NAME:-$distro_id}." >&2
  echo "Install the dependencies listed in README.md, then run pnpm tauri:dev." >&2
  exit 1
fi

if [ "$distro_id" != "steamos" ]; then
  if ! command -v cargo >/dev/null 2>&1; then
    echo "Rust is still missing. Install Rust 1.88 or newer with rustup." >&2
    exit 1
  fi
  if ! command -v pnpm >/dev/null 2>&1; then
    echo "pnpm is still missing. Install pnpm 11 with Corepack or npm." >&2
    exit 1
  fi
fi

echo "Linux environment ready for ${PRETTY_NAME:-$distro_id}. Run: pnpm tauri:dev"
