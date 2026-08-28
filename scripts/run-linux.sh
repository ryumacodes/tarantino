#!/bin/sh
set -eu

project_dir=$(pwd)

if [ -n "${CONTAINER_ID:-}" ]; then
  exec sh scripts/run-linux-container.sh
fi

if ! distrobox list 2>/dev/null | grep -q '[[:space:]]tarantino-dev[[:space:]]'; then
  echo "Linux environment is not installed. Run: pnpm setup:linux" >&2
  exit 1
fi

exec distrobox enter tarantino-dev -- bash -lc \
  'cd "$1" && exec sh scripts/run-linux-container.sh' bash "$project_dir"
