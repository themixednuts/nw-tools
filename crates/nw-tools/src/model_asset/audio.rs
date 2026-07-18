//! Animation-event audio-trigger resolution through the authored Cry/Wwise catalogs.
//!
//! Split out of `model_asset` as a cohesive unit that depends only on
//! `AssetSource`, `ResolvedAsset`, and a few shared helpers in the parent module.

use super::*;

mod catalog;

use catalog::AudioCatalogs;

/// Default ATL / preload documents shipped under `libs/gameaudio/wwise/`.
const ATL_CONTROLS_PATH: &str = "libs/gameaudio/wwise/atl_controls.xml";
const ATL_PRELOAD_PATH: &str = "libs/gameaudio/wwise/preloaddata.xml";
const ATL_DEFAULT_CONTROLS_PATH: &str = "libs/gameaudio/wwise/default_controls.xml";
const AUDIO_TAG_DATA_PATH: &str = "libs/gameaudio/wwise/audio_tag_data.csv";

/// Resolve animation-event audio triggers end-to-end through the authored
/// catalogs and ship the exact banks/media each trigger needs.
///
/// Every hop is catalog-driven — no filename, stem, or parameter-prefix
/// matching:
/// 1. `cryEvents[].parameter` → ATL trigger (`atl_controls.xml`) → Wwise event
///    name(s). A parameter that is itself a Wwise event name in the event-id
///    tables (mapping CSVs / FNV-1) is the only fallback; nothing else resolves.
/// 2. event → owning bank(s): the Wwise trigger-bank map
///    (`triggerbankmapatlbin.bin`, keyed by `AZ::Crc32` of the event name) is
///    consulted first, then the banks grouped with it in the ATL preload catalog
///    (`preloaddata.xml`), located by which bank's HIRC defines the event id.
/// 3. event → exact media: a typed HIRC walk (Event → Action → Sound / Ran-Seq /
///    Switch / Layer container) collecting only the source ids reachable from
///    the event, each tagged with the shipped bank whose DIDX owns it (or a
///    loose `sounds/wwise/<mediaId>.wem` when no shipped bank embeds it).
///
/// A manifest-level `extras.audioTriggers` table lets each keyframed event
/// resolve in one hop. Parameters that resolve to no catalog entry are dropped
/// with one summary note rather than shipped half-filled.
pub(super) fn resolve_animation_audio_triggers(
    source: &dyn AssetSource,
    resolved: &mut ResolvedAsset,
) -> Result<()> {
    // Cheap gate: skip the catalog load entirely when there is no animevent
    // audio and no Mannequin fragment audio to resolve.
    if !has_any_audio_work(resolved) {
        return Ok(());
    }

    ensure_atl_control_sources(source, resolved)?;
    let catalogs = AudioCatalogs::load(source, resolved)?;

    let mut unresolved_contexts = std::collections::BTreeSet::new();
    for entry in &mut resolved.extras.mannequin_audio {
        for clip in &mut entry.clips {
            character_event::map_clip(clip, &catalogs);
            if clip.character_event.is_some() && clip.context.is_none() {
                unresolved_contexts.insert(format!(
                    "{} (no same-entity receiver context)",
                    clip.character_event.as_deref().unwrap_or_default()
                ));
            } else if clip.context.as_ref().is_some_and(|context| {
                context.receivers.iter().any(|receiver| {
                    matches!(
                        receiver,
                        nw_model::CryCharacterEventReceiver::CommonNpcAudio { .. }
                            | nw_model::CryCharacterEventReceiver::MountAudio { .. }
                    )
                })
            }) && clip
                .dispatches
                .iter()
                .all(|dispatch| dispatch.valid_tag.is_none())
            {
                let context = clip.context.as_ref().expect("checked above");
                unresolved_contexts.insert(format!(
                    "{} entity {} in {} (no ValidTag CRC match)",
                    clip.character_event.as_deref().unwrap_or_default(),
                    context.entity_id,
                    context.scene_path
                ));
            }
        }
    }
    if !unresolved_contexts.is_empty() {
        eprintln!(
            "note: {} CharacterEvent receiver context(s) produced no tag-scoped audio: {}",
            unresolved_contexts.len(),
            unresolved_contexts
                .into_iter()
                .collect::<Vec<_>>()
                .join(", ")
        );
    }

    let mut triggers = collect_animation_audio_triggers(resolved);
    if triggers.is_empty() {
        return Ok(());
    }
    triggers.sort_by(|left, right| left.parameter.cmp(&right.parameter));
    // Dedup by parameter, keeping the footstep binding when an event fires the
    // same parameter both ways.
    triggers.dedup_by(|later, earlier| {
        if later.parameter == earlier.parameter {
            earlier.is_footstep |= later.is_footstep;
            true
        } else {
            false
        }
    });

    // Parse each Wwise bank at most once for the whole resolution and read raw
    // bank bytes only on demand: hundreds of candidate banks per event otherwise
    // re-decompress + re-parse on every trigger, and holding a whole preload
    // group's media DATA resident at once exhausts memory on large characters.
    let mut banks = BankStore::new(source);

    let mut resolutions = Vec::with_capacity(triggers.len());
    let mut dropped = Vec::new();
    for candidate in triggers {
        let Some(resolution) = resolve_one_audio_trigger(
            source,
            &mut banks,
            &catalogs,
            &candidate.parameter,
            candidate.is_footstep,
        )?
        else {
            dropped.push(candidate.parameter);
            continue;
        };
        if candidate.is_footstep {
            ship_material_effects_fxlib(source, &candidate.parameter, &mut resolved.extras)?;
        }
        for bank in &resolution.banks {
            add_audio_source(source, bank, &mut resolved.extras)?;
        }
        resolutions.push(resolution);
    }
    if !dropped.is_empty() {
        eprintln!(
            "note: {} animation audio trigger(s) resolved to no catalog entry and were dropped: {}",
            dropped.len(),
            dropped.join(", ")
        );
    }
    resolved.extras.audio_triggers = resolutions;
    Ok(())
}

