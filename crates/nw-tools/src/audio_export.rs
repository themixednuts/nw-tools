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
    let document: serde_json::Value = serde_json::from_slice(
        &fs::read(gltf_path).with_context(|| format!("read {}", gltf_path.display()))?,
    )
    .context("parse glTF JSON for blend schedule")?;
    // Blender resolves relative paths against its own working directory, and
    // background mode exits 0 on Python errors unless --python-exit-code is set.
    // Everything crossing the process boundary — including per-sound WAV paths
    // in the schedule — must be absolute.
    let package_root_abs = std::path::absolute(package_root)
        .with_context(|| format!("absolutize {}", package_root.display()))?;
    let schedule = build_blend_schedule(&document, &package_root_abs)?;
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

#[derive(Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct BlendSchedule {
    /// One fps for the whole file. The glTF importer converts each animation's
    /// second-based keyframe times to frames at the *scene* fps in effect when
    /// it runs, so the adapter must set this on the base scene BEFORE import and
    /// on every clip scene — otherwise the imported action (placed at Blender's
    /// factory-default fps) ends before the scene's `frame_end` and the last
    /// stretch of every clip plays with no motion.
    fps: u32,
    clips: Vec<BlendClip>,
}

/// One per-clip Blender scene, fully placed: the adapter executes this plan
/// verbatim (no Blender-side arithmetic).
#[derive(Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct BlendClip {
    /// Action name (also the scene name).
    name: String,
    /// Scene frame range is `1..=frame_end`, computed at the schedule fps so it
    /// coincides with the imported action's last keyframe.
    frame_end: u32,
    sounds: Vec<BlendSound>,
}

#[derive(Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct BlendSound {
    /// VSE strip name (clip.parameter.frame — unique per placement).
    name: String,
    /// Absolute scene frame for the strip start.
    frame: u32,
    /// VSE channel row, precycled in Rust.
    channel: u32,
    /// Absolute path to the decoded WAV (forward slashes).
    wav: String,
}

/// The decoded WAVs of the default surface's **engine-faithful weighted sequence**
/// (`surfaceMedia[default].sequence`) mapped to disk paths, in sequence order and
/// filtered to those present on disk. This is the deterministic order the engine's
/// `CAkRanSeqCntr` would play (weights + avoid-repeat honored), precomputed in the
/// manifest — the blend assigns consecutive footsteps straight down it, so no
/// bank re-parsing happens here. Empty when the manifest carries no default
/// sequence (older manifests), in which case the caller falls back.
fn default_sequence_wavs(trigger: &serde_json::Value, package_root: &Path) -> Vec<PathBuf> {
    let media = trigger
        .get("media")
        .and_then(|value| value.as_array())
        .cloned()
        .unwrap_or_default();
    let Some(default_surface) = trigger
        .get("surfaceMedia")
        .and_then(|value| value.as_array())
        .into_iter()
        .flatten()
        .find(|surface| {
            surface
                .get("default")
                .and_then(serde_json::Value::as_bool)
                .unwrap_or(false)
        })
    else {
        return Vec::new();
    };
    default_surface
        .get("sequence")
        .and_then(|value| value.as_array())
        .into_iter()
        .flatten()
        .filter_map(|value| value.as_u64())
        .filter_map(|media_id| wav_for_media_id(media_id, &media, package_root))
        .collect()
}

/// Resolve one media id to its decoded WAV path (the manifest's explicit
/// `media[].path` when present, else the catalog decode path), keeping only files
/// that exist on disk.
fn wav_for_media_id(media_id: u64, media: &[serde_json::Value], package_root: &Path) -> Option<PathBuf> {
    let explicit = media
        .iter()
        .find(|entry| entry.get("mediaId").and_then(serde_json::Value::as_u64) == Some(media_id))
        .and_then(|entry| entry.get("path"))
        .and_then(|value| value.as_str())
        .map(str::to_owned);
    let rel = explicit.unwrap_or_else(|| {
        cry_audio::decoded_wave_catalog_path(cry_audio::WwiseMediaId(media_id as u32))
    });
    let absolute = package_root.join(rel.replace('\\', "/"));
    absolute.is_file().then_some(absolute)
}

