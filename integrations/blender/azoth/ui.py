"""A compact AZoth sidebar: library, clips, resources, and diagnostics."""

import bpy

from . import bridge, metadata


class AZOTH_UL_manifests(bpy.types.UIList):
    def draw_item(self, context, layout, data, item, icon, active_data, active_propname, index):
        del context, data, icon, active_data, active_propname, index
        row = layout.row(align=True)
        row.label(text=item.label, icon="OUTLINER_COLLECTION")
        if item.animation_count:
            row.label(text=str(item.animation_count), icon="ACTION")
        if item.issue_count:
            row.label(text=str(item.issue_count), icon="ERROR")

    def filter_items(self, context, data, property_name):
        items = getattr(data, property_name)
        search = data.manifest_search.casefold().strip()
        flags = []
        for item in items:
            visible = not search or search in item.label.casefold() or search in item.source_path.casefold()
            flags.append(self.bitflag_filter_item if visible else 0)
        return flags, []


class AZOTH_UL_animations(bpy.types.UIList):
    def draw_item(self, context, layout, data, item, icon, active_data, active_propname, index):
        del context, data, icon, active_data, active_propname, index
        row = layout.row(align=True)
        row.label(text=item.name, icon="ACTION")
        if item.audio_count:
            row.label(text=str(item.audio_count), icon="SPEAKER")


class AZOTH_UL_resources(bpy.types.UIList):
    def draw_item(self, context, layout, data, item, icon, active_data, active_propname, index):
        del context, data, icon, active_data, active_propname, index
        row = layout.row(align=True)
        row.label(text=item.path.rsplit("/", 1)[-1], icon=_category_icon(item.category))
        row.label(text=item.kind)
        if item.status in {"missing", "unbound"}:
            row.label(text="", icon="ERROR")

    def filter_items(self, context, data, property_name):
        del context
        items = getattr(data, property_name)
        category = data.resource_category
        search = data.resource_search.casefold().strip()
        flags = []
        for item in items:
            visible = category == "ALL" or item.category == category
            visible = visible and (
                not search or search in item.path.casefold() or search in item.kind.casefold()
            )
            flags.append(self.bitflag_filter_item if visible else 0)
        return flags, []


class AZOTH_PT_main(bpy.types.Panel):
    bl_idname = "AZOTH_PT_main"
    bl_label = "AZoth"
    bl_space_type = "VIEW_3D"
    bl_region_type = "UI"
    bl_category = "AZoth"

    def draw(self, context):
        state = context.scene.azoth
        layout = self.layout

        box = layout.box()
        row = box.row(align=True)
        row.label(text=str(metadata.OUTPUT_ROOT), icon="FILE_FOLDER")
        row.operator("azoth.refresh", text="", icon="FILE_REFRESH")
        box.prop(state, "manifest_search", text="", icon="VIEWZOOM")
        box.template_list(
            "AZOTH_UL_manifests",
            "",
            state,
            "manifests",
            state,
            "manifest_index",
            rows=5,
        )
        row = box.row(align=True)
        row.scale_y = 1.35
        row.operator("azoth.import_selected", icon="ASSET_MANAGER")
        row.operator("azoth.import_file", text="", icon="FILEBROWSER")
        box.prop(state, "make_linked_workspace")

        export = layout.box()
        export.label(text="New World → AZoth", icon="PACKAGE")
        row = export.row(align=True)
        row.prop(state, "export_filter", text="")
        row.operator("azoth.export_asset", text="Export", icon="IMPORT")
        export.label(text="Complete structured glTF; automatic nw-jobs worker count", icon="INFO")

        status = layout.box()
        status.label(text=state.status, icon="CHECKMARK" if "fail" not in state.status.lower() else "ERROR")
        if state.source_path:
            status.label(text=state.source_path, icon="LINKED")
            status.operator("azoth.clean_character_view", icon="HIDE_OFF")
        if bridge.find_nw_tools() is None:
            status.label(text="Import ready; nw-tools not found for export/audio", icon="INFO")


class AZOTH_PT_animations(bpy.types.Panel):
    bl_idname = "AZOTH_PT_animations"
    bl_label = "Animations + Audio"
    bl_parent_id = "AZOTH_PT_main"
    bl_space_type = "VIEW_3D"
    bl_region_type = "UI"
    bl_category = "AZoth"
    bl_options = {"DEFAULT_CLOSED"}

    @classmethod
    def poll(cls, context):
        return bool(context.scene.azoth.manifest_path)

    def draw(self, context):
        state = context.scene.azoth
        layout = self.layout
        layout.template_list(
            "AZOTH_UL_animations",
            "",
            state,
            "animations",
            state,
            "animation_index",
            rows=6,
        )
        if state.animations:
            item = state.animations[state.animation_index]
            column = layout.column(align=True)
            column.label(text=item.source_path or item.action, icon="ANIM_DATA")
            column.label(text=f"{item.duration} · {item.frame_end} frames · {item.audio_count} sounds")
            row = layout.row(align=True)
            row.scale_y = 1.25
            row.operator("azoth.apply_animation", icon="PLAY")
            row.prop(state, "load_audio", text="", icon="SPEAKER")


class AZOTH_PT_resources(bpy.types.Panel):
    bl_idname = "AZOTH_PT_resources"
    bl_label = "Engine Resources"
    bl_parent_id = "AZOTH_PT_main"
    bl_space_type = "VIEW_3D"
    bl_region_type = "UI"
    bl_category = "AZoth"
    bl_options = {"DEFAULT_CLOSED"}

    @classmethod
    def poll(cls, context):
        return bool(context.scene.azoth.manifest_path)

    def draw(self, context):
        state = context.scene.azoth
        layout = self.layout
        layout.prop(state, "resource_category", text="")
        layout.prop(state, "resource_search", text="", icon="VIEWZOOM")
        layout.template_list(
            "AZOTH_UL_resources",
            "",
            state,
            "resources",
            state,
            "resource_index",
            rows=8,
        )
        if state.resources:
            item = state.resources[state.resource_index]
            box = layout.box()
            box.label(text=item.category, icon=_category_icon(item.category))
            box.label(text=item.kind)
            box.label(text=item.path)
            box.label(text=f"Status: {item.status}")


def _category_icon(category):
    return {
        "Animations": "ACTION",
        "Audio": "SPEAKER",
        "Cloth": "MOD_CLOTH",
        "Mannequin": "ARMATURE_DATA",
        "Particles": "PARTICLES",
        "Physics": "PHYSICS",
        "Materials": "MATERIAL",
        "Textures": "TEXTURE",
        "Variants": "DUPLICATE",
        "Terrain": "MESH_GRID",
        "Vegetation": "OUTLINER_OB_CURVES",
        "Diagnostics": "ERROR",
    }.get(category, "FILE")


CLASSES = (
    AZOTH_UL_manifests,
    AZOTH_UL_animations,
    AZOTH_UL_resources,
    AZOTH_PT_main,
    AZOTH_PT_animations,
    AZOTH_PT_resources,
)


def register():
    for cls in CLASSES:
        bpy.utils.register_class(cls)


def unregister():
    for cls in reversed(CLASSES):
        bpy.utils.unregister_class(cls)
