//! High-level streaming queries over ObjectStream payloads.
//!
//! Built on top of [`crate::visit`] — wraps the boilerplate of a
//! custom visitor struct in two ergonomic helpers:
//!
//! - [`find_first`] — walks until the predicate hits, then stops
//!   via [`VisitFlow::Stop`]. O(N) in the worst case but typically
//!   sub-tree-bounded.
//! - [`find_all`] — walks the entire tree, collecting every match.
//!
//! Both return owned [`FoundElement`] copies so the result outlives
//! the borrowed-data scratch buffer the visitor sees.

use std::collections::BTreeMap;
use std::convert::Infallible;
use std::fmt::Write as _;
use std::io;

use arcstr::ArcStr;
use uuid::Uuid;

use crate::ObjectStreamError;
use crate::lookup::NameLookup;
use crate::value::{self, ElementValue, child_by_field};
use crate::visit::{
    ElementHeader, ElementVisitor, VisitFlow, parse_streaming, parse_streaming_bytes,
};
use crate::{Element, ObjectStream, StreamTag, types};

/// Owned snapshot of an element header — the data the streaming
/// visitor sees borrowed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FoundElement {
    pub flags: u8,
    pub name_crc: Option<u32>,
    pub version: Option<u8>,
    pub id: Uuid,
    pub specialization: Option<Uuid>,
    pub name: Option<ArcStr>,
    pub field: Option<ArcStr>,
    pub data: Vec<u8>,
}

impl FoundElement {
    fn from_header(h: &ElementHeader<'_>) -> Self {
        Self {
            flags: h.flags,
            name_crc: h.name_crc,
            version: h.version,
            id: h.id,
            specialization: h.specialization,
            name: h.name.cloned(),
            field: h.field.cloned(),
            data: h.data.to_vec(),
        }
    }
}

/// ObjectStream surface used by search tools.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum ObjectStreamSearchKind {
    Path,
    Type,
    Field,
    TypeId,
    SpecializationId,
    FieldCrc,
    Flags,
    Version,
    DataLength,
    Value,
    RawUtf8,
    RawHex,
    Parent,
}

impl ObjectStreamSearchKind {
    #[inline]
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Path => "path",
            Self::Type => "type",
            Self::Field => "field",
            Self::TypeId => "type-id",
            Self::SpecializationId => "specialization-id",
            Self::FieldCrc => "field-crc",
            Self::Flags => "flags",
            Self::Version => "version",
            Self::DataLength => "data-len",
            Self::Value => "value",
            Self::RawUtf8 => "raw-utf8",
            Self::RawHex => "raw-hex",
            Self::Parent => "parent",
        }
    }
}

/// One matched ObjectStream path, name, UUID, field CRC, value, or raw payload view.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct ObjectStreamSearchHit {
    pub kind: ObjectStreamSearchKind,
    pub value: String,
}

/// Aggregate hit count and best caller-provided score for a match.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ObjectStreamMatchStats {
    pub count: u64,
    pub score: u32,
}

/// Collect every searchable ObjectStream surface whose text is
/// accepted by `score_match`.
///
/// Binary input uses the streaming visitor and borrows element value
/// data while parsing. XML and JSON inputs normalize through the DOM
/// first because their text values must be decoded before traversal.
pub fn collect_search_matches<F>(
    bytes: &[u8],
    hashes: Option<&NameLookup>,
    score_match: F,
) -> Result<BTreeMap<ObjectStreamSearchHit, ObjectStreamMatchStats>, ObjectStreamError>
where
    F: FnMut(&str) -> Option<u32>,
{
    let Some((&tag, _)) = bytes.split_first() else {
        return Err(ObjectStreamError::Io(io::Error::new(
            io::ErrorKind::UnexpectedEof,
            "empty ObjectStream payload",
        )));
    };

    let mut visitor = ObjectStreamSearchVisitor {
        score_match,
        hits: BTreeMap::new(),
        path: Vec::new(),
        next_child_indices: vec![0],
        ancestors: Vec::new(),
    };
    match StreamTag::from_byte(tag) {
        Some(StreamTag::BINARY) => {
            parse_streaming_bytes(bytes, hashes, &mut visitor)?;
        }
        Some(StreamTag::XML | StreamTag::JSON) => {
            let stream = ObjectStream::from_bytes(bytes, hashes)?;
            visitor.record_dom_roots(stream.elements());
        }
        _ => return Err(ObjectStreamError::InvalidStreamTag(tag)),
    }
    Ok(visitor.hits)
}

