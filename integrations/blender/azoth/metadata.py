"""Read nw-tools glTF/GLB packages without reimplementing legacy formats."""

from __future__ import annotations

from dataclasses import dataclass
import hashlib
import json
from pathlib import Path
import struct
from typing import Any, Iterable
from urllib.parse import unquote, urlparse

try:
    from . import paths
except ImportError:  # unittest loads sibling modules from azoth/ on sys.path
    import paths  # type: ignore

_GLB_MAGIC = b"glTF"
_GLB_JSON = 0x4E4F534A


class ManifestError(ValueError):
    """A file is not a readable glTF 2 package."""


@dataclass(frozen=True)
class ResourceRecord:
    category: str
    kind: str
    path: str
    status: str = "ready"


@dataclass(frozen=True)
class AnimationRecord:
    name: str
    source_path: str
    duration: float
    sample_rate: float
    event_count: int
    audio_count: int


@dataclass(frozen=True)
class ManifestSummary:
    path: Path
    source_path: str
    label: str
    animation_count: int
    resource_count: int
    issue_count: int


def load_document(path: str | Path) -> dict[str, Any]:
    """Load the JSON document from a `.gltf` or binary `.glb` container."""

    manifest = Path(path)
    if manifest.suffix.lower() == ".glb":
        raw = manifest.read_bytes()
        if len(raw) < 20:
            raise ManifestError(f"{manifest} is shorter than a GLB header")
        magic, version, total_length = struct.unpack_from("<4sII", raw)
        if magic != _GLB_MAGIC or version != 2 or total_length != len(raw):
            raise ManifestError(f"{manifest} is not a valid glTF 2 GLB")
        offset = 12
        while offset + 8 <= len(raw):
            chunk_length, chunk_type = struct.unpack_from("<II", raw, offset)
            offset += 8
            end = offset + chunk_length
            if end > len(raw):
                raise ManifestError(f"{manifest} contains a truncated GLB chunk")
            if chunk_type == _GLB_JSON:
                return _validate_document(json.loads(raw[offset:end].rstrip(b"\0 \t\r\n")))
            offset = end
        raise ManifestError(f"{manifest} has no JSON chunk")
    try:
        return _validate_document(json.loads(manifest.read_text(encoding="utf-8")))
    except (OSError, UnicodeError, json.JSONDecodeError) as error:
        raise ManifestError(f"cannot read {manifest}: {error}") from error


def _validate_document(value: Any) -> dict[str, Any]:
    if not isinstance(value, dict) or value.get("asset", {}).get("version") != "2.0":
        raise ManifestError("document is not glTF 2.0")
    return value


def scan_manifests(root: Path | None = None) -> list[ManifestSummary]:
    """Find package containers without pre-parsing their potentially huge JSON."""

    root = paths.package_root() if root is None else root
    azoth = paths.azoth_root(root)
    if not root.is_dir():
        return []
    found: list[ManifestSummary] = []
    for path in root.rglob("*"):
        if path.suffix.lower() not in {".gltf", ".glb"} or azoth in path.parents or path == azoth:
            continue
        found.append(
            ManifestSummary(
                path=path,
                source_path=str(path.relative_to(root)).replace("\\", "/"),
                label=path.stem,
                animation_count=0,
                resource_count=0,
                issue_count=0,
            )
        )
    found.sort(key=lambda item: (item.source_path.casefold(), str(item.path).casefold()))
    return found


