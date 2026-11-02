#!/bin/sh
set -eu
cd "$(dirname "$0")/.."
binary=$(mktemp "${TMPDIR:-/tmp}/tarantino-audio-test.XXXXXX")
trap 'rm -f "$binary"' EXIT HUP INT TERM
xcrun clang++ -std=c++17 -Wall -Wextra -Werror \
  src-tauri/src/capture/backends/macos/audio_pcm_test.cpp -o "$binary"
"$binary"