/// An animation-event parameter worth resolving, tagged with how it resolves:
/// a `footstep` event's parameter names a MaterialEffects FX library; a
/// `sound`/`audio` event's parameter is an ATL trigger directly.
struct AudioCandidate {
    parameter: String,
    is_footstep: bool,
}

fn collect_animation_audio_triggers(resolved: &ResolvedAsset) -> Vec<AudioCandidate> {
    // Event-name gating is only a cheap candidate filter; whether a parameter
    // contributes audio is decided by successful catalog resolution, not by
    // its name shape.
    let mut triggers = Vec::new();
    for animation in &resolved.animations {
        for event in &animation.clip.events {
            let Some(is_footstep) = audio_event_kind(event) else {
                continue;
            };
            let parameter = event.parameter.trim();
            if !parameter.is_empty() {
                triggers.push(AudioCandidate {
                    parameter: parameter.to_owned(),
                    is_footstep,
                });
            }
        }
    }
    for entry in &resolved.extras.mannequin_audio {
        for clip in &entry.clips {
            if clip.character_event.is_none() {
                for trigger in [Some(clip.trigger.as_str()), clip.stop_trigger.as_deref()]
                    .into_iter()
                    .flatten()
                {
                    let trigger = trigger.trim();
                    if !trigger.is_empty() {
                        triggers.push(AudioCandidate {
                            parameter: trigger.to_owned(),
                            is_footstep: false,
                        });
                    }
                }
            }
            for dispatch in &clip.dispatches {
                for operation in &dispatch.operations {
                    let nw_model::CryCharacterEventOperation::AudioControl { control, .. } =
                        operation
                    else {
                        continue;
                    };
                    let control = control.trim();
                    if !control.is_empty() {
                        triggers.push(AudioCandidate {
                            parameter: control.to_owned(),
                            is_footstep: false,
                        });
                    }
                }
            }
        }
    }
    triggers
}

/// Whether there is any animation audio to resolve at all: an animevent that
/// gates as audio with a parameter, or any Mannequin fragment-audio clip. Used to
/// skip the catalog load on assets with no audio.
fn has_any_audio_work(resolved: &ResolvedAsset) -> bool {
    let has_animevent =
        resolved.animations.iter().any(|animation| {
            animation.clip.events.iter().any(|event| {
                audio_event_kind(event).is_some() && !event.parameter.trim().is_empty()
            })
        });
    has_animevent
        || resolved
            .extras
            .mannequin_audio
            .iter()
            .any(|entry| !entry.clips.is_empty())
}

