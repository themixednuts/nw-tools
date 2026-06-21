//! ObjectStream binary / XML / JSON codec.
//!
//! Reads and writes ObjectStream payloads through one structured
//! [`ObjectStream`] graph. Binary, XML, and JSON variants normalize to
//! the same in-memory shape.
//!
//! # Quick start
//!
//! ```no_run
//! use nw_objectstream::ObjectStream;
//!
//! let bytes = [0, 0, 0, 0, 3, 0];
//! let stream = ObjectStream::from_bytes(&bytes, None)?;
//! let element_count = stream.iter_recursive().count();
//! assert_eq!(element_count, 0);
//! # Ok::<(), Box<dyn std::error::Error>>(())
//! ```
//!
//! Format boundaries live in [`deserialize`] and [`serialize`].
//! Callers select an [`ObjectStreamEncoding`] and let this crate own the
//! byte/XML/JSON details.

pub mod asset_id;
pub mod asset_reference;
pub(crate) mod binary;
pub mod component_id;
pub mod deserialize;
pub mod lookup;
pub mod object;
pub mod query;
pub mod region_slice_data;
pub mod serialize;
pub mod slice_meta;
pub mod stats;
pub mod type_uuid;
pub mod types;
pub mod value;
pub mod visit;

pub use component_id::ComponentId;

use std::borrow::Cow;
use std::fmt;
use std::io::{self, Cursor, Read, Write};
use std::str;

use arcstr::ArcStr;
use crc32fast::hash;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use thiserror::Error;
use uuid::Uuid;

use crate::lookup::NameLookup;
use crate::types::uuid_data_to_serialize;

// === binary flag bits ===

/// Top-3-bit mask isolating element-header flag bits.
pub const ST_BINARYFLAG_MASK: u8 = 0xF8;
/// Bottom-3-bit mask: encoded value-byte width (1, 2, or 4).
pub const ST_BINARY_VALUE_SIZE_MASK: u8 = 0x07;
/// Bit set when the byte introduces a new element header.
pub const ST_BINARYFLAG_ELEMENT_HEADER: u8 = 1 << 3;
pub const ST_BINARYFLAG_HAS_VALUE: u8 = 1 << 4;
pub const ST_BINARYFLAG_EXTRA_SIZE_FIELD: u8 = 1 << 5;
pub const ST_BINARYFLAG_HAS_NAME: u8 = 1 << 6;
pub const ST_BINARYFLAG_HAS_VERSION: u8 = 1 << 7;
/// Sentinel byte that terminates the current element list.
pub const ST_BINARYFLAG_ELEMENT_END: u8 = 0;

const BINARY_STREAM_TAG: u8 = 0;
const XML_STREAM_TAG: u8 = b'<';
const JSON_STREAM_TAG: u8 = b'{';

/// Magic-byte sequences seen at the head of an *uncompressed*
/// ObjectStream payload (first byte = binary tag 0, next 4 bytes =
/// version big-endian).
pub const UNCOMPRESSED_SIGNATURES: [[u8; 5]; 3] = [
    [0x00, 0x00, 0x00, 0x00, 0x03],
    [0x00, 0x00, 0x00, 0x00, 0x02],
    [0x00, 0x00, 0x00, 0x00, 0x01],
];

#[derive(Debug, Error)]
pub enum ObjectStreamError {
    #[error("io error: {0}")]
    Io(#[from] io::Error),

    #[error("not a valid ObjectStream binary payload (tag {0:#x})")]
    InvalidStreamTag(u8),

    #[error("unsupported ObjectStream version {0}")]
    UnsupportedVersion(u32),

    #[error("invalid ObjectStream binary element flags {0:#x}")]
    InvalidElementFlags(u8),

    #[error("ObjectStream binary payload has trailing bytes after the root terminator")]
    TrailingDataAfterRoot,

    #[error("ObjectStream encoding mismatch: expected {expected}, got {actual}")]
    UnexpectedEncoding {
        expected: ObjectStreamEncoding,
        actual: ObjectStreamEncoding,
    },

    #[error("ObjectStream XML is not UTF-8: {0}")]
    Utf8(#[from] str::Utf8Error),

    #[error("ObjectStream XML parse error: {0}")]
    Xml(#[from] quick_xml::DeError),

    #[error("ObjectStream JSON parse error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("unsupported data-size value-byte width: {0}")]
    UnsupportedSizeWidth(u8),

    #[error("malformed UUID at element header: {0}")]
    Uuid(#[from] uuid::Error),
}

/// First byte of an ObjectStream payload — flags the binary / XML /
/// JSON variant.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct StreamTag(pub u8);

impl StreamTag {
    pub const BINARY: Self = StreamTag(BINARY_STREAM_TAG);
    pub const XML: Self = StreamTag(XML_STREAM_TAG);
    pub const JSON: Self = StreamTag(JSON_STREAM_TAG);

    /// Detect the stream variant from the first byte. Returns
    /// `None` if the byte doesn't match any known tag.
    #[inline]
    #[must_use]
    pub const fn from_byte(byte: u8) -> Option<Self> {
        match byte {
            BINARY_STREAM_TAG => Some(Self::BINARY),
            XML_STREAM_TAG => Some(Self::XML),
            JSON_STREAM_TAG => Some(Self::JSON),
            _ => None,
        }
    }

    #[inline]
    #[must_use]
    pub const fn is_binary(self) -> bool {
        self.0 == BINARY_STREAM_TAG
    }

    #[inline]
    #[must_use]
    pub const fn is_xml(self) -> bool {
        self.0 == XML_STREAM_TAG
    }

    #[inline]
    #[must_use]
    pub const fn is_json(self) -> bool {
        self.0 == JSON_STREAM_TAG
    }
}

pub(crate) fn validate_stream_version(version: u32) -> Result<u32, ObjectStreamError> {
    if matches!(version, 1..=3) {
        Ok(version)
    } else {
        Err(ObjectStreamError::UnsupportedVersion(version))
    }
}

impl Default for StreamTag {
    fn default() -> Self {
        Self::BINARY
    }
}

impl fmt::Display for StreamTag {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match *self {
            Self::BINARY => "BINARY",
            Self::XML => "XML",
            Self::JSON => "JSON",
            _ => "UNKNOWN",
        })
    }
}

impl PartialEq<u8> for StreamTag {
    #[inline]
    fn eq(&self, other: &u8) -> bool {
        self.0 == *other
    }
}

/// Supported ObjectStream encodings.
#[derive(
    Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize,
)]
#[serde(rename_all = "lowercase")]
pub enum ObjectStreamEncoding {
    /// Binary ObjectStream.
    #[default]
    Binary,
    /// XML ObjectStream.
    Xml,
    /// JSON ObjectStream.
    Json,
}

