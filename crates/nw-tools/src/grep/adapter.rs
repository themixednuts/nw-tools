//! The global-find adapter port: one trait every searchable format implements.
//!
//! Adapters own their entry predicate, decoding, and text extraction. Decode
//! failure is reported as a skip reason and never propagates — provider error
//! types stay below this seam.

/// Name tier sorts above content tier (picker convention).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tier {
    Name = 0,
    Content = 1,
}

/// What a stored fact is pointing at. Integer in `idx_fact.kind`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(i64)]
pub enum FactKind {
    /// Archive entry path.
    FileName = 0,
    /// Datasheet cell (`loc_a` = row, `loc_b` = column).
    Cell = 1,
    /// Mannequin fragment group name.
    Fragment = 2,
    /// Mannequin variant tags (`field` = enclosing group).
    Tag = 3,
    /// Mannequin or CAF animation clip name.
    Animation = 4,
    /// ObjectStream string leaf (`field` = property name).
    ObjectValue = 5,
    /// ObjectStream field or type name stored as the value.
    ObjectField = 6,
    /// Catalog row (`field` = path / asset-id / type, `loc_a` = entry).
    Catalog = 7,
    /// Decompiled lua source line (`loc_a` = 1-based line).
    LuaLine = 8,
    /// Audio control or mapping name (`field` = kind).
    AudioName = 9,
    /// Animation event name.
    AnimEvent = 10,
    /// Reflection JSON string leaf (`field` = parent key).
    JsonString = 11,
    /// Pak text / config line (`loc_a` = 1-based line).
    TextLine = 12,
    /// Leftover XML node. `field` is the element path (`Root/Item/@id`).
    Xml = 13,
}

impl FactKind {
    /// Reconstruct from the persisted integer. Unknown values are [`Self::FileName`].
    #[must_use]
    pub fn from_i64(value: i64) -> Self {
        match value {
            1 => Self::Cell,
            2 => Self::Fragment,
            3 => Self::Tag,
            4 => Self::Animation,
            5 => Self::ObjectValue,
            6 => Self::ObjectField,
            7 => Self::Catalog,
            8 => Self::LuaLine,
            9 => Self::AudioName,
            10 => Self::AnimEvent,
            11 => Self::JsonString,
            12 => Self::TextLine,
            13 => Self::Xml,
            _ => Self::FileName,
        }
    }
}

/// One extracted `(field, value)` before interning.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FactDraft {
    /// Column, property, or enclosing group name. Empty for a bare name.
    pub field: String,
    /// The searchable text.
    pub value: String,
    /// How to reconstruct a location string.
    pub kind: FactKind,
    /// Row, fragment ordinal, or unused.
    pub loc_a: u32,
    /// Column, or unused.
    pub loc_b: u32,
}

/// Query terms, OR-combined.
#[derive(Debug, Clone)]
pub struct QuerySet {
    /// Raw query terms.
    pub terms: Vec<String>,
    /// Exact substring match instead of fuzzy ranking.
    pub exact: bool,
}

impl QuerySet {
    /// Build a set from CLI query args.
    #[must_use]
    pub fn new(terms: &[String], exact: bool) -> Self {
        Self {
            terms: terms.to_vec(),
            exact,
        }
    }
}

/// One matched piece of entry content.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContentHit {
    /// Where inside the entry (`row 0 · column Name`, …).
    pub location: String,
    /// Matched text.
    pub snippet: String,
    /// Fuzzy match score (0 in exact mode).
    pub score: u16,
}

/// An entry handed to content adapters. Bytes are owned so no lifetimes cross
/// worker threads; `None` only when routing never claimed the entry.
pub struct EntryView<'a> {
    /// Pak archive holding the entry, relative to the scan root.
    pub pak: &'a str,
    /// Archive path of the entry.
    pub name: &'a str,
    /// Decoded entry bytes.
    pub bytes: Option<Vec<u8>>,
}

/// One searchable format. Implementations are pure matching over the given
/// bytes: no I/O, no panics on hostile input — corruption is a skip reason.
pub trait SearchAdapter: Send + Sync {
    /// Format key shown in rows (`datasheet`, …).
    fn name(&self) -> &'static str;
    /// Which tier this adapter's hits merge into.
    fn tier(&self) -> Tier;
    /// Whether this adapter wants the entry. Name-only: must never read bytes.
    fn matches_entry(&self, entry_name: &str) -> bool;
    /// Match `queries` against the entry. `Err` is the skip reason.
    fn search(&self, entry: &EntryView<'_>, queries: &QuerySet) -> Result<Vec<ContentHit>, String>;
    /// Every indexable `(field, value)` in the entry. Decode failure is a skip.
    fn facts(&self, entry: &EntryView<'_>) -> Result<Vec<FactDraft>, String>;
}