/// `Some(true)` for a footstep event (parameter → FX library), `Some(false)` for
/// a direct `sound`/`audio` event (parameter → ATL trigger), `None` otherwise.
///
/// The semantic event *type* is the only candidate gate — no parameter
/// name-shape heuristics (`Play_`/`Stop_`/`blend_`/`ftsp_` …); the catalogs
/// decide inclusion downstream.
fn audio_event_kind(event: &cry_animation::AnimationEvent) -> Option<bool> {
    let name = event.name.trim();
    if name.eq_ignore_ascii_case("footstep") {
        Some(true)
    } else if name.eq_ignore_ascii_case("sound")
        || name.eq_ignore_ascii_case("audio")
        || name.eq_ignore_ascii_case("audio_trigger")
    {
        Some(false)
    } else {
        None
    }
}

fn ensure_atl_control_sources(
    source: &dyn AssetSource,
    resolved: &mut ResolvedAsset,
) -> Result<()> {
    for path in [
        ATL_CONTROLS_PATH,
        ATL_PRELOAD_PATH,
        ATL_DEFAULT_CONTROLS_PATH,
        AUDIO_TAG_DATA_PATH,
        cry_audio::WWISE_TRIGGER_BANK_MAP_FILE,
    ] {
        if has_source_asset(&resolved.extras, path) {
            continue;
        }
        if source.read(path).is_none() {
            continue;
        }
        // Control/lookup docs only — do not ship the entire preload catalog.
        add_audio_source_with_options(source, path, &mut resolved.extras, false)?;
    }
    Ok(())
}

