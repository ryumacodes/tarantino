#!/bin/sh
set -eu

if [ "$(uname -s)" != Linux ]; then
  echo "This native editor E2E suite runs on Linux." >&2
  exit 1
fi
for command in pnpm cargo python3 ffmpeg ffprobe WebKitWebDriver; do
  command -v "$command" >/dev/null 2>&1 || { echo "Missing E2E dependency: $command" >&2; exit 1; }
done
python3 -c 'from PIL import Image'
driver=${TARANTINO_E2E_DRIVER:-tauri-driver}
command -v "$driver" >/dev/null 2>&1 || { echo "Install tauri-driver with cargo install tauri-driver --locked" >&2; exit 1; }
results=${TARANTINO_E2E_OUTPUT_DIR:-$(mktemp -d "${TMPDIR:-/tmp}/tarantino-e2e.XXXXXX")}
mkdir -p "$results"
results=$(cd "$results" && pwd)
port=${TARANTINO_E2E_PORT:-4446}
native_port=$((port + 1))

pnpm build
cargo build --locked --manifest-path src-tauri/Cargo.toml
TARANTINO_E2E_FIXTURE_DIR="$results/fixture" cargo test --locked \
  --manifest-path src-tauri/Cargo.toml linux_editor_e2e_fixture -- --ignored --nocapture

# Match the app's KDE Wayland launch workaround. Other desktops retain their backend.
case "${XDG_CURRENT_DESKTOP:-}" in
  *KDE*|*kde*|*Plasma*|*plasma*)
    if [ -n "${WAYLAND_DISPLAY:-}" ]; then export GDK_BACKEND=${GDK_BACKEND:-x11}; fi ;;
esac
"$driver" --port "$port" --native-port "$native_port" >"$results/driver.log" 2>&1 &
driver_pid=$!
trap 'kill "$driver_pid" 2>/dev/null || true; wait "$driver_pid" 2>/dev/null || true' EXIT INT TERM
python3 - "$port" <<'PY'
import sys,time,urllib.request
for attempt in range(50):
    try:
        urllib.request.urlopen('http://127.0.0.1:'+sys.argv[1]+'/status',timeout=1).close()
        break
    except OSError:
        time.sleep(.1)
else:
    raise SystemExit('WebDriver did not start')
PY
python3 scripts/e2e-linux.py --app "$(pwd)/src-tauri/target/debug/tarantino" \
  --fixture "$results/fixture/recording.mp4" --output "$results" \
  --webdriver "http://127.0.0.1:$port"
echo "Linux editor E2E passed. Screenshots and results: $results"