def resource_records(
    document: dict[str, Any],
    output_root: Path | None = None,
    manifest_path: Path | None = None,
) -> list[ResourceRecord]:
    """Return one deduplicated, categorized index of every retained resource."""

    output_root = paths.package_root() if output_root is None else output_root
    extras = document.get("extras") or {}
    records: dict[tuple[str, str], ResourceRecord] = {}

    def add(kind: str, path: str, status: str = "ready") -> None:
        if not path:
            return
        normalized = path.replace("\\", "/")
        category = category_for(kind, normalized)
        key = (kind.casefold(), normalized.casefold())
        records[key] = ResourceRecord(category, kind or "dependency", normalized, status)

    for item in extras.get("sourceAssets") or []:
        if isinstance(item, dict):
            add(str(item.get("kind", "sourceAsset")), str(item.get("path", "")))
            _add_character_attachments(item, add)
    for item in extras.get("embeddedResources") or []:
        if isinstance(item, dict):
            add(str(item.get("kind", "embeddedResource")), str(item.get("sourcePath", "")))
    for path in extras.get("dependencies") or []:
        if isinstance(path, str):
            add("dependency", path, _catalog_status(output_root, path))
    for path in _alternate_source_paths(document):
        add("variant", path)
    for issue in extras.get("unboundAnimations") or []:
        if isinstance(issue, dict):
            add("unboundAnimation", str(issue.get("sourcePath", "")), "unbound")
    for issue in extras.get("unboundParticleEmitters") or []:
        if isinstance(issue, dict):
            context = issue.get("context") or {}
            add("unboundParticleEmitter", str(context.get("sourcePath", "")), "unbound")

    if manifest_path is not None:
        for uri in _external_uris(document):
            resolved = _resolve_uri(manifest_path, output_root, uri)
            add("glTFResource", uri, "ready" if resolved and resolved.is_file() else "missing")

    for index, mesh in enumerate(document.get("meshes") or []):
        if isinstance(mesh, dict):
            add("glTFMesh", str(mesh.get("name") or f"mesh_{index}"))
    for index, skin in enumerate(document.get("skins") or []):
        if isinstance(skin, dict):
            add("glTFSkin", str(skin.get("name") or f"skin_{index}"))
    for index, material in enumerate(document.get("materials") or []):
        if isinstance(material, dict):
            add("glTFMaterial", str(material.get("name") or f"material_{index}"))
    for index, image in enumerate(document.get("images") or []):
        if isinstance(image, dict):
            add("glTFTexture", str(image.get("uri") or image.get("name") or f"image_{index}"))
    for node in document.get("nodes") or []:
        if not isinstance(node, dict):
            continue
        node_extras = node.get("extras") or {}
        particle = node_extras.get("particleEmitter")
        if isinstance(particle, dict):
            emitter = str(particle.get("selectedEmitter") or "emitter")
            add("particleEmitter", f"{node.get('name') or 'emitter'} -> {emitter}")
        physics = node_extras.get("physics")
        if isinstance(physics, dict):
            index = physics.get("index", physics.get("shape", 0))
            add(str(physics.get("kind", "physics")), f"{node.get('name') or 'physics'} [index:{index}]")
        if node_extras.get("role") == "clothSimulation":
            add("clothSimulation", str(node.get("name") or "cloth_simulation"))

    return sorted(records.values(), key=lambda item: (item.category, item.kind, item.path.casefold()))


def category_for(kind: str, path: str) -> str:
    token = kind.casefold()
    suffix = Path(path).suffix.casefold()
    if token in {"variant"} or ".variant" in path.casefold():
        return "Variants"
    if "terrain" in token or token in {"regionmaterial", "worldmaterial", "regionchunks"}:
        return "Terrain"
    if "vegetation" in token or token == "distribution":
        return "Vegetation"
    if "particle" in token:
        return "Particles"
    if "cloth" in token or "ca_cloth" in token:
        return "Cloth"
    if "mannequin" in token or token in {"blendspace", "combinedblendspace"}:
        return "Mannequin"
    if "audio" in token or "wwise" in token or suffix in {".bnk", ".wem", ".wav"}:
        return "Audio"
    if (
        "physics" in token
        or "collision" in token
        or "rocknroll" in token
        or token in {"hitvolume", "rigidbody"}
        or suffix == ".rnr"
    ):
        return "Physics"
    if "animation" in token or suffix in {".caf", ".i_caf", ".dba", ".animevents"}:
        return "Animations"
    if token == "gltfskin":
        return "Rig"
    if token == "gltfmesh":
        return "Geometry"
    if token.startswith("characterattachment:"):
        return "Attachments"
    if "material" in token or suffix in {".mtl", ".clothmaterial"}:
        return "Materials"
    if "texture" in token or token == "gltfresource" and suffix in {".png", ".jpg", ".jpeg", ".dds"}:
        return "Textures"
    if token.startswith("unbound"):
        return "Diagnostics"
    return "Dependencies"


def animation_records(document: dict[str, Any]) -> list[AnimationRecord]:
    records: list[AnimationRecord] = []
    for index, animation in enumerate(document.get("animations") or []):
        if not isinstance(animation, dict):
            continue
        extras = animation.get("extras") or {}
        records.append(
            AnimationRecord(
                name=str(animation.get("name") or f"Animation {index + 1}"),
                source_path=str(extras.get("crySourcePath", "")),
                duration=float(extras.get("cryDuration") or 0.0),
                sample_rate=float(extras.get("crySampleRate") or 0.0),
                event_count=len(extras.get("cryEvents") or []),
                audio_count=len(extras.get("cryMannequinAudio") or []),
            )
        )
    return records


