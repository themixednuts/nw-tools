//! CAF clip names and `.animevents` event names.

use std::path::Path;

use cry_animation::AnimationClip;

use super::super::adapter::{
    ContentHit, EntryView, FactDraft, FactKind, QuerySet, SearchAdapter, Tier,
};
use super::score_drafts;

/// Matches `.caf` / `.i_caf` / `.animevents`.
pub struct AnimationAdapter;

impl SearchAdapter for AnimationAdapter {
    fn name(&self) -> &'static str {
        "animation"
    }

    fn tier(&self) -> Tier {
        Tier::Content
    }

    fn matches_entry(&self, entry_name: &str) -> bool {
        cry_animation::is_animation_path(Path::new(entry_name)) || is_animevents(entry_name)
    }

    fn search(&self, entry: &EntryView<'_>, queries: &QuerySet) -> Result<Vec<ContentHit>, String> {
        Ok(score_drafts(&self.facts(entry)?, queries))
    }

    fn facts(&self, entry: &EntryView<'_>) -> Result<Vec<FactDraft>, String> {
        let bytes = entry
            .bytes
            .as_ref()
            .ok_or_else(|| "missing bytes".to_string())?;
        if is_animevents(entry.name) {
            return event_facts(bytes);
        }
        let clip =
            AnimationClip::parse(entry.name, bytes).map_err(|_| "undecodable".to_string())?;
        let mut facts = Vec::new();
        if !clip.name.is_empty() {
            facts.push(FactDraft {
                field: String::new(),
                value: clip.name,
                kind: FactKind::Animation,
                loc_a: 0,
                loc_b: 0,
            });
        }
        if let Some(path) = clip.caf.header.file_path {
            if !path.is_empty() {
                facts.push(FactDraft {
                    field: "file_path".into(),
                    value: path,
                    kind: FactKind::Animation,
                    loc_a: 0,
                    loc_b: 0,
                });
            }
        }
        Ok(facts)
    }
}

fn is_animevents(name: &str) -> bool {
    Path::new(name)
        .extension()
        .and_then(|ext| ext.to_str())
        .is_some_and(|ext| ext.eq_ignore_ascii_case("animevents"))
}

fn event_facts(bytes: &[u8]) -> Result<Vec<FactDraft>, String> {
    let xml = std::str::from_utf8(bytes).map_err(|_| "undecodable".to_string())?;
    let database = cry_animation::AnimationEventDatabase::from_xml(xml)
        .map_err(|_| "undecodable".to_string())?;
    let mut facts = Vec::new();
    for list in &database.animations {
        if !list.animation_path.is_empty() {
            facts.push(FactDraft {
                field: "clip".into(),
                value: list.animation_path.clone(),
                kind: FactKind::AnimEvent,
                loc_a: 0,
                loc_b: 0,
            });
        }
        for event in &list.events {
            if !event.name.is_empty() {
                facts.push(FactDraft {
                    field: list.animation_path.clone(),
                    value: event.name.clone(),
                    kind: FactKind::AnimEvent,
                    loc_a: 0,
                    loc_b: 0,
                });
            }
            if !event.parameter.is_empty() {
                facts.push(FactDraft {
                    field: event.name.clone(),
                    value: event.parameter.clone(),
                    kind: FactKind::AnimEvent,
                    loc_a: 0,
                    loc_b: 0,
                });
            }
        }
    }
    Ok(facts)
}
