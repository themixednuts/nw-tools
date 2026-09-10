//! Type UUID constants used by ObjectStream decoding.
//!
//! These constants cover common primitive, container, and math value
//! types. Larger class registries can be supplied through lookup data.

use std::array::TryFromSliceError;

use serde_json::{Number, Value, json};
use uuid::{Uuid, uuid};

use crate::asset_reference;
use crate::type_uuid::{self, type_ids};

pub use crate::type_uuid::type_ids::{
    AABB, AZ_UUID, AZSTD_ARRAY, AZSTD_BASIC_STRING, AZSTD_BASIC_STRING_VIEW, AZSTD_BITSET,
    AZSTD_CHAR_TRAITS, AZSTD_EQUAL_TO, AZSTD_FIXED_FORWARD_LIST, AZSTD_FIXED_LIST,
    AZSTD_FIXED_VECTOR, AZSTD_FORWARD_LIST, AZSTD_FUNCTION, AZSTD_GREATER, AZSTD_GREATER_EQUAL,
    AZSTD_HASH, AZSTD_INTRUSIVE_PTR, AZSTD_LESS, AZSTD_LESS_EQUAL, AZSTD_LIST, AZSTD_MAP,
    AZSTD_MONOSTATE, AZSTD_OPTIONAL, AZSTD_PAIR, AZSTD_SET, AZSTD_SHARED_PTR, AZSTD_STRING,
    AZSTD_STRING_LEGACY_XML, AZSTD_STRING_XML_ALIAS, AZSTD_UNORDERED_MAP, AZSTD_UNORDERED_MULTIMAP,
    AZSTD_UNORDERED_MULTISET, AZSTD_UNORDERED_SET, AZSTD_VECTOR, AZSTD_VECTOR_XML_ALIAS, BOOL,
    CHAR, COLOR, COLORB, COLORF, CRC32, DOUBLE, ENTITY_ID, FLOAT, INT, LONG, MATRIX3X3, MATRIX4X4,
    OBB, PLANE, PLATFORM_ID, QUATERNION, SHORT, SIGNED_CHAR, TRANSFORM, VARIANT, VECTOR_FLOAT,
    VECTOR2, VECTOR3, VECTOR4, VOID,
};

pub const AZ_S8: Uuid = type_ids::S8;
pub const AZ_S64: Uuid = type_ids::S64;
pub const UNSIGNED_CHAR: Uuid = type_ids::U8;
pub const UNSIGNED_SHORT: Uuid = type_ids::U16;
pub const UNSIGNED_INT: Uuid = type_ids::U32;
pub const UNSIGNED_LONG: Uuid = type_ids::ULONG;
pub const AZ_U64: Uuid = type_ids::U64;
/// Folded `AZStd::vector<AZ::ComponentId>` used for component-id lists.
pub const COMPONENT_ID_VECTOR: Uuid = type_uuid::azstd_vector(type_ids::U64);

/// Reflected `AZ::Entity` root object used by slice/UI ObjectStreams.
pub const AZ_ENTITY: Uuid = type_ids::AZ_ENTITY;

/// Reflected `SliceComponent` wrapper for embedded slice entities.
pub const SLICE_COMPONENT: Uuid = uuid!("AFD304E4-1773-47C8-855A-8B622398934F");

pub const ASSET: Uuid = type_ids::AZ_DATA_ASSET_REFLECTION;
pub const ASSET_ID: Uuid = type_ids::AZ_DATA_ASSET_ID;
pub const BYTE_STREAM: Uuid = type_ids::BYTE_STREAM;

/// Render an `Element`'s raw `data` into a typed JSON [`Value`]
/// based on the type UUID.
///
/// Falls back to a UTF-8 string interpretation if `id` doesn't match
/// any known primitive / container.
///
/// # Panics
///
/// Panics if a vector-like payload is not four-byte aligned.
pub fn uuid_data_to_serialize(
    id: &Uuid,
    data: &[u8],
    is_json: bool,
) -> Result<Value, TryFromSliceError> {
    let res = match *id {
        CHAR | AZ_S8 | SIGNED_CHAR => Value::Number(i8::from_be_bytes(data.try_into()?).into()),
        SHORT => Value::Number(i16::from_be_bytes(data.try_into()?).into()),
        INT => Value::Number(i32::from_be_bytes(data.try_into()?).into()),
        LONG | AZ_S64 => Value::Number(i64::from_be_bytes(data.try_into()?).into()),

        UNSIGNED_CHAR => Value::Number(u8::from_be_bytes(data.try_into()?).into()),
        UNSIGNED_SHORT => Value::Number(u16::from_be_bytes(data.try_into()?).into()),
        UNSIGNED_INT => Value::Number(u32::from_be_bytes(data.try_into()?).into()),
        UNSIGNED_LONG | AZ_U64 => Value::Number(u64::from_be_bytes(data.try_into()?).into()),

        FLOAT => json!(format!("{:.7}", f32::from_be_bytes(data.try_into()?))),
        DOUBLE => json!(format!("{:.7}", f64::from_be_bytes(data.try_into()?))),

        BOOL => Value::Bool(u8::from_be_bytes(data.try_into()?) != 0),

        AZ_UUID => json!(
            Uuid::from_bytes(data.try_into()?)
                .braced()
                .encode_upper(&mut Uuid::encode_buffer())
        ),

        ASSET => asset_value(data, is_json).unwrap_or_else(|| utf8_or_empty(data)),
        ASSET_ID => asset_id_value(data, is_json).unwrap_or_else(|| utf8_or_empty(data)),

        VECTOR_FLOAT | VECTOR2 | VECTOR3 | VECTOR4 | TRANSFORM | QUATERNION | COLOR | MATRIX3X3
        | MATRIX4X4 => {
            assert!(data.len().is_multiple_of(4));
            let floats = data
                .chunks_exact(4)
                .map(|b| f32::from_be_bytes(b.try_into().unwrap()));

            if is_json {
                Value::Array(floats.map(float_json_number).collect())
            } else {
                json!(
                    floats
                        .map(|num| format!("{num:.7}"))
                        .collect::<Vec<_>>()
                        .join(" ")
                )
            }
        }

        AZSTD_STRING | AZSTD_BASIC_STRING | AZSTD_STRING_XML_ALIAS => {
            json!(String::from_utf8_lossy(data))
        }
        BYTE_STREAM => json!(hex::encode_upper(data)),

        _ => utf8_or_empty(data),
    };
    Ok(res)
}

