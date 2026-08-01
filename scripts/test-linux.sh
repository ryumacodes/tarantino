#!/bin/sh
set -eu

run_cargo() {
  cargo_log=$(mktemp)
  if "$@" >"$cargo_log" 2>&1; then
    cat "$cargo_log"
    rm -f "$cargo_log"
    return 0
  fi

  cat "$cargo_log" >&2
  if [ "${GITHUB_ACTIONS:-false}" = "true" ]; then
    tail -n 120 "$cargo_log" | while IFS= read -r line; do
      escaped=$(printf '%s' "$line" | sed 's/%/%25/g; s/\r/%0D/g')
      printf '::error title=Linux native build::%s\n' "$escaped"
    done
  fi
  rm -f "$cargo_log"
  return 1
}

if [ "$(uname -s)" != "Linux" ]; then
  echo "error: this verification suite must run on Linux" >&2
  exit 1
fi

for command in pnpm cargo gst-inspect-1.0 ffmpeg ffprobe; do
  if ! command -v "$command" >/dev/null 2>&1; then
    echo "error: required command is missing: $command" >&2
    exit 1
  fi
done

for plugin in pipewiresrc x264enc h264parse mp4mux; do
  if ! gst-inspect-1.0 "$plugin" >/dev/null 2>&1; then
    echo "error: required GStreamer plugin is missing: $plugin" >&2
    exit 1
  fi
done

echo "[1/7] Repository hygiene"
pnpm run check:repo-hygiene

echo "[2/7] Frontend unit tests"
pnpm run test:unit

echo "[3/7] Frontend type-check and production build"
pnpm run build

echo "[4/7] Rust formatting"
cargo fmt --manifest-path src-tauri/Cargo.toml --all -- --check

echo "[5/7] Rust and Linux native unit tests"
run_cargo cargo test --locked --all-targets --manifest-path src-tauri/Cargo.toml

echo "[6/7] Rust and Linux native compile check"
run_cargo cargo check --locked --all-targets --manifest-path src-tauri/Cargo.toml

echo "[7/7] Linux capture runtime plugins"
gst-inspect-1.0 pipewiresrc x264enc h264parse mp4mux >/dev/null

echo "Linux verification passed. A logged-in graphical session is still required for the portal recording smoke test."