impl ObjectStreamEncoding {
    #[inline]
    #[must_use]
    pub const fn from_tag(tag: StreamTag) -> Option<Self> {
        match tag {
            StreamTag::BINARY => Some(Self::Binary),
            StreamTag::XML => Some(Self::Xml),
            StreamTag::JSON => Some(Self::Json),
            _ => None,
        }
    }

    #[inline]
    #[must_use]
    pub const fn from_tag_byte(byte: u8) -> Option<Self> {
        match byte {
            BINARY_STREAM_TAG => Some(Self::Binary),
            XML_STREAM_TAG => Some(Self::Xml),
            JSON_STREAM_TAG => Some(Self::Json),
            _ => None,
        }
    }

    #[inline]
    #[must_use]
    pub const fn stream_tag(self) -> StreamTag {
        match self {
            Self::Binary => StreamTag::BINARY,
            Self::Xml => StreamTag::XML,
            Self::Json => StreamTag::JSON,
        }
    }

    #[inline]
    #[must_use]
    pub const fn extension(self) -> &'static str {
        match self {
            Self::Binary => "",
            Self::Xml => "xml",
            Self::Json => "json",
        }
    }
}

/// Cheaply classify bytes that look like an ObjectStream payload.
///
/// This is intentionally conservative and does not allocate: binary
/// payloads must have a supported big-endian version, XML payloads must
/// start with `<ObjectStream`, and JSON payloads must have the expected
/// ObjectStream members in the first small prefix.
#[must_use]
pub fn sniff_encoding(bytes: &[u8]) -> Option<ObjectStreamEncoding> {
    let (&tag, _) = bytes.split_first()?;
    match StreamTag::from_byte(tag)? {
        StreamTag::BINARY => {
            let version = bytes
                .get(1..5)
                .and_then(|bytes| bytes.try_into().ok())
                .map(u32::from_be_bytes)?;
            validate_stream_version(version)
                .ok()
                .map(|_| ObjectStreamEncoding::Binary)
        }
        StreamTag::XML => bytes
            .starts_with(b"<ObjectStream")
            .then_some(ObjectStreamEncoding::Xml),
        StreamTag::JSON => {
            let prefix_len = bytes.len().min(512);
            let prefix = std::str::from_utf8(&bytes[..prefix_len]).ok()?;
            (prefix.contains("\"ObjectStream\"") && prefix.contains("\"Objects\""))
                .then_some(ObjectStreamEncoding::Json)
        }
        _ => None,
    }
}

#[must_use]
pub fn looks_like_objectstream(bytes: &[u8]) -> bool {
    sniff_encoding(bytes).is_some()
}

impl fmt::Display for ObjectStreamEncoding {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Binary => "binary",
            Self::Xml => "xml",
            Self::Json => "json",
        })
    }
}

/// In-memory ObjectStream graph.
#[derive(Default, Debug, Serialize, Deserialize)]
pub struct ObjectStream {
    pub tag: StreamTag,
    pub version: u32,
    pub elements: Vec<Element>,
}

impl ObjectStream {
    /// Construct an empty binary stream at `version`.
    #[inline]
    #[must_use]
    pub const fn new(version: u32) -> Self {
        Self {
            tag: StreamTag::BINARY,
            version,
            elements: Vec::new(),
        }
    }

    #[inline]
    #[must_use]
    pub const fn version(&self) -> u32 {
        self.version
    }

    #[inline]
    #[must_use]
    pub const fn tag(&self) -> StreamTag {
        self.tag
    }

    /// Encoding detected when this ObjectStream was read.
    ///
    /// The decoded graph is independent of this value; write methods
    /// choose their output encoding explicitly.
    #[inline]
    #[must_use]
    pub fn encoding(&self) -> ObjectStreamEncoding {
        ObjectStreamEncoding::from_tag(self.tag).unwrap_or_default()
    }

    /// Top-level elements (non-recursive).
    #[inline]
    #[must_use]
    pub fn elements(&self) -> &[Element] {
        &self.elements
    }

