"""Blender-background acceptance test for an installed AZoth extension."""

import json
from pathlib import Path
import sys

import bpy

from bl_ext.user_default.azoth import presentation


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
    hidden_layers = presentation.hidden_layer_paths(bpy.context, root)
    expected_hidden_layers = set(presentation.HIDDEN_COLLECTION_PATHS)
    if hidden_layers != expected_hidden_layers:
        raise RuntimeError(
            "workspace helper visibility mismatch: "
            f"expected {sorted(expected_hidden_layers)}, got {sorted(hidden_layers)}"
        )
    root_object_names = {obj.name for obj in root.all_objects}
    visible_rigs = [
        obj.name
        for obj in bpy.context.view_layer.objects
        if obj.type == "ARMATURE" and obj.name in root_object_names and not obj.hide_get()
    ]
    if visible_rigs:
        raise RuntimeError(f"workspace rig overlays are visible: {visible_rigs}")
    hidden_rig = next(
        (
            obj
            for obj in bpy.context.view_layer.objects
            if obj.type == "ARMATURE" and obj.name in root_object_names
        ),
        None,
    )
    skinned_mesh = next(
        (
            obj
            for obj in root.all_objects
            if obj.type == "MESH"
            and any(modifier.type == "ARMATURE" for modifier in obj.modifiers)
        ),
        None,
    )
    if hidden_rig is None or skinned_mesh is None:
        raise RuntimeError("workspace has no rigged render mesh for presentation validation")
    hidden_coordinates = _evaluated_coordinates(skinned_mesh)
    hidden_rig.hide_set(False, view_layer=bpy.context.view_layer)
    bpy.context.view_layer.update()
    visible_coordinates = _evaluated_coordinates(skinned_mesh)
    hidden_rig.hide_set(True, view_layer=bpy.context.view_layer)
    bpy.context.view_layer.update()
    if hidden_coordinates != visible_coordinates:
        raise RuntimeError("hiding the rig overlay changed evaluated skin deformation")
    material_areas = [
        area
        for area in bpy.context.screen.areas
        if area.type == "VIEW_3D" and area.spaces.active.shading.type == "MATERIAL"
    ]
    if not material_areas:
        raise RuntimeError("workspace has no material-preview 3D View")
    incorrectly_framed_areas = [
        area
        for area in material_areas
        if area.spaces.active.region_3d.view_perspective != "ORTHO"
        or abs(
            area.spaces.active.region_3d.view_rotation.rotation_difference(
                presentation.FRONT_VIEW_ROTATION
            ).angle
        )
        > 1.0e-5
    ]
    if incorrectly_framed_areas:
        raise RuntimeError("workspace 3D View is not front-facing and orthographic")
    _center, render_extent = presentation.render_bounds(root)
    for area in material_areas:
        window_region = next(region for region in area.regions if region.type == "WINDOW")
        expected_distance = presentation.front_view_distance(
            render_extent,
            window_region.width,
            window_region.height,
            area.spaces.active.lens,
        )
        if area.spaces.active.region_3d.view_distance + 1.0e-4 < expected_distance:
            raise RuntimeError("workspace 3D View crops the render bounds")

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
        "hiddenHelperLayers": len(hidden_layers),
        "hiddenRigs": sum(
            obj.type == "ARMATURE" and obj.name in root_object_names and obj.hide_get()
            for obj in bpy.context.view_layer.objects
        ),
        "materialPreviewAreas": len(material_areas),
        "overrides": sum(obj.override_library is not None for obj in root.all_objects),
        "audioStrips": audio_strips,
    }
    print("AZOTH_ACCEPTANCE=" + json.dumps(result, sort_keys=True))


def _evaluated_coordinates(obj):
    evaluated = obj.evaluated_get(bpy.context.evaluated_depsgraph_get())
    mesh = evaluated.to_mesh()
    try:
        return tuple(tuple(vertex.co) for vertex in mesh.vertices[:64])
    finally:
        evaluated.to_mesh_clear()


main()
