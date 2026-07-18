//! Entity-scoped Mannequin receiver discovery and fragment-audio attachment.
//!
//! CharacterEvent association is defined by one serialized `AZ::Entity`: its
//! direct ActionListComponent, ordinary TagComponent, and ScriptComponent values
//! must co-own the data. Scene-wide string proximity is not an association.

use std::collections::BTreeMap;

use nw_objectstream::asset_reference::read_asset_value;
use nw_objectstream::query::{az_entity_elements, base_class_of_type};
use nw_objectstream::value::{
    child_by_field, read_bool, read_entity_id, read_string, read_string_vector_owned,
};
use uuid::{Uuid, uuid};

use super::*;

const ACTION_LIST_COMPONENT_ID: Uuid = uuid!("30ed0ace-51dd-48b9-ba41-2fa6775cd106");
const SCRIPT_COMPONENT_ID: Uuid = uuid!("8d1bc97e-c55d-4d34-a460-e63c57cd0d4b");
const SCRIPT_PROPERTY_ID: Uuid = uuid!("d227d737-f1ed-4fb3-a1fb-38e4985d2e7a");

/// Discover entity-owned Mannequin references, preserve direct Audio clips once,
/// and attach CharacterEvent clips separately for every valid receiver context.
pub(super) fn attach_fragment_audio(
    source: &dyn AssetSource,
    scene_paths: &[String],
    resolved: &mut ResolvedAsset,
) -> Result<()> {
    let discovery = discover_mannequin_entities(source, scene_paths)?;
    if discovery.adb_paths.is_empty() {
        return Ok(());
    }
    for path in discovery
        .adb_paths
        .iter()
        .chain(&discovery.controller_paths)
    {
        add_mannequin_source(source, path, &mut resolved.extras)?;
    }

    let merged = merge_databases(source, &discovery.adb_paths, &mut resolved.extras)?;
    let mut animation_audio = build_animation_audio(&merged, None, ClipSelection::DirectAudio);
    for context in discovery.contexts {
        let merged = merge_databases(source, &context.adb_paths, &mut resolved.extras)?;
        animation_audio.extend(build_animation_audio(
            &merged,
            Some(&context),
            ClipSelection::CharacterEvent,
        ));
    }
    animation_audio.sort_by_key(animation_audio_key);
    resolved.extras.mannequin_audio = animation_audio;
    Ok(())
}

struct MannequinDiscovery {
    adb_paths: Vec<String>,
    controller_paths: Vec<String>,
    contexts: Vec<nw_model::CryMannequinReceiverContext>,
}

fn discover_mannequin_entities(
    source: &dyn AssetSource,
    scene_paths: &[String],
) -> Result<MannequinDiscovery> {
    let mut scenes = scene_paths
        .iter()
        .filter(|path| is_legacy_scene_asset(path))
        .map(|path| normalize_path(path))
        .collect::<Vec<_>>();
    scenes.sort_by_key(|path| path.to_ascii_lowercase());
    scenes.dedup_by(|later, earlier| later.eq_ignore_ascii_case(earlier));

    let mut discovery = MannequinDiscovery {
        adb_paths: Vec::new(),
        controller_paths: Vec::new(),
        contexts: Vec::new(),
    };
    for scene_path in scenes {
        let Some(bytes) = source.read(&scene_path) else {
            continue;
        };
        let Ok(stream) =
            nw_objectstream::ObjectStream::from_bytes(&bytes, Some(&OBJECTSTREAM_LOOKUP))
        else {
            continue;
        };
        for entity in az_entity_elements(stream.elements()) {
            let Some(components) = child_by_field(entity, "Components") else {
                continue;
            };
            let (adb_paths, controller_paths) = action_list_paths(source, components)
                .context("read ActionListComponent references")?;
            if adb_paths.is_empty() {
                continue;
            }
            for path in &adb_paths {
                push_path(&mut discovery.adb_paths, path.clone());
            }
            for path in &controller_paths {
                push_path(&mut discovery.controller_paths, path.clone());
            }

            let Some(tags) = nw_objectstream::tag_component::read_entity_tag_component(entity)
                .with_context(|| format!("read ordinary TagComponent in {scene_path}"))?
            else {
                continue;
            };
            let receivers = receiver_scripts(source, components).with_context(|| {
                format!(
                    "read ScriptComponent receivers on entity {}",
                    tags.entity_id
                )
            })?;
            if receivers.is_empty() {
                continue;
            }
            let entity_name = child_by_field(entity, "Name")
                .map(read_string)
                .transpose()
                .with_context(|| format!("read Name on entity {}", tags.entity_id))?
                .unwrap_or_default()
                .trim()
                .to_owned();
            discovery
                .contexts
                .push(nw_model::CryMannequinReceiverContext {
                    scene_path: scene_path.clone(),
                    entity_id: tags.entity_id,
                    entity_name,
                    tag_crcs: tags.tags,
                    adb_paths,
                    controller_paths,
                    receivers,
                });
        }
    }
    discovery
        .adb_paths
        .sort_by_key(|path| path.to_ascii_lowercase());
    discovery
        .controller_paths
        .sort_by_key(|path| path.to_ascii_lowercase());
    discovery.contexts.sort_by_key(context_key);
    Ok(discovery)
}

