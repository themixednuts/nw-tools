"""Portable package-root and nw-tools discovery for AZoth."""

from __future__ import annotations

import os
from pathlib import Path
import shutil

try:
    import bpy
except ImportError:  # unit tests run outside Blender
    bpy = None  # type: ignore


def extension_dir() -> Path:
    """Directory of the installed/loaded AZoth Python package (sidecar home)."""

    return Path(__file__).resolve().parent


def addon_module() -> str:
    """Blender addon / extension module id (parent of this submodule)."""

    return __package__ or "azoth"


def default_package_root() -> Path:
    for key in ("AZOTH_PACKAGE_ROOT", "NWT_ROOT"):
        value = os.environ.get(key)
        if value:
            return Path(value).expanduser().resolve()
    return (Path.home() / "nwt").resolve()


def addon_preferences():
    if bpy is None:
        return None
    addon = bpy.context.preferences.addons.get(addon_module())
    return getattr(addon, "preferences", None) if addon else None


def package_root() -> Path:
    prefs = addon_preferences()
    if prefs is not None:
        configured = (prefs.package_root or "").strip()
        if configured:
            return Path(bpy_path(configured)).expanduser().resolve()
    return default_package_root()


def azoth_root(root: Path | None = None) -> Path:
    return (root or package_root()) / ".azoth"


def library_root(root: Path | None = None) -> Path:
    return azoth_root(root) / "libraries"


def workspace_root(root: Path | None = None) -> Path:
    return azoth_root(root) / "workspaces"


def find_nw_tools() -> Path | None:
    """Resolve the nw-tools sidecar / host binary.

    Order: addon preference → NW_TOOLS → PATH → extension sidecar → package root.
    """

    candidates: list[Path] = []
    prefs = addon_preferences()
    if prefs is not None:
        configured = (prefs.nw_tools_path or "").strip()
        if configured:
            candidates.append(Path(bpy_path(configured)).expanduser())
    env = os.environ.get("NW_TOOLS")
    if env:
        candidates.append(Path(env).expanduser())
    for name in ("nw-tools", "nw-tools.exe"):
        located = shutil.which(name)
        if located:
            candidates.append(Path(located))
    ext = extension_dir()
    for relative in (
        "nw-tools.exe",
        "nw-tools",
        Path("bin") / "nw-tools.exe",
        Path("bin") / "nw-tools",
    ):
        candidates.append(ext / relative)
    root = package_root()
    for name in ("nw-tools.exe", "nw-tools"):
        candidates.append(root / name)
    seen: set[str] = set()
    for path in candidates:
        if not path:
            continue
        key = str(path).casefold()
        if key in seen:
            continue
        seen.add(key)
        if path.is_file():
            return path.resolve()
    return None


def bpy_path(value: str) -> str:
    """Normalize Blender DIR_PATH / FILE_PATH values (may use `//`)."""

    if bpy is None or not value.startswith("//"):
        return value
    return bpy.path.abspath(value)
