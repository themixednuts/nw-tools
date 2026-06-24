
// Example code that deserializes and serializes the model.
// extern crate serde;
// #[macro_use]
// extern crate serde_derive;
// extern crate serde_json;
//
// use generated_module::SerializeContext;
//
// fn main() {
//     let json = r#"{"answer": 42}"#;
//     let model: SerializeContext = serde_json::from_str(&json).unwrap();
// }

use serde::{Serialize, Deserialize};
use std::collections::HashMap;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SerializeContext {
    #[serde(rename = "$id")]
    id: i64,
    edit_context: EditContext,
    uuid_map: HashMap<String, UuidMap>,
    class_name_to_uuid: Vec<Vec<ClassNameToUuid>>,
    uuid_generic_map: Vec<Vec<UuidGenericMapElement>>,
    uuid_any_creation_map: HashMap<String, String>,
    enum_type_id_to_underlying_type_id_map: HashMap<String, EnumTypeIdToUnderlyingTypeIdMap>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum ClassNameToUuid {
    Integer(i64),
    String(String),
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EditContext {
    #[serde(rename = "$id")]
    id: i64,
    class_data: Vec<Option<serde_json::Value>>,
    enum_data: Vec<Vec<EnumDatumElement>>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum EnumDatumElement {
    EnumDatumClass(EnumDatumClass),
    String(String),
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EnumDatumClass {
    #[serde(rename = "$id")]
    id: i64,
    element_id: i64,
    name: String,
    description: String,
    deprecated_name: Option<serde_json::Value>,
    attributes: Vec<Vec<EnumDatumAttribute>>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum EnumDatumAttribute {
    Integer(i64),
    PurpleAttribute(PurpleAttribute),
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PurpleAttribute {
    #[serde(rename = "$id")]
    id: i64,
    attribute_id: i64,
    attribute_name: PurpleAttributeName,
    describes_children: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    child_class_owned: Option<bool>,
    value: PurpleValue,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum PurpleAttributeName {
    #[serde(rename = "EnumValue")]
    EnumValue,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PurpleValue {
    kind: PurpleKind,
    #[serde(skip_serializing_if = "Option::is_none")]
    value_u64: Option<String>,
    value_u32: i64,
    value_i32: i64,
    value_high_u32: i64,
    #[serde(skip_serializing_if = "Option::is_none")]
    value_f32: Option<f64>,
    value_high_f32: f64,
    description: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum PurpleKind {
    #[serde(rename = "enumConstant")]
    EnumConstant,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING-KEBAB-CASE")]
pub enum EnumTypeIdToUnderlyingTypeIdMap {
    #[serde(rename = "D6597933-47CD-4FC8-B911-63F3E2B0993A")]
    D659793347Cd4Fc8B91163F3E2B0993A,
    #[serde(rename = "43DA906B-7DEF-4CA8-9790-854106D3F983")]
    The43Da906B7Def4Ca89790854106D3F983,
    #[serde(rename = "58422C0E-1E47-4854-98E6-34098F6FE12D")]
    The58422C0E1E47485498E634098F6Fe12D,
    #[serde(rename = "72039442-EB38-4D42-A1AD-CB68F7E0EEF6")]
    The72039442Eb384D42A1AdCb68F7E0Eef6,
    #[serde(rename = "72B9409A-7D1A-4831-9CFE-FCB3FADD3426")]
    The72B9409A7D1A48319CfeFcb3Fadd3426,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum UuidGenericMapElement {
    String(String),
    UuidGenericMap(Box<UuidGenericMap>),
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StickyGenericClassInfo {
    #[serde(rename = "$ref")]
    #[serde(skip_serializing_if = "Option::is_none")]
    generic_class_info_ref: Option<String>,
    #[serde(rename = "$id")]
    #[serde(skip_serializing_if = "Option::is_none")]
    id: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    type_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    registered_type_ids: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    templated_argument_count: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    templated_type_ids: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    type_id_fold_type_ids: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    specialized_type_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    generic_type_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    legacy_specialized_type_id: Option<String>,
    non_type_template_arguments: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    class_data: Option<UuidMap>,
    #[serde(skip_serializing_if = "Option::is_none")]
    elements: Option<Vec<StickyElement>>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UuidGenericMapElementClass {
    #[serde(rename = "$id")]
    id: i64,
    name: Name,
    name_crc: i64,
    type_id: String,
    data_size: String,
    offset: String,
    attribute_ownership: i64,
    flags: i64,
    #[serde(rename = "is_pointer")]
    is_pointer: bool,
    #[serde(rename = "is_base_class")]
    is_base_class: bool,
    #[serde(rename = "no_default_value")]
    no_default_value: bool,
    #[serde(rename = "is_dynamic_field")]
    is_dynamic_field: bool,
    #[serde(rename = "is_ui_element")]
    is_ui_element: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    az_rtti: Option<ElementAzRtti>,
    #[serde(skip_serializing_if = "Option::is_none")]
    generic_class_info: Option<StickyGenericClassInfo>,
    edit_data: Option<serde_json::Value>,
    attributes: Vec<Vec<IndigoAttribute>>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TentacledElement {
    #[serde(rename = "$id")]
    id: i64,
    name: Name,
    name_crc: i64,
    type_id: String,
    data_size: String,
    offset: String,
    attribute_ownership: i64,
    flags: i64,
    #[serde(rename = "is_pointer")]
    is_pointer: bool,
    #[serde(rename = "is_base_class")]
    is_base_class: bool,
    #[serde(rename = "no_default_value")]
    no_default_value: bool,
    #[serde(rename = "is_dynamic_field")]
    is_dynamic_field: bool,
    #[serde(rename = "is_ui_element")]
    is_ui_element: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    az_rtti: Option<ElementAzRtti>,
    #[serde(skip_serializing_if = "Option::is_none")]
    generic_class_info: Option<UuidGenericMap>,
    edit_data: Option<serde_json::Value>,
    attributes: Vec<Vec<IndigoAttribute>>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TentacledGenericClassInfo {
    #[serde(rename = "$ref")]
    #[serde(skip_serializing_if = "Option::is_none")]
    generic_class_info_ref: Option<String>,
    #[serde(rename = "$id")]
    #[serde(skip_serializing_if = "Option::is_none")]
    id: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    type_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    registered_type_ids: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    templated_argument_count: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    templated_type_ids: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    type_id_fold_type_ids: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    specialized_type_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    generic_type_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    legacy_specialized_type_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    non_type_template_arguments: Option<PurpleNonTypeTemplateArguments>,
    #[serde(skip_serializing_if = "Option::is_none")]
    class_data: Option<UuidMap>,
    #[serde(skip_serializing_if = "Option::is_none")]
    elements: Option<Vec<TentacledElement>>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FluffyElement {
    #[serde(rename = "$id")]
    id: i64,
    name: Name,
    name_crc: i64,
    type_id: String,
    data_size: String,
    offset: String,
    attribute_ownership: i64,
    flags: i64,
    #[serde(rename = "is_pointer")]
    is_pointer: bool,
    #[serde(rename = "is_base_class")]
    is_base_class: bool,
    #[serde(rename = "no_default_value")]
    no_default_value: bool,
    #[serde(rename = "is_dynamic_field")]
    is_dynamic_field: bool,
    #[serde(rename = "is_ui_element")]
    is_ui_element: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    az_rtti: Option<ElementAzRtti>,
    #[serde(skip_serializing_if = "Option::is_none")]
    generic_class_info: Option<TentacledGenericClassInfo>,
    edit_data: Option<serde_json::Value>,
    attributes: Vec<Vec<IndigoAttribute>>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FluffyGenericClassInfo {
    #[serde(rename = "$ref")]
    #[serde(skip_serializing_if = "Option::is_none")]
    generic_class_info_ref: Option<String>,
    #[serde(rename = "$id")]
    #[serde(skip_serializing_if = "Option::is_none")]
    id: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    type_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    registered_type_ids: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    templated_argument_count: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    templated_type_ids: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    type_id_fold_type_ids: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    specialized_type_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    generic_type_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    legacy_specialized_type_id: Option<String>,
    non_type_template_arguments: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    class_data: Option<UuidMap>,
    #[serde(skip_serializing_if = "Option::is_none")]
    elements: Option<Vec<FluffyElement>>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PurpleElement {
    #[serde(rename = "$id")]
    id: i64,
    name: Name,
    name_crc: i64,
    type_id: String,
    data_size: String,
    offset: String,
    attribute_ownership: i64,
    flags: i64,
    #[serde(rename = "is_pointer")]
    is_pointer: bool,
    #[serde(rename = "is_base_class")]
    is_base_class: bool,
    #[serde(rename = "no_default_value")]
    no_default_value: bool,
    #[serde(rename = "is_dynamic_field")]
    is_dynamic_field: bool,
    #[serde(rename = "is_ui_element")]
    is_ui_element: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    az_rtti: Option<ElementAzRtti>,
    #[serde(skip_serializing_if = "Option::is_none")]
    generic_class_info: Option<FluffyGenericClassInfo>,
    edit_data: Option<serde_json::Value>,
    attributes: Vec<Vec<IndigoAttribute>>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PurpleGenericClassInfo {
    #[serde(rename = "$id")]
    #[serde(skip_serializing_if = "Option::is_none")]
    id: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    type_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    registered_type_ids: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    templated_argument_count: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    templated_type_ids: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    type_id_fold_type_ids: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    specialized_type_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    generic_type_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    legacy_specialized_type_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    non_type_template_arguments: Option<FluffyNonTypeTemplateArguments>,
    #[serde(skip_serializing_if = "Option::is_none")]
    class_data: Option<UuidMap>,
    #[serde(skip_serializing_if = "Option::is_none")]
    elements: Option<Vec<PurpleElement>>,
    #[serde(rename = "$ref")]
    #[serde(skip_serializing_if = "Option::is_none")]
    generic_class_info_ref: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UuidMapElement {
    #[serde(rename = "$id")]
    id: i64,
    name: String,
    name_crc: i64,
    type_id: String,
    data_size: String,
    offset: String,
    attribute_ownership: i64,
    flags: i64,
    #[serde(rename = "is_pointer")]
    is_pointer: bool,
    #[serde(rename = "is_base_class")]
    is_base_class: bool,
    #[serde(rename = "no_default_value")]
    no_default_value: bool,
    #[serde(rename = "is_dynamic_field")]
    is_dynamic_field: bool,
    #[serde(rename = "is_ui_element")]
    is_ui_element: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    az_rtti: Option<ElementAzRtti>,
    #[serde(skip_serializing_if = "Option::is_none")]
    generic_class_info: Option<PurpleGenericClassInfo>,
    edit_data: Option<serde_json::Value>,
    attributes: Vec<Vec<StickyAttribute>>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UuidMap {
    #[serde(rename = "$id")]
    id: i64,
    name: String,
    type_id: String,
    version: i64,
    #[serde(skip_serializing_if = "Option::is_none")]
    converter: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    factory: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    persistent_id: Option<String>,
    do_save: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    serializer: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    event_handler: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    container: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    az_rtti: Option<UuidMapAzRtti>,
    data_converter: Option<serde_json::Value>,
    edit_data: Option<serde_json::Value>,
    elements: Vec<UuidMapElement>,
    attributes: Vec<Option<serde_json::Value>>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UuidGenericMap {
    #[serde(rename = "$ref")]
    #[serde(skip_serializing_if = "Option::is_none")]
    uuid_generic_map_ref: Option<String>,
    #[serde(rename = "$id")]
    #[serde(skip_serializing_if = "Option::is_none")]
    id: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    type_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    registered_type_ids: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    templated_argument_count: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    templated_type_ids: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    type_id_fold_type_ids: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    specialized_type_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    generic_type_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    legacy_specialized_type_id: Option<LegacySpecializedTypeId>,
    non_type_template_arguments: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    class_data: Option<UuidMap>,
    #[serde(skip_serializing_if = "Option::is_none")]
    elements: Option<Vec<UuidGenericMapElementClass>>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StickyElement {
    #[serde(rename = "$id")]
    id: i64,
    name: Name,
    name_crc: i64,
    type_id: String,
    data_size: String,
    offset: String,
    attribute_ownership: i64,
    flags: i64,
    #[serde(rename = "is_pointer")]
    is_pointer: bool,
    #[serde(rename = "is_base_class")]
    is_base_class: bool,
    #[serde(rename = "no_default_value")]
    no_default_value: bool,
    #[serde(rename = "is_dynamic_field")]
    is_dynamic_field: bool,
    #[serde(rename = "is_ui_element")]
    is_ui_element: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    az_rtti: Option<ElementAzRtti>,
    #[serde(skip_serializing_if = "Option::is_none")]
    generic_class_info: Option<IndigoGenericClassInfo>,
    edit_data: Option<serde_json::Value>,
    attributes: Vec<Option<serde_json::Value>>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ElementAzRtti {
    #[serde(rename = "$id")]
    #[serde(skip_serializing_if = "Option::is_none")]
    id: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    address: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    type_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    type_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    hierarchy: Option<Vec<Hierarchy>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    is_abstract: Option<bool>,
    #[serde(rename = "$ref")]
    #[serde(skip_serializing_if = "Option::is_none")]
    az_rtti_ref: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Hierarchy {
    type_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    type_name: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct IndigoGenericClassInfo {
    #[serde(rename = "$ref")]
    generic_class_info_ref: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Name {
    Element,
    #[serde(rename = "Value1")]
    NameValue1,
    #[serde(rename = "Value2")]
    NameValue2,
    Value,
    Value1,
    Value2,
    #[serde(rename = "Value3")]
    Value3,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum IndigoAttribute {
    Integer(i64),
    TentacledAttribute(TentacledAttribute),
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TentacledAttribute {
    #[serde(rename = "$id")]
    id: i64,
    attribute_id: i64,
    attribute_name: TentacledAttributeName,
    describes_children: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    child_class_owned: Option<bool>,
    value: TentacledValue,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum TentacledAttributeName {
    #[serde(rename = "EnumType")]
    EnumType,
    #[serde(rename = "KeyType")]
    KeyType,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TentacledValue {
    kind: FluffyKind,
    value: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FluffyKind {
    Bool,
    Function,
    U32,
    Unknown,
    #[serde(rename = "Uuid")]
    Uuid,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PurpleNonTypeTemplateArguments {
    capacity: i64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FluffyNonTypeTemplateArguments {
    #[serde(skip_serializing_if = "Option::is_none")]
    capacity: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    values: Option<Vec<ClassNameToUuid>>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum StickyAttribute {
    FluffyAttribute(FluffyAttribute),
    Integer(i64),
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FluffyAttribute {
    #[serde(rename = "$id")]
    id: i64,
    attribute_id: i64,
    attribute_name: FluffyAttributeName,
    describes_children: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    child_class_owned: Option<bool>,
    value: FluffyValue,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum FluffyAttributeName {
    #[serde(rename = "AddNotify")]
    AddNotify,
    #[serde(rename = "AutoExpand")]
    AutoExpand,
    #[serde(rename = "ChangeNotify")]
    ChangeNotify,
    #[serde(rename = "EnumType")]
    EnumType,
    #[serde(rename = "IdGeneratorFunction")]
    IdGeneratorFunction,
    #[serde(rename = "RemoveNotify")]
    RemoveNotify,
    #[serde(rename = "SliceFlags")]
    SliceFlags,
    #[serde(rename = "0x57c1db19")]
    The0X57C1Db19,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FluffyValue {
    kind: FluffyKind,
    value: Option<ValueUnion>,
    #[serde(skip_serializing_if = "Option::is_none")]
    function: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    member_function: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum ValueUnion {
    Bool(bool),
    Integer(i64),
    String(String),
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UuidMapAzRtti {
    address: String,
    type_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    type_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    hierarchy: Option<Vec<Hierarchy>>,
    is_abstract: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum LegacySpecializedTypeId {
    #[serde(rename = "00EB73A2-F67F-0000-3DD0-F4A9F67F0000")]
    The00Eb73A2F67F00003Dd0F4A9F67F0000,
    #[serde(rename = "3DD0F4A9-F67F-0000-3DD0-F4A9F67F0000")]
    The3Dd0F4A9F67F00003Dd0F4A9F67F0000,
}
