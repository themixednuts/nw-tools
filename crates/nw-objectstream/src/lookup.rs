//! Optional lookup tables that pretty-print element names + fields.
//!
//! ObjectStream payloads carry only UUIDs (type) and CRC-32 hashes
//! (field name). Resolving them back to human-readable strings requires
//! optional lookup data supplied by the caller.
//!
//! Callers that do not have lookup data pass `None` to
//! [`crate::ObjectStream::from_reader`] and the read still works; names
//! stay empty or unresolved.
//!
//! # Why `ArcStr`?
//!
//! ObjectStream payloads have ~500 unique type names and ~1000
//! unique field names duplicated across millions of elements.
//! Storing them as [`arcstr::ArcStr`] in this lookup means every
//! [`Element`](crate::Element) clone is an atomic refcount bump
//! instead of a heap allocation + memcpy, measured around 10x memory
//! reduction on a 1 M-element parse.

use std::collections::{HashMap, HashSet};

use arcstr::ArcStr;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use thiserror::Error;
use uuid::Uuid;

use crate::type_uuid::type_ids;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TypeEntry {
    name: &'static str,
    type_id: Uuid,
    canonical_type_id: Option<Uuid>,
}

impl TypeEntry {
    #[must_use]
    pub const fn new(name: &'static str, type_id: Uuid) -> Self {
        Self {
            name,
            type_id,
            canonical_type_id: None,
        }
    }

    #[must_use]
    pub const fn alias(name: &'static str, type_id: Uuid, canonical_type_id: Uuid) -> Self {
        Self {
            name,
            type_id,
            canonical_type_id: Some(canonical_type_id),
        }
    }

    #[must_use]
    pub const fn name(self) -> &'static str {
        self.name
    }

    #[must_use]
    pub const fn type_id(self) -> Uuid {
        self.type_id
    }

    #[must_use]
    pub const fn canonical_type_id(self) -> Option<Uuid> {
        self.canonical_type_id
    }
}

/// ObjectStream-specific type names and wire aliases.
///
/// These are parser-facing names for XML or binary ObjectStream ids that
/// either predate the canonical folded id or use a friendlier stream name
/// than the generic type formula.
pub const OBJECT_STREAM_TYPES: &[TypeEntry] = &[
    TypeEntry::alias(
        "AZStd::string",
        type_ids::AZSTD_STRING_XML_ALIAS,
        type_ids::AZSTD_STRING,
    ),
    TypeEntry::alias(
        "AZStd::vector",
        type_ids::AZSTD_VECTOR_XML_ALIAS,
        type_ids::AZSTD_VECTOR,
    ),
    TypeEntry::new("ByteStream", type_ids::BYTE_STREAM),
    TypeEntry::new("SliceComponent", crate::types::SLICE_COMPONENT),
];

#[derive(Debug, Default, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct NameLookup {
    /// Type UUID to class name.
    pub uuids: HashMap<Uuid, ArcStr>,
    /// Field-name CRC-32 to field-name string.
    pub crcs: HashMap<u32, ArcStr>,
}

impl NameLookup {
    /// Construct an empty table. Equivalent to [`Default::default`].
    #[inline]
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Pre-allocate space for `uuid_capacity` UUID entries and
    /// `crc_capacity` CRC entries. Useful when building from a
    /// known-size dump.
    #[inline]
    #[must_use]
    pub fn with_capacity(uuid_capacity: usize, crc_capacity: usize) -> Self {
        Self {
            uuids: HashMap::with_capacity(uuid_capacity),
            crcs: HashMap::with_capacity(crc_capacity),
        }
    }

    #[inline]
    #[must_use]
    pub fn with_uuids(mut self, uuids: HashMap<Uuid, ArcStr>) -> Self {
        self.uuids = uuids;
        self
    }

    #[inline]
    #[must_use]
    pub fn with_crcs(mut self, crcs: HashMap<u32, ArcStr>) -> Self {
        self.crcs = crcs;
        self
    }

    /// Build type UUID and field CRC lookup tables from a SerializeContext
    /// JSON snapshot.
    ///
    /// The snapshot shape uses `$id`/`$ref` pairs heavily, so this walks
    /// through references and extracts the two surfaces ObjectStream needs:
    /// class `typeId` names and reflected element `nameCrc` names.
    pub fn from_serialize_json(bytes: &[u8]) -> Result<Self, SerializeLookupError> {
        let root = serde_json::from_slice(bytes)?;
        Ok(Self::from_serialize_value(&root))
    }