/// Find a facet-like child by serialized field name and AZ class UUID.
///
/// New World component ObjectStreams often store client/server facet
/// payloads on derived classes while the serialized field lives on a
/// `BaseClass1` child. This helper checks direct children first, then
/// follows that AZ base-class chain.
#[must_use]
pub fn facet_by_field<'a>(element: &'a Element, field: &str, id: Uuid) -> Option<&'a Element> {
    let mut base = None;
    for child in element.children() {
        match child.field().map(ArcStr::as_str) {
            Some(actual) if actual == field && child.id() == &id => return Some(child),
            Some("BaseClass1") => base = Some(child),
            _ => {}
        }
    }
    base.and_then(|base| facet_by_field(base, field, id))
}

/// Find a reflected client facet by AZ class UUID.
#[must_use]
pub fn client_facet(element: &Element, id: Uuid) -> Option<&Element> {
    facet_by_field(element, "m_clientFacetPtr", id)
}

/// Find a reflected server facet by AZ class UUID.
#[must_use]
pub fn server_facet(element: &Element, id: Uuid) -> Option<&Element> {
    facet_by_field(element, "m_serverFacetPtr", id)
}

/// Find a reflected server facet by AZ class UUID, accepting derived
/// facet payloads whose `BaseClass1` chain contains the requested
/// server facet type.
///
/// New World component ObjectStreams sometimes serialize a derived
/// server facet under `m_serverFacetPtr` while common fields live on a
/// base facet child. This helper keeps that traversal in the
/// ObjectStream query Module.
#[must_use]
pub fn server_facet_or_base_class(element: &Element, id: Uuid) -> Option<&Element> {
    facet_or_base_class_by_field(element, "m_serverFacetPtr", id)
}

fn facet_or_base_class_by_field<'a>(
    element: &'a Element,
    field: &str,
    id: Uuid,
) -> Option<&'a Element> {
    let mut base = None;
    for child in element.children() {
        match child.field().map(ArcStr::as_str) {
            Some(actual) if actual == field => {
                if let Some(facet) = base_class_of_type(child, id) {
                    return Some(facet);
                }
            }
            Some("BaseClass1") => base = Some(child),
            _ => {}
        }
    }
    base.and_then(|base| facet_or_base_class_by_field(base, field, id))
}

/// Find a serialized child field on `element`, falling back to the
/// same field on the reflected server facet.
///
/// Some New World component ObjectStreams serialize fields directly on
/// the component in older captures and under `m_serverFacetPtr` in
/// newer reflected payloads. This helper keeps that layout choice in
/// the ObjectStream query Module instead of each caller hand-rolling
/// the same lookup.
#[must_use]
pub fn child_or_server_facet_child<'a>(
    element: &'a Element,
    field: &str,
    server_facet_id: Uuid,
) -> Option<&'a Element> {
    child_by_field(element, field).or_else(|| {
        server_facet(element, server_facet_id).and_then(|facet| child_by_field(facet, field))
    })
}

/// Find a serialized child field on `element`, falling back to the
/// same field on an untyped client facet.
///
/// Some reflected ObjectStreams wrap client facet payloads directly on
/// `element`, while others put the `m_clientFacetPtr` child one level
/// under another reflected wrapper. Use this when the caller only
/// needs the payload layout and does not have a stable facet UUID to
/// validate against.
#[must_use]
pub fn child_or_nested_client_facet_child<'a>(
    element: &'a Element,
    field: &str,
) -> Option<&'a Element> {
    child_by_field(element, field)
        .or_else(|| nested_client_facet(element).and_then(|facet| child_by_field(facet, field)))
}

fn nested_client_facet(element: &Element) -> Option<&Element> {
    child_by_field(element, "m_clientFacetPtr").or_else(|| {
        element
            .children()
            .iter()
            .find_map(|child| child_by_field(child, "m_clientFacetPtr"))
    })
}

/// Find an element with `id`, following only AZ `BaseClass1` children.
///
/// This is useful when a reflected derived object stores its common
/// fields on a base-class child and the caller needs to decode those
/// fields through the base type's schema.
#[must_use]
#[inline]
pub fn base_class_of_type(element: &Element, id: Uuid) -> Option<&Element> {
    if element.id() == &id {
        return Some(element);
    }

    for child in element.children() {
        if child
            .field()
            .is_some_and(|field| field.as_str() == "BaseClass1")
            && let Some(found) = base_class_of_type(child, id)
        {
            return Some(found);
        }
    }

    None
}

/// Resolve an AZ `AZStd::shared_ptr<T>` wrapper to its serialized pointee.
///
/// If `element` is not a shared pointer, or if the shared pointer has no
/// child payload, the original element is returned.
#[inline]
#[must_use]
pub fn pointee(element: &Element) -> &Element {
    if element.id() == &types::AZSTD_SHARED_PTR {
        element.children().first().unwrap_or(element)
    } else {
        element
    }
}

