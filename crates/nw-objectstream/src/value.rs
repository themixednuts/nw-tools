//! Typed value readers for ObjectStream leaf elements.

use std::array::TryFromSliceError;
use std::fmt::Write as _;
use std::str::Utf8Error;

use thiserror::Error;
use uuid::Uuid;

use crate::Element;
use crate::types::{
    AZ_S8, AZ_S64, AZ_U64, AZ_UUID, AZSTD_BASIC_STRING, AZSTD_STRING, AZSTD_STRING_XML_ALIAS, BOOL,
    BYTE_STREAM, CHAR, COLOR, CRC32, DOUBLE, ENTITY_ID, FLOAT, INT, LONG, MATRIX3X3, MATRIX4X4,
    QUATERNION, SHORT, SIGNED_CHAR, TRANSFORM, UNSIGNED_CHAR, UNSIGNED_INT, UNSIGNED_LONG,
    UNSIGNED_SHORT, VECTOR_FLOAT, VECTOR2, VECTOR3, VECTOR4,
};
use crate::visit::ElementHeader;

#[derive(Debug, Error)]
pub enum ObjectStreamValueError {
    #[error("field `{field}` has type {actual}, expected {expected}")]
    UnexpectedType {
        field: String,
        expected: &'static str,
        actual: Uuid,
    },
    #[error("field `{field}` has invalid value, expected {expected}")]
    InvalidValue {
        field: String,
        expected: &'static str,
    },
    #[error("field `{field}` has no value bytes")]
    MissingData { field: String },
    #[error("field `{field}` is missing")]
    MissingField { field: String },
    #[error("field `{field}` is not consumed by this ObjectStream object")]
    UnknownField { field: String },
    #[error("field `{field}` has {actual} value bytes, expected {expected}")]
    InvalidLength {
        field: String,
        expected: usize,
        actual: usize,
    },
    #[error("field `{field}` is not valid UTF-8")]
    Utf8 {
        field: String,
        #[source]
        source: Utf8Error,
    },
}

/// Borrowed value surface shared by owned DOM elements and streaming headers.
pub trait ElementValue {
    fn id(&self) -> Uuid;
    fn field_name(&self) -> Option<&str>;
    fn data(&self) -> Option<&[u8]>;
}

impl ElementValue for Element {
    #[inline]
    fn id(&self) -> Uuid {
        *self.id()
    }

    #[inline]
    fn field_name(&self) -> Option<&str> {
        self.field().map(arcstr::ArcStr::as_str)
    }

    #[inline]
    fn data(&self) -> Option<&[u8]> {
        self.data()
    }
}

impl ElementValue for ElementHeader<'_> {
    #[inline]
    fn id(&self) -> Uuid {
        self.id
    }

    #[inline]
    fn field_name(&self) -> Option<&str> {
        self.field.map(arcstr::ArcStr::as_str)
    }

    #[inline]
    fn data(&self) -> Option<&[u8]> {
        (self.flags & crate::ST_BINARYFLAG_HAS_VALUE != 0).then_some(self.data)
    }
}

/// Decode an AZ value from any borrowed ObjectStream element source.
pub trait DecodeAzValue<'a>: Sized {
    fn decode_az_value<E>(element: &'a E) -> Result<Self, ObjectStreamValueError>
    where
        E: ElementValue + ?Sized;
}

impl<'a> DecodeAzValue<'a> for bool {
    #[inline]
    fn decode_az_value<E>(element: &'a E) -> Result<Self, ObjectStreamValueError>
    where
        E: ElementValue + ?Sized,
    {
        read_bool(element)
    }
}

impl<'a> DecodeAzValue<'a> for i8 {
    #[inline]
    fn decode_az_value<E>(element: &'a E) -> Result<Self, ObjectStreamValueError>
    where
        E: ElementValue + ?Sized,
    {
        read_i8(element)
    }
}

impl<'a> DecodeAzValue<'a> for i16 {
    #[inline]
    fn decode_az_value<E>(element: &'a E) -> Result<Self, ObjectStreamValueError>
    where
        E: ElementValue + ?Sized,
    {
        read_i16(element)
    }
}

impl<'a> DecodeAzValue<'a> for i32 {
    #[inline]
    fn decode_az_value<E>(element: &'a E) -> Result<Self, ObjectStreamValueError>
    where
        E: ElementValue + ?Sized,
    {
        read_i32(element)
    }
}

impl<'a> DecodeAzValue<'a> for i64 {
    #[inline]
    fn decode_az_value<E>(element: &'a E) -> Result<Self, ObjectStreamValueError>
    where
        E: ElementValue + ?Sized,
    {
        read_i64(element)
    }
}

impl<'a> DecodeAzValue<'a> for u8 {
    #[inline]
    fn decode_az_value<E>(element: &'a E) -> Result<Self, ObjectStreamValueError>
    where
        E: ElementValue + ?Sized,
    {
        read_u8(element)
    }
}

impl<'a> DecodeAzValue<'a> for u16 {
    #[inline]
    fn decode_az_value<E>(element: &'a E) -> Result<Self, ObjectStreamValueError>
    where
        E: ElementValue + ?Sized,
    {
        read_u16(element)
    }
}

impl<'a> DecodeAzValue<'a> for u32 {
    #[inline]
    fn decode_az_value<E>(element: &'a E) -> Result<Self, ObjectStreamValueError>
    where
        E: ElementValue + ?Sized,
    {
        read_u32(element)
    }
}

impl<'a> DecodeAzValue<'a> for u64 {
    #[inline]
    fn decode_az_value<E>(element: &'a E) -> Result<Self, ObjectStreamValueError>
    where
        E: ElementValue + ?Sized,
    {
        read_u64(element)
    }
}

impl<'a> DecodeAzValue<'a> for f32 {
    #[inline]
    fn decode_az_value<E>(element: &'a E) -> Result<Self, ObjectStreamValueError>
    where
        E: ElementValue + ?Sized,
    {
        read_f32_value(element)
    }
}

impl<'a> DecodeAzValue<'a> for f64 {
    #[inline]
    fn decode_az_value<E>(element: &'a E) -> Result<Self, ObjectStreamValueError>
    where
        E: ElementValue + ?Sized,
    {
        read_f64(element)
    }
}

impl<'a> DecodeAzValue<'a> for Uuid {
    #[inline]
    fn decode_az_value<E>(element: &'a E) -> Result<Self, ObjectStreamValueError>
    where
        E: ElementValue + ?Sized,
    {
        read_uuid(element)
    }
}

impl<'a> DecodeAzValue<'a> for &'a str {
    #[inline]
    fn decode_az_value<E>(element: &'a E) -> Result<Self, ObjectStreamValueError>
    where
        E: ElementValue + ?Sized,
    {
        read_string(element)
    }
}

impl<'a> DecodeAzValue<'a> for String {
    #[inline]
    fn decode_az_value<E>(element: &'a E) -> Result<Self, ObjectStreamValueError>
    where
        E: ElementValue + ?Sized,
    {
        read_string(element).map(str::to_owned)
    }
}

impl<'a> DecodeAzValue<'a> for Box<str> {
    #[inline]
    fn decode_az_value<E>(element: &'a E) -> Result<Self, ObjectStreamValueError>
    where
        E: ElementValue + ?Sized,
    {
        read_string(element).map(Box::<str>::from)
    }
}

impl<'a> DecodeAzValue<'a> for &'a [u8] {
    #[inline]
    fn decode_az_value<E>(element: &'a E) -> Result<Self, ObjectStreamValueError>
    where
        E: ElementValue + ?Sized,
    {
        read_byte_stream(element)
    }
}

impl<'a> DecodeAzValue<'a> for [f32; 2] {
    #[inline]
    fn decode_az_value<E>(element: &'a E) -> Result<Self, ObjectStreamValueError>
    where
        E: ElementValue + ?Sized,
    {
        read_vec2(element)
    }
}

impl<'a> DecodeAzValue<'a> for [f32; 3] {
    #[inline]
    fn decode_az_value<E>(element: &'a E) -> Result<Self, ObjectStreamValueError>
    where
        E: ElementValue + ?Sized,
    {
        read_vec3(element)
    }
}

impl<'a> DecodeAzValue<'a> for [f32; 4] {
    #[inline]
    fn decode_az_value<E>(element: &'a E) -> Result<Self, ObjectStreamValueError>
    where
        E: ElementValue + ?Sized,
    {
        read_float4(element)
    }
}

impl<'a> DecodeAzValue<'a> for [f32; 9] {
    #[inline]
    fn decode_az_value<E>(element: &'a E) -> Result<Self, ObjectStreamValueError>
    where
        E: ElementValue + ?Sized,
    {
        read_matrix3x3(element)
    }
}

impl<'a> DecodeAzValue<'a> for [f32; 12] {
    #[inline]
    fn decode_az_value<E>(element: &'a E) -> Result<Self, ObjectStreamValueError>
    where
        E: ElementValue + ?Sized,
    {
        read_transform(element)
    }
}

impl<'a> DecodeAzValue<'a> for [f32; 16] {
    #[inline]
    fn decode_az_value<E>(element: &'a E) -> Result<Self, ObjectStreamValueError>
    where
        E: ElementValue + ?Sized,
    {
        read_matrix4x4(element)
    }
}

/// Forward-only field lookup over an element source.
pub trait FieldAccess {
    type Value: ElementValue + ?Sized;

    fn field(&mut self, field: &str) -> Option<&Self::Value>;

    fn field_any<'f>(&mut self, fields: &[&'f str]) -> Option<(&'f str, &Self::Value)>;

    fn read<T, F>(&mut self, field: &str, read: F) -> Result<Option<T>, ObjectStreamValueError>
    where
        F: FnOnce(&Self::Value) -> Result<T, ObjectStreamValueError>,
    {
        self.field(field).map(read).transpose()
    }

    /// Read an optional string field, trimming surrounding whitespace
    /// and preserving the difference between a missing field and a
    /// present-but-empty field.
    fn read_trimmed_string<'a>(
        &'a mut self,
        field: &str,
    ) -> Result<Option<Option<&'a str>>, ObjectStreamValueError>
    where
        Self::Value: 'a,
    {
        self.field(field)
            .map(crate::value::read_trimmed_string)
            .transpose()
    }