/// The decoded WAVs the default switch branch plays for a trigger — the default
/// surface's variations — ordered by media id and filtered to those present on
/// disk. Legacy fallback used only when the manifest carries no weighted
/// `sequence` (see [`default_sequence_wavs`]).
fn default_branch_wavs(media: &[serde_json::Value], package_root: &Path) -> Vec<PathBuf> {
    let mut entries: Vec<(u64, PathBuf)> = media
        .iter()
        .filter(|entry| {
            entry
                .get("defaultBranch")
                .and_then(serde_json::Value::as_bool)
                .unwrap_or(false)
        })
        .filter_map(|entry| {
            let media_id = entry.get("mediaId").and_then(serde_json::Value::as_u64)?;
            let rel = entry
                .get("path")
                .and_then(|value| value.as_str())
                .map(str::to_owned)
                .unwrap_or_else(|| {
                    cry_audio::decoded_wave_catalog_path(cry_audio::WwiseMediaId(media_id as u32))
                });
            let absolute = package_root.join(rel.replace('\\', "/"));
            absolute.is_file().then_some((media_id, absolute))
        })
        .collect();
    entries.sort_by_key(|(media_id, _)| *media_id);
    entries.into_iter().map(|(_, path)| path).collect()
}

/// The preview WAV for a trigger: the decoded media with the lowest media id
/// whose WAV is present on disk. Deterministic — a stable choice for the strip
/// regardless of how many variations the trigger reaches.
fn lowest_media_wav(media: &[serde_json::Value], package_root: &Path) -> Option<PathBuf> {
    media
        .iter()
        .filter_map(|entry| {
            let media_id = entry.get("mediaId").and_then(serde_json::Value::as_u64)?;
            let rel = entry
                .get("path")
                .and_then(|value| value.as_str())
                .map(str::to_owned)
                .unwrap_or_else(|| {
                    cry_audio::decoded_wave_catalog_path(cry_audio::WwiseMediaId(media_id as u32))
                });
            let absolute = package_root.join(rel.replace('\\', "/"));
            absolute.is_file().then_some((media_id, absolute))
        })
        .min_by_key(|(media_id, _)| *media_id)
        .map(|(_, path)| path)
}

