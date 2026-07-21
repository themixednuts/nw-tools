"""AZoth — one-click New World asset workspaces for Blender."""

import bpy
from bpy.app.handlers import persistent

from . import operators, organizer, preferences, properties, ui


bl_info = {
    "name": "AZoth",
    "author": "themixednuts",
    "version": (0, 1, 0),
    "blender": (5, 2, 0),
    "location": "3D View > Sidebar > AZoth",
    "description": "One-click linked New World asset workspaces powered by nw-tools",
    "category": "Import-Export",
}

# Blender's glTF importer discovers this exact module-level symbol. The class
# only maps the nw-tools node contract into Blender ID properties; all legacy
# parsing and runtime-faithful scheduling remain in Rust.
glTF2ImportUserExtension = organizer.glTF2ImportUserExtension


def _menu_import(self, context):
    del context
    self.layout.operator(operators.AZOTH_OT_import_file.bl_idname, text="AZoth package (.gltf/.glb)")


@persistent
def _after_load(_unused):
    scene = bpy.context.scene
    if scene is not None and hasattr(scene, "azoth") and not scene.azoth.manifests:
        operators.refresh_manifests(scene.azoth)


def register():
    preferences.register()
    properties.register()
    operators.register()
    ui.register()
    bpy.types.TOPBAR_MT_file_import.append(_menu_import)
    if _after_load not in bpy.app.handlers.load_post:
        bpy.app.handlers.load_post.append(_after_load)
    scene = getattr(bpy.context, "scene", None)
    if scene is not None:
        operators.refresh_manifests(scene.azoth)


def unregister():
    if _after_load in bpy.app.handlers.load_post:
        bpy.app.handlers.load_post.remove(_after_load)
    bpy.types.TOPBAR_MT_file_import.remove(_menu_import)
    ui.unregister()
    operators.unregister()
    properties.unregister()
    preferences.unregister()