    /// Read an optional string field as an owned value, trimming
    /// surrounding whitespace and preserving the difference between a
    /// missing field and a present-but-empty field.
    fn read_trimmed_string_owned(
        &mut self,
        field: &str,
    ) -> Result<Option<Option<String>>, ObjectStreamValueError> {
        self.read(field, crate::value::read_trimmed_string_owned)
    }

    /// Read an optional enum-like signed 32-bit scalar field.
    fn read_i32_scalar(&mut self, field: &str) -> Result<Option<i32>, ObjectStreamValueError> {
        self.read(field, crate::value::read_i32_scalar)
    }

    /// Read an optional unsigned 32-bit scalar field.
    fn read_u32_scalar(&mut self, field: &str) -> Result<Option<u32>, ObjectStreamValueError> {
        self.read(field, crate::value::read_u32_scalar)
    }

    fn read_any<'f, T, F>(
        &mut self,
        fields: &[&'f str],
        read: F,
    ) -> Result<Option<(&'f str, T)>, ObjectStreamValueError>
    where
        F: Fn(&Self::Value) -> Result<T, ObjectStreamValueError>,
    {
        self.field_any(fields)
            .map(|(field, value)| read(value).map(|value| (field, value)))
            .transpose()
    }

    /// Read the first matching string alias, trimming surrounding
    /// whitespace and preserving the matched alias plus the difference
    /// between a missing field and a present-but-empty field.
    fn read_trimmed_string_any<'a, 'f>(
        &'a mut self,
        fields: &[&'f str],
    ) -> Result<Option<(&'f str, Option<&'a str>)>, ObjectStreamValueError>
    where
        Self::Value: 'a,
    {
        self.field_any(fields)
            .map(|(field, value)| {
                crate::value::read_trimmed_string(value).map(|value| (field, value))
            })
            .transpose()
    }

    /// Read the first matching string alias as an owned value,
    /// trimming surrounding whitespace and preserving the matched alias
    /// plus the difference between a missing field and a
    /// present-but-empty field.
    fn read_trimmed_string_any_owned<'f>(
        &mut self,
        fields: &[&'f str],
    ) -> Result<Option<(&'f str, Option<String>)>, ObjectStreamValueError> {
        self.field_any(fields)
            .map(|(field, value)| {
                crate::value::read_trimmed_string_owned(value).map(|value| (field, value))
            })
            .transpose()
    }

    fn decode<'a, T>(&'a mut self, field: &str) -> Result<Option<T>, ObjectStreamValueError>
    where
        Self::Value: 'a,
        T: DecodeAzValue<'a>,
    {
        self.field(field).map(T::decode_az_value).transpose()
    }

    fn required_element<'a>(
        &'a mut self,
        field: &str,
    ) -> Result<&'a Self::Value, ObjectStreamValueError>
    where
        Self::Value: 'a,
    {
        self.field(field)
            .ok_or_else(|| ObjectStreamValueError::MissingField {
                field: field.to_string(),
            })
    }

    fn decode_any<'a, 's, 'f, T>(
        &'a mut self,
        fields: &'s [&'f str],
    ) -> Result<Option<(&'f str, T)>, ObjectStreamValueError>
    where
        Self::Value: 'a,
        T: DecodeAzValue<'a>,
    {
        self.field_any(fields)
            .map(|(field, value)| T::decode_az_value(value).map(|value| (field, value)))
            .transpose()
    }

    fn required_any<'a, 's, 'f, T>(
        &'a mut self,
        fields: &'s [&'f str],
    ) -> Result<(&'f str, T), ObjectStreamValueError>
    where
        Self::Value: 'a,
        T: DecodeAzValue<'a>,
    {
        self.decode_any(fields)?
            .ok_or_else(|| ObjectStreamValueError::MissingField {
                field: fields.join("|"),
            })
    }

    fn decode_any_or_default<'a, 's, 'f, T>(
        &'a mut self,
        fields: &'s [&'f str],
    ) -> Result<T, ObjectStreamValueError>
    where
        Self::Value: 'a,
        T: DecodeAzValue<'a> + Default,
    {
        self.decode_any(fields)
            .map(|value| value.map_or_else(T::default, |(_, value)| value))
    }

    fn required<'a, T>(&'a mut self, field: &str) -> Result<T, ObjectStreamValueError>
    where
        Self::Value: 'a,
        T: DecodeAzValue<'a>,
    {
        self.decode(field)?
            .ok_or_else(|| ObjectStreamValueError::MissingField {
                field: field.to_string(),
            })
    }

    fn decode_or<'a, T>(&'a mut self, field: &str, default: T) -> Result<T, ObjectStreamValueError>
    where
        Self::Value: 'a,
        T: DecodeAzValue<'a>,
    {
        self.decode(field).map(|value| value.unwrap_or(default))
    }

    fn decode_or_default<'a, T>(&'a mut self, field: &str) -> Result<T, ObjectStreamValueError>
    where
        Self::Value: 'a,
        T: DecodeAzValue<'a> + Default,
    {
        self.decode(field).map(Option::unwrap_or_default)
    }
}

/// Forward-only field lookup over an element's children.
///
/// This is useful when a decoder reads fields in serialized order:
/// each successful lookup advances the cursor past the matched child.
#[derive(Debug, Clone)]
pub struct FieldCursor<'a> {
    remaining: &'a [Element],
}

impl<'a> FieldCursor<'a> {
    #[inline]
    #[must_use]
    pub const fn new(children: &'a [Element]) -> Self {
        Self {
            remaining: children,
        }
    }

    #[inline]
    #[must_use]
    pub fn from_element(element: &'a Element) -> Self {
        Self::new(element.children())
    }

    #[inline]
    #[must_use]
    pub const fn remaining(&self) -> &'a [Element] {
        self.remaining
    }

    #[inline]
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.remaining.is_empty()
    }

    pub fn find(&mut self, field: &str) -> Option<&'a Element> {
        let index = self
            .remaining
            .iter()
            .position(|child| field_eq(child, field))?;
        let (consumed, rest) = self.remaining.split_at(index + 1);
        self.remaining = rest;
        consumed.last()
    }

    pub fn find_any<'s, 'f>(&mut self, fields: &'s [&'f str]) -> Option<(&'f str, &'a Element)> {
        let (index, field) = self
            .remaining
            .iter()
            .enumerate()
            .find_map(|(index, child)| matching_field(child, fields).map(|field| (index, field)))?;
        let (consumed, rest) = self.remaining.split_at(index + 1);
        self.remaining = rest;
        consumed.last().map(|child| (field, child))
    }
}

impl<'a> Iterator for FieldCursor<'a> {
    type Item = &'a Element;

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        let (first, rest) = self.remaining.split_first()?;
        self.remaining = rest;
        Some(first)
    }
}

impl FieldAccess for FieldCursor<'_> {
    type Value = Element;

    #[inline]
    fn field(&mut self, field: &str) -> Option<&Self::Value> {
        FieldCursor::find(self, field)
    }

    #[inline]
    fn field_any<'f>(&mut self, fields: &[&'f str]) -> Option<(&'f str, &Self::Value)> {
        FieldCursor::find_any(self, fields)
    }
}

/// Non-consuming field lookup over an element's direct children.
///
/// Unlike [`FieldCursor`], repeated lookups always scan the full child
/// list. Use this when reflected fields may be read independently rather
/// than in serialized order.
#[derive(Debug, Clone)]
pub struct ElementFields<'a> {
    element: &'a Element,
}

impl<'a> ElementFields<'a> {
    #[inline]
    #[must_use]
    pub const fn new(element: &'a Element) -> Self {
        Self { element }
    }

    #[inline]
    #[must_use]
    pub const fn element(&self) -> &'a Element {
        self.element
    }
}

impl FieldAccess for ElementFields<'_> {
    type Value = Element;

    #[inline]
    fn field(&mut self, field: &str) -> Option<&Self::Value> {
        child_by_field(self.element, field)
    }

    #[inline]
    fn field_any<'f>(&mut self, fields: &[&'f str]) -> Option<(&'f str, &Self::Value)> {
        self.element
            .children()
            .iter()
            .find_map(|child| matching_field(child, fields).map(|field| (field, child)))
    }
}

#[inline]
#[must_use]
pub fn child_by_field<'a>(element: &'a Element, field: &str) -> Option<&'a Element> {
    element
        .children()
        .iter()
        .find(|child| field_eq(child, field))
}

/// Find the first child whose field name matches any alias in `fields`.
///
/// Search order follows the serialized child order, not the alias order.
#[inline]
#[must_use]
pub fn child_by_field_any<'a>(element: &'a Element, fields: &[&str]) -> Option<&'a Element> {
    element
        .children()
        .iter()
        .find(|child| matching_field(child, fields).is_some())
}

/// Find a named child, falling back to a fixed child index.
///
/// Some reflected ObjectStreams preserve a stable child order
/// even when field names are missing or renamed. This helper keeps the
/// named lookup as the preferred path while supporting those ordered
/// payloads without duplicating fallback logic in callers.
#[inline]
#[must_use]
pub fn child_by_field_or_index<'a>(
    element: &'a Element,
    field: &str,
    index: usize,
) -> Option<&'a Element> {
    child_by_field(element, field).or_else(|| element.children().get(index))
}

/// Read a named child, falling back to a fixed child index when the
/// field name is absent.
pub fn read_child_by_field_or_index<T>(
    element: &Element,
    field: &str,
    index: usize,
    read: impl FnOnce(&Element) -> Result<T, ObjectStreamValueError>,
) -> Result<Option<T>, ObjectStreamValueError> {
    child_by_field_or_index(element, field, index)
        .map(read)
        .transpose()
}

/// Decode a named child, falling back to a fixed child index when the
/// field name is absent.
pub fn decode_child_by_field_or_index<'a, T>(
    element: &'a Element,
    field: &str,
    index: usize,
) -> Result<Option<T>, ObjectStreamValueError>
where
    T: DecodeAzValue<'a>,
{
    child_by_field_or_index(element, field, index)
        .map(T::decode_az_value)
        .transpose()
}

