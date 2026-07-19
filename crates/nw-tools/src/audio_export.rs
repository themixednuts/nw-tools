//! Decode Wwise media to PCM WAV and write a playable Blender `.blend`.
//!
//! `cry-audio` stays pure (parse only). This module owns the process shells to
//! `vgmstream-cli` (WEM → WAV) and `blender` (glTF + events → NLA/VSE `.blend`).

use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{Context, Result, bail};

use crate::model_asset::ResolvedAsset;

mod schedule;
use schedule::{BlendSchedule, build_blend_schedule};

/// Locate `vgmstream-cli` on PATH or common WinGet install locations.
pub(crate) fn find_vgmstream() -> Option<PathBuf> {
    which("vgmstream-cli")
        .or_else(|| which("vgmstream-cli.exe"))
        .or_else(find_winget_vgmstream)
}

/// Locate `blender` on PATH or under Program Files.
pub(crate) fn find_blender() -> Option<PathBuf> {
    which("blender")
        .or_else(|| which("blender.exe"))
        .or_else(find_program_files_blender)
}

/// Decode every bank referenced by `audioTriggers`, ship WAVs at catalog paths,
/// and fill `media[].path` so consumers (and the blend writer) resolve in one hop.
pub(crate) fn materialize_decoded_waves(
    source: &dyn crate::model::AssetSource,
    resolved: &mut ResolvedAsset,
    vgmstream: &Path,
) -> Result<usize> {
    let mut decoded = 0usize;
    let mut seen_media: HashSet<u32> = HashSet::new();
    let mut wav_for_media: HashMap<u32, String> = HashMap::new();

    let mut jobs: Vec<(Option<String>, u32)> = Vec::new();
    for trigger in &resolved.extras.audio_triggers {
        for media in &trigger.media {
            if seen_media.insert(media.media_id) {
                jobs.push((media.bank.clone(), media.media_id));
            }
        }
    }
    jobs.sort_by(|a, b| a.1.cmp(&b.1).then_with(|| a.0.cmp(&b.0)));

    let mut skipped = Vec::new();
    for (bank_path, media_id) in jobs {
        // WEM bytes come from the owning bank's DATA section, or — for media a
        // bank references but does not embed (streamed) — the loose
        // `sounds/wwise/<mediaId>.wem` catalog entry.
        let wem: Vec<u8> = if let Some(bank_path) = bank_path.as_deref() {
            let Some(bank_bytes) = source.read(bank_path) else {
                skipped.push(format!("{media_id} (bank {bank_path} unreadable)"));
                continue;
            };
            let bank = cry_audio::WwiseSoundBank::parse(&bank_bytes)
                .with_context(|| format!("parse soundbank {bank_path}"))?;
            let Some(entry) = bank
                .media
                .iter()
                .copied()
                .find(|entry| entry.id.0 == media_id)
            else {
                skipped.push(format!("{media_id} (not embedded in {bank_path})"));
                continue;
            };
            bank.embedded_media(&bank_bytes, entry)
                .with_context(|| format!("extract media {media_id} from {bank_path}"))?
                .to_vec()
        } else {
            let wem_path = format!("sounds/wwise/{media_id}.wem");
            let Some(bytes) = source.read(&wem_path) else {
                skipped.push(format!("{media_id} (loose {wem_path} unreadable)"));
                continue;
            };
            bytes
        };
        let wav = decode_wem_with_vgmstream(vgmstream, &wem)
            .with_context(|| format!("decode media {media_id}"))?;
        let catalog_path = cry_audio::decoded_wave_catalog_path(cry_audio::WwiseMediaId(media_id));
        if !resolved
            .extras
            .resource_payloads
            .iter()
            .any(|resource| resource.source_path.eq_ignore_ascii_case(&catalog_path))
        {
            resolved
                .extras
                .resource_payloads
                .push(nw_model::CryResourcePayload::new(
                    catalog_path.clone(),
                    nw_model::CryEmbeddedResourceKind::WwiseDecodedWave,
                    wav,
                ));
            decoded += 1;
        }
        wav_for_media.insert(media_id, catalog_path);
    }

    for trigger in &mut resolved.extras.audio_triggers {
        for media in &mut trigger.media {
            if media.path.is_none() {
                media.path = wav_for_media.get(&media.media_id).cloned();
            }
        }
    }
    if !skipped.is_empty() {
        eprintln!(
            "note: {} audio media entries not decoded: {}",
            skipped.len(),
            skipped.join(", ")
        );
    }
    Ok(decoded)
}