    /// Iterate the top-level element list.
    #[inline]
    pub fn iter(&self) -> std::slice::Iter<'_, Element> {
        self.elements.iter()
    }

    /// Iterate every element in the tree (depth-first pre-order).
    #[inline]
    pub fn iter_recursive(&self) -> RecursiveElements<'_> {
        RecursiveElements::new(&self.elements)
    }

    /// Depth-first search for the first element matching `query`.
    pub fn find<F>(&self, query: F) -> Option<&Element>
    where
        F: Fn(&Element) -> bool,
    {
        for element in &self.elements {
            if let Some(result) = element.find(&query) {
                return Some(result);
            }
        }
        None
    }

    /// Write the binary form back to a writer (round-trips
    /// [`from_reader`]).
    pub fn write_to<W: Write>(&self, writer: &mut W) -> io::Result<()> {
        serialize::write_to(self, writer)
    }

    /// Parse binary, XML, or JSON ObjectStream bytes.
    #[inline]
    pub fn from_bytes(
        bytes: &[u8],
        hashes: Option<&NameLookup>,
    ) -> Result<Self, ObjectStreamError> {
        deserialize::from_bytes(bytes, hashes)
    }

    /// Read a binary ObjectStream payload from a reader.
    #[inline]
    pub fn from_reader<R: Read>(
        reader: &mut R,
        hashes: Option<&NameLookup>,
    ) -> Result<Self, ObjectStreamError> {
        deserialize::from_reader(reader, hashes)
    }

    /// Parse ObjectStream bytes that must use `encoding`.
    #[inline]
    pub fn from_encoding_bytes(
        bytes: &[u8],
        encoding: ObjectStreamEncoding,
        hashes: Option<&NameLookup>,
    ) -> Result<Self, ObjectStreamError> {
        deserialize::from_encoding_bytes(bytes, encoding, hashes)
    }

    /// Encode this ObjectStream in binary form.
    #[inline]
    #[must_use]
    pub fn to_bytes(&self) -> Vec<u8> {
        serialize::to_bytes(self)
    }

    /// Encode this ObjectStream in the requested encoding.
    #[inline]
    pub fn to_encoding_bytes(&self, encoding: ObjectStreamEncoding) -> io::Result<Vec<u8>> {
        serialize::to_encoding_bytes(self, encoding)
    }

    /// Convert ObjectStream bytes into another ObjectStream encoding.
    ///
    /// Binary inputs use the crate's streaming reader/writer path so
    /// deep reflected assets can be written without growing the process
    /// stack.
    #[inline]
    pub fn transcode_bytes(
        bytes: &[u8],
        encoding: ObjectStreamEncoding,
        hashes: Option<&NameLookup>,
    ) -> Result<Vec<u8>, ObjectStreamError> {
        serialize::transcode_bytes(bytes, encoding, hashes)
    }

    /// Write ObjectStream bytes in another ObjectStream encoding.
    #[inline]
    pub fn transcode_to_writer<W: Write>(
        bytes: &[u8],
        encoding: ObjectStreamEncoding,
        hashes: Option<&NameLookup>,
        writer: &mut W,
    ) -> Result<(), ObjectStreamError> {
        serialize::transcode_to_writer(bytes, encoding, hashes, writer)
    }

    /// Write this ObjectStream in the requested encoding.
    #[inline]
    pub fn write_as<W: Write>(
        &self,
        encoding: ObjectStreamEncoding,
        writer: &mut W,
    ) -> io::Result<()> {
        serialize::write_as(self, encoding, writer)
    }
}

impl<'a> IntoIterator for &'a ObjectStream {
    type Item = &'a Element;
    type IntoIter = std::slice::Iter<'a, Element>;

    #[inline]
    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}

impl From<XMLObjectStream> for ObjectStream {
    fn from(value: XMLObjectStream) -> Self {
        Self {
            tag: StreamTag::XML,
            version: value.version,
            elements: value.elements.into_iter().map(Element::from).collect(),
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename = "ObjectStream")]
pub struct XMLObjectStream {
    #[serde(rename = "@version")]
    pub version: u32,
    #[serde(default, rename = "Class")]
    pub elements: Vec<XMLElement>,
}

impl XMLObjectStream {
    pub fn write_to(&mut self, buf: &mut impl Write) -> io::Result<u64> {
        let string = quick_xml::se::to_string(self).map_err(io::Error::other)?;
        io::copy(&mut Cursor::new(string), buf)
    }
}

impl From<ObjectStream> for XMLObjectStream {
    fn from(value: ObjectStream) -> Self {
        Self {
            version: value.version,
            elements: value.elements.into_iter().map(XMLElement::from).collect(),
        }
    }
}

impl From<&ObjectStream> for XMLObjectStream {
    fn from(value: &ObjectStream) -> Self {
        Self {
            version: value.version,
            elements: value.elements.iter().map(XMLElement::from).collect(),
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct JSONObjectStream {
    pub name: String,
    pub version: u32,
    #[serde(rename = "Objects")]
    pub elements: Vec<JSONElement>,
}

impl JSONObjectStream {
    pub fn write_to(&self, buf: &mut impl Write) -> serde_json::Result<()> {
        serde_json::to_writer_pretty(buf, self)
    }
}

impl From<ObjectStream> for JSONObjectStream {
    fn from(value: ObjectStream) -> Self {
        Self {
            name: "ObjectStream".into(),
            version: value.version,
            elements: value.elements.into_iter().map(JSONElement::from).collect(),
        }
    }
}

impl From<&ObjectStream> for JSONObjectStream {
    fn from(value: &ObjectStream) -> Self {
        Self {
            name: "ObjectStream".into(),
            version: value.version,
            elements: value.elements.iter().map(JSONElement::from).collect(),
        }
    }
}

impl From<JSONObjectStream> for ObjectStream {
    fn from(value: JSONObjectStream) -> Self {
        Self {
            tag: StreamTag::JSON,
            version: value.version,
            elements: value.elements.into_iter().map(Element::from).collect(),
        }
    }
}

/// One node in an ObjectStream tree.
///
/// `name` and `field` are [`ArcStr`]s — cloning them is an atomic
/// refcount bump, and the bytes are shared across every `Element`
/// that resolved to the same lookup table entry.
#[derive(PartialEq, Default, Debug, Serialize, Deserialize)]
pub struct Element {
    pub flags: u8,
    pub name_crc: Option<u32>,
    pub version: Option<u8>,
    #[serde(with = "uuid::serde::compact")]
    pub id: Uuid,
    pub specialization: Option<Uuid>,
    #[serde(skip)]
    pub name: ArcStr,
    pub data_size: Option<usize>,
    pub data: Option<Vec<u8>>,
    pub elements: Vec<Element>,
    pub field: Option<ArcStr>,
}

impl Element {
    #[inline]
    #[must_use]
    pub fn new(id: Uuid) -> Self {
        Self {
            id,
            ..Default::default()
        }
    }

    #[inline]
    #[must_use]
    pub fn with_field(mut self, field: impl Into<ArcStr>) -> Self {
        self.field = Some(field.into());
        self
    }

    #[inline]
    #[must_use]
    pub fn with_data(mut self, data: impl Into<Vec<u8>>) -> Self {
        self.data = Some(data.into());
        self
    }

    #[inline]
    #[must_use]
    pub fn with_children(mut self, children: impl Into<Vec<Element>>) -> Self {
        self.elements = children.into();
        self
    }

    /// AZ class UUID for this element.
    #[inline]
    #[must_use]
    pub const fn id(&self) -> &Uuid {
        &self.id
    }

    /// Human-readable type name (only populated when a
    /// [`NameLookup`] table was supplied during parse).
    /// Returns the wrapped [`ArcStr`]. It derefs to `&str` for
    /// most uses; clone it cheaply via [`ArcStr::clone`] when you
    /// want to keep an owned reference.
    #[inline]
    #[must_use]
    pub fn name(&self) -> &ArcStr {
        &self.name
    }

    /// Resolved field name from the [`NameLookup`] table, if any.
    #[inline]
    #[must_use]
    pub fn field(&self) -> Option<&ArcStr> {
        self.field.as_ref()
    }

    /// Field name CRC, if [`ST_BINARYFLAG_HAS_NAME`] was set.
    #[inline]
    #[must_use]
    pub const fn name_crc(&self) -> Option<u32> {
        self.name_crc
    }

    /// Class version, if [`ST_BINARYFLAG_HAS_VERSION`] was set.
    #[inline]
    #[must_use]
    pub const fn version(&self) -> Option<u8> {
        self.version
    }

    /// Element specialization UUID (only present when the parent
    /// stream is `version == 2`).
    #[inline]
    #[must_use]
    pub const fn specialization(&self) -> Option<&Uuid> {
        self.specialization.as_ref()
    }

    /// Raw payload bytes, if any.
    #[inline]
    #[must_use]
    pub fn data(&self) -> Option<&[u8]> {
        self.data.as_deref()
    }

    /// Decode this element's leaf payload as an AZ reflected value.
    pub fn decode<'a, T>(&'a self) -> Result<T, value::ObjectStreamValueError>
    where
        T: value::DecodeAzValue<'a>,
    {
        T::decode_az_value(self)
    }

    /// Decode a named child field as an AZ reflected value.
    pub fn field_value<'a, T>(
        &'a self,
        field: &str,
    ) -> Result<Option<T>, value::ObjectStreamValueError>
    where
        T: value::DecodeAzValue<'a>,
    {
        value::child_by_field(self, field)
            .map(T::decode_az_value)
            .transpose()
    }

    /// Deserialize this element as an ObjectStream object.
    pub fn deserialize<'a, T>(&'a self) -> Result<T, value::ObjectStreamValueError>
    where
        T: object::Deserialize<'a>,
    {
        object::deserialize(self)
    }