pub fn read_bool(element: &(impl ElementValue + ?Sized)) -> Result<bool, ObjectStreamValueError> {
    let data = typed_data(element, BOOL, "bool")?;
    fixed_bytes::<1>(element, data).map(|bytes| bytes[0] != 0)
}

pub fn read_i8(element: &(impl ElementValue + ?Sized)) -> Result<i8, ObjectStreamValueError> {
    if !matches!(element.id(), CHAR | SIGNED_CHAR | AZ_S8) {
        return Err(ObjectStreamValueError::UnexpectedType {
            field: field_name(element),
            expected: "AZ::s8",
            actual: element.id(),
        });
    }
    fixed_bytes::<1>(element, data(element)?).map(i8::from_be_bytes)
}

pub fn read_i16(element: &(impl ElementValue + ?Sized)) -> Result<i16, ObjectStreamValueError> {
    let data = typed_data(element, SHORT, "short")?;
    fixed_bytes::<2>(element, data).map(i16::from_be_bytes)
}

pub fn read_i32(element: &(impl ElementValue + ?Sized)) -> Result<i32, ObjectStreamValueError> {
    let data = typed_data(element, INT, "int")?;
    fixed_bytes::<4>(element, data).map(i32::from_be_bytes)
}

pub fn read_i64(element: &(impl ElementValue + ?Sized)) -> Result<i64, ObjectStreamValueError> {
    if !matches!(element.id(), LONG | AZ_S64) {
        return Err(ObjectStreamValueError::UnexpectedType {
            field: field_name(element),
            expected: "AZ::s64",
            actual: element.id(),
        });
    }
    fixed_bytes::<8>(element, data(element)?).map(i64::from_be_bytes)
}

pub fn read_u32(element: &(impl ElementValue + ?Sized)) -> Result<u32, ObjectStreamValueError> {
    let data = typed_data(element, UNSIGNED_INT, "unsigned int")?;
    fixed_bytes::<4>(element, data).map(u32::from_be_bytes)
}

pub fn read_u8(element: &(impl ElementValue + ?Sized)) -> Result<u8, ObjectStreamValueError> {
    let data = typed_data(element, UNSIGNED_CHAR, "unsigned char")?;
    fixed_bytes::<1>(element, data).map(u8::from_be_bytes)
}

pub fn read_u16(element: &(impl ElementValue + ?Sized)) -> Result<u16, ObjectStreamValueError> {
    let data = typed_data(element, UNSIGNED_SHORT, "unsigned short")?;
    fixed_bytes::<2>(element, data).map(u16::from_be_bytes)
}

/// Read an enum-like signed 32-bit scalar.
///
/// Reflected enum/class UUIDs often use the same four big-endian value
/// bytes as `int`. For plain AZ `int` this delegates to [`read_i32`];
/// for any other type UUID it accepts exactly four payload bytes and
/// interprets them as `i32`.
pub fn read_i32_scalar(
    element: &(impl ElementValue + ?Sized),
) -> Result<i32, ObjectStreamValueError> {
    if element.id() == INT {
        return read_i32(element);
    }

    fixed_bytes::<4>(element, data(element)?).map(i32::from_be_bytes)
}

/// Read a strict reflected `unsigned int` scalar.
#[inline]
pub fn read_u32_scalar(
    element: &(impl ElementValue + ?Sized),
) -> Result<u32, ObjectStreamValueError> {
    read_u32(element)
}

/// Read a reflected `unsigned short`, accepting widened
/// `unsigned int` payloads for fields that changed width.
///
/// The widened form preserves the historic caller behavior by casting
/// the `u32` payload down to `u16`.
pub fn read_u16_scalar(
    element: &(impl ElementValue + ?Sized),
) -> Result<u16, ObjectStreamValueError> {
    match element.id() {
        UNSIGNED_SHORT => read_u16(element),
        UNSIGNED_INT => read_u32(element).and_then(|value| {
            u16::try_from(value).map_err(|_| ObjectStreamValueError::InvalidValue {
                field: field_name(element),
                expected: "unsigned int value that fits unsigned short",
            })
        }),
        actual => Err(ObjectStreamValueError::UnexpectedType {
            field: field_name(element),
            expected: "unsigned short or unsigned int",
            actual,
        }),
    }
}

pub fn read_u64(element: &(impl ElementValue + ?Sized)) -> Result<u64, ObjectStreamValueError> {
    if !matches!(element.id(), UNSIGNED_LONG | AZ_U64) {
        return Err(ObjectStreamValueError::UnexpectedType {
            field: field_name(element),
            expected: "unsigned long or AZ::u64",
            actual: element.id(),
        });
    }
    fixed_bytes::<8>(element, data(element)?).map(u64::from_be_bytes)
}

/// Read exactly `N` raw value bytes without validating the element type.
///
/// Use this for reflected wrapper types whose UUID carries the domain
/// meaning but whose value bytes are still a fixed-width primitive payload.
pub fn read_payload_bytes<const N: usize>(
    element: &(impl ElementValue + ?Sized),
) -> Result<[u8; N], ObjectStreamValueError> {
    fixed_bytes(element, data(element)?)
}

/// Read exactly `N` raw value bytes when a value payload is present.
///
/// Missing value bytes are reported as `Ok(None)` instead of
/// [`ObjectStreamValueError::MissingData`]. Use this for reflected
/// wrappers where no payload means a domain default, but malformed
/// present payloads should still fail.
pub fn read_optional_payload_bytes<const N: usize>(
    element: &(impl ElementValue + ?Sized),
) -> Result<Option<[u8; N]>, ObjectStreamValueError> {
    element
        .data()
        .map(|data| fixed_bytes(element, data))
        .transpose()
}

/// Read a raw big-endian `u64` payload without validating the element type.
pub fn read_u64_payload(
    element: &(impl ElementValue + ?Sized),
) -> Result<u64, ObjectStreamValueError> {
    read_payload_bytes::<8>(element).map(u64::from_be_bytes)
}

/// Read a reflected unsigned 64-bit scalar, accepting widened
/// `unsigned int` payloads used by older editor-authored fields.
pub fn read_u64_scalar(
    element: &(impl ElementValue + ?Sized),
) -> Result<u64, ObjectStreamValueError> {
    match element.id() {
        UNSIGNED_LONG | AZ_U64 => read_u64(element),
        UNSIGNED_INT => read_u32(element).map(u64::from),
        actual => Err(ObjectStreamValueError::UnexpectedType {
            field: field_name(element),
            expected: "unsigned long, AZ::u64, or unsigned int",
            actual,
        }),
    }
}

pub fn read_uuid(element: &(impl ElementValue + ?Sized)) -> Result<Uuid, ObjectStreamValueError> {
    let data = typed_data(element, AZ_UUID, "AZ::Uuid")?;
    fixed_bytes::<16>(element, data).map(Uuid::from_bytes)
}

/// Decode a reflected `AZ::EntityId`.
///
/// ObjectStream XML uses lowercase `id`; binary payloads may use `Id`,
/// and some UI payloads use `ID`. The reader accepts all known aliases
/// and keeps that format detail out of callers.
pub fn read_entity_id(element: &Element) -> Result<u64, ObjectStreamValueError> {
    if element.id() != &ENTITY_ID {
        return Err(ObjectStreamValueError::UnexpectedType {
            field: field_name(element),
            expected: "AZ::EntityId",
            actual: *element.id(),
        });
    }

    let mut fields = ElementFields::new(element);
    fields.required_any(&["id", "Id", "ID"]).map(|(_, id)| id)
}

/// Decode entity ids from the `AZ::EntityId` children of a reflected vector.
///
/// Non-entity children are ignored, matching ObjectStream vector
/// payloads where bookkeeping or unrelated fields may appear beside
/// the actual values.
pub fn read_entity_id_vector(element: &Element) -> Result<Vec<u64>, ObjectStreamValueError> {
    element
        .children()
        .iter()
        .filter(|child| child.id() == &ENTITY_ID)
        .map(read_entity_id)
        .collect()
}

/// Decode a reflected `AZ::Crc32`.
///
/// Accepts `value` and `Value` payload field spellings.
pub fn read_crc32(element: &Element) -> Result<u32, ObjectStreamValueError> {
    if element.id() != &CRC32 {
        return Err(ObjectStreamValueError::UnexpectedType {
            field: field_name(element),
            expected: "AZ::Crc32",
            actual: *element.id(),
        });
    }

    let mut fields = ElementFields::new(element);
    fields
        .required_any(&["value", "Value"])
        .map(|(_, value)| value)
}

/// Decode a CRC32 value serialized either as reflected `AZ::Crc32` or
/// as an `unsigned int` payload.
pub fn read_crc32_or_u32(element: &Element) -> Result<u32, ObjectStreamValueError> {
    match *element.id() {
        CRC32 => read_crc32(element),
        UNSIGNED_INT => read_u32(element),
        actual => Err(ObjectStreamValueError::UnexpectedType {
            field: field_name(element),
            expected: "AZ::Crc32 or unsigned int",
            actual,
        }),
    }
}

/// Decode CRC32 values from the `AZ::Crc32` children of a reflected vector.
///
/// Non-CRC32 children are ignored, matching reflected vector payloads
/// where bookkeeping or unrelated fields may appear beside the actual
/// values.
pub fn read_crc32_vector(element: &Element) -> Result<Vec<u32>, ObjectStreamValueError> {
    element
        .children()
        .iter()
        .filter(|child| child.id() == &CRC32)
        .map(read_crc32)
        .collect()
}

pub fn read_f32(element: &(impl ElementValue + ?Sized)) -> Result<f32, ObjectStreamValueError> {
    let data = typed_data(element, FLOAT, "float")?;
    fixed_bytes::<4>(element, data).map(f32::from_be_bytes)
}