/// Decode one WEM payload to PCM WAV bytes via `vgmstream-cli`.
pub(crate) fn decode_wem_with_vgmstream(vgmstream: &Path, wem: &[u8]) -> Result<Vec<u8>> {
    let dir = tempfile::tempdir().context("create temp dir for WEM decode")?;
    let wem_path = dir.path().join("input.wem");
    let wav_path = dir.path().join("output.wav");
    fs::write(&wem_path, wem).context("write temp WEM")?;
    let output = Command::new(vgmstream)
        .args([
            "-i",
            "-o",
            wav_path.to_str().context("temp WAV path is not UTF-8")?,
            wem_path.to_str().context("temp WEM path is not UTF-8")?,
        ])
        .output()
        .with_context(|| format!("spawn {}", vgmstream.display()))?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        let stdout = String::from_utf8_lossy(&output.stdout);
        bail!("vgmstream failed ({}):\n{stdout}\n{stderr}", output.status);
    }
    fs::read(&wav_path).context("read decoded WAV")
}

/// Build a playable `.blend` next to the package: one Blender scene per clip,
/// each with the clip active on the armature and its audio events as VSE sound
/// strips at the keyframed frames. The scene dropdown is the clip browser; no
/// clip chaining. All placement decisions are precomputed here in Rust — the
/// generated Blender-side adapter only executes the plan.
pub(crate) fn write_playable_blend(
    blender: &Path,
    gltf_path: &Path,
    package_root: &Path,
    blend_path: &Path,
) -> Result<()> {
    // Blender resolves relative paths against its own working directory, and
    // background mode exits 0 on Python errors unless --python-exit-code is set.
    // Everything crossing the process boundary — including per-sound WAV paths
    // in the schedule — must be absolute.
    let package_root_abs = std::path::absolute(package_root)
        .with_context(|| format!("absolutize {}", package_root.display()))?;
    let schedule = blend_schedule(gltf_path, &package_root_abs)?;
    let gltf_abs = std::path::absolute(gltf_path)
        .with_context(|| format!("absolutize {}", gltf_path.display()))?;
    let blend_abs = std::path::absolute(blend_path)
        .with_context(|| format!("absolutize {}", blend_path.display()))?;
    let script = blender_write_script(&gltf_abs, &blend_abs, &schedule);
    let dir = tempfile::tempdir().context("create temp dir for blender script")?;
    let script_path = dir.path().join("write_blend.py");
    fs::write(&script_path, script).context("write blender script")?;
    let output = Command::new(blender)
        .args([
            "--background",
            "--factory-startup",
            "--python-exit-code",
            "1",
            "--python",
            script_path
                .to_str()
                .context("blender script path is not UTF-8")?,
        ])
        .output()
        .with_context(|| format!("spawn {}", blender.display()))?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        let stdout = String::from_utf8_lossy(&output.stdout);
        bail!(
            "blender failed writing {} ({}):\n{stdout}\n{stderr}",
            blend_path.display(),
            output.status
        );
    }
    if !blend_path.is_file() {
        bail!(
            "blender exited 0 but {} was not written",
            blend_path.display()
        );
    }
    Ok(())
}

/// Build the exact animation/audio preview plan consumed by both the legacy
/// one-shot writer and the reusable AZoth extension.
pub(crate) fn blend_schedule(gltf_path: &Path, package_root: &Path) -> Result<BlendSchedule> {
    let document: serde_json::Value = serde_json::from_slice(
        &fs::read(gltf_path).with_context(|| format!("read {}", gltf_path.display()))?,
    )
    .context("parse glTF JSON for blend schedule")?;
    build_blend_schedule(&document, package_root)
}

pub(crate) fn blend_schedule_document(
    document: &serde_json::Value,
    package_root: &Path,
) -> Result<BlendSchedule> {
    build_blend_schedule(document, package_root)
}