    /// Build type UUID and field CRC lookup tables from a parsed
    /// SerializeContext JSON value.
    #[must_use]
    pub fn from_serialize_value(root: &Value) -> Self {
        let mut lookup = Self::new();
        lookup.extend_types(OBJECT_STREAM_TYPES);

        let refs = JsonRefs::new(root);
        let mut walker = SerializeWalker {
            lookup: &mut lookup,
            refs: &refs,
            seen_refs: HashSet::new(),
        };
        walker.visit(root, 0);
        lookup
    }

    /// Insert or replace one type-name mapping.
    #[inline]
    pub fn insert_type_name(&mut self, uuid: Uuid, name: impl AsRef<str>) -> Option<ArcStr> {
        self.uuids.insert(uuid, ArcStr::from(name.as_ref()))
    }

    /// Insert or replace one static AZ type entry.
    #[inline]
    pub fn insert_type(&mut self, entry: TypeEntry) -> Option<ArcStr> {
        self.insert_type_name(entry.type_id(), entry.name())
    }

    /// Add fallback type-name mappings without replacing names already loaded
    /// from caller-supplied lookup data.
    pub fn extend_type_names<'a>(&mut self, type_names: impl IntoIterator<Item = (Uuid, &'a str)>) {
        for (uuid, name) in type_names {
            self.uuids.entry(uuid).or_insert_with(|| ArcStr::from(name));
        }
    }

    /// Add fallback type entries without replacing names already loaded from
    /// caller-supplied lookup data.
    pub fn extend_types<'a>(&mut self, type_entries: impl IntoIterator<Item = &'a TypeEntry>) {
        for entry in type_entries {
            self.uuids
                .entry(entry.type_id())
                .or_insert_with(|| ArcStr::from(entry.name()));
        }
    }

    #[inline]
    #[must_use]
    pub fn uuid_count(&self) -> usize {
        self.uuids.len()
    }

    #[inline]
    #[must_use]
    pub fn crc_count(&self) -> usize {
        self.crcs.len()
    }

    /// Look up a class name by AZ type UUID. The returned
    /// [`ArcStr`] is cheap to clone (refcount bump).
    #[inline]
    #[must_use]
    pub fn type_name(&self, uuid: &Uuid) -> Option<&ArcStr> {
        self.uuids.get(uuid)
    }

    /// Look up a field name by CRC-32 hash.
    #[inline]
    #[must_use]
    pub fn field_name(&self, crc: u32) -> Option<&ArcStr> {
        self.crcs.get(&crc)
    }
}

#[derive(Debug, Error)]
pub enum SerializeLookupError {
    #[error("parse serialize context JSON: {0}")]
    Json(#[from] serde_json::Error),
}

struct JsonRefs<'a> {
    by_id: HashMap<u64, &'a Value>,
}

impl<'a> JsonRefs<'a> {
    fn new(root: &'a Value) -> Self {
        let mut refs = Self {
            by_id: HashMap::new(),
        };
        refs.index(root);
        refs
    }

    fn index(&mut self, value: &'a Value) {
        match value {
            Value::Object(object) => {
                if let Some(id) = object.get("$id").and_then(Value::as_u64) {
                    self.by_id.entry(id).or_insert(value);
                }
                for value in object.values() {
                    self.index(value);
                }
            }
            Value::Array(values) => {
                for value in values {
                    self.index(value);
                }
            }
            Value::Null | Value::Bool(_) | Value::Number(_) | Value::String(_) => {}
        }
    }

    fn get(&self, reference: &str) -> Option<&'a Value> {
        let id = reference.strip_prefix('#')?.parse::<u64>().ok()?;
        self.by_id.get(&id).copied()
    }
}

struct SerializeWalker<'a, 'lookup> {
    lookup: &'lookup mut NameLookup,
    refs: &'a JsonRefs<'a>,
    seen_refs: HashSet<u64>,
}

