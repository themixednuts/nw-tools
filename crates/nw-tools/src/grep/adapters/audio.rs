//! Wwise mapping CSVs and ATL control XML names.

use super::super::adapter::{
    ContentHit, EntryView, FactDraft, FactKind, QuerySet, SearchAdapter, Tier,
};
use super::score_drafts;

const ATL_PATHS: &[&str] = &[
    "libs/gameaudio/wwise/atl_controls.xml",
    "libs/gameaudio/wwise/preloaddata.xml",
    "libs/gameaudio/wwise/default_controls.xml",
];

/// Matches audio mapping CSVs and the known ATL XML tables.
pub struct AudioAdapter;

impl SearchAdapter for AudioAdapter {
    fn name(&self) -> &'static str {
        "audio"
    }

    fn tier(&self) -> Tier {
        Tier::Content
    }

    fn matches_entry(&self, entry_name: &str) -> bool {
        cry_audio::is_audio_mapping_source(entry_name) || is_atl(entry_name)
    }

    fn search(&self, entry: &EntryView<'_>, queries: &QuerySet) -> Result<Vec<ContentHit>, String> {
        Ok(score_drafts(&self.facts(entry)?, queries))
    }

    fn facts(&self, entry: &EntryView<'_>) -> Result<Vec<FactDraft>, String> {
        let bytes = entry
            .bytes
            .as_ref()
            .ok_or_else(|| "missing bytes".to_string())?;
        if cry_audio::is_audio_mapping_source(entry.name) {
            return mapping_facts(entry.name, bytes);
        }
        atl_facts(entry.name, bytes)
    }
}

fn is_atl(name: &str) -> bool {
    let lowered = name.replace('\\', "/").to_ascii_lowercase();
    ATL_PATHS.iter().any(|path| lowered.ends_with(path))
}

fn mapping_facts(path: &str, bytes: &[u8]) -> Result<Vec<FactDraft>, String> {
    let document =
        cry_audio::parse_audio_mapping(path, bytes).map_err(|_| "undecodable".to_string())?;
    let mut facts = Vec::new();
    match document {
        cry_audio::AudioMappingDocument::EventIds(table) => {
            for (index, event) in table.events.iter().enumerate() {
                push_cell(&mut facts, index as u32, "Name", &event.name);
            }
        }
        cry_audio::AudioMappingDocument::TractSoundBanks(table) => {
            for (index, row) in table.entries.iter().enumerate() {
                push_cell(&mut facts, index as u32, "tract", &row.tract);
                push_cell(&mut facts, index as u32, "soundbank", &row.sound_bank);
            }
        }
        cry_audio::AudioMappingDocument::Tags(table) => {
            for (index, row) in table.entries.iter().enumerate() {
                push_cell(&mut facts, index as u32, "ValidTag", &row.valid_tag);
            }
        }
    }
    Ok(facts)
}

fn atl_facts(path: &str, bytes: &[u8]) -> Result<Vec<FactDraft>, String> {
    let xml = std::str::from_utf8(bytes).map_err(|_| "undecodable".to_string())?;
    let source = cry_audio::AudioControlsSource::from_xml(path, xml)
        .map_err(|_| "undecodable".to_string())?;
    let mut facts = Vec::new();
    for trigger in &source.triggers {
        push_name(&mut facts, "trigger", &trigger.name);
    }
    for rtpc in &source.rtpcs {
        push_name(&mut facts, "rtpc", &rtpc.name);
    }
    for switch in &source.switches {
        push_name(&mut facts, "switch", &switch.name);
    }
    for environment in &source.environments {
        push_name(&mut facts, "environment", &environment.name);
    }
    Ok(facts)
}

fn push_cell(facts: &mut Vec<FactDraft>, loc_a: u32, field: &str, value: &str) {
    if value.is_empty() {
        return;
    }
    facts.push(FactDraft {
        field: field.to_owned(),
        value: value.to_owned(),
        kind: FactKind::Cell,
        loc_a,
        loc_b: 0,
    });
}

fn push_name(facts: &mut Vec<FactDraft>, field: &str, value: &str) {
    if value.is_empty() {
        return;
    }
    facts.push(FactDraft {
        field: field.to_owned(),
        value: value.to_owned(),
        kind: FactKind::AudioName,
        loc_a: 0,
        loc_b: 0,
    });
}