    /// Child elements.
    #[inline]
    #[must_use]
    pub fn children(&self) -> &[Element] {
        &self.elements
    }

    /// `true` iff this element has no child elements.
    #[inline]
    #[must_use]
    pub const fn is_leaf(&self) -> bool {
        self.elements.is_empty()
    }

    /// `true` iff [`ST_BINARYFLAG_HAS_NAME`] is set in `flags`.
    #[inline]
    #[must_use]
    pub const fn has_name(&self) -> bool {
        self.flags & ST_BINARYFLAG_HAS_NAME != 0
    }

    /// `true` iff [`ST_BINARYFLAG_HAS_VALUE`] is set in `flags`.
    #[inline]
    #[must_use]
    pub const fn has_value(&self) -> bool {
        self.flags & ST_BINARYFLAG_HAS_VALUE != 0
    }

    /// `true` iff [`ST_BINARYFLAG_HAS_VERSION`] is set in `flags`.
    #[inline]
    #[must_use]
    pub const fn has_version(&self) -> bool {
        self.flags & ST_BINARYFLAG_HAS_VERSION != 0
    }

    /// `true` iff [`ST_BINARYFLAG_EXTRA_SIZE_FIELD`] is set in `flags`.
    #[inline]
    #[must_use]
    pub const fn has_extra_size_field(&self) -> bool {
        self.flags & ST_BINARYFLAG_EXTRA_SIZE_FIELD != 0
    }

    /// Encoded value-byte width (1, 2, or 4) from the bottom 3 bits
    /// of `flags`.
    #[inline]
    #[must_use]
    pub const fn value_width(&self) -> u8 {
        self.flags & ST_BINARY_VALUE_SIZE_MASK
    }

    /// Iterate all elements in this subtree (depth-first pre-order,
    /// includes `self`).
    #[inline]
    pub fn iter_recursive(&self) -> RecursiveElement<'_> {
        RecursiveElement {
            root: Some(self),
            stack: Vec::new(),
        }
    }

    /// Iterate this element's direct children.
    #[inline]
    pub fn iter(&self) -> std::slice::Iter<'_, Element> {
        self.elements.iter()
    }

    /// Depth-first search rooted at this element.
    pub fn find<F>(&self, query: &F) -> Option<&Element>
    where
        F: Fn(&Element) -> bool,
    {
        if query(self) {
            return Some(self);
        }
        for child in &self.elements {
            if let Some(result) = child.find(query) {
                return Some(result);
            }
        }
        None
    }
}

impl<'a> IntoIterator for &'a Element {
    type Item = &'a Element;
    type IntoIter = std::slice::Iter<'a, Element>;

    #[inline]
    fn into_iter(self) -> Self::IntoIter {
        self.elements.iter()
    }
}