fn context_key(context: &nw_model::CryMannequinReceiverContext) -> (String, u64, Vec<u32>) {
    (
        context.scene_path.to_ascii_lowercase(),
        context.entity_id,
        context.tag_crcs.clone(),
    )
}

fn animation_audio_key(
    entry: &nw_model::CryMannequinAnimationAudio,
) -> (String, Option<(String, u64, Vec<u32>)>) {
    (
        entry.animation.to_ascii_lowercase(),
        entry
            .clips
            .first()
            .and_then(|clip| clip.context.as_ref())
            .map(context_key),
    )
}

fn merge_databases(
    source: &dyn AssetSource,
    paths: &[String],
    extras: &mut nw_model::CryAssetExtras,
) -> Result<BTreeMap<String, MergedFragment>> {
    let mut merged = BTreeMap::new();
    for path in paths {
        let bytes = read_required(source, path)?;
        let database = cry_mannequin::MannequinAnimationDatabaseSource::from_legacy(path, &bytes)
            .with_context(|| format!("parse animation database {path}"))?
            .database;
        ship_fragment_tag_definitions(source, &database, extras)?;
        for fragment in database.fragment_audio() {
            merge_fragment(&mut merged, fragment);
        }
    }
    Ok(merged)
}

fn action_list_paths(
    source: &dyn AssetSource,
    components: &nw_objectstream::Element,
) -> Result<(Vec<String>, Vec<String>)> {
    let mut adbs = Vec::new();
    let mut controllers = Vec::new();
    for component in components
        .children()
        .iter()
        .filter(|component| component.id() == &ACTION_LIST_COMPONENT_ID)
    {
        for element in component.iter_recursive() {
            let Some(field) = element.field().map(|field| field.as_str()) else {
                continue;
            };
            let target = match field {
                field
                    if field.eq_ignore_ascii_case("m_animationDatabase")
                        || field.eq_ignore_ascii_case("ScopeADB") =>
                {
                    &mut adbs
                }
                field if field.eq_ignore_ascii_case("m_controllerDefinition") => &mut controllers,
                _ => continue,
            };
            let path = nw_objectstream::asset_reference::read_asset_path_or_string_owned(element)
                .with_context(|| format!("read authored Mannequin reference {field}"))?;
            let Some(path) = path.map(|path| normalize_path(&path)) else {
                continue;
            };
            if !is_mannequin_reference(&path) || source.read(&path).is_none() {
                continue;
            }
            push_path(target, path);
        }
    }
    adbs.sort_by_key(|path| path.to_ascii_lowercase());
    controllers.sort_by_key(|path| path.to_ascii_lowercase());
    Ok((adbs, controllers))
}