/// Blender-side adapter. Deliberately logic-free: it imports the glTF once,
/// then executes the precomputed per-clip scene plan (linked-data object
/// copies, action + slot assignment, VSE strips at fixed frames) and saves.
/// Version shims for the 4.x `sequences` → 5.x `strips` rename are the only
/// branching. Regenerated on every run; never shipped or kept on disk.
const BLENDER_ADAPTER: &str = r#"
def _seq_new_sound(sc, name, filepath, channel, frame):
    # sequence_editor_create() does not reliably return the editor; re-read the
    # property. 5.x renamed the container `sequences` -> `strips`.
    if sc.sequence_editor is None:
        sc.sequence_editor_create()
    sed = sc.sequence_editor
    for attr in ("strips", "sequences"):
        api = getattr(sed, attr, None)
        if api is not None and hasattr(api, "new_sound"):
            return api.new_sound(name, filepath, channel, frame)
    raise RuntimeError("SequenceEditor has no strips/sequences.new_sound")

def main():
    # Factory startup ships Cube/Camera/Light; the package is the whole scene.
    for obj in list(bpy.data.objects):
        bpy.data.objects.remove(obj, do_unlink=True)
    # The glTF importer converts each animation's second-based times to frames at
    # the scene fps in effect NOW, so set the blend fps before importing or the
    # actions end short of every clip scene's frame_end.
    fps = int(CFG["fps"])
    bpy.context.scene.render.fps = fps
    bpy.ops.import_scene.gltf(filepath=CFG["gltf"])
    base = bpy.context.scene
    # Name the base scene after the manifest stem (e.g. "alligator") so the file
    # opens on the package, with the per-clip scenes subordinate in the dropdown.
    base.name = CFG["baseName"]
    arm = next(o for o in bpy.data.objects if o.type == "ARMATURE")
    # Exclude bone-display widgets the importer creates (e.g. its Icosphere) —
    # they are custom shapes, not scene content.
    widgets = {pb.custom_shape for pb in arm.pose.bones if pb.custom_shape}
    meshes = [o for o in bpy.data.objects if o.type == "MESH" and o not in widgets]
    actions = {a.name: a for a in bpy.data.actions}
    sounds = {}

    for clip in CFG["clips"]:
        action = actions.get(clip["action"]) or next(
            (a for a in bpy.data.actions if a.name.startswith(clip["action"])), None
        )
        if action is None:
            print("skip missing action", clip["action"])
            continue
        sc = bpy.data.scenes.new(clip["name"])
        sc.render.fps = fps
        sc.frame_start = 1
        sc.frame_end = clip["frameEnd"]
        # Real-time audio during viewport playback (drop frames to keep sync).
        sc.sync_mode = "AUDIO_SYNC"
        sc.use_audio = True  # RNA "Play Audio": True = audible, False = muted
        sc["cryCharacterEventDispatches"] = json.dumps(
            clip["dispatches"], separators=(",", ":")
        )

        # Clip-named copies instead of Blender's `.001` suffixes: object names
        # are file-global, so 57 copies of "Armature" would otherwise number up.
        arm_copy = arm.copy()  # shares armature data
        arm_copy.name = clip["name"] + ".rig"
        arm_copy.animation_data_clear()
        arm_copy.animation_data_create()
        arm_copy.animation_data.action = action
        if len(action.slots):
            arm_copy.animation_data.action_slot = action.slots[0]
        sc.collection.objects.link(arm_copy)
        for mesh in meshes:
            copy = mesh.copy()  # shares mesh data
            copy.name = clip["name"] + "." + mesh.name
            if copy.parent == arm:
                copy.parent = arm_copy
            for modifier in copy.modifiers:
                if modifier.type == "ARMATURE" and modifier.object == arm:
                    modifier.object = arm_copy
            sc.collection.objects.link(copy)

        for sound in clip["sounds"]:
            wav = sound["wav"]
            if wav not in sounds:
                snd = bpy.data.sounds.load(wav, check_existing=True)
                # Packed audio plays reliably during viewport playback only when
                # cached in memory; set it once per loaded sound.
                snd.use_memory_cache = True
                sounds[wav] = snd
            strip = _seq_new_sound(sc, sound["name"], wav, sound["channel"], sound["frame"])
            # A zero-volume strip is silent even with the device configured; force
            # audible playback.
            if strip is not None and hasattr(strip, "volume"):
                strip.volume = 1.0
            if strip is not None and "endFrame" in sound:
                duration = max(1, int(sound["endFrame"]) - int(sound["frame"]))
                if hasattr(strip, "frame_final_duration"):
                    strip.frame_final_duration = duration

    # Pack sounds so the .blend stays playable when the package moves. The
    # import scene is already the active one; background mode has no window
    # to retarget.
    for sound in sounds.values():
        sound.pack()
    # Open textured: factory startup shows Solid shading, so packed materials look
    # absent until the user switches to Material Preview. Persist Material shading
    # on every saved 3D viewport (images are already packed by the importer).
    for screen in bpy.data.screens:
        for area in screen.areas:
            if area.type == "VIEW_3D":
                for space in area.spaces:
                    if space.type == "VIEW_3D":
                        space.shading.type = "MATERIAL"
    bpy.ops.wm.save_as_mainfile(filepath=CFG["out"])
    print("saved", CFG["out"], "with", len(CFG["clips"]), "clip scenes,", len(sounds), "sounds")