/// Read a Rust `f32` from either a primitive float or AZ math `VectorFloat`.
pub fn read_f32_value(
    element: &(impl ElementValue + ?Sized),
) -> Result<f32, ObjectStreamValueError> {
    match element.id() {
        FLOAT => read_f32(element),
        VECTOR_FLOAT => read_vector_float(element),
        actual => Err(ObjectStreamValueError::UnexpectedType {
            field: field_name(element),
            expected: "float or VectorFloat",
            actual,
        }),
    }
}

pub fn read_f64(element: &(impl ElementValue + ?Sized)) -> Result<f64, ObjectStreamValueError> {
    let data = typed_data(element, DOUBLE, "double")?;
    fixed_bytes::<8>(element, data).map(f64::from_be_bytes)
}

/// Read a reflected numeric scalar as `f64`.
///
/// ObjectStreams commonly serialize numeric properties as either
/// `double` or `float`; callers that only need the semantic number
/// should not have to duplicate that width choice.
pub fn read_f64_scalar(
    element: &(impl ElementValue + ?Sized),
) -> Result<f64, ObjectStreamValueError> {
    match element.id() {
        DOUBLE => read_f64(element),
        FLOAT => read_f32(element).map(f64::from),
        actual => Err(ObjectStreamValueError::UnexpectedType {
            field: field_name(element),
            expected: "double or float",
            actual,
        }),
    }
}

pub fn read_string(element: &(impl ElementValue + ?Sized)) -> Result<&str, ObjectStreamValueError> {
    if !matches!(
        element.id(),
        AZSTD_STRING | AZSTD_BASIC_STRING | AZSTD_STRING_XML_ALIAS
    ) {
        return Err(ObjectStreamValueError::UnexpectedType {
            field: field_name(element),
            expected: "AZStd::string",
            actual: element.id(),
        });
    }

    let data = data(element)?;
    std::str::from_utf8(data).map_err(|source| ObjectStreamValueError::Utf8 {
        field: field_name(element),
        source,
    })
}

/// Read an `AZStd::string`, trim surrounding whitespace, and return
/// `None` when the trimmed value is empty.
pub fn read_trimmed_string(
    element: &(impl ElementValue + ?Sized),
) -> Result<Option<&str>, ObjectStreamValueError> {
    let value = read_string(element)?.trim();
    Ok((!value.is_empty()).then_some(value))
}

/// Read an `AZStd::string`, trim surrounding whitespace, and return an
/// owned value when the trimmed value is not empty.
pub fn read_trimmed_string_owned(
    element: &(impl ElementValue + ?Sized),
) -> Result<Option<String>, ObjectStreamValueError> {
    read_trimmed_string(element).map(|value| value.map(str::to_string))
}

/// Read an optional `AZStd::string` payload, trimming surrounding
/// whitespace and treating missing value bytes as `None`.
pub fn read_optional_trimmed_string(
    element: &(impl ElementValue + ?Sized),
) -> Result<Option<&str>, ObjectStreamValueError> {
    if element.data().is_none() {
        return Ok(None);
    }
    read_trimmed_string(element)
}

/// Read an optional `AZStd::string` payload as an owned value, trimming
/// surrounding whitespace and treating missing value bytes as `None`.
pub fn read_optional_trimmed_string_owned(
    element: &(impl ElementValue + ?Sized),
) -> Result<Option<String>, ObjectStreamValueError> {
    read_optional_trimmed_string(element).map(|value| value.map(str::to_string))
}

/// Read string-like children, trimming and dropping empty values.
///
/// Non-string children are ignored. This matches AZ reflected vector
/// payloads where sibling fields may carry bookkeeping beside the
/// actual string entries.
pub fn read_string_vector(element: &Element) -> Result<Vec<&str>, ObjectStreamValueError> {
    let mut values = Vec::new();
    for child in element
        .children()
        .iter()
        .filter(|child| matches!(*child.id(), AZSTD_STRING | AZSTD_BASIC_STRING))
    {
        if let Some(value) = read_trimmed_string(child)? {
            values.push(value);
        }
    }
    Ok(values)
}

/// Read string-like children as owned values, trimming and dropping
/// empty values.
pub fn read_string_vector_owned(element: &Element) -> Result<Vec<String>, ObjectStreamValueError> {
    let mut values = Vec::new();
    for child in element
        .children()
        .iter()
        .filter(|child| matches!(*child.id(), AZSTD_STRING | AZSTD_BASIC_STRING))
    {
        if let Some(value) = read_trimmed_string(child)? {
            values.push(value.to_string());
        }
    }
    Ok(values)
}

/// Read bool-like children from an ObjectStream vector.
///
/// Non-bool children are ignored, matching reflected vector payloads
/// where sibling bookkeeping may sit beside actual entries.
pub fn read_bool_vector(element: &Element) -> Result<Vec<bool>, ObjectStreamValueError> {
    element
        .children()
        .iter()
        .filter(|child| child.id() == &BOOL)
        .map(read_bool)
        .collect()
}

/// Read numeric children from an ObjectStream vector as `f64`.
///
/// Accepts both `double` and `float` elements and ignores unrelated
/// children, mirroring the scalar reader's width normalization.
pub fn read_f64_vector(element: &Element) -> Result<Vec<f64>, ObjectStreamValueError> {
    element
        .children()
        .iter()
        .filter(|child| matches!(*child.id(), DOUBLE | FLOAT))
        .map(read_f64_scalar)
        .collect()
}

/// Read numeric children from an ObjectStream vector as `f32`.
///
/// Accepts both `float` and `double` elements and narrows doubles to match
/// reflected `AZStd::vector<float>` storage.
pub fn read_f32_vector(element: &Element) -> Result<Vec<f32>, ObjectStreamValueError> {
    read_f64_vector(element).and_then(|values| {
        values
            .into_iter()
            .map(|value| {
                if value < f64::from(f32::MIN) || value > f64::from(f32::MAX) {
                    return Err(ObjectStreamValueError::InvalidValue {
                        field: field_name(element),
                        expected: "float vector value in f32 range",
                    });
                }
                Ok(value as f32)
            })
            .collect()
    })
}

pub fn read_byte_stream(
    element: &(impl ElementValue + ?Sized),
) -> Result<&[u8], ObjectStreamValueError> {
    typed_data(element, BYTE_STREAM, "ByteStream")
}

pub fn read_vector_float(
    element: &(impl ElementValue + ?Sized),
) -> Result<f32, ObjectStreamValueError> {
    let data = typed_data(element, VECTOR_FLOAT, "VectorFloat")?;
    fixed_bytes::<4>(element, data).map(f32::from_be_bytes)
}

pub fn read_vec3(
    element: &(impl ElementValue + ?Sized),
) -> Result<[f32; 3], ObjectStreamValueError> {
    let data = typed_data(element, VECTOR3, "Vector3")?;
    read_f32_array::<3>(element, data)
}

pub fn read_vec4(
    element: &(impl ElementValue + ?Sized),
) -> Result<[f32; 4], ObjectStreamValueError> {
    let data = typed_data(element, VECTOR4, "Vector4")?;
    read_f32_array::<4>(element, data)
}

pub fn read_vec2(
    element: &(impl ElementValue + ?Sized),
) -> Result<[f32; 2], ObjectStreamValueError> {
    let data = typed_data(element, VECTOR2, "Vector2")?;
    read_f32_array::<2>(element, data)
}

pub fn read_color(
    element: &(impl ElementValue + ?Sized),
) -> Result<[f32; 4], ObjectStreamValueError> {
    let data = typed_data(element, COLOR, "Color")?;
    read_f32_array::<4>(element, data)
}

pub fn read_quat(
    element: &(impl ElementValue + ?Sized),
) -> Result<[f32; 4], ObjectStreamValueError> {
    let data = typed_data(element, QUATERNION, "Quaternion")?;
    read_f32_array::<4>(element, data)
}

/// Read any AZ leaf serializer whose Rust representation is four floats.
pub fn read_float4(
    element: &(impl ElementValue + ?Sized),
) -> Result<[f32; 4], ObjectStreamValueError> {
    match element.id() {
        VECTOR4 => read_vec4(element),
        COLOR => read_color(element),
        QUATERNION => read_quat(element),
        actual => Err(ObjectStreamValueError::UnexpectedType {
            field: field_name(element),
            expected: "Vector4, Color, or Quaternion",
            actual,
        }),
    }
}

pub fn read_transform(
    element: &(impl ElementValue + ?Sized),
) -> Result<[f32; 12], ObjectStreamValueError> {
    // Transform version 0 stores a 3x4 matrix as 12 floats.
    let data = typed_data(element, TRANSFORM, "Transform")?;
    read_f32_array::<12>(element, data)
}

pub fn read_matrix3x3(
    element: &(impl ElementValue + ?Sized),
) -> Result<[f32; 9], ObjectStreamValueError> {
    let data = typed_data(element, MATRIX3X3, "Matrix3x3")?;
    read_f32_array::<9>(element, data)
}

pub fn read_matrix4x4(
    element: &(impl ElementValue + ?Sized),
) -> Result<[f32; 16], ObjectStreamValueError> {
    let data = typed_data(element, MATRIX4X4, "Matrix4x4")?;
    read_f32_array::<16>(element, data)
}

