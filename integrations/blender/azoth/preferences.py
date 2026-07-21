"""Shareable AZoth addon preferences — no machine-local path defaults."""

from __future__ import annotations

import bpy
from bpy.props import StringProperty

from . import paths


def _on_package_root_update(self, context):
    del self
    if getattr(context, "scene", None) is None:
        return
    # Rescan through the operator so preferences never import operators at
    # module load (operators → bridge → paths is the stable direction).
    if bpy.ops.azoth.refresh.poll():
        bpy.ops.azoth.refresh()


class AZOTHPreferences(bpy.types.AddonPreferences):
    bl_idname = __package__

    package_root: StringProperty(
        name="Package root",
        description=(
            "Directory for structured glTF packages and .azoth workspaces. "
            "Leave empty to use AZOTH_PACKAGE_ROOT / NWT_ROOT, else ~/nwt"
        ),
        subtype="DIR_PATH",
        default="",
        update=_on_package_root_update,
    )
    nw_tools_path: StringProperty(
        name="nw-tools",
        description=(
            "Optional path to the nw-tools binary. Leave empty to search "
            "NW_TOOLS, PATH, the extension sidecar (bin/), then the package root"
        ),
        subtype="FILE_PATH",
        default="",
    )

    def draw(self, context):
        del context
        layout = self.layout
        layout.prop(self, "package_root")
        layout.label(text=f"Resolved: {paths.package_root()}", icon="FILE_FOLDER")
        layout.prop(self, "nw_tools_path")
        found = paths.find_nw_tools()
        if found is not None:
            layout.label(text=f"Using: {found}", icon="CHECKMARK")
        else:
            layout.label(
                text="nw-tools not found — put a sidecar in the extension folder/bin, or set the path",
                icon="ERROR",
            )


class AZOTH_OT_show_preferences(bpy.types.Operator):
    bl_idname = "azoth.show_preferences"
    bl_label = "AZoth Preferences"
    bl_description = "Open AZoth package root and nw-tools settings"

    def execute(self, context):
        del context
        bpy.ops.preferences.addon_show(module=paths.addon_module())
        return {"FINISHED"}


CLASSES = (
    AZOTHPreferences,
    AZOTH_OT_show_preferences,
)


def register():
    for cls in CLASSES:
        bpy.utils.register_class(cls)


def unregister():
    for cls in reversed(CLASSES):
        bpy.utils.unregister_class(cls)