fn build_blend_schedule(
    document: &serde_json::Value,
    package_root: &Path,
) -> Result<BlendSchedule> {
    let extras = document
        .get("extras")
        .cloned()
        .unwrap_or(serde_json::json!({}));
    let triggers = extras
        .get("audioTriggers")
        .and_then(|value| value.as_array())
        .cloned()
        .unwrap_or_default();

    let mut trigger_variations: HashMap<String, Vec<PathBuf>> = HashMap::new();
    for trigger in &triggers {
        let name = trigger
            .get("trigger")
            .and_then(|value| value.as_str())
            .unwrap_or_default();
        if name.is_empty() {
            continue;
        }
        let media = trigger
            .get("media")
            .and_then(|value| value.as_array())
            .cloned()
            .unwrap_or_default();
        // A trigger's events can reach many variations; the preview follows the
        // default surface's engine-faithful weighted sequence (precomputed in the
        // manifest) so consecutive footsteps play the samples the engine would.
        // Older manifests without a sequence fall back to the default branch's
        // media-id order, then to the single lowest media id.
        let mut variations = default_sequence_wavs(trigger, package_root);
        if variations.is_empty() {
            variations = default_branch_wavs(&media, package_root);
        }
        if variations.is_empty() {
            variations.extend(lowest_media_wav(&media, package_root));
        }
        if !variations.is_empty() {
            trigger_variations.insert(name.to_owned(), variations);
        }
    }

    let animations = document
        .get("animations")
        .and_then(|value| value.as_array())
        .cloned()
        .unwrap_or_default();

    // One fps for the whole blend: the max authored sample rate (so nothing is
    // undersampled), default 30. Used for import, every scene, and every
    // frame_end, keeping the imported actions, scene ranges, and sound frames
    // on one timeline.
    let clip_sample_rate = |animation: &serde_json::Value| {
        animation
            .get("extras")
            .and_then(|extras| extras.get("crySampleRate"))
            .and_then(serde_json::Value::as_f64)
            .filter(|value| *value > 1.0)
    };
    let fps = animations
        .iter()
        .filter_map(clip_sample_rate)
        .fold(30.0_f64, f64::max)
        .round()
        .max(1.0) as u32;

    let mut clips = Vec::new();
    // Per-trigger cursor: consecutive events for a surface walk straight down the
    // engine's weighted selection sequence (cycling only if a clip has more
    // footsteps than the precomputed sequence length).
    let mut rotation: HashMap<String, usize> = HashMap::new();
    for animation in animations {
        let name = animation
            .get("name")
            .and_then(|value| value.as_str())
            .unwrap_or("clip")
            .to_owned();
        let anim_extras = animation
            .get("extras")
            .cloned()
            .unwrap_or(serde_json::json!({}));
        let duration = anim_extras
            .get("cryDuration")
            .and_then(|value| value.as_f64())
            .unwrap_or(1.0)
            .max(0.001);
        let frame_end = (duration * f64::from(fps)).round().max(1.0) as u32;
        let mut sounds = Vec::new();
        let mut channel = 1u32;
        for event in anim_extras
            .get("cryEvents")
            .and_then(|value| value.as_array())
            .into_iter()
            .flatten()
        {
            let parameter = event
                .get("parameter")
                .and_then(|value| value.as_str())
                .unwrap_or("")
                .trim();
            if parameter.is_empty() {
                continue;
            }
            let Some(variations) = trigger_variations.get(parameter) else {
                continue;
            };
            let cursor = rotation.entry(parameter.to_owned()).or_default();
            let wav = &variations[*cursor % variations.len()];
            *cursor += 1;
            let normalized = event
                .get("normalizedTime")
                .and_then(|value| value.as_f64())
                .unwrap_or(0.0)
                .clamp(0.0, 1.0);
            let frame = 1 + (normalized * f64::from(frame_end)).round() as u32;
            sounds.push(BlendSound {
                name: format!("{name}.{parameter}.{frame}"),
                frame,
                channel,
                wav: wav.to_string_lossy().replace('\\', "/"),
            });
            channel = 1 + (channel % 8);
        }
        // Mannequin fragment audio (creature vocals/actions): each clip fires its
        // ATL trigger at an absolute `startTime` (seconds) rather than a
        // normalized keyframe. Placed with the same weighted-sequence selection as
        // footsteps so the same trigger stays consistent across scenes.
        for clip in anim_extras
            .get("cryMannequinAudio")
            .and_then(|value| value.as_array())
            .into_iter()
            .flatten()
        {
            let trigger = clip
                .get("trigger")
                .and_then(|value| value.as_str())
                .unwrap_or("")
                .trim();
            if trigger.is_empty() {
                continue;
            }
            let Some(variations) = trigger_variations.get(trigger) else {
                continue;
            };
            let cursor = rotation.entry(trigger.to_owned()).or_default();
            let wav = &variations[*cursor % variations.len()];
            *cursor += 1;
            let start_time = clip
                .get("startTime")
                .and_then(serde_json::Value::as_f64)
                .unwrap_or(0.0)
                .max(0.0);
            let frame = 1 + (start_time * f64::from(fps)).round() as u32;
            sounds.push(BlendSound {
                name: format!("{name}.{trigger}.{frame}"),
                frame,
                channel,
                wav: wav.to_string_lossy().replace('\\', "/"),
            });
            channel = 1 + (channel % 8);
        }
        // Every clip gets a scene — the dropdown doubles as the clip browser;
        // clips without audio simply have no sound strips.
        clips.push(BlendClip {
            name,
            frame_end,
            sounds,
        });
    }
    Ok(BlendSchedule { fps, clips })
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
        action = actions.get(clip["name"]) or next(
            (a for a in bpy.data.actions if a.name.startswith(clip["name"])), None
        )
        if action is None:
            print("skip missing action", clip["name"])
            continue
        sc = bpy.data.scenes.new(clip["name"])
        sc.render.fps = fps
        sc.frame_start = 1
        sc.frame_end = clip["frameEnd"]
        # Real-time audio during viewport playback (drop frames to keep sync).
        sc.sync_mode = "AUDIO_SYNC"
        sc.use_audio = True  # RNA "Play Audio": True = audible, False = muted

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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lowest_media_wav_is_deterministic_by_media_id() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        for id in [500u32, 100u32] {
            let path = root.join(cry_audio::decoded_wave_catalog_path(
                cry_audio::WwiseMediaId(id),
            ));
            fs::create_dir_all(path.parent().unwrap()).unwrap();
            fs::write(&path, b"RIFF").unwrap();
        }

        // Out of two decoded variations, the lowest media id wins — regardless
        // of array order.
        let media = vec![
            serde_json::json!({ "mediaId": 500 }),
            serde_json::json!({ "mediaId": 100 }),
        ];
        let chosen = lowest_media_wav(&media, root).unwrap();
        assert!(chosen.ends_with("100.wav"), "chose {chosen:?}");

        // A media id whose WAV is absent is ignored; the lowest *present* wins.
        let media = vec![
            serde_json::json!({ "mediaId": 1 }),
            serde_json::json!({ "mediaId": 500 }),
        ];
        let chosen = lowest_media_wav(&media, root).unwrap();
        assert!(chosen.ends_with("500.wav"), "chose {chosen:?}");
    }

    #[test]
    fn blend_schedule_rotates_default_branch_variations_per_event() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        // Two default-branch variations (10, 20) and one non-default sample (5),
        // all decoded on disk.
        for id in [10u32, 20u32, 5u32] {
            let path = root.join(cry_audio::decoded_wave_catalog_path(
                cry_audio::WwiseMediaId(id),
            ));
            fs::create_dir_all(path.parent().unwrap()).unwrap();
            fs::write(&path, b"RIFF").unwrap();
        }

        // Media array is deliberately out of order and mixes in the non-default
        // sample; only the default-branch variations rotate, ordered by media id.
        let document = serde_json::json!({
            "extras": {
                "audioTriggers": [{
                    "trigger": "step",
                    "media": [
                        { "mediaId": 20, "defaultBranch": true },
                        { "mediaId": 5 },
                        { "mediaId": 10, "defaultBranch": true },
                    ],
                }],
            },
            "animations": [{
                "name": "walk",
                "extras": {
                    "cryDuration": 1.0,
                    "crySampleRate": 30,
                    "cryEvents": [
                        { "parameter": "step", "normalizedTime": 0.1 },
                        { "parameter": "step", "normalizedTime": 0.2 },
                        { "parameter": "step", "normalizedTime": 0.3 },
                    ],
                },
            }],
        });

        let schedule = build_blend_schedule(&document, root).unwrap();
        let sounds = &schedule.clips[0].sounds;
        assert_eq!(sounds.len(), 3);
        // Round-robin over [10, 20]: adjacent overlapping events differ.
        assert!(sounds[0].wav.ends_with("10.wav"), "{:?}", sounds[0].wav);
        assert!(sounds[1].wav.ends_with("20.wav"), "{:?}", sounds[1].wav);
        assert!(sounds[2].wav.ends_with("10.wav"), "{:?}", sounds[2].wav);
        // The non-default sample never appears.
        assert!(sounds.iter().all(|sound| !sound.wav.ends_with("5.wav")));
    }

    #[test]
    fn blend_schedule_follows_default_surface_weighted_sequence() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        // Three decoded variations on disk.
        for id in [10u32, 20u32, 30u32] {
            let path = root.join(cry_audio::decoded_wave_catalog_path(
                cry_audio::WwiseMediaId(id),
            ));
            fs::create_dir_all(path.parent().unwrap()).unwrap();
            fs::write(&path, b"RIFF").unwrap();
        }

        // The default surface's weighted sequence [30, 10, 30, 20] is the engine
        // order; the blend must assign consecutive footsteps straight down it, not
        // by media-id order and not the non-default branch.
        let document = serde_json::json!({
            "extras": {
                "audioTriggers": [{
                    "trigger": "step",
                    "media": [
                        { "mediaId": 10, "defaultBranch": true },
                        { "mediaId": 20, "defaultBranch": true },
                        { "mediaId": 30, "defaultBranch": true },
                    ],
                    "surfaceMedia": [
                        { "switchId": 7, "default": true, "media": [10, 20, 30],
                          "sequence": [30, 10, 30, 20] },
                        { "switchId": 9, "media": [30] },
                    ],
                }],
            },
            "animations": [{
                "name": "walk",
                "extras": {
                    "cryDuration": 1.0,
                    "crySampleRate": 30,
                    "cryEvents": [
                        { "parameter": "step", "normalizedTime": 0.1 },
                        { "parameter": "step", "normalizedTime": 0.2 },
                        { "parameter": "step", "normalizedTime": 0.3 },
                        { "parameter": "step", "normalizedTime": 0.4 },
                        { "parameter": "step", "normalizedTime": 0.5 },
                    ],
                },
            }],
        });

        let schedule = build_blend_schedule(&document, root).unwrap();
        let sounds = &schedule.clips[0].sounds;
        assert_eq!(sounds.len(), 5);
        // Follows the weighted sequence, cycling once past its length of 4.
        let expected = ["30.wav", "10.wav", "30.wav", "20.wav", "30.wav"];
        for (sound, want) in sounds.iter().zip(expected) {
            assert!(sound.wav.ends_with(want), "want {want}, got {}", sound.wav);
        }
    }

    #[test]
    fn blend_schedule_places_mannequin_audio_strips_at_start_time_frames() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        for id in [10u32, 20u32] {
            let path = root.join(cry_audio::decoded_wave_catalog_path(
                cry_audio::WwiseMediaId(id),
            ));
            fs::create_dir_all(path.parent().unwrap()).unwrap();
            fs::write(&path, b"RIFF").unwrap();
        }
        // A Mannequin audio clip fires an ATL trigger at an absolute startTime
        // (seconds), unlike a normalized-time animevent.
        let document = serde_json::json!({
            "extras": {
                "audioTriggers": [{
                    "trigger": "play_vox_alligator_hurt",
                    "media": [
                        { "mediaId": 10, "defaultBranch": true },
                        { "mediaId": 20, "defaultBranch": true },
                    ],
                }],
            },
            "animations": [{
                "name": "alligator_r_r0_b",
                "extras": {
                    "cryDuration": 2.0,
                    "crySampleRate": 30,
                    "cryMannequinAudio": [
                        { "trigger": "play_vox_alligator_hurt", "joint": "bind_mouth_web",
                          "startTime": 0.5, "fragment": "r_R0_B" },
                        { "trigger": "play_vox_alligator_hurt", "joint": "bind_mouth_web",
                          "startTime": 0.0, "fragment": "r_R0_B" },
                    ],
                },
            }],
        });
        let schedule = build_blend_schedule(&document, root).unwrap();
        let sounds = &schedule.clips[0].sounds;
        assert_eq!(sounds.len(), 2);
        // frame = 1 + round(startTime * fps); fps = 30.
        assert_eq!(sounds[0].frame, 1 + 15);
        assert_eq!(sounds[1].frame, 1);
        // Weighted-sequence rotation applies across the two placements.
        assert!(sounds[0].wav.ends_with("10.wav"), "{:?}", sounds[0].wav);
        assert!(sounds[1].wav.ends_with("20.wav"), "{:?}", sounds[1].wav);
    }

    #[test]
    fn blend_schedule_places_resolved_bite_character_event_strip() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        let path = root.join(cry_audio::decoded_wave_catalog_path(
            cry_audio::WwiseMediaId(42),
        ));
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(&path, b"RIFF").unwrap();
        // A `CharacterEvent` bite clip after resolution: `trigger` is the resolved
        // Wwise event (the audioTriggers key), `characterEvent` keeps the short
        // name. It must place a strip at the authored ExitTime (0.6s).
        let document = serde_json::json!({
            "extras": {
                "audioTriggers": [{
                    "trigger": "Play_SFX_Alligator_Bite",
                    "media": [{ "mediaId": 42, "defaultBranch": true }],
                }],
            },
            "animations": [{
                "name": "alligator_bite",
                "extras": {
                    "cryDuration": 2.0,
                    "crySampleRate": 30,
                    "cryMannequinAudio": [
                        { "trigger": "Play_SFX_Alligator_Bite", "characterEvent": "Bite",
                          "joint": "", "startTime": 0.6, "fragment": "Attack_Bite" },
                    ],
                },
            }],
        });
        let schedule = build_blend_schedule(&document, root).unwrap();
        let sounds = &schedule.clips[0].sounds;
        assert_eq!(sounds.len(), 1, "alligator_bite finally gets a bite strip");
        // frame = 1 + round(0.6 * 30) = 19.
        assert_eq!(sounds[0].frame, 1 + 18);
        assert!(sounds[0].wav.ends_with("42.wav"), "{:?}", sounds[0].wav);
    }

    #[test]
    fn blend_schedule_falls_back_to_lowest_when_no_default_branch() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        for id in [7u32, 3u32] {
            let path = root.join(cry_audio::decoded_wave_catalog_path(
                cry_audio::WwiseMediaId(id),
            ));
            fs::create_dir_all(path.parent().unwrap()).unwrap();
            fs::write(&path, b"RIFF").unwrap();
        }
        let document = serde_json::json!({
            "extras": {
                "audioTriggers": [{
                    "trigger": "step",
                    "media": [{ "mediaId": 7 }, { "mediaId": 3 }],
                }],
            },
            "animations": [{
                "name": "walk",
                "extras": {
                    "cryDuration": 1.0,
                    "crySampleRate": 30,
                    "cryEvents": [
                        { "parameter": "step", "normalizedTime": 0.1 },
                        { "parameter": "step", "normalizedTime": 0.2 },
                    ],
                },
            }],
        });
        let schedule = build_blend_schedule(&document, root).unwrap();
        let sounds = &schedule.clips[0].sounds;
        // No default branch → the single lowest media id repeats (old behavior).
        assert_eq!(sounds.len(), 2);
        assert!(sounds.iter().all(|sound| sound.wav.ends_with("3.wav")));
    }
}
