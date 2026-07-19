"""The narrow subprocess bridge to nw-tools-owned transformations."""

import json
import os
from pathlib import Path
import shutil
import subprocess

from . import metadata


_SCHEDULE_CACHE = {}
_DESCRIPTION_CACHE = {}


def find_nw_tools():
    override = os.environ.get("NW_TOOLS")
    candidates = [
        Path(override) if override else None,
        Path(shutil.which("nw-tools") or ""),
        metadata.OUTPUT_ROOT / "nw-tools.exe",
        Path(r"E:\Projects\nw-tools\target\release\nw-tools.exe"),
        Path(r"E:\Projects\nw-tools\target\debug\nw-tools.exe"),
    ]
    return next((path for path in candidates if path and path.is_file()), None)


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
            str(metadata.OUTPUT_ROOT),
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
            str(metadata.OUTPUT_ROOT),
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
            "nw-tools was not found. Put nw-tools.exe on PATH, in C:\\nwt, or set NW_TOOLS."
        )
    return [
        str(executable),
        "--plain",
        "--color",
        "never",
        "format",
        "model",
        "--out",
        str(metadata.OUTPUT_ROOT),
        "--container",
        "gltf",
        "--filter",
        asset_filter,
        "--overwrite",
        "--no-blend",
        "--no-progress",
    ]