fn receiver_scripts(
    source: &dyn AssetSource,
    components: &nw_objectstream::Element,
) -> Result<Vec<nw_model::CryCharacterEventReceiver>> {
    let mut receivers = Vec::new();
    for component in components
        .children()
        .iter()
        .filter(|component| component.id() == &SCRIPT_COMPONENT_ID)
    {
        let Some(script) = child_by_field(component, "Script") else {
            continue;
        };
        let asset = read_asset_value(script).context("decode ScriptComponent Script asset")?;
        let asset_id = nw_asset::AssetId::new(asset.guid(), asset.sub_id());
        let script_path = source
            .path_by_id(asset_id)
            .map(|path| normalize_path(&path))
            .or_else(|| {
                let hint = normalize_path(asset.hint().trim());
                (source.allows_asset_hint_fallback() && !hint.is_empty() && source.contains(&hint))
                    .then_some(hint)
            });
        let Some(script_path) = script_path else {
            continue;
        };
        let Some(kind) = receiver_script_kind(&script_path) else {
            continue;
        };
        let properties = child_by_field(component, "Properties");
        let receiver = match kind {
            ReceiverScriptKind::CommonNpc => {
                nw_model::CryCharacterEventReceiver::CommonNpcAudio { script_path }
            }
            ReceiverScriptKind::Bone => {
                let (bindings, spawn_sound) = bone_audio_properties(properties)?;
                nw_model::CryCharacterEventReceiver::BoneAudio {
                    script_path,
                    bindings,
                    spawn_sound,
                }
            }
            ReceiverScriptKind::SubtitleNpc => {
                nw_model::CryCharacterEventReceiver::SubtitleNpcAudio { script_path }
            }
            ReceiverScriptKind::Mount => {
                nw_model::CryCharacterEventReceiver::MountAudio { script_path }
            }
        };
        receivers.push(receiver);
    }
    receivers.sort_by_key(receiver_key);
    Ok(receivers)
}

#[derive(Debug, Clone, Copy)]
enum ReceiverScriptKind {
    CommonNpc,
    Bone,
    SubtitleNpc,
    Mount,
}

fn receiver_script_kind(path: &str) -> Option<ReceiverScriptKind> {
    let file = path.rsplit('/').next().unwrap_or(path);
    let stem = file.rsplit_once('.').map_or(file, |(stem, _)| stem);
    match stem.to_ascii_lowercase().as_str() {
        "commonnpc_audio" => Some(ReceiverScriptKind::CommonNpc),
        "boneaudio" => Some(ReceiverScriptKind::Bone),
        "subtitlenpc_audio" => Some(ReceiverScriptKind::SubtitleNpc),
        "mountaudio" => Some(ReceiverScriptKind::Mount),
        _ => None,
    }
}

fn receiver_key(receiver: &nw_model::CryCharacterEventReceiver) -> (u8, String) {
    match receiver {
        nw_model::CryCharacterEventReceiver::CommonNpcAudio { script_path } => {
            (0, script_path.to_ascii_lowercase())
        }
        nw_model::CryCharacterEventReceiver::BoneAudio { script_path, .. } => {
            (1, script_path.to_ascii_lowercase())
        }
        nw_model::CryCharacterEventReceiver::SubtitleNpcAudio { script_path } => {
            (2, script_path.to_ascii_lowercase())
        }
        nw_model::CryCharacterEventReceiver::MountAudio { script_path } => {
            (3, script_path.to_ascii_lowercase())
        }
    }
}

fn bone_audio_properties(
    properties: Option<&nw_objectstream::Element>,
) -> Result<(Vec<nw_model::CryBoneAudioBinding>, bool)> {
    let Some(properties) = properties else {
        return Ok((Vec::new(), false));
    };
    let events = script_string_array(properties, "characterEventName")?;
    let controls = script_string_array(properties, "wwiseEvent")?;
    let entities = script_entity_array(properties, "audioEntity")?;
    let spawn_sound = script_bool(properties, "spawnSound")?.unwrap_or(false);
    let bindings = events
        .into_iter()
        .zip(entities)
        .zip(controls)
        .filter_map(|((character_event, audio_entity), wwise_event)| {
            let character_event = character_event.trim().to_owned();
            let wwise_event = wwise_event.trim().to_owned();
            (!character_event.is_empty() && !wwise_event.is_empty()).then_some(
                nw_model::CryBoneAudioBinding {
                    character_event,
                    audio_entity,
                    wwise_event,
                },
            )
        })
        .collect();
    Ok((bindings, spawn_sound))
}

fn script_string_array(properties: &nw_objectstream::Element, name: &str) -> Result<Vec<String>> {
    let Some(property) = script_property(properties, name)? else {
        return Ok(Vec::new());
    };
    let Some(values) = child_by_field(property, "values") else {
        return Ok(Vec::new());
    };
    read_string_vector_owned(values).with_context(|| format!("read script property {name}"))
}

