"""Deterministic, reversible presentation defaults for AZoth workspaces."""

from math import sqrt

from mathutils import Quaternion, Vector


HIDDEN_COLLECTION_PATHS = (
    "Geometry/Cloth",
    "Geometry/Shadow Proxies",
    "Runtime/Particles",
    "Runtime/Physics/Hit Volumes",
    "Runtime/Physics/Rigid Bodies",
    "Runtime/Physics/RockNRoll",
    "Runtime/Physics/Other",
    "Diagnostics",
)

FRONT_VIEW_ROTATION = Quaternion((sqrt(0.5), sqrt(0.5), 0.0, 0.0))

# Blender's perspective projection uses a 32 mm virtual sensor for the 3D View.
# Orthographic zoom retains that lens-derived scale when view_distance changes.
VIEWPORT_SENSOR_WIDTH_MM = 32.0
FRAME_MARGIN = 1.25


def configure_source_collections(collections):
    """Keep engine helpers available without rendering or showing them by default."""

    for path in HIDDEN_COLLECTION_PATHS:
        collection = collections[path]
        collection.hide_viewport = True
        collection.hide_render = True


def configure_workspace(context, root, *, frame=True):
    """Apply the clean character view to a linked or local AZoth package."""

    _configure_layer_collections(context, root)
    _hide_rig_objects(context, root)
    _configure_viewport(context, root if frame else None)


def find_scene_root(scene):
    """Return the AZoth package collection linked into the active scene."""

    return next(
        (
            collection
            for collection in _walk_collections(scene.collection)
            if collection.get("azoth.manifest")
        ),
        None,
    )


def hidden_layer_paths(context, root):
    """Report helper categories hidden in the active View Layer."""

    prefix = root.name + " | "
    expected = set(HIDDEN_COLLECTION_PATHS)
    return {
        layer.collection.name[len(prefix) :]
        for layer in _walk_layer_collections(context.view_layer.layer_collection)
        if layer.hide_viewport
        and layer.collection.name.startswith(prefix)
        and layer.collection.name[len(prefix) :] in expected
    }


def _configure_layer_collections(context, root):
    # Collection.hide_viewport is a datablock default. Blender 5.2 also stores
    # visibility on each LayerCollection; without this second setting, linked
    # workspaces still draw physics, particle, cloth, and diagnostic helpers.
    hidden_names = {f"{root.name} | {path}" for path in HIDDEN_COLLECTION_PATHS}
    for layer in _walk_layer_collections(context.view_layer.layer_collection):
        if layer.collection.name in hidden_names:
            layer.hide_viewport = True


def _hide_rig_objects(context, root):
    # The eye flag is per View Layer and affects only drawing. Armature modifiers
    # continue to evaluate, so animation remains playable and the Rig collection
    # stays one click away in the Outliner.
    root_object_names = {obj.name for obj in root.all_objects}
    for obj in context.view_layer.objects:
        if obj.type == "ARMATURE" and obj.name in root_object_names:
            obj.hide_set(True, view_layer=context.view_layer)


def _configure_viewport(context, root):
    screen = getattr(context, "screen", None)
    if screen is None:
        return
    bounds = render_bounds(root) if root is not None else None
    for area in screen.areas:
        if area.type != "VIEW_3D":
            continue
        space = area.spaces.active
        space.shading.type = "MATERIAL"
        if bounds is None:
            continue
        center, extent = bounds
        region_3d = space.region_3d
        region_3d.view_rotation = FRONT_VIEW_ROTATION
        region_3d.view_perspective = "ORTHO"
        region_3d.view_location = center

        window_region = next(
            (region for region in area.regions if region.type == "WINDOW"),
            None,
        )
        if window_region is not None:
            region_3d.view_distance = front_view_distance(
                extent,
                window_region.width,
                window_region.height,
                space.lens,
            )


def front_view_distance(extent, width, height, lens):
    """Fit X/Z render bounds using Blender's 3D View projection model."""

    aspect = max(width, 1) / max(height, 1)
    tangent_half_horizontal_fov = VIEWPORT_SENSOR_WIDTH_MM / (2.0 * lens)
    horizontal_distance = 0.5 * extent.x / tangent_half_horizontal_fov
    vertical_distance = 0.5 * extent.z * aspect / tangent_half_horizontal_fov
    return max(1.0, horizontal_distance, vertical_distance) * FRAME_MARGIN


def render_bounds(root):
    render = next(
        (
            collection
            for collection in _walk_collections(root)
            if collection.name == f"{root.name} | Geometry/Render"
        ),
        None,
    )
    if render is None:
        return None
    points = [
        obj.matrix_world @ Vector(corner)
        for obj in render.all_objects
        if obj.type == "MESH"
        for corner in obj.bound_box
    ]
    if not points:
        return None
    lower = Vector(tuple(min(point[axis] for point in points) for axis in range(3)))
    upper = Vector(tuple(max(point[axis] for point in points) for axis in range(3)))
    return (lower + upper) * 0.5, upper - lower


def _walk_collections(root):
    for child in root.children:
        yield child
        yield from _walk_collections(child)


def _walk_layer_collections(root):
    yield root
    for child in root.children:
        yield from _walk_layer_collections(child)
