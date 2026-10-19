#!/usr/bin/env python3
"""Exercise Rust bootstrap without network access or changing user toolchains."""
import os
from pathlib import Path
import shutil
import subprocess
import tempfile
import unittest

SCRIPT = Path(__file__).with_name('setup-linux-rust.sh').resolve()


class RustSetupTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name)
        self.bin = self.root / 'bin'
        self.bin.mkdir()
        self.ready = self.root / 'ready'
        self.calls = self.root / 'calls'
        self.env = dict(os.environ, PATH=str(self.bin),
                        CARGO_HOME=str(self.root / 'cargo'),
                        TEST_READY=str(self.ready), TEST_CALLS=str(self.calls))
        for name in ('awk', 'sh', 'mktemp', 'rm'):
            (self.bin / name).symlink_to(shutil.which(name))
        self.command('cargo', '[ -f "$TEST_READY" ] || exit 1\necho cargo 1.88.0')
        self.command('rustc', '[ -f "$TEST_READY" ] || exit 1\necho rustc 1.88.0')

    def command(self, name, body):
        path = self.bin / name
        path.write_text('#!/bin/sh\nset -eu\n' + body + '\n')
        path.chmod(0o755)

    def run_setup(self):
        return subprocess.run([str(self.bin / 'sh'), '-c', 'set -eu; . "$1"',
                               'rust-test', str(SCRIPT)], env=self.env,
                              capture_output=True, text=True)

    def test_existing_compiler_does_not_install(self):
        self.ready.touch()
        self.command('rustc', 'echo rustc 1.95.0')
        self.command('curl', 'exit 99')
        self.command('rustup', 'exit 99')
        self.assertEqual(self.run_setup().returncode, 0)
        self.assertFalse(self.calls.exists())

    def test_existing_rustup_without_toolchain_is_repaired(self):
        self.command('rustup', 'echo "$*" >> "$TEST_CALLS"\n: > "$TEST_READY"')
        self.assertEqual(self.run_setup().returncode, 0)
        self.assertEqual(self.calls.read_text().splitlines(), [
            'toolchain install 1.88.0 --profile minimal --component rustfmt',
            'default 1.88.0'])

    def test_fresh_install_uses_downloaded_installer(self):
        self.command('curl', '''while [ "$1" != "-o" ]; do shift; done
shift
printf '%s\n' 'echo "$*" > "$TEST_CALLS"' ': > "$TEST_READY"' > "$1"''')
        result = self.run_setup()
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(self.calls.read_text().strip(),
                         '-y --profile minimal --default-toolchain 1.88.0 --component rustfmt')

    def test_failed_download_never_runs_partial_script(self):
        self.command('curl', '''while [ "$1" != "-o" ]; do shift; done
shift
printf '%s\n' ': > "$TEST_CALLS"' > "$1"
exit 22''')
        self.assertNotEqual(self.run_setup().returncode, 0)
        self.assertFalse(self.calls.exists())

    def test_custom_cargo_home_is_found_without_profile(self):
        self.ready.touch()
        cargo_bin = Path(self.env['CARGO_HOME']) / 'bin'
        cargo_bin.mkdir(parents=True)
        for name in ('cargo', 'rustc'):
            (self.bin / name).rename(cargo_bin / name)
        self.assertEqual(self.run_setup().returncode, 0)


if __name__ == '__main__':
    unittest.main()
