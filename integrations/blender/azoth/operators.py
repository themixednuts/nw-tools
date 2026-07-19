"""One-click import, linked-workspace, export, animation, and audio operators."""

import json
import gc
from pathlib import Path
import subprocess

import bpy
from bpy.props import StringProperty
from bpy_extras.io_utils import ImportHelper

from . import bridge, metadata, organizer, presentation


_DATA_TABLES = ("objects", "meshes", "armatures", "materials", "images", "actions", "sounds")


def refresh_manifests(state):
    previous = state.manifests[state.manifest_index].path if state.manifests else ""
    state.manifests.clear()
    selected = 0
    for index, summary in enumerate(metadata.scan_manifests()):
        item = state.manifests.add()
        item.label = summary.label
        item.path = str(summary.path)
        item.source_path = summary.source_path
        item.animation_count = summary.animation_count
        item.resource_count = summary.resource_count
        item.issue_count = summary.issue_count
        if item.path == previous:
            selected = index
    state.manifest_index = min(selected, max(0, len(state.manifests) - 1))
    state.status = f"{len(state.manifests)} package(s) in {metadata.OUTPUT_ROOT}"


class AZOTH_OT_refresh(bpy.types.Operator):
    bl_idname = "azoth.refresh"
    bl_label = "Refresh AZoth Library"
    bl_description = "Rescan C:\\nwt for nw-tools glTF and GLB packages"

    def execute(self, context):
        refresh_manifests(context.scene.azoth)
        return {"FINISHED"}


class AZOTH_OT_import_selected(bpy.types.Operator):
    bl_idname = "azoth.import_selected"
    bl_label = "Open AZoth Workspace"
    bl_description = "Import, organize, link shared data, and save the character workspace"
    bl_options = {"REGISTER"}

    @classmethod
    def poll(cls, context):
        state = getattr(context.scene, "azoth", None)
        return state is not None and bool(state.manifests)

    def execute(self, context):
        state = context.scene.azoth
        manifest = Path(state.manifests[state.manifest_index].path)
        return _import_manifest(self, context, manifest)


class AZOTH_OT_import_file(bpy.types.Operator, ImportHelper):
    bl_idname = "azoth.import_file"
    bl_label = "Import AZoth Package"
    bl_description = "Import an nw-tools glTF or GLB package with full AZoth organization"
    filename_ext = ".gltf"
    filter_glob: StringProperty(default="*.gltf;*.glb", options={"HIDDEN"})

    def execute(self, context):
        return _import_manifest(self, context, Path(self.filepath))


class AZOTH_OT_export_asset(bpy.types.Operator):
    bl_idname = "azoth.export_asset"
    bl_label = "Export from New World"
    bl_description = "Run nw-tools in parallel and write the complete structured package to C:\\nwt"
    bl_options = {"REGISTER"}

    _process = None
    _timer = None
    _log = None

    def execute(self, context):
        state = context.scene.azoth
        asset_filter = state.export_filter.strip()
        if not asset_filter:
            self.report({"ERROR"}, "Enter an asset name or catalog path")
            return {"CANCELLED"}
        try:
            command = bridge.export_command(asset_filter)
        except FileNotFoundError as error:
            self.report({"ERROR"}, str(error))
            return {"CANCELLED"}
        metadata.AZOTH_ROOT.mkdir(parents=True, exist_ok=True)
        log_path = metadata.AZOTH_ROOT / "export.log"
        self._log = log_path.open("w", encoding="utf-8")
        self._process = subprocess.Popen(
            command,
            stdout=self._log,
            stderr=subprocess.STDOUT,
            creationflags=getattr(subprocess, "CREATE_NO_WINDOW", 0),
        )
        self._timer = context.window_manager.event_timer_add(0.5, window=context.window)
        context.window_manager.modal_handler_add(self)
        state.status = f"Exporting {asset_filter} with nw-jobs…"
        return {"RUNNING_MODAL"}

    def modal(self, context, event):
        if event.type == "ESC":
            self._process.terminate()
            return self._finish(context, False, "Export cancelled")
        if event.type != "TIMER" or self._process.poll() is None:
            return {"PASS_THROUGH"}
        ok = self._process.returncode == 0
        message = "Export complete" if ok else "Export failed; see C:\\nwt\\.azoth\\export.log"
        return self._finish(context, ok, message)

    def _finish(self, context, ok, message):
        if self._timer is not None:
            context.window_manager.event_timer_remove(self._timer)
            self._timer = None
        if self._log is not None:
            self._log.close()
            self._log = None
        refresh_manifests(context.scene.azoth)
        context.scene.azoth.status = message
        self.report({"INFO"} if ok else {"ERROR"}, message)
        return {"FINISHED"} if ok else {"CANCELLED"}


