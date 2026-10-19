#!/usr/bin/env python3
"""No root access or package changes: regression tests for SteamOS repair."""
import importlib.util
import os
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest
from unittest.mock import patch

sys.dont_write_bytecode = True
spec = importlib.util.spec_from_file_location("repair", Path(__file__).with_name("repair-steamos-dev.py"))
repair = importlib.util.module_from_spec(spec)
spec.loader.exec_module(repair)


class RepairTests(unittest.TestCase):
    def test_native_owner_wins_in_both_listing_orders(self):
        rows = ["glib2 /usr/lib/pkgconfig/glib-2.0.pc",
                "lib32-glib2 /usr/lib32/pkgconfig/glib-2.0.pc"]
        for listing in [rows, rows[::-1]]:
            providers, _ = repair.inventory("\n".join(listing), [Path("/usr/lib/pkgconfig")])
            self.assertEqual(providers["glib-2.0"], "glib2")

    def test_search_order_wins_over_listing_order(self):
        rows = "system /usr/lib/pkgconfig/test.pc\nlocal /opt/lib/pkgconfig/test.pc"
        providers, _ = repair.inventory(rows, [Path("/opt/lib/pkgconfig"), Path("/usr/lib/pkgconfig")])
        self.assertEqual(providers["test"], "local")

    def test_shared_metadata_is_included(self):
        providers, _ = repair.inventory("shared-mime-info /usr/share/pkgconfig/shared-mime-info.pc",
                                       [Path("/usr/lib/pkgconfig"), Path("/usr/share/pkgconfig")])
        self.assertEqual(providers["shared-mime-info"], "shared-mime-info")

    def test_search_path_environment(self):
        response = subprocess.CompletedProcess([], 0, "/usr/lib/pkgconfig:/usr/share/pkgconfig", "")
        with patch.object(repair, "run", return_value=response), patch.dict(os.environ, {
            "PKG_CONFIG_PATH": "/custom/pkgconfig", "PKG_CONFIG_LIBDIR": "/native/pkgconfig"
        }):
            self.assertEqual(repair.pkgconfig_paths(), [Path("/custom/pkgconfig"), Path("/native/pkgconfig")])

    def test_repair_selects_native_packages_and_then_succeeds(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            native, multilib = root / "lib/pkgconfig", root / "lib32/pkgconfig"
            native.mkdir(parents=True)
            multilib.mkdir(parents=True)
            modules = {"glib-2.0": "glib2", "harfbuzz": "harfbuzz", "freetype2": "freetype2", "alsa": "alsa-lib"}
            rows = []
            for module, package in modules.items():
                rows.extend([f"{package} {native / (module + '.pc')}",
                             f"lib32-{package} {multilib / (module + '.pc')}"])
                (multilib / (module + ".pc")).touch()

            def query(*args):
                if args[0] == "pacman":
                    return subprocess.CompletedProcess(args, 0, "\n".join(rows), "")
                missing = [name for name in modules if name in args and not (native / (name + ".pc")).exists()]
                return subprocess.CompletedProcess(args, bool(missing), "",
                    "\n".join(f"Package '{name}' not found" for name in missing))

            def install(args, **kwargs):
                self.assertEqual(args[-4:], sorted(modules.values()))
                self.assertNotIn("--needed", args)
                for name in modules:
                    (native / (name + ".pc")).touch()
                return subprocess.CompletedProcess(args, 0)

            with patch.object(repair, "MODULES", list(modules)), \
                 patch.object(repair, "pkgconfig_paths", return_value=[native]), \
                 patch.object(repair, "run", side_effect=query), \
                 patch.object(repair.subprocess, "run", side_effect=install) as installer:
                repair.repair_dependencies()
                self.assertEqual(installer.call_count, 1)
                repair.repair_dependencies()
                self.assertEqual(installer.call_count, 1)

    def test_present_metadata_with_missing_header_needs_repair(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            pc = root / "test.pc"
            pc.touch()
            providers, files = repair.inventory(
                f"test-package {pc}\ntest-package {root / 'include/test.h'}", [root])
            with patch.object(repair, "MODULES", ["test"]), patch.object(repair, "run",
                return_value=subprocess.CompletedProcess([], 0, "", "")):
                packages, _ = repair.inspect(providers, files)
                self.assertEqual(packages, {"test-package"})


if __name__ == "__main__":
    unittest.main()
