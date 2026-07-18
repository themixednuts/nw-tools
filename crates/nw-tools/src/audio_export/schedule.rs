//! Deterministic Blender scene, strip, and CharacterEvent scheduling.

use std::collections::{BTreeSet, HashMap, HashSet};
use std::path::{Path, PathBuf};

use anyhow::Result;

#[derive(Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct BlendSchedule {
    /// One fps for the whole file. The glTF importer converts each animation's
    /// second-based keyframe times to frames at the *scene* fps in effect when
    /// it runs, so the adapter must set this on the base scene BEFORE import and
    /// on every clip scene — otherwise the imported action (placed at Blender's
    /// factory-default fps) ends before the scene's `frame_end` and the last
    /// stretch of every clip plays with no motion.
    pub(super) fps: u32,
    pub(super) clips: Vec<BlendClip>,
}

/// One per-clip Blender scene, fully placed: the adapter executes this plan
/// verbatim (no Blender-side arithmetic).
#[derive(Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct BlendClip {
    /// Blender action imported from the glTF animation.
    action: String,
    /// Scene name. Multi-tag CharacterEvent alternatives receive a suffix while
    /// still using the same imported action.
    name: String,
    /// Scene frame range is `1..=frame_end`, computed at the schedule fps so it
    /// coincides with the imported action's last keyframe.
    frame_end: u32,
    sounds: Vec<BlendSound>,
    /// Lossless receiver callback metadata, including conditional breathing and
    /// ordered non-audio side effects. The Blender adapter stores this on the
    /// scene and never turns conditional controls into preview strips.
    dispatches: Vec<serde_json::Value>,
}