class AZOTH_OT_apply_animation(bpy.types.Operator):
    bl_idname = "azoth.apply_animation"
    bl_label = "Play Selected Animation"
    bl_description = "Apply the Action and build its exact nw-tools audio timeline without duplicating scenes"

    @classmethod
    def poll(cls, context):
        state = getattr(context.scene, "azoth", None)
        return state is not None and bool(state.animations)

    def execute(self, context):
        state = context.scene.azoth
        item = state.animations[state.animation_index]
        track_index = _track_index(context.scene, item.action)
        if track_index is not None:
            bpy.ops.scene.gltf2_animation_apply(index=track_index)
        elif not _apply_nla_action(context.scene, item.action):
            self.report({"ERROR"}, f"Animation track not found: {item.action}")
            return {"CANCELLED"}
        context.scene.frame_start = 1
        context.scene.frame_end = item.frame_end
        context.scene.frame_set(1)
        if item.schedule_index >= 0:
            try:
                schedule = bridge.schedule(state.manifest_path)
                clip = schedule["clips"][item.schedule_index]
                context.scene.render.fps = int(schedule["fps"])
                context.scene["azoth.character_event_dispatches"] = json.dumps(
                    clip.get("dispatches", []), separators=(",", ":")
                )
                if state.load_audio:
                    _replace_audio(context.scene, clip.get("sounds", []))
            except (OSError, RuntimeError, subprocess.SubprocessError, json.JSONDecodeError) as error:
                self.report({"WARNING"}, f"Action applied; audio unavailable: {error}")
        state.status = f"Animation: {item.name}"
        return {"FINISHED"}


class AZOTH_OT_clean_character_view(bpy.types.Operator):
    bl_idname = "azoth.clean_character_view"
    bl_label = "Clean Character View"
    bl_description = "Hide engine helpers and rig overlays, center the render geometry, and show materials"

    @classmethod
    def poll(cls, context):
        return presentation.find_scene_root(context.scene) is not None

    def execute(self, context):
        root = presentation.find_scene_root(context.scene)
        presentation.configure_workspace(context, root)
        return {"FINISHED"}


def _import_manifest(operator, context, manifest):
    try:
        manifest = manifest.resolve(strict=True)
        if metadata.OUTPUT_ROOT.resolve() not in manifest.parents:
            raise metadata.ManifestError(f"AZoth packages must be under {metadata.OUTPUT_ROOT}")
    except (OSError, metadata.ManifestError) as error:
        operator.report({"ERROR"}, str(error))
        return {"CANCELLED"}

    state = context.scene.azoth
    if state.make_linked_workspace:
        library, workspace = metadata.workspace_paths(manifest)
        if _workspace_is_current(manifest, library, workspace):
            if Path(bpy.data.filepath).resolve() == workspace.resolve():
                state.status = f"Workspace already open: {workspace}"
                operator.report({"INFO"}, state.status)
                return {"FINISHED"}
            operator.report({"INFO"}, f"Opening linked workspace: {workspace}")
            bpy.ops.wm.open_mainfile(filepath=str(workspace))
            return {"FINISHED"}
    try:
        description = bridge.describe(manifest)
    except (OSError, RuntimeError, subprocess.SubprocessError, json.JSONDecodeError) as error:
        operator.report({"WARNING"}, f"nw-tools describe unavailable; using local fallback: {error}")
        description = None
    if description is None:
        try:
            document = metadata.load_document(manifest)
            description = metadata.description_from_document(document, metadata.OUTPUT_ROOT, manifest)
            del document
            gc.collect()
        except (OSError, metadata.ManifestError) as error:
            operator.report({"ERROR"}, str(error))
            return {"CANCELLED"}
    _clear_factory_objects()
    before = _snapshot()
    sample_rates = [float(animation.get("sampleRate") or 0.0) for animation in description["animations"]]
    context.scene.render.fps = max(1, round(max(sample_rates, default=30.0)))
    result = bpy.ops.import_scene.gltf(
        filepath=str(manifest),
        import_pack_images=False,
        import_scene_extras=True,
        import_scene_as_collection=False,
        import_select_created_objects=True,
        disable_bone_shape=True,
    )
    if "FINISHED" not in result:
        operator.report({"ERROR"}, f"Blender could not import {manifest}")
        return {"CANCELLED"}
    after = _snapshot()
    imported = {name: after[name] - before[name] for name in _DATA_TABLES}
    root = organizer.organize(
        context, manifest, str(description.get("sourcePath", "")), imported["objects"], imported
    )
    _populate_state(state, manifest, description)

    if state.make_linked_workspace:
        try:
            library, workspace = _make_linked_workspace(context, root, imported, manifest)
            state.linked_library = str(library)
            bpy.ops.wm.save_as_mainfile(filepath=str(workspace), compress=True)
            state.status = f"Linked workspace: {workspace}"
        except (OSError, RuntimeError, metadata.ManifestError) as error:
            state.status = f"Imported locally; linked workspace failed: {error}"
            operator.report({"WARNING"}, state.status)
    else:
        presentation.configure_workspace(context, root)
        state.status = f"Imported and organized: {manifest.name}"
    operator.report({"INFO"}, state.status)
    return {"FINISHED"}