fn serialize_value_to_uuid_data(id: &Uuid, value: &Value) -> Option<Vec<u8>> {
    let text = value_text(value)?;
    match *id {
        types::CHAR | types::AZ_S8 | types::SIGNED_CHAR => {
            Some((text.parse::<i8>().ok()?).to_be_bytes().to_vec())
        }
        types::SHORT => Some((text.parse::<i16>().ok()?).to_be_bytes().to_vec()),
        types::INT => Some((text.parse::<i32>().ok()?).to_be_bytes().to_vec()),
        types::LONG | types::AZ_S64 => Some((text.parse::<i64>().ok()?).to_be_bytes().to_vec()),
        types::UNSIGNED_CHAR => Some((text.parse::<u8>().ok()?).to_be_bytes().to_vec()),
        types::UNSIGNED_SHORT => Some((text.parse::<u16>().ok()?).to_be_bytes().to_vec()),
        types::UNSIGNED_INT => Some((text.parse::<u32>().ok()?).to_be_bytes().to_vec()),
        types::UNSIGNED_LONG | types::AZ_U64 => {
            Some((text.parse::<u64>().ok()?).to_be_bytes().to_vec())
        }
        types::FLOAT => Some((text.parse::<f32>().ok()?).to_be_bytes().to_vec()),
        types::DOUBLE => Some((text.parse::<f64>().ok()?).to_be_bytes().to_vec()),
        types::BOOL => Some([u8::from(parse_bool_text(&text)?)].to_vec()),
        types::AZ_UUID => Some(parse_uuid_text(&text)?.into_bytes().to_vec()),
        types::ASSET => serialize_asset_value(&text),
        types::VECTOR_FLOAT
        | types::VECTOR2
        | types::VECTOR3
        | types::VECTOR4
        | types::TRANSFORM
        | types::QUATERNION
        | types::COLOR
        | types::MATRIX3X3
        | types::MATRIX4X4 => {
            let mut bytes = Vec::new();
            for value in text.split_whitespace() {
                bytes.extend_from_slice(&value.parse::<f32>().ok()?.to_be_bytes());
            }
            Some(bytes)
        }
        types::AZSTD_STRING | types::AZSTD_BASIC_STRING | types::AZSTD_STRING_XML_ALIAS => {
            Some(text.as_bytes().to_vec())
        }
        types::BYTE_STREAM => hex::decode(text.trim().as_bytes()).ok(),
        _ => Some(text.as_bytes().to_vec()),
    }
}

fn value_text(value: &Value) -> Option<Cow<'_, str>> {
    match value {
        Value::String(value) => Some(Cow::Borrowed(value.as_str())),
        Value::Number(value) => Some(Cow::Owned(value.to_string())),
        Value::Bool(value) => Some(Cow::Borrowed(if *value { "true" } else { "false" })),
        _ => None,
    }
}

fn parse_bool_text(value: &str) -> Option<bool> {
    match value.trim() {
        "true" | "True" | "TRUE" | "1" => Some(true),
        "false" | "False" | "FALSE" | "0" => Some(false),
        _ => None,
    }
}

fn parse_uuid_text(value: &str) -> Option<Uuid> {
    Uuid::parse_str(value.trim().trim_start_matches('{').trim_end_matches('}')).ok()
}

fn serialize_asset_value(value: &str) -> Option<Vec<u8>> {
    let (asset_id, rest) = value.trim().strip_prefix("id=")?.split_once(",type=")?;
    let (guid, sub_id) = asset_id.rsplit_once(':')?;
    let (type_id, hint) = rest.split_once(",hint={")?;
    let hint = hint.split_once('}').map_or(hint, |(hint, _)| hint);
    let guid = parse_uuid_text(guid)?;
    let sub_id = sub_id.parse::<u64>().ok()?;
    let type_id = parse_uuid_text(type_id)?;
    let hint = hint.as_bytes();

    let mut bytes = Vec::with_capacity(48 + hint.len());
    bytes.extend_from_slice(guid.as_bytes());
    bytes.extend_from_slice(&sub_id.to_be_bytes());
    bytes.extend_from_slice(type_id.as_bytes());
    bytes.extend_from_slice(&(hint.len() as u64).to_be_bytes());
    bytes.extend_from_slice(hint);
    Some(bytes)
}

impl From<XMLElement> for Element {
    fn from(value: XMLElement) -> Self {
        let name_crc = if value.name.is_empty() {
            None
        } else {
            Some(hash(value.name.as_bytes()))
        };
        let data = value
            .value
            .as_ref()
            .and_then(|data| serialize_value_to_uuid_data(&value.id, data));
        Self {
            id: value.id,
            name: ArcStr::from(value.name),
            name_crc,
            field: value.field.map(ArcStr::from),
            version: value.version,
            elements: value.elements.into_iter().map(Element::from).collect(),
            data,
            ..Default::default()
        }
    }
}

impl From<JSONElement> for Element {
    fn from(value: JSONElement) -> Self {
        let name_crc = if value.name.is_empty() {
            None
        } else {
            Some(hash(value.name.as_bytes()))
        };
        let data = value
            .value
            .as_ref()
            .and_then(|data| serialize_value_to_uuid_data(&value.id, data));
        Self {
            id: value.id,
            name: ArcStr::from(value.name),
            name_crc,
            field: value.field.map(ArcStr::from),
            version: value.version,
            specialization: value.specialization,
            data,
            elements: value
                .elements
                .map(|ele| ele.into_iter().map(Element::from).collect())
                .unwrap_or_default(),
            ..Default::default()
        }
    }
}

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct XMLElement {
    #[serde(rename = "@name")]
    pub name: String,
    #[serde(rename = "@field", skip_serializing_if = "Option::is_none")]
    pub field: Option<String>,
    #[serde(rename = "@value", skip_serializing_if = "Option::is_none")]
    pub value: Option<Value>,
    #[serde(rename = "@version", skip_serializing_if = "Option::is_none")]
    pub version: Option<u8>,
    #[serde(rename = "@type", with = "uuid_braced_uppercase")]
    pub id: Uuid,
    #[serde(default, rename = "Class")]
    pub elements: Vec<XMLElement>,
}

