#!/bin/sh
set -eu
# Sourced by Linux installers so Cargo is available in the same setup process.
# Keep a suitable existing compiler; do not downgrade a user's newer toolchain.
tarantino_cargo_home=${CARGO_HOME:-"$HOME/.cargo"}
PATH="$tarantino_cargo_home/bin:$PATH"
export PATH

linux_rust_ready() {
  command -v cargo >/dev/null 2>&1 && cargo --version >/dev/null 2>&1 &&
    rustc --version 2>/dev/null | awk '
      { split($2, version, "."); ok = version[1] > 1 || (version[1] == 1 && version[2] >= 88) }
      END { exit !ok }'
}

if ! linux_rust_ready; then
  echo "Installing Rust 1.88 with Cargo for native Linux development."
  if command -v rustup >/dev/null 2>&1; then
    rustup toolchain install 1.88.0 --profile minimal --component rustfmt
    rustup default 1.88.0
  else
    # Download completely before executing; curl errors must not be hidden by
    # a successful shell on the right-hand side of a pipeline.
    (
      tarantino_rust_installer=$(mktemp)
      trap 'rm -f "$tarantino_rust_installer"' EXIT
      trap 'exit 130' INT
      trap 'exit 143' TERM
      curl --proto '=https' --tlsv1.2 -fsS https://sh.rustup.rs -o "$tarantino_rust_installer"
      sh "$tarantino_rust_installer" -y --profile minimal --default-toolchain 1.88.0 --component rustfmt
    )
  fi
fi
if ! linux_rust_ready; then
  echo "Rust 1.88 or newer is still unavailable. Check rustup overrides and CARGO_HOME." >&2
  exit 1
fi
cargo --version
