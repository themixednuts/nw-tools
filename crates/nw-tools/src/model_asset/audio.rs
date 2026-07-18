//! Animation-event audio-trigger resolution through the authored Cry/Wwise catalogs.
//!
//! Split out of `model_asset` as a cohesive unit that depends only on
//! `AssetSource`, `ResolvedAsset`, and a few shared helpers in the parent module.

use super::*;

/// Default ATL / preload documents shipped under `libs/gameaudio/wwise/`.
const ATL_CONTROLS_PATH: &str = "libs/gameaudio/wwise/atl_controls.xml";
const ATL_PRELOAD_PATH: &str = "libs/gameaudio/wwise/preloaddata.xml";
const ATL_DEFAULT_CONTROLS_PATH: &str = "libs/gameaudio/wwise/default_controls.xml";

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

    // Expand Mannequin `CharacterEvent` short names (`Bite`, `VOX_Attack1`) to
    // shipped Wwise events, or drop the clip when the catalog confirms none —
    // this rewrites each clip's `trigger` in place so it flows through the same
    // trigger→bank→HIRC pipeline below.
    resolve_mannequin_character_events(&catalogs, resolved);

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

    let mut resolutions = Vec::with_capacity(triggers.len());
    let mut dropped = Vec::new();
    for candidate in triggers {
        let Some(resolution) = resolve_one_audio_trigger(
            source,
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
    // Mannequin fragment audio clips fire ATL triggers directly (creature
    // vocals/actions). They resolve like a direct `sound` event: the trigger name
    // is the ATL control itself.
    for entry in &resolved.extras.mannequin_audio {
        for clip in &entry.clips {
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
    }
    triggers
}

/// Whether there is any animation audio to resolve at all: an animevent that
/// gates as audio with a parameter, or any Mannequin fragment-audio clip. Used to
/// skip the catalog load on assets with no audio.
fn has_any_audio_work(resolved: &ResolvedAsset) -> bool {
    let has_animevent = resolved.animations.iter().any(|animation| {
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

/// Expand every Mannequin `CharacterEvent` clip's short name to a shipped Wwise
/// event through the catalog, rewriting the clip's `trigger` in place. A clip
/// whose short name the catalog confirms as no event is dropped (never shipped
/// with a guessed trigger), matching the footstep surface discipline.
fn resolve_mannequin_character_events(catalogs: &AudioCatalogs, resolved: &mut ResolvedAsset) {
    let tags = catalogs.character_audio_tags();
    let mut dropped: Vec<String> = Vec::new();
    for entry in &mut resolved.extras.mannequin_audio {
        entry.clips.retain_mut(|clip| {
            let Some(short_name) = clip.character_event.clone() else {
                // A direct `type="Audio"` ATL clip already carries a real trigger.
                return true;
            };
            match catalogs.resolve_character_event(&tags, &short_name) {
                Some(event) => {
                    clip.trigger = event;
                    true
                }
                None => {
                    dropped.push(short_name);
                    false
                }
            }
        });
    }
    if !dropped.is_empty() {
        dropped.sort();
        dropped.dedup();
        eprintln!(
            "note: {} Mannequin CharacterEvent name(s) matched no catalog event and were dropped: {}",
            dropped.len(),
            dropped.join(", ")
        );
    }
}

/// The character audio tag(s) a Wwise event name carries: the token after
/// `Play_`/`Stop_` and an optional `SFX`/`VOX`/`MMFX` type token
/// (`Play_SFX_Aligator_Body_Mvt` → `Aligator`, `Play_Alligator_Breathing` →
/// `Alligator`). `None` when the name does not fit the template.
fn character_tag_from_event(name: &str) -> Option<String> {
    let mut parts = name.split('_').filter(|part| !part.is_empty());
    let verb = parts.next()?;
    if !verb.eq_ignore_ascii_case("Play") && !verb.eq_ignore_ascii_case("Stop") {
        return None;
    }
    let second = parts.next()?;
    let tag = if audio_type_token(second).is_some() {
        parts.next()?
    } else {
        second
    };
    (!tag.is_empty()).then(|| tag.to_owned())
}

/// The canonical Wwise audio type token (`SFX`/`VOX`/`MMFX`) a token names,
/// case-insensitively, or `None`.
fn audio_type_token(token: &str) -> Option<&'static str> {
    ["SFX", "VOX", "MMFX"]
        .into_iter()
        .find(|candidate| token.eq_ignore_ascii_case(candidate))
}

/// Split a `SFX_`/`VOX_`/`MMFX_` prefix off a short character-event name:
/// `VOX_Attack1` → `(Some("VOX"), Some("Attack1"))`; anything else →
/// `(None, None)`.
fn split_audio_type_prefix(name: &str) -> (Option<&'static str>, Option<String>) {
    if let Some((first, rest)) = name.split_once('_')
        && let Some(token) = audio_type_token(first)
        && !rest.is_empty()
    {
        return (Some(token), Some(rest.to_owned()));
    }
    (None, None)
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
            if catalogs.knows_event_name(atl_trigger) {
                event_names.push(atl_trigger.clone());
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
            candidates = catalogs.preload_banks_defining_event(source, event_id);
        }
        if candidates.is_empty() {
            continue;
        }

        // Step 3: typed HIRC walk in the defining bank(s); locate each media's
        // owning bank among the shipped candidates.
        let loaded = load_audio_banks(source, &candidates);
        let event = cry_audio::WwiseObjectId(event_id);
        let mut source_ids = std::collections::BTreeSet::new();
        for entry in &loaded {
            if entry.bank.defines_event(event) {
                push_unique_path(&mut ship_banks, &entry.path);
                source_ids.extend(
                    entry
                        .bank
                        .event_media(&entry.bytes, event)
                        .iter()
                        .map(|id| id.0),
                );
                let event_default: Vec<u32> = entry
                    .bank
                    .event_default_media(&entry.bytes, event)
                    .iter()
                    .map(|id| id.0)
                    .collect();
                default_media.extend(&event_default);
                accumulate_surface_branches(
                    &entry.bank,
                    &entry.bytes,
                    event,
                    &event_default,
                    &mut surface_branches,
                );
            }
        }
        for media_id in source_ids {
            let owner = loaded
                .iter()
                .find(|entry| entry.bank.media.iter().any(|media| media.id.0 == media_id))
                .map(|entry| entry.path.clone());
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
    if let Some(index) = branches.iter().position(|entry| entry.switch_id == switch_id) {
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

/// A bank parsed once, retained with its bytes for the HIRC walk and DIDX
/// owner lookup.
struct LoadedAudioBank {
    path: String,
    bank: cry_audio::WwiseSoundBank,
    bytes: Vec<u8>,
}

fn load_audio_banks(source: &dyn AssetSource, paths: &[String]) -> Vec<LoadedAudioBank> {
    let mut loaded = Vec::new();
    let mut seen = HashSet::new();
    for path in paths {
        if !seen.insert(path.to_ascii_lowercase()) {
            continue;
        }
        let Some(bytes) = source.read(path) else {
            continue;
        };
        let Ok(bank) = cry_audio::WwiseSoundBank::parse(&bytes) else {
            continue;
        };
        loaded.push(LoadedAudioBank {
            path: path.clone(),
            bank,
            bytes,
        });
    }
    loaded
}

fn parse_audio_controls_document(
    source: &dyn AssetSource,
    path: &str,
) -> Result<Option<cry_audio::AudioControlsSource>> {
    let Some(bytes) = source.read(path) else {
        return Ok(None);
    };
    let xml =
        str::from_utf8(&bytes).with_context(|| format!("decode UTF-8 ATL controls {path}"))?;
    let controls = cry_audio::AudioControlsSource::from_xml(path, xml)
        .with_context(|| format!("parse ATL controls {path}"))?;
    Ok(Some(controls))
}

fn collect_wwise_event_names(reference: &cry_audio::AudioBackendReference, out: &mut Vec<String>) {
    if reference.kind == cry_audio::AudioBackendReferenceKind::WwiseEvent {
        if let Some(name) = reference
            .wwise_name
            .as_deref()
            .or(reference.atl_name.as_deref())
        {
            let name = name.trim();
            if !name.is_empty() {
                out.push(name.to_owned());
            }
        }
    }
    for child in &reference.children {
        collect_wwise_event_names(child, out);
    }
}

/// Collect every `wwise_name` in a backend-reference subtree (a switch definition
/// nests `WwiseSwitch` → `WwiseValue`); the switch-state value name the engine
/// hashes is among them.
fn collect_wwise_value_names(reference: &cry_audio::AudioBackendReference, out: &mut Vec<String>) {
    if let Some(name) = reference.wwise_name.as_deref().or(reference.atl_name.as_deref()) {
        let name = name.trim();
        if !name.is_empty() {
            out.push(name.to_owned());
        }
    }
    for child in &reference.children {
        collect_wwise_value_names(child, out);
    }
}

/// The authored audio catalogs, parsed once per export to drive trigger
/// resolution without any name/stem matching.
struct AudioCatalogs {
    /// Every loaded ATL control document (discovery + `--audio`).
    controls: Vec<cry_audio::AudioControlsSource>,
    /// Wwise trigger-bank map entries (empty when the install ships none).
    trigger_bank_map: Vec<cry_audio::WwiseTriggerBankMapEntry>,
    /// `AZ::Crc32(bank stem)` → catalog bank path, over the whole preload
    /// catalog. Lets the trigger-bank map's crc fields resolve to bank paths.
    crc_to_bank: std::collections::HashMap<u32, String>,
    /// Preload config-group bank sets, for the fallback bank lookup. A group
    /// bundles an event bank with its media bank(s), so the whole group is the
    /// candidate set once one member defines the event.
    preload_groups: Vec<Vec<String>>,
    /// Wwise event name → id, from the shipped event-id mapping CSVs.
    event_ids: std::collections::HashMap<String, u32>,
}

impl AudioCatalogs {
    fn load(source: &dyn AssetSource, resolved: &ResolvedAsset) -> Result<Self> {
        let mut controls = Vec::new();
        for asset in &resolved.extras.source_assets {
            if matches!(asset.kind, nw_model::CrySourceAssetKind::AudioControls) {
                if let Some(document) = parse_audio_controls_document(source, &asset.path)? {
                    controls.push(document);
                }
            }
        }

        let mut crc_to_bank = std::collections::HashMap::new();
        let mut preload_groups = Vec::new();
        for control in &controls {
            for group in preload_bank_groups(control) {
                for path in &group {
                    crc_to_bank
                        .entry(cry_audio::az_crc32(bank_stem(path).as_bytes()))
                        .or_insert_with(|| path.clone());
                }
                if !group.is_empty() {
                    preload_groups.push(group);
                }
            }
        }

        let trigger_bank_map = source
            .read(cry_audio::WWISE_TRIGGER_BANK_MAP_FILE)
            .and_then(|bytes| {
                cry_audio::WwiseTriggerBankMap::parse(&bytes)
                    .ok()
                    .map(|map| map.entries().collect::<Vec<_>>())
            })
            .unwrap_or_default();

        let mut event_ids = std::collections::HashMap::new();
        for asset in &resolved.extras.source_assets {
            if !matches!(asset.kind, nw_model::CrySourceAssetKind::AudioMapping) {
                continue;
            }
            let Some(bytes) = source.read(&asset.path) else {
                continue;
            };
            // Re-parse the mapping CSV with the typed parser; a malformed or
            // non-event-id mapping is skipped rather than failing the export.
            if let Ok(cry_audio::AudioMappingDocument::EventIds(ids)) =
                cry_audio::parse_audio_mapping(&asset.path, &bytes)
            {
                for entry in ids.events {
                    event_ids.insert(entry.name, entry.id);
                }
            }
        }

        Ok(Self {
            controls,
            trigger_bank_map,
            crc_to_bank,
            preload_groups,
            event_ids,
        })
    }

    /// The Wwise event name(s) an ATL trigger fans out to, plus its playback
    /// info. Empty when the parameter is not an authored ATL trigger.
    fn trigger_events(
        &self,
        trigger: &str,
    ) -> (Vec<String>, Option<nw_model::AudioTriggerPlayback>) {
        let mut events = Vec::new();
        let mut playback = None;
        for control in &self.controls {
            let Some(atl) = control
                .triggers
                .iter()
                .find(|entry| entry.name.eq_ignore_ascii_case(trigger))
            else {
                continue;
            };
            for request in &atl.requests {
                collect_wwise_event_names(request, &mut events);
            }
            if playback.is_none() {
                if let Some(info) = &atl.playback_info {
                    playback = Some(nw_model::AudioTriggerPlayback {
                        max_radius: info.max_radius,
                        max_duration: info.max_duration,
                    });
                }
            }
        }
        (events, playback)
    }

    /// The Wwise object id for an event name: the authored mapping-CSV id when
    /// present, else the FNV-1 hash the engine derives from the name.
    fn event_id(&self, name: &str) -> u32 {
        self.event_ids
            .iter()
            .find(|(candidate, _)| candidate.eq_ignore_ascii_case(name))
            .map(|(_, id)| *id)
            .unwrap_or_else(|| cry_audio::WwiseNameId::from_name(name).0)
    }

    fn knows_event_name(&self, name: &str) -> bool {
        self.canonical_event_name(name).is_some()
    }

    /// The event-id catalog's canonical spelling of an event name (its exact CSV
    /// casing), matched case-insensitively, or `None` when no event matches.
    fn canonical_event_name(&self, name: &str) -> Option<String> {
        self.event_ids
            .keys()
            .find(|candidate| candidate.eq_ignore_ascii_case(name))
            .cloned()
    }

    /// Whether an ATL trigger with this exact name is authored in the loaded
    /// audio-controls (the first hop a `CharacterEvent` short name tries).
    fn has_trigger(&self, name: &str) -> bool {
        self.controls.iter().any(|control| {
            control
                .triggers
                .iter()
                .any(|trigger| trigger.name.eq_ignore_ascii_case(name))
        })
    }

    /// The character audio tag(s) the loaded event-id catalog uses, most-frequent
    /// first — derived from the shipped event names themselves, never hardcoded.
    /// New World's alligator catalog mixes the correct `Alligator` with the
    /// misspelled `Aligator`, so both are returned; candidate validation against
    /// the CSV keeps only names that are real events.
    fn character_audio_tags(&self) -> Vec<String> {
        let mut counts: Vec<(String, usize)> = Vec::new();
        for name in self.event_ids.keys() {
            let Some(tag) = character_tag_from_event(name) else {
                continue;
            };
            if let Some(entry) = counts
                .iter_mut()
                .find(|(existing, _)| existing.eq_ignore_ascii_case(&tag))
            {
                entry.1 += 1;
            } else {
                counts.push((tag, 1));
            }
        }
        counts.sort_by(|left, right| right.1.cmp(&left.1).then_with(|| left.0.cmp(&right.0)));
        counts.into_iter().map(|(tag, _)| tag).collect()
    }

    /// Resolve a Mannequin `CharacterEvent` short name (`Bite`, `VOX_Attack1`) to
    /// a shipped Wwise event, catalog-validated at every hop:
    ///   a. the short name is itself an authored ATL trigger → use it directly;
    ///   b. else derive candidate Wwise event names by the observed template
    ///      (`Play_SFX_<Tag>_<Name>`, and `Play_<TYPE>_<Tag>_<rest>` for a
    ///      `VOX_`/`SFX_`/`MMFX_`-prefixed name) over each character tag and keep
    ///      the first the event-id catalog confirms.
    /// `None` when nothing in the catalog confirms a candidate (drop, don't guess).
    fn resolve_character_event(&self, tags: &[String], short_name: &str) -> Option<String> {
        let short = short_name.trim();
        if short.is_empty() {
            return None;
        }
        // (a) Direct ATL trigger — resolves through the ATL → event pipeline.
        if self.has_trigger(short) {
            return Some(short.to_owned());
        }
        // (b) Template-then-validate against the event-id catalog.
        let (type_token, typed_rest) = split_audio_type_prefix(short);
        for tag in tags {
            let default = format!("Play_SFX_{tag}_{short}");
            if let Some(name) = self.canonical_event_name(&default) {
                return Some(name);
            }
            if let (Some(type_token), Some(rest)) = (type_token, typed_rest.as_deref()) {
                let typed = format!("Play_{type_token}_{tag}_{rest}");
                if let Some(name) = self.canonical_event_name(&typed) {
                    return Some(name);
                }
            }
        }
        None
    }

    /// The Wwise switch/state value name(s) an ATL switch definition maps a
    /// `(group, state)` pair to (e.g. `SurfaceType`/`metal` → `WwiseValue`
    /// `metal`). These are the strings the engine FNV-hashes into the switch id
    /// stored in a `CAkSwitchCntr` branch, so a caller validates the hash against
    /// the authored branch ids. Empty when no ATL switch defines the pair.
    fn wwise_switch_state_names(&self, group: &str, state: &str) -> Vec<String> {
        let mut names = Vec::new();
        for control in &self.controls {
            for switch in control.switches.iter().chain(&control.states) {
                if !switch.name.eq_ignore_ascii_case(group) {
                    continue;
                }
                for switch_state in &switch.states {
                    if !switch_state.name.eq_ignore_ascii_case(state) {
                        continue;
                    }
                    for request in &switch_state.requests {
                        collect_wwise_value_names(request, &mut names);
                    }
                }
            }
        }
        names
    }

    /// Banks the trigger-bank map associates with an event name. A single record
    /// carries the event-defining and media-owning bank crcs alongside the
    /// event's `AZ::Crc32`, so the resolved banks are exactly the record's crc
    /// fields that name a known preload bank.
    fn map_banks(&self, event_name: &str) -> Vec<String> {
        let event = u64::from(cry_audio::AudioControlId::from_name(event_name).0);
        let mut banks = Vec::new();
        for entry in &self.trigger_bank_map {
            let fields = [
                u64::from(entry.bank_id.0),
                entry.control_ids[0].0,
                entry.control_ids[1].0,
                entry.control_ids[2].0,
            ];
            if !fields.contains(&event) {
                continue;
            }
            for field in fields {
                if let Ok(field) = u32::try_from(field) {
                    if let Some(path) = self.crc_to_bank.get(&field) {
                        push_unique_path(&mut banks, path);
                    }
                }
            }
        }
        banks
    }

    /// Fallback for events the trigger-bank map does not cover: the preload
    /// group whose banks include one whose HIRC defines the event id.
    fn preload_banks_defining_event(&self, source: &dyn AssetSource, event_id: u32) -> Vec<String> {
        let event = cry_audio::WwiseObjectId(event_id);
        for group in &self.preload_groups {
            let defines = group.iter().any(|path| {
                source
                    .read(path)
                    .and_then(|bytes| cry_audio::WwiseSoundBank::parse(&bytes).ok())
                    .is_some_and(|bank| bank.defines_event(event))
            });
            if defines {
                return group.clone();
            }
        }
        Vec::new()
    }
}

/// Bank sets per ATL preload request / config group, as catalog paths.
fn preload_bank_groups(control: &cry_audio::AudioControlsSource) -> Vec<Vec<String>> {
    control
        .preloads
        .iter()
        .map(|preload| {
            let mut banks = Vec::new();
            for file in preload
                .files
                .iter()
                .chain(preload.config_groups.iter().flat_map(|group| &group.files))
            {
                push_unique_path(&mut banks, &preload_bank_path(&file.wwise_name));
            }
            banks
        })
        .filter(|banks| !banks.is_empty())
        .collect()
}

fn preload_bank_path(wwise_name: &str) -> String {
    let name = normalize_path(wwise_name);
    if name.contains('/') {
        name
    } else {
        format!("sounds/wwise/{name}")
    }
}

/// The bank-name stem the `AZ::Crc32` catalog keys hash: the basename with the
/// `.bnk` extension removed.
fn bank_stem(path: &str) -> String {
    let base = path.rsplit('/').next().unwrap_or(path);
    base.strip_suffix(".bnk")
        .unwrap_or(base)
        .to_ascii_lowercase()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model_asset::tests::{ContextSource, EmptySource};

    #[test]
    fn audio_trigger_resolution_is_catalog_driven_not_name_shaped() {
        let controls = cry_audio::AudioControlsSource::from_xml(
            "libs/gameaudio/wwise/atl_controls.xml",
            r#"<ATLConfig atl_name="main">
                 <AudioTriggers>
                   <ATLTrigger atl_name="blend_ftsp_alligator">
                     <WwiseEvent wwise_name="blend_ftsp_alligator"/>
                   </ATLTrigger>
                 </AudioTriggers>
               </ATLConfig>"#,
        )
        .unwrap();
        let mut event_ids = std::collections::HashMap::new();
        event_ids.insert("Play_BareEvent".to_owned(), 4242u32);
        let catalogs = AudioCatalogs {
            controls: vec![controls],
            trigger_bank_map: Vec::new(),
            crc_to_bank: std::collections::HashMap::new(),
            preload_groups: Vec::new(),
            event_ids,
        };

        // A parameter that matches no ATL trigger and no event-id table entry is
        // dropped — even though its name shape looks exactly like a footstep
        // blend trigger, there is no prefix acceptance.
        assert!(
            resolve_one_audio_trigger(&EmptySource, &catalogs, "blend_ftsp_unlisted", false)
                .unwrap()
                .is_none()
        );

        // Resolves via the ATL trigger → Wwise event.
        let atl = resolve_one_audio_trigger(&EmptySource, &catalogs, "blend_ftsp_alligator", false)
            .unwrap()
            .expect("ATL trigger resolves");
        assert_eq!(atl.wwise_events.len(), 1);
        assert_eq!(atl.wwise_events[0].name, "blend_ftsp_alligator");

        // Resolves as a bare Wwise event name present in the event-id table.
        let bare = resolve_one_audio_trigger(&EmptySource, &catalogs, "Play_BareEvent", false)
            .unwrap()
            .expect("bare event name resolves");
        assert_eq!(bare.wwise_events[0].id, Some(4242));
    }

    #[test]
    fn audio_event_kind_distinguishes_footstep_from_direct_and_ignores_others() {
        let footstep = cry_animation::AnimationEvent {
            name: "footstep".into(),
            name_lowercase_crc32: 0,
            normalized_time: 0.5,
            normalized_end_time: 0.5,
            parameter: "blend_ftsp_alligator".into(),
            bone: String::new(),
            second_bone: String::new(),
            offset: [0.0; 3],
            direction: [0.0; 3],
            model: String::new(),
            source: cry_xml::XmlElement {
                name: "event".into(),
                attributes: Default::default(),
                children: Vec::new(),
                text: String::new(),
            },
        };
        assert_eq!(audio_event_kind(&footstep), Some(true));
        let direct = cry_animation::AnimationEvent {
            name: "sound".into(),
            ..footstep.clone()
        };
        assert_eq!(audio_event_kind(&direct), Some(false));
        let unrelated = cry_animation::AnimationEvent {
            name: "hit".into(),
            parameter: String::new(),
            ..footstep.clone()
        };
        assert_eq!(audio_event_kind(&unrelated), None);
    }

    fn empty_catalogs() -> AudioCatalogs {
        AudioCatalogs {
            controls: Vec::new(),
            trigger_bank_map: Vec::new(),
            crc_to_bank: std::collections::HashMap::new(),
            preload_groups: Vec::new(),
            event_ids: std::collections::HashMap::new(),
        }
    }

    fn metal_fxlib() -> cry_audio::MaterialEffectsLibrary {
        cry_audio::MaterialEffectsLibrary::from_xml(
            "libs/materialeffects/fxlibs/blend_ftsp_test.xml",
            r#"<FXLib type="playerfootstep">
                 <Effect name="metal">
                   <Audio trigger="t"><Switch name="SurfaceType" state="metal"/></Audio>
                 </Effect>
               </FXLib>"#,
        )
        .unwrap()
    }

    #[test]
    fn surface_name_resolves_only_when_the_hash_validates() {
        let library = metal_fxlib();
        let catalogs = empty_catalogs();
        // The FX-library surface state "metal" hashes to the branch switch id →
        // the branch is tagged.
        let metal_id = cry_audio::WwiseNameId::from_name("metal").0;
        assert_eq!(
            resolve_surface_name(metal_id, Some(&library), &catalogs).as_deref(),
            Some("metal")
        );
        // A switch id that no surface state hash matches (the alligator's real
        // creature switch) resolves to nothing — kept by id only, never guessed.
        assert_eq!(
            resolve_surface_name(0xDEAD_BEEF, Some(&library), &catalogs),
            None
        );
    }

    #[test]
    fn build_surface_media_marks_default_and_keeps_unresolved_branches_by_id() {
        let branches = vec![
            SurfaceBranch {
                switch_id: cry_audio::WwiseNameId::from_name("metal").0,
                is_default: true,
                media: [10u32, 20].into_iter().collect(),
                sequence: vec![20, 10, 20],
            },
            SurfaceBranch {
                switch_id: 999,
                is_default: false,
                media: [30u32].into_iter().collect(),
                sequence: Vec::new(),
            },
        ];
        let library = metal_fxlib();
        let catalogs = empty_catalogs();
        let default_media: std::collections::BTreeSet<u32> = [10, 20].into_iter().collect();

        let out =
            build_surface_media(branches, &default_media, Some(&library), &catalogs, "blend_ftsp_test");
        assert_eq!(out.len(), 2);
        // Default branch: resolved surface, media pool, weighted sequence.
        assert_eq!(out[0].surface.as_deref(), Some("metal"));
        assert!(out[0].default);
        assert_eq!(out[0].media, vec![10, 20]);
        assert_eq!(out[0].sequence, vec![20, 10, 20]);
        // Unresolved branch: kept by id, no surface, no sequence.
        assert_eq!(out[1].surface, None);
        assert!(!out[1].default);
        assert_eq!(out[1].switch_id, 999);
        assert!(out[1].sequence.is_empty());
    }

    /// A catalog seeded with the real `npc_alligator_events.csv` event names,
    /// including the mixed `Alligator`/`Aligator` spellings the shipped file uses.
    fn alligator_event_catalogs() -> AudioCatalogs {
        let mut event_ids = std::collections::HashMap::new();
        for (name, id) in [
            ("Play_SFX_Alligator_Bite", 3440898348u32),
            ("Play_VOX_Alligator_Attack1", 1858679537),
            ("Play_VOX_Alligator_Attack2", 1858679538),
            ("Play_VOX_Alligator_Chatters", 2839370678),
            ("Play_VOX_Alligator_Alert", 4122091654),
            ("Play_VOX_Alligator_Hurt", 441453735),
            ("Play_VOX_Alligator_Death", 1651525330),
            ("Play_SFX_Aligator_Tail_Whip_Fast", 566522816),
            ("Play_SFX_Aligator_Tail_Swipe", 2977758565),
            ("Play_Alligator_Breathing", 2750234764),
        ] {
            event_ids.insert(name.to_owned(), id);
        }
        AudioCatalogs {
            controls: Vec::new(),
            trigger_bank_map: Vec::new(),
            crc_to_bank: std::collections::HashMap::new(),
            preload_groups: Vec::new(),
            event_ids,
        }
    }

    #[test]
    fn character_tag_is_derived_from_event_names_most_frequent_first() {
        let catalogs = alligator_event_catalogs();
        let tags = catalogs.character_audio_tags();
        // `Alligator` (7) outnumbers the misspelled `Aligator` (2), so it leads;
        // both are present so either spelling's events can be validated.
        assert_eq!(tags.first().map(String::as_str), Some("Alligator"));
        assert!(tags.iter().any(|tag| tag == "Aligator"));
    }

    #[test]
    fn character_event_resolves_short_name_only_against_the_catalog() {
        let catalogs = alligator_event_catalogs();
        let tags = catalogs.character_audio_tags();

        // Bare name → default SFX template, validated against the CSV.
        assert_eq!(
            catalogs
                .resolve_character_event(&tags, "Bite")
                .as_deref(),
            Some("Play_SFX_Alligator_Bite")
        );
        // `VOX_`-prefixed name → typed template `Play_VOX_<Tag>_<rest>` (the SFX
        // default `Play_SFX_Alligator_VOX_Attack1` is not in the catalog).
        assert_eq!(
            catalogs
                .resolve_character_event(&tags, "VOX_Attack1")
                .as_deref(),
            Some("Play_VOX_Alligator_Attack1")
        );
        // A name whose only catalog spelling is the misspelled tag still resolves
        // because both tags are tried and validated.
        assert_eq!(
            catalogs
                .resolve_character_event(&tags, "Tail_Swipe")
                .as_deref(),
            Some("Play_SFX_Aligator_Tail_Swipe")
        );
        // A short name no template + catalog combination confirms is dropped, not
        // guessed.
        assert_eq!(catalogs.resolve_character_event(&tags, "Nonexistent_Growl"), None);
    }

    #[test]
    fn audio_catalogs_builds_event_ids_from_typed_mapping() {
        // The event-id table is built by re-parsing each AudioMapping CSV with
        // the typed `cry_audio::parse_audio_mapping`, not by probing the stored
        // `serde_json::Value` document (which is left `Null` here on purpose).
        let path = "sounds/wwise/npc_alligator_events.csv";
        let source =
            ContextSource::default().with(path, b"Name,Id\nPlay_Alligator,7\nStop_Alligator,9\n");
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
        resolved
            .extras
            .source_assets
            .push(nw_model::CrySourceAsset {
                path: path.to_owned(),
                kind: nw_model::CrySourceAssetKind::AudioMapping,
                document: serde_json::Value::Null,
            });

        let catalogs = AudioCatalogs::load(&source, &resolved).unwrap();
        assert_eq!(catalogs.event_id("Play_Alligator"), 7);
        assert_eq!(catalogs.event_id("Stop_Alligator"), 9);
        assert!(catalogs.knows_event_name("play_alligator"));
    }
}
