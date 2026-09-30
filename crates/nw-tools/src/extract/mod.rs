//! Entry → facts. Claim is predicate-only; decode failure does not fall through.

mod xml;

use std::path::Path;
use std::sync::OnceLock;

use crate::grep::adapter::{EntryView, FactDraft, FactKind, SearchAdapter, Tier};
use crate::grep::adapters;
use crate::index::EntryFormat;

/// Why content facts are missing for a claimed entry.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SkipReason {
    Unreadable,
    Undecodable,
}

/// Facts for one entry, including the FileName fact.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Extracted {
    pub format: EntryFormat,
    pub facts: Vec<FactDraft>,
    pub skip: Option<SkipReason>,
}

/// Extract from bytes already in hand (session dumps, tests, bundled ingest).
#[must_use]
pub fn bytes(name: &str, data: &[u8]) -> Extracted {
    apply(name, first_claim(name), Some(data))
}

/// Extract one pak entry. `pull` runs at most once, and only after a content claim.
#[must_use]
pub fn entry(name: &str, pull: impl FnOnce() -> Option<Vec<u8>>) -> Extracted {
    match first_claim(name) {
        None => name_only(name),
        Some(claim) => match pull() {
            None => skipped(name, claim.format(), SkipReason::Unreadable),
            Some(data) => apply(name, Some(claim), Some(&data)),
        },
    }
}

enum Claim {
    Adapter(&'static dyn SearchAdapter),
    LeftoverXml,
}

impl Claim {
    fn format(&self) -> EntryFormat {
        match self {
            Self::Adapter(adapter) => EntryFormat::from_adapter(adapter.name()),
            Self::LeftoverXml => EntryFormat::Xml,
        }
    }
}

fn apply(name: &str, claim: Option<Claim>, data: Option<&[u8]>) -> Extracted {
    let mut facts = path_facts(name);
    let Some(claim) = claim else {
        return Extracted {
            format: EntryFormat::Unknown,
            facts,
            skip: None,
        };
    };
    let format = claim.format();
    let Some(data) = data else {
        return Extracted {
            format,
            facts,
            skip: Some(SkipReason::Unreadable),
        };
    };
    match claim_facts(&claim, name, data) {
        Ok(content) => {
            facts.extend(content.into_iter().filter(|fact| !fact.value.is_empty()));
            Extracted {
                format,
                facts,
                skip: None,
            }
        }
        Err(_) => Extracted {
            format,
            facts,
            skip: Some(SkipReason::Undecodable),
        },
    }
}

fn claim_facts(claim: &Claim, name: &str, data: &[u8]) -> Result<Vec<FactDraft>, String> {
    match claim {
        Claim::LeftoverXml => xml::leftover_xml_facts(data),
        Claim::Adapter(adapter) => adapter.facts(&EntryView {
            pak: "",
            name,
            bytes: Some(data.to_vec()),
        }),
    }
}

fn first_claim(name: &str) -> Option<Claim> {
    for adapter in content_adapters() {
        if adapter.matches_entry(name) {
            return Some(Claim::Adapter(adapter.as_ref()));
        }
    }
    is_xml(name).then_some(Claim::LeftoverXml)
}

fn content_adapters() -> &'static [Box<dyn SearchAdapter>] {
    static ADAPTERS: OnceLock<Vec<Box<dyn SearchAdapter>>> = OnceLock::new();
    ADAPTERS.get_or_init(|| {
        adapters::all()
            .into_iter()
            .filter(|adapter| adapter.tier() == Tier::Content)
            .collect()
    })
}

fn is_xml(name: &str) -> bool {
    Path::new(name)
        .extension()
        .and_then(|ext| ext.to_str())
        .is_some_and(|ext| ext.eq_ignore_ascii_case("xml"))
}

fn name_only(name: &str) -> Extracted {
    Extracted {
        format: EntryFormat::Unknown,
        facts: path_facts(name),
        skip: None,
    }
}

fn skipped(name: &str, format: EntryFormat, reason: SkipReason) -> Extracted {
    Extracted {
        format,
        facts: path_facts(name),
        skip: Some(reason),
    }
}

/// Full archive path, basename, and stem — each as a FileName fact so CRC
/// lookup finds `items.datasheet` and `items`, not only the full path.
#[must_use]
pub fn path_name_facts(name: &str) -> Vec<FactDraft> {
    let mut facts: Vec<FactDraft> = Vec::with_capacity(3);
    let mut push = |value: &str| {
        if value.is_empty() || facts.iter().any(|fact| fact.value == value) {
            return;
        }
        facts.push(FactDraft {
            field: String::new(),
            value: value.to_owned(),
            kind: FactKind::FileName,
            loc_a: 0,
            loc_b: 0,
        });
    };
    let normalized = name.replace('\\', "/");
    push(&normalized);
    let base = normalized.rsplit('/').next().unwrap_or(normalized.as_str());
    push(base);
    if let Some((stem, ext)) = base.rsplit_once('.')
        && !ext.is_empty()
        && !stem.is_empty()
    {
        push(stem);
    }
    facts
}

fn path_facts(name: &str) -> Vec<FactDraft> {
    path_name_facts(name)
}
