#!/bin/sh
set -eu

project_dir=$(pwd)
test_command='pnpm test:unit && pnpm build && cargo test --manifest-path src-tauri/Cargo.toml'

if [ -n "${CONTAINER_ID:-}" ]; then
  exec bash -lc "$test_command"
fi

exec distrobox enter tarantino-dev -- bash -lc \
  'cd "$1" && pnpm test:unit && pnpm build && cargo test --manifest-path src-tauri/Cargo.toml' \
  bash "$project_dir"
