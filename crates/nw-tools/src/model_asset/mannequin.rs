//! Catalog-driven Mannequin fragment-audio discovery and attachment.
//!
//! Creature vocal/action sounds (bite, hurt, tail whip, body falls) live in
//! Mannequin ADB `ProcLayer` `type="Audio"` clips, not CryAnimation animevents,
//! so the animevent audio pass alone misses them entirely.
//!
//! The character's slice (already scanned for hit volumes) names its animation
//! databases and controller definition in an ActionController-style component as
//! authored asset-path fields. Those paths are stored as raw strings the asset
//! graph does not follow, so we read them straight from the ObjectStream — no
//! filename stemming. Each discovered ADB is shipped with its FragDef/TagDef, its
//! fragments are parsed for audio clips, and each clip is attached to the glTF
//! animation matching the fragment's AnimLayer animation. The clip triggers then
//! resolve through the existing ATL → bank/media pipeline alongside footsteps.

use std::collections::BTreeMap;

use super::*;

/// Discover the character's Mannequin databases from its scene slices, ship them
/// with their FragDef/TagDef, and attach each fragment's audio clips to the glTF
/// animation its AnimLayer animation names.
///
/// `scene_paths` is the model's resolved dependency closure; only legacy
/// ObjectStream scenes (slices/prefabs) are scanned. A character with no authored
/// Mannequin linkage leaves `resolved` untouched.
pub(super) fn attach_fragment_audio(
    source: &dyn AssetSource,
    scene_paths: &[String],
    resolved: &mut ResolvedAsset,
) -> Result<()> {
    let database_paths = discover_mannequin_reference_paths(source, scene_paths)?;
    if database_paths.is_empty() {
        return Ok(());
    }

    // Ship every discovered Mannequin document (ADB + controllerdef) and, for each
    // animation database, its FragDef/TagDef — then merge fragments across all of
    // them by name so an audio-only fragment pairs with the animation database's
    // matching animation.
    let mut merged: BTreeMap<String, MergedFragment> = BTreeMap::new();
    for path in &database_paths {
        add_mannequin_source(source, path, &mut resolved.extras)?;
        if !cry_mannequin::is_animation_database_name(path) {
            continue;
        }
        let bytes = read_required(source, path)?;
        let database = cry_mannequin::MannequinAnimationDatabaseSource::from_legacy(path, &bytes)
            .with_context(|| format!("parse animation database {path}"))?
            .database;
        ship_fragment_tag_definitions(source, &database, &mut resolved.extras)?;
        for fragment in database.fragment_audio() {
            merge_fragment(&mut merged, fragment);
        }
    }

    // Ship the character's Wwise event-id catalog so `CharacterEvent` short names
    // (`Bite`) can be validated against real events during trigger resolution.
    // The events CSV is not in the model's dependency closure — its banks are only
    // pulled in later, during trigger resolution — so it is discovered here from
    // the audio ADB's character stem.
    ship_character_event_catalogs(source, &database_paths, &mut resolved.extras)?;

    resolved.extras.mannequin_audio = build_animation_audio(&merged);
    Ok(())
}

/// Known ADB role suffixes; stripping one from an ADB basename yields the
/// character's audio stem (`npc_alligator_audio` → `npc_alligator`).
const ADB_ROLE_SUFFIXES: &[&str] = &["_audio", "_anims", "_vfx", "_actions"];

/// Ship the character's `sounds/wwise/<char>_events.csv` event-id catalog, derived
/// from each discovered ADB's character stem and shipped only when the install
/// actually provides it (existence-validated, never a bare guess). Loaded as an
/// `AudioMapping` source so audio resolution can validate `CharacterEvent` short
/// names against it.
fn ship_character_event_catalogs(
    source: &dyn AssetSource,
    database_paths: &[String],
    extras: &mut nw_model::CryAssetExtras,
) -> Result<()> {
    let mut candidates: Vec<String> = Vec::new();
    for path in database_paths {
        if !cry_mannequin::is_animation_database_name(path) {
            continue;
        }
        let Some(stem) = adb_character_stem(path) else {
            continue;
        };
        let csv = format!("sounds/wwise/{stem}_events.csv");
        if cry_audio::is_audio_mapping_source(&csv)
            && !candidates.iter().any(|existing| existing.eq_ignore_ascii_case(&csv))
        {
            candidates.push(csv);
        }
    }
    for csv in candidates {
        if has_source_asset(extras, &csv) {
            continue;
        }
        if source.read(&csv).is_none() {
            continue;
        }
        add_audio_source(source, &csv, extras)?;
    }
    Ok(())
}