fn resolve_one_audio_trigger(
    source: &dyn AssetSource,
    banks: &mut BankStore,
    catalogs: &AudioCatalogs,
    parameter: &str,
    is_footstep: bool,
) -> Result<Option<nw_model::AudioTriggerResolution>> {
    // Step 1: parameter → ATL trigger(s). A footstep parameter names a
    // MaterialEffects FX library that lists the real ATL trigger(s) plus the
    // surfaces they cover; the trigger frequently differs in case from the
    // parameter (`fstp_run_dryad` → `FTSP_run_dryad`) and can differ outright.
    // A direct `sound`/`audio` parameter is the ATL trigger itself.
    let library = if is_footstep {
        load_footstep_fxlib(source, parameter)?
    } else {
        None
    };
    let (atl_triggers, surfaces) = match &library {
        Some(library) => (library.triggers(), library.surfaces()),
        // No FX library shipped (or a direct `sound`/`audio` event) — fall back to
        // treating the parameter as the trigger so a direct authoring resolves.
        None => (vec![parameter.to_owned()], Vec::new()),
    };

    // Step 2: ATL trigger(s) → Wwise event name(s).
    let mut event_names = Vec::new();
    let mut playback = None;
    for atl_trigger in &atl_triggers {
        let (events, trigger_playback) = catalogs.trigger_events(atl_trigger);
        event_names.extend(events);
        if playback.is_none() {
            playback = trigger_playback;
        }
    }
    if event_names.is_empty() {
        // Fallback: an ATL trigger that is itself an authored Wwise event name.
        for atl_trigger in &atl_triggers {
            if let Some(canonical) = catalogs.canonical_event_name(atl_trigger) {
                event_names.push(canonical);
            }
        }
    }
    event_names.sort();
    event_names.dedup();
    if event_names.is_empty() {
        // Matches no ATL trigger and no event-id table entry — nothing resolves.
        return Ok(None);
    }

    let mut wwise_events = Vec::new();
    let mut ship_banks: Vec<String> = Vec::new();
    // media id → owning bank path (None ⇒ streamed / loose `.wem`).
    let mut media: std::collections::BTreeMap<u32, Option<String>> =
        std::collections::BTreeMap::new();
    // Media the default switch branch reaches (the default surface's variations),
    // a subset of `media`. Consumers rotate these instead of combing one sample.
    let mut default_media: std::collections::BTreeSet<u32> = std::collections::BTreeSet::new();
    // Per-surface breakdown: every switch-container branch, unioned across events
    // by switch id in first-seen order (item 2), plus the default branch's
    // deterministic weighted sequence (item 3).
    let mut surface_branches: Vec<SurfaceBranch> = Vec::new();

    for name in &event_names {
        let event_id = catalogs.event_id(name);
        wwise_events.push(nw_model::AudioTriggerWwiseEvent {
            name: name.clone(),
            id: Some(event_id),
        });

        // Step 2: event → candidate banks (trigger-bank map, else preload group).
        let mut candidates = catalogs.map_banks(name);
        if candidates.is_empty() {
            candidates = catalogs.preload_banks_defining_event(banks, event_id);
        }
        if candidates.is_empty() {
            continue;
        }

        // The readable, parseable candidate banks, deduped by path in first-seen
        // order (matching the former eager loader). Only the light bank index is
        // held here — the raw media DATA is never resident for the whole group.
        let mut seen = HashSet::new();
        let loaded: Vec<(String, Arc<cry_audio::WwiseSoundBank>)> = candidates
            .iter()
            .filter(|path| seen.insert(path.to_ascii_lowercase()))
            .filter_map(|path| banks.parsed(path).map(|bank| (path.clone(), bank)))
            .collect();

        // Step 3: typed HIRC walk in the defining bank(s); locate each media's
        // owning bank among the shipped candidates. Raw bytes are read on demand
        // for the (few) defining banks and dropped immediately after the walk.
        let event = cry_audio::WwiseObjectId(event_id);
        let mut source_ids = std::collections::BTreeSet::new();
        for (path, bank) in &loaded {
            if !bank.defines_event(event) {
                continue;
            }
            let Some(bytes) = banks.bytes(path) else {
                continue;
            };
            push_unique_path(&mut ship_banks, path);
            source_ids.extend(bank.event_media(&bytes, event).iter().map(|id| id.0));
            let event_default: Vec<u32> = bank
                .event_default_media(&bytes, event)
                .iter()
                .map(|id| id.0)
                .collect();
            default_media.extend(&event_default);
            accumulate_surface_branches(bank, &bytes, event, &event_default, &mut surface_branches);
        }
        for media_id in source_ids {
            let owner = loaded
                .iter()
                .find(|(_, bank)| bank.media.iter().any(|media| media.id.0 == media_id))
                .map(|(path, _)| path.clone());
            if let Some(bank) = &owner {
                push_unique_path(&mut ship_banks, bank);
            }
            media.entry(media_id).or_insert(owner);
        }
    }

    ship_banks.sort_by_key(|path| path.to_ascii_lowercase());
    ship_banks.dedup_by(|left, right| left.eq_ignore_ascii_case(right));

    let media = media
        .into_iter()
        .map(|(media_id, bank)| nw_model::AudioTriggerMediaRef {
            media_id,
            bank,
            path: None,
            default_branch: default_media.contains(&media_id),
        })
        .collect();

    let surface_media = build_surface_media(
        surface_branches,
        &default_media,
        library.as_ref(),
        catalogs,
        parameter,
    );

    Ok(Some(nw_model::AudioTriggerResolution {
        // Keyed by the authored parameter so `cryEvents[].parameter` resolves in
        // one hop, regardless of the intermediate FX-library trigger name.
        trigger: parameter.to_owned(),
        wwise_events,
        banks: ship_banks,
        media,
        surfaces,
        surface_media,
        playback,
    }))
}

/// A switch-container branch accumulated during the HIRC walk: its switch id,
/// whether it is the engine default, the media it reaches, and — for the default
/// branch — the deterministic weighted selection order.
struct SurfaceBranch {
    switch_id: u32,
    is_default: bool,
    media: std::collections::BTreeSet<u32>,
    sequence: Vec<u32>,
}

/// A blend preview sequence long enough to cover a clip's footsteps without an
/// obvious cycle, bounded so the manifest stays small.
fn weighted_preview_len(distinct: usize) -> usize {
    (distinct.saturating_mul(2)).clamp(8, 64)
}

