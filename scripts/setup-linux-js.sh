#!/bin/sh
# Sourced by Linux setup. Install user tools without replacing distro packages.
set -eu
tarantino_js_root=${XDG_DATA_HOME:-"$HOME/.local/share"}/tarantino/toolchain
PATH="$tarantino_js_root/bin:$tarantino_js_root/global/bin:$PATH"
export PATH

linux_node_ready() {
  command -v node >/dev/null 2>&1 && node -e '
    const [major, minor] = process.versions.node.split(".").map(Number);
    process.exit(major > 22 || (major === 22 && minor >= 13) ? 0 : 1);
  ' >/dev/null 2>&1
}

if ! linux_node_ready; then
  # Official LTS binary, verified before extraction. Keep system Node untouched.
  case "$(uname -m)" in
    x86_64) tarantino_node_arch=x64 ;;
    aarch64|arm64) tarantino_node_arch=arm64 ;;
    *) echo "Install Node 22.13 or newer for this architecture, then rerun setup." >&2; exit 1 ;;
  esac
  tarantino_node_name=node-v24.22.0-linux-$tarantino_node_arch
  mkdir -p "$tarantino_js_root/bin"
  (
    tarantino_node_temp=$(mktemp -d "$tarantino_js_root/.node-install.XXXXXX")
    trap 'rm -rf "$tarantino_node_temp"' EXIT
    trap 'exit 130' INT
    trap 'exit 143' TERM
    cd "$tarantino_node_temp"
    tarantino_node_url=https://nodejs.org/dist/v24.22.0
    curl --proto '=https' --tlsv1.2 -fsS "$tarantino_node_url/SHASUMS256.txt" -o SHASUMS256.txt
    curl --proto '=https' --tlsv1.2 -fsS "$tarantino_node_url/$tarantino_node_name.tar.gz" -o "$tarantino_node_name.tar.gz"
    awk -v file="$tarantino_node_name.tar.gz" '$2 == file { print; found=1 } END { exit !found }' SHASUMS256.txt > selected.sha256
    sha256sum -c selected.sha256
    tar -xzf "$tarantino_node_name.tar.gz"
    # Do not overwrite a pre-existing installation with unknown contents.
    if [ -e "$tarantino_js_root/$tarantino_node_name" ]; then
      echo "Existing Node installation is unusable: $tarantino_js_root/$tarantino_node_name" >&2
      exit 1
    fi
    mv "$tarantino_node_name" "$tarantino_js_root/"
    ln -s "$tarantino_js_root/$tarantino_node_name/bin/node" "$tarantino_js_root/bin/node"
    ln -s "$tarantino_js_root/$tarantino_node_name/bin/npm" "$tarantino_js_root/bin/npm"
  )
fi
# Forget shell command-cache entries for the older system tools.
hash -r
if ! linux_node_ready; then
  echo "Node 22.13 or newer is required by this project's pnpm version." >&2
  exit 1
fi
# Match package.json; never install an unpinned latest pnpm release.
if ! command -v pnpm >/dev/null 2>&1 || [ "$(pnpm --version 2>/dev/null || true)" != 11.17.0 ]; then
  npm install --global --prefix "$tarantino_js_root/global" pnpm@11.17.0
  hash -r
fi
if [ "$(pnpm --version)" != 11.17.0 ]; then
  echo "The project requires pnpm 11.17.0; setup could not activate it." >&2
  exit 1
fi
node --version
pnpm --version
