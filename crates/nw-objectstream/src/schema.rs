//! Tooling-only reflected ObjectStream value surface.
//!
//! This module is an import adapter, not native Prefab source. It preserves
//! reflected type/field metadata while legacy tooling proves coverage and
//! generates exact native component payloads.

use std::collections::BTreeMap;

use crate::lookup::NameLookup;
use crate::value::{self, ObjectStreamValueError};
use crate::{Element, types};
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Number, Value};
use thiserror::Error;
use uuid::Uuid;

const AMAZON_PERVASIVES_UID_TYPE_ID: Uuid = uuid::uuid!("3485F20A-98C0-5315-876B-21BCD23A7BC0");

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct SchemaFieldMap {
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub values: BTreeMap<String, SchemaFieldValue>,
}

impl SchemaFieldMap {
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.values.is_empty()
    }

    pub fn to_serde_value(&self) -> Result<Value, SchemaValueSerdeError> {
        map_to_serde_value(None, &self.values)
    }

    /// Find a reflected field using AZ's case-insensitive authoring-name
    /// semantics. This also accepts the uniqueness suffix used when malformed
    /// inputs repeat a field so callers can diagnose duplicates explicitly.
    #[must_use]
    pub fn field(&self, name: &str) -> Option<&SchemaFieldValue> {
        self.values
            .iter()
            .find(|(candidate, _)| {
                candidate
                    .split_once('#')
                    .map_or(candidate.as_str(), |(base, _)| base)
                    .eq_ignore_ascii_case(name)
            })
            .map(|(_, value)| value)
    }

    /// Count reflected fields with the same AZ authoring name, including the
    /// codec's `#N` suffix for duplicate serialized fields.
    #[must_use]
    pub fn field_count(&self, name: &str) -> usize {
        self.values
            .keys()
            .filter(|candidate| {
                candidate
                    .split_once('#')
                    .map_or(candidate.as_str(), |(base, _)| base)
                    .eq_ignore_ascii_case(name)
            })
            .count()
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SchemaFieldValue {
    pub schema: SchemaValueSchema,
    pub value: SchemaValue,
}

impl SchemaFieldValue {
    pub fn to_serde_value(&self) -> Result<Value, SchemaValueSerdeError> {
        schema_value_to_serde_value(&self.schema, &self.value)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SchemaValueSchema {
    pub type_id: Uuid,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub type_name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub version: Option<u8>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub field_id: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub field_name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub specialization: Option<Uuid>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum SchemaValue {
    Unit,
    Bool(bool),
    I64(i64),
    U64(u64),
    F64(f64),
    F64List(Vec<f64>),
    String(String),
    Uuid(Uuid),
    EntityId(u64),
    AssetId(SchemaAssetId),
    Asset(SchemaAsset),
    Bytes(Vec<u8>),
    List(Vec<SchemaFieldValue>),
    Struct(BTreeMap<String, SchemaFieldValue>),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SchemaAssetId {
    pub guid: Uuid,
    pub sub_id: u32,
}

impl SchemaAssetId {
    #[must_use]
    pub fn nil() -> Self {
        Self {
            guid: Uuid::nil(),
            sub_id: 0,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SchemaAsset {
    pub id: SchemaAssetId,
    pub asset_type: Uuid,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hint: Option<String>,
}

#[derive(Debug, Error, Clone, PartialEq)]
pub enum SchemaValueSerdeError {
    #[error("field `{field}` contains non-finite float {value}")]
    NonFiniteFloat { field: String, value: f64 },
    #[error("field `{field}` has {actual} transform values, expected 12")]
    TransformLength { field: String, actual: usize },
}

pub struct SchemaValueCodec<'a> {
    hashes: Option<&'a NameLookup>,
}

impl<'a> SchemaValueCodec<'a> {
    #[must_use]
    pub const fn new(hashes: Option<&'a NameLookup>) -> Self {
        Self { hashes }
    }

    pub fn field_map(&self, element: &Element) -> Result<SchemaFieldMap, SchemaValueCodecError> {
        Ok(SchemaFieldMap {
            values: self.children_to_map(element.children())?,
        })
    }

    pub fn field_value(
        &self,
        element: &Element,
    ) -> Result<SchemaFieldValue, SchemaValueCodecError> {
        Ok(SchemaFieldValue {
            schema: self.schema(element),
            value: self.value(element)?,
        })
    }

    /// Decode one reflected element into a generated SerializeContext type.
    ///
    /// The schema codec performs the ObjectStream wire conversion (including
    /// assets, AZ maps, smart pointers, transforms, and scalar enums), while
    /// the generated type remains the authority for the field shape.
    pub fn deserialize<T>(&self, element: &Element) -> Result<T, SchemaDeserializeError>
    where
        T: DeserializeOwned,
    {
        let value = self.field_value(element)?.to_serde_value()?;
        serde_json::from_value(value).map_err(SchemaDeserializeError::Deserialize)
    }

    fn value(&self, element: &Element) -> Result<SchemaValue, SchemaValueCodecError> {
        if self.is_sequence(element) {
            return element
                .children()
                .iter()
                .map(|child| self.field_value(child))
                .collect::<Result<Vec<_>, _>>()
                .map(SchemaValue::List);
        }

        if self.is_map(element) {
            return self
                .children_to_map(element.children())
                .map(SchemaValue::Struct);
        }

        if element.id() == &types::ENTITY_ID {
            return value::read_entity_id(element)
                .map(SchemaValue::EntityId)
                .map_err(SchemaValueCodecError::Value);
        }

        if element.id() == &types::CRC32 {
            return value::read_crc32(element)
                .map(u64::from)
                .map(SchemaValue::U64)
                .map_err(SchemaValueCodecError::Value);
        }

        if let Some(underlying_type) = self
            .hashes
            .and_then(|hashes| hashes.enum_underlying_type(element.id()))
        {
            return enum_value(element, underlying_type);
        }

        let value = match *element.id() {
            types::BOOL => SchemaValue::Bool(value::read_bool(element)?),
            types::CHAR | types::SIGNED_CHAR | types::AZ_S8 => {
                SchemaValue::I64(i64::from(value::read_i8(element)?))
            }
            types::SHORT => SchemaValue::I64(i64::from(value::read_i16(element)?)),
            types::INT => SchemaValue::I64(i64::from(value::read_i32(element)?)),
            types::LONG | types::AZ_S64 => SchemaValue::I64(value::read_i64(element)?),
            types::UNSIGNED_CHAR => SchemaValue::U64(u64::from(value::read_u8(element)?)),
            types::UNSIGNED_SHORT => SchemaValue::U64(u64::from(value::read_u16(element)?)),
            types::UNSIGNED_INT => SchemaValue::U64(u64::from(value::read_u32(element)?)),
            types::UNSIGNED_LONG | types::AZ_U64 => SchemaValue::U64(value::read_u64(element)?),
            types::FLOAT | types::VECTOR_FLOAT => {
                SchemaValue::F64(value::read_f32_value(element)?.into())
            }
            types::DOUBLE => SchemaValue::F64(value::read_f64(element)?),
            types::AZ_UUID => SchemaValue::Uuid(value::read_uuid(element)?),
            types::AZSTD_STRING | types::AZSTD_BASIC_STRING | types::AZSTD_STRING_LEGACY_XML => {
                SchemaValue::String(value::read_string(element)?.to_owned())
            }
            types::BYTE_STREAM => SchemaValue::Bytes(value::read_byte_stream(element)?.to_vec()),
            types::VECTOR2 => SchemaValue::F64List(f32s_to_f64s(value::read_vec2(element)?)),
            types::VECTOR3 => SchemaValue::F64List(f32s_to_f64s(value::read_vec3(element)?)),
            types::VECTOR4 => SchemaValue::F64List(f32s_to_f64s(value::read_vec4(element)?)),
            types::COLOR => SchemaValue::F64List(f32s_to_f64s(value::read_color(element)?)),
            types::QUATERNION => SchemaValue::F64List(f32s_to_f64s(value::read_quat(element)?)),
            types::TRANSFORM => SchemaValue::F64List(f32s_to_f64s(value::read_transform(element)?)),
            types::MATRIX3X3 => SchemaValue::F64List(f32s_to_f64s(value::read_matrix3x3(element)?)),
            types::MATRIX4X4 => SchemaValue::F64List(f32s_to_f64s(value::read_matrix4x4(element)?)),
            types::ASSET_ID => self.asset_id_value(element)?,
            types::ASSET => self.asset_value(element)?,
            types::VOID | types::AZSTD_MONOSTATE => SchemaValue::Unit,
            _ if !element.children().is_empty() => {
                SchemaValue::Struct(self.children_to_map(element.children())?)
            }
            _ if let Some(data) = element.data() => SchemaValue::Bytes(data.to_vec()),
            // A reflected class with no serialized fields is still a concrete
            // object. Keeping an empty struct preserves its `$type` tag for
            // zero-field polymorphic variants such as QueryShapePoint.
            _ => SchemaValue::Struct(BTreeMap::new()),
        };
        Ok(value)
    }

    fn schema(&self, element: &Element) -> SchemaValueSchema {
        SchemaValueSchema {
            type_id: *element.id(),
            type_name: self.type_name(element),
            version: element.version(),
            field_id: element.name_crc(),
            field_name: self.field_name(element),
            specialization: element.specialization().copied(),
        }
    }

    fn children_to_map(
        &self,
        children: &[Element],
    ) -> Result<BTreeMap<String, SchemaFieldValue>, SchemaValueCodecError> {
        let mut values = BTreeMap::new();
        for (index, child) in children.iter().enumerate() {
            let key = self.unique_field_key(&values, child, index);
            values.insert(key, self.field_value(child)?);
        }
        Ok(values)
    }

    fn unique_field_key(
        &self,
        existing: &BTreeMap<String, SchemaFieldValue>,
        element: &Element,
        index: usize,
    ) -> String {
        let base = self
            .field_name(element)
            .or_else(|| element.name_crc().map(|crc| format!("field_{crc:08x}")))
            .or_else(|| (!element.name().is_empty()).then(|| element.name().to_string()))
            .unwrap_or_else(|| format!("field_{index}"));

        if !existing.contains_key(&base) {
            return base;
        }

        let mut suffix = 1;
        loop {
            let candidate = format!("{base}#{suffix}");
            if !existing.contains_key(&candidate) {
                return candidate;
            }
            suffix += 1;
        }
    }

    fn type_name(&self, element: &Element) -> Option<String> {
        if !element.name().is_empty() {
            return Some(element.name().to_string());
        }
        self.hashes
            .and_then(|hashes| hashes.type_name(element.id()))
            .map(ToString::to_string)
    }

    fn field_name(&self, element: &Element) -> Option<String> {
        element.field().map(ToString::to_string).or_else(|| {
            element
                .name_crc()
                .and_then(|crc| self.hashes.and_then(|hashes| hashes.field_name(crc)))
                .map(ToString::to_string)
        })
    }

    fn asset_id_value(&self, element: &Element) -> Result<SchemaValue, SchemaValueCodecError> {
        let id = match element.data() {
            Some(data) => decode_asset_id(data)?,
            None if element.children().is_empty() => SchemaAssetId::nil(),
            None => {
                let asset_id = crate::asset_id::read_asset_id(element)?;
                SchemaAssetId {
                    guid: asset_id.guid,
                    sub_id: asset_id.sub_id,
                }
            }
        };
        Ok(SchemaValue::AssetId(id))
    }

    fn asset_value(&self, element: &Element) -> Result<SchemaValue, SchemaValueCodecError> {
        let Some(data) = element.data() else {
            return Ok(SchemaValue::Struct(
                self.children_to_map(element.children())?,
            ));
        };
        decode_asset(data)
            .map(SchemaValue::Asset)
            .or_else(|_| Ok(SchemaValue::Bytes(data.to_vec())))
    }

    fn is_sequence(&self, element: &Element) -> bool {
        is_sequence_type(element.id())
            || is_sequence_type_name(element.name())
            || self
                .hashes
                .and_then(|hashes| hashes.type_name(element.id()))
                .is_some_and(|name| is_sequence_type_name(name))
    }

    fn is_map(&self, element: &Element) -> bool {
        is_map_type(element.id())
            || is_map_type_name(element.name())
            || self
                .hashes
                .and_then(|hashes| hashes.type_name(element.id()))
                .is_some_and(|name| is_map_type_name(name))
    }
}

fn enum_value(
    element: &Element,
    underlying_type: Uuid,
) -> Result<SchemaValue, SchemaValueCodecError> {
    let data = element.data().ok_or(SchemaValueCodecError::InvalidLength {
        field: "enum",
        expected: 1,
        actual: 0,
    })?;
    macro_rules! integer {
        ($size:literal, $type:ty, $variant:ident) => {{
            let bytes: [u8; $size] =
                data.try_into()
                    .map_err(|_| SchemaValueCodecError::InvalidLength {
                        field: "enum",
                        expected: $size,
                        actual: data.len(),
                    })?;
            Ok(SchemaValue::$variant(<$type>::from_be_bytes(bytes).into()))
        }};
    }
    match underlying_type {
        types::CHAR | types::SIGNED_CHAR | types::AZ_S8 => integer!(1, i8, I64),
        types::SHORT => integer!(2, i16, I64),
        types::INT => integer!(4, i32, I64),
        types::LONG | types::AZ_S64 => integer!(8, i64, I64),
        types::UNSIGNED_CHAR => integer!(1, u8, U64),
        types::UNSIGNED_SHORT => integer!(2, u16, U64),
        types::UNSIGNED_INT => integer!(4, u32, U64),
        types::UNSIGNED_LONG | types::AZ_U64 => integer!(8, u64, U64),
        _ => Err(SchemaValueCodecError::UnsupportedEnumStorage {
            enum_type: *element.id(),
            underlying_type,
        }),
    }
}

#[derive(Debug, Error)]
pub enum SchemaValueCodecError {
    #[error(transparent)]
    Value(#[from] ObjectStreamValueError),
    #[error("field `{field}` has {actual} value bytes, expected at least {expected}")]
    InvalidLength {
        field: &'static str,
        expected: usize,
        actual: usize,
    },
    #[error("enum type {enum_type} uses unsupported storage type {underlying_type}")]
    UnsupportedEnumStorage {
        enum_type: Uuid,
        underlying_type: Uuid,
    },
}

#[derive(Debug, Error)]
pub enum SchemaDeserializeError {
    #[error(transparent)]
    Codec(#[from] SchemaValueCodecError),
    #[error(transparent)]
    SerdeValue(#[from] SchemaValueSerdeError),
    #[error("deserialize reflected generated type: {0}")]
    Deserialize(serde_json::Error),
}

fn is_sequence_type(id: &Uuid) -> bool {
    matches!(
        *id,
        types::AZSTD_VECTOR
            | types::COMPONENT_ID_VECTOR
            | types::AZSTD_LIST
            | types::AZSTD_FORWARD_LIST
            | types::AZSTD_SET
            | types::AZSTD_UNORDERED_SET
            | types::AZSTD_UNORDERED_MULTISET
            | types::AZSTD_ARRAY
            | types::AZSTD_FIXED_VECTOR
            | types::AZSTD_FIXED_LIST
            | types::AZSTD_FIXED_FORWARD_LIST
    )
}

fn is_sequence_type_name(name: &str) -> bool {
    let base = name.split('<').next().unwrap_or(name).trim();
    matches!(
        base,
        "AZStd::vector"
            | "AZStd::fixed_vector"
            | "AZStd::array"
            | "AZStd::list"
            | "AZStd::fixed_list"
            | "AZStd::forward_list"
            | "AZStd::fixed_forward_list"
            | "AZStd::set"
            | "AZStd::unordered_set"
            | "AZStd::unordered_multiset"
    )
}

fn is_map_type(id: &Uuid) -> bool {
    matches!(
        *id,
        types::AZSTD_MAP | types::AZSTD_UNORDERED_MAP | types::AZSTD_UNORDERED_MULTIMAP
    )
}

fn is_map_type_name(name: &str) -> bool {
    let base = name.split('<').next().unwrap_or(name).trim();
    matches!(
        base,
        "AZStd::map"
            | "AZStd::unordered_map"
            | "AZStd::flat_map"
            | "AZStd::unordered_flat_map"
            | "AZStd::multimap"
            | "AZStd::unordered_multimap"
    )
}

fn f32s_to_f64s<const N: usize>(values: [f32; N]) -> Vec<f64> {
    values.into_iter().map(f64::from).collect()
}

fn schema_value_to_serde_value(
    schema: &SchemaValueSchema,
    value: &SchemaValue,
) -> Result<Value, SchemaValueSerdeError> {
    if is_empty_asset_reference(schema, value) {
        return Ok(asset_reference_to_serde_value(&SchemaAsset {
            id: SchemaAssetId::nil(),
            asset_type: Uuid::nil(),
            hint: None,
        }));
    }

    match value {
        SchemaValue::Unit => Ok(Value::Null),
        SchemaValue::Bool(value) => Ok(Value::Bool(*value)),
        SchemaValue::I64(value) => Ok(Value::Number(Number::from(*value))),
        SchemaValue::U64(value) | SchemaValue::EntityId(value) => {
            Ok(Value::Number(Number::from(*value)))
        }
        SchemaValue::F64(value) => number_from_f64(schema, *value),
        SchemaValue::F64List(values) if schema.type_id == types::TRANSFORM => {
            transform_to_serde_value(schema, values)
        }
        SchemaValue::F64List(values) => f64_list_to_serde_value(schema, values),
        SchemaValue::String(value) if string_field_deserializes_as_bytes(schema) => {
            Ok(Value::Array(
                value
                    .bytes()
                    .map(|byte| Value::Number(byte.into()))
                    .collect(),
            ))
        }
        SchemaValue::String(value) => Ok(Value::String(value.clone())),
        SchemaValue::Uuid(value) => Ok(Value::String(value.to_string())),
        SchemaValue::AssetId(value) => Ok(asset_id_to_serde_value(value)),
        SchemaValue::Asset(value) => Ok(asset_reference_to_serde_value(value)),
        SchemaValue::Bytes(values) if schema.type_id == AMAZON_PERVASIVES_UID_TYPE_ID => {
            amazon_pervasives_uid_to_serde_value(values)
        }
        SchemaValue::Bytes(values) => bytes_to_serde_array(values),
        SchemaValue::List(values) => values
            .iter()
            .map(SchemaFieldValue::to_serde_value)
            .collect::<Result<Vec<_>, _>>()
            .map(Value::Array),
        SchemaValue::Struct(fields) => struct_to_serde_value(schema, fields),
    }
}

fn struct_to_serde_value(
    schema: &SchemaValueSchema,
    fields: &BTreeMap<String, SchemaFieldValue>,
) -> Result<Value, SchemaValueSerdeError> {
    if is_smart_pointer_schema(schema) {
        return smart_pointer_to_serde_value(fields);
    }

    if is_pair_schema(schema) {
        return pair_to_serde_value(fields);
    }

    if is_map_schema(schema) {
        return az_map_to_serde_value(fields);
    }

    let mut map = fields_to_serde_map(schema.type_name.as_deref(), fields)?;
    if !schema.type_id.is_nil() {
        let mut tagged = Map::new();
        tagged.insert(
            "$type".to_owned(),
            Value::String(schema.type_id.to_string()),
        );
        tagged.extend(map);
        map = tagged;
    }
    Ok(Value::Object(map))
}

fn map_to_serde_value(
    parent_type_name: Option<&str>,
    fields: &BTreeMap<String, SchemaFieldValue>,
) -> Result<Value, SchemaValueSerdeError> {
    fields_to_serde_map(parent_type_name, fields).map(Value::Object)
}

fn az_map_to_serde_value(
    fields: &BTreeMap<String, SchemaFieldValue>,
) -> Result<Value, SchemaValueSerdeError> {
    let mut map = Map::new();
    for value in fields.values() {
        let Value::Array(mut pair) = value.to_serde_value()? else {
            return map_to_serde_value(None, fields);
        };
        if pair.len() != 2 {
            return map_to_serde_value(None, fields);
        }
        let value = pair.pop().expect("pair length checked");
        let key = pair.pop().expect("pair length checked");
        map.insert(serde_map_key(key), value);
    }
    Ok(Value::Object(map))
}

fn serde_map_key(value: Value) -> String {
    match value {
        Value::String(value) => value,
        Value::Number(value) => value.to_string(),
        Value::Bool(value) => value.to_string(),
        Value::Null => "null".to_owned(),
        value => value.to_string(),
    }
}

fn fields_to_serde_map(
    parent_type_name: Option<&str>,
    fields: &BTreeMap<String, SchemaFieldValue>,
) -> Result<Map<String, Value>, SchemaValueSerdeError> {
    let mut map = Map::new();
    for (key, value) in fields {
        let field_value = value.to_serde_value()?;
        map.insert(
            canonical_field_name(parent_type_name, key).to_owned(),
            field_value,
        );
    }
    Ok(map)
}

fn number_from_f64(schema: &SchemaValueSchema, value: f64) -> Result<Value, SchemaValueSerdeError> {
    Number::from_f64(value).map(Value::Number).ok_or_else(|| {
        SchemaValueSerdeError::NonFiniteFloat {
            field: schema_field_label(schema),
            value,
        }
    })
}

fn f64_list_to_serde_value(
    schema: &SchemaValueSchema,
    values: &[f64],
) -> Result<Value, SchemaValueSerdeError> {
    values
        .iter()
        .map(|value| number_from_f64(schema, *value))
        .collect::<Result<Vec<_>, _>>()
        .map(Value::Array)
}

fn amazon_pervasives_uid_to_serde_value(values: &[u8]) -> Result<Value, SchemaValueSerdeError> {
    if values.len() != 16 {
        return bytes_to_serde_array(values);
    }
    let bytes = values.try_into().expect("length checked");
    Ok(Value::String(Uuid::from_bytes(bytes).to_string()))
}

fn bytes_to_serde_array(values: &[u8]) -> Result<Value, SchemaValueSerdeError> {
    Ok(Value::Array(
        values
            .iter()
            .map(|value| Value::Number((*value).into()))
            .collect(),
    ))
}

fn transform_to_serde_value(
    schema: &SchemaValueSchema,
    values: &[f64],
) -> Result<Value, SchemaValueSerdeError> {
    let values = transform_columns(schema, values)?;
    let transform = glam::Mat4::from_cols_array(&[
        values[0], values[1], values[2], 0.0, values[3], values[4], values[5], 0.0, values[6],
        values[7], values[8], 0.0, values[9], values[10], values[11], 1.0,
    ]);
    let (scale, rotation, translation) = transform.to_scale_rotation_translation();
    let mut map = Map::new();
    map.insert(
        "translation".to_owned(),
        f32_array_to_serde_value(schema, translation.to_array())?,
    );
    map.insert(
        "rotation".to_owned(),
        f32_array_to_serde_value(schema, rotation.to_array())?,
    );
    map.insert(
        "scale".to_owned(),
        f32_array_to_serde_value(schema, scale.to_array())?,
    );
    Ok(Value::Object(map))
}

fn transform_columns(
    schema: &SchemaValueSchema,
    values: &[f64],
) -> Result<[f32; 12], SchemaValueSerdeError> {
    if values.len() != 12 {
        return Err(SchemaValueSerdeError::TransformLength {
            field: schema_field_label(schema),
            actual: values.len(),
        });
    }

    let mut result = [0.0; 12];
    for (index, value) in values.iter().enumerate() {
        number_from_f64(schema, *value)?;
        result[index] = *value as f32;
    }
    Ok(result)
}

fn f32_array_to_serde_value<const N: usize>(
    schema: &SchemaValueSchema,
    values: [f32; N],
) -> Result<Value, SchemaValueSerdeError> {
    f64_list_to_serde_value(
        schema,
        &values.into_iter().map(f64::from).collect::<Vec<_>>(),
    )
}

fn schema_field_label(schema: &SchemaValueSchema) -> String {
    schema.field_name.clone().unwrap_or_else(|| {
        schema
            .type_name
            .clone()
            .unwrap_or_else(|| schema.type_id.to_string())
    })
}

fn canonical_field_name<'a>(parent_type_name: Option<&str>, field: &'a str) -> &'a str {
    if field.eq_ignore_ascii_case("ID") {
        return match parent_type_name {
            Some("GDEID") => "id",
            _ => "Id",
        };
    }

    if field.eq_ignore_ascii_case("EntityID") {
        return "EntityId";
    }

    field
}

fn is_map_schema(schema: &SchemaValueSchema) -> bool {
    is_map_type(&schema.type_id) || schema.type_name.as_deref().is_some_and(is_map_type_name)
}

fn is_smart_pointer_schema(schema: &SchemaValueSchema) -> bool {
    schema
        .type_name
        .as_deref()
        .is_some_and(is_smart_pointer_type_name)
}

fn is_smart_pointer_type_name(name: &str) -> bool {
    let base = name.split('<').next().unwrap_or(name).trim();
    matches!(
        base,
        "AZStd::shared_ptr" | "AZStd::unique_ptr" | "AZStd::intrusive_ptr" | "AZStd::weak_ptr"
    )
}

fn is_pair_schema(schema: &SchemaValueSchema) -> bool {
    schema.type_name.as_deref().is_some_and(is_pair_type_name)
}

fn is_pair_type_name(name: &str) -> bool {
    name.split('<').next().unwrap_or(name).trim() == "AZStd::pair"
}

fn pair_to_serde_value(
    fields: &BTreeMap<String, SchemaFieldValue>,
) -> Result<Value, SchemaValueSerdeError> {
    let Some(first) = field_case_insensitive(fields, "value1") else {
        return map_to_serde_value(None, fields);
    };
    let Some(second) = field_case_insensitive(fields, "value2") else {
        return map_to_serde_value(None, fields);
    };

    Ok(Value::Array(vec![
        first.to_serde_value()?,
        second.to_serde_value()?,
    ]))
}

fn smart_pointer_to_serde_value(
    fields: &BTreeMap<String, SchemaFieldValue>,
) -> Result<Value, SchemaValueSerdeError> {
    if let Some(value) = field_case_insensitive(fields, "element") {
        return value.to_serde_value();
    }
    if fields.is_empty() {
        return Ok(Value::Null);
    }
    map_to_serde_value(None, fields)
}

fn field_case_insensitive<'a>(
    fields: &'a BTreeMap<String, SchemaFieldValue>,
    field: &str,
) -> Option<&'a SchemaFieldValue> {
    fields.get(field).or_else(|| {
        fields
            .iter()
            .find(|(candidate, _)| candidate.eq_ignore_ascii_case(field))
            .map(|(_, value)| value)
    })
}

fn is_empty_asset_reference(schema: &SchemaValueSchema, value: &SchemaValue) -> bool {
    schema.type_id == types::ASSET
        && matches!(value, SchemaValue::Struct(fields) if fields.is_empty())
}

fn string_field_deserializes_as_bytes(schema: &SchemaValueSchema) -> bool {
    schema
        .field_name
        .as_deref()
        .is_some_and(|field| field.eq_ignore_ascii_case("m_sliceVariant"))
}

fn asset_id_to_serde_value(value: &SchemaAssetId) -> Value {
    let mut map = Map::new();
    map.insert("guid".to_owned(), Value::String(value.guid.to_string()));
    map.insert("sub_id".to_owned(), Value::Number(value.sub_id.into()));
    Value::Object(map)
}

fn asset_reference_to_serde_value(value: &SchemaAsset) -> Value {
    let mut map = Map::new();
    map.insert("asset_id".to_owned(), asset_id_to_serde_value(&value.id));
    map.insert(
        "asset_type".to_owned(),
        Value::String(value.asset_type.to_string()),
    );
    map.insert(
        "hint".to_owned(),
        value
            .hint
            .as_ref()
            .map(|hint| Value::String(hint.clone()))
            .unwrap_or(Value::Null),
    );
    Value::Object(map)
}

fn decode_asset_id(data: &[u8]) -> Result<SchemaAssetId, SchemaValueCodecError> {
    if data.len() < 20 {
        return Err(SchemaValueCodecError::InvalidLength {
            field: "AssetId",
            expected: 20,
            actual: data.len(),
        });
    }
    Ok(SchemaAssetId {
        guid: Uuid::from_bytes(data[0..16].try_into().expect("slice length is checked")),
        sub_id: u32::from_be_bytes(data[16..20].try_into().expect("slice length is checked")),
    })
}

fn decode_asset(data: &[u8]) -> Result<SchemaAsset, SchemaValueCodecError> {
    if data.len() < 48 {
        return Err(SchemaValueCodecError::InvalidLength {
            field: "Asset",
            expected: 48,
            actual: data.len(),
        });
    }

    let id = SchemaAssetId {
        guid: Uuid::from_bytes(data[0..16].try_into().expect("slice length is checked")),
        sub_id: u64::from_be_bytes(data[16..24].try_into().expect("slice length is checked"))
            as u32,
    };
    let asset_type = Uuid::from_bytes(data[24..40].try_into().expect("slice length is checked"));
    let hint_len =
        u64::from_be_bytes(data[40..48].try_into().expect("slice length is checked")) as usize;
    let hint = data
        .get(48..48 + hint_len)
        .and_then(|bytes| std::str::from_utf8(bytes).ok())
        .map(str::to_owned);

    Ok(SchemaAsset {
        id,
        asset_type,
        hint,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_asset_id_element_is_default_asset_id() {
        let codec = SchemaValueCodec::new(None);
        let value = codec
            .field_value(&Element::new(types::ASSET_ID).with_field("ParticleLibraryAssetId"))
            .unwrap();

        assert_eq!(value.value, SchemaValue::AssetId(SchemaAssetId::nil()));
    }

    #[test]
    fn struct_values_preserve_concrete_type_for_polymorphic_deserialization() {
        let type_id = uuid::uuid!("CF33E3AA-E48A-464D-B319-ECD8D0BAD687");
        let value = SchemaFieldValue {
            schema: SchemaValueSchema {
                type_id,
                type_name: Some("DefaultCondition".to_owned()),
                version: None,
                field_id: None,
                field_name: Some("m_condition".to_owned()),
                specialization: None,
            },
            value: SchemaValue::Struct(BTreeMap::new()),
        };

        assert_eq!(
            value.to_serde_value().unwrap(),
            serde_json::json!({ "$type": type_id.to_string() })
        );
    }

    #[test]
    fn nil_struct_values_do_not_emit_synthetic_type_tags() {
        let value = SchemaFieldValue {
            schema: SchemaValueSchema {
                type_id: Uuid::nil(),
                type_name: None,
                version: None,
                field_id: None,
                field_name: None,
                specialization: None,
            },
            value: SchemaValue::Struct(BTreeMap::new()),
        };

        assert_eq!(value.to_serde_value().unwrap(), serde_json::json!({}));
    }

    #[test]
    fn transform_values_emit_bevy_transform_serde_shape() {
        let value = SchemaFieldValue {
            schema: SchemaValueSchema {
                type_id: types::TRANSFORM,
                type_name: Some("Transform".to_owned()),
                version: None,
                field_id: None,
                field_name: Some("Target Offset".to_owned()),
                specialization: None,
            },
            value: SchemaValue::F64List(vec![
                1.0, 0.0, 0.0, //
                0.0, 1.0, 0.0, //
                0.0, 0.0, 1.0, //
                2.0, 3.0, 4.0,
            ]),
        };

        assert_eq!(
            value.to_serde_value().unwrap(),
            serde_json::json!({
                "translation": [2.0, 3.0, 4.0],
                "rotation": [0.0, 0.0, 0.0, 1.0],
                "scale": [1.0, 1.0, 1.0]
            })
        );
    }

    #[test]
    fn transform_values_reject_wrong_length() {
        let value = SchemaFieldValue {
            schema: SchemaValueSchema {
                type_id: types::TRANSFORM,
                type_name: Some("Transform".to_owned()),
                version: None,
                field_id: None,
                field_name: Some("Target Offset".to_owned()),
                specialization: None,
            },
            value: SchemaValue::F64List(vec![1.0; 11]),
        };

        assert_eq!(
            value.to_serde_value().unwrap_err(),
            SchemaValueSerdeError::TransformLength {
                field: "Target Offset".to_owned(),
                actual: 11,
            }
        );
    }

    #[test]
    fn amazon_pervasives_uid_bytes_emit_uuid_serde_shape() {
        let value = SchemaFieldValue {
            schema: SchemaValueSchema {
                type_id: AMAZON_PERVASIVES_UID_TYPE_ID,
                type_name: Some("Amazon::Pervasives::UID<u16>".to_owned()),
                version: None,
                field_id: None,
                field_name: Some("m_actorId".to_owned()),
                specialization: None,
            },
            value: SchemaValue::Bytes([0x11; 16].into()),
        };

        assert_eq!(
            value.to_serde_value().unwrap(),
            serde_json::json!("11111111-1111-1111-1111-111111111111")
        );
    }

    #[test]
    fn specialized_azstd_sequence_type_names_emit_serde_arrays() {
        let mut sequence = Element::new(uuid::uuid!("191CA7C2-92DE-5474-8D2C-A095A6B6EDC6"))
            .with_field("m_overrides")
            .with_children([
                Element::new(types::AZSTD_STRING)
                    .with_field("element")
                    .with_data("spawn"),
                Element::new(types::AZSTD_STRING)
                    .with_field("element")
                    .with_data("despawn"),
            ]);
        sequence.name = "AZStd::vector<MaterialOverrideInfo>".into();

        let value = SchemaValueCodec::new(None).field_value(&sequence).unwrap();

        assert!(matches!(value.value, SchemaValue::List(_)));
        assert_eq!(
            value.to_serde_value().unwrap(),
            serde_json::json!(["spawn", "despawn"])
        );
    }

    #[test]
    fn azstd_map_type_names_are_not_sequences() {
        let mut map = Element::new(uuid::uuid!("6CC71AC2-3B52-5F11-9957-0F6521F0F1C1"))
            .with_field("m_values")
            .with_children([Element::new(types::AZSTD_STRING)
                .with_field("element")
                .with_data("value")]);
        map.name = "AZStd::map<AZStd::string, AZStd::string>".into();

        let value = SchemaValueCodec::new(None).field_value(&map).unwrap();

        assert!(matches!(value.value, SchemaValue::Struct(_)));
        assert_eq!(
            value.to_serde_value().unwrap(),
            serde_json::json!({ "element": "value" })
        );
    }

    #[test]
    fn empty_azstd_map_type_names_emit_serde_objects() {
        let mut map = Element::new(types::AZSTD_MAP).with_field("m_values");
        map.name = "AZStd::map<AZStd::string, AZStd::string>".into();

        let value = SchemaValueCodec::new(None).field_value(&map).unwrap();

        assert!(matches!(value.value, SchemaValue::Struct(_)));
        assert_eq!(value.to_serde_value().unwrap(), serde_json::json!({}));
    }

    #[test]
    fn azstd_map_pairs_emit_serde_maps() {
        let value = SchemaFieldValue {
            schema: SchemaValueSchema {
                type_id: types::AZSTD_MAP,
                type_name: Some("AZStd::map<s32, AZStd::string>".to_owned()),
                version: None,
                field_id: None,
                field_name: Some("m_paperdollVisualSlotMapping".to_owned()),
                specialization: None,
            },
            value: SchemaValue::Struct(BTreeMap::from([
                (
                    "element".to_owned(),
                    pair_field(
                        "element",
                        SchemaValue::I64(0),
                        SchemaValue::String("hat".to_owned()),
                    ),
                ),
                (
                    "element#1".to_owned(),
                    pair_field(
                        "element",
                        SchemaValue::I64(1),
                        SchemaValue::String("shirt".to_owned()),
                    ),
                ),
            ])),
        };

        assert_eq!(
            value.to_serde_value().unwrap(),
            serde_json::json!({ "0": "hat", "1": "shirt" })
        );
    }

    #[test]
    fn azstd_unordered_flat_map_pairs_emit_serde_maps() {
        let value = SchemaFieldValue {
            schema: SchemaValueSchema {
                type_id: uuid::uuid!("2840B537-C3D1-5257-9659-94E0B170825C"),
                type_name: Some(
                    "AZStd::unordered_flat_map<AZStd::string, AZStd::vector<EventData>>".to_owned(),
                ),
                version: None,
                field_id: None,
                field_name: Some("TimelineEventMappings".to_owned()),
                specialization: None,
            },
            value: SchemaValue::Struct(BTreeMap::from([(
                "element".to_owned(),
                pair_field(
                    "element",
                    SchemaValue::String("VO".to_owned()),
                    SchemaValue::List(Vec::new()),
                ),
            )])),
        };

        assert_eq!(
            value.to_serde_value().unwrap(),
            serde_json::json!({ "VO": [] })
        );
    }

    #[test]
    fn azstd_pair_type_names_emit_serde_tuples() {
        let value = SchemaFieldValue {
            schema: SchemaValueSchema {
                type_id: uuid::uuid!("30DDE93C-E899-5AB9-856D-FC456D054EDB"),
                type_name: Some("AZStd::pair<AZ::EntityId, AZ::EntityId>".to_owned()),
                version: None,
                field_id: None,
                field_name: Some("element".to_owned()),
                specialization: None,
            },
            value: SchemaValue::Struct(BTreeMap::from([
                (
                    "value1".to_owned(),
                    SchemaFieldValue {
                        schema: SchemaValueSchema {
                            type_id: types::ENTITY_ID,
                            type_name: Some("AZ::EntityId".to_owned()),
                            version: None,
                            field_id: None,
                            field_name: Some("value1".to_owned()),
                            specialization: None,
                        },
                        value: SchemaValue::EntityId(7),
                    },
                ),
                (
                    "value2".to_owned(),
                    SchemaFieldValue {
                        schema: SchemaValueSchema {
                            type_id: types::ENTITY_ID,
                            type_name: Some("AZ::EntityId".to_owned()),
                            version: None,
                            field_id: None,
                            field_name: Some("value2".to_owned()),
                            specialization: None,
                        },
                        value: SchemaValue::EntityId(9),
                    },
                ),
            ])),
        };

        assert_eq!(value.to_serde_value().unwrap(), serde_json::json!([7, 9]));
    }

    fn pair_field(field_name: &str, first: SchemaValue, second: SchemaValue) -> SchemaFieldValue {
        SchemaFieldValue {
            schema: SchemaValueSchema {
                type_id: uuid::uuid!("FD14840B-21BD-5C50-9FAC-D20CE0B95474"),
                type_name: Some("AZStd::pair<s32, AZStd::string>".to_owned()),
                version: None,
                field_id: None,
                field_name: Some(field_name.to_owned()),
                specialization: None,
            },
            value: SchemaValue::Struct(BTreeMap::from([
                (
                    "value1".to_owned(),
                    SchemaFieldValue {
                        schema: SchemaValueSchema {
                            type_id: types::INT,
                            type_name: Some("s32".to_owned()),
                            version: None,
                            field_id: None,
                            field_name: Some("value1".to_owned()),
                            specialization: None,
                        },
                        value: first,
                    },
                ),
                (
                    "value2".to_owned(),
                    SchemaFieldValue {
                        schema: SchemaValueSchema {
                            type_id: types::AZSTD_STRING,
                            type_name: Some("AZStd::string".to_owned()),
                            version: None,
                            field_id: None,
                            field_name: Some("value2".to_owned()),
                            specialization: None,
                        },
                        value: second,
                    },
                ),
            ])),
        }
    }

    #[test]
    fn azstd_smart_pointer_values_emit_pointee_shape() {
        let pointee_type = uuid::uuid!("CF33E3AA-E48A-464D-B319-ECD8D0BAD687");
        let value = SchemaFieldValue {
            schema: SchemaValueSchema {
                type_id: uuid::uuid!("618711A2-DAD6-5BDC-B450-4A070D8B364F"),
                type_name: Some("AZStd::shared_ptr<InteractCondition>".to_owned()),
                version: None,
                field_id: None,
                field_name: Some("m_condition".to_owned()),
                specialization: None,
            },
            value: SchemaValue::Struct(BTreeMap::from([(
                "element".to_owned(),
                SchemaFieldValue {
                    schema: SchemaValueSchema {
                        type_id: pointee_type,
                        type_name: Some("DefaultCondition".to_owned()),
                        version: None,
                        field_id: None,
                        field_name: Some("element".to_owned()),
                        specialization: None,
                    },
                    value: SchemaValue::Struct(BTreeMap::new()),
                },
            )])),
        };

        assert_eq!(
            value.to_serde_value().unwrap(),
            serde_json::json!({ "$type": pointee_type.to_string() })
        );
    }

    #[test]
    fn empty_azstd_smart_pointer_values_emit_null() {
        let value = SchemaFieldValue {
            schema: SchemaValueSchema {
                type_id: uuid::uuid!("C7C97151-93BA-587D-AC78-EA9B5F52D7CE"),
                type_name: Some("AZStd::unique_ptr<DebugVariantData>".to_owned()),
                version: None,
                field_id: None,
                field_name: Some("m_debugVariant".to_owned()),
                specialization: None,
            },
            value: SchemaValue::Struct(BTreeMap::new()),
        };

        assert_eq!(value.to_serde_value().unwrap(), Value::Null);
    }

    #[test]
    fn serialize_context_enum_storage_decodes_for_generated_types() {
        #[derive(Debug, PartialEq, serde::Deserialize)]
        #[serde(try_from = "u8")]
        enum ExampleEnum {
            Two,
        }

        impl TryFrom<u8> for ExampleEnum {
            type Error = String;

            fn try_from(value: u8) -> Result<Self, Self::Error> {
                (value == 2)
                    .then_some(Self::Two)
                    .ok_or_else(|| format!("unexpected enum value {value}"))
            }
        }

        #[derive(Debug, PartialEq, serde::Deserialize)]
        struct Example {
            #[serde(rename = "Mode")]
            mode: ExampleEnum,
        }

        let enum_type = uuid::uuid!("CCCCCCCC-CCCC-4CCC-CCCC-CCCCCCCCCCCC");
        let mut lookup = NameLookup::new();
        lookup
            .enum_underlying_types
            .insert(enum_type, types::UNSIGNED_CHAR);
        let root = Element::new(uuid::Uuid::nil())
            .with_children([Element::new(enum_type).with_field("Mode").with_data([2])]);

        let decoded: Example = SchemaValueCodec::new(Some(&lookup))
            .deserialize(&root)
            .unwrap();
        assert_eq!(decoded.mode, ExampleEnum::Two);
    }
}
