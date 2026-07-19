"""Blender-background acceptance test for an installed AZoth extension."""

import json
from pathlib import Path
import sys

import bpy


def main():
    args = sys.argv[sys.argv.index("--") + 1 :] if "--" in sys.argv else []
    requested = Path(args[0]).resolve() if args else None
    state = bpy.context.scene.azoth
    bpy.ops.azoth.refresh()
    if requested is not None:
        index = next(
            index
            for index, item in enumerate(state.manifests)
            if Path(item.path).resolve() == requested
        )
        state.manifest_index = index
    state.make_linked_workspace = True
    result = bpy.ops.azoth.import_selected()
    if "FINISHED" not in result:
        raise RuntimeError(f"AZoth import failed: {result}")
    # Opening an existing current workspace replaces the active Scene datablock.
    state = bpy.context.scene.azoth

    roots = [collection for collection in bpy.data.collections if collection.name.startswith("AZoth | ")]
    if not roots:
        raise RuntimeError("AZoth root collection was not created")
    root = roots[0]
    category_counts = {}
    for collection in bpy.data.collections:
        if collection.name.startswith(root.name + " | "):
            path = collection.name[len(root.name) + 3 :]
            category_counts[path] = len(collection.objects)
    resource_counts = {}
    for item in state.resources:
        resource_counts[item.category] = resource_counts.get(item.category, 0) + 1

    library = Path(state.linked_library)
    if not library.is_file():
        raise RuntimeError(f"linked library was not written: {library}")
    if not bpy.data.filepath or not Path(bpy.data.filepath).is_file():
        raise RuntimeError("per-character workspace was not saved")
    if not any(obj.library is not None or obj.override_library is not None for obj in root.all_objects):
        raise RuntimeError("workspace contains no linked/overridden objects")
    linked_data = sum(
        obj.data is not None and obj.data.library is not None
        for obj in root.all_objects
        if hasattr(obj, "data")
    )
    if linked_data == 0:
        raise RuntimeError("workspace contains no shared linked object data")

    playable = next((index for index, item in enumerate(state.animations) if item.audio_count), None)
    audio_strips = 0
    if playable is not None:
        state.animation_index = playable
        if "FINISHED" not in bpy.ops.azoth.apply_animation():
            raise RuntimeError("AZoth could not apply a scheduled animation")
        editor = bpy.context.scene.sequence_editor
        strips = getattr(editor, "strips", None)
        if strips is None:
            strips = getattr(editor, "sequences", None)
        audio_strips = sum(strip.name.startswith("AZoth | ") for strip in strips)
        if audio_strips == 0:
            raise RuntimeError("scheduled animation created no AZoth audio strips")

    result = {
        "workspace": bpy.data.filepath,
        "library": str(library),
        "sourcePath": state.source_path,
        "objects": len(root.all_objects),
        "actions": len(state.animations),
        "resources": len(state.resources),
        "categories": category_counts,
        "resourceCategories": resource_counts,
        "linkedObjects": sum(obj.library is not None for obj in root.all_objects),
        "linkedData": linked_data,
        "overrides": sum(obj.override_library is not None for obj in root.all_objects),
        "audioStrips": audio_strips,
    }
    print("AZOTH_ACCEPTANCE=" + json.dumps(result, sort_keys=True))


main()
