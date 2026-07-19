"""Map nw-tools glTF contracts onto a predictable Blender hierarchy."""

import json
from pathlib import Path

import bpy


COLLECTION_PATHS = (
    "Geometry/Render",
    "Geometry/Attachments",
    "Geometry/Cloth",
    "Geometry/Shadow Proxies",
    "Geometry/Terrain",
    "Geometry/Vegetation",
    "Rig",
    "Runtime/Audio",
    "Runtime/Mannequin",
    "Runtime/Particles",
    "Runtime/Physics/Hit Volumes",
    "Runtime/Physics/Rigid Bodies",
    "Runtime/Physics/RockNRoll",
    "Runtime/Physics/Other",
    "Runtime/Variants",
    "Resources/Materials",
    "Resources/Textures",
    "Resources/Dependencies",
    "Diagnostics",
)


class glTF2ImportUserExtension:
    """glTF importer hook that preserves small, actionable node metadata."""

    def gather_import_node_after_hook(self, vnode, gltf_node, obj, gltf):
        del gltf, vnode
        if obj is None or gltf_node is None:
            return
        extras = gltf_node.extras if isinstance(gltf_node.extras, dict) else {}
        if not extras:
            return
        obj["azoth.managed"] = True
        obj["azoth.shadow_proxy"] = bool(extras.get("shadowProxy", False))
        if extras.get("role"):
            obj["azoth.role"] = str(extras["role"])
        particle = extras.get("particleEmitter")
        if isinstance(particle, dict):
            obj["azoth.category"] = "particle"
            obj["azoth.particle.emitter"] = str(particle.get("selectedEmitter", ""))
            obj["azoth.particle.library"] = str(particle.get("particleLibraryPath", ""))
            placement = particle.get("placement") or {}
            obj["azoth.particle.placement"] = str(placement.get("kind", "entity"))
            if placement.get("boneName"):
                obj["azoth.particle.bone"] = str(placement["boneName"])
            context = particle.get("context") or {}
            obj["azoth.source_path"] = str(context.get("sourcePath", ""))
            alternates = context.get("alternateSourcePaths") or []
            if alternates:
                obj["azoth.variants"] = json.dumps(alternates, separators=(",", ":"))
        physics = extras.get("physics")
        if isinstance(physics, dict):
            obj["azoth.category"] = "physics"
            obj["azoth.physics.kind"] = str(physics.get("kind", "other"))
            obj["azoth.physics.index"] = int(physics.get("index", physics.get("shape", 0)))


def organize(context, manifest, source_path, imported_objects, imported_data):
    stem = Path(manifest).stem
    root = bpy.data.collections.new(_unique_collection_name(f"AZoth | {stem}"))
    context.scene.collection.children.link(root)
    root["azoth.manifest"] = str(Path(manifest).resolve())
    root["azoth.source_path"] = source_path
    collections = _collection_tree(root)
    armatures = [obj for obj in imported_objects if obj.type == "ARMATURE"]
    armature = armatures[0] if armatures else None

    for obj in imported_objects:
        target = collections[_object_path(obj)]
        _move_to_collection(obj, target)
        obj["azoth.manifest"] = str(Path(manifest).resolve())
        if obj.get("azoth.category") == "particle":
            obj.empty_display_type = "SPHERE"
            obj.empty_display_size = 0.08
            obj.color = (0.25, 0.65, 1.0, 1.0)
            _attach_particle_to_bone(obj, armature)
        elif obj.get("azoth.category") == "physics":
            obj.display_type = "WIRE"
            obj.show_in_front = True
            obj.color = (1.0, 0.25, 0.08, 0.45)

    for material in imported_data.get("materials", ()):
        material["azoth.category"] = "Materials"
        material["azoth.manifest"] = str(Path(manifest).resolve())
    for image in imported_data.get("images", ()):
        image["azoth.category"] = "Textures"
        image["azoth.manifest"] = str(Path(manifest).resolve())
    for action in imported_data.get("actions", ()):
        action["azoth.category"] = "Animations"
        action["azoth.manifest"] = str(Path(manifest).resolve())

    _remove_empty_import_collections(root, imported_objects)
    _configure_visibility(collections)
    return root


