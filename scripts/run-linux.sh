#!/bin/sh
set -eu

project_dir=$(pwd)
container_name="${TARANTINO_CONTAINER_NAME:-tarantino-dev}"

if [ -n "${CONTAINER_ID:-}" ]; then
  exec sh scripts/run-linux-container.sh
fi

linux_id=unknown
if [ -r /etc/os-release ]; then
  . /etc/os-release
  linux_id=${ID:-unknown}
fi

if [ "$linux_id" = "steamos" ]; then
  if ! command -v distrobox >/dev/null 2>&1 || \
     ! distrobox list 2>/dev/null | grep -q "[[:space:]]${container_name}[[:space:]]"; then
    echo "The SteamOS development container is not installed. Run: pnpm setup:linux" >&2
    exit 1
  fi

  exec distrobox enter "$container_name" -- bash -lc \
    'cd "$1" && exec sh scripts/run-linux-container.sh' bash "$project_dir"
fi

# Conventional Linux distributions use their native toolchain directly.
for command in cargo pnpm gst-inspect-1.0 ffmpeg; do
  if ! command -v "$command" >/dev/null 2>&1; then
    echo "Linux dependency is missing: $command. Run: pnpm setup:linux" >&2
    exit 1
  fi
done

exec sh scripts/run-linux-container.sh