/// Spell an `AZ::Data::Asset` payload: the asset id, the asset type, and
/// the hint. JSON takes the object [`crate::serialize_value_to_uuid_data`]
/// reads back; XML keeps the `id=...,type=...,hint={...}` text the engine
/// writes.
///
/// `None` for a payload in no layout [`asset_reference::read_asset_value_bytes`]
/// knows, which is a payload this module cannot spell without losing bytes.
fn asset_value(data: &[u8], is_json: bool) -> Option<Value> {
    let asset = asset_reference::read_asset_value_bytes(data).ok()?;
    let mut guid_buf = Uuid::encode_buffer();
    let guid = asset.guid().braced().encode_upper(&mut guid_buf);
    let mut type_buf = Uuid::encode_buffer();
    let asset_type = asset.asset_type().braced().encode_upper(&mut type_buf);
    let sub_id = asset.sub_id();
    let hint = asset.hint();

    Some(if is_json {
        json!({
            "assetId": json!({ "guid": guid, "subId": sub_id }),
            "type": asset_type,
            "hint": hint,
        })
    } else {
        json!(format!(
            "id={guid}:{sub_id},type={asset_type},hint={{{hint}}}"
        ))
    })
}

/// Spell an `AZ::Data::AssetId` payload: sixteen guid bytes and a
/// big-endian `u32` sub-id.
///
/// `None` for a payload of any other width. A wider one holds something
/// this module does not know about, and spelling only its first twenty
/// bytes would drop the rest on the way back.
fn asset_id_value(data: &[u8], is_json: bool) -> Option<Value> {
    let guid: [u8; 16] = data.get(..16)?.try_into().ok()?;
    let sub_id: [u8; 4] = data.get(16..)?.try_into().ok()?;
    let sub_id = u32::from_be_bytes(sub_id);
    let mut buf = Uuid::encode_buffer();
    let guid = Uuid::from_bytes(guid).braced().encode_upper(&mut buf);

    Some(if is_json {
        json!({ "guid": guid, "subId": sub_id })
    } else {
        json!(format!("{guid}:{sub_id}"))
    })
}

/// The spelling a payload with no typed parse keeps: its UTF-8 text, or
/// an empty string when the bytes are not text.
fn utf8_or_empty(data: &[u8]) -> Value {
    match std::str::from_utf8(data) {
        Ok(text) => json!(text),
        Err(_) => json!(""),
    }
}

/// Render an `Element`'s raw `data` as the JSON `value` member a JSON
/// writer emits.
///
/// [`uuid_data_to_serialize`] spells the typed shape; a shape the JSON
/// reader has no parse for keeps its textual form here. Float sequences
/// stay a JSON array, and asset values stay a JSON object, because those
/// are the forms the reader takes back: folding either into a string
/// produced a literal that no reverse conversion accepted, and the
/// element came back with no payload at all.
///
/// # Errors
///
/// Returns the [`TryFromSliceError`] from [`uuid_data_to_serialize`] when
/// `data` is not the width the type expects.
pub fn uuid_data_to_json(id: &Uuid, data: &[u8]) -> Result<Value, TryFromSliceError> {
    let value = uuid_data_to_serialize(id, data, true)?;
    Ok(match value {
        Value::String(_) | Value::Array(_) | Value::Object(_) => value,
        other => Value::String(other.to_string()),
    })
}

/// Spell one float the way this module spells scalars — seven decimals —
/// but as a JSON number rather than a quoted string.
///
/// A value JSON cannot write as a number, such as a NaN or an infinity,
/// keeps the textual form so the element still carries its payload.
fn float_json_number(value: f32) -> Value {
    let text = format!("{value:.7}");
    text.parse::<f64>()
        .ok()
        .and_then(Number::from_f64)
        .map_or(Value::String(text), Value::Number)
}