def description_from_document(
    document: dict[str, Any],
    output_root: Path | None = None,
    manifest_path: Path | None = None,
) -> dict[str, Any]:
    """Compact the fallback parser result before Blender starts its own import."""

    output_root = paths.package_root() if output_root is None else output_root
    extras = document.get("extras") or {}
    return {
        "schemaVersion": 1,
        "sourcePath": str(extras.get("sourcePath", "")),
        "resources": [record.__dict__ for record in resource_records(document, output_root, manifest_path)],
        "animations": [
            {
                "name": record.name,
                "sourcePath": record.source_path,
                "duration": record.duration,
                "sampleRate": record.sample_rate,
                "eventCount": record.event_count,
                "audioCount": record.audio_count,
            }
            for record in animation_records(document)
        ],
        "diagnostics": diagnostics(document, output_root, manifest_path),
    }


def diagnostics(
    document: dict[str, Any],
    output_root: Path | None = None,
    manifest_path: Path | None = None,
) -> list[str]:
    output_root = paths.package_root() if output_root is None else output_root
    issues: list[str] = []
    extras = document.get("extras") or {}
    if extras.get("unboundAnimations"):
        issues.append(f"{len(extras['unboundAnimations'])} unbound animation(s)")
    if extras.get("unboundParticleEmitters"):
        issues.append(f"{len(extras['unboundParticleEmitters'])} unbound particle emitter(s)")
    if manifest_path is not None:
        for uri in _external_uris(document):
            resolved = _resolve_uri(manifest_path, output_root, uri)
            if resolved is None or not resolved.is_file():
                issues.append(f"missing external resource: {uri}")
    return issues


def workspace_paths(manifest: Path, output_root: Path | None = None) -> tuple[Path, Path]:
    """Stable per-asset linked-library and workspace paths under the package root."""

    output_root = paths.package_root() if output_root is None else output_root
    manifest = manifest.resolve()
    try:
        relative = manifest.relative_to(output_root.resolve())
    except ValueError as error:
        raise ManifestError(f"manifest must be inside {output_root}: {manifest}") from error
    stem = relative.with_suffix("")
    return (
        paths.library_root(output_root) / stem.with_suffix(".blend"),
        paths.workspace_root(output_root) / stem.with_suffix(".blend"),
    )


def manifest_identity(path: Path) -> str:
    """Short content identity used to tell linked libraries when to rebuild."""

    return hashlib.sha256(path.read_bytes()).hexdigest()[:16]


def _alternate_source_paths(value: Any) -> Iterable[str]:
    extras = value.get("extras") if isinstance(value, dict) else None
    if not isinstance(extras, dict):
        return
    yield from _alternate_paths_in(extras.get("physics"))
    for node in value.get("nodes") or []:
        if isinstance(node, dict):
            particle = (node.get("extras") or {}).get("particleEmitter")
            if isinstance(particle, dict):
                yield from _alternate_paths_in(particle.get("context"))


def _alternate_paths_in(value: Any) -> Iterable[str]:
    if isinstance(value, dict):
        paths = value.get("alternateSourcePaths")
        if isinstance(paths, list):
            yield from (path for path in paths if isinstance(path, str))
        for child in value.values():
            if isinstance(child, (dict, list)):
                yield from _alternate_paths_in(child)
    elif isinstance(value, list):
        for child in value:
            yield from _alternate_paths_in(child)


def _external_uris(document: dict[str, Any]) -> Iterable[str]:
    for table in ("buffers", "images"):
        for item in document.get(table) or []:
            if isinstance(item, dict) and isinstance(item.get("uri"), str):
                uri = item["uri"]
                if not uri.startswith("data:"):
                    yield uri


def _resolve_uri(manifest: Path, output_root: Path, uri: str) -> Path | None:
    parsed = urlparse(uri)
    if parsed.scheme not in {"", "file"}:
        return None
    path = Path(unquote(parsed.path))
    if path.is_absolute():
        return path
    beside = (manifest.parent / path).resolve()
    if beside.is_file():
        return beside
    catalog = (output_root / path).resolve()
    return catalog


def _catalog_status(root: Path, path: str) -> str:
    candidate = root / Path(path.replace("/", "\\"))
    # A dependency can be represented structurally in the glTF without its raw
    # payload being emitted. "indexed" is therefore informative, not an error.
    return "ready" if candidate.is_file() else "indexed"


def _add_character_attachments(item: dict[str, Any], add) -> None:
    if item.get("kind") != "characterDefinition":
        return
    children = (item.get("document") or {}).get("children") or []
    attachment_list = next(
        (child for child in children if isinstance(child, dict) and child.get("name") == "AttachmentList"),
        None,
    )
    if not attachment_list:
        return
    for attachment in attachment_list.get("children") or []:
        attributes = attachment.get("attributes") if isinstance(attachment, dict) else None
        if not isinstance(attributes, dict):
            continue
        kind = str(attributes.get("Type", "unknown"))
        path = str(
            attributes.get("Binding")
            or attributes.get("AName")
            or attributes.get("BoneName")
            or "attachment"
        )
        add(f"characterAttachment:{kind}", path)