/// Resolve a pointee and return the first immediate child with `expected`
/// when the pointee itself is a wrapper/object of another type.
///
/// Falls back to the resolved pointee when no matching child is present.
#[must_use]
pub fn pointee_of_type<'a>(element: &'a Element, expected: &Uuid) -> &'a Element {
    let element = pointee(element);
    if element.id() == expected {
        return element;
    }
    element
        .children()
        .iter()
        .find(|child| child.id() == expected)
        .unwrap_or(element)
}

/// Human-readable ObjectStream type label for diagnostics and
/// dynamic-class metadata.
///
/// Returns the resolved AZ class name when the element was parsed with
/// a serialize-context hash table. Falls back to the class UUID when
/// no name was resolved.
#[must_use]
pub fn type_name(element: &Element) -> String {
    let name = element.name().as_str();
    if name.is_empty() {
        element.id().to_string()
    } else {
        name.to_string()
    }
}

/// Iterate every reflected `AZ::Entity` under a set of DOM roots.
///
/// This is the common ObjectStream traversal used by slice-like source
/// assets. It keeps callers from owning the stack-walk and `AZ::Entity`
/// type UUID directly.
#[must_use]
pub fn az_entity_elements(roots: &[Element]) -> AzEntityElements<'_> {
    AzEntityElements::new(roots)
}

/// Iterate reflected `AZ::Entity` values while omitting entire subtrees
/// whose serialized field name matches `skip_fields`.
///
/// Scene readers use this for embedded shape payloads that happen to
/// contain entity-shaped data but are not scene entities.
#[must_use]
pub fn az_entity_elements_skipping_fields<'a>(
    roots: &'a [Element],
    skip_fields: &'static [&'static str],
) -> AzEntityElements<'a> {
    AzEntityElements::new(roots).with_skip_fields(skip_fields)
}

/// Depth-first iterator over reflected `AZ::Entity` elements.
#[derive(Debug)]
pub struct AzEntityElements<'a> {
    stack: Vec<std::slice::Iter<'a, Element>>,
    skip_fields: &'static [&'static str],
}

impl<'a> AzEntityElements<'a> {
    #[must_use]
    pub fn new(roots: &'a [Element]) -> Self {
        Self {
            stack: vec![roots.iter()],
            skip_fields: &[],
        }
    }

    #[must_use]
    pub const fn with_skip_fields(mut self, skip_fields: &'static [&'static str]) -> Self {
        self.skip_fields = skip_fields;
        self
    }

    #[inline]
    fn should_skip_subtree(&self, element: &Element) -> bool {
        let Some(field) = element.field() else {
            return false;
        };
        self.skip_fields.iter().any(|skip| field.as_str() == *skip)
    }
}

impl<'a> Iterator for AzEntityElements<'a> {
    type Item = &'a Element;

    fn next(&mut self) -> Option<Self::Item> {
        while let Some(top) = self.stack.last_mut() {
            let Some(element) = top.next() else {
                self.stack.pop();
                continue;
            };
            if self.should_skip_subtree(element) {
                continue;
            }
            if !element.children().is_empty() {
                self.stack.push(element.children().iter());
            }
            if element.id() == &types::AZ_ENTITY {
                return Some(element);
            }
        }
        None
    }
}

