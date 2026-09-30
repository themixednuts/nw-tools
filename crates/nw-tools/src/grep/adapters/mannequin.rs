//! Mannequin content matching: fragment groups, variant tags, and animation
//! names at full recall, via the streaming visitor (no DOM build).

use std::path::Path;

use super::super::adapter::{
    ContentHit, EntryView, FactDraft, FactKind, QuerySet, SearchAdapter, Tier,
};
use crate::fuzzy;

/// Matches `.adb` animation databases.
pub struct MannequinAdapter;

impl SearchAdapter for MannequinAdapter {
    fn name(&self) -> &'static str {
        "mannequin"
    }

    fn tier(&self) -> Tier {
        Tier::Content
    }

    fn matches_entry(&self, entry_name: &str) -> bool {
        cry_mannequin::is_legacy_mannequin_source(entry_name)
            || cry_mannequin::is_animation_database_path(Path::new(entry_name))
    }

    fn search(&self, entry: &EntryView<'_>, queries: &QuerySet) -> Result<Vec<ContentHit>, String> {
        let Some(bytes) = entry.bytes.as_ref() else {
            return Err("missing bytes".to_string());
        };
        let lowered_terms: Vec<String> = queries
            .terms
            .iter()
            .map(|term| term.to_ascii_lowercase())
            .collect();
        let mut fuzzy = (!queries.exact).then(|| fuzzy::MultiSearch::new(&lowered_terms));
        let mut collector = HitCollector::default();
        cry_mannequin::visit_animation_database(bytes, |item| {
            collector.visit(item, &mut fuzzy, &lowered_terms);
            Ok(())
        })
        .map_err(|_| "undecodable".to_string())?;
        Ok(collector.hits)
    }

    fn facts(&self, entry: &EntryView<'_>) -> Result<Vec<FactDraft>, String> {
        let Some(bytes) = entry.bytes.as_ref() else {
            return Err("missing bytes".to_string());
        };
        if let Some(kind) = cry_mannequin::MannequinXmlKind::from_source_path(entry.name) {
            if kind != cry_mannequin::MannequinXmlKind::AnimationDatabase {
                return xml_facts(kind, entry.name, bytes);
            }
        }
        let mut facts = Vec::new();
        let mut group = String::new();
        cry_mannequin::visit_animation_database(bytes, |item| {
            match item {
                cry_mannequin::AnimationDatabaseItem::FragmentGroup(fragment_group) => {
                    group = fragment_group.name.into_owned();
                    if !group.is_empty() {
                        facts.push(FactDraft {
                            field: String::new(),
                            value: group.clone(),
                            kind: FactKind::Fragment,
                            loc_a: 0,
                            loc_b: 0,
                        });
                    }
                }
                cry_mannequin::AnimationDatabaseItem::Fragment(fragment) => {
                    for tags in [fragment.tags, fragment.fragment_tags]
                        .into_iter()
                        .flatten()
                    {
                        let value = tags.into_owned();
                        if value.is_empty() {
                            continue;
                        }
                        facts.push(FactDraft {
                            field: group.clone(),
                            value,
                            kind: FactKind::Tag,
                            loc_a: 0,
                            loc_b: 0,
                        });
                    }
                }
                cry_mannequin::AnimationDatabaseItem::Animation(animation) => {
                    let value = animation.name.into_owned();
                    if !value.is_empty() {
                        facts.push(FactDraft {
                            field: String::new(),
                            value,
                            kind: FactKind::Animation,
                            loc_a: 0,
                            loc_b: 0,
                        });
                    }
                }
                _ => {}
            }
            Ok(())
        })
        .map_err(|_| "undecodable".to_string())?;
        Ok(facts)
    }
}

/// The fragment group enclosing the items visited so far.
#[derive(Default)]
struct HitCollector {
    group: String,
    hits: Vec<ContentHit>,
}

impl HitCollector {
    fn visit(
        &mut self,
        item: cry_mannequin::AnimationDatabaseItem<'_>,
        fuzzy: &mut Option<fuzzy::MultiSearch>,
        lowered_terms: &[String],
    ) {
        match item {
            cry_mannequin::AnimationDatabaseItem::FragmentGroup(group) => {
                self.group = group.name.into_owned();
                self.score_into(
                    format!("fragment {}", self.group),
                    self.group.clone(),
                    fuzzy,
                    lowered_terms,
                );
            }
            cry_mannequin::AnimationDatabaseItem::Fragment(fragment) => {
                for tags in [fragment.tags, fragment.fragment_tags]
                    .into_iter()
                    .flatten()
                {
                    self.score_into(
                        format!("fragment {} tags", self.group),
                        tags.into_owned(),
                        fuzzy,
                        lowered_terms,
                    );
                }
            }
            cry_mannequin::AnimationDatabaseItem::Animation(animation) => {
                let name = animation.name.into_owned();
                self.score_into(format!("animation {name}"), name, fuzzy, lowered_terms);
            }
            _ => {}
        }
    }

    fn score_into(
        &mut self,
        location: String,
        text: String,
        fuzzy: &mut Option<fuzzy::MultiSearch>,
        lowered_terms: &[String],
    ) {
        let lowered = text.to_ascii_lowercase();
        let score = match fuzzy {
            Some(search) => search.score(lowered.as_str()),
            None => lowered_terms
                .iter()
                .any(|query| lowered.contains(query))
                .then_some(0),
        };
        if let Some(score) = score {
            self.hits.push(ContentHit {
                location,
                snippet: text,
                score,
            });
        }
    }
}

fn xml_facts(
    kind: cry_mannequin::MannequinXmlKind,
    name: &str,
    bytes: &[u8],
) -> Result<Vec<FactDraft>, String> {
    match kind {
        cry_mannequin::MannequinXmlKind::AnimationDatabase => Err("adb is not xml".to_string()),
        cry_mannequin::MannequinXmlKind::Actions | cry_mannequin::MannequinXmlKind::Tags => {
            let source = cry_mannequin::MannequinTagDefinitionSource::from_legacy(name, bytes)
                .map_err(|_| "undecodable".to_string())?;
            Ok(source
                .entries
                .into_iter()
                .flat_map(|entry| match entry {
                    cry_mannequin::MannequinTagDefinitionEntry::Tag(tag) => {
                        vec![tag_fact(String::new(), tag.name)]
                    }
                    cry_mannequin::MannequinTagDefinitionEntry::Group(group) => {
                        let mut facts = vec![tag_fact(String::new(), group.name.clone())];
                        facts.extend(
                            group
                                .tags
                                .into_iter()
                                .map(|tag| tag_fact(group.name.clone(), tag.name)),
                        );
                        facts
                    }
                })
                .flatten()
                .collect())
        }
        cry_mannequin::MannequinXmlKind::ControllerDefinition => {
            let source =
                cry_mannequin::MannequinControllerDefinitionSource::from_legacy(name, bytes)
                    .map_err(|_| "undecodable".to_string())?;
            Ok(source
                .fragment_definitions
                .into_iter()
                .filter(|fragment| !fragment.name.is_empty())
                .map(|fragment| FactDraft {
                    field: String::new(),
                    value: fragment.name,
                    kind: FactKind::Fragment,
                    loc_a: 0,
                    loc_b: 0,
                })
                .collect())
        }
    }
}

fn tag_fact(field: String, value: String) -> Option<FactDraft> {
    if value.is_empty() {
        return None;
    }
    Some(FactDraft {
        field,
        value,
        kind: FactKind::Tag,
        loc_a: 0,
        loc_b: 0,
    })
}
