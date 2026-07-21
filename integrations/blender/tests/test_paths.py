from __future__ import annotations

import os
from pathlib import Path
import sys
import tempfile
import unittest
from unittest import mock


AZOTH = Path(__file__).parents[1] / "azoth"
sys.path.insert(0, str(AZOTH))
import paths  # noqa: E402


class PathsTests(unittest.TestCase):
    def test_default_package_root_uses_home_nwt(self) -> None:
        env = {key: value for key, value in os.environ.items() if key not in {"AZOTH_PACKAGE_ROOT", "NWT_ROOT"}}
        with mock.patch.dict(os.environ, env, clear=True):
            self.assertEqual(paths.default_package_root(), (Path.home() / "nwt").resolve())

    def test_env_overrides_default_package_root(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            with mock.patch.dict(os.environ, {"AZOTH_PACKAGE_ROOT": str(root)}, clear=False):
                self.assertEqual(paths.default_package_root(), root.resolve())

    def test_find_nw_tools_prefers_extension_sidecar(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            sidecar = root / "nw-tools.exe"
            sidecar.write_bytes(b"")
            env = {
                key: value
                for key, value in os.environ.items()
                if key not in {"NW_TOOLS", "AZOTH_PACKAGE_ROOT", "NWT_ROOT", "PATH"}
            }
            with (
                mock.patch.dict(os.environ, env, clear=True),
                mock.patch.object(paths, "extension_dir", return_value=root),
                mock.patch.object(paths, "addon_preferences", return_value=None),
                mock.patch.object(paths, "package_root", return_value=root / "packages"),
                mock.patch.object(paths.shutil, "which", return_value=None),
            ):
                self.assertEqual(paths.find_nw_tools(), sidecar.resolve())

    def test_find_nw_tools_checks_bin_sidecar(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            sidecar = root / "bin" / "nw-tools"
            sidecar.parent.mkdir()
            sidecar.write_bytes(b"")
            env = {
                key: value
                for key, value in os.environ.items()
                if key not in {"NW_TOOLS", "AZOTH_PACKAGE_ROOT", "NWT_ROOT", "PATH"}
            }
            with (
                mock.patch.dict(os.environ, env, clear=True),
                mock.patch.object(paths, "extension_dir", return_value=root),
                mock.patch.object(paths, "addon_preferences", return_value=None),
                mock.patch.object(paths, "package_root", return_value=root / "packages"),
                mock.patch.object(paths.shutil, "which", return_value=None),
            ):
                self.assertEqual(paths.find_nw_tools(), sidecar.resolve())


if __name__ == "__main__":
    unittest.main()