fn script_entity_array(properties: &nw_objectstream::Element, name: &str) -> Result<Vec<u64>> {
    let Some(property) = script_property(properties, name)? else {
        return Ok(Vec::new());
    };
    let Some(values) = child_by_field(property, "values") else {
        return Ok(Vec::new());
    };
    values
        .iter_recursive()
        .filter(|element| element.id() == &nw_objectstream::types::ENTITY_ID)
        .map(read_entity_id)
        .collect::<Result<Vec<_>, _>>()
        .with_context(|| format!("read script property {name}"))
}

fn script_bool(properties: &nw_objectstream::Element, name: &str) -> Result<Option<bool>> {
    let Some(property) = script_property(properties, name)? else {
        return Ok(None);
    };
    child_by_field(property, "value")
        .map(read_bool)
        .transpose()
        .with_context(|| format!("read script property {name}"))
}

fn script_property<'a>(
    properties: &'a nw_objectstream::Element,
    wanted: &str,
) -> Result<Option<&'a nw_objectstream::Element>> {
    for property in properties.iter_recursive() {
        let Some(base) = base_class_of_type(property, SCRIPT_PROPERTY_ID) else {
            continue;
        };
        let Some(name) = child_by_field(base, "name") else {
            continue;
        };
        if read_string(name)
            .context("read ScriptProperty name")?
            .eq_ignore_ascii_case(wanted)
        {
            return Ok(Some(property));
        }
    }
    Ok(None)
}

fn push_path(paths: &mut Vec<String>, path: String) {
    if !paths
        .iter()
        .any(|existing| existing.eq_ignore_ascii_case(&path))
    {
        paths.push(path);
    }
}

struct MergedFragment {
    fragment: String,
    tags: String,
    animations: Vec<String>,
    clips: Vec<cry_mannequin::MannequinAudioClip>,
}

fn merge_fragment(
    merged: &mut BTreeMap<String, MergedFragment>,
    fragment: cry_mannequin::MannequinFragmentAudio,
) {
    let entry = merged
        .entry(fragment.fragment.to_ascii_lowercase())
        .or_insert_with(|| MergedFragment {
            fragment: fragment.fragment.clone(),
            tags: String::new(),
            animations: Vec::new(),
            clips: Vec::new(),
        });
    if entry.tags.is_empty() && !fragment.tags.is_empty() {
        entry.tags = fragment.tags;
    }
    for animation in fragment.animations {
        if !entry
            .animations
            .iter()
            .any(|existing| existing.eq_ignore_ascii_case(&animation))
        {
            entry.animations.push(animation);
        }
    }
    for clip in fragment.clips {
        if !entry
            .clips
            .iter()
            .any(|existing| clips_equal(existing, &clip))
        {
            entry.clips.push(clip);
        }
    }
}

fn clips_equal(
    left: &cry_mannequin::MannequinAudioClip,
    right: &cry_mannequin::MannequinAudioClip,
) -> bool {
    left.trigger.eq_ignore_ascii_case(&right.trigger)
        && left.stop_trigger == right.stop_trigger
        && left.joint.eq_ignore_ascii_case(&right.joint)
        && left.start_time == right.start_time
        && left.exit_time == right.exit_time
        && left.option_on_enter == right.option_on_enter
        && left.option_on_exit == right.option_on_exit
        && left.proc_layer_ordinal == right.proc_layer_ordinal
        && left.procedural_ordinal == right.procedural_ordinal
        && left.kind == right.kind
}

#[derive(Debug, Clone, Copy)]
enum ClipSelection {
    DirectAudio,
    CharacterEvent,
}

impl ClipSelection {
    fn matches(self, clip: &cry_mannequin::MannequinAudioClip) -> bool {
        matches!(
            (self, clip.kind),
            (
                Self::DirectAudio,
                cry_mannequin::MannequinAudioKind::AtlTrigger
            ) | (
                Self::CharacterEvent,
                cry_mannequin::MannequinAudioKind::CharacterEvent
            )
        )
    }
}