/// Decode a known ObjectStream leaf value into display/search text.
///
/// This covers AZ types with fixed payload serializers. Reflected
/// objects such as `AZ::EntityId`, `AZ::Crc32`, `AZ::Aabb`, and
/// `ColorF` are represented by child elements and are decoded by
/// walking those children instead of treating the parent as a byte blob.
pub fn read_leaf_text(element: &(impl ElementValue + ?Sized)) -> Option<String> {
    match element.id() {
        BOOL => read_bool(element).ok().map(|value| value.to_string()),
        CHAR | SIGNED_CHAR | AZ_S8 => read_i8(element).ok().map(|value| value.to_string()),
        SHORT => read_i16(element).ok().map(|value| value.to_string()),
        INT => read_i32(element).ok().map(|value| value.to_string()),
        LONG | AZ_S64 => read_i64(element).ok().map(|value| value.to_string()),
        UNSIGNED_CHAR => read_u8(element).ok().map(|value| value.to_string()),
        UNSIGNED_SHORT => read_u16(element).ok().map(|value| value.to_string()),
        UNSIGNED_INT => read_u32(element).ok().map(|value| value.to_string()),
        UNSIGNED_LONG | AZ_U64 => read_u64(element).ok().map(|value| value.to_string()),
        FLOAT => read_f32(element).ok().map(format_f32),
        DOUBLE => read_f64(element).ok().map(format_f64),
        AZ_UUID => read_uuid(element).ok().map(|value| value.to_string()),
        AZSTD_STRING | AZSTD_BASIC_STRING | AZSTD_STRING_XML_ALIAS => {
            read_string(element).ok().map(str::to_string)
        }
        BYTE_STREAM => read_byte_stream(element).ok().map(format_hex_preview),
        VECTOR_FLOAT => read_vector_float(element).ok().map(format_f32),
        VECTOR2 => read_vec2(element).ok().map(format_f32_array),
        VECTOR3 => read_vec3(element).ok().map(format_f32_array),
        VECTOR4 => read_vec4(element).ok().map(format_f32_array),
        COLOR => read_color(element).ok().map(format_f32_array),
        QUATERNION => read_quat(element).ok().map(format_f32_array),
        TRANSFORM => read_transform(element).ok().map(format_f32_array),
        MATRIX3X3 => read_matrix3x3(element).ok().map(format_f32_array),
        MATRIX4X4 => read_matrix4x4(element).ok().map(format_f32_array),
        _ => None,
    }
}

fn format_f32(value: f32) -> String {
    format!("{value:.7}")
}

fn format_f64(value: f64) -> String {
    format!("{value:.7}")
}

fn format_f32_array<const N: usize>(values: [f32; N]) -> String {
    values
        .iter()
        .map(|value| format!("{value:.7}"))
        .collect::<Vec<_>>()
        .join(" ")
}

#[must_use]
pub fn format_hex_preview(data: &[u8]) -> String {
    const MAX_BYTES: usize = 96;
    let mut out = String::new();
    for (index, byte) in data.iter().take(MAX_BYTES).enumerate() {
        if index > 0 {
            out.push(' ');
        }
        let _ = write!(out, "{byte:02X}");
    }
    if data.len() > MAX_BYTES {
        let len = data.len();
        let _ = write!(out, " ... ({len} bytes)");
    }
    out
}

fn typed_data<'a, E: ElementValue + ?Sized>(
    element: &'a E,
    expected: Uuid,
    expected_name: &'static str,
) -> Result<&'a [u8], ObjectStreamValueError> {
    if element.id() != expected {
        return Err(ObjectStreamValueError::UnexpectedType {
            field: field_name(element),
            expected: expected_name,
            actual: element.id(),
        });
    }
    data(element)
}

fn data(element: &(impl ElementValue + ?Sized)) -> Result<&[u8], ObjectStreamValueError> {
    element
        .data()
        .ok_or_else(|| ObjectStreamValueError::MissingData {
            field: field_name(element),
        })
}

fn fixed_bytes<const N: usize>(
    element: &(impl ElementValue + ?Sized),
    data: &[u8],
) -> Result<[u8; N], ObjectStreamValueError> {
    data.try_into().map_err(
        |_: TryFromSliceError| ObjectStreamValueError::InvalidLength {
            field: field_name(element),
            expected: N,
            actual: data.len(),
        },
    )
}

fn read_f32_array<const N: usize>(
    element: &(impl ElementValue + ?Sized),
    data: &[u8],
) -> Result<[f32; N], ObjectStreamValueError> {
    if data.len() != N * 4 {
        return Err(ObjectStreamValueError::InvalidLength {
            field: field_name(element),
            expected: N * 4,
            actual: data.len(),
        });
    }

    let mut values = [0.0; N];
    for (slot, bytes) in values.iter_mut().zip(data.chunks_exact(4)) {
        *slot = f32::from_be_bytes(bytes.try_into().expect("chunks_exact width is four"));
    }
    Ok(values)
}

/// Human-readable field name for error reporting.
///
/// Returns `"<unnamed>"` for elements without resolved field metadata.
pub fn field_name(element: &(impl ElementValue + ?Sized)) -> String {
    element.field_name().unwrap_or("<unnamed>").to_string()
}

fn field_eq(element: &Element, field: &str) -> bool {
    element.field().is_some_and(|value| value.as_str() == field)
}