def linked_override(context, collection):
    """Create an editable override for the linked rig while meshes stay shared."""

    armature = next((obj for obj in collection.all_objects if obj.type == "ARMATURE"), None)
    if armature is None or armature.library is None:
        return
    bpy.ops.object.select_all(action="DESELECT")
    armature.select_set(True)
    context.view_layer.objects.active = armature
    try:
        bpy.ops.object.make_override_library(collection=collection.session_uid)
    except (RuntimeError, TypeError):
        # The collection remains a valid read-only package even if Blender cannot
        # create an override (for example in a restricted background context).
        pass


def _collection_tree(root):
    result = {}
    for path in COLLECTION_PATHS:
        parent = root
        built = []
        for component in path.split("/"):
            built.append(component)
            key = "/".join(built)
            collection = result.get(key)
            if collection is None:
                collection = bpy.data.collections.new(f"{root.name} | {key}")
                parent.children.link(collection)
                result[key] = collection
            parent = collection
    return result


def _object_path(obj):
    category = obj.get("azoth.category")
    if category == "particle":
        return "Runtime/Particles"
    if category == "physics":
        kind = obj.get("azoth.physics.kind", "other")
        return {
            "hitVolume": "Runtime/Physics/Hit Volumes",
            "rigidBody": "Runtime/Physics/Rigid Bodies",
            "rockNRoll": "Runtime/Physics/RockNRoll",
        }.get(kind, "Runtime/Physics/Other")
    role = str(obj.get("azoth.role", ""))
    if role == "clothSimulation":
        return "Geometry/Cloth"
    if obj.get("azoth.shadow_proxy"):
        return "Geometry/Shadow Proxies"
    if obj.type == "ARMATURE":
        return "Rig"
    if obj.type == "MESH":
        return "Geometry/Render"
    if obj.parent and obj.parent.type == "ARMATURE":
        return "Geometry/Attachments"
    return "Diagnostics"


def _move_to_collection(obj, target):
    if obj.name not in target.objects:
        target.objects.link(obj)
    for collection in tuple(obj.users_collection):
        if collection != target:
            collection.objects.unlink(obj)


def _attach_particle_to_bone(obj, armature):
    bone_name = obj.get("azoth.particle.bone")
    if not armature or not bone_name or bone_name not in armature.data.bones:
        return
    world = obj.matrix_world.copy()
    obj.parent = armature
    obj.parent_type = "BONE"
    obj.parent_bone = bone_name
    obj.matrix_world = world


def _remove_empty_import_collections(root, imported_objects):
    keep = {root}
    keep.update(_walk_collections(root))
    candidates = {collection for obj in imported_objects for collection in obj.users_collection}
    for collection in candidates - keep:
        if not collection.objects and not collection.children and collection.users == 0:
            bpy.data.collections.remove(collection)


def _walk_collections(root):
    for child in root.children:
        yield child
        yield from _walk_collections(child)


def _configure_visibility(collections):
    # Engine helpers remain available in their named collections, but a newly
    # opened workspace should present the authored character, not a wall of
    # wireframes and emitter markers. Users can reveal any category directly
    # from the Outliner when they need to inspect it.
    for path in (
        "Geometry/Cloth",
        "Geometry/Shadow Proxies",
        "Runtime/Particles",
        "Runtime/Physics/Hit Volumes",
        "Runtime/Physics/Rigid Bodies",
        "Runtime/Physics/RockNRoll",
        "Runtime/Physics/Other",
        "Diagnostics",
    ):
        collections[path].hide_viewport = True
        collections[path].hide_render = True


def _unique_collection_name(base):
    if base not in bpy.data.collections:
        return base
    index = 2
    while f"{base} [{index}]" in bpy.data.collections:
        index += 1
    return f"{base} [{index}]"