fn build_animation_audio(
    merged: &BTreeMap<String, MergedFragment>,
    context: Option<&nw_model::CryMannequinReceiverContext>,
    selection: ClipSelection,
) -> Vec<nw_model::CryMannequinAnimationAudio> {
    let mut by_animation = BTreeMap::<String, nw_model::CryMannequinAnimationAudio>::new();
    let mut orphaned = Vec::new();
    for fragment in merged.values() {
        let clips = fragment
            .clips
            .iter()
            .filter(|clip| selection.matches(clip))
            .collect::<Vec<_>>();
        if clips.is_empty() {
            continue;
        }
        if fragment.animations.is_empty() {
            orphaned.push(fragment.fragment.clone());
            continue;
        }
        for animation in &fragment.animations {
            let entry = by_animation
                .entry(animation.to_ascii_lowercase())
                .or_insert_with(|| nw_model::CryMannequinAnimationAudio {
                    animation: animation.clone(),
                    clips: Vec::new(),
                });
            entry.clips.extend(clips.iter().map(|clip| {
                let character_event =
                    matches!(clip.kind, cry_mannequin::MannequinAudioKind::CharacterEvent)
                        .then(|| clip.trigger.clone());
                nw_model::CryMannequinAudioClip {
                    trigger: clip.trigger.clone(),
                    stop_trigger: clip.stop_trigger.clone(),
                    character_event,
                    joint: clip.joint.clone(),
                    start_time: clip.start_time.unwrap_or(0.0),
                    exit_time: clip.exit_time,
                    option_on_enter: clip.option_on_enter.map(convert_option),
                    option_on_exit: clip.option_on_exit.map(convert_option),
                    proc_layer_ordinal: clip.proc_layer_ordinal,
                    procedural_ordinal: clip.procedural_ordinal,
                    producer: match clip.kind {
                        cry_mannequin::MannequinAudioKind::AtlTrigger => {
                            nw_model::CryMannequinAudioProducer::MannequinAudio
                        }
                        cry_mannequin::MannequinAudioKind::CharacterEvent => {
                            nw_model::CryMannequinAudioProducer::MannequinCharacterEvent
                        }
                    },
                    fragment: fragment.fragment.clone(),
                    tags: fragment.tags.clone(),
                    context: context.cloned(),
                    dispatches: Vec::new(),
                }
            }));
        }
    }
    if !orphaned.is_empty() {
        eprintln!(
            "note: {} Mannequin fragment(s) carry audio but no animation and were dropped: {}",
            orphaned.len(),
            orphaned.join(", ")
        );
    }
    by_animation.into_values().collect()
}

fn convert_option(
    option: cry_mannequin::MannequinCharacterEventOption,
) -> nw_model::CryCharacterEventOption {
    match option {
        cry_mannequin::MannequinCharacterEventOption::Enable => {
            nw_model::CryCharacterEventOption::Enable
        }
        cry_mannequin::MannequinCharacterEventOption::Disable => {
            nw_model::CryCharacterEventOption::Disable
        }
        cry_mannequin::MannequinCharacterEventOption::NoEffect => {
            nw_model::CryCharacterEventOption::NoEffect
        }
    }
}

fn ship_fragment_tag_definitions(
    source: &dyn AssetSource,
    database: &cry_mannequin::MannequinAnimationDatabase,
    extras: &mut nw_model::CryAssetExtras,
) -> Result<()> {
    for path in [
        database.fragment_definition.as_deref(),
        database.tag_definition.as_deref(),
    ]
    .into_iter()
    .flatten()
    .filter(|path| !path.trim().is_empty())
    {
        if cry_mannequin::MannequinXmlKind::from_source_path(path).is_some()
            && source.read(path).is_some()
        {
            add_mannequin_source(source, path, extras)?;
        }
    }
    Ok(())
}

fn is_mannequin_reference(path: &str) -> bool {
    cry_mannequin::is_animation_database_name(path)
        || matches!(
            cry_mannequin::MannequinXmlKind::from_source_path(path),
            Some(cry_mannequin::MannequinXmlKind::ControllerDefinition)
        )
}

fn is_legacy_scene_asset(path: &str) -> bool {
    matches!(
        source_extension(path).as_str(),
        "slice" | "dynamicslice" | "entity" | "entities" | "entities_xml" | "prefab"
    )
}

#[cfg(test)]
#[path = "mannequin_tests.rs"]
mod tests;