/// Find the first element matching `predicate` in a binary
/// ObjectStream. Returns `Ok(None)` if no element matches.
///
/// The predicate is `FnMut` so it can carry state (e.g. counters)
/// across visits.
pub fn find_first<F>(
    bytes: &[u8],
    hashes: Option<&NameLookup>,
    predicate: F,
) -> Result<Option<FoundElement>, ObjectStreamError>
where
    F: FnMut(&ElementHeader<'_>) -> bool,
{
    let mut visitor = FindFirst {
        predicate,
        found: None,
    };
    parse_streaming_bytes(bytes, hashes, &mut visitor)?;
    Ok(visitor.found)
}

/// Find every element matching `predicate`. Walks the full tree
/// (no early termination).
pub fn find_all<F>(
    bytes: &[u8],
    hashes: Option<&NameLookup>,
    predicate: F,
) -> Result<Vec<FoundElement>, ObjectStreamError>
where
    F: FnMut(&ElementHeader<'_>) -> bool,
{
    let mut visitor = FindAll {
        predicate,
        results: Vec::new(),
    };
    parse_streaming_bytes(bytes, hashes, &mut visitor)?;
    Ok(visitor.results)
}

/// Find the first matching element from any [`std::io::Read`]
/// source.
pub fn find_first_from<R, F>(
    reader: &mut R,
    hashes: Option<&NameLookup>,
    predicate: F,
) -> Result<Option<FoundElement>, ObjectStreamError>
where
    R: std::io::Read,
    F: FnMut(&ElementHeader<'_>) -> bool,
{
    let mut visitor = FindFirst {
        predicate,
        found: None,
    };
    parse_streaming(reader, hashes, &mut visitor)?;
    Ok(visitor.found)
}

/// Find every matching element from any [`std::io::Read`] source.
pub fn find_all_from<R, F>(
    reader: &mut R,
    hashes: Option<&NameLookup>,
    predicate: F,
) -> Result<Vec<FoundElement>, ObjectStreamError>
where
    R: std::io::Read,
    F: FnMut(&ElementHeader<'_>) -> bool,
{
    let mut visitor = FindAll {
        predicate,
        results: Vec::new(),
    };
    parse_streaming(reader, hashes, &mut visitor)?;
    Ok(visitor.results)
}

fn type_name_from_element(element: &Element) -> String {
    if element.name().is_empty() {
        element.id().to_string()
    } else {
        element.name().to_string()
    }
}

fn type_name_from_header(header: &ElementHeader<'_>) -> String {
    header
        .name
        .map_or_else(|| header.id.to_string(), ToString::to_string)
}

fn format_path(path: &[usize]) -> String {
    let mut out = String::new();
    for (index, value) in path.iter().enumerate() {
        if index > 0 {
            out.push('.');
        }
        let _ = write!(&mut out, "{value}");
    }
    out
}

// --- internal visitors ---

#[derive(Debug, Clone)]
struct AncestorSearchFrame {
    label: String,
}

impl AncestorSearchFrame {
    fn new(path: &str, type_name: &str, type_id: Uuid, field: Option<&str>) -> Self {
        let field = field
            .map(|field| format!(" field={field}"))
            .unwrap_or_default();
        Self {
            label: format!("{path} {type_name} id={type_id}{field}"),
        }
    }
}

struct ObjectStreamSearchVisitor<F> {
    score_match: F,
    hits: BTreeMap<ObjectStreamSearchHit, ObjectStreamMatchStats>,
    path: Vec<usize>,
    next_child_indices: Vec<usize>,
    ancestors: Vec<AncestorSearchFrame>,
}

impl<F> ObjectStreamSearchVisitor<F>
where
    F: FnMut(&str) -> Option<u32>,
{
    fn record_dom_roots(&mut self, roots: &[Element]) {
        for (index, element) in roots.iter().enumerate() {
            self.record_dom_element(element, index);
        }
    }

    fn record_dom_element(&mut self, element: &Element, index: usize) {
        self.path.push(index);
        self.record_element(element);
        let path = format_path(&self.path);
        let type_name = type_name_from_element(element);
        self.ancestors.push(AncestorSearchFrame::new(
            &path,
            &type_name,
            *element.id(),
            element.field().map(ArcStr::as_str),
        ));
        for (child_index, child) in element.children().iter().enumerate() {
            self.record_dom_element(child, child_index);
        }
        self.ancestors.pop();
        self.path.pop();
    }

    fn record_element(&mut self, element: &Element) {
        self.record_if_match(ObjectStreamSearchKind::Path, &format_path(&self.path));
        if !element.name().is_empty() {
            self.record_if_match(ObjectStreamSearchKind::Type, element.name().as_str());
        }
        if let Some(field) = element.field() {
            self.record_if_match(ObjectStreamSearchKind::Field, field.as_str());
        }
        self.record_if_match(ObjectStreamSearchKind::TypeId, &element.id().to_string());
        if let Some(specialization) = element.specialization() {
            self.record_if_match(
                ObjectStreamSearchKind::SpecializationId,
                &specialization.to_string(),
            );
        }
        if let Some(crc) = element.name_crc() {
            self.record_field_crc_if_match(crc);
        }
        if let Some(version) = element.version() {
            self.record_if_match(ObjectStreamSearchKind::Version, &version.to_string());
        }
        self.record_flags_if_match(element.flags);
        if let Some(data) = element.data() {
            self.record_payload_surfaces(element, data);
        }
        self.record_parent_matches();
    }

    fn record_header(&mut self, header: &ElementHeader<'_>) {
        self.record_if_match(ObjectStreamSearchKind::Path, &format_path(&self.path));
        if let Some(name) = header.name {
            self.record_if_match(ObjectStreamSearchKind::Type, name.as_str());
        }
        if let Some(field) = header.field {
            self.record_if_match(ObjectStreamSearchKind::Field, field.as_str());
        }
        self.record_if_match(ObjectStreamSearchKind::TypeId, &header.id.to_string());
        if let Some(specialization) = header.specialization {
            self.record_if_match(
                ObjectStreamSearchKind::SpecializationId,
                &specialization.to_string(),
            );
        }
        if let Some(crc) = header.name_crc {
            self.record_field_crc_if_match(crc);
        }
        if let Some(version) = header.version {
            self.record_if_match(ObjectStreamSearchKind::Version, &version.to_string());
        }
        self.record_flags_if_match(header.flags);
        if !header.data.is_empty() {
            self.record_payload_surfaces(header, header.data);
        }
        self.record_parent_matches();
    }

    fn record_payload_surfaces<E>(&mut self, element: &E, data: &[u8])
    where
        E: ElementValue + ?Sized,
    {
        self.record_if_match(ObjectStreamSearchKind::DataLength, &data.len().to_string());
        if let Some(value) = value::read_leaf_text(element) {
            self.record_if_match(ObjectStreamSearchKind::Value, &value);
        }
        if let Ok(text) = std::str::from_utf8(data)
            && !text.is_empty()
        {
            self.record_if_match(ObjectStreamSearchKind::RawUtf8, text);
        }
        self.record_hex_if_match(data);
    }

    fn record_if_match(&mut self, kind: ObjectStreamSearchKind, value: &str) {
        let Some(score) = (self.score_match)(value) else {
            return;
        };
        let stats = self
            .hits
            .entry(ObjectStreamSearchHit {
                kind,
                value: value.to_string(),
            })
            .or_default();
        stats.count += 1;
        stats.score = stats.score.max(score);
    }

    fn record_parent_matches(&mut self) {
        for index in 0..self.ancestors.len() {
            let score = {
                let label = &self.ancestors[index].label;
                (self.score_match)(label)
            };
            let Some(score) = score else {
                continue;
            };
            let stats = self
                .hits
                .entry(ObjectStreamSearchHit {
                    kind: ObjectStreamSearchKind::Parent,
                    value: self.ancestors[index].label.clone(),
                })
                .or_default();
            stats.count += 1;
            stats.score = stats.score.max(score);
        }
    }

    fn record_field_crc_if_match(&mut self, crc: u32) {
        let hex = format!("0x{crc:08x}");
        let decimal = crc.to_string();
        let score = if let Some(score) = (self.score_match)(&hex) {
            Some(score)
        } else {
            (self.score_match)(&decimal)
        };
        let Some(score) = score else {
            return;
        };
        let stats = self
            .hits
            .entry(ObjectStreamSearchHit {
                kind: ObjectStreamSearchKind::FieldCrc,
                value: format!("{hex}/{decimal}"),
            })
            .or_default();
        stats.count += 1;
        stats.score = stats.score.max(score);
    }

    fn record_flags_if_match(&mut self, flags: u8) {
        let hex = format!("0x{flags:02x}");
        let decimal = flags.to_string();
        let score = if let Some(score) = (self.score_match)(&hex) {
            Some(score)
        } else {
            (self.score_match)(&decimal)
        };
        let Some(score) = score else {
            return;
        };
        let stats = self
            .hits
            .entry(ObjectStreamSearchHit {
                kind: ObjectStreamSearchKind::Flags,
                value: format!("{hex}/{decimal}"),
            })
            .or_default();
        stats.count += 1;
        stats.score = stats.score.max(score);
    }

    fn record_hex_if_match(&mut self, data: &[u8]) {
        let spaced = value::format_hex_preview(data);
        let compact = if data.len() <= 96 {
            Some(hex::encode_upper(data))
        } else {
            None
        };
        let score = if let Some(score) = (self.score_match)(&spaced) {
            Some(score)
        } else if let Some(compact) = compact.as_deref() {
            (self.score_match)(compact)
        } else {
            None
        };
        let Some(score) = score else {
            return;
        };
        let value = if let Some(compact) = compact {
            format!("{spaced}/{compact}")
        } else {
            spaced
        };
        let stats = self
            .hits
            .entry(ObjectStreamSearchHit {
                kind: ObjectStreamSearchKind::RawHex,
                value,
            })
            .or_default();
        stats.count += 1;
        stats.score = stats.score.max(score);
    }
}

impl<F> ElementVisitor for ObjectStreamSearchVisitor<F>
where
    F: FnMut(&str) -> Option<u32>,
{
    type Error = ObjectStreamError;

    #[inline]
    fn open_element(&mut self, header: &ElementHeader<'_>) -> Result<VisitFlow, Self::Error> {
        let index = self
            .next_child_indices
            .last_mut()
            .expect("ObjectStream traversal always has a sibling counter");
        self.path.push(*index);
        *index += 1;
        self.record_header(header);
        let path = format_path(&self.path);
        let type_name = type_name_from_header(header);
        self.ancestors.push(AncestorSearchFrame::new(
            &path,
            &type_name,
            header.id,
            header.field.map(ArcStr::as_str),
        ));
        self.next_child_indices.push(0);
        Ok(VisitFlow::Continue)
    }

    #[inline]
    fn close_element(&mut self) -> Result<(), Self::Error> {
        self.next_child_indices.pop();
        self.ancestors.pop();
        self.path.pop();
        Ok(())
    }
}

struct FindFirst<F> {
    predicate: F,
    found: Option<FoundElement>,
}

impl<F: FnMut(&ElementHeader<'_>) -> bool> ElementVisitor for FindFirst<F> {
    type Error = ObjectStreamError;

    fn open_element(&mut self, header: &ElementHeader<'_>) -> Result<VisitFlow, Self::Error> {
        if (self.predicate)(header) {
            self.found = Some(FoundElement::from_header(header));
            Ok(VisitFlow::Stop)
        } else {
            Ok(VisitFlow::Continue)
        }
    }
}

struct FindAll<F> {
    predicate: F,
    results: Vec<FoundElement>,
}

impl<F: FnMut(&ElementHeader<'_>) -> bool> ElementVisitor for FindAll<F> {
    type Error = ObjectStreamError;

    fn open_element(&mut self, header: &ElementHeader<'_>) -> Result<VisitFlow, Self::Error> {
        if (self.predicate)(header) {
            self.results.push(FoundElement::from_header(header));
        }
        Ok(VisitFlow::Continue)
    }
}

// Reserved for a future infallible-visitor adapter.
const _: Option<Infallible> = None;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Element, ObjectStream, ST_BINARYFLAG_ELEMENT_HEADER, StreamTag};

    fn three_elements_with_ids(ids: &[u128]) -> Vec<u8> {
        let mut stream = ObjectStream {
            tag: StreamTag::BINARY,
            version: 3,
            elements: ids
                .iter()
                .map(|&id| Element {
                    flags: ST_BINARYFLAG_ELEMENT_HEADER,
                    id: Uuid::from_u128(id),
                    ..Default::default()
                })
                .collect(),
        };
        let mut buf = Vec::new();
        stream.elements.iter_mut().for_each(|e| {
            // (no-op — placeholder so we keep `mut`)
            let _ = e;
        });
        stream.write_to(&mut buf).expect("write");
        buf
    }

    #[test]
    fn find_first_stops_after_match() -> Result<(), ObjectStreamError> {
        let buf = three_elements_with_ids(&[1, 2, 3]);
        let target = Uuid::from_u128(2);
        let found = find_first(&buf, None, |h| h.id == target)?.expect("should find");
        assert_eq!(found.id, target);
        Ok(())
    }

    #[test]
    fn find_first_returns_none_on_miss() -> Result<(), ObjectStreamError> {
        let buf = three_elements_with_ids(&[1, 2, 3]);
        let result = find_first(&buf, None, |h| h.id == Uuid::from_u128(99))?;
        assert!(result.is_none());
        Ok(())
    }

    #[test]
    fn find_all_collects_every_match() -> Result<(), ObjectStreamError> {
        let buf = three_elements_with_ids(&[1, 2, 1, 3, 1]);
        let target = Uuid::from_u128(1);
        let hits = find_all(&buf, None, |h| h.id == target)?;
        assert_eq!(hits.len(), 3);
        assert!(hits.iter().all(|h| h.id == target));
        Ok(())
    }

    #[test]
    fn facet_by_field_follows_base_class_chain() {
        let target = Uuid::from_u128(0x20);
        let root = Element::new(Uuid::from_u128(0x01)).with_children(vec![
            Element::new(Uuid::from_u128(0x10))
                .with_field("BaseClass1")
                .with_children(vec![Element::new(target).with_field("m_serverFacetPtr")]),
        ]);

        let found = facet_by_field(&root, "m_serverFacetPtr", target).expect("facet");
        assert_eq!(found.id(), &target);
    }

    #[test]
    fn facet_by_field_prefers_direct_child() {
        let target = Uuid::from_u128(0x20);
        let direct = Element::new(target).with_field("m_serverFacetPtr");
        let base = Element::new(Uuid::from_u128(0x10))
            .with_field("BaseClass1")
            .with_children(vec![Element::new(target).with_field("m_serverFacetPtr")]);
        let root = Element::new(Uuid::from_u128(0x01)).with_children(vec![direct, base]);

        let found = facet_by_field(&root, "m_serverFacetPtr", target).expect("facet");
        assert_same_element(found, &root.children()[0]);
    }

    #[test]
    fn server_facet_uses_server_field() {
        let target = Uuid::from_u128(0x20);
        let root = Element::new(Uuid::from_u128(0x01)).with_children(vec![
            Element::new(Uuid::from_u128(0x10))
                .with_field("BaseClass1")
                .with_children(vec![Element::new(target).with_field("m_serverFacetPtr")]),
        ]);

        let found = server_facet(&root, target).expect("server facet");
        assert_eq!(found.id(), &target);
    }

    #[test]
    fn server_facet_or_base_class_returns_direct_server_facet() {
        let target = Uuid::from_u128(0x20);
        let root = Element::new(Uuid::from_u128(0x01))
            .with_children(vec![Element::new(target).with_field("m_serverFacetPtr")]);

        let found = server_facet_or_base_class(&root, target).expect("server facet");
        assert_eq!(found.id(), &target);
    }

    #[test]
    fn server_facet_or_base_class_returns_base_inside_derived_server_facet() {
        let target = Uuid::from_u128(0x20);
        let root = Element::new(Uuid::from_u128(0x01)).with_children(vec![
            Element::new(Uuid::from_u128(0x30))
                .with_field("m_serverFacetPtr")
                .with_children(vec![Element::new(target).with_field("BaseClass1")]),
        ]);

        let found = server_facet_or_base_class(&root, target).expect("base facet");
        assert_same_element(found, &root.children()[0].children()[0]);
    }

    #[test]
    fn server_facet_or_base_class_follows_component_base_class_chain() {
        let target = Uuid::from_u128(0x20);
        let root = Element::new(Uuid::from_u128(0x01)).with_children(vec![
            Element::new(Uuid::from_u128(0x10))
                .with_field("BaseClass1")
                .with_children(vec![
                    Element::new(Uuid::from_u128(0x30))
                        .with_field("m_serverFacetPtr")
                        .with_children(vec![Element::new(target).with_field("BaseClass1")]),
                ]),
        ]);

        let found = server_facet_or_base_class(&root, target).expect("base facet");
        assert_same_element(found, &root.children()[0].children()[0].children()[0]);
    }

    #[test]
    fn child_or_server_facet_child_prefers_direct_child() {
        let target = Uuid::from_u128(0x20);
        let root = Element::new(Uuid::from_u128(0x01)).with_children(vec![
            Element::new(Uuid::from_u128(0x30)).with_field("m_depthTest"),
            Element::new(target)
                .with_field("m_serverFacetPtr")
                .with_children(vec![
                    Element::new(Uuid::from_u128(0x31)).with_field("m_depthTest"),
                ]),
        ]);

        let found = child_or_server_facet_child(&root, "m_depthTest", target).expect("field");
        assert_same_element(found, &root.children()[0]);
    }

    #[test]
    fn child_or_server_facet_child_falls_back_to_server_facet() {
        let target = Uuid::from_u128(0x20);
        let root = Element::new(Uuid::from_u128(0x01)).with_children(vec![
            Element::new(target)
                .with_field("m_serverFacetPtr")
                .with_children(vec![
                    Element::new(Uuid::from_u128(0x31)).with_field("m_depthTest"),
                ]),
        ]);

        let found = child_or_server_facet_child(&root, "m_depthTest", target).expect("field");
        assert_same_element(found, &root.children()[0].children()[0]);
    }

    #[test]
    fn child_or_nested_client_facet_child_prefers_direct_child() {
        let root = Element::new(Uuid::from_u128(0x01)).with_children(vec![
            Element::new(Uuid::from_u128(0x30)).with_field("m_overrides"),
            Element::new(Uuid::from_u128(0x20))
                .with_field("m_clientFacetPtr")
                .with_children(vec![
                    Element::new(Uuid::from_u128(0x31)).with_field("m_overrides"),
                ]),
        ]);

        let found = child_or_nested_client_facet_child(&root, "m_overrides").expect("field");
        assert_same_element(found, &root.children()[0]);
    }

    #[test]
    fn child_or_nested_client_facet_child_falls_back_to_direct_client_facet() {
        let root = Element::new(Uuid::from_u128(0x01)).with_children(vec![
            Element::new(Uuid::from_u128(0x20))
                .with_field("m_clientFacetPtr")
                .with_children(vec![
                    Element::new(Uuid::from_u128(0x31)).with_field("m_overrides"),
                ]),
        ]);

        let found = child_or_nested_client_facet_child(&root, "m_overrides").expect("field");
        assert_same_element(found, &root.children()[0].children()[0]);
    }

    #[test]
    fn child_or_nested_client_facet_child_falls_back_to_nested_client_facet() {
        let root = Element::new(Uuid::from_u128(0x01)).with_children(vec![
            Element::new(Uuid::from_u128(0x10)).with_children(vec![
                Element::new(Uuid::from_u128(0x20))
                    .with_field("m_clientFacetPtr")
                    .with_children(vec![
                        Element::new(Uuid::from_u128(0x31)).with_field("m_overrides"),
                    ]),
            ]),
        ]);

        let found = child_or_nested_client_facet_child(&root, "m_overrides").expect("field");
        assert_same_element(found, &root.children()[0].children()[0].children()[0]);
    }

    #[test]
    fn client_facet_uses_client_field() {
        let target = Uuid::from_u128(0x20);
        let root = Element::new(Uuid::from_u128(0x01)).with_children(vec![
            Element::new(Uuid::from_u128(0x10))
                .with_field("BaseClass1")
                .with_children(vec![
                    Element::new(target).with_field("m_serverFacetPtr"),
                    Element::new(target).with_field("m_clientFacetPtr"),
                ]),
        ]);

        let found = client_facet(&root, target).expect("client facet");
        assert_eq!(
            found.field().map(arcstr::ArcStr::as_str),
            Some("m_clientFacetPtr")
        );
    }

    #[test]
    fn base_class_of_type_follows_base_class_chain() {
        let target = Uuid::from_u128(0x20);
        let root = Element::new(Uuid::from_u128(0x01)).with_children(vec![
            Element::new(Uuid::from_u128(0x10))
                .with_field("BaseClass1")
                .with_children(vec![Element::new(target).with_field("BaseClass1")]),
        ]);

        let found = base_class_of_type(&root, target).expect("base class");
        assert_eq!(found.id(), &target);
    }

    #[test]
    fn base_class_of_type_ignores_non_base_children() {
        let target = Uuid::from_u128(0x20);
        let root = Element::new(Uuid::from_u128(0x01)).with_children(vec![
            Element::new(target).with_field("value"),
            Element::new(Uuid::from_u128(0x10)).with_field("BaseClass2"),
        ]);

        assert!(base_class_of_type(&root, target).is_none());
    }

    #[test]
    fn pointee_unwraps_shared_ptr() {
        let target = Uuid::from_u128(0x30);
        let wrapped = Element::new(types::AZSTD_SHARED_PTR)
            .with_children(vec![Element::new(target).with_field("value")]);

        assert_eq!(pointee(&wrapped).id(), &target);
    }

    #[test]
    fn pointee_of_type_finds_typed_child_after_unwrap() {
        let expected = Uuid::from_u128(0x40);
        let wrapped = Element::new(types::AZSTD_SHARED_PTR).with_children(vec![
            Element::new(Uuid::from_u128(0x41)).with_children(vec![Element::new(expected)]),
        ]);

        assert_eq!(pointee_of_type(&wrapped, &expected).id(), &expected);
    }

    #[test]
    fn pointee_of_type_falls_back_to_resolved_pointee() {
        let pointee_id = Uuid::from_u128(0x50);
        let wrapped =
            Element::new(types::AZSTD_SHARED_PTR).with_children(vec![Element::new(pointee_id)]);

        assert_eq!(
            pointee_of_type(&wrapped, &Uuid::from_u128(0x51)).id(),
            &pointee_id
        );
    }

    #[test]
    fn type_name_prefers_resolved_name() {
        let mut element = Element::new(Uuid::from_u128(0x60));
        element.name = "AzFramework::ScriptPropertyNumber".into();

        assert_eq!(type_name(&element), "AzFramework::ScriptPropertyNumber");
    }

    #[test]
    fn type_name_falls_back_to_uuid() {
        let id = Uuid::from_u128(0x70);
        let element = Element::new(id);

        assert_eq!(type_name(&element), id.to_string());
    }

    #[test]
    fn az_entity_elements_returns_nested_entities() {
        let roots = vec![Element::new(Uuid::from_u128(0x01)).with_children(vec![
            Element::new(types::AZ_ENTITY),
            Element::new(Uuid::from_u128(0x02)).with_children(vec![Element::new(types::AZ_ENTITY)]),
        ])];

        let ids = az_entity_elements(&roots)
            .map(|element| *element.id())
            .collect::<Vec<_>>();

        assert_eq!(ids, vec![types::AZ_ENTITY, types::AZ_ENTITY]);
    }

    #[test]
    fn az_entity_elements_can_skip_named_subtrees() {
        let roots = vec![Element::new(Uuid::from_u128(0x01)).with_children(vec![
            Element::new(types::AZ_ENTITY),
            Element::new(Uuid::from_u128(0x02))
                .with_field("m_inlineShape")
                .with_children(vec![Element::new(types::AZ_ENTITY)]),
            Element::new(Uuid::from_u128(0x03))
                .with_field("m_collisionShape")
                .with_children(vec![Element::new(types::AZ_ENTITY)]),
        ])];

        let ids =
            az_entity_elements_skipping_fields(&roots, &["m_inlineShape", "m_collisionShape"])
                .map(|element| *element.id())
                .collect::<Vec<_>>();

        assert_eq!(ids, vec![types::AZ_ENTITY]);
    }

    fn assert_same_element(actual: &Element, expected: &Element) {
        assert!(std::ptr::addr_eq(
            std::ptr::from_ref(actual),
            std::ptr::from_ref(expected)
        ));
    }
}