#[derive(Debug, Clone, serde::Serialize)]
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
    /// Exclusive frame at which a Disable callback trims this active strip.
    #[serde(skip_serializing_if = "Option::is_none")]
    end_frame: Option<u32>,
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
fn wav_for_media_id(
    media_id: u64,
    media: &[serde_json::Value],
    package_root: &Path,
) -> Option<PathBuf> {
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

pub(super) fn build_blend_schedule(
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
    // Per-trigger cursor for authored animevents and direct Mannequin Audio clips.
    // CharacterEvent alternatives use scene-local cursors so one variant cannot
    // perturb another variant's deterministic preview.
    let mut rotation: HashMap<String, usize> = HashMap::new();
    for animation in animations {
        let action = animation
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
        let mut base_sounds = Vec::new();
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
            base_sounds.push(BlendSound {
                name: format!("{action}.{parameter}.{frame}"),
                frame,
                channel,
                wav: wav.to_string_lossy().replace('\\', "/"),
                end_frame: None,
            });
            channel = 1 + (channel % 8);
        }

        let mannequin = anim_extras
            .get("cryMannequinAudio")
            .and_then(|value| value.as_array())
            .cloned()
            .unwrap_or_default();
        for clip in &mannequin {
            // Receiver-owned CharacterEvents are represented only by their phase
            // records. Their scalar authored event name is never previewed as a
            // direct ATL trigger.
            if clip.get("characterEvent").is_some() {
                continue;
            }
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
            let frame = absolute_frame(start_time, fps);
            base_sounds.push(BlendSound {
                name: format!("{action}.{trigger}.{frame}"),
                frame,
                channel,
                wav: wav.to_string_lossy().replace('\\', "/"),
                end_frame: None,
            });
            channel = 1 + (channel % 8);
        }

        let mut dispatches = mannequin
            .iter()
            .flat_map(|clip| {
                clip.get("dispatches")
                    .and_then(serde_json::Value::as_array)
                    .into_iter()
                    .flatten()
                    .cloned()
            })
            .collect::<Vec<_>>();
        dispatches.sort_by(dispatch_value_order);
        let variants = dispatches
            .iter()
            .filter_map(AudioVariantKey::from_dispatch)
            .collect::<BTreeSet<_>>();
        let scene_variants = if variants.len() <= 1 {
            vec![None]
        } else {
            variants.into_iter().map(Some).collect()
        };
        let duplicate_names = duplicate_variant_names(&scene_variants);

        for variant in scene_variants {
            let selected = dispatches
                .iter()
                .filter(|dispatch| {
                    variant.as_ref().is_none_or(|variant| {
                        AudioVariantKey::from_dispatch(dispatch).as_ref() == Some(variant)
                    })
                })
                .cloned()
                .collect::<Vec<_>>();
            let mut sounds = base_sounds.clone();
            let mut variant_channel = channel;
            schedule_character_dispatches(
                &action,
                fps,
                &selected,
                &trigger_variations,
                &mut sounds,
                &mut variant_channel,
            );
            clips.push(BlendClip {
                action: action.clone(),
                name: variant_scene_name(&action, variant.as_ref(), &duplicate_names),
                frame_end,
                sounds,
                dispatches: selected,
            });
        }
    }
    Ok(BlendSchedule { fps, clips })
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
struct AudioVariantKey {
    scene_path: String,
    entity_id: u64,
    valid_tag: Option<String>,
    valid_tag_crc: Option<u32>,
}

impl AudioVariantKey {
    fn from_dispatch(dispatch: &serde_json::Value) -> Option<Self> {
        Some(Self {
            scene_path: dispatch
                .get("scenePath")?
                .as_str()?
                .replace('\\', "/")
                .to_ascii_lowercase(),
            entity_id: dispatch.get("entityId")?.as_u64()?,
            valid_tag: dispatch
                .get("validTag")
                .and_then(serde_json::Value::as_str)
                .map(str::to_owned),
            valid_tag_crc: dispatch
                .get("validTagCrc")
                .and_then(serde_json::Value::as_u64)
                .and_then(|value| u32::try_from(value).ok()),
        })
    }

    fn label(&self) -> String {
        self.valid_tag
            .clone()
            .unwrap_or_else(|| format!("entity-{}", self.entity_id))
    }
}

fn duplicate_variant_names(variants: &[Option<AudioVariantKey>]) -> HashSet<String> {
    let mut counts = HashMap::<String, usize>::new();
    for variant in variants.iter().flatten() {
        *counts.entry(variant.label()).or_default() += 1;
    }
    counts
        .into_iter()
        .filter_map(|(label, count)| (count > 1).then_some(label))
        .collect()
}

fn variant_scene_name(
    action: &str,
    variant: Option<&AudioVariantKey>,
    duplicate_names: &HashSet<String>,
) -> String {
    let Some(variant) = variant else {
        return action.to_owned();
    };
    let label = variant.label();
    if duplicate_names.contains(&label) {
        format!("{action} [audio:{label} @{}]", variant.entity_id)
    } else {
        format!("{action} [audio:{label}]")
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct DispatchIdentity {
    scene_path: String,
    entity_id: u64,
    valid_tag: Option<String>,
    receiver: String,
    receiver_script_path: String,
    fragment: String,
    proc_layer_ordinal: u64,
    procedural_ordinal: u64,
    character_event: String,
    joint: String,
    producer: String,
}

impl DispatchIdentity {
    fn from_dispatch(dispatch: &serde_json::Value) -> Option<Self> {
        Some(Self {
            scene_path: dispatch.get("scenePath")?.as_str()?.to_owned(),
            entity_id: dispatch.get("entityId")?.as_u64()?,
            valid_tag: dispatch
                .get("validTag")
                .and_then(serde_json::Value::as_str)
                .map(str::to_owned),
            receiver: dispatch.get("receiver")?.as_str()?.to_owned(),
            receiver_script_path: dispatch.get("receiverScriptPath")?.as_str()?.to_owned(),
            fragment: dispatch.get("fragment")?.as_str()?.to_owned(),
            proc_layer_ordinal: dispatch.get("procLayerOrdinal")?.as_u64()?,
            procedural_ordinal: dispatch.get("proceduralOrdinal")?.as_u64()?,
            character_event: dispatch.get("characterEvent")?.as_str()?.to_owned(),
            joint: dispatch
                .get("joint")
                .and_then(serde_json::Value::as_str)
                .unwrap_or_default()
                .to_owned(),
            producer: dispatch.get("producer")?.as_str()?.to_owned(),
        })
    }
}

fn schedule_character_dispatches(
    action: &str,
    fps: u32,
    dispatches: &[serde_json::Value],
    trigger_variations: &HashMap<String, Vec<PathBuf>>,
    sounds: &mut Vec<BlendSound>,
    channel: &mut u32,
) {
    let mut active = HashMap::<DispatchIdentity, Vec<usize>>::new();
    let mut rotation = HashMap::<String, usize>::new();
    for dispatch in dispatches {
        let time = dispatch
            .get("time")
            .and_then(serde_json::Value::as_f64)
            .unwrap_or(0.0)
            .max(0.0);
        let frame = absolute_frame(time, fps);
        let Some(identity) = DispatchIdentity::from_dispatch(dispatch) else {
            continue;
        };
        let enabled = dispatch
            .get("enabled")
            .and_then(serde_json::Value::as_bool)
            .unwrap_or(false);
        if !enabled {
            if let Some(indices) = active.remove(&identity) {
                for index in indices {
                    let sound = &mut sounds[index];
                    let end = frame.max(sound.frame.saturating_add(1));
                    sound.end_frame = Some(sound.end_frame.map_or(end, |old| old.min(end)));
                }
            }
            continue;
        }

        for operation in dispatch
            .get("operations")
            .and_then(serde_json::Value::as_array)
            .into_iter()
            .flatten()
        {
            if operation.get("kind").and_then(serde_json::Value::as_str) != Some("audioControl")
                || operation
                    .get("condition")
                    .is_some_and(|condition| !condition.is_null())
            {
                continue;
            }
            let control = operation
                .get("control")
                .and_then(serde_json::Value::as_str)
                .unwrap_or_default();
            let Some(variations) = trigger_variations.get(control) else {
                continue;
            };
            let cursor = rotation.entry(control.to_owned()).or_default();
            let wav = &variations[*cursor % variations.len()];
            *cursor += 1;
            let index = sounds.len();
            sounds.push(BlendSound {
                name: format!("{action}.{control}.{frame}.{index}"),
                frame,
                channel: *channel,
                wav: wav.to_string_lossy().replace('\\', "/"),
                end_frame: None,
            });
            *channel = 1 + (*channel % 8);
            active.entry(identity.clone()).or_default().push(index);
        }
    }
}

fn absolute_frame(time: f64, fps: u32) -> u32 {
    1 + (time.max(0.0) * f64::from(fps)).round() as u32
}

fn dispatch_value_order(left: &serde_json::Value, right: &serde_json::Value) -> std::cmp::Ordering {
    let number = |value: &serde_json::Value, field: &str| {
        value
            .get(field)
            .and_then(serde_json::Value::as_f64)
            .unwrap_or(0.0)
    };
    let ordinal = |value: &serde_json::Value, field: &str| {
        value
            .get(field)
            .and_then(serde_json::Value::as_u64)
            .unwrap_or(0)
    };
    number(left, "time")
        .total_cmp(&number(right, "time"))
        .then_with(|| {
            phase_value_order(json_text(left, "phase"))
                .cmp(&phase_value_order(json_text(right, "phase")))
        })
        .then_with(|| ordinal(left, "procLayerOrdinal").cmp(&ordinal(right, "procLayerOrdinal")))
        .then_with(|| ordinal(left, "proceduralOrdinal").cmp(&ordinal(right, "proceduralOrdinal")))
        .then_with(|| json_text(left, "scenePath").cmp(json_text(right, "scenePath")))
        .then_with(|| ordinal(left, "entityId").cmp(&ordinal(right, "entityId")))
        .then_with(|| {
            json_text(left, "validTag")
                .as_bytes()
                .cmp(json_text(right, "validTag").as_bytes())
        })
        .then_with(|| json_text(left, "receiver").cmp(json_text(right, "receiver")))
        .then_with(|| {
            json_text(left, "receiverScriptPath").cmp(json_text(right, "receiverScriptPath"))
        })
}

fn json_text<'a>(value: &'a serde_json::Value, field: &str) -> &'a str {
    value
        .get(field)
        .and_then(serde_json::Value::as_str)
        .unwrap_or_default()
}

fn phase_value_order(phase: &str) -> u8 {
    if phase == "exit" { 0 } else { 1 }
}

#[cfg(test)]
#[path = "schedule_tests.rs"]
mod tests;
