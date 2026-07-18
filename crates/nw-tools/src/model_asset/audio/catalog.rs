//! Authored ATL, preload, event-id, and audio-tag catalogs.

use super::*;

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
    if reference.kind == cry_audio::AudioBackendReferenceKind::WwiseEvent
        && let Some(name) = reference
            .wwise_name
            .as_deref()
            .or(reference.atl_name.as_deref())
    {
        let name = name.trim();
        if !name.is_empty() {
            out.push(name.to_owned());
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
    for child in &reference.children {
        collect_wwise_value_names(child, out);
    }
}

/// The authored audio catalogs, parsed once per export to drive trigger
/// resolution without any name/stem matching.
pub(super) struct AudioCatalogs {
    /// Every loaded ATL control document (discovery + `--audio`).
    pub(super) controls: Vec<cry_audio::AudioControlsSource>,
    /// Wwise trigger-bank map entries (empty when the install ships none).
    pub(super) trigger_bank_map: Vec<cry_audio::WwiseTriggerBankMapEntry>,
    /// `AZ::Crc32(bank stem)` → catalog bank path, over the whole preload
    /// catalog. Lets the trigger-bank map's crc fields resolve to bank paths.
    pub(super) crc_to_bank: std::collections::HashMap<u32, String>,
    /// Preload config-group bank sets, for the fallback bank lookup. A group
    /// bundles an event bank with its media bank(s), so the whole group is the
    /// candidate set once one member defines the event.
    pub(super) preload_groups: Vec<Vec<String>>,
    /// Wwise event name → id, from the shipped event-id mapping CSVs.
    pub(super) event_ids: std::collections::BTreeMap<String, u32>,
    /// Case-folded `AZ::Crc32(ValidTag)` → exact authored spellings from the
    /// global audio tag table. Collisions remain explicit alternatives.
    pub(super) audio_tags: std::collections::BTreeMap<u32, Vec<String>>,
}

impl AudioCatalogs {
    pub(super) fn load(source: &dyn AssetSource, resolved: &ResolvedAsset) -> Result<Self> {
        let mut controls = Vec::new();
        for asset in &resolved.extras.source_assets {
            if matches!(asset.kind, nw_model::CrySourceAssetKind::AudioControls)
                && let Some(document) = parse_audio_controls_document(source, &asset.path)?
            {
                controls.push(document);
            }
        }

        controls.sort_by(|left, right| {
            left.source_path
                .to_ascii_lowercase()
                .cmp(&right.source_path.to_ascii_lowercase())
        });

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

        let mut event_ids = std::collections::BTreeMap::new();
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

        let mut audio_tags = std::collections::BTreeMap::<u32, Vec<String>>::new();
        if let Some(bytes) = source.read(AUDIO_TAG_DATA_PATH) {
            let cry_audio::AudioMappingDocument::Tags(tags) =
                cry_audio::parse_audio_mapping(AUDIO_TAG_DATA_PATH, &bytes)
                    .context("parse global audio tag data")?
            else {
                unreachable!("audio tag path selects the tag mapping schema");
            };
            for entry in tags.entries {
                let valid_tag = entry.valid_tag.trim();
                if valid_tag.is_empty() {
                    continue;
                }
                let crc = cry_audio::az_crc32(valid_tag.as_bytes());
                let spellings = audio_tags.entry(crc).or_default();
                if !spellings.iter().any(|tag| tag == valid_tag) {
                    spellings.push(valid_tag.to_owned());
                }
            }
            for spellings in audio_tags.values_mut() {
                spellings.sort_by(|left, right| left.as_bytes().cmp(right.as_bytes()));
            }
        }

        Ok(Self {
            controls,
            trigger_bank_map,
            crc_to_bank,
            preload_groups,
            event_ids,
            audio_tags,
        })
    }

    /// The Wwise event name(s) an ATL trigger fans out to, plus its playback
    /// info. Empty when the parameter is not an authored ATL trigger.
    pub(super) fn trigger_events(
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
            if playback.is_none()
                && let Some(info) = &atl.playback_info
            {
                playback = Some(nw_model::AudioTriggerPlayback {
                    max_radius: info.max_radius,
                    max_duration: info.max_duration,
                });
            }
        }
        (events, playback)
    }

    /// The Wwise object id for an event name: the authored mapping-CSV id when
    /// present, else the FNV-1 hash the engine derives from the name.
    pub(super) fn event_id(&self, name: &str) -> u32 {
        self.event_ids
            .iter()
            .find(|(candidate, _)| candidate.eq_ignore_ascii_case(name))
            .map(|(_, id)| *id)
            .unwrap_or_else(|| cry_audio::WwiseNameId::from_name(name).0)
    }

    /// The event-id catalog's canonical spelling of an event name (its exact CSV
    /// casing), matched case-insensitively, or `None` when no event matches.
    pub(super) fn canonical_event_name(&self, name: &str) -> Option<String> {
        self.event_ids
            .keys()
            .find(|candidate| candidate.eq_ignore_ascii_case(name))
            .cloned()
    }

    /// Canonical spelling of an authored ATL trigger. This is the first exact
    /// resolution hop for receiver-generated controls.
    fn canonical_trigger(&self, name: &str) -> Option<String> {
        self.controls
            .iter()
            .flat_map(|control| &control.triggers)
            .find(|trigger| trigger.name.eq_ignore_ascii_case(name))
            .map(|trigger| trigger.name.clone())
    }

    /// The Wwise switch/state value name(s) an ATL switch definition maps a
    /// `(group, state)` pair to (e.g. `SurfaceType`/`metal` → `WwiseValue`
    /// `metal`). These are the strings the engine FNV-hashes into the switch id
    /// stored in a `CAkSwitchCntr` branch, so a caller validates the hash against
    /// the authored branch ids. Empty when no ATL switch defines the pair.
    pub(super) fn wwise_switch_state_names(&self, group: &str, state: &str) -> Vec<String> {
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
    pub(super) fn map_banks(&self, event_name: &str) -> Vec<String> {
        let event = cry_audio::AudioControlId::from_name(event_name).0;
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
                if let Ok(field) = u32::try_from(field)
                    && let Some(path) = self.crc_to_bank.get(&field)
                {
                    push_unique_path(&mut banks, path);
                }
            }
        }
        banks
    }

    /// Fallback for events the trigger-bank map does not cover: the preload
    /// group whose banks include one whose HIRC defines the event id.
    ///
    /// Bank parses are served from `banks`, so each candidate bank is read and
    /// parsed at most once across the whole resolution — the scan touches only
    /// the light HIRC index, never a bank's raw media DATA.
    pub(super) fn preload_banks_defining_event(
        &self,
        banks: &mut BankStore,
        event_id: u32,
    ) -> Vec<String> {
        let event = cry_audio::WwiseObjectId(event_id);
        for group in &self.preload_groups {
            let defines = group.iter().any(|path| {
                banks
                    .parsed(path)
                    .is_some_and(|bank| bank.defines_event(event))
            });
            if defines {
                return group.clone();
            }
        }
        Vec::new()
    }
}

impl character_event::CharacterEventCatalogs for AudioCatalogs {
    fn valid_tags(&self, tag_crcs: &[u32]) -> Vec<character_event::ValidAudioTag> {
        tag_crcs
            .iter()
            .flat_map(|crc| {
                self.audio_tags.get(crc).into_iter().flatten().map(|name| {
                    character_event::ValidAudioTag {
                        name: name.clone(),
                        crc: *crc,
                    }
                })
            })
            .collect()
    }

    fn resolve_control(&self, candidate: &str) -> Option<String> {
        self.canonical_trigger(candidate)
            .or_else(|| self.canonical_event_name(candidate))
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