/// Enumerate the event's switch branches (item 2) and compute the default
/// branch's engine-faithful weighted sequence (item 3), merging into `branches`
/// by switch id. Events with no switch container get one synthetic default
/// branch (switch id 0) carrying the blend/random weighted sequence.
fn accumulate_surface_branches(
    bank: &cry_audio::WwiseSoundBank,
    bytes: &[u8],
    event: cry_audio::WwiseObjectId,
    event_default: &[u32],
    branches: &mut Vec<SurfaceBranch>,
) {
    let switch_branches = bank.event_switch_branches(bytes, event);
    if switch_branches.is_empty() {
        // No switch: a single synthetic default branch over the blend/random pool.
        let entry = branch_entry(branches, 0);
        entry.is_default = true;
        entry.media.extend(event_default.iter().copied());
        if entry.sequence.is_empty() {
            let count = weighted_preview_len(event_default.len());
            entry.sequence = bank
                .event_weighted_sequence(bytes, event, 0, count)
                .iter()
                .map(|id| id.0)
                .collect();
        }
        return;
    }

    let default_switch = switch_branches
        .iter()
        .find(|branch| branch.is_default)
        .map(|branch| branch.switch_id);
    for branch in &switch_branches {
        let entry = branch_entry(branches, branch.switch_id);
        entry.is_default |= branch.is_default;
        entry.media.extend(branch.media.iter().map(|media| media.0));
    }
    if let Some(default_switch) = default_switch {
        let distinct = switch_branches
            .iter()
            .find(|branch| branch.switch_id == default_switch)
            .map_or(0, |branch| branch.media.len());
        let count = weighted_preview_len(distinct);
        let sequence: Vec<u32> = bank
            .event_weighted_sequence(bytes, event, default_switch, count)
            .iter()
            .map(|id| id.0)
            .collect();
        if let Some(entry) = branches
            .iter_mut()
            .find(|entry| entry.switch_id == default_switch)
            && entry.sequence.is_empty()
        {
            entry.sequence = sequence;
        }
    }
}

fn branch_entry(branches: &mut Vec<SurfaceBranch>, switch_id: u32) -> &mut SurfaceBranch {
    if let Some(index) = branches
        .iter()
        .position(|entry| entry.switch_id == switch_id)
    {
        return &mut branches[index];
    }
    branches.push(SurfaceBranch {
        switch_id,
        is_default: false,
        media: std::collections::BTreeSet::new(),
        sequence: Vec::new(),
    });
    branches.last_mut().expect("just pushed")
}

/// Finish the per-surface breakdown: resolve each branch's surface name through
/// the FX library + Wwise switch ids (validating the hash — never a guess), and
/// emit a note when a footstep's branches resolve by switch id only.
fn build_surface_media(
    branches: Vec<SurfaceBranch>,
    default_media: &std::collections::BTreeSet<u32>,
    library: Option<&cry_audio::MaterialEffectsLibrary>,
    catalogs: &AudioCatalogs,
    parameter: &str,
) -> Vec<nw_model::AudioTriggerSurfaceMedia> {
    let mut resolved_by_name = 0usize;
    let mut id_only = 0usize;
    let mut surface_media = Vec::with_capacity(branches.len());
    for branch in branches {
        // A synthetic single branch (switch id 0) carries the blend's default
        // media; a real switch branch carries the media it plays.
        let media: Vec<u32> = if branch.switch_id == 0 && branch.media.is_empty() {
            default_media.iter().copied().collect()
        } else {
            branch.media.iter().copied().collect()
        };
        let surface = if branch.switch_id == 0 {
            None
        } else {
            let name = resolve_surface_name(branch.switch_id, library, catalogs);
            if name.is_some() {
                resolved_by_name += 1;
            } else {
                id_only += 1;
            }
            name
        };
        surface_media.push(nw_model::AudioTriggerSurfaceMedia {
            surface,
            switch_id: branch.switch_id,
            default: branch.is_default,
            media,
            sequence: if branch.is_default {
                branch.sequence
            } else {
                Vec::new()
            },
        });
    }
    if id_only > 0 {
        eprintln!(
            "note: footstep trigger '{parameter}': {resolved_by_name} surface(s) resolved by \
             name, {id_only} switch branch(es) unresolved (kept by switch id only — no guessed \
             surface mapping shipped)"
        );
    }
    surface_media
}

/// Resolve a Wwise switch id to an FX-library surface name, but only when the
/// name's `AK::SoundEngine` hash (or its ATL-mapped Wwise state name) actually
/// equals `switch_id`. A mismatch returns `None` — the branch is kept by id
/// alone rather than shipping a guessed mapping.
fn resolve_surface_name(
    switch_id: u32,
    library: Option<&cry_audio::MaterialEffectsLibrary>,
    catalogs: &AudioCatalogs,
) -> Option<String> {
    let library = library?;
    for effect in &library.effects {
        for audio in &effect.audio {
            for switch in &audio.switches {
                // Candidate Wwise state names: the ATL switch definition's mapped
                // `WwiseValue`(s), plus the raw FX-library state as a fallback.
                let mut candidates = catalogs.wwise_switch_state_names(&switch.name, &switch.state);
                candidates.push(switch.state.clone());
                for candidate in candidates {
                    if cry_audio::WwiseNameId::from_name(&candidate).0 == switch_id {
                        return Some(switch.state.clone());
                    }
                }
            }
        }
    }
    None
}

