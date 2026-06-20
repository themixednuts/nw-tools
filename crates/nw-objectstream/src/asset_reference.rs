use std::num::TryFromIntError;
use std::str::Utf8Error;

use thiserror::Error;
use uuid::{Uuid, uuid};

use nw_asset::{AssetId, AssetReference, AssetType};

use crate::Element;
use crate::types;
use crate::value::{self, DecodeAzValue, ElementValue, FieldCursor, ObjectStreamValueError};

pub const SIMPLE_ASSET_REFERENCE_BASE: &str = "SimpleAssetReferenceBase";
pub const ASSET_PATH_FIELD: &str = "AssetPath";

pub const SIMPLE_ASSET_REFERENCE_TYPE_ID: Uuid = types::ASSET;
pub const SIMPLE_TEXTURE_ASSET_REFERENCE_TYPE_ID: Uuid =
    uuid!("68e92460-5c0c-4031-9620-6f1a08763243");
pub const SIMPLE_ASSET_REFERENCE_BASE_TYPE_ID: Uuid = uuid!("e16ca6c5-5c78-4ad9-8e9b-f8c1fb4d1db8");

pub const BASE_CLASS_FIELD_CRC: u32 = 3_566_360_373;
pub const ASSET_PATH_FIELD_CRC: u32 = 741_691_769;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AssetValue<'a> {
    guid: Uuid,
    sub_id: u32,
    asset_type: Uuid,
    hint: &'a str,
}

impl<'a> AssetValue<'a> {
    #[inline]
    #[must_use]
    pub const fn new(guid: Uuid, sub_id: u32, asset_type: Uuid, hint: &'a str) -> Self {
        Self {
            guid,
            sub_id,
            asset_type,
            hint,
        }
    }

    #[inline]
    #[must_use]
    pub const fn guid(self) -> Uuid {
        self.guid
    }

    #[inline]
    #[must_use]
    pub const fn sub_id(self) -> u32 {
        self.sub_id
    }

    #[inline]
    #[must_use]
    pub const fn asset_type(self) -> Uuid {
        self.asset_type
    }

    #[inline]
    #[must_use]
    pub const fn hint(self) -> &'a str {
        self.hint
    }

    #[inline]
    #[must_use]
    pub fn into_asset_reference(self) -> AssetReference {
        let hint = (!self.hint.trim().is_empty()).then(|| self.hint.to_string());
        AssetReference::new(
            AssetId::new(self.guid, self.sub_id),
            AssetType::new(self.asset_type),
            hint,
        )
    }
}

#[derive(Debug, Error)]
pub enum AssetValueError {
    #[error("expected AZ::Data::Asset, got {actual}")]
    UnexpectedType { actual: Uuid },

    #[error("AZ::Data::Asset has no value bytes")]
    MissingData,

    #[error("AZ::Data::Asset has {actual} bytes, expected at least {expected}")]
    TooShort { expected: usize, actual: usize },

    #[error("AZ::Data::Asset sub id overflows u32")]
    SubIdOverflow(#[from] TryFromIntError),

    #[error("AZ::Data::Asset hint is not valid UTF-8")]
    Utf8(#[from] Utf8Error),

    #[error("AZ::Data::Asset does not match a known value layout in {0} bytes")]
    UnsupportedLayout(usize),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SimpleAssetReferenceValue<'a> {
    guid: Uuid,
    asset_type: Uuid,
    path: &'a str,
}

impl<'a> SimpleAssetReferenceValue<'a> {
    #[inline]
    #[must_use]
    pub const fn new(guid: Uuid, asset_type: Uuid, path: &'a str) -> Self {
        Self {
            guid,
            asset_type,
            path,
        }
    }

    #[inline]
    #[must_use]
    pub const fn guid(self) -> Uuid {
        self.guid
    }

    #[inline]
    #[must_use]
    pub const fn asset_type(self) -> Uuid {
        self.asset_type
    }

    #[inline]
    #[must_use]
    pub const fn path(self) -> &'a str {
        self.path
    }
}

#[derive(Debug, Error)]
pub enum SimpleAssetReferenceValueError {
    #[error("expected SimpleAssetReference, got {actual}")]
    UnexpectedType { actual: Uuid },

    #[error("SimpleAssetReference has no value bytes")]
    MissingData,

    #[error("SimpleAssetReference has {actual} bytes, expected at least {expected}")]
    TooShort { expected: usize, actual: usize },

