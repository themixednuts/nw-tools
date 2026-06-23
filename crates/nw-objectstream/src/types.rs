//! Type UUID constants used by ObjectStream decoding.
//!
//! These constants cover common primitive, container, and math value
//! types. Larger class registries can be supplied through lookup data.

use std::array::TryFromSliceError;

use serde_json::{Value, json};
use uuid::{Uuid, uuid};

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
/// Panics if an asset payload declares a hint length that does not match the
/// remaining bytes, or if a vector-like payload is not four-byte aligned.
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

        ASSET => {
            let mut buf = Uuid::encode_buffer();
            let guid = Uuid::from_bytes(data[0..16].try_into()?)
                .braced()
                .encode_upper(&mut buf);
            let sub_id = u64::from_be_bytes(data[16..32].try_into()?);
            let mut buf = Uuid::encode_buffer();
            let asset_type = Uuid::from_bytes(data[32..48].try_into()?)
                .braced()
                .encode_upper(&mut buf);
            let size = u64::from_be_bytes(data[48..56].try_into()?);
            let hint = String::from_utf8_lossy(&data[56..]);
            assert_eq!(
                hint.len(),
                usize::try_from(size).expect("asset hint length fits usize")
            );
            if is_json {
                json!({"assetId": json!({ "guid": guid, "subId": sub_id}), "type": asset_type, "hint": hint})
            } else {
                json!(format!(
                    "id={guid}:{sub_id},type={asset_type},hint={{{hint}}}"
                ))
            }
        }
        ASSET_ID => {
            let mut buf = Uuid::encode_buffer();
            let guid = Uuid::from_bytes(data[0..16].try_into()?)
                .braced()
                .encode_upper(&mut buf);
            let sub_id = u32::from_be_bytes(data[16..20].try_into()?);
            if is_json {
                json!({ "guid": guid, "subId": sub_id })
            } else {
                json!(format!("{guid}:{sub_id}"))
            }
        }

        VECTOR_FLOAT | VECTOR2 | VECTOR3 | VECTOR4 | TRANSFORM | QUATERNION | COLOR | MATRIX3X3
        | MATRIX4X4 => {
            assert!(data.len().is_multiple_of(4));
            let data = data.chunks_exact(4);
            let data = data.map(|b| {
                let num = f32::from_be_bytes(b.try_into().unwrap());
                format!("{num:.7}")
            });

            if is_json {
                Value::Array(data.map(|v| json!(v)).collect())
            } else {
                json!(data.collect::<Vec<_>>().join(" "))
            }
        }

        AZSTD_STRING | AZSTD_BASIC_STRING | AZSTD_STRING_XML_ALIAS => {
            json!(String::from_utf8_lossy(data))
        }
        BYTE_STREAM => json!(hex::encode_upper(data)),

        _ => match String::from_utf8(data.into()) {
            Ok(string) => json!(string),
            _ => json!(""),
        },
    };
    Ok(res)
}
