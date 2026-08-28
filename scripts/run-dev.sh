#!/bin/sh
set -eu

platform=$(uname -s)

case "$platform" in
  Darwin)
    exec pnpm tauri dev
    ;;
  Linux)
    exec sh scripts/run-linux.sh
    ;;
  *)
    echo "Tarantino development is not supported on $platform yet." >&2
    exit 1
    ;;
esac