/// The character audio stem of an ADB path: its basename minus extension and a
/// trailing role suffix (`.../npc_alligator_audio.adb` → `npc_alligator`).
fn adb_character_stem(path: &str) -> Option<String> {
    let base = normalize_path(path);
    let base = base.rsplit('/').next().unwrap_or(&base);
    let stem = base.rsplit_once('.').map_or(base, |(stem, _)| stem);
    if stem.is_empty() {
        return None;
    }
    let lower = stem.to_ascii_lowercase();
    for suffix in ADB_ROLE_SUFFIXES {
        if let Some(trimmed) = lower.strip_suffix(suffix)
            && !trimmed.is_empty()
        {
            return Some(trimmed.to_owned());
        }
    }
    Some(lower)
}

/// A fragment gathered across every discovered database, keyed by lowercased name.
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
        // Dedup only truly identical clips (same trigger/short name, joint, kind,
        // and fire time) across databases — a fragment can legitimately fire the
        // same event twice at different clip-relative times, so `start_time` is
        // part of the identity.
        if !entry.clips.iter().any(|existing| {
            existing.trigger.eq_ignore_ascii_case(&clip.trigger)
                && existing.joint.eq_ignore_ascii_case(&clip.joint)
                && existing.kind == clip.kind
                && existing.start_time == clip.start_time
        }) {
            entry.clips.push(clip);
        }
    }
}