impl From<Element> for XMLElement {
    fn from(value: Element) -> Self {
        Self {
            name: value.name.to_string(),
            field: value.field.map(|s| s.to_string()),
            value: match value.data {
                Some(data) if !data.is_empty() || value.elements.is_empty() => {
                    uuid_data_to_serialize(&value.id, &data, false).ok()
                }
                _ => None,
            },
            version: value.version,
            id: value.id,
            elements: value.elements.into_iter().map(XMLElement::from).collect(),
        }
    }
}

impl From<&Element> for XMLElement {
    fn from(value: &Element) -> Self {
        Self {
            name: value.name.to_string(),
            field: value.field.as_ref().map(arcstr::ArcStr::to_string),
            value: match &value.data {
                Some(data) if !data.is_empty() || value.elements.is_empty() => {
                    uuid_data_to_serialize(&value.id, data, false).ok()
                }
                _ => None,
            },
            version: value.version,
            id: value.id,
            elements: value.elements.iter().map(XMLElement::from).collect(),
        }
    }
}

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct JSONElement {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub field: Option<String>,
    #[serde(rename = "typeId", with = "uuid_braced_uppercase")]
    pub id: Uuid,
    #[serde(rename = "typeName")]
    pub name: String,
    #[serde(
        rename = "specializationTypeId",
        with = "option_braced_uppercase",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    pub specialization: Option<Uuid>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub value: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub version: Option<u8>,
    #[serde(rename = "Objects", skip_serializing_if = "Option::is_none")]
    pub elements: Option<Vec<JSONElement>>,
}

impl From<Element> for JSONElement {
    fn from(value: Element) -> Self {
        Self {
            field: value.field.map(|field| field.to_string()),
            id: value.id,
            name: value.name.to_string(),
            specialization: value.specialization,
            value: match &value.data {
                Some(data) if !data.is_empty() => {
                    uuid_data_to_serialize(&value.id, data, true).ok().map(|v| {
                        if v.is_string() {
                            v
                        } else {
                            Value::String(v.to_string())
                        }
                    })
                }
                Some(data) if data.is_empty() && value.elements.is_empty() => Some("".into()),
                _ => None,
            },
            version: value.version,
            elements: {
                let ele: Vec<JSONElement> =
                    value.elements.into_iter().map(JSONElement::from).collect();
                if ele.is_empty() && value.data.is_some() {
                    None
                } else {
                    Some(ele)
                }
            },
        }
    }
}

impl From<&Element> for JSONElement {
    fn from(value: &Element) -> Self {
        Self {
            field: value.field.as_ref().map(arcstr::ArcStr::to_string),
            id: value.id,
            name: value.name.to_string(),
            specialization: value.specialization,
            value: match &value.data {
                Some(data) if !data.is_empty() => {
                    uuid_data_to_serialize(&value.id, data, true).ok().map(|v| {
                        if v.is_string() {
                            v
                        } else {
                            Value::String(v.to_string())
                        }
                    })
                }
                Some(data) if data.is_empty() && value.elements.is_empty() => Some("".into()),
                _ => None,
            },
            version: value.version,
            elements: {
                let elements: Vec<JSONElement> =
                    value.elements.iter().map(JSONElement::from).collect();
                if elements.is_empty() && value.data.is_some() {
                    None
                } else {
                    Some(elements)
                }
            },
        }
    }
}

/// Serde helper for braced uppercase UUID rendering (`{ABCDEF...}`).
pub mod uuid_braced_uppercase {
    use serde::{Deserialize, Deserializer, Serializer};
    use uuid::Uuid;

    pub fn serialize<S>(uuid: &Uuid, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let s = uuid.as_braced().to_string().to_uppercase();
        serializer.serialize_str(&s)
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<Uuid, D::Error>
    where
        D: Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        Uuid::parse_str(&s).map_err(serde::de::Error::custom)
    }
}

/// Serde helper for `Option<Uuid>` in braced uppercase format.
pub mod option_braced_uppercase {
    use serde::{Deserialize, Deserializer, Serializer};
    use uuid::Uuid;

    pub fn serialize<S>(value: &Option<Uuid>, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        match value {
            Some(uuid) => super::uuid_braced_uppercase::serialize(uuid, serializer),
            None => serializer.serialize_none(),
        }
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<Option<Uuid>, D::Error>
    where
        D: Deserializer<'de>,
    {
        let opt = Option::<String>::deserialize(deserializer)?;
        match opt {
            Some(s) => Uuid::parse_str(&s)
                .map(Some)
                .map_err(serde::de::Error::custom),
            None => Ok(None),
        }
    }
}

// === recursive iterators ===

/// Depth-first iterator over every element in an [`ObjectStream`]
/// (pre-order). Top-level call order matches `iter_recursive`.
#[must_use]
pub struct RecursiveElements<'a> {
    stack: Vec<std::slice::Iter<'a, Element>>,
}

impl<'a> RecursiveElements<'a> {
    fn new(roots: &'a [Element]) -> Self {
        Self {
            stack: vec![roots.iter()],
        }
    }
}

impl<'a> Iterator for RecursiveElements<'a> {
    type Item = &'a Element;

    fn next(&mut self) -> Option<Self::Item> {
        while let Some(top) = self.stack.last_mut() {
            if let Some(element) = top.next() {
                if !element.elements.is_empty() {
                    self.stack.push(element.elements.iter());
                }
                return Some(element);
            }
            self.stack.pop();
        }
        None
    }
}

/// Single-rooted depth-first iterator (yields `self` first, then
/// descendants in pre-order).
#[must_use]
pub struct RecursiveElement<'a> {
    root: Option<&'a Element>,
    stack: Vec<std::slice::Iter<'a, Element>>,
}

impl<'a> Iterator for RecursiveElement<'a> {
    type Item = &'a Element;