def _populate_state(state, manifest, description):
    state.manifest_path = str(manifest)
    state.source_path = str(description.get("sourcePath", ""))
    state.resources.clear()
    for record in description.get("resources", []):
        item = state.resources.add()
        item.kind = str(record.get("kind", "dependency"))
        item.path = str(record.get("path", ""))
        item.category = str(record.get("category") or metadata.category_for(item.kind, item.path))
        item.status = str(record.get("status", "ready"))
    state.resource_index = 0
    state.animations.clear()
    try:
        schedule = bridge.schedule(manifest)
    except (OSError, RuntimeError, subprocess.SubprocessError, json.JSONDecodeError):
        schedule = None
    source_by_name = {str(record.get("name", "")): record for record in description["animations"]}
    if schedule:
        for index, clip in enumerate(schedule.get("clips", [])):
            action = str(clip.get("action", ""))
            source = source_by_name.get(action)
            item = state.animations.add()
            item.name = str(clip.get("name", action))
            item.action = action
            item.source_path = str(source.get("sourcePath", "")) if source else ""
            item.duration = f"{float(source.get('duration') or 0.0):.3f}s" if source else ""
            item.frame_end = max(1, int(clip.get("frameEnd", 1)))
            item.audio_count = len(clip.get("sounds") or [])
            item.schedule_index = index
    else:
        fps = context_fps = 30
        for record in description["animations"]:
            item = state.animations.add()
            item.name = str(record.get("name", "animation"))
            item.action = item.name
            item.source_path = str(record.get("sourcePath", ""))
            duration = float(record.get("duration") or 0.0)
            item.duration = f"{duration:.3f}s"
            fps = max(1, round(float(record.get("sampleRate") or context_fps)))
            item.frame_end = max(1, round(duration * fps))
            item.audio_count = int(record.get("audioCount") or 0)
    state.animation_index = 0


def _make_linked_workspace(context, root, imported, manifest):
    library, workspace = metadata.workspace_paths(manifest)
    library.parent.mkdir(parents=True, exist_ok=True)
    workspace.parent.mkdir(parents=True, exist_ok=True)
    root["azoth.manifest_identity"] = metadata.manifest_identity(manifest)
    root_name = root.name
    bpy.data.libraries.write(str(library), {root}, path_remap="RELATIVE", fake_user=True, compress=True)

    collections = list(_walk_collections(root))
    for collection in reversed(collections):
        if collection.name in bpy.data.collections:
            bpy.data.collections.remove(collection, do_unlink=True)
    if root.name in bpy.data.collections:
        bpy.data.collections.remove(root, do_unlink=True)
    for table_name in _DATA_TABLES:
        table = getattr(bpy.data, table_name)
        for block in imported[table_name]:
            current = table.get(block.name)
            if current is not None and current.users == 0:
                table.remove(current)

    with bpy.data.libraries.load(str(library), link=True) as (source, target):
        if root_name not in source.collections:
            raise RuntimeError(f"linked library has no {root_name} collection")
        target.collections = [root_name]
    linked = target.collections[0]
    context.scene.collection.children.link(linked)
    organizer.linked_override(context, linked)
    presentation.configure_workspace(context, linked)
    bpy.ops.object.select_all(action="DESELECT")
    context.view_layer.objects.active = None
    return library, workspace


