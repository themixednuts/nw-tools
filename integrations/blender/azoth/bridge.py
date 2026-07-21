"""The narrow subprocess bridge to nw-tools-owned transformations."""

import json
from pathlib import Path
import subprocess

try:
    from . import metadata, paths
except ImportError:  # unittest loads sibling modules from azoth/ on sys.path
    import metadata  # type: ignore
    import paths  # type: ignore


_SCHEDULE_CACHE = {}
_DESCRIPTION_CACHE = {}


def find_nw_tools():
    return paths.find_nw_tools()


def schedule(manifest):
    manifest = Path(manifest).resolve()
    identity = metadata.manifest_identity(manifest)
    key = (str(manifest).casefold(), identity)
    if key in _SCHEDULE_CACHE:
        return _SCHEDULE_CACHE[key]
    executable = find_nw_tools()
    if executable is None:
        return None
    completed = subprocess.run(
        [
            str(executable),
            "--plain",
            "--color",
            "never",
            "azoth",
            "schedule",
            str(manifest),
            "--package-root",
            str(paths.package_root()),
        ],
        check=True,
        capture_output=True,
        text=True,
        creationflags=getattr(subprocess, "CREATE_NO_WINDOW", 0),
    )
    payload = json.loads(completed.stdout)
    if payload.get("schemaVersion") != 1:
        raise RuntimeError("nw-tools returned an unsupported AZoth schedule schema")
    result = payload["schedule"]
    _SCHEDULE_CACHE[key] = result
    return result


def describe(manifest):
    """Return nw-tools' compact package index, never the heavyweight payload tree."""

    manifest = Path(manifest).resolve()
    identity = metadata.manifest_identity(manifest)
    key = (str(manifest).casefold(), identity)
    if key in _DESCRIPTION_CACHE:
        return _DESCRIPTION_CACHE[key]
    executable = find_nw_tools()
    if executable is None:
        return None
    completed = subprocess.run(
        [
            str(executable),
            "--plain",
            "--color",
            "never",
            "azoth",
            "describe",
            str(manifest),
            "--package-root",
            str(paths.package_root()),
        ],
        check=True,
        capture_output=True,
        text=True,
        creationflags=getattr(subprocess, "CREATE_NO_WINDOW", 0),
    )
    result = json.loads(completed.stdout)
    if result.get("schemaVersion") != 1:
        raise RuntimeError("nw-tools returned an unsupported AZoth description schema")
    _DESCRIPTION_CACHE[key] = result
    return result


def export_command(asset_filter):
    executable = find_nw_tools()
    if executable is None:
        raise FileNotFoundError(
            "nw-tools was not found. Set Preferences → AZoth → nw-tools, "
            "put a sidecar next to the extension (or in bin/), add it to PATH, "
            "or set NW_TOOLS."
        )
    return [
        str(executable),
        "--plain",
        "--color",
        "never",
        "format",
        "model",
        "--out",
        str(paths.package_root()),
        "--container",
        "gltf",
        "--filter",
        asset_filter,
        "--overwrite",
        "--no-blend",
        "--no-progress",
    ]