    fn next(&mut self) -> Option<Self::Item> {
        if let Some(root) = self.root.take() {
            if !root.elements.is_empty() {
                self.stack.push(root.elements.iter());
            }
            return Some(root);
        }
        while let Some(top) = self.stack.last_mut() {
            if let Some(element) = top.next() {
                if !element.elements.is_empty() {
                    self.stack.push(element.elements.iter());
                }
                return Some(element);
            }
            self.stack.pop();
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn xml_round_trip() {
        let xml = r#"<ObjectStream version="3"><Class name="int" field="test" value="2" type="{72039442-EB38-4D42-A1AD-CB68F7E0EEF6}"/><Class name="Asset" type="{72039442-EB38-4D42-A1AD-CB68F7E0EEF6}"><Class name="int" field="element" value="100" type="{72039442-EB38-4D42-A1AD-CB68F7E0EEF6}"/></Class></ObjectStream>"#;
        let xml_object_stream: XMLObjectStream = quick_xml::de::from_str(xml).unwrap();
        assert_eq!(
            quick_xml::se::to_string(&xml_object_stream)
                .unwrap()
                .as_str(),
            xml
        );
    }

    #[test]
    fn from_bytes_accepts_xml_objectstream() -> Result<(), ObjectStreamError> {
        let xml = br#"<ObjectStream version="3"><Class name="int" field="test" value="2" type="{72039442-EB38-4D42-A1AD-CB68F7E0EEF6}"/></ObjectStream>"#;
        let stream = ObjectStream::from_bytes(xml, None)?;

        assert_eq!(stream.version(), 3);
        assert_eq!(stream.elements().len(), 1);
        assert_eq!(stream.elements()[0].name().as_str(), "int");
        assert_eq!(
            stream.elements()[0].field().map(arcstr::ArcStr::as_str),
            Some("test")
        );
        Ok(())
    }

    #[test]
    fn xml_scalar_values_decode_as_element_data() -> Result<(), Box<dyn std::error::Error>> {
        let xml = br#"<ObjectStream version="3"><Class name="int" field="test" value="2" type="{72039442-EB38-4D42-A1AD-CB68F7E0EEF6}"/></ObjectStream>"#;
        let stream = ObjectStream::from_bytes(xml, None)?;

        assert_eq!(value::read_i32(&stream.elements()[0])?, 2);
        Ok(())
    }

    #[test]
    fn xml_asset_values_decode_as_binary_asset_data() -> Result<(), Box<dyn std::error::Error>> {
        let xml = br#"<ObjectStream version="3"><Class name="Asset" field="Material" value="id={1E9A1948-F2A6-5500-B918-964558497331}:7,type={F46985B5-F7FF-4FCB-8E8C-DC240D701841},hint={materials/terrain/foo.mtl}" version="1" type="{77A19D40-8731-4D3C-9041-1B43047366A4}"/></ObjectStream>"#;
        let stream = ObjectStream::from_bytes(xml, None)?;
        let data = stream.elements()[0].data().expect("asset value bytes");

        assert_eq!(
            &data[..16],
            Uuid::parse_str("1E9A1948-F2A6-5500-B918-964558497331")?.as_bytes()
        );
        assert_eq!(u64::from_be_bytes(data[16..24].try_into()?), 7);
        assert_eq!(
            &data[24..40],
            Uuid::parse_str("F46985B5-F7FF-4FCB-8E8C-DC240D701841")?.as_bytes()
        );
        let hint_len = usize::try_from(u64::from_be_bytes(data[40..48].try_into()?))?;
        assert_eq!(hint_len, "materials/terrain/foo.mtl".len());
        assert_eq!(
            std::str::from_utf8(&data[48..])?,
            "materials/terrain/foo.mtl"
        );
        Ok(())
    }

    #[test]
    fn xml_byte_stream_values_decode_as_binary_data() -> Result<(), Box<dyn std::error::Error>> {
        let xml = br#"<ObjectStream version="3"><Class name="ByteStream" field="Data" value="000102FF" type="{ADFD596B-7177-5519-9752-BC418FE42963}"/></ObjectStream>"#;
        let stream = ObjectStream::from_bytes(xml, None)?;

        assert_eq!(
            value::read_byte_stream(&stream.elements()[0])?,
            &[0, 1, 2, 255]
        );
        Ok(())
    }

    #[test]
    fn json_round_trip() -> serde_json::Result<()> {
        let json = r#"{"name":"ObjectStream","version":3,"Objects":[]}"#;
        let json_object_stream: JSONObjectStream = serde_json::from_str(json)?;
        assert_eq!(serde_json::to_string(&json_object_stream)?, json);
        Ok(())
    }

    #[test]
    fn from_bytes_accepts_json_objectstream() -> Result<(), ObjectStreamError> {
        let json = br#"{"name":"ObjectStream","version":3,"Objects":[{"field":"test","typeId":"{72039442-EB38-4D42-A1AD-CB68F7E0EEF6}","typeName":"int","value":2}]}"#;
        let stream = ObjectStream::from_bytes(json, None)?;

        assert_eq!(stream.version(), 3);
        assert_eq!(stream.elements().len(), 1);
        assert_eq!(stream.elements()[0].name().as_str(), "int");
        assert_eq!(
            stream.elements()[0].field().map(arcstr::ArcStr::as_str),
            Some("test")
        );
        Ok(())
    }

    #[test]
    fn stream_tag_classification() {
        assert!(StreamTag::BINARY.is_binary());
        assert!(StreamTag::XML.is_xml());
        assert!(StreamTag::JSON.is_json());
        assert_eq!(StreamTag::from_byte(b'<'), Some(StreamTag::XML));
        assert_eq!(StreamTag::from_byte(b'{'), Some(StreamTag::JSON));
        assert_eq!(StreamTag::from_byte(0), Some(StreamTag::BINARY));
        assert_eq!(StreamTag::from_byte(b'?'), None);
    }

    #[test]
    fn sniff_encoding_checks_payload_shape() {
        assert_eq!(
            sniff_encoding(&[0, 0, 0, 0, 3, 0]),
            Some(ObjectStreamEncoding::Binary)
        );
        assert_eq!(
            sniff_encoding(br#"<ObjectStream version="3"></ObjectStream>"#),
            Some(ObjectStreamEncoding::Xml)
        );
        assert_eq!(
            sniff_encoding(br#"{"name":"ObjectStream","version":3,"Objects":[]}"#),
            Some(ObjectStreamEncoding::Json)
        );
        assert_eq!(sniff_encoding(&[0, 0, 1, 0, 2, 0]), None);
        assert_eq!(sniff_encoding(b"<NotObjectStream/>"), None);
    }

    #[test]
    fn element_flag_predicates() {
        let e = Element {
            flags: ST_BINARYFLAG_HAS_NAME | ST_BINARYFLAG_HAS_VALUE | 0x04,
            ..Default::default()
        };
        assert!(e.has_name());
        assert!(e.has_value());
        assert!(!e.has_version());
        assert!(!e.has_extra_size_field());
        assert_eq!(e.value_width(), 4);
    }

    #[test]
    fn recursive_iter_pre_order() {
        let stream = ObjectStream {
            tag: StreamTag::BINARY,
            version: 3,
            elements: vec![
                Element {
                    name: ArcStr::from("A"),
                    elements: vec![
                        Element {
                            name: ArcStr::from("B"),
                            ..Default::default()
                        },
                        Element {
                            name: ArcStr::from("C"),
                            ..Default::default()
                        },
                    ],
                    ..Default::default()
                },
                Element {
                    name: ArcStr::from("D"),
                    ..Default::default()
                },
            ],
        };
        let names: Vec<&str> = stream.iter_recursive().map(|e| e.name().as_str()).collect();
        assert_eq!(names, vec!["A", "B", "C", "D"]);
    }

    #[test]
    fn element_recursive_iter_pre_order() {
        let element = Element {
            name: ArcStr::from("A"),
            elements: vec![
                Element {
                    name: ArcStr::from("B"),
                    elements: vec![Element {
                        name: ArcStr::from("C"),
                        ..Default::default()
                    }],
                    ..Default::default()
                },
                Element {
                    name: ArcStr::from("D"),
                    ..Default::default()
                },
            ],
            ..Default::default()
        };
        let names: Vec<&str> = element
            .iter_recursive()
            .map(|e| e.name().as_str())
            .collect();
        assert_eq!(names, vec!["A", "B", "C", "D"]);
    }

    #[test]
    fn binary_round_trip_empty() -> Result<(), ObjectStreamError> {
        let stream = ObjectStream::new(3);
        let mut buf = Vec::new();
        stream.write_to(&mut buf)?;
        let parsed = ObjectStream::from_bytes(&buf, None)?;
        assert_eq!(parsed.version, 3);
        assert!(parsed.elements.is_empty());
        Ok(())
    }

    #[test]
    fn binary_rejects_unsupported_stream_version() {
        // Real New World .cgfheap sidecars often begin with little-endian
        // index data: 00 00 01 00 02. If treated as ObjectStream, that
        // misreads as tag 0 and version 0x00010002, then the next zero byte
        // looks like an empty element list.
        let bytes = [0x00, 0x00, 0x01, 0x00, 0x02, 0x00];
        let error = ObjectStream::from_bytes(&bytes, None).unwrap_err();

        assert!(matches!(
            error,
            ObjectStreamError::UnsupportedVersion(65538)
        ));
    }

    #[test]
    fn xml_rejects_unsupported_stream_version() {
        let bytes = br#"<ObjectStream version="74"></ObjectStream>"#;
        let error = ObjectStream::from_bytes(bytes, None).unwrap_err();

        assert!(matches!(error, ObjectStreamError::UnsupportedVersion(74)));
    }

    #[test]
    fn json_rejects_unsupported_stream_version() {
        let bytes = br#"{"name":"ObjectStream","version":0,"Objects":[]}"#;
        let error = ObjectStream::from_bytes(bytes, None).unwrap_err();

        assert!(matches!(error, ObjectStreamError::UnsupportedVersion(0)));
    }

    #[test]
    fn binary_rejects_trailing_bytes_after_root_terminator() {
        let bytes = [
            0x00, 0x00, 0x00, 0x00, 0x03, // binary ObjectStream version 3
            0x00, // root element-list terminator
            0xFF, // stale payload that does not belong to the stream
        ];
        let error = ObjectStream::from_bytes(&bytes, None).unwrap_err();

        assert!(matches!(error, ObjectStreamError::TrailingDataAfterRoot));
    }

    #[test]
    fn binary_rejects_element_flags_without_header_bit() {
        let bytes = [
            0x00,
            0x00,
            0x00,
            0x00,
            0x03,                    // binary ObjectStream version 3
            ST_BINARYFLAG_HAS_VALUE, // value flag without element-header bit
            0x00,
        ];
        let error = ObjectStream::from_bytes(&bytes, None).unwrap_err();

        assert!(matches!(
            error,
            ObjectStreamError::InvalidElementFlags(ST_BINARYFLAG_HAS_VALUE)
        ));
    }
}

// SerializeContext flags:
//   FLG_POINTER          = (1 << 0)  // Element is stored as pointer (not a value).
//   FLG_BASE_CLASS       = (1 << 1)  // Element is a base class of the holding class.
//   FLG_NO_DEFAULT_VALUE = (1 << 2)  // Class element can't have a default value.
//   FLG_DYNAMIC_FIELD    = (1 << 3)  // Element represents a dynamic field
//                                    // (DynamicSerializableField::m_data).
//   FLG_UI_ELEMENT       = (1 << 4)  // Element represents a UI element tied to the
//                                    // ClassData of its parent.
