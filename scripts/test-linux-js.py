#!/usr/bin/env python3
"""Network-free checks for the Linux JavaScript toolchain bootstrap."""
import json
import os
from pathlib import Path
import shutil
import subprocess
import tempfile
import unittest

SCRIPT = Path(__file__).with_name('setup-linux-js.sh').resolve()


class JsSetupTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name)
        self.bin = self.root / 'bin'
        self.bin.mkdir()
        self.calls = self.root / 'calls'
        self.env = dict(os.environ, PATH=str(self.bin),
                        XDG_DATA_HOME=str(self.root / 'data'), TEST_CALLS=str(self.calls))
        for name in ('sh', 'mkdir', 'mktemp', 'rm', 'awk', 'sha256sum', 'tar', 'chmod'):
            (self.bin / name).symlink_to(shutil.which(name))
        self.command('node', 'exit 0')
        self.command('pnpm', 'echo 11.17.0')
        self.command('uname', 'echo x86_64')
        self.command('npm', 'echo "$*" > "$TEST_CALLS"\njs_bin="$XDG_DATA_HOME/tarantino/toolchain/global/bin"\nmkdir -p "$js_bin"\nprintf \'#!/bin/sh\\necho 11.17.0\\n\' > "$js_bin/pnpm"\nchmod +x "$js_bin/pnpm"')
        self.command('curl', 'exit 99')

    def command(self, name, body):
        path = self.bin / name
        path.write_text('#!/bin/sh\nset -eu\n' + body + '\n')
        path.chmod(0o755)

    def run_setup(self):
        return subprocess.run([str(self.bin / 'sh'), '-c', '. "$1"', 'js-test', str(SCRIPT)],
                              env=self.env, capture_output=True, text=True)

    def test_existing_tools_are_preserved(self):
        self.assertEqual(self.run_setup().returncode, 0)
        self.assertFalse(self.calls.exists())
        self.assertFalse((self.root / 'data').exists())

    def test_wrong_pnpm_installs_project_version_in_user_directory(self):
        self.command('pnpm', 'echo 10.0.0')
        self.assertEqual(self.run_setup().returncode, 0)
        pin = json.loads(SCRIPT.parent.parent.joinpath('package.json').read_text())['packageManager']
        self.assertEqual(self.calls.read_text().strip(),
                         f'install --global --prefix {self.root}/data/tarantino/toolchain/global {pin}')

    def test_failed_download_stops_before_installing_pnpm(self):
        self.command('node', 'exit 1')
        result = self.run_setup()
        self.assertNotEqual(result.returncode, 0)
        self.assertFalse(self.calls.exists())
        self.assertFalse(list((self.root / 'data').rglob('.node-install.*')))

    def test_checksum_mismatch_never_extracts_archive(self):
        self.command('node', 'exit 1')
        self.command('curl', 'while [ "$1" != "-o" ]; do shift; done\nshift\ncase "$1" in\n SHASUMS256.txt) printf "%064d  node-v24.22.0-linux-x64.tar.gz\\n" 0 > "$1" ;;\n *) printf invalid > "$1" ;;\nesac')
        result = self.run_setup()
        self.assertNotEqual(result.returncode, 0)
        self.assertIn('FAILED', result.stdout)
        self.assertFalse(self.calls.exists())
        self.assertFalse(list((self.root / 'data').rglob('.node-install.*')))

    def test_unsupported_architecture_does_not_download(self):
        self.command('node', 'exit 1')
        self.command('uname', 'echo unknown')
        result = self.run_setup()
        self.assertNotEqual(result.returncode, 0)
        self.assertIn('architecture', result.stderr)
        self.assertFalse((self.root / 'data').exists())


if __name__ == '__main__':
    unittest.main()
