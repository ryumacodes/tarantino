#!/usr/bin/env python3
"""Restore missing development files for the app's installed native dependencies."""
import os
from pathlib import Path
import re
import subprocess
import sys

MODULES = ["gtk+-3.0", "webkit2gtk-4.1", "libpipewire-0.3",
           "gstreamer-1.0", "gstreamer-app-1.0", "alsa", "openssl", "x11", "xtst",
           "ayatana-appindicator3-0.1"]


def run(*args):
    return subprocess.run(args, capture_output=True, text=True,
                          env={**os.environ, "LC_ALL": "C"})


def pkgconfig_paths():
    defaults = run("pkg-config", "--variable=pc_path", "pkg-config")
    if defaults.returncode:
        raise RuntimeError(defaults.stderr or "Cannot determine pkg-config search directories")
    base = os.environ.get("PKG_CONFIG_LIBDIR", defaults.stdout.strip())
    paths = os.environ.get("PKG_CONFIG_PATH", "").split(os.pathsep) + base.split(os.pathsep)
    result = list(dict.fromkeys(Path(path).absolute() for path in paths if path))
    if not result:
        raise RuntimeError("pkg-config reports no search directories")
    return result


def inventory(output, search_paths):
    providers, files, provider_ranks = {}, {}, {}
    ranks = {Path(path).absolute(): index for index, path in enumerate(search_paths)}
    for line in output.splitlines():
        package, separator, filename = line.partition(" ")
        if not separator:
            continue
        path = Path(filename.strip())
        if path.suffix == ".pc" and path.parent.absolute() in ranks:
            # Multilib packages share .pc basenames. Only consider directories
            # used by this pkg-config invocation, in its actual search order.
            rank = ranks[path.parent.absolute()]
            if path.stem not in providers or rank < provider_ranks[path.stem]:
                providers[path.stem] = package
                provider_ranks[path.stem] = rank
        if (path.suffix == ".pc" or path.suffix == ".so"
                or ("include" in path.parts and path.suffix in {".h", ".hpp"})):
            files.setdefault(package, []).append(path)
    return providers, files


def inspect(providers, files):
    pending, visited, packages, errors = list(MODULES), set(), {"xdotool"}, []
    while pending:
        module = pending.pop()
        if module in visited:
            continue
        visited.add(module)
        owner = providers.get(module)
        if owner:
            packages.add(owner)
        # pkg-config includes private dependencies when resolving build flags.
        result = run("pkg-config", "--print-errors", "--print-requires",
                     "--print-requires-private", module)
        if result.returncode:
            errors.append(result.stderr)
            missing = re.findall(r"Package '([^']+)'", result.stderr)
            missing += re.findall(r"Package ([^\s']+) was not found", result.stderr)
            pending.extend(name for name in missing if name in providers)
        pending.extend(line.split()[0] for line in result.stdout.splitlines() if line.strip())
    repair = {package for package in packages
              if any(not path.exists() for path in files.get(package, []))}
    return repair, errors


def main():
    if Path("/.flatpak-info").exists():
        raise RuntimeError("Run SteamOS setup from the host terminal.")
    release = Path("/etc/os-release").read_text()
    if not re.search(r'^ID=["\']?steamos["\']?$', release, re.MULTILINE):
        raise RuntimeError("This development-file repair is only for SteamOS.")
    repair_dependencies()


def repair_dependencies():
    search_paths = pkgconfig_paths()
    repaired = set()
    for _ in range(16):
        listed = run("pacman", "-Ql")
        if listed.returncode:
            raise RuntimeError(listed.stderr)
        providers, files = inventory(listed.stdout, search_paths)
        repair, errors = inspect(providers, files)
        if not repair:
            check = run("pkg-config", "--print-errors", "--exists", *MODULES)
            if check.returncode:
                raise RuntimeError(check.stderr or "\n".join(errors))
            print("Native development files and pkg-config dependencies are present.")
            return
        if repair & repaired:
            raise RuntimeError("Reinstallation did not restore development files for: "
                               + ", ".join(sorted(repair & repaired)))
        print("Restoring missing development files: " + ", ".join(sorted(repair)), flush=True)
        # Deliberately omit --needed: package records can outlive stripped files.
        prefix = [] if os.geteuid() == 0 else ["sudo"]
        subprocess.run([*prefix, "pacman", "-S", *sorted(repair)], check=True)
        repaired.update(repair)
    raise RuntimeError("Dependency repair exceeded 16 passes; inspect pkg-config errors.")


if __name__ == "__main__":
    try:
        main()
    except (RuntimeError, subprocess.CalledProcessError) as error:
        print(f"SteamOS development-file repair failed: {error}", file=sys.stderr)
        sys.exit(1)
