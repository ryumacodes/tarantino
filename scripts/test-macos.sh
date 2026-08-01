#!/bin/sh
set -eu

if [ "$(uname -s)" != "Darwin" ]; then
  echo "error: this verification suite currently supports macOS only" >&2
  exit 1
fi

if ! command -v pnpm >/dev/null 2>&1; then
  echo "error: pnpm is required (see packageManager in package.json)" >&2
  exit 1
fi

if ! command -v cargo >/dev/null 2>&1; then
  echo "error: Rust and cargo are required (https://rustup.rs)" >&2
  exit 1
fi

echo "[1/6] Repository hygiene"
pnpm run check:repo-hygiene

echo "[2/6] Frontend unit tests"
pnpm run test:unit

echo "[3/6] Frontend type-check and production build"
pnpm run build

echo "[4/6] Rust formatting"
cargo fmt --manifest-path src-tauri/Cargo.toml --all -- --check

echo "[5/6] Rust and macOS native unit tests"
cargo test --locked --all-targets --manifest-path src-tauri/Cargo.toml

echo "[6/6] Rust and macOS native compile check"
cargo check --locked --all-targets --manifest-path src-tauri/Cargo.toml

echo "macOS verification passed. Live screen, camera, microphone, and system-audio permissions still require a manual smoke test."