def _workspace_is_current(manifest, library, workspace):
    try:
        source_times = [manifest.stat().st_mtime_ns]
        source_times.extend(path.stat().st_mtime_ns for path in Path(__file__).parent.glob("*.py"))
        source_time = max(source_times)
        return (
            library.is_file()
            and workspace.is_file()
            and library.stat().st_mtime_ns >= source_time
            and workspace.stat().st_mtime_ns >= source_time
        )
    except OSError:
        return False


def _walk_collections(root):
    for child in root.children:
        yield child
        yield from _walk_collections(child)


def _snapshot():
    return {name: set(getattr(bpy.data, name)) for name in _DATA_TABLES}


def _track_index(scene, action):
    tracks = getattr(scene, "gltf2_animation_tracks", ())
    for index, track in enumerate(tracks):
        if track.name == action or track.name.startswith(action):
            return index
    return None


def _apply_nla_action(scene, action_name):
    """Apply one imported glTF NLA track when Blender's scene list was not retained."""

    found = False
    for obj in scene.objects:
        animation = obj.animation_data
        if animation is not None:
            match = next(
                (
                    track.strips[0]
                    for track in animation.nla_tracks
                    if track.name == action_name and track.strips and track.strips[0].action is not None
                ),
                None,
            )
            if match is not None:
                animation.action = match.action
                if hasattr(animation, "action_slot"):
                    animation.action_slot = match.action_slot
                found = True
        shape_keys = obj.data.shape_keys if obj.type == "MESH" and obj.data else None
        shape_animation = shape_keys.animation_data if shape_keys else None
        if shape_animation is not None:
            match = next(
                (
                    track.strips[0]
                    for track in shape_animation.nla_tracks
                    if track.name == action_name and track.strips and track.strips[0].action is not None
                ),
                None,
            )
            if match is not None:
                shape_animation.action = match.action
                if hasattr(shape_animation, "action_slot"):
                    shape_animation.action_slot = match.action_slot
                found = True
    return found


def _replace_audio(scene, sounds):
    if scene.sequence_editor is None:
        scene.sequence_editor_create()
    editor = scene.sequence_editor
    strips = getattr(editor, "strips", None)
    if strips is None:
        strips = getattr(editor, "sequences", None)
    if strips is None:
        raise RuntimeError("Blender Sequence Editor sound-strip API is unavailable")
    for strip in list(strips):
        if strip.name.startswith("AZoth | "):
            strips.remove(strip)
    for sound in sounds:
        path = Path(sound["wav"])
        if not path.is_file():
            continue
        strip = strips.new_sound(
            "AZoth | " + str(sound["name"]),
            str(path),
            int(sound["channel"]),
            int(sound["frame"]),
        )
        strip.volume = 1.0
        if sound.get("endFrame") is not None:
            strip.frame_final_duration = max(1, int(sound["endFrame"]) - int(sound["frame"]))
    scene.sync_mode = "AUDIO_SYNC"
    scene.use_audio = True


def _clear_factory_objects():
    if bpy.data.filepath or {obj.name for obj in bpy.context.scene.objects} - {"Cube", "Camera", "Light"}:
        return
    for obj in list(bpy.context.scene.objects):
        bpy.data.objects.remove(obj, do_unlink=True)


CLASSES = (
    AZOTH_OT_refresh,
    AZOTH_OT_import_selected,
    AZOTH_OT_import_file,
    AZOTH_OT_export_asset,
    AZOTH_OT_apply_animation,
    AZOTH_OT_clean_character_view,
)


def register():
    for cls in CLASSES:
        bpy.utils.register_class(cls)


def unregister():
    for cls in reversed(CLASSES):
        bpy.utils.unregister_class(cls)