impl SerializeWalker<'_, '_> {
    fn visit(&mut self, value: &Value, depth: usize) {
        const MAX_DEPTH: usize = 128;
        if depth > MAX_DEPTH {
            return;
        }

        match value {
            Value::Object(object) => {
                if let Some(reference) = object.get("$ref").and_then(Value::as_str) {
                    if let Some(id) = reference.strip_prefix('#').and_then(|id| id.parse().ok())
                        && self.seen_refs.insert(id)
                    {
                        if let Some(target) = self.refs.get(reference) {
                            self.visit(target, depth + 1);
                        }
                        self.seen_refs.remove(&id);
                    }
                    return;
                }

                if let (Some(type_id), Some(name)) = (
                    object.get("typeId").and_then(value_uuid),
                    object
                        .get("name")
                        .and_then(Value::as_str)
                        .filter(|name| !name.is_empty())
                        .or_else(|| {
                            object
                                .get("typeName")
                                .and_then(Value::as_str)
                                .filter(|name| !name.is_empty())
                        }),
                ) {
                    self.lookup.insert_type_name(type_id, name);
                }

                if let (Some(name_crc), Some(name)) = (
                    object.get("nameCrc").and_then(value_u32),
                    object
                        .get("name")
                        .and_then(Value::as_str)
                        .filter(|name| !name.is_empty()),
                ) {
                    self.lookup
                        .crcs
                        .entry(name_crc)
                        .or_insert_with(|| ArcStr::from(name));
                }

                for value in object.values() {
                    self.visit(value, depth + 1);
                }
            }
            Value::Array(values) => {
                for value in values {
                    self.visit(value, depth + 1);
                }
            }
            Value::Null | Value::Bool(_) | Value::Number(_) | Value::String(_) => {}
        }
    }
}

fn value_u32(value: &Value) -> Option<u32> {
    value.as_u64().and_then(|value| u32::try_from(value).ok())
}

fn value_uuid(value: &Value) -> Option<Uuid> {
    Uuid::parse_str(value.as_str()?).ok()
}

impl FromIterator<(Uuid, ArcStr)> for NameLookup {
    fn from_iter<I: IntoIterator<Item = (Uuid, ArcStr)>>(iter: I) -> Self {
        Self {
            uuids: iter.into_iter().collect(),
            crcs: HashMap::new(),
        }
    }
}

impl FromIterator<(u32, ArcStr)> for NameLookup {
    fn from_iter<I: IntoIterator<Item = (u32, ArcStr)>>(iter: I) -> Self {
        Self {
            uuids: HashMap::new(),
            crcs: iter.into_iter().collect(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn objectstream_type_aliases_seed_lookup_names() {
        let mut hashes = NameLookup::new();
        hashes.extend_types(OBJECT_STREAM_TYPES);

        assert_eq!(
            hashes
                .type_name(&type_ids::AZSTD_STRING_XML_ALIAS)
                .map(ArcStr::as_str),
            Some("AZStd::string")
        );
        assert_eq!(
            hashes
                .type_name(&type_ids::AZSTD_VECTOR_XML_ALIAS)
                .map(ArcStr::as_str),
            Some("AZStd::vector")
        );
        assert_eq!(
            hashes.type_name(&type_ids::BYTE_STREAM).map(ArcStr::as_str),
            Some("ByteStream")
        );
    }

    #[test]
    fn type_entries_do_not_replace_runtime_names() {
        let mut hashes = NameLookup::new();
        hashes.insert_type_name(type_ids::AZSTD_STRING_XML_ALIAS, "RuntimeName");
        hashes.extend_types(OBJECT_STREAM_TYPES);

        assert_eq!(
            hashes
                .type_name(&type_ids::AZSTD_STRING_XML_ALIAS)
                .map(ArcStr::as_str),
            Some("RuntimeName")
        );
    }

    #[test]
    fn serialize_json_lookup_reads_type_and_field_names() {
        let root = serde_json::json!({
            "$id": 1,
            "uuidMap": {
                "AAAAAAAA-AAAA-4AAA-AAAA-AAAAAAAAAAAA": {
                    "$id": 2,
                    "name": "Example",
                    "typeId": "AAAAAAAA-AAAA-4AAA-AAAA-AAAAAAAAAAAA",
                    "elements": [
                        {
                            "name": "m_enabled",
                            "nameCrc": 1234,
                            "typeId": "72B9409A-7D1A-4831-9CFE-FCB3FADD3426"
                        }
                    ]
                },
                "BBBBBBBB-BBBB-4BBB-BBBB-BBBBBBBBBBBB": { "$ref": "#2" }
            }
        });

        let hashes = NameLookup::from_serialize_value(&root);
        let uuid = Uuid::parse_str("AAAAAAAA-AAAA-4AAA-AAAA-AAAAAAAAAAAA").unwrap();

        assert_eq!(hashes.type_name(&uuid).map(ArcStr::as_str), Some("Example"));
        assert_eq!(
            hashes.field_name(1234).map(ArcStr::as_str),
            Some("m_enabled")
        );
    }
}