/// Distribute each merged fragment's audio clips onto the animations it plays,
/// keyed by animation name. Fragments whose audio never pairs with an animation
/// are reported and dropped.
fn build_animation_audio(
    merged: &BTreeMap<String, MergedFragment>,
) -> Vec<nw_model::CryMannequinAnimationAudio> {
    let mut by_animation: BTreeMap<String, nw_model::CryMannequinAnimationAudio> = BTreeMap::new();
    let mut orphaned = Vec::new();
    for fragment in merged.values() {
        if fragment.clips.is_empty() {
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
            for clip in &fragment.clips {
                // A `CharacterEvent` clip's `trigger` still holds the short
                // `CharacterEventName`; audio resolution (in `audio.rs`) expands
                // it to a Wwise event or drops the clip. Preserve the short name
                // as `character_event` for provenance.
                let character_event = matches!(
                    clip.kind,
                    cry_mannequin::MannequinAudioKind::CharacterEvent
                )
                .then(|| clip.trigger.clone());
                entry.clips.push(nw_model::CryMannequinAudioClip {
                    trigger: clip.trigger.clone(),
                    stop_trigger: clip.stop_trigger.clone(),
                    character_event,
                    joint: clip.joint.clone(),
                    start_time: clip.start_time.unwrap_or(0.0),
                    fragment: fragment.fragment.clone(),
                    tags: fragment.tags.clone(),
                });
            }
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

/// Ship the animation database's FragDef and TagDef (the authored `Actions`/`Tags`
/// XML the ADB header names) as embedded Mannequin sources.
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

/// Scan the model's scene slices for authored Mannequin database and controller
/// references. A path counts only when the ObjectStream stores it as a string
/// value that also resolves in the catalog — the authored link, never a guessed
/// `npc_<name>_anims.adb` stem.
fn discover_mannequin_reference_paths(
    source: &dyn AssetSource,
    scene_paths: &[String],
) -> Result<Vec<String>> {
    let mut paths: Vec<String> = Vec::new();
    for scene in scene_paths.iter().filter(|path| is_legacy_scene_asset(path)) {
        let Some(bytes) = source.read(scene) else {
            continue;
        };
        let Ok(stream) =
            nw_objectstream::ObjectStream::from_bytes(&bytes, Some(&OBJECTSTREAM_LOOKUP))
        else {
            continue;
        };
        let mut strings = Vec::new();
        for element in stream.elements() {
            collect_string_leaves(element, &mut strings);
        }
        for value in strings {
            let candidate = normalize_path(&value);
            if !is_mannequin_reference(&candidate) {
                continue;
            }
            if source.read(&candidate).is_none() {
                continue;
            }
            if !paths
                .iter()
                .any(|existing| existing.eq_ignore_ascii_case(&candidate))
            {
                paths.push(candidate);
            }
        }
    }
    paths.sort_by_key(|path| path.to_ascii_lowercase());
    Ok(paths)
}

fn collect_string_leaves(element: &nw_objectstream::Element, out: &mut Vec<String>) {
    if let Ok(value) = nw_objectstream::value::read_string(element) {
        let value = value.trim();
        if !value.is_empty() {
            out.push(value.to_owned());
        }
    }
    for child in element.children() {
        collect_string_leaves(child, out);
    }
}

/// An authored Mannequin animation database (`.adb`) or controller definition —
/// the audio-bearing databases and the controllerdef the slice references.
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
mod tests {
    use super::*;
    use crate::model_asset::tests::ContextSource;

    const ANIMS_ADB: &[u8] = br#"
        <AnimDB FragDef="animations/mannequin/adb/creature_actions.xml"
                TagDef="animations/mannequin/adb/creature_tags.xml">
          <FragmentList>
            <r_Death>
              <Fragment BlendOutDuration="0.2" Tags="">
                <AnimLayer>
                  <Blend ExitTime="0" StartTime="0" Duration="0"/>
                  <Animation name="creature_death"/>
                </AnimLayer>
              </Fragment>
            </r_Death>
            <Attack_Bite>
              <Fragment BlendOutDuration="0.2" Tags="Young">
                <AnimLayer>
                  <Blend ExitTime="0" StartTime="0" Duration="0"/>
                  <Animation name="creature_bite"/>
                </AnimLayer>
                <ProcLayer>
                  <Blend ExitTime="0" StartTime="0.5" Duration="0"/>
                  <Procedural type="Audio" contextType="AudioContext">
                    <ProceduralParams>
                      <StartTrigger value="play_vox_creature_bite"/>
                      <StopTrigger value="do_nothing"/>
                      <AttachmentJoint value="bind_mouth"/>
                    </ProceduralParams>
                  </Procedural>
                </ProcLayer>
              </Fragment>
            </Attack_Bite>
          </FragmentList>
        </AnimDB>
    "#;

    // A sibling audio database contributes ProcLayer audio for `r_Death` with no
    // animation of its own — it must pair with the animation database's animation.
    const AUDIO_ADB: &[u8] = br#"
        <AnimDB FragDef="animations/mannequin/adb/creature_actions.xml"
                TagDef="animations/mannequin/adb/creature_tags.xml">
          <FragmentList>
            <r_Death>
              <Fragment BlendOutDuration="0.2" Tags="">
                <ProcLayer>
                  <Blend ExitTime="0" StartTime="0" Duration="0"/>
                  <Procedural type="Audio" contextType="AudioContext">
                    <ProceduralParams>
                      <StartTrigger value="play_bodyfall_big"/>
                      <StopTrigger value="do_nothing"/>
                      <AttachmentJoint value="bind_pelvis"/>
                    </ProceduralParams>
                  </Procedural>
                </ProcLayer>
              </Fragment>
            </r_Death>
          </FragmentList>
        </AnimDB>
    "#;

    /// A binary ObjectStream slice naming both ADBs as `AZStd::string` fields.
    fn slice_referencing_adbs() -> Vec<u8> {
        use nw_objectstream::types;
        let string_field = |field: &str, value: &str| {
            format!(
                r#"<Class name="AZStd::string" field="{field}" value="{value}" type="{{{string}}}"/>"#,
                string = types::AZSTD_STRING,
            )
        };
        let xml = format!(
            r#"<ObjectStream version="3"><Class name="ActionControllerComponent" type="{{B1E2C3D4-0000-0000-0000-000000000001}}">{anims}{audio}{other}</Class></ObjectStream>"#,
            anims = string_field(
                "animationDatabase",
                "animations/mannequin/adb/creature_anims.adb"
            ),
            audio = string_field(
                "audioDatabase",
                "animations/mannequin/adb/creature_audio.adb"
            ),
            other = string_field("unrelated", "materials/creature.mtl"),
        );
        xml.into_bytes()
    }

    fn context() -> ContextSource {
        ContextSource::default()
            .with("slices/creature.dynamicslice", slice_referencing_adbs())
            .with("animations/mannequin/adb/creature_anims.adb", ANIMS_ADB)
            .with("animations/mannequin/adb/creature_audio.adb", AUDIO_ADB)
            .with(
                "animations/mannequin/adb/creature_actions.xml",
                b"<TagDefinition version=\"2\"><Tags/></TagDefinition>".to_vec(),
            )
            .with(
                "animations/mannequin/adb/creature_tags.xml",
                b"<TagDefinition version=\"2\"><Tags/></TagDefinition>".to_vec(),
            )
    }

    #[test]
    fn discovers_adbs_from_slice_string_fields_only_when_they_resolve() {
        let source = context();
        let discovered = discover_mannequin_reference_paths(
            &source,
            &["slices/creature.dynamicslice".to_owned()],
        )
        .unwrap();
        assert_eq!(
            discovered,
            vec![
                "animations/mannequin/adb/creature_anims.adb".to_owned(),
                "animations/mannequin/adb/creature_audio.adb".to_owned(),
            ]
        );
    }

    #[test]
    fn attaches_bite_audio_and_pairs_cross_database_death_audio() {
        let source = context();
        let mut resolved = ResolvedAsset {
            model: nw_model::Model {
                meshes: Vec::new(),
                skeletons: Vec::new(),
                auxiliary_nodes: Vec::new(),
            },
            materials: None,
            animations: Vec::new(),
            extras: nw_model::CryAssetExtras::default(),
            physics: nw_model::PhysicsScene::default(),
            parsed_animation_assets: std::collections::HashSet::new(),
        };
        attach_fragment_audio(
            &source,
            &["slices/creature.dynamicslice".to_owned()],
            &mut resolved,
        )
        .unwrap();

        let audio = &resolved.extras.mannequin_audio;
        let bite = audio
            .iter()
            .find(|entry| entry.animation == "creature_bite")
            .expect("bite animation gains its trigger");
        assert_eq!(bite.clips.len(), 1);
        assert_eq!(bite.clips[0].trigger, "play_vox_creature_bite");
        assert_eq!(bite.clips[0].start_time, 0.5);
        assert_eq!(bite.clips[0].fragment, "Attack_Bite");
        assert_eq!(bite.clips[0].tags, "Young");

        // The death fragment's audio lives in the sibling audio ADB while its
        // animation lives in the animation ADB — the merge pairs them by name.
        let death = audio
            .iter()
            .find(|entry| entry.animation == "creature_death")
            .expect("death animation gains cross-database body-fall audio");
        assert_eq!(death.clips[0].trigger, "play_bodyfall_big");

        // Both ADBs and their shared FragDef/TagDef ship as embedded resources.
        let shipped: Vec<&str> = resolved
            .extras
            .source_assets
            .iter()
            .map(|asset| asset.path.as_str())
            .collect();
        assert!(shipped.contains(&"animations/mannequin/adb/creature_anims.adb"));
        assert!(shipped.contains(&"animations/mannequin/adb/creature_audio.adb"));
        assert!(shipped.contains(&"animations/mannequin/adb/creature_actions.xml"));
        assert!(shipped.contains(&"animations/mannequin/adb/creature_tags.xml"));
    }
}
