"""Blender RNA state for the AZoth sidebar."""

import bpy
from bpy.props import BoolProperty, CollectionProperty, EnumProperty, IntProperty, StringProperty


CATEGORIES = (
    ("ALL", "All", "Show every indexed resource"),
    ("Animations", "Animations", "CAF, DBA, animation events, and Actions"),
    ("Audio", "Audio", "ATL, Wwise banks/media, and decoded previews"),
    ("Cloth", "Cloth", "NvCloth fabric, material, render, simulation, and colliders"),
    ("Mannequin", "Mannequin", "ADB, controller/tag definitions, and blend spaces"),
    ("Particles", "Particles", "Emitter attachments, libraries, textures, materials, and geometry"),
    ("Physics", "Physics", "Hit volumes, rigid bodies, RockNRoll, and collision assets"),
    ("Materials", "Materials", "Cry material resources"),
    ("Textures", "Textures", "Shared source and display textures"),
    ("Variants", "Variants", "Context-equivalent source variants"),
    ("Terrain", "Terrain", "Terrain height, surface, water, tract, and region data"),
    ("Vegetation", "Vegetation", "Distribution, region, and vegetation images"),
    ("Dependencies", "Dependencies", "Retained source assets and glTF resources"),
    ("Diagnostics", "Diagnostics", "Unbound and missing resources"),
    ("Geometry", "Geometry", "Native glTF meshes"),
    ("Rig", "Rig", "Skins, skeletons, and bones"),
    ("Attachments", "Attachments", "CDF skin, bone, face, proxy, and cloth attachments"),
)


class AZOTHManifestItem(bpy.types.PropertyGroup):
    label: StringProperty()
    path: StringProperty(subtype="FILE_PATH")
    source_path: StringProperty()
    animation_count: IntProperty()
    resource_count: IntProperty()
    issue_count: IntProperty()


class AZOTHResourceItem(bpy.types.PropertyGroup):
    category: StringProperty()
    kind: StringProperty()
    path: StringProperty()
    status: StringProperty()


class AZOTHAnimationItem(bpy.types.PropertyGroup):
    name: StringProperty()
    action: StringProperty()
    source_path: StringProperty()
    duration: StringProperty()
    frame_end: IntProperty(default=1, min=1)
    audio_count: IntProperty(default=0, min=0)
    schedule_index: IntProperty(default=-1)


class AZOTHSceneState(bpy.types.PropertyGroup):
    manifests: CollectionProperty(type=AZOTHManifestItem)
    manifest_index: IntProperty(default=0, min=0)
    manifest_search: StringProperty(name="Search", options={"TEXTEDIT_UPDATE"})

    resources: CollectionProperty(type=AZOTHResourceItem)
    resource_index: IntProperty(default=0, min=0)
    resource_category: EnumProperty(name="Category", items=CATEGORIES, default="ALL")
    resource_search: StringProperty(name="Find", options={"TEXTEDIT_UPDATE"})

    animations: CollectionProperty(type=AZOTHAnimationItem)
    animation_index: IntProperty(default=0, min=0)

    manifest_path: StringProperty(name="Manifest", subtype="FILE_PATH")
    source_path: StringProperty(name="Source asset")
    linked_library: StringProperty(name="Linked library", subtype="FILE_PATH")
    status: StringProperty(default="Ready")
    export_filter: StringProperty(name="Asset", description="Catalog path substring, e.g. isabella_t2")
    make_linked_workspace: BoolProperty(
        name="Linked workspace",
        description="Keep heavy data in one reusable library and save a small per-character workspace",
        default=True,
    )
    load_audio: BoolProperty(
        name="Animation audio",
        description="Place shared decoded WAV previews on the selected animation timeline",
        default=True,
    )
    show_advanced: BoolProperty(name="Advanced", default=False)


CLASSES = (
    AZOTHManifestItem,
    AZOTHResourceItem,
    AZOTHAnimationItem,
    AZOTHSceneState,
)


def register():
    for cls in CLASSES:
        bpy.utils.register_class(cls)
    bpy.types.Scene.azoth = bpy.props.PointerProperty(type=AZOTHSceneState)


def unregister():
    del bpy.types.Scene.azoth
    for cls in reversed(CLASSES):
        bpy.utils.unregister_class(cls)