fn matching_field<'f>(element: &Element, fields: &[&'f str]) -> Option<&'f str> {
    let actual = element.field()?;
    fields
        .iter()
        .copied()
        .find(|field| actual.as_str() == *field)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ST_BINARYFLAG_HAS_VALUE;
    use crate::types;
    use arcstr::ArcStr;

    #[test]
    fn reads_scalar_leaf_values() {
        assert!(read_bool(&leaf("Visible", types::BOOL, vec![1])).unwrap());
        assert_eq!(
            read_i8(&leaf("Small", types::AZ_S8, (-3_i8).to_be_bytes())).unwrap(),
            -3
        );
        assert_eq!(
            read_i16(&leaf("Short", types::SHORT, (-9_i16).to_be_bytes())).unwrap(),
            -9
        );
        assert_eq!(
            read_i32(&leaf("ProjectionType", types::INT, 2_i32.to_be_bytes())).unwrap(),
            2
        );
        assert_eq!(
            read_u32(&leaf(
                "SortPriority",
                types::UNSIGNED_INT,
                16_u32.to_be_bytes()
            ))
            .unwrap(),
            16
        );
        assert_eq!(
            read_u8(&leaf("AreaType", types::UNSIGNED_CHAR, [2])).unwrap(),
            2
        );
        assert_eq!(
            read_i64(&leaf("DelayMs", types::AZ_S64, (-1500_i64).to_be_bytes())).unwrap(),
            -1500
        );
        assert_eq!(
            read_u64(&leaf("Id", types::AZ_U64, 123_u64.to_be_bytes())).unwrap(),
            123
        );
        assert_eq!(
            read_u64_payload(&leaf(
                "m_objectiveInstanceId",
                Uuid::from_u128(0x11111111_2222_3333_4444_555555555555),
                0x9876_u64.to_be_bytes()
            ))
            .unwrap(),
            0x9876
        );
        assert_eq!(
            read_payload_bytes::<1>(&leaf(
                "m_achievementServerState",
                Uuid::from_u128(0x33333333_4444_5555_6666_777777777777),
                [2]
            ))
            .unwrap(),
            [2]
        );
        assert_eq!(
            read_payload_bytes::<12>(&leaf(
                "m_collisionFilterOverride",
                Uuid::from_u128(0x22222222_3333_4444_5555_666666666666),
                [1_u8, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12]
            ))
            .unwrap(),
            [1_u8, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12]
        );
        assert_eq!(
            read_optional_payload_bytes::<24>(&leaf(
                "m_actorId",
                Uuid::from_u128(0x33333333_4444_5555_6666_777777777777),
                [
                    1_u8, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21,
                    22, 23, 24,
                ]
            ))
            .unwrap(),
            Some([
                1_u8, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21, 22,
                23, 24,
            ])
        );
        assert_eq!(
            read_optional_payload_bytes::<24>(&Element::new(Uuid::from_u128(
                0x33333333_4444_5555_6666_777777777777
            )))
            .unwrap(),
            None
        );
        assert_eq!(
            read_uuid(&leaf(
                "guid",
                types::AZ_UUID,
                uuid::Uuid::from_u128(0x11111111_2222_3333_4444_555555555555).as_bytes()
            ))
            .unwrap(),
            uuid::Uuid::from_u128(0x11111111_2222_3333_4444_555555555555)
        );
        assert_eq!(
            read_f32(&leaf("Depth", types::FLOAT, 1.25_f32.to_be_bytes())).unwrap(),
            1.25
        );
        assert_eq!(
            read_f64(&leaf("value", types::DOUBLE, 2.5_f64.to_be_bytes())).unwrap(),
            2.5
        );
    }

    #[test]
    fn reads_enum_like_scalar_payloads() {
        let enum_type = Uuid::from_u128(0x11111111_2222_3333_4444_555555555555);

        assert_eq!(
            read_i32_scalar(&leaf("Mode", types::INT, (-7_i32).to_be_bytes())).unwrap(),
            -7
        );
        assert_eq!(
            read_i32_scalar(&leaf("Mode", enum_type, 42_i32.to_be_bytes())).unwrap(),
            42
        );

        let element = Element::new(types::AZSTD_VECTOR).with_children([leaf(
            "Mode",
            enum_type,
            3_i32.to_be_bytes(),
        )]);
        let mut fields = FieldCursor::from_element(&element);
        assert_eq!(fields.read_i32_scalar("Mode").unwrap(), Some(3));

        let element = Element::new(types::AZSTD_VECTOR).with_children([leaf(
            "Layer",
            types::UNSIGNED_INT,
            9_u32.to_be_bytes(),
        )]);
        let mut fields = FieldCursor::from_element(&element);
        assert_eq!(fields.read_u32_scalar("Layer").unwrap(), Some(9));

        assert_eq!(
            read_u32_scalar(&leaf("Layer", types::UNSIGNED_INT, 9_u32.to_be_bytes())).unwrap(),
            9
        );
        assert_eq!(
            read_u16_scalar(&leaf(
                "Granularity",
                types::UNSIGNED_SHORT,
                12_u16.to_be_bytes()
            ))
            .unwrap(),
            12
        );
        assert_eq!(
            read_u16_scalar(&leaf(
                "Granularity",
                types::UNSIGNED_INT,
                13_u32.to_be_bytes()
            ))
            .unwrap(),
            13
        );
        assert_eq!(
            read_u64_scalar(&leaf("Id", types::AZ_U64, 0xCAFE_u64.to_be_bytes())).unwrap(),
            0xCAFE
        );
        assert_eq!(
            read_u64_scalar(&leaf("Id", types::UNSIGNED_INT, 0xBEEF_u32.to_be_bytes())).unwrap(),
            0xBEEF
        );
        assert_eq!(
            read_f64_scalar(&leaf("Delay", types::DOUBLE, 2.5_f64.to_be_bytes())).unwrap(),
            2.5
        );
        assert_eq!(
            read_f64_scalar(&leaf("Delay", types::FLOAT, 1.25_f32.to_be_bytes())).unwrap(),
            1.25
        );
    }

    #[test]
    fn reads_wrapped_az_ids() {
        assert_eq!(
            read_entity_id(&Element::new(types::ENTITY_ID).with_children([leaf(
                "id",
                types::AZ_U64,
                0xCAFE_u64.to_be_bytes()
            )]))
            .unwrap(),
            0xCAFE
        );
        assert_eq!(
            read_entity_id(&Element::new(types::ENTITY_ID).with_children([leaf(
                "Id",
                types::AZ_U64,
                0xBEEF_u64.to_be_bytes()
            )]))
            .unwrap(),
            0xBEEF
        );
        assert_eq!(
            read_entity_id(&Element::new(types::ENTITY_ID).with_children([leaf(
                "ID",
                types::AZ_U64,
                0xFACE_u64.to_be_bytes()
            )]))
            .unwrap(),
            0xFACE
        );
        assert_eq!(
            read_crc32(&Element::new(types::CRC32).with_children([leaf(
                "Value",
                types::UNSIGNED_INT,
                0x1234_u32.to_be_bytes()
            )]))
            .unwrap(),
            0x1234
        );
        assert_eq!(
            read_crc32_or_u32(&Element::new(types::CRC32).with_children([leaf(
                "Value",
                types::UNSIGNED_INT,
                0x5678_u32.to_be_bytes()
            )]))
            .unwrap(),
            0x5678
        );
        assert_eq!(
            read_crc32_or_u32(&leaf(
                "m_crc",
                types::UNSIGNED_INT,
                0x9ABC_u32.to_be_bytes()
            ))
            .unwrap(),
            0x9ABC
        );
    }

    #[test]
    fn reads_entity_id_vector() {
        let element = Element::new(types::AZSTD_VECTOR).with_children([
            Element::new(types::ENTITY_ID).with_children([leaf(
                "id",
                types::AZ_U64,
                0xCAFE_u64.to_be_bytes(),
            )]),
            leaf("ignored", types::UNSIGNED_INT, 1_u32.to_be_bytes()),
            Element::new(types::ENTITY_ID).with_children([leaf(
                "ID",
                types::AZ_U64,
                0xFACE_u64.to_be_bytes(),
            )]),
        ]);

        assert_eq!(
            read_entity_id_vector(&element).unwrap(),
            vec![0xCAFE, 0xFACE]
        );
    }

    #[test]
    fn rejects_malformed_entity_id_vector_child() {
        let element =
            Element::new(types::AZSTD_VECTOR).with_children([Element::new(types::ENTITY_ID)]);

        assert!(matches!(
            read_entity_id_vector(&element).unwrap_err(),
            ObjectStreamValueError::MissingField { .. }
        ));
    }

    #[test]
    fn reads_crc32_vector() {
        let element = Element::new(types::AZSTD_VECTOR).with_children([
            Element::new(types::CRC32).with_children([leaf(
                "Value",
                types::UNSIGNED_INT,
                0x1234_u32.to_be_bytes(),
            )]),
            leaf("ignored", types::UNSIGNED_INT, 1_u32.to_be_bytes()),
            Element::new(types::CRC32).with_children([leaf(
                "value",
                types::UNSIGNED_INT,
                0x5678_u32.to_be_bytes(),
            )]),
        ]);

        assert_eq!(read_crc32_vector(&element).unwrap(), vec![0x1234, 0x5678]);
    }

    #[test]
    fn rejects_malformed_crc32_vector_child() {
        let element = Element::new(types::AZSTD_VECTOR).with_children([Element::new(types::CRC32)]);

        assert!(matches!(
            read_crc32_vector(&element).unwrap_err(),
            ObjectStreamValueError::MissingField { .. }
        ));
    }

    #[test]
    fn rejects_malformed_wrapped_az_ids() {
        assert!(matches!(
            read_entity_id(&Element::new(types::CRC32)).unwrap_err(),
            ObjectStreamValueError::UnexpectedType {
                expected: "AZ::EntityId",
                ..
            }
        ));
        assert!(matches!(
            read_entity_id(&Element::new(types::ENTITY_ID)).unwrap_err(),
            ObjectStreamValueError::MissingField { .. }
        ));
        assert!(matches!(
            read_crc32(&Element::new(types::CRC32)).unwrap_err(),
            ObjectStreamValueError::MissingField { .. }
        ));
        assert!(matches!(
            read_crc32_or_u32(&leaf("Crc", types::BOOL, [1])).unwrap_err(),
            ObjectStreamValueError::UnexpectedType {
                expected: "AZ::Crc32 or unsigned int",
                ..
            }
        ));
    }

    #[test]
    fn rejects_bad_scalar_payloads() {
        let enum_type = Uuid::from_u128(0x11111111_2222_3333_4444_555555555555);

        assert!(matches!(
            read_i32_scalar(&leaf("Mode", enum_type, [1, 2, 3])).unwrap_err(),
            ObjectStreamValueError::InvalidLength { expected: 4, .. }
        ));
        assert!(matches!(
            read_u16_scalar(&leaf("Granularity", types::BOOL, [1])).unwrap_err(),
            ObjectStreamValueError::UnexpectedType {
                expected: "unsigned short or unsigned int",
                ..
            }
        ));
        assert!(matches!(
            read_u64_scalar(&leaf("Id", types::BOOL, [1])).unwrap_err(),
            ObjectStreamValueError::UnexpectedType {
                expected: "unsigned long, AZ::u64, or unsigned int",
                ..
            }
        ));
        assert!(matches!(
            read_u64_payload(&leaf("Id", types::BOOL, [1])).unwrap_err(),
            ObjectStreamValueError::InvalidLength {
                expected: 8,
                actual: 1,
                ..
            }
        ));
        assert!(matches!(
            read_payload_bytes::<12>(&leaf("Bytes", types::BOOL, [1])).unwrap_err(),
            ObjectStreamValueError::InvalidLength {
                expected: 12,
                actual: 1,
                ..
            }
        ));
        assert!(matches!(
            read_optional_payload_bytes::<24>(&leaf("Bytes", types::BOOL, [1])).unwrap_err(),
            ObjectStreamValueError::InvalidLength {
                expected: 24,
                actual: 1,
                ..
            }
        ));
        assert!(matches!(
            read_f64_scalar(&leaf("Delay", types::BOOL, [1])).unwrap_err(),
            ObjectStreamValueError::UnexpectedType {
                expected: "double or float",
                ..
            }
        ));
    }

    #[test]
    fn reads_string_and_packed_floats() {
        assert_eq!(
            read_string(&leaf(
                "AssetPath",
                types::AZSTD_STRING,
                b"Materials/Foo.mtl"
            ))
            .unwrap(),
            "Materials/Foo.mtl"
        );
        assert_eq!(
            read_vector_float(&leaf("Scalar", types::VECTOR_FLOAT, floats([1.5]))).unwrap(),
            1.5
        );
        assert_eq!(
            read_vec3(&leaf("Offset", types::VECTOR3, floats([1.0, 2.0, 3.0]))).unwrap(),
            [1.0, 2.0, 3.0]
        );
        assert_eq!(
            read_vec2(&leaf("Vertex", types::VECTOR2, floats([1.0, 2.0]))).unwrap(),
            [1.0, 2.0]
        );
        assert_eq!(
            read_color(&leaf("Color", types::COLOR, floats([0.1, 0.2, 0.3, 0.4]))).unwrap(),
            [0.1, 0.2, 0.3, 0.4]
        );
        assert_eq!(
            read_vec4(&leaf(
                "Plane",
                types::VECTOR4,
                floats([0.0, 1.0, 0.0, -2.0])
            ))
            .unwrap(),
            [0.0, 1.0, 0.0, -2.0]
        );
        assert_eq!(
            read_quat(&leaf(
                "Rotation",
                types::QUATERNION,
                floats([0.0, 0.0, 0.0, 1.0])
            ))
            .unwrap(),
            [0.0, 0.0, 0.0, 1.0]
        );
        assert_eq!(
            read_transform(&leaf(
                "m_worldTM",
                types::TRANSFORM,
                floats([1.0, 0.0, 0.0, 2.0, 0.0, 1.0, 0.0, 3.0, 0.0, 0.0, 1.0, 4.0])
            ))
            .unwrap(),
            [1.0, 0.0, 0.0, 2.0, 0.0, 1.0, 0.0, 3.0, 0.0, 0.0, 1.0, 4.0]
        );
        assert_eq!(
            read_matrix3x3(&leaf(
                "basis",
                types::MATRIX3X3,
                floats([1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0])
            ))
            .unwrap(),
            [1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0]
        );
        assert_eq!(
            read_matrix4x4(&leaf(
                "matrix",
                types::MATRIX4X4,
                floats([
                    1.0, 0.0, 0.0, 5.0, 0.0, 1.0, 0.0, 6.0, 0.0, 0.0, 1.0, 7.0, 0.0, 0.0, 0.0, 1.0,
                ])
            ))
            .unwrap(),
            [
                1.0, 0.0, 0.0, 5.0, 0.0, 1.0, 0.0, 6.0, 0.0, 0.0, 1.0, 7.0, 0.0, 0.0, 0.0, 1.0,
            ]
        );
        assert_eq!(
            read_byte_stream(&leaf("Data", types::BYTE_STREAM, [0, 1, 2, 255])).unwrap(),
            &[0, 1, 2, 255]
        );
    }

    #[test]
    fn formats_known_leaf_values_for_search() {
        assert_eq!(
            read_leaf_text(&leaf("label", types::AZSTD_BASIC_STRING, b"surface")).unwrap(),
            "surface"
        );
        assert_eq!(
            read_leaf_text(&leaf("scalar", types::VECTOR_FLOAT, floats([1.5]))).unwrap(),
            "1.5000000"
        );
        assert_eq!(
            read_leaf_text(&leaf(
                "plane",
                types::VECTOR4,
                floats([0.0, 1.0, 0.0, -2.0])
            ))
            .unwrap(),
            "0.0000000 1.0000000 0.0000000 -2.0000000"
        );
        assert_eq!(
            read_leaf_text(&leaf(
                "matrix",
                types::MATRIX4X4,
                floats([
                    1.0, 0.0, 0.0, 5.0, 0.0, 1.0, 0.0, 6.0, 0.0, 0.0, 1.0, 7.0, 0.0, 0.0, 0.0, 1.0,
                ])
            ))
            .unwrap(),
            "1.0000000 0.0000000 0.0000000 5.0000000 0.0000000 1.0000000 0.0000000 6.0000000 0.0000000 0.0000000 1.0000000 7.0000000 0.0000000 0.0000000 0.0000000 1.0000000"
        );
    }

    #[test]
    fn generic_decode_accepts_equivalent_float_leaf_shapes() {
        let scalar: f32 =
            DecodeAzValue::decode_az_value(&leaf("Scalar", types::VECTOR_FLOAT, floats([2.25])))
                .unwrap();
        assert_eq!(scalar, 2.25);

        let vector: [f32; 4] =
            DecodeAzValue::decode_az_value(&leaf("Vector", types::VECTOR4, floats([1.0; 4])))
                .unwrap();
        assert_eq!(vector, [1.0; 4]);

        let quat: [f32; 4] = DecodeAzValue::decode_az_value(&leaf(
            "Rotation",
            types::QUATERNION,
            floats([0.0, 0.0, 0.0, 1.0]),
        ))
        .unwrap();
        assert_eq!(quat, [0.0, 0.0, 0.0, 1.0]);

        let matrix: [f32; 16] = DecodeAzValue::decode_az_value(&leaf(
            "Matrix",
            types::MATRIX4X4,
            floats([
                1.0, 0.0, 0.0, 5.0, 0.0, 1.0, 0.0, 6.0, 0.0, 0.0, 1.0, 7.0, 0.0, 0.0, 0.0, 1.0,
            ]),
        ))
        .unwrap();
        assert_eq!(matrix[15], 1.0);
    }

    #[test]
    fn reads_trimmed_string_values() {
        assert_eq!(
            read_trimmed_string(&leaf("Name", types::AZSTD_STRING, b"  detector  ")).unwrap(),
            Some("detector")
        );
        assert_eq!(
            read_trimmed_string(&leaf("Name", types::AZSTD_STRING, b"   ")).unwrap(),
            None
        );
    }

    #[test]
    fn reads_owned_trimmed_string_values() {
        assert_eq!(
            read_trimmed_string_owned(&leaf("Name", types::AZSTD_STRING, b"  detector  ")).unwrap(),
            Some("detector".to_string())
        );
        assert_eq!(
            read_trimmed_string_owned(&leaf("Name", types::AZSTD_STRING, b"   ")).unwrap(),
            None
        );
    }

    #[test]
    fn reads_optional_trimmed_string_values() {
        assert_eq!(
            read_optional_trimmed_string(&Element::new(types::AZSTD_STRING).with_field("Name"))
                .unwrap(),
            None
        );
        assert_eq!(
            read_optional_trimmed_string(&leaf("Name", types::AZSTD_STRING, b"  detector  "))
                .unwrap(),
            Some("detector")
        );
        assert_eq!(
            read_optional_trimmed_string_owned(&leaf("Name", types::AZSTD_STRING, b"   ")).unwrap(),
            None
        );
        assert!(matches!(
            read_optional_trimmed_string(&leaf("Name", types::BOOL, [1])).unwrap_err(),
            ObjectStreamValueError::UnexpectedType {
                expected: "AZStd::string",
                ..
            }
        ));
    }

    #[test]
    fn reads_string_vector_children() {
        let element = Element::new(types::AZSTD_VECTOR).with_children([
            leaf("element", types::AZSTD_STRING, b"  alpha  "),
            leaf("element", types::AZSTD_BASIC_STRING, b"beta"),
            leaf("element", types::AZSTD_STRING, b" "),
            leaf("count", types::UNSIGNED_INT, 3_u32.to_be_bytes()),
        ]);

        assert_eq!(read_string_vector(&element).unwrap(), vec!["alpha", "beta"]);
    }

    #[test]
    fn reads_owned_string_vector_children() {
        let element = Element::new(types::AZSTD_VECTOR).with_children([
            leaf("element", types::AZSTD_STRING, b"  alpha  "),
            leaf("element", types::AZSTD_BASIC_STRING, b"beta"),
            leaf("element", types::AZSTD_STRING, b" "),
            leaf("count", types::UNSIGNED_INT, 3_u32.to_be_bytes()),
        ]);

        assert_eq!(
            read_string_vector_owned(&element).unwrap(),
            vec!["alpha".to_string(), "beta".to_string()]
        );
    }

    #[test]
    fn reads_bool_vector_children() {
        let element = Element::new(types::AZSTD_VECTOR).with_children([
            leaf("element", types::BOOL, [1]),
            leaf("element", types::UNSIGNED_INT, 2_u32.to_be_bytes()),
            leaf("element", types::BOOL, [0]),
        ]);

        assert_eq!(read_bool_vector(&element).unwrap(), vec![true, false]);
    }

    #[test]
    fn reads_f64_vector_children() {
        let element = Element::new(types::AZSTD_VECTOR).with_children([
            leaf("element", types::DOUBLE, 2.5_f64.to_be_bytes()),
            leaf("element", types::BOOL, [1]),
            leaf("element", types::FLOAT, 1.25_f32.to_be_bytes()),
        ]);

        assert_eq!(read_f64_vector(&element).unwrap(), vec![2.5, 1.25]);
    }

    #[test]
    fn rejects_unexpected_type() {
        let err = read_bool(&leaf("Visible", types::INT, 1_i32.to_be_bytes())).unwrap_err();
        assert!(matches!(
            err,
            ObjectStreamValueError::UnexpectedType {
                expected: "bool",
                ..
            }
        ));
    }

    #[test]
    fn reads_streaming_header_values() {
        let field = ArcStr::from("Count");
        let data = 42_u32.to_be_bytes();
        let header = ElementHeader {
            flags: ST_BINARYFLAG_HAS_VALUE,
            name_crc: None,
            version: None,
            id: types::UNSIGNED_INT,
            specialization: None,
            name: None,
            field: Some(&field),
            data: &data,
        };

        assert_eq!(read_u32(&header).unwrap(), 42);
    }

    #[test]
    fn field_name_uses_unnamed_fallback() {
        assert_eq!(
            field_name(&leaf("Named", types::BOOL, [1])),
            "Named".to_string()
        );
        assert_eq!(field_name(&Element::new(types::BOOL)), "<unnamed>");
    }

    #[test]
    fn field_cursor_consumes_matches_in_order() {
        let element = Element::new(types::AZSTD_VECTOR).with_children([
            leaf("First", types::BOOL, [1]),
            leaf("Second", types::BOOL, [0]),
            leaf("Third", types::BOOL, [1]),
        ]);
        let mut cursor = FieldCursor::from_element(&element);

        assert_eq!(
            cursor.find("Second").and_then(Element::field).unwrap(),
            "Second"
        );
        assert!(cursor.find("First").is_none());
        assert_eq!(
            cursor.find("Third").and_then(Element::field).unwrap(),
            "Third"
        );
        assert!(cursor.is_empty());
    }

    #[test]
    fn element_fields_reuses_full_child_list() {
        let element = Element::new(types::AZSTD_VECTOR).with_children([
            leaf("First", types::BOOL, [1]),
            leaf("Second", types::BOOL, [0]),
        ]);
        let mut fields = ElementFields::new(&element);

        assert_eq!(fields.read("Second", read_bool).unwrap(), Some(false));
        assert_eq!(fields.read("First", read_bool).unwrap(), Some(true));
        assert_eq!(fields.read("Second", read_bool).unwrap(), Some(false));
    }

    #[test]
    fn element_fields_reads_first_serialized_alias() {
        let element = Element::new(types::AZSTD_VECTOR).with_children([
            leaf("OldName", types::FLOAT, 1.5_f32.to_be_bytes()),
            leaf("NewName", types::FLOAT, 2.5_f32.to_be_bytes()),
        ]);
        let mut fields = ElementFields::new(&element);

        let (field, value) = fields
            .read_any(&["NewName", "OldName"], read_f32)
            .unwrap()
            .unwrap();

        assert_eq!(field, "OldName");
        assert_eq!(value, 1.5);
    }

    #[test]
    fn field_access_reads_values_without_materializing_names() {
        let element = Element::new(types::AZSTD_VECTOR).with_children([
            leaf("Enabled", types::BOOL, [1]),
            leaf("Name", types::AZSTD_STRING, b"detector"),
            leaf("Count", types::UNSIGNED_INT, 7_u32.to_be_bytes()),
        ]);
        let mut fields = FieldCursor::from_element(&element);

        assert_eq!(fields.read("Enabled", read_bool).unwrap(), Some(true));
        assert!(fields.read("Missing", read_u8).unwrap().is_none());
        assert_eq!(
            fields.field("Name").map(read_string).transpose().unwrap(),
            Some("detector")
        );
        assert_eq!(fields.read("Count", read_u32).unwrap(), Some(7));
        assert!(fields.is_empty());
    }

    #[test]
    fn field_access_decodes_values_by_type() {
        let element = Element::new(types::AZSTD_VECTOR).with_children([
            leaf("Enabled", types::BOOL, [1]),
            leaf("Name", types::AZSTD_STRING, b"detector"),
            leaf("Label", types::AZSTD_STRING, b"raw label"),
            leaf("Count", types::UNSIGNED_INT, 7_u32.to_be_bytes()),
        ]);
        let mut fields = FieldCursor::from_element(&element);

        let enabled: Option<bool> = fields.decode("Enabled").unwrap();
        assert_eq!(enabled, Some(true));
        let name: Option<&str> = fields.decode("Name").unwrap();
        assert_eq!(name, Some("detector"));
        let label: Option<String> = fields.decode("Label").unwrap();
        assert_eq!(label.as_deref(), Some("raw label"));
        let count: Option<u32> = fields.decode("Count").unwrap();
        assert_eq!(count, Some(7));
        assert!(fields.is_empty());
    }

    #[test]
    fn field_access_reads_trimmed_string_fields() {
        let element = Element::new(types::AZSTD_VECTOR).with_children([
            leaf("Name", types::AZSTD_STRING, b"  detector  "),
            leaf("Empty", types::AZSTD_STRING, b"   "),
            leaf("Label", types::AZSTD_BASIC_STRING, b"active"),
        ]);
        let mut cursor = FieldCursor::from_element(&element);

        assert_eq!(
            cursor.read_trimmed_string("Name").unwrap(),
            Some(Some("detector"))
        );
        assert_eq!(cursor.read_trimmed_string("Empty").unwrap(), Some(None));
        assert_eq!(
            cursor.read_trimmed_string_owned("Label").unwrap(),
            Some(Some("active".to_string()))
        );
        assert_eq!(cursor.read_trimmed_string_owned("Missing").unwrap(), None);
        assert!(cursor.is_empty());
    }

    #[test]
    fn field_access_reads_trimmed_string_aliases() {
        let element = Element::new(types::AZSTD_VECTOR).with_children([
            leaf("name", types::AZSTD_STRING, b"  serialized first  "),
            leaf("Name", types::AZSTD_STRING, b"alias priority second"),
            leaf("Empty", types::AZSTD_STRING, b"   "),
        ]);
        let mut cursor = FieldCursor::from_element(&element);

        let (field, value) = cursor
            .read_trimmed_string_any(&["Name", "name"])
            .unwrap()
            .unwrap();
        assert_eq!(field, "name");
        assert_eq!(value, Some("serialized first"));
        let (field, value) = cursor
            .read_trimmed_string_any_owned(&["Empty"])
            .unwrap()
            .unwrap();
        assert_eq!(field, "Empty");
        assert_eq!(value, None);
        assert_eq!(
            cursor.read_trimmed_string_any_owned(&["Missing"]).unwrap(),
            None
        );
        assert!(cursor.is_empty());
    }

    #[test]
    fn field_access_reads_first_matching_alias() {
        let element = Element::new(types::AZSTD_VECTOR).with_children([
            leaf("OldName", types::FLOAT, 1.5_f32.to_be_bytes()),
            leaf("NewName", types::FLOAT, 2.5_f32.to_be_bytes()),
        ]);
        let mut fields = FieldCursor::from_element(&element);

        let (field, value) = fields
            .read_any(&["NewName", "OldName"], read_f32)
            .unwrap()
            .unwrap();

        assert_eq!(field, "OldName");
        assert_eq!(value, 1.5);
        assert_eq!(fields.read("NewName", read_f32).unwrap(), Some(2.5));
    }

    #[test]
    fn child_by_field_any_follows_child_order() {
        let element = Element::new(types::AZSTD_VECTOR).with_children([
            leaf("OldName", types::FLOAT, 1.5_f32.to_be_bytes()),
            leaf("NewName", types::FLOAT, 2.5_f32.to_be_bytes()),
        ]);

        let child = child_by_field_any(&element, &["NewName", "OldName"]).expect("alias child");
        assert_eq!(child.field().map(arcstr::ArcStr::as_str), Some("OldName"));
        assert_eq!(read_f32(child).unwrap(), 1.5);
    }

    #[test]
    fn child_by_field_or_index_prefers_named_child() {
        let element = Element::new(types::AZSTD_VECTOR).with_children([
            leaf("OldName", types::FLOAT, 1.5_f32.to_be_bytes()),
            leaf("NewName", types::FLOAT, 2.5_f32.to_be_bytes()),
        ]);

        let child = child_by_field_or_index(&element, "NewName", 0).expect("field");
        assert_eq!(child.field().map(arcstr::ArcStr::as_str), Some("NewName"));
        assert_eq!(read_f32(child).unwrap(), 2.5);
    }

    #[test]
    fn child_by_field_or_index_falls_back_to_index() {
        let element = Element::new(types::AZSTD_VECTOR).with_children([
            leaf("OldName", types::FLOAT, 1.5_f32.to_be_bytes()),
            leaf("OtherName", types::FLOAT, 2.5_f32.to_be_bytes()),
        ]);

        let child = child_by_field_or_index(&element, "NewName", 0).expect("index child");
        assert_eq!(child.field().map(arcstr::ArcStr::as_str), Some("OldName"));
        assert_eq!(read_f32(child).unwrap(), 1.5);
    }

    #[test]
    fn read_child_by_field_or_index_decodes_named_child() {
        let element = Element::new(types::AZSTD_VECTOR).with_children([
            leaf("OldName", types::FLOAT, 1.5_f32.to_be_bytes()),
            leaf("NewName", types::FLOAT, 2.5_f32.to_be_bytes()),
        ]);

        assert_eq!(
            read_child_by_field_or_index(&element, "NewName", 0, read_f32).unwrap(),
            Some(2.5)
        );
    }

    #[test]
    fn decode_child_by_field_or_index_falls_back_to_index() {
        let element = Element::new(types::AZSTD_VECTOR).with_children([
            leaf("OldName", types::INT, 7_i32.to_be_bytes()),
            leaf("OtherName", types::INT, 11_i32.to_be_bytes()),
        ]);

        assert_eq!(
            decode_child_by_field_or_index::<i32>(&element, "NewName", 0).unwrap(),
            Some(7)
        );
    }

    #[test]
    fn child_by_field_or_index_returns_none_on_miss() {
        let element = Element::new(types::AZSTD_VECTOR).with_children([leaf(
            "OldName",
            types::FLOAT,
            1.5_f32.to_be_bytes(),
        )]);

        assert!(child_by_field_or_index(&element, "NewName", 2).is_none());
    }

    #[test]
    fn field_access_decodes_first_matching_alias() {
        let element = Element::new(types::AZSTD_VECTOR).with_children([
            leaf("OldName", types::FLOAT, 1.5_f32.to_be_bytes()),
            leaf("NewName", types::FLOAT, 2.5_f32.to_be_bytes()),
        ]);
        let mut fields = FieldCursor::from_element(&element);

        let (field, value): (&str, f32) =
            fields.decode_any(&["NewName", "OldName"]).unwrap().unwrap();

        assert_eq!(field, "OldName");
        assert_eq!(value, 1.5);
        let next: Option<f32> = fields.decode("NewName").unwrap();
        assert_eq!(next, Some(2.5));
    }

    #[test]
    fn field_access_requires_first_matching_alias() {
        let element = Element::new(types::AZSTD_VECTOR).with_children([
            leaf("OldName", types::FLOAT, 1.5_f32.to_be_bytes()),
            leaf("NewName", types::FLOAT, 2.5_f32.to_be_bytes()),
        ]);
        let mut fields = ElementFields::new(&element);

        let (field, value): (&str, f32) = fields.required_any(&["NewName", "OldName"]).unwrap();

        assert_eq!(field, "OldName");
        assert_eq!(value, 1.5);
        assert!(matches!(
            fields.required_any::<u32>(&["Missing", "AlsoMissing"]),
            Err(ObjectStreamValueError::MissingField { field })
                if field == "Missing|AlsoMissing"
        ));
    }

    #[test]
    fn field_access_handles_required_and_defaulted_fields() {
        let element = Element::new(types::AZSTD_VECTOR).with_children([
            leaf("Enabled", types::BOOL, [1]),
            leaf("Count", types::UNSIGNED_INT, 7_u32.to_be_bytes()),
        ]);
        let mut fields = FieldCursor::from_element(&element);

        let enabled: bool = fields.required("Enabled").unwrap();
        assert!(enabled);
        let missing: u32 = fields.decode_or_default("Missing").unwrap();
        assert_eq!(missing, 0);
        let count: u32 = fields.decode_or("Count", 10).unwrap();
        assert_eq!(count, 7);

        let mut fields = FieldCursor::from_element(&element);
        assert!(matches!(
            fields.required::<u32>("Missing"),
            Err(ObjectStreamValueError::MissingField { .. })
        ));
    }

    #[test]
    fn field_access_decodes_owned_strings() {
        let element = Element::new(types::AZSTD_VECTOR).with_children([leaf(
            "Name",
            types::AZSTD_STRING,
            b"shape",
        )]);
        let mut fields = FieldCursor::from_element(&element);

        let name: Box<str> = fields.required("Name").unwrap();
        assert_eq!(&*name, "shape");
    }

    #[test]
    fn field_access_decodes_packed_float_arrays() {
        let element = Element::new(types::AZSTD_VECTOR).with_children([
            leaf("Position", types::VECTOR3, floats([1.0, 2.0, 3.0])),
            leaf("Tint", types::COLOR, floats([0.1, 0.2, 0.3, 0.4])),
            leaf("Missing", types::BOOL, [1]),
        ]);
        let mut fields = FieldCursor::from_element(&element);

        let position: [f32; 3] = fields.required("Position").unwrap();
        assert_eq!(position, [1.0, 2.0, 3.0]);
        let tint: [f32; 4] = fields.required("Tint").unwrap();
        assert_eq!(tint, [0.1, 0.2, 0.3, 0.4]);
        let transform: [f32; 12] = fields.decode_or_default("Transform").unwrap();
        assert_eq!(transform, [0.0; 12]);
    }

    #[test]
    fn decodes_streaming_header_without_copying_string() {
        let field = ArcStr::from("Name");
        let data = b"volume";
        let header = ElementHeader {
            flags: ST_BINARYFLAG_HAS_VALUE,
            name_crc: None,
            version: None,
            id: types::AZSTD_STRING,
            specialization: None,
            name: None,
            field: Some(&field),
            data,
        };

        let value: &str = DecodeAzValue::decode_az_value(&header).unwrap();
        assert_eq!(value, "volume");
    }

    fn leaf(field: &str, id: Uuid, data: impl Into<Vec<u8>>) -> Element {
        Element::new(id).with_field(field).with_data(data)
    }

    fn floats<const N: usize>(values: [f32; N]) -> Vec<u8> {
        values.into_iter().flat_map(f32::to_be_bytes).collect()
    }
}