/// Load and parse the MaterialEffects FX library a footstep `parameter` names,
/// or `None` when the install ships no such library.
fn load_footstep_fxlib(
    source: &dyn AssetSource,
    parameter: &str,
) -> Result<Option<cry_audio::MaterialEffectsLibrary>> {
    let path = cry_audio::footstep_fxlib_path(parameter);
    let Some(bytes) = source.read(&path) else {
        return Ok(None);
    };
    let xml = str::from_utf8(&bytes).with_context(|| format!("decode UTF-8 FX library {path}"))?;
    let library = cry_audio::MaterialEffectsLibrary::from_xml(&path, xml)
        .with_context(|| format!("parse FX library {path}"))?;
    Ok(Some(library))
}

/// Ship the footstep FX library at its catalog path — a control document glTF
/// cannot represent, retained losslessly alongside the ATL chain.
fn ship_material_effects_fxlib(
    source: &dyn AssetSource,
    parameter: &str,
    extras: &mut nw_model::CryAssetExtras,
) -> Result<()> {
    let path = cry_audio::footstep_fxlib_path(parameter);
    if has_source_asset(extras, &path) {
        return Ok(());
    }
    let Some(bytes) = source.read(&path) else {
        return Ok(());
    };
    let xml = str::from_utf8(&bytes).with_context(|| format!("decode UTF-8 FX library {path}"))?;
    let library = cry_audio::MaterialEffectsLibrary::from_xml(&path, xml)
        .with_context(|| format!("parse FX library {path}"))?;
    extras.source_assets.push(nw_model::CrySourceAsset {
        path: normalize_path(&path),
        kind: nw_model::CrySourceAssetKind::MaterialEffectsFxLibrary,
        document: serde_json::to_value(library)?,
    });
    add_resource(
        extras,
        &path,
        nw_model::CryEmbeddedResourceKind::MaterialEffectsFxLibrary,
        bytes,
    );
    add_dependency(extras, &path);
    Ok(())
}

/// A resolution-scoped cache of parsed Wwise banks.
///
/// Bank resolution otherwise re-reads and re-parses the same banks hundreds of
/// times (every trigger's fallback scan walks every preload group) and, worse,
/// holds a whole preload group's raw media DATA resident at once — enough to
/// exhaust memory on a player-grade character with many audio events. The store
/// parses each bank at most once and keeps only the light index (sections, media
/// table, HIRC), which is all that `defines_event` and the DIDX owner lookup
/// need; the raw bytes required for the typed HIRC walk are read on demand for
/// the (few) defining banks and dropped immediately after.
struct BankStore<'a> {
    source: &'a dyn AssetSource,
    /// Lowercased path → parsed bank (`None` when unreadable or unparseable),
    /// so a repeated miss is not re-read every time either.
    parsed: std::collections::HashMap<String, Option<Arc<cry_audio::WwiseSoundBank>>>,
}

impl<'a> BankStore<'a> {
    fn new(source: &'a dyn AssetSource) -> Self {
        Self {
            source,
            parsed: std::collections::HashMap::new(),
        }
    }

    /// The parsed bank at `path`, cached by lowercased path. `None` when the bank
    /// is unreadable or does not parse. Never retains the raw bank bytes.
    fn parsed(&mut self, path: &str) -> Option<Arc<cry_audio::WwiseSoundBank>> {
        if let Some(cached) = self.parsed.get(&path.to_ascii_lowercase()) {
            return cached.clone();
        }
        let parsed = self
            .source
            .read(path)
            .and_then(|bytes| cry_audio::WwiseSoundBank::parse(&bytes).ok())
            .map(Arc::new);
        self.parsed
            .insert(path.to_ascii_lowercase(), parsed.clone());
        parsed
    }

    /// The raw bank bytes at `path`, read fresh (never cached). Deterministic for
    /// a given path, so the absolute HIRC offsets in the cached parse still line
    /// up with this buffer.
    fn bytes(&self, path: &str) -> Option<Vec<u8>> {
        self.source.read(path)
    }
}

#[cfg(test)]
mod tests;