main()
"#;

fn blender_write_script(gltf_path: &Path, blend_path: &Path, schedule: &BlendSchedule) -> String {
    // The base scene takes the manifest stem (the glTF filename without its
    // extension), computed here so the Blender adapter stays logic-free.
    let base_name = gltf_path
        .file_stem()
        .and_then(|stem| stem.to_str())
        .filter(|stem| !stem.is_empty())
        .unwrap_or("import");
    let payload = serde_json::json!({
        "gltf": gltf_path.to_string_lossy().replace('\\', "/"),
        "out": blend_path.to_string_lossy().replace('\\', "/"),
        "baseName": base_name,
        "fps": schedule.fps,
        "clips": schedule.clips,
    });
    let blob = payload.to_string();
    // The plan JSON is path/number/name only; embed as a Python triple-quoted
    // string without format!-escaping gymnastics.
    let mut script = String::new();
    script.push_str("# Generated by nw-tools format model — do not edit by hand.\n");
    script.push_str("import json\nimport bpy\n\n");
    script.push_str("CFG = json.loads('''");
    script.push_str(&blob);
    script.push_str("''')\n");
    script.push_str(BLENDER_ADAPTER);
    script
}

fn which(name: &str) -> Option<PathBuf> {
    let path = std::env::var_os("PATH")?;
    for dir in std::env::split_paths(&path) {
        let candidate = dir.join(name);
        if candidate.is_file() {
            return Some(candidate);
        }
        #[cfg(windows)]
        {
            let exe = dir.join(format!("{name}.exe"));
            if exe.is_file() {
                return Some(exe);
            }
        }
    }
    None
}

fn find_winget_vgmstream() -> Option<PathBuf> {
    let home = dirs_next_home()?;
    let root = home.join("AppData/Local/Microsoft/WinGet/Packages");
    if !root.is_dir() {
        return None;
    }
    let Ok(entries) = fs::read_dir(&root) else {
        return None;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }
        let candidate = path.join("vgmstream-cli.exe");
        if candidate.is_file() {
            return Some(candidate);
        }
        if let Ok(inner) = fs::read_dir(&path) {
            for child in inner.flatten() {
                let nested = child.path().join("vgmstream-cli.exe");
                if nested.is_file() {
                    return Some(nested);
                }
            }
        }
    }
    None
}

fn find_program_files_blender() -> Option<PathBuf> {
    let roots = [
        PathBuf::from(r"C:\Program Files\Blender Foundation"),
        PathBuf::from(r"C:\Program Files (x86)\Blender Foundation"),
    ];
    for root in roots {
        if !root.is_dir() {
            continue;
        }
        let Ok(entries) = fs::read_dir(&root) else {
            continue;
        };
        let mut versions = entries
            .flatten()
            .map(|entry| entry.path())
            .filter(|path| path.is_dir())
            .collect::<Vec<_>>();
        versions.sort();
        versions.reverse();
        for dir in versions {
            let exe = dir.join("blender.exe");
            if exe.is_file() {
                return Some(exe);
            }
        }
    }
    None
}

fn dirs_next_home() -> Option<PathBuf> {
    std::env::var_os("USERPROFILE")
        .or_else(|| std::env::var_os("HOME"))
        .map(PathBuf::from)
}