    #[error("SimpleAssetReference reserved AssetId bytes are not zero")]
    NonZeroAssetIdExtension,

    #[error("SimpleAssetReference path length {declared} does not match {actual} bytes")]
    InvalidPathLength { declared: usize, actual: usize },

    #[error("SimpleAssetReference path length overflows usize")]
    PathLengthOverflow(#[from] TryFromIntError),

    #[error("SimpleAssetReference path is not valid UTF-8")]
    Utf8(#[from] Utf8Error),
}

#[derive(Debug, Error)]
pub enum SimpleAssetReferenceElementError {
    #[error("expected SimpleAssetReference type {expected}, got {actual}")]
    UnexpectedType { expected: Uuid, actual: Uuid },

    #[error("SimpleAssetReference is missing BaseClass1")]
    MissingBase,

    #[error("SimpleAssetReference base has unexpected type {actual}")]
    UnexpectedBaseType { actual: Uuid },

    #[error("SimpleAssetReference has unexpected field with type {type_id}")]
    UnexpectedField { type_id: Uuid },

    #[error("SimpleAssetReference base is missing AssetPath")]
    MissingAssetPath,

    #[error("SimpleAssetReference value error")]
    Value(#[from] ObjectStreamValueError),
}

#[derive(Debug, Error)]
pub enum AssetHintError {
    #[error("expected AZ::Data::Asset, got {0}")]
    UnexpectedType(Uuid),
    #[error("asset reference has no value bytes")]
    MissingData,
    #[error("asset reference has {actual} bytes, expected at least {expected}")]
    TooShort { expected: usize, actual: usize },
    #[error("asset reference has no valid hint layout in {0} bytes")]
    InvalidHintLayout(usize),
    #[error("asset hint is not valid UTF-8")]
    Utf8(#[from] Utf8Error),
}

#[derive(Debug, Error)]
pub enum TypedAssetOrSimpleReferenceError {
    #[error("typed asset hint error")]
    Hint(#[from] AssetHintError),

    #[error("simple asset reference path error")]
    Path(#[from] ObjectStreamValueError),
}

pub fn read_asset_value<E>(element: &E) -> Result<AssetValue<'_>, AssetValueError>
where
    E: ElementValue + ?Sized,
{
    if element.id() != types::ASSET {
        return Err(AssetValueError::UnexpectedType {
            actual: element.id(),
        });
    }
    let data = element.data().ok_or(AssetValueError::MissingData)?;
    AssetValueLayout::read(data)
}

pub fn read_simple_asset_reference_value<E>(
    element: &E,
) -> Result<SimpleAssetReferenceValue<'_>, SimpleAssetReferenceValueError>
where
    E: ElementValue + ?Sized,
{
    if element.id() != SIMPLE_ASSET_REFERENCE_TYPE_ID {
        return Err(SimpleAssetReferenceValueError::UnexpectedType {
            actual: element.id(),
        });
    }
    let data = element
        .data()
        .ok_or(SimpleAssetReferenceValueError::MissingData)?;
    SimpleAssetReferenceLayout::read(data)
}

pub fn read_simple_asset_reference_path(
    element: &Element,
    expected_type_id: Uuid,
) -> Result<&str, SimpleAssetReferenceElementError> {
    if *element.id() != expected_type_id {
        return Err(SimpleAssetReferenceElementError::UnexpectedType {
            expected: expected_type_id,
            actual: *element.id(),
        });
    }

    let mut fields = FieldCursor::from_element(element);
    let Some(base) = fields.next() else {
        return Err(SimpleAssetReferenceElementError::MissingBase);
    };
    if base
        .field()
        .is_none_or(|field| field.as_str() != "BaseClass1")
    {
        return Err(SimpleAssetReferenceElementError::UnexpectedField {
            type_id: *base.id(),
        });
    }
    if *base.id() != SIMPLE_ASSET_REFERENCE_BASE_TYPE_ID {
        return Err(SimpleAssetReferenceElementError::UnexpectedBaseType { actual: *base.id() });
    }
    if let Some(extra) = fields.next() {
        return Err(SimpleAssetReferenceElementError::UnexpectedField {
            type_id: *extra.id(),
        });
    }

    let mut base_fields = FieldCursor::from_element(base);
    let Some(path) = base_fields.next() else {
        return Err(SimpleAssetReferenceElementError::MissingAssetPath);
    };
    if path
        .field()
        .is_none_or(|field| field.as_str() != ASSET_PATH_FIELD)
    {
        return Err(SimpleAssetReferenceElementError::UnexpectedField {
            type_id: *path.id(),
        });
    }
    if let Some(extra) = base_fields.next() {
        return Err(SimpleAssetReferenceElementError::UnexpectedField {
            type_id: *extra.id(),
        });
    }

    <&str>::decode_az_value(path).map_err(SimpleAssetReferenceElementError::Value)
}

/// Read a reflected simple asset reference path without requiring a
/// specific wrapper type.
///
/// Some ObjectStreams store `AssetPath` directly under the
/// reference object, while others place it one level down in the base
/// reference object. This helper keeps that relaxed fallback shape in
/// the ObjectStream asset-reference module instead of repeating it in
/// callers.
pub fn read_simple_asset_reference_path_any(
    element: &Element,
) -> Result<Option<&str>, ObjectStreamValueError> {
    let Some(field) = value::child_by_field(element, ASSET_PATH_FIELD).or_else(|| {
        element
            .children()
            .iter()
            .find_map(|child| value::child_by_field(child, ASSET_PATH_FIELD))
    }) else {
        return Ok(None);
    };

    let path = value::read_string(field)?.trim();
    Ok((!path.is_empty()).then_some(path))
}

pub fn read_simple_asset_reference_path_any_owned(
    element: &Element,
) -> Result<Option<String>, ObjectStreamValueError> {
    read_simple_asset_reference_path_any(element).map(|path| path.map(str::to_string))
}

/// Read an asset path serialized either as an `AZStd::string` payload
/// or as a reflected SimpleAssetReference object.
pub fn read_asset_path_or_string(
    element: &Element,
) -> Result<Option<&str>, ObjectStreamValueError> {
    if element.data().is_some() {
        value::read_trimmed_string(element)
    } else {
        read_simple_asset_reference_path_any(element)
    }
}

pub fn read_asset_path_or_string_owned(
    element: &Element,
) -> Result<Option<String>, ObjectStreamValueError> {
    read_asset_path_or_string(element).map(|path| path.map(str::to_string))
}

pub fn asset_hint(element: &Element) -> Result<Option<&str>, AssetHintError> {
    if element.id() != &types::ASSET {
        return Err(AssetHintError::UnexpectedType(*element.id()));
    }

    asset_hint_from_data(element)
}

pub fn asset_hint_owned(element: &Element) -> Result<Option<String>, AssetHintError> {
    asset_hint(element).map(|path| path.map(str::to_string))
}

pub fn asset_hint_from_data(element: &Element) -> Result<Option<&str>, AssetHintError> {
    let data = element.data().ok_or(AssetHintError::MissingData)?;
    asset_hint_from_bytes(data)
}

pub fn asset_hint_from_data_owned(element: &Element) -> Result<Option<String>, AssetHintError> {
    asset_hint_from_data(element).map(|path| path.map(str::to_string))
}

/// Read an asset hint from raw asset value bytes when those bytes are
/// present.
///
/// Missing value bytes are treated as `Ok(None)`, matching reflected
/// typed asset references where an empty asset field is encoded as an
/// element without a payload. Malformed present payloads still fail.
pub fn optional_asset_hint_from_data(element: &Element) -> Result<Option<&str>, AssetHintError> {
    let Some(data) = element.data() else {
        return Ok(None);
    };
    asset_hint_from_bytes(data)
}

pub fn optional_asset_hint_from_data_owned(
    element: &Element,
) -> Result<Option<String>, AssetHintError> {
    optional_asset_hint_from_data(element).map(|path| path.map(str::to_string))
}

/// Read a typed asset hint when a value payload is present, otherwise
/// read a reflected simple asset reference path.
///
/// This matches `AZ::Data::Asset<T>` fields that may be emitted either
/// as raw asset value bytes or as a reflected SimpleAssetReference shape.
pub fn typed_asset_hint_or_simple_path(
    element: &Element,
) -> Result<Option<&str>, TypedAssetOrSimpleReferenceError> {
    if element.data().is_some() {
        asset_hint_from_data(element).map_err(TypedAssetOrSimpleReferenceError::Hint)
    } else {
        read_simple_asset_reference_path_any(element)
            .map_err(TypedAssetOrSimpleReferenceError::Path)
    }
}

pub fn typed_asset_hint_or_simple_path_owned(
    element: &Element,
) -> Result<Option<String>, TypedAssetOrSimpleReferenceError> {
    typed_asset_hint_or_simple_path(element).map(|path| path.map(str::to_string))
}

fn asset_hint_from_bytes(data: &[u8]) -> Result<Option<&str>, AssetHintError> {
    if data.len() < AssetHintLayout::MINIMUM_LEN {
        return match asset_hint_from_text(data)? {
            Some(hint) => Ok(Some(hint)),
            None => Err(AssetHintError::TooShort {
                expected: AssetHintLayout::MINIMUM_LEN,
                actual: data.len(),
            }),
        };
    }

    for layout in AssetHintLayout::CANDIDATES {
        if let Some(hint) = layout.read_hint(data)? {
            let hint = hint.trim();
            return Ok((!hint.is_empty()).then_some(hint));
        }
    }

    if let Some(hint) = asset_hint_from_text(data)? {
        return Ok(Some(hint));
    }

    Err(AssetHintError::InvalidHintLayout(data.len()))
}

fn asset_hint_from_text(data: &[u8]) -> Result<Option<&str>, Utf8Error> {
    let value = std::str::from_utf8(data)?.trim();
    let Some(after_prefix) = value
        .find("hint={")
        .map(|start| &value[start + "hint={".len()..])
    else {
        return Ok(None);
    };
    let hint = after_prefix
        .split_once('}')
        .map_or(after_prefix, |(hint, _)| hint)
        .trim();
    Ok((!hint.is_empty()).then_some(hint))
}

#[derive(Debug, Clone, Copy)]
struct AssetHintLayout {
    hint_size_offset: usize,
}

impl AssetHintLayout {
    const MINIMUM_LEN: usize = 48;
    const CANDIDATES: &'static [Self] = &[
        Self {
            hint_size_offset: 40,
        },
        Self {
            hint_size_offset: 36,
        },
        Self {
            hint_size_offset: 48,
        },
    ];

    fn read_hint(self, data: &[u8]) -> Result<Option<&str>, AssetHintError> {
        let Some(size_bytes) = data.get(self.hint_size_offset..self.hint_size_offset + 8) else {
            return Ok(None);
        };
        let size_bytes: [u8; 8] = size_bytes.try_into().expect("slice width is eight");
        let hint_start = self.hint_size_offset + 8;
        let Some(available) = data.len().checked_sub(hint_start) else {
            return Ok(None);
        };

        for declared in [
            u64::from_be_bytes(size_bytes),
            u64::from_le_bytes(size_bytes),
        ] {
            let Ok(declared) = usize::try_from(declared) else {
                continue;
            };
            if declared == available {
                return Ok(Some(std::str::from_utf8(&data[hint_start..])?));
            }
        }

        Ok(None)
    }
}

#[derive(Debug, Clone, Copy)]
struct SimpleAssetReferenceLayout;

impl SimpleAssetReferenceLayout {
    const ASSET_ID_EXTENSION: std::ops::Range<usize> = 16..32;
    const ASSET_TYPE: std::ops::Range<usize> = 32..48;
    const PATH_LEN: std::ops::Range<usize> = 48..56;
    const PATH_START: usize = 56;

    fn read(data: &[u8]) -> Result<SimpleAssetReferenceValue<'_>, SimpleAssetReferenceValueError> {
        if data.len() < Self::PATH_START {
            return Err(SimpleAssetReferenceValueError::TooShort {
                expected: Self::PATH_START,
                actual: data.len(),
            });
        }
        if data[Self::ASSET_ID_EXTENSION].iter().any(|byte| *byte != 0) {
            return Err(SimpleAssetReferenceValueError::NonZeroAssetIdExtension);
        }

        let guid = Uuid::from_bytes(data[0..16].try_into().expect("guid slice is sixteen bytes"));
        let asset_type = Uuid::from_bytes(
            data[Self::ASSET_TYPE]
                .try_into()
                .expect("asset type slice is sixteen bytes"),
        );
        let path_len: usize = u64::from_be_bytes(
            data[Self::PATH_LEN]
                .try_into()
                .expect("path length slice is eight bytes"),
        )
        .try_into()?;
        let path_end = Self::PATH_START + path_len;
        if path_end != data.len() {
            return Err(SimpleAssetReferenceValueError::InvalidPathLength {
                declared: path_len,
                actual: data.len().saturating_sub(Self::PATH_START),
            });
        }
        let path = std::str::from_utf8(&data[Self::PATH_START..path_end])?;
        Ok(SimpleAssetReferenceValue::new(guid, asset_type, path))
    }
}

#[derive(Debug, Clone)]
struct AssetValueLayout {
    sub_id: SubIdLayout,
    asset_type_offset: usize,
    hint_len_offset: usize,
    hint_offset: usize,
    reserved: Option<std::ops::Range<usize>>,
}

impl AssetValueLayout {
    const PADDED: Self = Self {
        sub_id: SubIdLayout::U32,
        asset_type_offset: 32,
        hint_len_offset: 48,
        hint_offset: 56,
        reserved: Some(20..32),
    };
    const U32_RESERVED: Self = Self {
        sub_id: SubIdLayout::U32,
        asset_type_offset: 24,
        hint_len_offset: 40,
        hint_offset: 48,
        reserved: Some(20..24),
    };
    const U64_SUB_ID: Self = Self {
        sub_id: SubIdLayout::U64,
        asset_type_offset: 24,
        hint_len_offset: 40,
        hint_offset: 48,
        reserved: None,
    };
    const COMPACT: Self = Self {
        sub_id: SubIdLayout::U32,
        asset_type_offset: 20,
        hint_len_offset: 36,
        hint_offset: 44,
        reserved: None,
    };
    const CANDIDATES: &'static [Self] = &[
        Self::PADDED,
        Self::U32_RESERVED,
        Self::U64_SUB_ID,
        Self::COMPACT,
    ];

    fn read(data: &[u8]) -> Result<AssetValue<'_>, AssetValueError> {
        let minimum_len = Self::CANDIDATES
            .iter()
            .map(|layout| layout.hint_offset)
            .min()
            .expect("asset layouts are non-empty");
        if data.len() < minimum_len {
            return Err(AssetValueError::TooShort {
                expected: minimum_len,
                actual: data.len(),
            });
        }

        for layout in Self::CANDIDATES {
            if let Some(value) = layout.try_read(data)? {
                return Ok(value);
            }
        }

        Err(AssetValueError::UnsupportedLayout(data.len()))
    }

    fn try_read<'a>(&self, data: &'a [u8]) -> Result<Option<AssetValue<'a>>, AssetValueError> {
        if data.len() < self.hint_offset {
            return Ok(None);
        }
        if let Some(reserved) = &self.reserved
            && data[reserved.clone()].iter().any(|byte| *byte != 0)
        {
            return Ok(None);
        }

        let Ok(hint_len) = usize::try_from(u64::from_be_bytes(
            data[self.hint_len_offset..self.hint_len_offset + 8]
                .try_into()
                .expect("hint length slice is eight bytes"),
        )) else {
            return Ok(None);
        };
        let available = data.len() - self.hint_offset;
        if hint_len != available {
            return Ok(None);
        }

        let guid = Uuid::from_bytes(data[0..16].try_into().expect("guid slice is sixteen bytes"));
        let sub_id = self.sub_id.read(data)?;
        let asset_type = Uuid::from_bytes(
            data[self.asset_type_offset..self.asset_type_offset + 16]
                .try_into()
                .expect("asset type slice is sixteen bytes"),
        );
        let hint = std::str::from_utf8(&data[self.hint_offset..])?;

        Ok(Some(AssetValue::new(guid, sub_id, asset_type, hint)))
    }
}

#[derive(Debug, Clone, Copy)]
enum SubIdLayout {
    U32,
    U64,
}

impl SubIdLayout {
    fn read(self, data: &[u8]) -> Result<u32, AssetValueError> {
        match self {
            Self::U32 => Ok(u32::from_be_bytes(
                data[16..20].try_into().expect("sub id slice is four bytes"),
            )),
            Self::U64 => Ok(u64::from_be_bytes(
                data[16..24]
                    .try_into()
                    .expect("sub id slice is eight bytes"),
            )
            .try_into()?),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Element;

    #[test]
    fn reads_padded_asset_value() {
        let guid = uuid!("7a1472d1-df54-5362-bc71-9974d5f25572");
        let asset_type = uuid!("78802abf-9595-463a-8d2b-d022f906f9b1");
        let element =
            Element::new(types::ASSET).with_data(asset_bytes(guid, 2, asset_type, "bobber"));

        let asset = read_asset_value(&element).unwrap();

        assert_eq!(asset.guid(), guid);
        assert_eq!(asset.sub_id(), 2);
        assert_eq!(asset.asset_type(), asset_type);
        assert_eq!(asset.hint(), "bobber");
    }

    #[test]
    fn reads_source_uuid_sub_id_from_first_uuid_word() {
        let guid = uuid!("699fa9e5-4f8a-5b01-87b2-d5f718c927b8");
        let source_sub_id = uuid!("d087f9c9-0000-0000-0000-000000000000");
        let asset_type = uuid!("c2869e3b-dda0-4e01-8fe3-6770d788866b");
        let element = Element::new(types::ASSET).with_data(source_uuid_sub_id_asset_bytes(
            guid,
            source_sub_id,
            asset_type,
            "slices/dungeon/firstlight/ancientgrate_circular__28236438930.cgf",
        ));

        let asset = read_asset_value(&element).unwrap();

        assert_eq!(asset.guid(), guid);
        assert_eq!(asset.sub_id(), 0xd087_f9c9);
        assert_eq!(asset.asset_type(), asset_type);
    }

    #[test]
    fn reads_u32_reserved_asset_value() {
        let guid = uuid!("7a1472d1-df54-5362-bc71-9974d5f25572");
        let asset_type = uuid!("78802abf-9595-463a-8d2b-d022f906f9b1");
        let element = Element::new(types::ASSET).with_data(u32_reserved_asset_bytes(
            guid,
            2,
            asset_type,
            "slices/spawner.slice",
        ));

        let asset = read_asset_value(&element).unwrap();

        assert_eq!(asset.guid(), guid);
        assert_eq!(asset.sub_id(), 2);
        assert_eq!(asset.asset_type(), asset_type);
        assert_eq!(asset.hint(), "slices/spawner.slice");
    }

    #[test]
    fn reads_simple_asset_reference_value() {
        let guid = uuid!("7a1472d1-df54-5362-bc71-9974d5f25572");
        let asset_type = uuid!("78802abf-9595-463a-8d2b-d022f906f9b1");
        let element = Element::new(SIMPLE_ASSET_REFERENCE_TYPE_ID).with_data(
            simple_asset_reference_bytes(guid, asset_type, "objects/cannon.cgf"),
        );

        let asset = read_simple_asset_reference_value(&element).unwrap();

        assert_eq!(asset.guid(), guid);
        assert_eq!(asset.asset_type(), asset_type);
        assert_eq!(asset.path(), "objects/cannon.cgf");
    }

    #[test]
    fn reads_nested_simple_asset_reference_path() {
        let element =
            Element::new(SIMPLE_TEXTURE_ASSET_REFERENCE_TYPE_ID).with_children([Element::new(
                SIMPLE_ASSET_REFERENCE_BASE_TYPE_ID,
            )
            .with_field("BaseClass1")
            .with_children([Element::new(types::AZSTD_STRING)
                .with_field(ASSET_PATH_FIELD)
                .with_data(b"textures/icon.dds")])]);

        let path =
            read_simple_asset_reference_path(&element, SIMPLE_TEXTURE_ASSET_REFERENCE_TYPE_ID)
                .unwrap();

        assert_eq!(path, "textures/icon.dds");
    }

    #[test]
    fn reads_relaxed_simple_asset_reference_path_direct_field() {
        let element = Element::new(SIMPLE_ASSET_REFERENCE_TYPE_ID).with_children([Element::new(
            types::AZSTD_STRING,
        )
        .with_field(ASSET_PATH_FIELD)
        .with_data(b" objects/cannon.cgf ".as_slice())]);

        let path = read_simple_asset_reference_path_any(&element).unwrap();

        assert_eq!(path, Some("objects/cannon.cgf"));
    }

    #[test]
    fn reads_relaxed_simple_asset_reference_path_nested_field() {
        let element =
            Element::new(SIMPLE_TEXTURE_ASSET_REFERENCE_TYPE_ID).with_children([Element::new(
                SIMPLE_ASSET_REFERENCE_BASE_TYPE_ID,
            )
            .with_field(SIMPLE_ASSET_REFERENCE_BASE)
            .with_children([Element::new(types::AZSTD_STRING)
                .with_field(ASSET_PATH_FIELD)
                .with_data(b"textures/icon.dds")])]);

        let path = read_simple_asset_reference_path_any_owned(&element).unwrap();

        assert_eq!(path, Some("textures/icon.dds".to_string()));
    }

    #[test]
    fn skips_blank_relaxed_simple_asset_reference_path() {
        let element = Element::new(SIMPLE_ASSET_REFERENCE_TYPE_ID).with_children([Element::new(
            types::AZSTD_STRING,
        )
        .with_field(ASSET_PATH_FIELD)
        .with_data(b"  ".as_slice())]);

        let path = read_simple_asset_reference_path_any(&element).unwrap();

        assert_eq!(path, None);
    }

    #[test]
    fn reads_asset_path_or_string_from_string_payload() {
        let element = Element::new(types::AZSTD_STRING).with_data(b" textures/cubemap.dds ");

        let path = read_asset_path_or_string(&element).unwrap();

        assert_eq!(path, Some("textures/cubemap.dds"));
    }

    #[test]
    fn reads_asset_path_or_string_from_simple_reference() {
        let element =
            Element::new(SIMPLE_TEXTURE_ASSET_REFERENCE_TYPE_ID).with_children([Element::new(
                SIMPLE_ASSET_REFERENCE_BASE_TYPE_ID,
            )
            .with_field(SIMPLE_ASSET_REFERENCE_BASE)
            .with_children([Element::new(types::AZSTD_STRING)
                .with_field(ASSET_PATH_FIELD)
                .with_data(b"textures/cubemap.dds")])]);

        let path = read_asset_path_or_string_owned(&element).unwrap();

        assert_eq!(path, Some("textures/cubemap.dds".to_string()));
    }

    #[test]
    fn skips_blank_asset_path_string() {
        let element = Element::new(types::AZSTD_STRING).with_data(b"   ");

        let path = read_asset_path_or_string(&element).unwrap();

        assert_eq!(path, None);
    }

    #[test]
    fn reads_asset_hint_from_text_payload() {
        let element = Element::new(types::ASSET).with_data(
            b"id={1E9A1948-F2A6-5500-B918-964558497331}:0,type={F46985B5-F7FF-4FCB-8E8C-DC240D701841},hint={materials/foo.mtl}",
        );

        assert_eq!(asset_hint(&element).unwrap(), Some("materials/foo.mtl"));
    }

    #[test]
    fn reads_asset_hint_from_big_endian_layout() {
        let hint = "slices/foo.slice";
        let mut data = vec![0; 48];
        data[40..48].copy_from_slice(&(hint.len() as u64).to_be_bytes());
        data.extend_from_slice(hint.as_bytes());
        let element = Element::new(types::ASSET).with_data(data);

        assert_eq!(asset_hint(&element).unwrap(), Some(hint));
    }

    #[test]
    fn reads_owned_asset_hint_from_data_without_type_check() {
        let element = Element::new(uuid!("aaaaaaaa-bbbb-cccc-dddd-eeeeeeeeeeee")).with_data(
            b"id={1E9A1948-F2A6-5500-B918-964558497331}:0,type={F46985B5-F7FF-4FCB-8E8C-DC240D701841},hint={scripts/foo.lua}",
        );

        let hint = asset_hint_from_data_owned(&element).unwrap();

        assert_eq!(hint, Some("scripts/foo.lua".to_string()));
    }

    #[test]
    fn reads_optional_asset_hint_from_data_without_type_check() {
        let missing = Element::new(uuid!("aaaaaaaa-bbbb-cccc-dddd-eeeeeeeeeeee"));
        assert_eq!(optional_asset_hint_from_data_owned(&missing).unwrap(), None);

        let present = Element::new(uuid!("aaaaaaaa-bbbb-cccc-dddd-eeeeeeeeeeee")).with_data(
            b"id={1E9A1948-F2A6-5500-B918-964558497331}:0,type={F46985B5-F7FF-4FCB-8E8C-DC240D701841},hint={scripts/foo.lua}",
        );
        assert_eq!(
            optional_asset_hint_from_data(&present).unwrap(),
            Some("scripts/foo.lua")
        );

        let malformed = Element::new(uuid!("aaaaaaaa-bbbb-cccc-dddd-eeeeeeeeeeee")).with_data([1]);
        assert!(matches!(
            optional_asset_hint_from_data(&malformed).unwrap_err(),
            AssetHintError::TooShort {
                expected: 48,
                actual: 1
            }
        ));
    }

    #[test]
    fn reads_typed_asset_hint_or_simple_path() {
        let typed = Element::new(uuid!("aaaaaaaa-bbbb-cccc-dddd-eeeeeeeeeeee")).with_data(
            b"id={1E9A1948-F2A6-5500-B918-964558497331}:0,type={F46985B5-F7FF-4FCB-8E8C-DC240D701841},hint={scripts/foo.lua}",
        );
        assert_eq!(
            typed_asset_hint_or_simple_path(&typed).unwrap(),
            Some("scripts/foo.lua")
        );

        let simple =
            Element::new(SIMPLE_TEXTURE_ASSET_REFERENCE_TYPE_ID).with_children([Element::new(
                SIMPLE_ASSET_REFERENCE_BASE_TYPE_ID,
            )
            .with_field(SIMPLE_ASSET_REFERENCE_BASE)
            .with_children([Element::new(types::AZSTD_STRING)
                .with_field(ASSET_PATH_FIELD)
                .with_data(b"textures/icon.dds")])]);
        assert_eq!(
            typed_asset_hint_or_simple_path_owned(&simple).unwrap(),
            Some("textures/icon.dds".to_string())
        );

        let blank = Element::new(SIMPLE_ASSET_REFERENCE_TYPE_ID).with_children([Element::new(
            types::AZSTD_STRING,
        )
        .with_field(ASSET_PATH_FIELD)
        .with_data(b"  ".as_slice())]);
        assert_eq!(typed_asset_hint_or_simple_path(&blank).unwrap(), None);

        let malformed = Element::new(uuid!("aaaaaaaa-bbbb-cccc-dddd-eeeeeeeeeeee")).with_data([1]);
        assert!(matches!(
            typed_asset_hint_or_simple_path(&malformed).unwrap_err(),
            TypedAssetOrSimpleReferenceError::Hint(AssetHintError::TooShort {
                expected: 48,
                actual: 1
            })
        ));
    }

    #[test]
    fn reads_owned_asset_hint_from_typed_asset() {
        let hint = "materials/foo.mtl";
        let mut data = vec![0; 48];
        data[40..48].copy_from_slice(&(hint.len() as u64).to_be_bytes());
        data.extend_from_slice(hint.as_bytes());
        let element = Element::new(types::ASSET).with_data(data);

        let hint = asset_hint_owned(&element).unwrap();

        assert_eq!(hint, Some("materials/foo.mtl".to_string()));
    }

    fn asset_bytes(guid: Uuid, sub_id: u32, asset_type: Uuid, hint: &str) -> Vec<u8> {
        let mut bytes = Vec::with_capacity(56 + hint.len());
        bytes.extend_from_slice(guid.as_bytes());
        bytes.extend_from_slice(&sub_id.to_be_bytes());
        bytes.extend_from_slice(&[0; 12]);
        bytes.extend_from_slice(asset_type.as_bytes());
        bytes.extend_from_slice(&(hint.len() as u64).to_be_bytes());
        bytes.extend_from_slice(hint.as_bytes());
        bytes
    }

    fn source_uuid_sub_id_asset_bytes(
        guid: Uuid,
        sub_id: Uuid,
        asset_type: Uuid,
        hint: &str,
    ) -> Vec<u8> {
        let mut bytes = Vec::with_capacity(56 + hint.len());
        bytes.extend_from_slice(guid.as_bytes());
        bytes.extend_from_slice(sub_id.as_bytes());
        bytes.extend_from_slice(asset_type.as_bytes());
        bytes.extend_from_slice(&(hint.len() as u64).to_be_bytes());
        bytes.extend_from_slice(hint.as_bytes());
        bytes
    }

    fn u32_reserved_asset_bytes(guid: Uuid, sub_id: u32, asset_type: Uuid, hint: &str) -> Vec<u8> {
        let mut bytes = Vec::with_capacity(48 + hint.len());
        bytes.extend_from_slice(guid.as_bytes());
        bytes.extend_from_slice(&sub_id.to_be_bytes());
        bytes.extend_from_slice(&[0; 4]);
        bytes.extend_from_slice(asset_type.as_bytes());
        bytes.extend_from_slice(&(hint.len() as u64).to_be_bytes());
        bytes.extend_from_slice(hint.as_bytes());
        bytes
    }

    fn simple_asset_reference_bytes(guid: Uuid, asset_type: Uuid, path: &str) -> Vec<u8> {
        let mut bytes = Vec::with_capacity(56 + path.len());
        bytes.extend_from_slice(guid.as_bytes());
        bytes.extend_from_slice(&[0; 16]);
        bytes.extend_from_slice(asset_type.as_bytes());
        bytes.extend_from_slice(&(path.len() as u64).to_be_bytes());
        bytes.extend_from_slice(path.as_bytes());
        bytes
    }
}
