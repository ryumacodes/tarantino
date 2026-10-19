import { mkdtempSync, writeFileSync, rmSync, existsSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join, dirname, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';
import { spawnSync } from 'node:child_process';

if (process.platform !== 'linux') throw new Error('Build Linux packages on Linux.');
// Tauri's bundler discovers the tray library through pkg-config after compiling.
// Catch stripped SteamOS development metadata before spending time on a build.
const trayAvailable = ['ayatana-appindicator3-0.1', 'appindicator3-0.1'].some((name) =>
  spawnSync('pkg-config', ['--exists', name]).status === 0);
if (!trayAvailable) {
  throw new Error('Tray development dependencies are incomplete. Run pnpm setup:linux before packaging.');
}
if (spawnSync('patchelf', ['--version']).status !== 0) {
  throw new Error('Install patchelf (pnpm setup:linux) before packaging.');
}
const scanner = spawnSync('pkg-config', ['--variable=pluginscannerdir', 'gstreamer-1.0'],
  { encoding: 'utf8' });
const helpers = process.env.GSTREAMER_HELPERS_DIR ?? scanner.stdout?.trim();
if (!helpers || !existsSync(join(helpers, 'gst-plugin-scanner'))) {
  throw new Error('GStreamer plugin scanner is missing. Run pnpm setup:linux before packaging.');
}
const root = resolve(dirname(fileURLToPath(import.meta.url)), '..');
const temporary = mkdtempSync(join(tmpdir(), 'tarantino-bundle-'));
try {
  const files = {};
  for (const name of ['ffmpeg', 'ffprobe']) {
    const found = spawnSync('sh', ['-c', 'command -v "$1"', 'sh', name], { encoding: 'utf8' });
    if (found.status !== 0 || !found.stdout.trim().startsWith('/')) {
      throw new Error(`Install ${name} before building Linux packages.`);
    }
    // linuxdeploy scans AppDir ELF executables and bundles their linked libraries.
    // Keep these inside the AppImage; Debian uses its declared ffmpeg dependency.
    files[`/usr/bin/${name}`] = found.stdout.trim();
  }
  // linuxdeploy's base-library exclusions can split a bundled dependency pair:
  // newer libgcrypt requires newer libgpg-error, and FFmpeg/plugins may require
  // JACK or libusb even when the destination desktop has neither installed.
  for (const name of ['libgpg-error.so.0', 'libjack.so.0', 'libusb-1.0.so.0']) {
    const located = spawnSync(process.env.CC ?? 'cc', [`-print-file-name=${name}`],
      { encoding: 'utf8' });
    const library = located.stdout?.trim();
    if (located.status !== 0 || !library?.startsWith('/') || !existsSync(library)) {
      throw new Error(`Cannot locate ${name} for the portable media runtime.`);
    }
    files[`/usr/lib/${name}`] = library;
  }
  const config = join(temporary, 'media.json');
  writeFileSync(config, JSON.stringify({ bundle: { linux: { appimage: { files } } } }));
  const build = spawnSync('pnpm', ['exec', 'tauri', 'build', '--config', config,
    '--bundles', 'deb,appimage'], {
    cwd: root,
    stdio: 'inherit',
    // linuxdeploy's bundled strip cannot read modern RELR sections on SteamOS.
    // Preserve system libraries instead of failing or rewriting their binaries.
    env: { ...process.env, NO_STRIP: process.env.NO_STRIP ?? '1', GSTREAMER_HELPERS_DIR: helpers },
  });
  if (build.error) throw build.error;
  process.exitCode = build.status ?? 1;
} finally {
  rmSync(temporary, { recursive: true, force: true });
}
