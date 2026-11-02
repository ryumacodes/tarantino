#!/bin/sh
set -eu

if [ "$#" -ne 0 ]; then
  echo "error: this command builds the release app at its standard path; no arguments supported" >&2
  exit 1
fi

if [ "$(uname -s)" != Darwin ]; then
  echo "error: this build requires macOS" >&2
  exit 1
fi

cd "$(dirname "$0")/.."

# Keep a certificate-backed identity across rebuilds so privacy grants can
# match the app's designated requirement instead of one executable's hash.
APPLE_SIGNING_IDENTITY=${APPLE_SIGNING_IDENTITY:-Tarantino Local Development}
export APPLE_SIGNING_IDENTITY
if [ "$APPLE_SIGNING_IDENTITY" = "-" ]; then
  echo "error: use a code-signing certificate; ad-hoc builds change privacy identity" >&2
  exit 1
fi

identities=$(security find-identity -v -p codesigning)
if ! printf '%s\n' "$identities" | grep -F -- "$APPLE_SIGNING_IDENTITY" >/dev/null; then
  echo "error: signing identity unavailable: $APPLE_SIGNING_IDENTITY" >&2
  echo "Set APPLE_SIGNING_IDENTITY to an available code-signing certificate." >&2
  exit 1
fi

# JSON serialization handles certificate names without shell interpolation.
config=$(node -e 'process.stdout.write(JSON.stringify({bundle:{macOS:{signingIdentity:process.env.APPLE_SIGNING_IDENTITY}}}))')
pnpm tauri build --bundles app --config "$config"

bundle=src-tauri/target/release/bundle/macos/Tarantino.app
codesign --verify --deep --strict "$bundle"
helper="$bundle/Contents/MacOS/tarantino-speech"
if [ ! -x "$helper" ]; then
  echo "error: speech helper is missing from the app bundle" >&2
  exit 1
fi
codesign --verify --strict "$helper"
helper_identity=$(codesign -d -r- "$helper" 2>&1)
if printf '%s\n' "$helper_identity" | grep -F 'designated => cdhash' >/dev/null; then
  echo "error: speech helper has an unstable ad-hoc identity" >&2
  exit 1
fi
if ! codesign -dv "$helper" 2>&1 | grep -F 'Identifier=com.tarantino.speech' >/dev/null; then
  echo "error: speech helper identifier does not match" >&2
  exit 1
fi
requirement=$(codesign -d -r- "$bundle" 2>&1)
printf '%s\n' "$requirement"
if printf '%s\n' "$requirement" | grep -F 'designated => cdhash' >/dev/null; then
  echo "error: built app still has a build-specific ad-hoc identity" >&2
  exit 1
fi
echo "Certificate-signed app ready: $bundle"
