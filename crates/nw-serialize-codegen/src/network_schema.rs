use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Deserializer, Serialize, Serializer};
use serde_json::{Map, Value};
use thiserror::Error;
use uuid::Uuid;

use crate::CodegenContext;
use crate::ir::{
    SerializeCodegenIndex, SerializeCodegenItem, SerializeCodegenItemKind, SerializeCodegenUnit,
};
use crate::role::ReflectedTypeRole;
use crate::types::{ResolvedType, ScalarType};

pub const NETWORK_SCHEMA_VERSION: &str = "newworld.network_schema.v1";
pub const NETWORK_STATIC_REPORT_SCHEMA_VERSION: &str = "newworld.network_schema.static.v1";

#[derive(Debug, Error)]
pub enum NetworkSchemaImportError {
    #[error("network schema import expected a JSON object root")]
    ExpectedObjectRoot,
    #[error(
        "Ghidra network report contains private source-derived evidence; rerun NetworkSchemaExtractor without source ingestion"
    )]
    PrivateSourceEvidence,
    #[error("typeindex import expected a JSON object with a `typeIndex` array")]
    ExpectedTypeIndexArray,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NetworkSchema {
    pub schema: String,
    pub sources: Vec<NetworkSchemaSource>,
    pub summary: NetworkSchemaSummary,
    pub types: Vec<NetworkType>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub serialize_types: Vec<NetworkSerializeType>,
    pub field_registration_functions: Vec<NetworkFieldRegistrationFunction>,
    #[serde(default)]
    pub field_handler_vtables: Vec<NetworkFieldHandlerVtable>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NetworkSchemaSummary {
    pub type_count: usize,
    pub type_registry_entry_count: usize,
    pub typed_type_count: usize,
    pub named_type_count: usize,
    pub register_field_function_count: usize,
    pub register_field_count: usize,
    pub typed_register_field_function_count: usize,
    pub high_confidence_field_count: usize,
    #[serde(default)]
    pub message_unmarshal_field_count: usize,
    pub type_index_evidence_count: usize,
    #[serde(default)]
    pub serialize_source_type_count: usize,
    pub serialize_type_count: usize,
    #[serde(default)]
    pub serialize_field_type_count: usize,
    pub serialize_dependency_count: usize,
    #[serde(default)]
    pub field_handler_vtable_count: usize,
    #[serde(default)]
    pub message_source_field_count: usize,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NetworkTypeIndexMergeReport {
    pub source_type_count: usize,
    pub matched_type_count: usize,
    pub filled_type_index_count: usize,
    pub matching_type_index_count: usize,
    pub conflicting_type_index_count: usize,
    pub unmatched_schema_type_count: usize,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NetworkSerializeMergeReport {
    pub source_type_count: usize,
    pub matched_type_count: usize,
    pub type_id_matched_count: usize,
    pub name_matched_count: usize,
    pub ambiguous_name_match_count: usize,
    #[serde(default)]
    pub matched_field_type_count: usize,
    #[serde(default)]
    pub field_type_id_matched_count: usize,
    #[serde(default)]
    pub field_factory_matched_count: usize,
    #[serde(default)]
    pub field_name_matched_count: usize,
    #[serde(default)]
    pub ambiguous_field_name_match_count: usize,
    pub filled_name_count: usize,
    pub unmatched_schema_type_count: usize,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NetworkMessageSignatureMergeReport {
    pub source_message_count: usize,
    pub matched_message_count: usize,
    pub ambiguous_message_count: usize,
    pub unmatched_message_count: usize,
    pub field_count_mismatch_count: usize,
    pub field_index_mismatch_count: usize,
    pub field_name_filled_count: usize,
    pub field_name_conflict_count: usize,
    pub native_type_filled_count: usize,
    pub wire_shape_filled_count: usize,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NetworkFieldOverrideMergeReport {
    pub source_field_count: usize,
    pub matched_field_count: usize,
    pub unmatched_type_count: usize,
    pub ambiguous_type_count: usize,
    pub unmatched_field_count: usize,
    pub ambiguous_field_count: usize,
    pub native_type_updated_count: usize,
    pub rust_type_updated_count: usize,
    pub wire_shape_updated_count: usize,
    pub confidence_updated_count: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct NetworkFieldOverrideFile {
    pub fields: Vec<NetworkFieldOverride>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct NetworkFieldOverride {
    pub type_id: Option<Uuid>,
    pub type_index: Option<u32>,
    pub type_name: Option<String>,
    pub field_index: Option<u32>,
    pub field: Option<String>,
    pub native_type: Option<String>,
    pub rust_type: Option<String>,
    pub wire_shape: Option<NetworkWireShape>,
    pub wire_shape_source: Option<String>,
    pub confidence: Option<NetworkConfidence>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NetworkMessageSignature {
    pub type_id: Option<Uuid>,
    pub type_index: Option<u32>,
    pub name: Option<String>,
    pub rust_name: Option<String>,
    pub source: Option<String>,
    pub fields: Vec<NetworkMessageFieldSignature>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NetworkMessageFieldSignature {
    pub index: Option<u32>,
    pub name: String,
    pub rust_type: Option<String>,
    pub native_type: Option<String>,
    pub wire_shape: Option<NetworkWireShape>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NetworkSchemaSource {
    pub kind: NetworkSchemaSourceKind,
    pub path: Option<String>,
    pub schema: Option<String>,
    pub program: Option<String>,
    pub image_base: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum NetworkSchemaSourceKind {
    GhidraNetworkStaticReport,
    TypeRegistry,
    TypeIndex,
    SerializeContext,
    MessageSignatures,
    FieldOverrides,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NetworkType {
    pub type_id: Option<Uuid>,
    pub type_index: Option<u32>,
    pub registry_index: Option<u32>,
    pub name: Option<String>,
    pub name_source: Option<String>,
    pub capabilities: Vec<NetworkTypeCapability>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub storage_address: Option<String>,
    pub base_vtable: Option<String>,
    pub vtable: Option<String>,
    pub handler: Option<NetworkHandler>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub instance: Option<NetworkInstanceLayout>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fragment_metadata: Option<NetworkFragmentMetadata>,
    pub serialize: Option<NetworkSerializeType>,
    pub az_rtti: Option<NetworkAzRtti>,
    pub registration_type_name: Option<String>,
    pub registration_hook: Option<NetworkRegistrationHook>,
    pub fields: Vec<NetworkField>,
    pub evidence: Vec<NetworkEvidence>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NetworkSerializeType {
    pub type_id: Uuid,
    pub kind: NetworkSerializeKind,
    pub name: String,
    pub role: NetworkSerializeRole,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub factory: Option<String>,
    pub field_count: usize,
    pub variant_count: usize,
    pub direct_dependency_type_ids: Vec<Uuid>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub wire_shapes: Vec<NetworkWireScalarShape>,
    pub is_abstract: Option<bool>,
    pub is_reflection_marker: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum NetworkSerializeKind {
    Struct,
    Enum,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum NetworkSerializeRole {
    FacetedComponent,
    AzComponent,
    ClientFacet,
    ServerFacet,
    AzEntity,
    SupportType,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NetworkSerializeFieldType {
    pub type_id: Uuid,
    pub kind: NetworkSerializeKind,
    pub name: String,
    pub role: NetworkSerializeRole,
    pub field_count: usize,
    pub variant_count: usize,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub wire_shapes: Vec<NetworkWireScalarShape>,
    pub source: String,
    pub confidence: NetworkConfidence,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum NetworkTypeCapability {
    ReplicatedState,
    DirectMessage,
    RegisteredFields,
    SupportData,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NetworkField {
    pub index: Option<u32>,
    pub name: Option<String>,
    pub name_address: Option<String>,
    pub group: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub registration_kind: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub filter_group_attribute: Option<bool>,
    pub handler_offset: Option<String>,
    pub handler_expression: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub handler_vtable: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub handler_kind: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub handler_vtable_slots: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub physical_field_count: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub native_type: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source_type_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source_type_id: Option<Uuid>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rust_type: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub storage_expression: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub storage_offset: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub raw_byte_length: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub wire_shape: Option<NetworkWireShape>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub wire_shape_source: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub constructor_writes: Vec<NetworkFieldConstructorWrite>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub unmarshal_evidence: Option<NetworkFieldUnmarshalEvidence>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub nested_type_shape: Option<NetworkNestedTypeShape>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub serialize: Option<NetworkSerializeFieldType>,
    pub callsite: Option<String>,
    pub confidence: NetworkConfidence,
    pub evidence: Vec<NetworkEvidence>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NetworkFieldUnmarshalEvidence {
    pub callsite: Option<String>,
    pub target_name: Option<String>,
    pub target_kind: Option<String>,
    pub evidence_source: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NetworkNestedTypeShape {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub type_id: Option<Uuid>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub type_id_source: Option<String>,
    pub type_name: Option<String>,
    pub type_name_full: Option<String>,
    pub type_name_source: Option<String>,
    pub function: Option<String>,
    pub function_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub factory: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub az_rtti_address: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub constructor: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub vtable: Option<String>,
    pub member_base: Option<String>,
    pub member_name_source: Option<String>,
    pub member_names_proven: Option<bool>,
    pub datatype_path: Option<String>,
    pub validation: Option<String>,
    pub members: Vec<NetworkNestedTypeMember>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NetworkNestedTypeMember {
    pub index: Option<u32>,
    pub offset: Option<String>,
    pub native_offset: Option<String>,
    pub name: Option<String>,
    pub name_source: Option<String>,
    pub name_proven: Option<bool>,
    pub name_evidence: Option<String>,
    pub native_type: Option<String>,
    pub wire_shape: Option<String>,
    pub byte_width: Option<u32>,
    pub evidence_source: Option<String>,
    pub callsite: Option<String>,
    pub target: Option<String>,
    pub target_name: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NetworkFieldConstructorWrite {
    pub write: Option<String>,
    pub handler_offset: Option<String>,
    pub relative_offset: Option<String>,
    pub width_bits: Option<u32>,
    pub byte_length: Option<u32>,
    pub value_kind: Option<String>,
    pub value: Option<String>,
    pub value_hex: Option<String>,
    pub source_operand: Option<String>,
    pub source: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NetworkFieldRegistrationFunction {
    pub address: Option<String>,
    pub name: Option<String>,
    pub constructor_type_name: Option<String>,
    pub owner_type_id: Option<Uuid>,
    pub owner_type_name: Option<String>,
    pub instance_vtable: Option<String>,
    pub virtual_functions: Vec<NetworkVirtualFunction>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fragment_metadata: Option<NetworkFragmentMetadata>,
    pub az_rtti: Option<NetworkAzRtti>,
    pub fields: Vec<NetworkField>,
    pub evidence: Vec<NetworkEvidence>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NetworkFragmentMetadata {
    pub source: Option<String>,
    pub is_metadata_slot: Option<u32>,
    pub is_metadata_function: Option<String>,
    pub is_metadata: Option<bool>,
    pub category_slot: Option<u32>,
    pub category_function: Option<String>,
    pub category_value: Option<u32>,
    pub category: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NetworkVirtualFunction {
    pub slot: Option<u32>,
    pub slot_offset: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    pub address: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub target: Option<String>,
    pub function: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NetworkFieldHandlerVtable {
    pub address: Option<String>,
    pub field_count: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub handler_kind: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub vtable_slots: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub physical_field_count: Option<u32>,
    pub marshal: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub marshal_target: Option<String>,
    pub unmarshal: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub unmarshal_target: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub wire_shape: Option<NetworkWireShape>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub wire_shape_source: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub delta_wire_shape: Option<NetworkWireShape>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub full_wire_shape: Option<NetworkWireShape>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub key_native_type: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub key_native_type_source: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub delta_marshal_shapes: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub full_marshal_shapes: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub value_type_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub value_type_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub value_type_info_address: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub value_type_candidates: Vec<NetworkNativeTypeInfoEvidence>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub value_type_shape: Option<NetworkNestedTypeShape>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub embedded_value_type_shapes: Vec<NetworkNestedTypeShape>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub container_shape: Option<NetworkReplicatedContainerShape>,
    pub slots: Vec<NetworkVirtualFunction>,
    pub evidence: Vec<NetworkEvidence>,
}

impl NetworkFieldHandlerVtable {
    fn has_structured_container_value(&self) -> bool {
        self.value_type_shape.is_some()
            || replicated_container_data_shape_count(&self.full_marshal_shapes) > 2
    }

    fn should_suppress_replicated_container_wire_shape(&self) -> bool {
        self.container_shape.as_ref().is_some_and(|shape| {
            shape.storage != NetworkReplicatedContainerStorageKind::Map
                || shape.value_wire_shapes.len() != 1
        }) || self.has_structured_container_value()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NetworkNativeTypeInfoEvidence {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub address: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub type_id: Option<Uuid>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name_source: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum NetworkReplicatedContainerStorageKind {
    Map,
    Vec,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NetworkReplicatedContainerShape {
    pub storage: NetworkReplicatedContainerStorageKind,
    pub key_wire_shape: NetworkWireScalarShape,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub key_wire_shapes: Vec<NetworkWireScalarShape>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub key_native_type: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub key_native_type_source: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub key_type_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub key_type_shape: Option<NetworkNestedTypeShape>,
    pub value_wire_shapes: Vec<NetworkWireScalarShape>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub delta_value_wire_shapes: Vec<NetworkWireScalarShape>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub value_type_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub value_type_id: Option<Uuid>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub value_type_info_address: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub value_type_shape: Option<NetworkNestedTypeShape>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub embedded_value_type_shapes: Vec<NetworkNestedTypeShape>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NetworkWireScalarShape {
    Bool,
    U8,
    U16,
    U32,
    U64,
    F32,
    F64,
    HalfF32,
    VlqU32,
    VlqU64,
    SequenceNumber,
    Vec2,
    Vec3,
    Vec4,
    Quat,
    QuatCompNorm,
    Vec2Comp,
    Vec3Comp,
    Vec3CompNorm,
    QuatComp,
    QuatSmallestThree,
    NonUniformScaleComp,
    PositionAnchor,
    TransformCompressor,
    PackedSize,
    Mat3,
    Affine3,
    Aabb2d,
    Aabb3d,
    ActorRef,
    EntityRef,
    FixedBytes(u16),
    String,
}

impl Serialize for NetworkWireScalarShape {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&self.wire_string())
    }
}

impl<'de> Deserialize<'de> for NetworkWireScalarShape {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = String::deserialize(deserializer)?;
        parse_network_wire_scalar_shape(&value)
            .ok_or_else(|| serde::de::Error::custom(format!("unknown wire scalar shape `{value}`")))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NetworkReplicatedContainerWireShape {
    pub key: NetworkWireScalarShape,
    pub value: NetworkWireScalarShape,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NetworkWireShape {
    Bool,
    U8,
    U16,
    U32,
    U64,
    F32,
    F64,
    HalfF32,
    VlqU32,
    VlqU64,
    SequenceNumber,
    Vec2,
    Vec3,
    Vec4,
    Quat,
    QuatCompNorm,
    Vec2Comp,
    Vec3Comp,
    Vec3CompNorm,
    QuatComp,
    QuatSmallestThree,
    NonUniformScaleComp,
    PositionAnchor,
    TransformCompressor,
    PackedSize,
    Mat3,
    Affine3,
    Aabb2d,
    Aabb3d,
    ActorRef,
    EntityRef,
    FixedBytes(u16),
    String,
    ReplicatedContainer(NetworkReplicatedContainerWireShape),
}

impl NetworkWireScalarShape {
    fn as_static_str(self) -> Option<&'static str> {
        match self {
            Self::Bool => Some("bool"),
            Self::U8 => Some("u8"),
            Self::U16 => Some("u16"),
            Self::U32 => Some("u32"),
            Self::U64 => Some("u64"),
            Self::F32 => Some("f32"),
            Self::F64 => Some("f64"),
            Self::HalfF32 => Some("half-f32"),
            Self::VlqU32 => Some("vlq-u32"),
            Self::VlqU64 => Some("vlq-u64"),
            Self::SequenceNumber => Some("sequence-number"),
            Self::Vec2 => Some("vec2"),
            Self::Vec3 => Some("vec3"),
            Self::Vec4 => Some("vec4"),
            Self::Quat => Some("quat"),
            Self::QuatCompNorm => Some("quat-comp-norm"),
            Self::Vec2Comp => Some("vec2-comp"),
            Self::Vec3Comp => Some("vec3-comp"),
            Self::Vec3CompNorm => Some("vec3-comp-norm"),
            Self::QuatComp => Some("quat-comp"),
            Self::QuatSmallestThree => Some("quat-smallest-three"),
            Self::NonUniformScaleComp => Some("non-uniform-scale-comp"),
            Self::PositionAnchor => Some("position-anchor"),
            Self::TransformCompressor => Some("transform-compressor"),
            Self::PackedSize => Some("packed-size"),
            Self::Mat3 => Some("mat3"),
            Self::Affine3 => Some("affine3"),
            Self::Aabb2d => Some("aabb2d"),
            Self::Aabb3d => Some("aabb3d"),
            Self::ActorRef => Some("actor-ref"),
            Self::EntityRef => Some("entity-ref"),
            Self::FixedBytes(_) => None,
            Self::String => Some("string"),
        }
    }

    fn wire_string(self) -> String {
        self.as_static_str().map_or_else(
            || match self {
                Self::FixedBytes(len) => format!("fixed-bytes-{len}"),
                _ => unreachable!("non-static wire scalar handled above"),
            },
            ToOwned::to_owned,
        )
    }
}

impl From<NetworkWireScalarShape> for NetworkWireShape {
    fn from(value: NetworkWireScalarShape) -> Self {
        match value {
            NetworkWireScalarShape::Bool => Self::Bool,
            NetworkWireScalarShape::U8 => Self::U8,
            NetworkWireScalarShape::U16 => Self::U16,
            NetworkWireScalarShape::U32 => Self::U32,
            NetworkWireScalarShape::U64 => Self::U64,
            NetworkWireScalarShape::F32 => Self::F32,
            NetworkWireScalarShape::F64 => Self::F64,
            NetworkWireScalarShape::HalfF32 => Self::HalfF32,
            NetworkWireScalarShape::VlqU32 => Self::VlqU32,
            NetworkWireScalarShape::VlqU64 => Self::VlqU64,
            NetworkWireScalarShape::SequenceNumber => Self::SequenceNumber,
            NetworkWireScalarShape::Vec2 => Self::Vec2,
            NetworkWireScalarShape::Vec3 => Self::Vec3,
            NetworkWireScalarShape::Vec4 => Self::Vec4,
            NetworkWireScalarShape::Quat => Self::Quat,
            NetworkWireScalarShape::QuatCompNorm => Self::QuatCompNorm,
            NetworkWireScalarShape::Vec2Comp => Self::Vec2Comp,
            NetworkWireScalarShape::Vec3Comp => Self::Vec3Comp,
            NetworkWireScalarShape::Vec3CompNorm => Self::Vec3CompNorm,
            NetworkWireScalarShape::QuatComp => Self::QuatComp,
            NetworkWireScalarShape::QuatSmallestThree => Self::QuatSmallestThree,
            NetworkWireScalarShape::NonUniformScaleComp => Self::NonUniformScaleComp,
            NetworkWireScalarShape::PositionAnchor => Self::PositionAnchor,
            NetworkWireScalarShape::TransformCompressor => Self::TransformCompressor,
            NetworkWireScalarShape::PackedSize => Self::PackedSize,
            NetworkWireScalarShape::Mat3 => Self::Mat3,
            NetworkWireScalarShape::Affine3 => Self::Affine3,
            NetworkWireScalarShape::Aabb2d => Self::Aabb2d,
            NetworkWireScalarShape::Aabb3d => Self::Aabb3d,
            NetworkWireScalarShape::ActorRef => Self::ActorRef,
            NetworkWireScalarShape::EntityRef => Self::EntityRef,
            NetworkWireScalarShape::FixedBytes(len) => Self::FixedBytes(len),
            NetworkWireScalarShape::String => Self::String,
        }
    }
}

impl NetworkWireShape {
    #[must_use]
    pub const fn is_replicated_container(self) -> bool {
        matches!(self, Self::ReplicatedContainer(_))
    }

    fn as_static_str(self) -> Option<&'static str> {
        match self {
            Self::Bool => Some("bool"),
            Self::U8 => Some("u8"),
            Self::U16 => Some("u16"),
            Self::U32 => Some("u32"),
            Self::U64 => Some("u64"),
            Self::F32 => Some("f32"),
            Self::F64 => Some("f64"),
            Self::HalfF32 => Some("half-f32"),
            Self::VlqU32 => Some("vlq-u32"),
            Self::VlqU64 => Some("vlq-u64"),
            Self::SequenceNumber => Some("sequence-number"),
            Self::Vec2 => Some("vec2"),
            Self::Vec3 => Some("vec3"),
            Self::Vec4 => Some("vec4"),
            Self::Quat => Some("quat"),
            Self::QuatCompNorm => Some("quat-comp-norm"),
            Self::Vec2Comp => Some("vec2-comp"),
            Self::Vec3Comp => Some("vec3-comp"),
            Self::Vec3CompNorm => Some("vec3-comp-norm"),
            Self::QuatComp => Some("quat-comp"),
            Self::QuatSmallestThree => Some("quat-smallest-three"),
            Self::NonUniformScaleComp => Some("non-uniform-scale-comp"),
            Self::PositionAnchor => Some("position-anchor"),
            Self::TransformCompressor => Some("transform-compressor"),
            Self::PackedSize => Some("packed-size"),
            Self::Mat3 => Some("mat3"),
            Self::Affine3 => Some("affine3"),
            Self::Aabb2d => Some("aabb2d"),
            Self::Aabb3d => Some("aabb3d"),
            Self::ActorRef => Some("actor-ref"),
            Self::EntityRef => Some("entity-ref"),
            Self::FixedBytes(_) | Self::ReplicatedContainer(_) => None,
            Self::String => Some("string"),
        }
    }
}

impl Serialize for NetworkWireShape {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        if let Some(value) = self.as_static_str() {
            return serializer.serialize_str(value);
        }
        match self {
            Self::FixedBytes(len) => serializer.serialize_str(&format!("fixed-bytes-{len}")),
            Self::ReplicatedContainer(container) => serializer.serialize_str(&format!(
                "replicated-container<{},{}>",
                container.key.wire_string(),
                container.value.wire_string()
            )),
            _ => unreachable!("non-static wire shape handled above"),
        }
    }
}

impl<'de> Deserialize<'de> for NetworkWireShape {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = String::deserialize(deserializer)?;
        parse_network_wire_shape(&value)
            .ok_or_else(|| serde::de::Error::custom(format!("unknown wire shape `{value}`")))
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NetworkHandler {
    pub destructor: Option<String>,
    pub get_empty_value: Option<String>,
    pub create_instance: Option<String>,
    pub copy_value: Option<String>,
    pub marshal: Option<String>,
    pub unmarshal: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NetworkInstanceLayout {
    pub create_instance: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub size: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub size_source: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub constructor: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub constructor_callsite: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub constructor_name: Option<String>,
    pub evidence: Vec<NetworkEvidence>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NetworkAzRtti {
    pub source: Option<String>,
    pub address: Option<String>,
    pub type_id: Option<Uuid>,
    pub type_name: Option<String>,
    pub providers: Vec<NetworkAzRttiProvider>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NetworkAzRttiProvider {
    pub kind: Option<String>,
    pub slot: Option<u32>,
    pub slot_offset: Option<String>,
    pub function: Option<String>,
    pub provider: Option<String>,
    pub type_id: Option<Uuid>,
    pub type_id_source: Option<String>,
    pub type_name: Option<String>,
    pub source_address: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NetworkRegistrationHook {
    pub type_id: Option<Uuid>,
    pub type_name: Option<String>,
    pub slot_type_name: Option<String>,
    pub hook_function: Option<String>,
    pub helper_table: Option<String>,
    pub register_thunk: Option<String>,
    pub type_provider: Option<String>,
    pub uuid_source: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NetworkEvidence {
    pub kind: NetworkEvidenceKind,
    pub source: String,
    pub address: Option<String>,
    pub detail: Option<String>,
    pub confidence: NetworkConfidence,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum NetworkEvidenceKind {
    TypeRegistry,
    TypeIndex,
    SerializeContext,
    HandlerVtable,
    InstallRegistrationHook,
    AzRtti,
    InstanceLayout,
    RegisterField,
    FieldRegistrationFunction,
    MessageUnmarshal,
    MessageSource,
    FieldOverride,
    FragmentMetadata,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum NetworkConfidence {
    Exact,
    High,
    Inferred,
    Weak,
    Unknown,
}

impl NetworkConfidence {
    #[must_use]
    pub const fn is_high_or_exact(self) -> bool {
        matches!(self, Self::Exact | Self::High)
    }
}

impl NetworkSchema {
    pub fn from_ghidra_static_network_report(
        report: &Value,
    ) -> Result<Self, NetworkSchemaImportError> {
        Self::from_ghidra_static_network_report_with_context(report, &CodegenContext::inline())
    }

    pub fn from_ghidra_static_network_report_with_context(
        report: &Value,
        context: &CodegenContext,
    ) -> Result<Self, NetworkSchemaImportError> {
        let root = report
            .as_object()
            .ok_or(NetworkSchemaImportError::ExpectedObjectRoot)?;
        if contains_private_source_evidence(report) {
            return Err(NetworkSchemaImportError::PrivateSourceEvidence);
        }
        let registry_entries = array_values(root, "registryEntries")
            .filter_map(Value::as_object)
            .collect::<Vec<_>>();
        let field_registration_function_entries = array_values(root, "fieldRegistrationFunctions")
            .filter_map(Value::as_object)
            .collect::<Vec<_>>();
        let field_handler_vtable_entries = array_values(root, "fieldHandlerVtables")
            .filter_map(Value::as_object)
            .collect::<Vec<_>>();
        let ((types, field_registration_functions), field_handler_vtables) = context.runner().join(
            || {
                context.runner().join(
                    || {
                        context.runner().map(&registry_entries, |entry| {
                            network_type_from_registry_entry(entry)
                        })
                    },
                    || {
                        context
                            .runner()
                            .map(&field_registration_function_entries, |entry| {
                                network_field_registration_function(entry)
                            })
                    },
                )
            },
            || {
                context
                    .runner()
                    .map(&field_handler_vtable_entries, |entry| {
                        network_field_handler_vtable(entry)
                    })
            },
        );
        let mut schema = Self {
            schema: NETWORK_SCHEMA_VERSION.to_owned(),
            sources: vec![NetworkSchemaSource {
                kind: NetworkSchemaSourceKind::GhidraNetworkStaticReport,
                path: string(root, "input"),
                schema: Some(NETWORK_STATIC_REPORT_SCHEMA_VERSION.to_owned()),
                program: string(root, "program"),
                image_base: string(root, "imageBase"),
            }],
            summary: NetworkSchemaSummary::default(),
            types,
            serialize_types: Vec::new(),
            field_registration_functions,
            field_handler_vtables,
        };
        schema.normalize_derived_shapes();
        Ok(schema)
    }

    pub fn normalize_derived_shapes(&mut self) {
        for vtable in &mut self.field_handler_vtables {
            if vtable.container_shape.is_none() {
                vtable.container_shape = replicated_container_shape_from_vtable(vtable);
            }
            if vtable.should_suppress_replicated_container_wire_shape() {
                vtable.wire_shape = None;
                vtable.wire_shape_source = None;
                vtable.delta_wire_shape = None;
                vtable.full_wire_shape = None;
            }
        }
        self.suppress_under_shaped_container_wire_shapes();
        self.summary = self.build_summary();
    }

    fn suppress_under_shaped_container_wire_shapes(&mut self) {
        let structured_container_vtables = self
            .field_handler_vtables
            .iter()
            .filter(|vtable| vtable.should_suppress_replicated_container_wire_shape())
            .filter_map(|vtable| vtable.address.as_deref())
            .collect::<BTreeSet<_>>();
        if structured_container_vtables.is_empty() {
            return;
        }

        for network_type in &mut self.types {
            suppress_field_wire_shapes_for_vtables(
                &mut network_type.fields,
                &structured_container_vtables,
            );
        }
        for function in &mut self.field_registration_functions {
            suppress_field_wire_shapes_for_vtables(
                &mut function.fields,
                &structured_container_vtables,
            );
        }
    }

    pub fn merge_typeindex_root(
        &mut self,
        typeindex: &Value,
        source_path: Option<String>,
    ) -> Result<NetworkTypeIndexMergeReport, NetworkSchemaImportError> {
        let root = typeindex
            .as_object()
            .ok_or(NetworkSchemaImportError::ExpectedTypeIndexArray)?;
        let type_ids = root
            .get("typeIndex")
            .and_then(Value::as_array)
            .ok_or(NetworkSchemaImportError::ExpectedTypeIndexArray)?;
        let type_indices = type_ids
            .iter()
            .enumerate()
            .filter_map(|(index, value)| {
                value
                    .as_str()
                    .and_then(parse_uuid)
                    .zip(u32::try_from(index).ok())
            })
            .collect::<BTreeMap<_, _>>();

        let mut report = NetworkTypeIndexMergeReport {
            source_type_count: type_indices.len(),
            ..NetworkTypeIndexMergeReport::default()
        };
        for network_type in &mut self.types {
            let Some(type_id) = network_type.type_id else {
                report.unmatched_schema_type_count += 1;
                continue;
            };
            let Some(type_index) = type_indices.get(&type_id).copied() else {
                report.unmatched_schema_type_count += 1;
                continue;
            };

            report.matched_type_count += 1;
            match network_type.type_index {
                Some(existing) if existing == type_index => {
                    report.matching_type_index_count += 1;
                    network_type.evidence.push(typeindex_evidence(
                        type_index,
                        NetworkConfidence::Exact,
                        None,
                    ));
                }
                Some(existing) => {
                    report.conflicting_type_index_count += 1;
                    network_type.evidence.push(typeindex_evidence(
                        type_index,
                        NetworkConfidence::Weak,
                        Some(format!("typeindex.json={type_index}, existing={existing}")),
                    ));
                }
                None => {
                    report.filled_type_index_count += 1;
                    network_type.type_index = Some(type_index);
                    network_type.evidence.push(typeindex_evidence(
                        type_index,
                        NetworkConfidence::Exact,
                        None,
                    ));
                }
            }
        }
        self.sources.push(NetworkSchemaSource {
            kind: NetworkSchemaSourceKind::TypeIndex,
            path: source_path,
            schema: None,
            program: None,
            image_base: None,
        });
        self.summary = self.build_summary();
        Ok(report)
    }

    pub fn merge_serialize_codegen_unit(
        &mut self,
        unit: &SerializeCodegenUnit,
        source_path: Option<String>,
    ) -> NetworkSerializeMergeReport {
        let index = unit.index();
        let name_index = serialize_items_by_name(unit);
        let factory_index = serialize_items_by_factory(unit);
        let selected_value_types =
            selected_value_type_info_by_handler_vtable(&self.field_handler_vtables);
        let mut report = NetworkSerializeMergeReport {
            source_type_count: unit.items.len(),
            ..NetworkSerializeMergeReport::default()
        };
        self.merge_serialize_type_index(unit, &index);

        for network_type in &mut self.types {
            let Some((item, confidence, source)) =
                serialize_match(network_type, &index, &name_index, &mut report)
            else {
                merge_field_serialize_types(
                    network_type,
                    &index,
                    &factory_index,
                    &selected_value_types,
                    &mut report,
                );
                continue;
            };
            report.matched_type_count += 1;
            if network_type.name.is_none() {
                network_type.name = Some(item.source_name.clone());
                network_type.name_source = Some(source.clone());
                report.filled_name_count += 1;
            }
            network_type.serialize = Some(network_serialize_type(item, &index));
            network_type.evidence.push(NetworkEvidence {
                kind: NetworkEvidenceKind::SerializeContext,
                source,
                address: None,
                detail: Some(item.source_name.clone()),
                confidence,
            });
            merge_field_serialize_types(
                network_type,
                &index,
                &factory_index,
                &selected_value_types,
                &mut report,
            );
        }

        self.sources.push(NetworkSchemaSource {
            kind: NetworkSchemaSourceKind::SerializeContext,
            path: source_path,
            schema: None,
            program: None,
            image_base: None,
        });
        self.summary = self.build_summary();
        report
    }

    fn merge_serialize_type_index(
        &mut self,
        unit: &SerializeCodegenUnit,
        index: &SerializeCodegenIndex<'_>,
    ) {
        let mut merged = self
            .serialize_types
            .iter()
            .cloned()
            .map(|serialize| (serialize.type_id, serialize))
            .collect::<BTreeMap<_, _>>();
        for item in &unit.items {
            merged
                .entry(item.source_type_id)
                .or_insert_with(|| network_serialize_type(item, index));
        }
        self.serialize_types = merged.into_values().collect();
    }

    pub fn merge_message_signatures(
        &mut self,
        signatures: &[NetworkMessageSignature],
        source_path: Option<String>,
    ) -> NetworkMessageSignatureMergeReport {
        let mut report = NetworkMessageSignatureMergeReport {
            source_message_count: signatures.len(),
            ..NetworkMessageSignatureMergeReport::default()
        };

        for signature in signatures {
            let candidates = message_signature_candidates(&self.types, signature);
            let [network_type_index] = candidates.as_slice() else {
                if candidates.is_empty() {
                    report.unmatched_message_count += 1;
                } else {
                    report.ambiguous_message_count += 1;
                }
                continue;
            };

            let network_type = &mut self.types[*network_type_index];
            let source = signature
                .source
                .clone()
                .or_else(|| source_path.clone())
                .unwrap_or_else(|| "messageSignatures".to_owned());
            if !signature.fields.is_empty() && network_type.fields.len() < signature.fields.len() {
                if !network_type.fields.is_empty() {
                    report.field_count_mismatch_count += 1;
                }
                network_type.fields =
                    network_fields_from_message_signature(&signature.fields, source.clone());
                report.matched_message_count += 1;
                report.field_name_filled_count += signature.fields.len();
                report.native_type_filled_count += signature
                    .fields
                    .iter()
                    .filter(|field| field.native_type.is_some())
                    .count();
                report.wire_shape_filled_count += signature
                    .fields
                    .iter()
                    .filter(|field| field.wire_shape.is_some())
                    .count();
                continue;
            }

            if network_type.fields.len() != signature.fields.len() {
                report.field_count_mismatch_count += 1;
                continue;
            }

            report.matched_message_count += 1;
            for (field, field_signature) in
                network_type.fields.iter_mut().zip(signature.fields.iter())
            {
                if let (Some(existing), Some(expected)) = (field.index, field_signature.index)
                    && existing != expected
                {
                    report.field_index_mismatch_count += 1;
                    continue;
                }

                if field.name.as_deref().is_none_or(is_placeholder_field_name)
                    || field_has_native_type_name(field)
                {
                    field.name = Some(field_signature.name.clone());
                    report.field_name_filled_count += 1;
                } else if field.name.as_deref() != Some(field_signature.name.as_str()) {
                    report.field_name_conflict_count += 1;
                }

                if field.native_type.is_none()
                    || should_replace_native_type_from_message_signature(
                        field.native_type.as_deref(),
                        field_signature.native_type.as_deref(),
                    )
                {
                    field.native_type = field_signature.native_type.clone();
                    if field.native_type.is_some() {
                        report.native_type_filled_count += 1;
                    }
                }

                if field.rust_type.is_none() {
                    field.rust_type = field_signature.rust_type.clone();
                }

                if field.wire_shape.is_none()
                    && let Some(wire_shape) = field_signature.wire_shape
                {
                    field.wire_shape = Some(wire_shape);
                    field.wire_shape_source = Some(source.clone());
                    report.wire_shape_filled_count += 1;
                }

                field.evidence.push(NetworkEvidence {
                    kind: NetworkEvidenceKind::MessageSource,
                    source: source.clone(),
                    address: None,
                    detail: Some(field_signature.name.clone()),
                    confidence: NetworkConfidence::High,
                });
            }
        }

        self.sources.push(NetworkSchemaSource {
            kind: NetworkSchemaSourceKind::MessageSignatures,
            path: source_path,
            schema: None,
            program: None,
            image_base: None,
        });
        self.summary = self.build_summary();
        report
    }

    pub fn merge_field_overrides(
        &mut self,
        overrides: &NetworkFieldOverrideFile,
        source_path: Option<String>,
    ) -> NetworkFieldOverrideMergeReport {
        let mut report = NetworkFieldOverrideMergeReport {
            source_field_count: overrides.fields.len(),
            ..NetworkFieldOverrideMergeReport::default()
        };

        for field_override in &overrides.fields {
            let type_candidates = field_override_type_candidates(&self.types, field_override);
            let [network_type_index] = type_candidates.as_slice() else {
                if type_candidates.is_empty() {
                    report.unmatched_type_count += 1;
                } else {
                    report.ambiguous_type_count += 1;
                }
                continue;
            };

            let network_type = &mut self.types[*network_type_index];
            let field_candidates = field_override_field_candidates(network_type, field_override);
            let [field_index] = field_candidates.as_slice() else {
                if field_candidates.is_empty() {
                    report.unmatched_field_count += 1;
                } else {
                    report.ambiguous_field_count += 1;
                }
                continue;
            };

            let source = source_path
                .clone()
                .unwrap_or_else(|| "fieldOverrides".to_owned());
            let field = &mut network_type.fields[*field_index];
            if let Some(native_type) = field_override.native_type.as_ref()
                && field.native_type.as_deref() != Some(native_type.as_str())
            {
                field.native_type = Some(native_type.clone());
                report.native_type_updated_count += 1;
            }
            if let Some(rust_type) = field_override.rust_type.as_ref()
                && field.rust_type.as_deref() != Some(rust_type.as_str())
            {
                field.rust_type = Some(rust_type.clone());
                report.rust_type_updated_count += 1;
            }
            if let Some(wire_shape) = field_override.wire_shape
                && field.wire_shape != Some(wire_shape)
            {
                field.wire_shape = Some(wire_shape);
                field.wire_shape_source = Some(
                    field_override
                        .wire_shape_source
                        .clone()
                        .unwrap_or_else(|| source.clone()),
                );
                report.wire_shape_updated_count += 1;
            } else if let Some(wire_shape_source) = field_override.wire_shape_source.as_ref()
                && field.wire_shape.is_some()
                && field.wire_shape_source.as_deref() != Some(wire_shape_source.as_str())
            {
                field.wire_shape_source = Some(wire_shape_source.clone());
                report.wire_shape_updated_count += 1;
            }
            if let Some(confidence) = field_override.confidence
                && field.confidence != confidence
            {
                field.confidence = confidence;
                report.confidence_updated_count += 1;
            }
            field.evidence.push(NetworkEvidence {
                kind: NetworkEvidenceKind::FieldOverride,
                source: source.clone(),
                address: None,
                detail: Some(field_override_detail(field_override)),
                confidence: field_override.confidence.unwrap_or(NetworkConfidence::High),
            });
            report.matched_field_count += 1;
        }

        self.sources.push(NetworkSchemaSource {
            kind: NetworkSchemaSourceKind::FieldOverrides,
            path: source_path,
            schema: None,
            program: None,
            image_base: None,
        });
        self.summary = self.build_summary();
        report
    }

    #[must_use]
    pub fn build_summary(&self) -> NetworkSchemaSummary {
        let register_field_count = self
            .field_registration_functions
            .iter()
            .map(|function| function.fields.len())
            .sum::<usize>();
        NetworkSchemaSummary {
            type_count: self.types.len(),
            type_registry_entry_count: self.types.len(),
            typed_type_count: self
                .types
                .iter()
                .filter(|network_type| network_type.type_id.is_some())
                .count(),
            named_type_count: self
                .types
                .iter()
                .filter(|network_type| network_type.name.is_some())
                .count(),
            register_field_function_count: self.field_registration_functions.len(),
            register_field_count,
            typed_register_field_function_count: self
                .field_registration_functions
                .iter()
                .filter(|function| function.owner_type_id.is_some())
                .count(),
            high_confidence_field_count: self
                .types
                .iter()
                .flat_map(|network_type| &network_type.fields)
                .filter(|field| field.confidence.is_high_or_exact())
                .count(),
            message_unmarshal_field_count: self
                .types
                .iter()
                .flat_map(|network_type| &network_type.fields)
                .filter(|field| {
                    field
                        .evidence
                        .iter()
                        .any(|evidence| evidence.kind == NetworkEvidenceKind::MessageUnmarshal)
                })
                .count(),
            type_index_evidence_count: self
                .types
                .iter()
                .flat_map(|network_type| &network_type.evidence)
                .filter(|evidence| evidence.kind == NetworkEvidenceKind::TypeIndex)
                .count(),
            serialize_source_type_count: self.serialize_types.len(),
            serialize_type_count: self
                .types
                .iter()
                .filter(|network_type| network_type.serialize.is_some())
                .count(),
            serialize_field_type_count: self
                .types
                .iter()
                .flat_map(|network_type| &network_type.fields)
                .filter(|field| field.serialize.is_some())
                .count(),
            serialize_dependency_count: self
                .types
                .iter()
                .filter_map(|network_type| network_type.serialize.as_ref())
                .map(|serialize| serialize.direct_dependency_type_ids.len())
                .sum(),
            field_handler_vtable_count: self.field_handler_vtables.len(),
            message_source_field_count: self
                .types
                .iter()
                .flat_map(|network_type| &network_type.fields)
                .filter(|field| {
                    field
                        .evidence
                        .iter()
                        .any(|evidence| evidence.kind == NetworkEvidenceKind::MessageSource)
                })
                .count(),
        }
    }
}

fn network_fields_from_message_signature(
    fields: &[NetworkMessageFieldSignature],
    source: String,
) -> Vec<NetworkField> {
    fields
        .iter()
        .enumerate()
        .map(|(index, field)| network_field_from_message_signature(index, field, source.clone()))
        .collect()
}

fn network_field_from_message_signature(
    fallback_index: usize,
    signature: &NetworkMessageFieldSignature,
    source: String,
) -> NetworkField {
    let index = signature
        .index
        .or_else(|| u32::try_from(fallback_index).ok());
    let evidence = vec![NetworkEvidence {
        kind: NetworkEvidenceKind::MessageSource,
        source: source.clone(),
        address: None,
        detail: Some(signature.name.clone()),
        confidence: NetworkConfidence::High,
    }];
    NetworkField {
        index,
        name: Some(signature.name.clone()),
        name_address: None,
        group: None,
        registration_kind: None,
        filter_group_attribute: None,
        handler_offset: None,
        handler_expression: None,
        handler_vtable: None,
        handler_kind: None,
        handler_vtable_slots: None,
        physical_field_count: None,
        native_type: signature.native_type.clone(),
        source_type_name: None,
        source_type_id: None,
        rust_type: signature.rust_type.clone(),
        storage_expression: None,
        storage_offset: None,
        raw_byte_length: None,
        wire_shape: signature.wire_shape,
        wire_shape_source: signature.wire_shape.map(|_| source),
        constructor_writes: Vec::new(),
        unmarshal_evidence: None,
        nested_type_shape: None,
        serialize: None,
        callsite: None,
        confidence: NetworkConfidence::High,
        evidence,
    }
}

fn field_override_type_candidates(
    types: &[NetworkType],
    field_override: &NetworkFieldOverride,
) -> Vec<usize> {
    if field_override.type_id.is_none()
        && field_override.type_index.is_none()
        && field_override.type_name.is_none()
    {
        return Vec::new();
    }

    types
        .iter()
        .enumerate()
        .filter_map(|(index, network_type)| {
            field_override_matches_type(network_type, field_override).then_some(index)
        })
        .collect()
}

fn field_override_matches_type(
    network_type: &NetworkType,
    field_override: &NetworkFieldOverride,
) -> bool {
    field_override
        .type_id
        .is_none_or(|type_id| network_type.type_id == Some(type_id))
        && field_override
            .type_index
            .is_none_or(|type_index| network_type.type_index == Some(type_index))
        && field_override.type_name.as_deref().is_none_or(|type_name| {
            network_type.name.as_deref() == Some(type_name)
                || network_type.registration_type_name.as_deref() == Some(type_name)
        })
}

fn field_override_field_candidates(
    network_type: &NetworkType,
    field_override: &NetworkFieldOverride,
) -> Vec<usize> {
    if field_override.field_index.is_none() && field_override.field.is_none() {
        return Vec::new();
    }

    network_type
        .fields
        .iter()
        .enumerate()
        .filter_map(|(index, field)| {
            field_override_matches_field(field, field_override).then_some(index)
        })
        .collect()
}

fn field_override_matches_field(
    field: &NetworkField,
    field_override: &NetworkFieldOverride,
) -> bool {
    field_override
        .field_index
        .is_none_or(|field_index| field.index == Some(field_index))
        && field_override
            .field
            .as_deref()
            .is_none_or(|field_name| field.name.as_deref() == Some(field_name))
}

fn field_override_detail(field_override: &NetworkFieldOverride) -> String {
    let type_part = field_override
        .type_name
        .as_deref()
        .map(ToOwned::to_owned)
        .or_else(|| {
            field_override
                .type_index
                .map(|type_index| type_index.to_string())
        })
        .or_else(|| field_override.type_id.map(|type_id| type_id.to_string()))
        .unwrap_or_else(|| "<unknown-type>".to_owned());
    let field_part = field_override
        .field
        .clone()
        .or_else(|| {
            field_override
                .field_index
                .map(|field_index| field_index.to_string())
        })
        .unwrap_or_else(|| "<unknown-field>".to_owned());
    format!("{type_part}.{field_part}")
}

fn serialize_items_by_name(
    unit: &SerializeCodegenUnit,
) -> BTreeMap<&str, Vec<&SerializeCodegenItem>> {
    let mut index = BTreeMap::<&str, Vec<&SerializeCodegenItem>>::new();
    for item in &unit.items {
        index.entry(&item.source_name).or_default().push(item);
    }
    index
}

fn serialize_items_by_factory(
    unit: &SerializeCodegenUnit,
) -> BTreeMap<String, Vec<&SerializeCodegenItem>> {
    let mut index = BTreeMap::<String, Vec<&SerializeCodegenItem>>::new();
    for item in &unit.items {
        let Some(factory) = item.factory.as_deref() else {
            continue;
        };
        index
            .entry(factory.to_ascii_lowercase())
            .or_default()
            .push(item);
    }
    index
}

fn serialize_match<'a>(
    network_type: &NetworkType,
    type_index: &'a SerializeCodegenIndex<'a>,
    name_index: &'a BTreeMap<&str, Vec<&'a SerializeCodegenItem>>,
    report: &mut NetworkSerializeMergeReport,
) -> Option<(&'a SerializeCodegenItem, NetworkConfidence, String)> {
    if let Some(type_id) = network_type.type_id
        && !type_id.is_nil()
        && let Some(item) = type_index.item_by_type_id(type_id)
    {
        report.type_id_matched_count += 1;
        return Some((item, NetworkConfidence::High, "serializeContext".to_owned()));
    }

    let Some(name) = network_type.name.as_deref() else {
        report.unmatched_schema_type_count += 1;
        return None;
    };
    let Some(candidates) = name_index.get(name) else {
        report.unmatched_schema_type_count += 1;
        return None;
    };
    let [item] = candidates.as_slice() else {
        report.ambiguous_name_match_count += 1;
        report.unmatched_schema_type_count += 1;
        return None;
    };
    report.name_matched_count += 1;
    Some((
        item,
        NetworkConfidence::Inferred,
        "serializeContext:name".to_owned(),
    ))
}

fn merge_field_serialize_types(
    network_type: &mut NetworkType,
    type_index: &SerializeCodegenIndex<'_>,
    factory_index: &BTreeMap<String, Vec<&SerializeCodegenItem>>,
    selected_value_types: &BTreeMap<String, NetworkNativeTypeInfoEvidence>,
    report: &mut NetworkSerializeMergeReport,
) {
    for field in &mut network_type.fields {
        if field.serialize.is_some() {
            continue;
        }
        let Some((item, confidence, source, address)) = serialize_field_match(
            field,
            type_index,
            factory_index,
            selected_value_types,
            report,
        ) else {
            continue;
        };
        report.matched_field_type_count += 1;
        field.serialize = Some(network_serialize_field_type(
            item,
            type_index,
            source.clone(),
            confidence,
        ));
        field.evidence.push(NetworkEvidence {
            kind: NetworkEvidenceKind::SerializeContext,
            source,
            address,
            detail: Some(item.source_name.clone()),
            confidence,
        });
    }
}

fn serialize_field_match<'a>(
    field: &NetworkField,
    type_index: &'a SerializeCodegenIndex<'a>,
    factory_index: &'a BTreeMap<String, Vec<&'a SerializeCodegenItem>>,
    selected_value_types: &BTreeMap<String, NetworkNativeTypeInfoEvidence>,
    report: &mut NetworkSerializeMergeReport,
) -> Option<(
    &'a SerializeCodegenItem,
    NetworkConfidence,
    String,
    Option<String>,
)> {
    if let Some(type_id) = field.source_type_id.or_else(|| {
        field
            .nested_type_shape
            .as_ref()
            .and_then(|shape| shape.type_id)
    }) && !type_id.is_nil()
        && let Some(item) = type_index.item_by_type_id(type_id)
    {
        report.field_type_id_matched_count += 1;
        return Some((
            item,
            NetworkConfidence::Exact,
            "serializeContext:field-type-id".to_owned(),
            None,
        ));
    }

    if let Some(value_type) = field
        .handler_vtable
        .as_deref()
        .and_then(|handler_vtable| selected_value_types.get(handler_vtable))
        && let Some(type_id) = value_type.type_id
        && !type_id.is_nil()
        && let Some(item) = type_index.item_by_type_id(type_id)
    {
        report.field_type_id_matched_count += 1;
        return Some((
            item,
            NetworkConfidence::High,
            "serializeContext:handler-value-type-id".to_owned(),
            value_type.address.clone(),
        ));
    }

    if let Some(factory) = field
        .nested_type_shape
        .as_ref()
        .and_then(|shape| shape.factory.as_deref())
    {
        let key = factory.to_ascii_lowercase();
        if let Some(candidates) = factory_index.get(&key)
            && let [item] = candidates.as_slice()
        {
            report.field_factory_matched_count += 1;
            return Some((
                item,
                NetworkConfidence::High,
                "serializeContext:field-factory".to_owned(),
                Some(factory.to_owned()),
            ));
        }
    }

    None
}

fn selected_value_type_info_by_handler_vtable(
    vtables: &[NetworkFieldHandlerVtable],
) -> BTreeMap<String, NetworkNativeTypeInfoEvidence> {
    vtables
        .iter()
        .filter_map(|vtable| {
            let address = vtable.address.clone()?;
            let type_id = vtable
                .value_type_id
                .as_deref()
                .and_then(|type_id| Uuid::parse_str(type_id.trim_matches(['{', '}'])).ok())?;
            Some((
                address,
                NetworkNativeTypeInfoEvidence {
                    address: vtable.value_type_info_address.clone(),
                    name: vtable.value_type_name.clone(),
                    type_id: Some(type_id),
                    source: Some("selected-value-type-info".to_owned()),
                    name_source: Some("selected-value-type-info".to_owned()),
                },
            ))
        })
        .collect()
}

fn message_signature_candidates(
    types: &[NetworkType],
    signature: &NetworkMessageSignature,
) -> Vec<usize> {
    if let Some(type_id) = signature.type_id {
        let matches = types
            .iter()
            .enumerate()
            .filter_map(|(index, network_type)| {
                (network_type.type_id == Some(type_id)).then_some(index)
            })
            .collect::<Vec<_>>();
        if !matches.is_empty() {
            return matches;
        }
    }

    if let Some(type_index) = signature.type_index {
        let matches = types
            .iter()
            .enumerate()
            .filter_map(|(index, network_type)| {
                (network_type.type_index == Some(type_index)).then_some(index)
            })
            .collect::<Vec<_>>();
        if !matches.is_empty() {
            return matches;
        }
    }

    if let Some(name) = signature.name.as_deref() {
        let matches = types
            .iter()
            .enumerate()
            .filter_map(|(index, network_type)| {
                network_type
                    .name
                    .as_deref()
                    .is_some_and(|network_name| {
                        network_name == name || type_leaf_name(network_name) == name
                    })
                    .then_some(index)
            })
            .collect::<Vec<_>>();
        if !matches.is_empty() {
            return matches;
        }
    }

    if let Some(rust_name) = signature.rust_name.as_deref() {
        return types
            .iter()
            .enumerate()
            .filter_map(|(index, network_type)| {
                network_type
                    .name
                    .as_deref()
                    .is_some_and(|network_name| type_leaf_name(network_name) == rust_name)
                    .then_some(index)
            })
            .collect();
    }

    Vec::new()
}

fn type_leaf_name(name: &str) -> &str {
    name.rsplit("::").next().unwrap_or(name)
}

fn is_placeholder_field_name(value: &str) -> bool {
    value
        .strip_prefix("field_")
        .is_some_and(|suffix| !suffix.is_empty() && suffix.chars().all(|c| c.is_ascii_digit()))
}

fn field_has_native_type_name(field: &NetworkField) -> bool {
    field.evidence.iter().any(|evidence| {
        evidence.kind == NetworkEvidenceKind::MessageSource
            && evidence.source == "message-native-type-name"
    })
}

fn should_replace_native_type_from_message_signature(
    existing: Option<&str>,
    signature: Option<&str>,
) -> bool {
    let (Some(existing), Some(signature)) = (existing, signature) else {
        return false;
    };
    if existing == signature {
        return false;
    }
    matches!(
        (existing.trim(), signature.trim()),
        ("u32" | "uint32_t" | "AZ::u32", "FragmentKey")
            | ("ProxyAddress" | "HubAddress", "ActorRef")
    )
}

fn network_serialize_type(
    item: &SerializeCodegenItem,
    index: &SerializeCodegenIndex<'_>,
) -> NetworkSerializeType {
    let mut direct_dependency_type_ids = item
        .direct_dependency_type_ids()
        .into_iter()
        .collect::<Vec<_>>();
    direct_dependency_type_ids.sort_unstable();
    NetworkSerializeType {
        type_id: item.source_type_id,
        kind: network_serialize_kind(item.kind),
        name: item.source_name.clone(),
        role: network_serialize_role(item.role),
        factory: item.factory.clone(),
        field_count: item.fields.len(),
        variant_count: item.variants.len(),
        direct_dependency_type_ids,
        wire_shapes: serialize_item_wire_shapes(item, index).unwrap_or_default(),
        is_abstract: item.is_abstract,
        is_reflection_marker: item.is_reflection_marker,
    }
}

fn network_serialize_field_type(
    item: &SerializeCodegenItem,
    index: &SerializeCodegenIndex<'_>,
    source: String,
    confidence: NetworkConfidence,
) -> NetworkSerializeFieldType {
    NetworkSerializeFieldType {
        type_id: item.source_type_id,
        kind: network_serialize_kind(item.kind),
        name: item.source_name.clone(),
        role: network_serialize_role(item.role),
        field_count: item.fields.len(),
        variant_count: item.variants.len(),
        wire_shapes: serialize_item_wire_shapes(item, index).unwrap_or_default(),
        source,
        confidence,
    }
}

fn serialize_item_wire_shapes(
    item: &SerializeCodegenItem,
    index: &SerializeCodegenIndex<'_>,
) -> Option<Vec<NetworkWireScalarShape>> {
    serialize_item_wire_shapes_with_seen(item, index, &mut BTreeSet::new())
}

fn serialize_item_wire_shapes_with_seen(
    item: &SerializeCodegenItem,
    index: &SerializeCodegenIndex<'_>,
    seen: &mut BTreeSet<Uuid>,
) -> Option<Vec<NetworkWireScalarShape>> {
    if !seen.insert(item.source_type_id) {
        return None;
    }
    match item.kind {
        SerializeCodegenItemKind::Struct => {
            let mut shapes = Vec::new();
            for field in item.fields.iter().filter(|field| !field.is_base_class) {
                shapes.extend(resolved_type_wire_shapes(
                    &field.resolved_type,
                    index,
                    seen,
                )?);
            }
            seen.remove(&item.source_type_id);
            Some(shapes)
        }
        SerializeCodegenItemKind::Enum => {
            let shapes = item
                .enum_underlying_type
                .as_ref()
                .and_then(|underlying| resolved_type_wire_shapes(underlying, index, seen));
            seen.remove(&item.source_type_id);
            shapes
        }
    }
}

fn resolved_type_wire_shapes(
    resolved: &ResolvedType,
    index: &SerializeCodegenIndex<'_>,
    seen: &mut BTreeSet<Uuid>,
) -> Option<Vec<NetworkWireScalarShape>> {
    match resolved {
        ResolvedType::Scalar(scalar) => scalar_wire_shape(*scalar).map(|shape| vec![shape]),
        ResolvedType::Named { type_id, .. } => {
            let item = index.item_by_type_id(*type_id)?;
            serialize_item_wire_shapes_with_seen(item, index, seen)
        }
        ResolvedType::RangedInteger { value, .. } => resolved_type_wire_shapes(value, index, seen),
        ResolvedType::Tuple { elements } => {
            let mut shapes = Vec::new();
            for element in elements {
                shapes.extend(resolved_type_wire_shapes(element, index, seen)?);
            }
            Some(shapes)
        }
        ResolvedType::Sequence { .. }
        | ResolvedType::Map { .. }
        | ResolvedType::Asset { .. }
        | ResolvedType::Uid { .. }
        | ResolvedType::ReplicatedField { .. }
        | ResolvedType::ByteStream
        | ResolvedType::Pair { .. }
        | ResolvedType::Pointer { .. }
        | ResolvedType::Optional { .. }
        | ResolvedType::Unknown { .. } => None,
    }
}

const fn scalar_wire_shape(scalar: ScalarType) -> Option<NetworkWireScalarShape> {
    match scalar {
        ScalarType::Char | ScalarType::SignedChar | ScalarType::I8 | ScalarType::U8 => {
            Some(NetworkWireScalarShape::U8)
        }
        ScalarType::I16 | ScalarType::U16 => Some(NetworkWireScalarShape::U16),
        ScalarType::I32 | ScalarType::U32 | ScalarType::Crc32 => Some(NetworkWireScalarShape::U32),
        ScalarType::I64 | ScalarType::U64 | ScalarType::UnsignedLong | ScalarType::EntityId => {
            Some(NetworkWireScalarShape::U64)
        }
        ScalarType::F32 => Some(NetworkWireScalarShape::F32),
        ScalarType::F64 => Some(NetworkWireScalarShape::F64),
        ScalarType::Bool => Some(NetworkWireScalarShape::Bool),
        ScalarType::Uuid => Some(NetworkWireScalarShape::FixedBytes(16)),
        ScalarType::Vector2 => Some(NetworkWireScalarShape::Vec2),
        ScalarType::Vector3 => Some(NetworkWireScalarShape::Vec3),
        ScalarType::Vector4 => Some(NetworkWireScalarShape::Vec4),
        ScalarType::Quaternion => Some(NetworkWireScalarShape::Quat),
        ScalarType::Transform => Some(NetworkWireScalarShape::Affine3),
        ScalarType::String => Some(NetworkWireScalarShape::String),
        ScalarType::AssetId | ScalarType::Color | ScalarType::ColorF | ScalarType::ColorB => None,
    }
}

const fn network_serialize_kind(kind: SerializeCodegenItemKind) -> NetworkSerializeKind {
    match kind {
        SerializeCodegenItemKind::Struct => NetworkSerializeKind::Struct,
        SerializeCodegenItemKind::Enum => NetworkSerializeKind::Enum,
    }
}

const fn network_serialize_role(role: ReflectedTypeRole) -> NetworkSerializeRole {
    match role {
        ReflectedTypeRole::FacetedComponent => NetworkSerializeRole::FacetedComponent,
        ReflectedTypeRole::AzComponent => NetworkSerializeRole::AzComponent,
        ReflectedTypeRole::ClientFacet => NetworkSerializeRole::ClientFacet,
        ReflectedTypeRole::ServerFacet => NetworkSerializeRole::ServerFacet,
        ReflectedTypeRole::AzEntity => NetworkSerializeRole::AzEntity,
        ReflectedTypeRole::SupportType => NetworkSerializeRole::SupportType,
    }
}

fn typeindex_evidence(
    type_index: u32,
    confidence: NetworkConfidence,
    detail: Option<String>,
) -> NetworkEvidence {
    NetworkEvidence {
        kind: NetworkEvidenceKind::TypeIndex,
        source: "typeIndex".to_owned(),
        address: None,
        detail: detail.or_else(|| Some(format!("typeIndex={type_index}"))),
        confidence,
    }
}

fn network_type_from_registry_entry(entry: &Map<String, Value>) -> NetworkType {
    let type_id = uuid(entry, "uuid");
    let storage_address = stable_address(entry, "storageAddress");
    let base_vtable = stable_address(entry, "baseVtable");
    let vtable = stable_address(entry, "vtable");
    let handler = entry
        .get("handler")
        .and_then(Value::as_object)
        .map(network_handler);
    let instance = entry
        .get("messageUnmarshal")
        .and_then(Value::as_object)
        .map(network_instance_layout);
    let fragment_metadata = network_type_fragment_metadata(entry);
    let az_rtti = entry
        .get("azRtti")
        .and_then(Value::as_object)
        .map(network_az_rtti);
    let registration_hook = entry
        .get("registrationHook")
        .and_then(Value::as_object)
        .map(network_registration_hook);
    let name = registry_entry_name(entry, az_rtti.as_ref(), registration_hook.as_ref());
    let mut fields = array_values(entry, "fields")
        .filter_map(Value::as_object)
        .filter(|field| is_plausible_network_field(field))
        .map(network_field)
        .collect::<Vec<_>>();
    reindex_message_fields(&mut fields);
    let has_registered_fields = fields.iter().any(|field| {
        field
            .evidence
            .iter()
            .any(|evidence| evidence.kind == NetworkEvidenceKind::RegisterField)
    });
    let capabilities = network_type_capabilities(name.as_deref(), has_registered_fields);
    let mut evidence = Vec::new();

    if type_id.is_some() || entry.contains_key("typeIndex") || entry.contains_key("index") {
        evidence.push(NetworkEvidence {
            kind: NetworkEvidenceKind::TypeRegistry,
            source: "registryEntries".to_owned(),
            address: storage_address.clone(),
            detail: name.clone(),
            confidence: NetworkConfidence::Exact,
        });
    }
    if let Some(hook) = &registration_hook {
        evidence.push(NetworkEvidence {
            kind: NetworkEvidenceKind::InstallRegistrationHook,
            source: "registrationHook".to_owned(),
            address: hook.hook_function.clone(),
            detail: hook
                .type_name
                .clone()
                .or_else(|| hook.slot_type_name.clone()),
            confidence: NetworkConfidence::High,
        });
    }
    if let Some(rtti) = &az_rtti {
        evidence.push(NetworkEvidence {
            kind: NetworkEvidenceKind::AzRtti,
            source: rtti.source.clone().unwrap_or_else(|| "azRtti".to_owned()),
            address: rtti.address.clone(),
            detail: rtti.type_name.clone(),
            confidence: NetworkConfidence::High,
        });
    }
    if handler.is_some() {
        evidence.push(NetworkEvidence {
            kind: NetworkEvidenceKind::HandlerVtable,
            source: "handler".to_owned(),
            address: vtable.clone().or_else(|| base_vtable.clone()),
            detail: None,
            confidence: NetworkConfidence::High,
        });
    }
    if let Some(metadata) = &fragment_metadata {
        evidence.push(NetworkEvidence {
            kind: NetworkEvidenceKind::FragmentMetadata,
            source: metadata
                .source
                .clone()
                .unwrap_or_else(|| "fragmentMetadata".to_owned()),
            address: metadata.category_function.clone(),
            detail: metadata.category.clone(),
            confidence: NetworkConfidence::High,
        });
    }

    NetworkType {
        type_id,
        type_index: u32_value(entry, "typeIndex"),
        registry_index: u32_value(entry, "index"),
        name,
        name_source: string(entry, "typeNameSource"),
        capabilities,
        storage_address,
        base_vtable,
        vtable,
        handler,
        instance,
        fragment_metadata,
        serialize: None,
        az_rtti,
        registration_type_name: string(entry, "registrationTypeName"),
        registration_hook,
        fields,
        evidence,
    }
}

fn is_plausible_network_field(field: &Map<String, Value>) -> bool {
    let Some(confidence) = string_ref(field, "confidence") else {
        return true;
    };
    if !confidence.starts_with("message-unmarshal") {
        return true;
    }

    let has_known_field_type = string_ref(field, "wireShape").is_some()
        || string_ref(field, "rustType").is_some()
        || string_ref(field, "nativeType").is_some();
    let Some(storage) = string_ref(field, "storageExpression") else {
        return has_known_field_type;
    };
    let storage = storage.trim();
    storage.starts_with("_Dst")
        || ((storage.contains("param_") || storage.contains("plVar") || storage.contains("puVar"))
            && storage.contains('+'))
}

fn reindex_message_fields(fields: &mut [NetworkField]) {
    if fields.iter().all(|field| {
        field
            .evidence
            .iter()
            .any(|evidence| evidence.kind == NetworkEvidenceKind::MessageUnmarshal)
    }) {
        for (index, field) in fields.iter_mut().enumerate() {
            field.index = Some(index as u32);
        }
    }
}

fn network_field_registration_function(
    function: &Map<String, Value>,
) -> NetworkFieldRegistrationFunction {
    let az_rtti = function
        .get("azRtti")
        .and_then(Value::as_object)
        .map(network_az_rtti);
    let fragment_metadata = function
        .get("fragmentMetadata")
        .and_then(Value::as_object)
        .map(network_fragment_metadata);
    let fields = array_values(function, "fields")
        .filter_map(Value::as_object)
        .map(network_field)
        .collect::<Vec<_>>();
    let virtual_functions = array_values(function, "virtualFunctions")
        .filter_map(Value::as_object)
        .map(network_virtual_function)
        .collect::<Vec<_>>();
    let mut evidence = vec![NetworkEvidence {
        kind: NetworkEvidenceKind::FieldRegistrationFunction,
        source: "fieldRegistrationFunctions".to_owned(),
        address: string(function, "address"),
        detail: string(function, "name"),
        confidence: NetworkConfidence::High,
    }];
    if let Some(rtti) = &az_rtti {
        evidence.push(NetworkEvidence {
            kind: NetworkEvidenceKind::AzRtti,
            source: rtti.source.clone().unwrap_or_else(|| "azRtti".to_owned()),
            address: rtti.address.clone(),
            detail: rtti.type_name.clone(),
            confidence: NetworkConfidence::High,
        });
    }
    if let Some(metadata) = &fragment_metadata {
        evidence.push(NetworkEvidence {
            kind: NetworkEvidenceKind::FragmentMetadata,
            source: metadata
                .source
                .clone()
                .unwrap_or_else(|| "fragmentMetadata".to_owned()),
            address: metadata.category_function.clone(),
            detail: metadata.category.clone(),
            confidence: NetworkConfidence::High,
        });
    }

    NetworkFieldRegistrationFunction {
        address: string(function, "address"),
        name: string(function, "name"),
        constructor_type_name: string(function, "constructorTypeName"),
        owner_type_id: az_rtti.as_ref().and_then(|rtti| rtti.type_id),
        owner_type_name: string(function, "constructorTypeName")
            .or_else(|| az_rtti.as_ref().and_then(|rtti| rtti.type_name.clone())),
        instance_vtable: string(function, "instanceVtable"),
        virtual_functions,
        fragment_metadata,
        az_rtti,
        fields,
        evidence,
    }
}

fn network_type_fragment_metadata(entry: &Map<String, Value>) -> Option<NetworkFragmentMetadata> {
    entry
        .get("fragmentMetadata")
        .and_then(Value::as_object)
        .map(network_fragment_metadata)
        .or_else(|| {
            array_values(entry, "constructorMatches")
                .filter_map(Value::as_object)
                .find_map(|constructor| {
                    constructor
                        .get("fragmentMetadata")
                        .and_then(Value::as_object)
                        .map(network_fragment_metadata)
                })
        })
}

fn network_fragment_metadata(metadata: &Map<String, Value>) -> NetworkFragmentMetadata {
    NetworkFragmentMetadata {
        source: string(metadata, "source"),
        is_metadata_slot: u32_value(metadata, "isMetadataSlot"),
        is_metadata_function: stable_address(metadata, "isMetadataFunction"),
        is_metadata: bool_value(metadata, "isMetadata"),
        category_slot: u32_value(metadata, "categorySlot"),
        category_function: stable_address(metadata, "categoryFunction"),
        category_value: u32_value(metadata, "categoryValue"),
        category: string(metadata, "category"),
    }
}

fn network_field(field: &Map<String, Value>) -> NetworkField {
    let raw_confidence = string(field, "confidence");
    let confidence = confidence_from_raw(string_ref(field, "confidence"));
    let evidence_kind = match raw_confidence.as_deref() {
        Some(value) if value.starts_with("message-unmarshal") => {
            NetworkEvidenceKind::MessageUnmarshal
        }
        Some(value) if value.starts_with("message-signature") => NetworkEvidenceKind::MessageSource,
        _ => NetworkEvidenceKind::RegisterField,
    };
    let mut evidence = vec![NetworkEvidence {
        kind: evidence_kind,
        source: raw_confidence.unwrap_or_else(|| "field".to_owned()),
        address: string(field, "callsite"),
        detail: string(field, "name").or_else(|| string(field, "nativeType")),
        confidence,
    }];
    if let Some(name_source) = string(field, "nameSource") {
        evidence.push(NetworkEvidence {
            kind: NetworkEvidenceKind::MessageSource,
            source: name_source,
            address: string(field, "nameSourceAddress"),
            detail: string(field, "sourceTypeName").or_else(|| string(field, "name")),
            confidence: NetworkConfidence::High,
        });
    }
    let mut native_type = string(field, "nativeType");
    let mut wire_shape = wire_shape(field, "wireShape");
    let mut wire_shape_source = string(field, "wireShapeSource");
    let raw_byte_length = u32_value(field, "rawByteLength");
    let helper_internal_conflict =
        raw_byte_length_conflicts_with_wire_shape(raw_byte_length, wire_shape)
            && wire_shape_source
                .as_deref()
                .is_some_and(|source| source.starts_with("message-unmarshal-helper-"));
    let raw_byte_length = consistent_raw_byte_length(raw_byte_length, wire_shape);
    if helper_internal_conflict {
        native_type = None;
        wire_shape = None;
        wire_shape_source = None;
    }
    NetworkField {
        index: u32_value(field, "index"),
        name: string(field, "name"),
        name_address: string(field, "nameAddress"),
        group: u32_value(field, "group"),
        registration_kind: string(field, "registrationKind"),
        filter_group_attribute: bool_value(field, "filterGroupAttribute"),
        handler_offset: string(field, "handlerOffset"),
        handler_expression: string(field, "handlerExpression"),
        handler_vtable: string(field, "handlerVtable"),
        handler_kind: string(field, "handlerKind"),
        handler_vtable_slots: u32_value(field, "handlerVtableSlots"),
        physical_field_count: u32_value(field, "physicalFieldCount"),
        native_type,
        source_type_name: string(field, "sourceTypeName"),
        source_type_id: uuid(field, "sourceTypeId"),
        rust_type: string(field, "rustType"),
        storage_expression: string(field, "storageExpression"),
        storage_offset: hex_or_decimal_u32(field, "storageOffset"),
        raw_byte_length,
        wire_shape,
        wire_shape_source,
        constructor_writes: network_field_constructor_writes(field),
        unmarshal_evidence: network_field_unmarshal_evidence(field),
        nested_type_shape: network_field_nested_type_shape(field),
        serialize: None,
        callsite: string(field, "callsite"),
        confidence,
        evidence,
    }
}

fn suppress_field_wire_shapes_for_vtables(fields: &mut [NetworkField], vtables: &BTreeSet<&str>) {
    for field in fields {
        let Some(handler_vtable) = field.handler_vtable.as_deref() else {
            continue;
        };
        if !vtables.contains(handler_vtable) {
            continue;
        }
        if field
            .wire_shape
            .is_some_and(NetworkWireShape::is_replicated_container)
        {
            field.wire_shape = None;
            field.wire_shape_source = None;
        }
    }
}

fn network_field_unmarshal_evidence(
    field: &Map<String, Value>,
) -> Option<NetworkFieldUnmarshalEvidence> {
    let evidence = field.get("unmarshalEvidence")?.as_object()?;
    Some(NetworkFieldUnmarshalEvidence {
        callsite: string(evidence, "callsite"),
        target_name: string(evidence, "targetName"),
        target_kind: string(evidence, "targetKind"),
        evidence_source: string(evidence, "evidenceSource"),
    })
}

fn network_field_nested_type_shape(field: &Map<String, Value>) -> Option<NetworkNestedTypeShape> {
    let shape = field.get("nestedTypeShape")?.as_object()?;
    Some(network_nested_type_shape(shape))
}

fn network_nested_type_shape(shape: &Map<String, Value>) -> NetworkNestedTypeShape {
    NetworkNestedTypeShape {
        type_id: uuid(shape, "typeId"),
        type_id_source: string(shape, "typeIdSource"),
        type_name: string(shape, "typeName"),
        type_name_full: string(shape, "typeNameFull"),
        type_name_source: string(shape, "typeNameSource"),
        function: string(shape, "function"),
        function_name: string(shape, "functionName"),
        factory: string(shape, "factory"),
        az_rtti_address: string(shape, "azRttiAddress"),
        constructor: string(shape, "constructor"),
        vtable: string(shape, "vtable"),
        member_base: string(shape, "memberBase"),
        member_name_source: string(shape, "memberNameSource"),
        member_names_proven: bool_value(shape, "memberNamesProven"),
        datatype_path: string(shape, "datatypePath"),
        validation: string(shape, "validation"),
        members: shape
            .get("members")
            .and_then(Value::as_array)
            .map(|members| {
                members
                    .iter()
                    .filter_map(Value::as_object)
                    .map(network_nested_type_member)
                    .collect()
            })
            .unwrap_or_default(),
    }
}

fn network_nested_type_member(member: &Map<String, Value>) -> NetworkNestedTypeMember {
    NetworkNestedTypeMember {
        index: u32_value(member, "index"),
        offset: string(member, "offset"),
        native_offset: string(member, "nativeOffset"),
        name: string(member, "name"),
        name_source: string(member, "nameSource"),
        name_proven: bool_value(member, "nameProven"),
        name_evidence: string(member, "nameEvidence"),
        native_type: string(member, "nativeType"),
        wire_shape: string(member, "wireShape"),
        byte_width: u32_value(member, "byteWidth"),
        evidence_source: string(member, "evidenceSource"),
        callsite: string(member, "callsite"),
        target: string(member, "target"),
        target_name: string(member, "targetName"),
    }
}

fn network_field_constructor_writes(
    field: &Map<String, Value>,
) -> Vec<NetworkFieldConstructorWrite> {
    array_values(field, "constructorWrites")
        .filter_map(Value::as_object)
        .map(|write| NetworkFieldConstructorWrite {
            write: string(write, "write"),
            handler_offset: string(write, "handlerOffset"),
            relative_offset: string(write, "relativeOffset"),
            width_bits: u32_value(write, "widthBits"),
            byte_length: u32_value(write, "byteLength"),
            value_kind: string(write, "valueKind"),
            value: string(write, "value"),
            value_hex: string(write, "valueHex"),
            source_operand: string(write, "sourceOperand"),
            source: string(write, "source"),
        })
        .collect()
}

fn contains_private_source_evidence(value: &Value) -> bool {
    match value {
        Value::Object(object) => object.iter().any(|(key, value)| {
            key.starts_with("sourceReplicated")
                || contains_private_source_marker(key)
                || contains_private_source_evidence(value)
        }),
        Value::Array(values) => values.iter().any(contains_private_source_evidence),
        Value::String(value) => contains_private_source_marker(value),
        _ => false,
    }
}

fn contains_private_source_marker(value: &str) -> bool {
    let normalized = value.replace('\\', "/").to_ascii_lowercase();
    normalized == "source-replicated-field-handler"
        || normalized.contains("resources/newworld/src")
        || normalized.contains("new-world/gems/newworld/src")
        || normalized.contains("newworld/src")
}

fn consistent_raw_byte_length(
    raw_byte_length: Option<u32>,
    wire_shape: Option<NetworkWireShape>,
) -> Option<u32> {
    let byte_length = raw_byte_length?;
    if raw_byte_length_conflicts_with_wire_shape(raw_byte_length, wire_shape) {
        None
    } else {
        Some(byte_length)
    }
}

fn raw_byte_length_conflicts_with_wire_shape(
    raw_byte_length: Option<u32>,
    wire_shape: Option<NetworkWireShape>,
) -> bool {
    let Some(byte_length) = raw_byte_length else {
        return false;
    };
    match wire_shape {
        Some(NetworkWireShape::U8) => byte_length != 1,
        Some(NetworkWireShape::FixedBytes(width)) => byte_length != u32::from(width),
        Some(_) => true,
        None => false,
    }
}

fn network_virtual_function(function: &Map<String, Value>) -> NetworkVirtualFunction {
    NetworkVirtualFunction {
        slot: u32_value(function, "slot"),
        slot_offset: string(function, "slotOffset"),
        name: string(function, "name"),
        address: string(function, "address"),
        target: string(function, "target"),
        function: string(function, "function"),
    }
}

fn network_field_handler_vtable(vtable: &Map<String, Value>) -> NetworkFieldHandlerVtable {
    let confidence = NetworkConfidence::High;
    let mut result = NetworkFieldHandlerVtable {
        address: string(vtable, "address"),
        field_count: usize_value(vtable, "fieldCount").unwrap_or_default(),
        handler_kind: string(vtable, "handlerKind"),
        vtable_slots: u32_value(vtable, "vtableSlots"),
        physical_field_count: u32_value(vtable, "physicalFieldCount"),
        marshal: string(vtable, "marshal"),
        marshal_target: string(vtable, "marshalTarget"),
        unmarshal: string(vtable, "unmarshal"),
        unmarshal_target: string(vtable, "unmarshalTarget"),
        wire_shape: wire_shape(vtable, "wireShape"),
        wire_shape_source: string(vtable, "wireShapeSource"),
        delta_wire_shape: wire_shape(vtable, "deltaWireShape"),
        full_wire_shape: wire_shape(vtable, "fullWireShape"),
        key_native_type: string(vtable, "keyNativeType"),
        key_native_type_source: string(vtable, "keyNativeTypeSource"),
        delta_marshal_shapes: string_array(vtable, "deltaMarshalShapes"),
        full_marshal_shapes: string_array(vtable, "fullMarshalShapes"),
        value_type_name: string(vtable, "valueTypeName"),
        value_type_id: string(vtable, "valueTypeId"),
        value_type_info_address: string(vtable, "valueTypeInfoAddress"),
        value_type_candidates: native_type_info_candidates(vtable, "valueTypeInfoCandidates"),
        value_type_shape: vtable
            .get("valueTypeShape")
            .and_then(Value::as_object)
            .map(network_nested_type_shape),
        embedded_value_type_shapes: array_values(vtable, "embeddedValueTypeShapes")
            .filter_map(Value::as_object)
            .map(network_nested_type_shape)
            .collect(),
        container_shape: vtable
            .get("containerShape")
            .cloned()
            .and_then(|shape| serde_json::from_value(shape).ok()),
        slots: array_values(vtable, "slots")
            .filter_map(Value::as_object)
            .map(network_virtual_function)
            .collect(),
        evidence: vec![NetworkEvidence {
            kind: NetworkEvidenceKind::HandlerVtable,
            source: "fieldHandlerVtables".to_owned(),
            address: string(vtable, "address"),
            detail: None,
            confidence,
        }],
    };
    if result.container_shape.is_none() {
        result.container_shape = replicated_container_shape_from_vtable(&result);
    }
    if result.should_suppress_replicated_container_wire_shape() {
        result.wire_shape = None;
        result.wire_shape_source = None;
        result.delta_wire_shape = None;
        result.full_wire_shape = None;
    }
    result
}

fn native_type_info_candidates(
    object: &Map<String, Value>,
    key: &str,
) -> Vec<NetworkNativeTypeInfoEvidence> {
    array_values(object, key)
        .filter_map(Value::as_object)
        .map(native_type_info_evidence)
        .collect()
}

fn native_type_info_evidence(object: &Map<String, Value>) -> NetworkNativeTypeInfoEvidence {
    NetworkNativeTypeInfoEvidence {
        address: string(object, "address"),
        name: string(object, "name"),
        type_id: uuid(object, "typeId"),
        source: string(object, "source"),
        name_source: string(object, "nameSource"),
    }
}

fn network_handler(handler: &Map<String, Value>) -> NetworkHandler {
    NetworkHandler {
        destructor: string(handler, "Destructor"),
        get_empty_value: string(handler, "GetEmptyValue"),
        create_instance: string(handler, "CreateInstance"),
        copy_value: string(handler, "CopyValue"),
        marshal: string(handler, "Marshal"),
        unmarshal: string(handler, "Unmarshal"),
    }
}

fn network_instance_layout(message_unmarshal: &Map<String, Value>) -> NetworkInstanceLayout {
    let confidence = if message_unmarshal.contains_key("instanceSize") {
        NetworkConfidence::High
    } else {
        NetworkConfidence::Inferred
    };
    NetworkInstanceLayout {
        create_instance: string(message_unmarshal, "createInstance"),
        size: hex_or_decimal_u32(message_unmarshal, "instanceSize"),
        size_source: string(message_unmarshal, "instanceSizeSource"),
        constructor: string(message_unmarshal, "instanceConstructor"),
        constructor_callsite: string(message_unmarshal, "instanceConstructorCallsite"),
        constructor_name: string(message_unmarshal, "instanceConstructorName"),
        evidence: vec![NetworkEvidence {
            kind: NetworkEvidenceKind::InstanceLayout,
            source: string(message_unmarshal, "instanceSizeSource")
                .unwrap_or_else(|| "messageUnmarshal".to_owned()),
            address: string(message_unmarshal, "createInstance"),
            detail: string(message_unmarshal, "instanceConstructorName"),
            confidence,
        }],
    }
}

fn network_az_rtti(rtti: &Map<String, Value>) -> NetworkAzRtti {
    NetworkAzRtti {
        source: string(rtti, "source"),
        address: string(rtti, "address"),
        type_id: uuid(rtti, "typeId"),
        type_name: string(rtti, "typeName"),
        providers: array_values(rtti, "providers")
            .filter_map(Value::as_object)
            .map(network_az_rtti_provider)
            .collect(),
    }
}

fn network_az_rtti_provider(provider: &Map<String, Value>) -> NetworkAzRttiProvider {
    NetworkAzRttiProvider {
        kind: string(provider, "kind"),
        slot: u32_value(provider, "slot"),
        slot_offset: string(provider, "slotOffset"),
        function: string(provider, "function"),
        provider: string(provider, "provider"),
        type_id: uuid(provider, "typeId"),
        type_id_source: string(provider, "typeIdSource"),
        type_name: string(provider, "typeName"),
        source_address: string(provider, "sourceAddress"),
    }
}

fn network_registration_hook(hook: &Map<String, Value>) -> NetworkRegistrationHook {
    NetworkRegistrationHook {
        type_id: uuid(hook, "typeId"),
        type_name: string(hook, "typeName"),
        slot_type_name: string(hook, "slotTypeName"),
        hook_function: string(hook, "hookFunction"),
        helper_table: string(hook, "helperTable"),
        register_thunk: string(hook, "registerThunk"),
        type_provider: string(hook, "typeProvider"),
        uuid_source: string(hook, "uuidSource"),
    }
}

fn registry_entry_name(
    entry: &Map<String, Value>,
    az_rtti: Option<&NetworkAzRtti>,
    registration_hook: Option<&NetworkRegistrationHook>,
) -> Option<String> {
    string(entry, "typeName")
        .or_else(|| string(entry, "name"))
        .or_else(|| string(entry, "registrationTypeName"))
        .or_else(|| registration_hook.and_then(|hook| hook.type_name.clone()))
        .or_else(|| az_rtti.and_then(|rtti| rtti.type_name.clone()))
}

fn network_type_capabilities(
    name: Option<&str>,
    has_registered_fields: bool,
) -> Vec<NetworkTypeCapability> {
    let mut capabilities = Vec::new();
    let is_direct_message = name.is_some_and(is_direct_message_name);
    if name.is_some_and(is_replicated_state_name) && !is_direct_message {
        capabilities.push(NetworkTypeCapability::ReplicatedState);
    }
    if is_direct_message {
        capabilities.push(NetworkTypeCapability::DirectMessage);
    }
    if has_registered_fields {
        capabilities.push(NetworkTypeCapability::RegisteredFields);
    }
    if capabilities.is_empty() {
        capabilities.push(NetworkTypeCapability::SupportData);
    }
    capabilities
}

fn is_replicated_state_name(name: &str) -> bool {
    let leaf = name.rsplit("::").next().unwrap_or(name);
    leaf != "ReplicatedState"
        && (leaf.ends_with("ReplicatedState") || leaf.contains("ReplicatedState<"))
}

fn is_direct_message_name(name: &str) -> bool {
    name.contains("ClientMessages::")
        || name.contains("ServerMessages::")
        || name.ends_with("Msg")
        || name.contains("Msg<")
}

fn confidence_from_raw(raw: Option<&str>) -> NetworkConfidence {
    match raw {
        Some("exact") => NetworkConfidence::Exact,
        Some(
            "register-field-call"
            | "registration-hook"
            | "az-rtti"
            | "message-unmarshal-call"
            | "message-signature-source",
        ) => NetworkConfidence::High,
        Some(value) if value.starts_with("message-unmarshal-") => NetworkConfidence::High,
        Some(value) if value.starts_with("message-signature-") => NetworkConfidence::High,
        Some(value) if value.starts_with("fixed-field-table-append") => NetworkConfidence::High,
        Some(value) if value.starts_with("fixed-attribute-table-append") => NetworkConfidence::High,
        Some("constructor-match" | "vtable-match") => NetworkConfidence::Inferred,
        Some("hint") => NetworkConfidence::Weak,
        Some(_) => NetworkConfidence::Unknown,
        None => NetworkConfidence::Unknown,
    }
}

fn array_values<'a>(
    object: &'a Map<String, Value>,
    key: &str,
) -> impl Iterator<Item = &'a Value> + 'a {
    object
        .get(key)
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
}

fn string(object: &Map<String, Value>, key: &str) -> Option<String> {
    string_ref(object, key).map(ToOwned::to_owned)
}

fn string_array(object: &Map<String, Value>, key: &str) -> Vec<String> {
    array_values(object, key)
        .filter_map(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
        .collect()
}

fn string_ref<'a>(object: &'a Map<String, Value>, key: &str) -> Option<&'a str> {
    object
        .get(key)
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
}

fn bool_value(object: &Map<String, Value>, key: &str) -> Option<bool> {
    object.get(key).and_then(Value::as_bool)
}

fn stable_address(object: &Map<String, Value>, key: &str) -> Option<String> {
    string_ref(object, key)
        .filter(|value| value.starts_with("NewWorld+0x"))
        .map(ToOwned::to_owned)
}

fn wire_shape(object: &Map<String, Value>, key: &str) -> Option<NetworkWireShape> {
    string_ref(object, key).and_then(parse_network_wire_shape)
}

fn parse_network_wire_shape(value: &str) -> Option<NetworkWireShape> {
    if let Some(container) = parse_replicated_container_wire_shape(value) {
        return Some(NetworkWireShape::ReplicatedContainer(container));
    }
    parse_network_wire_scalar_shape(value).map(Into::into)
}

fn parse_network_wire_scalar_shape(value: &str) -> Option<NetworkWireScalarShape> {
    match value {
        "bool" => Some(NetworkWireScalarShape::Bool),
        "u8" => Some(NetworkWireScalarShape::U8),
        "u16" => Some(NetworkWireScalarShape::U16),
        "u32" => Some(NetworkWireScalarShape::U32),
        "u64" => Some(NetworkWireScalarShape::U64),
        "f32" => Some(NetworkWireScalarShape::F32),
        "f64" => Some(NetworkWireScalarShape::F64),
        "half-f32" => Some(NetworkWireScalarShape::HalfF32),
        "vlq-u32" => Some(NetworkWireScalarShape::VlqU32),
        "vlq-u64" => Some(NetworkWireScalarShape::VlqU64),
        "sequence-number" => Some(NetworkWireScalarShape::SequenceNumber),
        "vec2" => Some(NetworkWireScalarShape::Vec2),
        "vec3" => Some(NetworkWireScalarShape::Vec3),
        "vec4" => Some(NetworkWireScalarShape::Vec4),
        "quat" => Some(NetworkWireScalarShape::Quat),
        "quat-comp-norm" => Some(NetworkWireScalarShape::QuatCompNorm),
        "vec2-comp" => Some(NetworkWireScalarShape::Vec2Comp),
        "vec3-comp" => Some(NetworkWireScalarShape::Vec3Comp),
        "vec3-comp-norm" => Some(NetworkWireScalarShape::Vec3CompNorm),
        "quat-comp" => Some(NetworkWireScalarShape::QuatComp),
        "quat-smallest-three" => Some(NetworkWireScalarShape::QuatSmallestThree),
        "non-uniform-scale-comp" => Some(NetworkWireScalarShape::NonUniformScaleComp),
        "position-anchor" => Some(NetworkWireScalarShape::PositionAnchor),
        "transform-compressor" => Some(NetworkWireScalarShape::TransformCompressor),
        "packed-size" => Some(NetworkWireScalarShape::PackedSize),
        "mat3" => Some(NetworkWireScalarShape::Mat3),
        "affine3" => Some(NetworkWireScalarShape::Affine3),
        "aabb2d" => Some(NetworkWireScalarShape::Aabb2d),
        "aabb3d" => Some(NetworkWireScalarShape::Aabb3d),
        "actor-ref" => Some(NetworkWireScalarShape::ActorRef),
        "entity-ref" => Some(NetworkWireScalarShape::EntityRef),
        "string" => Some(NetworkWireScalarShape::String),
        value => fixed_bytes_wire_shape(value),
    }
}

fn fixed_bytes_wire_shape(value: &str) -> Option<NetworkWireScalarShape> {
    let len = value
        .strip_prefix("fixed-bytes-")
        .or_else(|| value.strip_prefix("fixed-bytes"))?
        .parse::<u16>()
        .ok()?;
    (len > 0).then_some(NetworkWireScalarShape::FixedBytes(len))
}

fn parse_replicated_container_wire_shape(
    value: &str,
) -> Option<NetworkReplicatedContainerWireShape> {
    let inner = value
        .strip_prefix("replicated-container<")?
        .strip_suffix('>')?;
    let (key, value) = inner.split_once(',')?;
    Some(NetworkReplicatedContainerWireShape {
        key: parse_network_wire_scalar_shape(key.trim())?,
        value: parse_network_wire_scalar_shape(value.trim())?,
    })
}

fn replicated_container_shape_from_vtable(
    vtable: &NetworkFieldHandlerVtable,
) -> Option<NetworkReplicatedContainerShape> {
    let full_values_with_counts =
        replicated_container_full_value_scalar_shapes(&vtable.full_marshal_shapes);
    let full_data = replicated_container_data_scalar_shapes(&vtable.full_marshal_shapes);
    let full_values_match_data = full_values_with_counts == full_data
        || without_terminal_overflow_count(&full_values_with_counts).as_deref()
            == Some(full_data.as_slice());
    let delta_key_shapes = replicated_container_delta_key_shapes(&vtable.delta_marshal_shapes);
    let delta_key = replicated_container_delta_key_shape(&vtable.delta_marshal_shapes);
    let delta_value_shapes = replicated_container_delta_value_shapes(&vtable.delta_marshal_shapes);
    let value_type_shape_wire_shapes = selected_structured_container_value_wire_shapes(vtable);
    let whole_value_shapes =
        if selected_structured_container_value_shape_matches(vtable, &full_values_with_counts) {
            full_values_with_counts.clone()
        } else if selected_structured_container_value_shape_matches(vtable, &full_data) {
            full_data.clone()
        } else if delta_key == Some(NetworkWireScalarShape::VlqU64)
            && !value_type_shape_wire_shapes.is_empty()
        {
            value_type_shape_wire_shapes.clone()
        } else {
            Vec::new()
        };
    let whole_value_is_structured = !whole_value_shapes.is_empty();
    let structured_map_split =
        structured_container_map_split(vtable, &full_data, &delta_key_shapes);
    let (
        storage,
        key_wire_shape,
        key_wire_shapes,
        key_type_name,
        key_type_shape,
        value_wire_shapes,
        value_type_name,
        value_type_id,
        value_type_shape,
        delta_value_wire_shapes,
        source,
    ) = if let Some(split) = structured_map_split {
        (
            NetworkReplicatedContainerStorageKind::Map,
            split.key_wire_shapes[0],
            split.key_wire_shapes,
            split.key_type_name,
            split.key_type_shape,
            split.value_wire_shapes,
            split.value_type_name,
            None,
            split.value_type_shape,
            delta_value_shapes.unwrap_or_default(),
            "replicated-container-map-structured-key-shape",
        )
    } else if full_data.is_empty()
        && !value_type_shape_wire_shapes.is_empty()
        && vtable
            .value_type_shape
            .as_ref()
            .is_some_and(is_validated_anonymous_container_value_shape)
    {
        (
            NetworkReplicatedContainerStorageKind::Map,
            delta_key?,
            vec![delta_key?],
            None,
            None,
            value_type_shape_wire_shapes,
            selected_structured_container_value_type_name(vtable),
            selected_structured_container_value_type_id(vtable),
            vtable.value_type_shape.clone(),
            delta_value_shapes.unwrap_or_default(),
            "replicated-container-map-delta-value-shape",
        )
    } else if ((!full_data.is_empty() && full_values_match_data) || whole_value_is_structured)
        && (delta_key == Some(NetworkWireScalarShape::VlqU64)
            || (delta_key.is_none() && full_data.len() == 1 && !whole_value_is_structured))
        && delta_value_shapes.as_ref().is_none_or(|delta_values| {
            delta_key == Some(NetworkWireScalarShape::VlqU64)
                || delta_values.is_empty()
                || delta_values == &full_data
                || delta_values == &whole_value_shapes
                || whole_value_is_structured
        })
    {
        let source = if delta_key.is_some() && delta_value_shapes.is_some() {
            "replicated-container-vector-shape"
        } else {
            "replicated-container-vector-full-shape"
        };
        let value_wire_shapes = if whole_value_is_structured {
            whole_value_shapes
        } else {
            full_data
        };
        let value_type_shape = selected_vector_value_type_shape(vtable, &value_wire_shapes);
        (
            NetworkReplicatedContainerStorageKind::Vec,
            NetworkWireScalarShape::VlqU64,
            vec![NetworkWireScalarShape::VlqU64],
            None,
            None,
            value_wire_shapes,
            value_type_shape
                .as_ref()
                .and_then(nested_shape_source_type_name)
                .or_else(|| selected_structured_container_value_type_name(vtable)),
            selected_structured_container_value_type_id(vtable),
            value_type_shape,
            delta_value_shapes.unwrap_or_default(),
            source,
        )
    } else if full_data.len() >= 2 {
        let key = full_data[0];
        let values_with_counts = full_values_with_counts
            .get(1..)
            .map_or_else(Vec::new, <[_]>::to_vec);
        let value_shape_is_vector = container_value_shape_is_vector(vtable);
        let values =
            if selected_structured_container_value_shape_matches(vtable, &values_with_counts)
                || (value_shape_is_vector
                    && values_with_counts.first() == Some(&NetworkWireScalarShape::VlqU32))
            {
                values_with_counts
            } else {
                full_data[1..].to_vec()
            };
        if delta_key.is_some_and(|delta_key| delta_key != key) {
            return None;
        }
        let delta_shape_complete = delta_key.is_some() && delta_value_shapes.is_some();
        let mut value_type_shape =
            selected_map_value_type_shape(vtable, &[key], &values, &full_data)?;
        if value_type_shape.is_none() && value_shape_is_vector {
            value_type_shape = vtable.value_type_shape.clone();
        }
        if value_type_shape.is_none()
            && values.len() > 1
            && full_values_match_data
            && !has_explicit_container_value_evidence(vtable)
        {
            value_type_shape = Some(synthetic_container_value_shape_from_wire_shapes(
                "replicated-container-map-value-shape",
                &values,
            ));
        }
        let delta_values = delta_value_shapes.unwrap_or_default();
        let delta_values_match = delta_shape_complete && delta_values == values;
        if !delta_values_match
            && value_type_shape.is_none()
            && values.len() != 1
            && !has_selected_structured_container_value_identity(vtable)
        {
            return None;
        }
        (
            NetworkReplicatedContainerStorageKind::Map,
            key,
            vec![key],
            None,
            None,
            values,
            value_type_shape
                .as_ref()
                .and_then(nested_shape_source_type_name)
                .or_else(|| selected_structured_container_value_type_name(vtable)),
            selected_structured_container_value_type_id(vtable),
            value_type_shape,
            delta_values,
            if delta_values_match {
                "replicated-container-map-shape"
            } else {
                "replicated-container-map-full-shape"
            },
        )
    } else {
        return None;
    };

    Some(NetworkReplicatedContainerShape {
        storage,
        key_wire_shape,
        key_wire_shapes,
        key_native_type: vtable.key_native_type.clone(),
        key_native_type_source: vtable.key_native_type_source.clone(),
        key_type_name,
        key_type_shape,
        value_wire_shapes,
        delta_value_wire_shapes,
        value_type_name,
        value_type_id,
        value_type_info_address: vtable.value_type_info_address.clone(),
        value_type_shape,
        embedded_value_type_shapes: vtable.embedded_value_type_shapes.clone(),
        source: Some(source.to_owned()),
    })
}

fn without_terminal_overflow_count(
    shapes: &[NetworkWireScalarShape],
) -> Option<Vec<NetworkWireScalarShape>> {
    if shapes.len() <= 1
        || shapes.last() != Some(&NetworkWireScalarShape::VlqU32)
        || !shapes[..shapes.len() - 1]
            .iter()
            .any(|shape| *shape != NetworkWireScalarShape::VlqU32)
    {
        return None;
    }
    Some(shapes[..shapes.len() - 1].to_vec())
}

fn has_explicit_container_value_evidence(vtable: &NetworkFieldHandlerVtable) -> bool {
    vtable.value_type_shape.is_some()
        || vtable.value_type_name.is_some()
        || vtable.value_type_id.is_some()
        || !vtable.value_type_candidates.is_empty()
}

fn container_value_shape_is_vector(vtable: &NetworkFieldHandlerVtable) -> bool {
    vtable.value_type_shape.as_ref().is_some_and(|shape| {
        shape.members.iter().any(|member| {
            member
                .wire_shape
                .as_deref()
                .is_some_and(|wire_shape| vector_element_wire_shape(wire_shape).is_some())
        })
    })
}

fn selected_vector_value_type_shape(
    vtable: &NetworkFieldHandlerVtable,
    value_wire_shapes: &[NetworkWireScalarShape],
) -> Option<NetworkNestedTypeShape> {
    if value_wire_shapes.len() <= 1 {
        return None;
    }
    if let Some(shape) = vtable.value_type_shape.as_ref()
        && nested_type_shape_matches_wire_shapes(
            shape,
            value_wire_shapes,
            &vtable.embedded_value_type_shapes,
        )
    {
        return Some(shape.clone());
    }
    Some(synthetic_container_value_shape_from_wire_shapes(
        "replicated-container-vector-value-shape",
        value_wire_shapes,
    ))
}

fn selected_map_value_type_shape(
    vtable: &NetworkFieldHandlerVtable,
    key_wire_shapes: &[NetworkWireScalarShape],
    value_wire_shapes: &[NetworkWireScalarShape],
    full_wire_shapes: &[NetworkWireScalarShape],
) -> Option<Option<NetworkNestedTypeShape>> {
    let Some(shape) = vtable.value_type_shape.as_ref() else {
        return Some(None);
    };
    if nested_type_shape_matches_wire_shapes(
        shape,
        value_wire_shapes,
        &vtable.embedded_value_type_shapes,
    ) {
        return Some(Some(shape.clone()));
    }
    if !nested_type_shape_matches_wire_shapes(
        shape,
        full_wire_shapes,
        &vtable.embedded_value_type_shapes,
    ) {
        return Some(None);
    }
    split_nested_type_shape_after_wire_prefix(
        shape,
        key_wire_shapes,
        &vtable.embedded_value_type_shapes,
    )
    .map(Some)
}

fn synthetic_container_value_shape_from_wire_shapes(
    validation: &str,
    value_wire_shapes: &[NetworkWireScalarShape],
) -> NetworkNestedTypeShape {
    NetworkNestedTypeShape {
        type_id: None,
        type_id_source: None,
        type_name: Some("Value".to_owned()),
        type_name_full: None,
        type_name_source: Some(validation.to_owned()),
        function: None,
        function_name: None,
        factory: None,
        az_rtti_address: None,
        constructor: None,
        vtable: None,
        member_base: Some("value".to_owned()),
        member_name_source: Some(validation.to_owned()),
        member_names_proven: Some(false),
        datatype_path: None,
        validation: Some(validation.to_owned()),
        members: value_wire_shapes
            .iter()
            .enumerate()
            .map(|(index, shape)| synthetic_split_member(*shape, index))
            .collect(),
    }
}

struct StructuredContainerMapSplit {
    key_wire_shapes: Vec<NetworkWireScalarShape>,
    key_type_name: Option<String>,
    key_type_shape: Option<NetworkNestedTypeShape>,
    value_wire_shapes: Vec<NetworkWireScalarShape>,
    value_type_name: Option<String>,
    value_type_shape: Option<NetworkNestedTypeShape>,
}

fn structured_container_map_split(
    vtable: &NetworkFieldHandlerVtable,
    full_data: &[NetworkWireScalarShape],
    delta_key_shapes: &[NetworkWireScalarShape],
) -> Option<StructuredContainerMapSplit> {
    if delta_key_shapes.len() <= 1 {
        return None;
    }
    let shape = vtable.value_type_shape.as_ref()?;
    if !is_validated_anonymous_container_value_shape(shape) {
        return None;
    }
    let member_shapes = nested_type_member_wire_shapes(shape)?;
    let full_shape = member_shapes.iter().flatten().copied().collect::<Vec<_>>();
    if full_shape != full_data {
        return None;
    }

    let mut key_len = 0usize;
    for split_index in 1..member_shapes.len() {
        key_len += member_shapes[split_index - 1].len();
        if key_len >= full_shape.len() {
            break;
        }
        let key_wire_shapes = full_shape[..key_len].to_vec();
        if key_wire_shapes != delta_key_shapes {
            continue;
        }
        let value_wire_shapes = full_shape[key_len..].to_vec();
        if value_wire_shapes.is_empty() {
            continue;
        }
        let key_type_shape = split_nested_type_shape(shape, 0, split_index)?;
        let value_type_shape = split_nested_type_shape(shape, split_index, shape.members.len())?;
        return Some(StructuredContainerMapSplit {
            key_wire_shapes,
            key_type_name: nested_shape_source_type_name(&key_type_shape),
            key_type_shape: Some(key_type_shape),
            value_wire_shapes,
            value_type_name: nested_shape_source_type_name(&value_type_shape),
            value_type_shape: Some(value_type_shape),
        });
    }
    None
}

fn split_nested_type_shape_after_wire_prefix(
    shape: &NetworkNestedTypeShape,
    prefix: &[NetworkWireScalarShape],
    embedded_shapes: &[NetworkNestedTypeShape],
) -> Option<NetworkNestedTypeShape> {
    if prefix.is_empty() {
        return Some(shape.clone());
    }

    let mut remaining_prefix = prefix;
    let mut members = Vec::new();
    for member in &shape.members {
        let member_wire_shape = member.wire_shape.as_deref()?;
        let member_shapes = nested_member_wire_shapes(member_wire_shape, embedded_shapes)?;
        if !remaining_prefix.is_empty() {
            if remaining_prefix.len() >= member_shapes.len()
                && remaining_prefix.starts_with(&member_shapes)
            {
                remaining_prefix = &remaining_prefix[member_shapes.len()..];
                continue;
            }
            if member_shapes.starts_with(remaining_prefix) {
                for shape in &member_shapes[remaining_prefix.len()..] {
                    members.push(synthetic_split_member(*shape, members.len()));
                }
                remaining_prefix = &[];
                continue;
            }
            return None;
        }

        let mut member = member.clone();
        member.index = u32::try_from(members.len()).ok();
        members.push(member);
    }

    if !remaining_prefix.is_empty() || members.is_empty() {
        return None;
    }

    let mut split = shape.clone();
    split.type_id = None;
    split.type_id_source = None;
    split.type_name_full = None;
    split.type_name = shape.type_name.clone().or_else(|| Some("Value".to_owned()));
    split.type_name_source = Some("container-value-prefix-split".to_owned());
    split.member_name_source = Some("container-value-prefix-split".to_owned());
    split.member_names_proven = Some(false);
    split.validation = Some("container-value-shape-prefix-split".to_owned());
    split.members = members;
    Some(split)
}

fn synthetic_split_member(shape: NetworkWireScalarShape, index: usize) -> NetworkNestedTypeMember {
    NetworkNestedTypeMember {
        index: u32::try_from(index).ok(),
        offset: None,
        native_offset: None,
        name: Some(format!("field_{index}")),
        name_source: Some("container-value-prefix-split".to_owned()),
        name_proven: Some(false),
        name_evidence: Some("scalar suffix after replicated-container key split".to_owned()),
        native_type: None,
        wire_shape: Some(shape.wire_string()),
        byte_width: None,
        evidence_source: Some("container-value-shape-prefix-split".to_owned()),
        callsite: None,
        target: None,
        target_name: None,
    }
}

fn nested_type_member_wire_shapes(
    shape: &NetworkNestedTypeShape,
) -> Option<Vec<Vec<NetworkWireScalarShape>>> {
    shape
        .members
        .iter()
        .map(|member| {
            let wire_shape = member.wire_shape.as_deref()?;
            let mut member_shape = Vec::new();
            if let Some(scalar) = wire_scalar_shape_from_member_name(wire_shape) {
                member_shape.push(scalar);
            } else if let Some(composite) = composite_member_wire_shapes(wire_shape) {
                member_shape.extend(composite);
            } else {
                match wire_shape {
                    "vec2" => member_shape.extend([NetworkWireScalarShape::F32; 2]),
                    "vec3" => member_shape.extend([NetworkWireScalarShape::F32; 3]),
                    "vec4" | "quat" => member_shape.extend([NetworkWireScalarShape::F32; 4]),
                    _ => return None,
                }
            }
            Some(member_shape)
        })
        .collect()
}

fn split_nested_type_shape(
    shape: &NetworkNestedTypeShape,
    start: usize,
    end: usize,
) -> Option<NetworkNestedTypeShape> {
    let members = shape.members.get(start..end)?.to_vec();
    if members.is_empty() {
        return None;
    }
    let mut split = shape.clone();
    split.type_id = None;
    split.type_id_source = None;
    split.type_name = None;
    split.type_name_full = None;
    split.type_name_source = Some("container-structured-member-split".to_owned());
    split.members = members;
    if let Some(type_name) = nested_shape_source_type_name(&split) {
        split.type_name = Some(type_name);
    }
    Some(split)
}

fn nested_shape_source_type_name(shape: &NetworkNestedTypeShape) -> Option<String> {
    let [member] = shape.members.as_slice() else {
        return None;
    };
    let native_type = member.native_type.as_deref()?.trim();
    let leaf = native_type.rsplit("::").next().unwrap_or(native_type);
    leaf.chars()
        .next()
        .is_some_and(|first| first.is_ascii_uppercase())
        .then(|| leaf.to_owned())
}

fn selected_structured_container_value_type_name(
    vtable: &NetworkFieldHandlerVtable,
) -> Option<String> {
    vtable
        .value_type_name
        .clone()
        .or_else(|| vtable.value_type_shape.as_ref()?.type_name_full.clone())
        .or_else(|| vtable.value_type_shape.as_ref()?.type_name.clone())
}

fn selected_structured_container_value_type_id(vtable: &NetworkFieldHandlerVtable) -> Option<Uuid> {
    vtable
        .value_type_id
        .as_deref()
        .and_then(|type_id| Uuid::parse_str(type_id.trim_matches(['{', '}'])).ok())
        .or_else(|| vtable.value_type_shape.as_ref()?.type_id)
}

fn has_selected_structured_container_value_identity(vtable: &NetworkFieldHandlerVtable) -> bool {
    selected_structured_container_value_type_id(vtable).is_some()
        || vtable
            .value_type_shape
            .as_ref()
            .is_some_and(is_validated_anonymous_container_value_shape)
}

fn is_validated_anonymous_container_value_shape(shape: &NetworkNestedTypeShape) -> bool {
    shape.type_id.is_none()
        && !shape.members.is_empty()
        && shape.validation.as_deref().is_some_and(|validation| {
            (validation.contains("container-value")
                && (validation.contains("serialize-type-sequence")
                    || validation.contains("wire-sequence")))
                || (validation == "custom-replicated-container-value-shape"
                    && shape.member_names_proven == Some(true))
        })
}

fn replicated_container_full_value_scalar_shapes(shapes: &[String]) -> Vec<NetworkWireScalarShape> {
    let start = shapes
        .iter()
        .position(|shape| shape == "sequence-number")
        .map_or(0, |index| index + 1);
    let mut skipped_outer_count = false;
    shapes[start..]
        .iter()
        .filter_map(|shape| {
            let shape = shape.as_str();
            if shape == "sequence-number" {
                return None;
            }
            if !skipped_outer_count && shape == "vlq-u32" {
                skipped_outer_count = true;
                return None;
            }
            parse_network_wire_scalar_shape(shape)
        })
        .collect()
}

fn replicated_container_data_scalar_shapes(shapes: &[String]) -> Vec<NetworkWireScalarShape> {
    shapes
        .iter()
        .filter_map(|shape| {
            let shape = shape.as_str();
            is_replicated_container_data_shape(shape)
                .then(|| parse_network_wire_scalar_shape(shape))
                .flatten()
        })
        .collect()
}

fn replicated_container_delta_key_shape(shapes: &[String]) -> Option<NetworkWireScalarShape> {
    let key_shapes = replicated_container_delta_key_shapes(shapes);
    let [key_shape] = key_shapes.as_slice() else {
        return None;
    };
    Some(*key_shape)
}

fn replicated_container_delta_key_shapes(shapes: &[String]) -> Vec<NetworkWireScalarShape> {
    let Some(sequence_index) = shapes.iter().position(|shape| shape == "sequence-number") else {
        return Vec::new();
    };
    let prefix = shapes[..sequence_index]
        .iter()
        .filter_map(|shape| parse_network_wire_scalar_shape(shape.as_str()))
        .collect::<Vec<_>>();
    if let Some(key_shapes) = strip_replicated_container_delta_control_prefix(&prefix) {
        key_shapes.to_vec()
    } else if prefix.len() > 3
        && prefix.first() == Some(&NetworkWireScalarShape::VlqU32)
        && prefix.get(1) == Some(&NetworkWireScalarShape::U8)
    {
        prefix[2..].to_vec()
    } else if prefix.len() > 2
        && prefix.first() == Some(&NetworkWireScalarShape::VlqU32)
        && prefix.get(1) != Some(&NetworkWireScalarShape::U8)
    {
        prefix[1..].to_vec()
    } else {
        prefix.last().copied().into_iter().collect()
    }
}

fn strip_replicated_container_delta_control_prefix(
    shapes: &[NetworkWireScalarShape],
) -> Option<&[NetworkWireScalarShape]> {
    if matches!(
        shapes,
        [
            NetworkWireScalarShape::VlqU32,
            NetworkWireScalarShape::VlqU32,
            NetworkWireScalarShape::U8,
            ..
        ]
    ) {
        Some(&shapes[3..])
    } else {
        None
    }
}

fn replicated_container_delta_value_shapes(
    shapes: &[String],
) -> Option<Vec<NetworkWireScalarShape>> {
    let sequence_index = shapes.iter().position(|shape| shape == "sequence-number")?;
    Some(
        shapes[sequence_index + 1..]
            .iter()
            .filter_map(|shape| {
                let shape = shape.as_str();
                (shape != "sequence-number")
                    .then(|| parse_network_wire_scalar_shape(shape))
                    .flatten()
            })
            .collect(),
    )
}

fn replicated_container_data_shape_count(shapes: &[String]) -> usize {
    shapes
        .iter()
        .filter(|shape| is_replicated_container_data_shape(shape))
        .count()
}

fn is_replicated_container_data_shape(shape: &str) -> bool {
    shape != "vlq-u32" && shape != "sequence-number"
}

fn selected_structured_container_value_shape_matches(
    vtable: &NetworkFieldHandlerVtable,
    wire_shapes: &[NetworkWireScalarShape],
) -> bool {
    selected_structured_container_value_type_name(vtable)
        .as_deref()
        .is_some_and(|name| {
            let name = name.trim();
            !name.is_empty() && name != "unknown"
        })
        && has_selected_structured_container_value_identity(vtable)
        && vtable.value_type_shape.as_ref().is_some_and(|shape| {
            nested_type_shape_matches_wire_shapes(
                shape,
                wire_shapes,
                &vtable.embedded_value_type_shapes,
            )
        })
}

fn selected_structured_container_value_wire_shapes(
    vtable: &NetworkFieldHandlerVtable,
) -> Vec<NetworkWireScalarShape> {
    if selected_structured_container_value_type_name(vtable)
        .as_deref()
        .is_none_or(|name| {
            let name = name.trim();
            name.is_empty() || name == "unknown"
        })
        || !has_selected_structured_container_value_identity(vtable)
    {
        return Vec::new();
    }

    vtable
        .value_type_shape
        .as_ref()
        .and_then(|shape| nested_type_shape_wire_shapes(shape, &vtable.embedded_value_type_shapes))
        .unwrap_or_default()
}

fn nested_type_shape_wire_shapes(
    shape: &NetworkNestedTypeShape,
    embedded_shapes: &[NetworkNestedTypeShape],
) -> Option<Vec<NetworkWireScalarShape>> {
    if shape.members.is_empty() {
        return None;
    }

    let mut shapes = Vec::new();
    for member in &shape.members {
        let wire_shape = member.wire_shape.as_deref()?;
        shapes.extend(nested_member_wire_shapes(wire_shape, embedded_shapes)?);
    }
    (!shapes.is_empty()).then_some(shapes)
}

fn nested_type_shape_matches_wire_shapes(
    shape: &NetworkNestedTypeShape,
    wire_shapes: &[NetworkWireScalarShape],
    embedded_shapes: &[NetworkNestedTypeShape],
) -> bool {
    if shape.members.is_empty() || wire_shapes.is_empty() {
        return false;
    }
    let mut index = 0;
    for member in &shape.members {
        let Some(wire_shape) = member.wire_shape.as_deref() else {
            return false;
        };
        let Some(span) =
            nested_member_wire_shape_span(wire_shape, wire_shapes, index, embedded_shapes)
        else {
            return false;
        };
        index += span;
    }
    index == wire_shapes.len()
}

fn nested_member_wire_shape_span(
    observed: &str,
    expected: &[NetworkWireScalarShape],
    index: usize,
    embedded_shapes: &[NetworkNestedTypeShape],
) -> Option<usize> {
    let next = *expected.get(index)?;
    if wire_scalar_shape_from_member_name(observed) == Some(next) {
        return Some(1);
    }
    if let Some(composite) = composite_member_wire_shapes(observed) {
        let end = index.checked_add(composite.len())?;
        let expected = expected.get(index..end)?;
        return (expected == composite.as_slice()).then_some(composite.len());
    }
    if let Some(embedded) = nested_shape_by_wire_name(observed, embedded_shapes) {
        let embedded_shapes = nested_type_shape_wire_shapes(embedded, embedded_shapes)?;
        let span = embedded_shapes.len();
        let expected = expected.get(index..index.checked_add(span)?)?;
        return (expected == embedded_shapes.as_slice()).then_some(span);
    }
    match observed {
        "vec2" if expected_shape_run(expected, index, NetworkWireScalarShape::F32, 2) => Some(2),
        "vec3" if expected_shape_run(expected, index, NetworkWireScalarShape::F32, 3) => Some(3),
        "vec4" | "quat" if expected_shape_run(expected, index, NetworkWireScalarShape::F32, 4) => {
            Some(4)
        }
        observed => {
            let element = vector_element_wire_shape(observed)?;
            if next != NetworkWireScalarShape::VlqU32 {
                return None;
            }
            if let Some(embedded) = nested_shape_by_wire_name(element, embedded_shapes) {
                let element_shapes = nested_type_shape_wire_shapes(embedded, embedded_shapes)?;
                let span = 1usize.checked_add(element_shapes.len())?;
                let expected = expected.get(index + 1..index + span)?;
                return (expected == element_shapes.as_slice()).then_some(span);
            }
            if expected
                .get(index + 1)
                .is_some_and(|shape| wire_scalar_shape_from_member_name(element) == Some(*shape))
            {
                Some(2)
            } else {
                Some(1)
            }
        }
    }
}

fn nested_member_wire_shapes(
    observed: &str,
    embedded_shapes: &[NetworkNestedTypeShape],
) -> Option<Vec<NetworkWireScalarShape>> {
    if let Some(scalar) = wire_scalar_shape_from_member_name(observed) {
        return Some(vec![scalar]);
    }
    if let Some(composite) = composite_member_wire_shapes(observed) {
        return Some(composite);
    }
    if let Some(embedded) = nested_shape_by_wire_name(observed, embedded_shapes) {
        return nested_type_shape_wire_shapes(embedded, embedded_shapes);
    }
    match observed {
        "vec2" => Some(vec![NetworkWireScalarShape::F32; 2]),
        "vec3" => Some(vec![NetworkWireScalarShape::F32; 3]),
        "vec4" | "quat" => Some(vec![NetworkWireScalarShape::F32; 4]),
        observed => {
            let element = vector_element_wire_shape(observed)?;
            let embedded = nested_shape_by_wire_name(element, embedded_shapes)?;
            let mut shapes = vec![NetworkWireScalarShape::VlqU32];
            shapes.extend(nested_type_shape_wire_shapes(embedded, embedded_shapes)?);
            Some(shapes)
        }
    }
}

fn nested_shape_by_wire_name<'a>(
    name: &str,
    shapes: &'a [NetworkNestedTypeShape],
) -> Option<&'a NetworkNestedTypeShape> {
    shapes.iter().find(|shape| {
        [shape.type_name.as_deref(), shape.type_name_full.as_deref()]
            .into_iter()
            .flatten()
            .any(|candidate| type_name_leaf(candidate) == name)
    })
}

fn type_name_leaf(name: &str) -> &str {
    name.rsplit("::").next().unwrap_or(name).trim()
}

fn expected_shape_run(
    shapes: &[NetworkWireScalarShape],
    start: usize,
    shape: NetworkWireScalarShape,
    count: usize,
) -> bool {
    shapes
        .get(start..start + count)
        .is_some_and(|slice| slice.iter().all(|candidate| *candidate == shape))
}

fn vector_element_wire_shape(value: &str) -> Option<&str> {
    value
        .strip_prefix("vec<")?
        .strip_suffix('>')
        .map(str::trim)
        .filter(|value| !value.is_empty())
}

fn composite_member_wire_shapes(value: &str) -> Option<Vec<NetworkWireScalarShape>> {
    value
        .strip_prefix("composite<")?
        .strip_suffix('>')?
        .split(',')
        .map(str::trim)
        .map(parse_network_wire_scalar_shape)
        .collect()
}

fn wire_scalar_shape_from_member_name(value: &str) -> Option<NetworkWireScalarShape> {
    parse_network_wire_scalar_shape(value)
}

fn u32_value(object: &Map<String, Value>, key: &str) -> Option<u32> {
    object.get(key).and_then(|value| match value {
        Value::Number(number) => number.as_u64().and_then(|value| value.try_into().ok()),
        Value::String(value) => value.parse().ok(),
        _ => None,
    })
}

fn hex_or_decimal_u32(object: &Map<String, Value>, key: &str) -> Option<u32> {
    object.get(key).and_then(|value| match value {
        Value::Number(number) => number.as_u64().and_then(|value| value.try_into().ok()),
        Value::String(value) => {
            let trimmed = value.trim();
            trimmed
                .strip_prefix("0x")
                .or_else(|| trimmed.strip_prefix("0X"))
                .map_or_else(
                    || trimmed.parse().ok(),
                    |hex| u32::from_str_radix(hex, 16).ok(),
                )
        }
        _ => None,
    })
}

fn usize_value(object: &Map<String, Value>, key: &str) -> Option<usize> {
    object.get(key).and_then(|value| match value {
        Value::Number(number) => number.as_u64().and_then(|value| value.try_into().ok()),
        Value::String(value) => value.parse().ok(),
        _ => None,
    })
}

fn uuid(object: &Map<String, Value>, key: &str) -> Option<Uuid> {
    string_ref(object, key).and_then(parse_uuid)
}

fn parse_uuid(value: &str) -> Option<Uuid> {
    Uuid::parse_str(value.trim_matches(['{', '}'])).ok()
}

#[cfg(test)]
mod tests {
    use crate::ir::{
        SerializeCodegenItem, SerializeCodegenItemKind, SerializeCodegenRttiBase,
        SerializeCodegenUnit,
    };
    use crate::role::ReflectedTypeRole;
    use serde_json::json;
    use uuid::uuid;

    use super::*;

    fn fragment_access_message_signatures() -> Vec<NetworkMessageSignature> {
        vec![
            NetworkMessageSignature {
                type_id: Some(uuid!("96a58e69-7bd5-45c5-86e4-daf9f5eb1e86")),
                type_index: Some(397),
                name: Some("Replicate::RegisterFragmentAccessMsg".to_owned()),
                rust_name: Some("RegisterFragmentAccessMsg".to_owned()),
                source: None,
                fields: fragment_access_fields(),
            },
            NetworkMessageSignature {
                type_id: Some(uuid!("2b7640e0-4204-4e52-998a-c2db02e0a480")),
                type_index: Some(399),
                name: Some("Replicate::UnregisterFragmentAccessMsg".to_owned()),
                rust_name: Some("UnregisterFragmentAccessMsg".to_owned()),
                source: None,
                fields: fragment_access_fields(),
            },
            NetworkMessageSignature {
                type_id: Some(uuid!("951ef3ed-c9a0-4e3d-a6fd-7fe0673d28d2")),
                type_index: Some(422),
                name: Some("ReplicateClient::FragmentUpdateMsg".to_owned()),
                rust_name: Some("FragmentUpdateMsg".to_owned()),
                source: None,
                fields: vec![
                    message_field_signature(0, "TargetRef", "ActorRef"),
                    message_field_signature(1, "Key", "FragmentKey"),
                    message_field_signature(2, "Fragment", "BaselineableFragment"),
                ],
            },
        ]
    }

    fn fragment_access_fields() -> Vec<NetworkMessageFieldSignature> {
        vec![
            message_field_signature(0, "ProxyRef", "ActorRef"),
            message_field_signature(1, "Key", "FragmentKey"),
        ]
    }

    fn message_field_signature(
        index: u32,
        name: &str,
        native_type: &str,
    ) -> NetworkMessageFieldSignature {
        NetworkMessageFieldSignature {
            index: Some(index),
            name: name.to_owned(),
            rust_type: None,
            native_type: Some(native_type.to_owned()),
            wire_shape: None,
        }
    }

    fn assert_fragment_access_fields(fields: &[NetworkField]) {
        assert_eq!(fields[0].name.as_deref(), Some("ProxyRef"));
        assert_eq!(fields[0].native_type.as_deref(), Some("ActorRef"));
        assert_eq!(fields[1].name.as_deref(), Some("Key"));
        assert_eq!(fields[1].native_type.as_deref(), Some("FragmentKey"));
    }

    #[test]
    fn imports_fragment_metadata_from_constructor_matches() {
        let schema = NetworkSchema::from_ghidra_static_network_report(&json!({
            "registryEntries": [{
                "uuid": "39B4C919-3A6D-46B5-92D0-3B4ACB284B1D",
                "typeIndex": 16,
                "typeName": "MB::ProjectileReplicatedState",
                "constructorMatches": [{
                    "address": "NewWorld+0x683fe00",
                    "name": "MB::ProjectileReplicatedState::ProjectileReplicatedState",
                    "instanceVtable": "NewWorld+0x8549c70",
                    "fragmentMetadata": {
                        "source": "i-fragment-vtable",
                        "isMetadataSlot": 12,
                        "isMetadataFunction": "NewWorld+0x294910",
                        "isMetadata": false,
                        "categorySlot": 13,
                        "categoryFunction": "NewWorld+0x294910",
                        "categoryValue": 0,
                        "category": "Uncategorized"
                    },
                    "fields": []
                }]
            }],
            "fieldRegistrationFunctions": [{
                "address": "NewWorld+0x683fe00",
                "name": "MB::ProjectileReplicatedState::RegisterFields",
                "instanceVtable": "NewWorld+0x8549c70",
                "fragmentMetadata": {
                    "source": "i-fragment-vtable",
                    "isMetadataSlot": 12,
                    "isMetadataFunction": "NewWorld+0x294910",
                    "isMetadata": false,
                    "categorySlot": 13,
                    "categoryFunction": "NewWorld+0x294910",
                    "categoryValue": 0,
                    "category": "Uncategorized"
                },
                "fields": []
            }],
            "fieldHandlerVtables": []
        }))
        .expect("schema");

        let metadata = schema.types[0]
            .fragment_metadata
            .as_ref()
            .expect("type fragment metadata");
        assert_eq!(metadata.is_metadata, Some(false));
        assert_eq!(metadata.category_value, Some(0));
        assert_eq!(metadata.category.as_deref(), Some("Uncategorized"));
        assert_eq!(
            metadata.category_function.as_deref(),
            Some("NewWorld+0x294910")
        );

        let function_metadata = schema.field_registration_functions[0]
            .fragment_metadata
            .as_ref()
            .expect("function fragment metadata");
        assert_eq!(function_metadata.category.as_deref(), Some("Uncategorized"));
    }

    #[test]
    fn converts_ghidra_report_to_normalized_network_schema() {
        let report = json!({
            "schema": "newworld.network_schema.static.v1",
            "program": "NewWorld.exe",
            "imageBase": "NewWorld+0x0",
            "input": "E:/Projects/new-world/resources/typeregistry.json",
            "registryEntries": [{
                "uuid": "A85DF621-DCE0-409F-8D39-A447EA0807FF",
                "index": 1637,
                "typeIndex": 28,
                "storageAddress": "0x1e0e00aa6c0",
                "baseVtable": "NewWorld+0x84cb580",
                "vtable": "0x1e0e00aa6b0",
                "typeName": "Javelin::RaidDataComponentReplicatedState",
                "typeNameSource": "registrationHook",
                "handler": {
                    "Destructor": "NewWorld+0x3495230",
                    "GetEmptyValue": "NewWorld+0x3495270",
                    "CreateInstance": "NewWorld+0x34952b0",
                    "CopyValue": "NewWorld+0x34952c0",
                    "Marshal": "NewWorld+0x34952d0",
                    "Unmarshal": "NewWorld+0x3495310"
                },
                "azRtti": {
                    "source": "instance-vtable",
                    "address": "NewWorld+0x81e23a8",
                    "typeId": "A85DF621-DCE0-409F-8D39-A447EA0807FF",
                    "providers": [{
                        "kind": "typeId",
                        "slot": 1,
                        "slotOffset": "0x8",
                        "function": "NewWorld+0x34aa660",
                        "provider": "NewWorld+0x34aa660",
                        "typeId": "A85DF621-DCE0-409F-8D39-A447EA0807FF",
                        "typeIdSource": "sourceLiteral",
                        "sourceAddress": "NewWorld+0x81ddfb8"
                    }]
                },
                "registrationHook": {
                    "typeId": "A85DF621-DCE0-409F-8D39-A447EA0807FF",
                    "typeName": "Javelin::RaidDataComponentReplicatedState",
                    "slotTypeName": "Javelin::RaidDataComponentReplicatedState",
                    "hookFunction": "NewWorld+0x15ce50",
                    "helperTable": "NewWorld+0x81e03b0",
                    "registerThunk": "NewWorld+0x34761e0",
                    "typeProvider": "NewWorld+0x34aa660",
                    "uuidSource": "NewWorld+0x81ddfb8"
                },
                "fields": [{
                    "index": 0,
                    "callsite": "NewWorld+0x3495762",
                    "name": "raidId",
                    "nameAddress": "NewWorld+0x81db5f4",
                    "group": 0,
                    "handlerExpression": "R15",
                    "handlerVtable": "NewWorld+0x81dad80",
                    "confidence": "register-field-call"
                }]
            }],
            "fieldRegistrationFunctions": [{
                "address": "NewWorld+0x3495550",
                "name": "Javelin::RaidDataComponentReplicatedState::RegisterFields",
                "instanceVtable": "NewWorld+0x81e23a8",
                "azRtti": {
                    "source": "instance-vtable",
                    "address": "NewWorld+0x81e23a8",
                    "typeId": "A85DF621-DCE0-409F-8D39-A447EA0807FF"
                },
                "fields": [{
                    "index": 0,
                    "callsite": "NewWorld+0x3495762",
                    "name": "raidId",
                    "group": 0,
                    "confidence": "register-field-call"
                }]
            }],
            "fieldHandlerVtables": [{
                "address": "NewWorld+0x81dad80",
                "fieldCount": 1,
                "marshal": "NewWorld+0x344a700",
                "marshalTarget": "NewWorld+0x17266c0",
                "unmarshal": "NewWorld+0x3464830",
                "wireShape": "u64",
                "wireShapeSource": "marshal-call:marshal-function-name",
                "slots": [{
                    "slot": 5,
                    "slotOffset": "0x28",
                    "name": "Marshal",
                    "address": "NewWorld+0x344a700",
                    "target": "NewWorld+0x17266c0"
                }, {
                    "slot": 6,
                    "slotOffset": "0x30",
                    "name": "Unmarshal",
                    "address": "NewWorld+0x3464830"
                }]
            }]
        });

        let schema =
            NetworkSchema::from_ghidra_static_network_report(&report).expect("normalized schema");

        assert_eq!(schema.schema, NETWORK_SCHEMA_VERSION);
        assert_eq!(
            schema.sources[0].path.as_deref(),
            Some("E:/Projects/new-world/resources/typeregistry.json")
        );
        assert_eq!(
            schema.sources[0].schema.as_deref(),
            Some(NETWORK_STATIC_REPORT_SCHEMA_VERSION)
        );
        assert_eq!(schema.summary.type_count, 1);
        assert_eq!(schema.summary.register_field_function_count, 1);
        assert_eq!(schema.summary.register_field_count, 1);
        assert_eq!(schema.summary.high_confidence_field_count, 1);
        assert_eq!(schema.summary.field_handler_vtable_count, 1);

        let network_type = &schema.types[0];
        assert_eq!(
            network_type.type_id,
            Some(uuid!("a85df621-dce0-409f-8d39-a447ea0807ff"))
        );
        assert_eq!(network_type.type_index, Some(28));
        assert_eq!(
            network_type.name.as_deref(),
            Some("Javelin::RaidDataComponentReplicatedState")
        );
        assert_eq!(network_type.storage_address, None);
        assert_eq!(
            network_type.base_vtable.as_deref(),
            Some("NewWorld+0x84cb580")
        );
        assert_eq!(network_type.vtable, None);
        assert_eq!(
            network_type.capabilities,
            vec![
                NetworkTypeCapability::ReplicatedState,
                NetworkTypeCapability::RegisteredFields
            ]
        );
        assert_eq!(
            network_type
                .handler
                .as_ref()
                .and_then(|handler| handler.unmarshal.as_deref()),
            Some("NewWorld+0x3495310")
        );
        assert_eq!(network_type.fields[0].name.as_deref(), Some("raidId"));
        assert_eq!(network_type.fields[0].group, Some(0));
        assert_eq!(
            network_type.fields[0].handler_vtable.as_deref(),
            Some("NewWorld+0x81dad80")
        );
        assert_eq!(network_type.fields[0].confidence, NetworkConfidence::High);

        let function = &schema.field_registration_functions[0];
        assert_eq!(function.owner_type_id, network_type.type_id);
        assert_eq!(
            function.fields[0].callsite.as_deref(),
            Some("NewWorld+0x3495762")
        );

        let handler_vtable = &schema.field_handler_vtables[0];
        assert_eq!(
            handler_vtable.address.as_deref(),
            Some("NewWorld+0x81dad80")
        );
        assert_eq!(handler_vtable.field_count, 1);
        assert_eq!(
            handler_vtable.marshal_target.as_deref(),
            Some("NewWorld+0x17266c0")
        );
        assert_eq!(handler_vtable.wire_shape, Some(NetworkWireShape::U64));
        assert_eq!(
            handler_vtable.wire_shape_source.as_deref(),
            Some("marshal-call:marshal-function-name")
        );
        assert_eq!(handler_vtable.slots[0].name.as_deref(), Some("Marshal"));
        assert_eq!(
            handler_vtable.slots[0].target.as_deref(),
            Some("NewWorld+0x17266c0")
        );
    }

    #[test]
    fn rejects_private_source_derived_ghidra_reports() {
        let report = json!({
            "registryEntries": [],
            "fieldRegistrationFunctions": [{
                "address": "NewWorld+0x3495600",
                "fields": [{
                    "index": 0,
                    "name": "characterId",
                    "wireShape": "entity-ref",
                    "wireShapeSource": "source-replicated-field-handler",
                    "confidence": "high"
                }]
            }],
            "fieldHandlerVtables": []
        });

        let error =
            NetworkSchema::from_ghidra_static_network_report(&report).expect_err("tainted report");

        assert!(matches!(
            error,
            NetworkSchemaImportError::PrivateSourceEvidence
        ));
    }

    #[test]
    fn suppresses_scalar_container_shape_when_full_value_is_structured() {
        let report = json!({
            "registryEntries": [{
                "uuid": "111aebb0-4f23-4914-b732-a349ccbd82d4",
                "typeIndex": 3780,
                "typeName": "Javelin::GlobalMapDataManagerComponentReplicatedState",
                "fields": [{
                    "index": 0,
                    "name": "globalMapData",
                    "handlerVtable": "NewWorld+0x8223838",
                    "wireShape": "replicated-container<u64,u8>",
                    "wireShapeSource": "replicated-container-marshal-calls",
                    "confidence": "register-field-call"
                }]
            }],
            "fieldRegistrationFunctions": [{
                "address": "NewWorld+0x37be1f0",
                "fields": [{
                    "index": 0,
                    "name": "globalMapData",
                    "handlerVtable": "NewWorld+0x8223838",
                    "wireShape": "replicated-container<u64,u8>",
                    "wireShapeSource": "replicated-container-marshal-calls",
                    "confidence": "register-field-call"
                }]
            }],
            "fieldHandlerVtables": [{
                "address": "NewWorld+0x8223838",
                "fieldCount": 1,
                "wireShape": "replicated-container<u64,u8>",
                "wireShapeSource": "replicated-container-marshal-calls",
                "deltaWireShape": "replicated-container<u64,u8>",
                "fullWireShape": "replicated-container<u64,vec2>",
                "deltaMarshalShapes": ["vlq-u32", "u64", "sequence-number", "u8"],
                "fullMarshalShapes": ["sequence-number", "vlq-u32", "u64", "vec2", "u16", "u32"],
                "valueTypeName": "GlobalMapData",
                "valueTypeId": "0DC02DD0-993E-48C0-8B60-5715D4383B0D",
                "valueTypeInfoAddress": "NewWorld+0x82203b0",
                "valueTypeInfoCandidates": [{
                    "address": "NewWorld+0x82203b0",
                    "name": "GlobalMapData",
                    "typeId": "0DC02DD0-993E-48C0-8B60-5715D4383B0D",
                    "source": "native-type-info-layout"
                }, {
                    "address": "NewWorld+0x8123450",
                    "name": "TimePoint",
                    "typeId": "7B883BA2-DFE0-4678-B5FB-C732FD10B7D7",
                    "source": "rtti-provider-vtable"
                }]
            }]
        });

        let schema =
            NetworkSchema::from_ghidra_static_network_report(&report).expect("normalized schema");

        assert_eq!(schema.field_handler_vtables[0].wire_shape, None);
        assert_eq!(schema.field_handler_vtables[0].delta_wire_shape, None);
        assert_eq!(schema.field_handler_vtables[0].full_wire_shape, None);
        assert_eq!(
            schema.field_handler_vtables[0].value_type_name.as_deref(),
            Some("GlobalMapData")
        );
        assert_eq!(
            schema.field_handler_vtables[0].value_type_id.as_deref(),
            Some("0DC02DD0-993E-48C0-8B60-5715D4383B0D")
        );
        assert_eq!(
            schema.field_handler_vtables[0]
                .value_type_info_address
                .as_deref(),
            Some("NewWorld+0x82203b0")
        );
        assert_eq!(
            schema.field_handler_vtables[0].value_type_candidates.len(),
            2
        );
        assert_eq!(
            schema.field_handler_vtables[0].value_type_candidates[0]
                .name
                .as_deref(),
            Some("GlobalMapData")
        );
        assert_eq!(
            schema.field_handler_vtables[0].value_type_candidates[0].type_id,
            Some(uuid!("0dc02dd0-993e-48c0-8b60-5715d4383b0d"))
        );
        assert_eq!(
            schema.field_handler_vtables[0].value_type_candidates[1]
                .source
                .as_deref(),
            Some("rtti-provider-vtable")
        );
        assert_eq!(
            schema.field_handler_vtables[0].delta_marshal_shapes,
            vec![
                "vlq-u32".to_owned(),
                "u64".to_owned(),
                "sequence-number".to_owned(),
                "u8".to_owned()
            ]
        );
        assert_eq!(
            schema.field_handler_vtables[0].full_marshal_shapes,
            vec![
                "sequence-number".to_owned(),
                "vlq-u32".to_owned(),
                "u64".to_owned(),
                "vec2".to_owned(),
                "u16".to_owned(),
                "u32".to_owned()
            ]
        );
        assert_eq!(schema.types[0].fields[0].wire_shape, None);
        assert_eq!(
            schema.field_registration_functions[0].fields[0].wire_shape,
            None
        );
    }

    #[test]
    fn keeps_selected_structured_container_shape_when_delta_value_is_partial() {
        let report = json!({
            "registryEntries": [],
            "fieldRegistrationFunctions": [],
            "fieldHandlerVtables": [{
                "address": "NewWorld+0x8223838",
                "fieldCount": 1,
                "wireShape": "replicated-container<u64,u8>",
                "wireShapeSource": "replicated-container-marshal-calls",
                "deltaMarshalShapes": ["vlq-u32", "u64", "sequence-number", "u8"],
                "fullMarshalShapes": [
                    "sequence-number",
                    "vlq-u32",
                    "u64",
                    "vec2",
                    "u16",
                    "u32"
                ],
                "valueTypeName": "GlobalMapData",
                "valueTypeId": "0DC02DD0-993E-48C0-8B60-5715D4383B0D",
                "slots": []
            }]
        });

        let schema =
            NetworkSchema::from_ghidra_static_network_report(&report).expect("normalized schema");
        let container_shape = schema.field_handler_vtables[0]
            .container_shape
            .as_ref()
            .expect("selected structured value keeps full container shape");

        assert_eq!(
            container_shape.storage,
            NetworkReplicatedContainerStorageKind::Map
        );
        assert_eq!(container_shape.key_wire_shape, NetworkWireScalarShape::U64);
        assert_eq!(
            container_shape.value_wire_shapes,
            vec![
                NetworkWireScalarShape::Vec2,
                NetworkWireScalarShape::U16,
                NetworkWireScalarShape::U32
            ]
        );
        assert_eq!(
            container_shape.delta_value_wire_shapes,
            vec![NetworkWireScalarShape::U8]
        );
        assert_eq!(
            container_shape.source.as_deref(),
            Some("replicated-container-map-full-shape")
        );
        assert_eq!(schema.field_handler_vtables[0].wire_shape, None);
    }

    #[test]
    fn marks_selected_structured_container_shape_full_only_when_delta_is_incomplete() {
        let report = json!({
            "registryEntries": [],
            "fieldRegistrationFunctions": [],
            "fieldHandlerVtables": [{
                "address": "NewWorld+0x8219138",
                "fieldCount": 1,
                "deltaMarshalShapes": ["vlq-u32", "vlq-u32"],
                "fullMarshalShapes": [
                    "sequence-number",
                    "vlq-u32",
                    "u32",
                    "u64",
                    "u64",
                    "u16",
                    "u8"
                ],
                "valueTypeName": "LootLimitData",
                "valueTypeId": "EC6027F0-84B8-46F1-9683-B850C37348EE",
                "slots": []
            }]
        });

        let schema =
            NetworkSchema::from_ghidra_static_network_report(&report).expect("normalized schema");
        let container_shape = schema.field_handler_vtables[0]
            .container_shape
            .as_ref()
            .expect("selected structured value keeps full-only shape");

        assert_eq!(container_shape.key_wire_shape, NetworkWireScalarShape::U32);
        assert_eq!(
            container_shape.value_wire_shapes,
            vec![
                NetworkWireScalarShape::U64,
                NetworkWireScalarShape::U64,
                NetworkWireScalarShape::U16,
                NetworkWireScalarShape::U8
            ]
        );
        assert!(container_shape.delta_value_wire_shapes.is_empty());
        assert_eq!(
            container_shape.source.as_deref(),
            Some("replicated-container-map-full-shape")
        );
    }

    #[test]
    fn infers_vector_container_from_single_full_value_shape() {
        let report = json!({
            "registryEntries": [],
            "fieldRegistrationFunctions": [],
            "fieldHandlerVtables": [{
                "address": "NewWorld+0x803f218",
                "fieldCount": 1,
                "deltaMarshalShapes": ["vlq-u32", "vlq-u32"],
                "fullMarshalShapes": [
                    "sequence-number",
                    "vlq-u32",
                    "string"
                ],
                "slots": []
            }]
        });

        let schema =
            NetworkSchema::from_ghidra_static_network_report(&report).expect("normalized schema");
        let container_shape = schema.field_handler_vtables[0]
            .container_shape
            .as_ref()
            .expect("one full value shape is a vector container");

        assert_eq!(
            container_shape.storage,
            NetworkReplicatedContainerStorageKind::Vec
        );
        assert_eq!(
            container_shape.key_wire_shape,
            NetworkWireScalarShape::VlqU64
        );
        assert_eq!(
            container_shape.value_wire_shapes,
            vec![NetworkWireScalarShape::String]
        );
        assert_eq!(
            container_shape.source.as_deref(),
            Some("replicated-container-vector-full-shape")
        );
    }

    #[test]
    fn infers_vector_container_when_full_shape_includes_overflow_count_branch() {
        let report = json!({
            "registryEntries": [],
            "fieldRegistrationFunctions": [],
            "fieldHandlerVtables": [{
                "address": "NewWorld+0x81b81c0",
                "fieldCount": 1,
                "deltaMarshalShapes": [
                    "vlq-u32",
                    "vlq-u32",
                    "u8",
                    "vlq-u64",
                    "sequence-number",
                    "u8",
                    "u8",
                    "u8",
                    "sequence-number",
                    "vlq-u64",
                    "sequence-number",
                    "vlq-u32"
                ],
                "fullMarshalShapes": [
                    "sequence-number",
                    "vlq-u32",
                    "u32",
                    "vec2",
                    "vlq-u32"
                ],
                "slots": []
            }]
        });

        let schema =
            NetworkSchema::from_ghidra_static_network_report(&report).expect("normalized schema");
        let container_shape = schema.field_handler_vtables[0]
            .container_shape
            .as_ref()
            .expect("full-shape vector container");

        assert_eq!(
            container_shape.storage,
            NetworkReplicatedContainerStorageKind::Vec
        );
        assert_eq!(
            container_shape.value_wire_shapes,
            vec![NetworkWireScalarShape::U32, NetworkWireScalarShape::Vec2]
        );
        assert_eq!(
            container_shape.source.as_deref(),
            Some("replicated-container-vector-shape")
        );
    }

    #[test]
    fn infers_map_value_shape_when_full_shape_includes_overflow_count_branch() {
        let report = json!({
            "registryEntries": [],
            "fieldRegistrationFunctions": [],
            "fieldHandlerVtables": [{
                "address": "NewWorld+0x81c1258",
                "fieldCount": 1,
                "deltaMarshalShapes": [
                    "vlq-u32",
                    "vlq-u32",
                    "u8",
                    "u32",
                    "sequence-number",
                    "u8",
                    "u8",
                    "u8",
                    "sequence-number",
                    "u32",
                    "sequence-number",
                    "vlq-u32"
                ],
                "fullMarshalShapes": [
                    "sequence-number",
                    "vlq-u32",
                    "u32",
                    "u32",
                    "u32",
                    "u32",
                    "vlq-u32"
                ],
                "slots": []
            }]
        });

        let schema =
            NetworkSchema::from_ghidra_static_network_report(&report).expect("normalized schema");
        let container_shape = schema.field_handler_vtables[0]
            .container_shape
            .as_ref()
            .expect("full-shape map container");

        assert_eq!(
            container_shape.storage,
            NetworkReplicatedContainerStorageKind::Map
        );
        assert_eq!(container_shape.key_wire_shape, NetworkWireScalarShape::U32);
        assert_eq!(
            container_shape.value_wire_shapes,
            vec![
                NetworkWireScalarShape::U32,
                NetworkWireScalarShape::U32,
                NetworkWireScalarShape::U32
            ]
        );
        assert_eq!(
            container_shape
                .value_type_shape
                .as_ref()
                .and_then(|shape| shape.validation.as_deref()),
            Some("replicated-container-map-value-shape")
        );
    }

    #[test]
    fn parses_fixed_byte_wire_shapes() {
        let report = json!({
            "registryEntries": [],
            "fieldRegistrationFunctions": [],
            "fieldHandlerVtables": [{
                "address": "NewWorld+0x81b6eb8",
                "fieldCount": 1,
                "wireShape": "fixed-bytes-6",
                "wireShapeSource": "marshal-raw-write-length",
                "slots": []
            }, {
                "address": "NewWorld+0x80b9830",
                "fieldCount": 1,
                "wireShape": "fixed-bytes-16",
                "wireShapeSource": "marshal-raw-write-length",
                "slots": []
            }]
        });

        let schema =
            NetworkSchema::from_ghidra_static_network_report(&report).expect("normalized schema");

        assert_eq!(
            schema.field_handler_vtables[0].wire_shape,
            Some(NetworkWireShape::FixedBytes(6))
        );
        assert_eq!(
            schema.field_handler_vtables[1].wire_shape,
            Some(NetworkWireShape::FixedBytes(16))
        );
    }

    #[test]
    fn parses_container_wire_shapes() {
        let report = json!({
            "registryEntries": [],
            "fieldRegistrationFunctions": [],
            "fieldHandlerVtables": [{
                "address": "NewWorld+0x81b6eb8",
                "fieldCount": 1,
                "wireShape": "replicated-container<u32,vlq-u64>",
                "wireShapeSource": "replicated-container-marshal-calls",
                "slots": []
            }, {
                "address": "NewWorld+0x81b6ec0",
                "fieldCount": 1,
                "wireShape": "sequence-number",
                "wireShapeSource": "marshal-call:sequence-number",
                "slots": []
            }, {
                "address": "NewWorld+0x81b6ec8",
                "fieldCount": 1,
                "wireShape": "vlq-u64",
                "wireShapeSource": "marshal-call:vlq-u64",
                "slots": []
            }]
        });

        let schema =
            NetworkSchema::from_ghidra_static_network_report(&report).expect("normalized schema");

        assert_eq!(
            schema.field_handler_vtables[0].wire_shape,
            Some(NetworkWireShape::ReplicatedContainer(
                NetworkReplicatedContainerWireShape {
                    key: NetworkWireScalarShape::U32,
                    value: NetworkWireScalarShape::VlqU64,
                }
            ))
        );
        assert_eq!(
            schema.field_handler_vtables[1].wire_shape,
            Some(NetworkWireShape::SequenceNumber)
        );
        assert_eq!(
            schema.field_handler_vtables[2].wire_shape,
            Some(NetworkWireShape::VlqU64)
        );

        assert_eq!(
            serde_json::to_value(schema.field_handler_vtables[0].wire_shape.unwrap()).unwrap(),
            json!("replicated-container<u32,vlq-u64>")
        );
    }

    #[test]
    fn ignores_raw_byte_lengths_that_conflict_with_wire_shape() {
        let conflict = json!({
            "index": 0,
            "name": "field_0",
            "rawByteLength": 16,
            "wireShape": "u64",
            "wireShapeSource": "message-unmarshal-helper-nested-call",
            "confidence": "message-unmarshal-helper-argument"
        });
        let field = network_field(conflict.as_object().expect("field object"));

        assert_eq!(field.raw_byte_length, None);
        assert_eq!(field.native_type, None);
        assert_eq!(field.wire_shape, None);

        let fixed = json!({
            "index": 0,
            "name": "field_0",
            "rawByteLength": 16,
            "wireShape": "fixed-bytes-16",
            "wireShapeSource": "message-unmarshal-read-raw",
            "confidence": "message-unmarshal-read-raw"
        });
        let field = network_field(fixed.as_object().expect("field object"));

        assert_eq!(field.raw_byte_length, Some(16));
        assert_eq!(field.wire_shape, Some(NetworkWireShape::FixedBytes(16)));
    }

    #[test]
    fn assigns_direct_message_and_support_data_capabilities() {
        let report = json!({
            "registryEntries": [
                {
                    "uuid": "E3578B38-69AD-4C13-A7DD-3FFF752D98AA",
                    "typeName": "ClientActorRoutingAuthorizationTrait::ClientAddEntryMsg"
                },
                {
                    "uuid": "5566F141-5C23-4BFB-BEFF-372DAF60F713",
                    "typeName": "Javelin::ContractActionParamsSellCompletion"
                }
            ],
            "fieldRegistrationFunctions": []
        });

        let schema =
            NetworkSchema::from_ghidra_static_network_report(&report).expect("normalized schema");

        assert_eq!(
            schema.types[0].capabilities,
            vec![NetworkTypeCapability::DirectMessage]
        );
        assert_eq!(
            schema.types[1].capabilities,
            vec![NetworkTypeCapability::SupportData]
        );
    }

    #[test]
    fn replicated_state_capability_requires_state_leaf_name() {
        let report = json!({
            "registryEntries": [
                {
                    "uuid": "11111111-1111-4111-9111-111111111111",
                    "typeName": "Javelin::GameModeReplicatedState"
                },
                {
                    "uuid": "22222222-2222-4222-9222-222222222222",
                    "typeName": "Javelin::ClientMessages::ObjectiveInteractorComponentServerFacet_DEBUG_RequestForceUpdateReplicatedState"
                },
                {
                    "uuid": "33333333-3333-4333-9333-333333333333",
                    "typeName": "MB::ReplicatedState"
                },
                {
                    "uuid": "44444444-4444-4444-9444-444444444444",
                    "typeName": "Amazon::Hub::ReplicatedStateBundle"
                },
                {
                    "uuid": "55555555-5555-4555-9555-555555555555",
                    "typeName": "MB::SocialReplicatedState::ChattingStateMessageType"
                }
            ],
            "fieldRegistrationFunctions": []
        });

        let schema =
            NetworkSchema::from_ghidra_static_network_report(&report).expect("normalized schema");

        assert_eq!(
            schema.types[0].capabilities,
            vec![NetworkTypeCapability::ReplicatedState]
        );
        assert_eq!(
            schema.types[1].capabilities,
            vec![NetworkTypeCapability::DirectMessage]
        );
        assert_eq!(
            schema.types[2].capabilities,
            vec![NetworkTypeCapability::SupportData]
        );
        assert_eq!(
            schema.types[3].capabilities,
            vec![NetworkTypeCapability::SupportData]
        );
        assert_eq!(
            schema.types[4].capabilities,
            vec![NetworkTypeCapability::SupportData]
        );
    }

    #[test]
    fn imports_message_unmarshal_fields_without_registered_fields_capability() {
        let report = json!({
            "registryEntries": [{
                "uuid": "0B826B33-89F5-49E0-B8CB-FE4433427778",
                "typeIndex": 19,
                "typeName": "RegistrationRequestV3Msg",
                "messageUnmarshal": {
                    "wrapper": "NewWorld+0x7ce8e0",
                    "helperCallsite": "NewWorld+0x7ce955",
                    "helper": "NewWorld+0x7d9620",
                    "helperName": "Amazon::REP::REPClient::RegistrationRequestV3Msg::UnmarshalFields<ClientVersionTokenMap,LoginToken,AuthToken,ImpersonatedValues>",
                    "createInstance": "NewWorld+0x7ce840",
                    "instanceSize": "0x470",
                    "instanceSizeSource": "create-instance-operator-new",
                    "instanceConstructorCallsite": "NewWorld+0x7ce8fc",
                    "instanceConstructor": "NewWorld+0x7e37d0",
                    "instanceConstructorName": "Amazon::REP::REPClient::RegistrationRequestV3::RegistrationRequestV3",
                    "templateTypes": [
                        "ClientVersionTokenMap",
                        "LoginToken",
                        "AuthToken",
                        "ImpersonatedValues"
                    ]
                },
                "fields": [{
                    "index": 0,
                    "callsite": "NewWorld+0x7ce955",
                    "name": "TypeIndexCrc",
                    "nameSource": "msvc-rtti-source-signature",
                    "nameSourceAddress": "NewWorld+0xa268e80",
                    "sourceTypeName": "AZ::Crc32",
                    "nativeType": "u32",
                    "storageExpression": "(plVar1 + 1)",
                    "storageOffset": "0x8",
                    "wireShape": "u32",
                    "wireShapeSource": "message-unmarshal-native-type",
                    "unmarshalEvidence": {
                        "callsite": "NewWorld+0x7ce955",
                        "targetName": "GridMate::Marshaler<AZ::Crc32>::Unmarshal",
                        "targetKind": "field-helper",
                        "evidenceSource": "message-unmarshal-pcode-call"
                    },
                    "confidence": "message-unmarshal-call"
                }, {
                    "index": 2,
                    "callsite": "NewWorld+0x7ce955",
                    "name": "ConnTicket",
                    "nativeType": "AZStd::string",
                    "storageExpression": "(plVar1 + 0x14)",
                    "storageOffset": "0xa0",
                    "wireShape": "string",
                    "wireShapeSource": "message-unmarshal-native-type",
                    "confidence": "message-unmarshal-call"
                }, {
                    "index": 6,
                    "callsite": "NewWorld+0x7ce955",
                    "name": "UseCapabilities",
                    "nameSource": "msvc-rtti-source-signature",
                    "nameSourceAddress": "NewWorld+0xa268e80",
                    "sourceTypeName": "bool",
                    "nativeType": "bool",
                    "storageExpression": "plVar1 + 0x8c",
                    "storageOffset": "0x460",
                    "wireShape": "bool",
                    "wireShapeSource": "nested-unmarshal-bool-write",
                    "confidence": "message-unmarshal-call"
                }]
            }],
            "fieldRegistrationFunctions": []
        });

        let schema =
            NetworkSchema::from_ghidra_static_network_report(&report).expect("normalized schema");

        assert_eq!(schema.summary.message_unmarshal_field_count, 3);
        assert_eq!(
            schema.types[0].capabilities,
            vec![NetworkTypeCapability::DirectMessage]
        );
        let instance = schema.types[0].instance.as_ref().expect("instance layout");
        assert_eq!(instance.size, Some(0x470));
        assert_eq!(instance.constructor.as_deref(), Some("NewWorld+0x7e37d0"));
        assert_eq!(
            schema.types[0].fields[0].native_type.as_deref(),
            Some("u32")
        );
        assert_eq!(
            schema.types[0].fields[0].source_type_name.as_deref(),
            Some("AZ::Crc32")
        );
        assert_eq!(
            schema.types[0]
                .fields
                .iter()
                .map(|field| field.index)
                .collect::<Vec<_>>(),
            vec![Some(0), Some(1), Some(2)]
        );
        assert_eq!(
            schema.types[0].fields[0].name.as_deref(),
            Some("TypeIndexCrc")
        );
        assert_eq!(schema.types[0].fields[0].storage_offset, Some(0x8));
        assert_eq!(
            schema.types[0].fields[0].wire_shape,
            Some(NetworkWireShape::U32)
        );
        let unmarshal_evidence = schema.types[0].fields[0]
            .unmarshal_evidence
            .as_ref()
            .expect("unmarshal evidence");
        assert_eq!(
            unmarshal_evidence.target_name.as_deref(),
            Some("GridMate::Marshaler<AZ::Crc32>::Unmarshal")
        );
        assert_eq!(
            unmarshal_evidence.evidence_source.as_deref(),
            Some("message-unmarshal-pcode-call")
        );
        assert_eq!(
            schema.types[0].fields[0].evidence[0].kind,
            NetworkEvidenceKind::MessageUnmarshal
        );
        assert_eq!(
            schema.types[0].fields[0].evidence[1].kind,
            NetworkEvidenceKind::MessageSource
        );
        assert_eq!(
            schema.types[0].fields[0].evidence[1].detail.as_deref(),
            Some("AZ::Crc32")
        );
        assert_eq!(
            schema.types[0].fields[1].wire_shape,
            Some(NetworkWireShape::String)
        );
        assert_eq!(schema.types[0].fields[2].storage_offset, Some(0x460));
        assert_eq!(
            schema.types[0].fields[2].wire_shape,
            Some(NetworkWireShape::Bool)
        );
    }

    #[test]
    fn filters_implausible_message_unmarshal_storage_and_reindexes_remaining_fields() {
        let report = json!({
            "registryEntries": [{
                "uuid": "0B826B33-89F5-49E0-B8CB-FE4433427778",
                "typeIndex": 19,
                "typeName": "RegistrationRequestV3Msg",
                "fields": [{
                    "index": 0,
                    "name": "param_4",
                    "storageExpression": "param_4",
                    "confidence": "message-unmarshal-helper-argument"
                }, {
                    "index": 5,
                    "name": "UseCapabilities",
                    "nativeType": "bool",
                    "storageExpression": "param_3 + 0x8c",
                    "wireShape": "bool",
                    "confidence": "message-unmarshal-helper-argument"
                }]
            }],
            "fieldRegistrationFunctions": []
        });

        let schema =
            NetworkSchema::from_ghidra_static_network_report(&report).expect("normalized schema");

        assert_eq!(schema.types[0].fields.len(), 1);
        assert_eq!(schema.types[0].fields[0].index, Some(0));
        assert_eq!(
            schema.types[0].fields[0].name.as_deref(),
            Some("UseCapabilities")
        );
    }

    #[test]
    fn merges_message_signature_field_names_without_overwriting_real_names() {
        let report = json!({
            "registryEntries": [{
                "uuid": "6A379FB8-8E18-4D62-89A1-9A891DC98CAD",
                "typeIndex": 349,
                "typeName": "REPClient::PingMsg",
                "fields": [{
                    "index": 0,
                    "name": "field_0",
                    "storageExpression": "param_3 + 1",
                    "wireShape": "u64",
                    "confidence": "message-unmarshal-call"
                }]
            }],
            "fieldRegistrationFunctions": []
        });
        let mut schema = NetworkSchema::from_ghidra_static_network_report(&report).expect("schema");

        let merge = schema.merge_message_signatures(
            &[NetworkMessageSignature {
                type_id: Some(uuid!("6a379fb8-8e18-4d62-89a1-9a891dc98cad")),
                type_index: Some(349),
                name: Some("REPClient::PingMsg".to_owned()),
                rust_name: Some("PingMsg".to_owned()),
                source: None,
                fields: vec![NetworkMessageFieldSignature {
                    index: Some(0),
                    name: "epoch_time_send".to_owned(),
                    rust_type: Some("u64".to_owned()),
                    native_type: Some("u64".to_owned()),
                    wire_shape: Some(NetworkWireShape::U64),
                }],
            }],
            Some("rust-source".to_owned()),
        );

        assert_eq!(merge.matched_message_count, 1);
        assert_eq!(merge.field_name_filled_count, 1);
        assert_eq!(merge.field_name_conflict_count, 0);
        assert_eq!(schema.summary.message_source_field_count, 1);
        let field = &schema.types[0].fields[0];
        assert_eq!(field.name.as_deref(), Some("epoch_time_send"));
        assert_eq!(field.native_type.as_deref(), Some("u64"));
        assert_eq!(field.wire_shape, Some(NetworkWireShape::U64));
    }

    #[test]
    fn merges_message_signature_field_names_over_native_type_names() {
        let report = json!({
            "registryEntries": [{
                "uuid": "96A58E69-7BD5-45C5-86E4-DAF9F5EB1E86",
                "typeIndex": 397,
                "typeName": "Replicate::RegisterFragmentAccessMsg",
                "fields": [{
                    "index": 0,
                    "name": "ProxyAddress",
                    "nameSource": "message-native-type-name",
                    "nativeType": "ProxyAddress",
                    "confidence": "message-unmarshal-helper-direct-type-call"
                }, {
                    "index": 1,
                    "name": "field_1",
                    "nativeType": "u32",
                    "wireShape": "u32",
                    "confidence": "message-unmarshal-helper-nested-call"
                }]
            }],
            "fieldRegistrationFunctions": []
        });
        let mut schema = NetworkSchema::from_ghidra_static_network_report(&report).expect("schema");

        let merge = schema.merge_message_signatures(
            &[NetworkMessageSignature {
                type_id: Some(uuid!("96a58e69-7bd5-45c5-86e4-daf9f5eb1e86")),
                type_index: Some(397),
                name: Some("Replicate::RegisterFragmentAccessMsg".to_owned()),
                rust_name: Some("RegisterFragmentAccessMsg".to_owned()),
                source: None,
                fields: vec![
                    NetworkMessageFieldSignature {
                        index: Some(0),
                        name: "ProxyRef".to_owned(),
                        rust_type: None,
                        native_type: Some("ActorRef".to_owned()),
                        wire_shape: None,
                    },
                    NetworkMessageFieldSignature {
                        index: Some(1),
                        name: "Key".to_owned(),
                        rust_type: None,
                        native_type: Some("FragmentKey".to_owned()),
                        wire_shape: None,
                    },
                ],
            }],
            Some("message-signatures.json".to_owned()),
        );

        assert_eq!(merge.matched_message_count, 1);
        assert_eq!(merge.field_name_filled_count, 2);
        assert_eq!(merge.field_name_conflict_count, 0);
        assert_eq!(schema.types[0].fields[0].name.as_deref(), Some("ProxyRef"));
        assert_eq!(schema.types[0].fields[1].name.as_deref(), Some("Key"));
        assert_eq!(
            schema.types[0].fields[0].native_type.as_deref(),
            Some("ActorRef")
        );
        assert_eq!(
            schema.types[0].fields[1].native_type.as_deref(),
            Some("FragmentKey")
        );
    }

    #[test]
    fn message_signatures_replace_partial_ghidra_fragment_message_fields() {
        let report = json!({
            "registryEntries": [{
                "uuid": "96A58E69-7BD5-45C5-86E4-DAF9F5EB1E86",
                "typeIndex": 397,
                "typeName": "Replicate::RegisterFragmentAccessMsg",
                "fields": [{
                    "index": 0,
                    "name": "field_0",
                    "nativeType": "u32",
                    "storageExpression": "param_3 + 1",
                    "wireShape": "u32",
                    "confidence": "message-unmarshal-helper-wrapper"
                }]
            }, {
                "uuid": "2B7640E0-4204-4E52-998A-C2DB02E0A480",
                "typeIndex": 399,
                "typeName": "Replicate::UnregisterFragmentAccessMsg",
                "fields": [{
                    "index": 0,
                    "name": "field_0",
                    "nativeType": "u32",
                    "storageExpression": "param_3 + 1",
                    "wireShape": "u32",
                    "confidence": "message-unmarshal-helper-wrapper"
                }]
            }, {
                "uuid": "951EF3ED-C9A0-4E3D-A6FD-7FE0673D28D2",
                "typeIndex": 422,
                "typeName": "ReplicateClient::FragmentUpdateMsg",
                "fields": [{
                    "index": 0,
                    "name": "ProxyAddress",
                    "nameSource": "message-native-type-name",
                    "nativeType": "ProxyAddress",
                    "confidence": "message-unmarshal-inline-direct-type-call"
                }, {
                    "index": 1,
                    "name": "field_1",
                    "nativeType": "u32",
                    "wireShape": "u32",
                    "confidence": "message-unmarshal-inline-call"
                }]
            }],
            "fieldRegistrationFunctions": []
        });
        let mut schema = NetworkSchema::from_ghidra_static_network_report(&report).expect("schema");

        let merge = schema.merge_message_signatures(
            &fragment_access_message_signatures(),
            Some("message-signatures.json".to_owned()),
        );

        assert_eq!(merge.matched_message_count, 3);
        assert_eq!(merge.field_count_mismatch_count, 3);
        assert_eq!(schema.types[0].fields.len(), 2);
        assert_eq!(schema.types[1].fields.len(), 2);
        assert_eq!(schema.types[2].fields.len(), 3);
        assert_fragment_access_fields(&schema.types[0].fields);
        assert_fragment_access_fields(&schema.types[1].fields);
        assert_eq!(schema.types[2].fields[0].name.as_deref(), Some("TargetRef"));
        assert_eq!(
            schema.types[2].fields[0].native_type.as_deref(),
            Some("ActorRef")
        );
        assert_eq!(schema.types[2].fields[1].name.as_deref(), Some("Key"));
        assert_eq!(
            schema.types[2].fields[1].native_type.as_deref(),
            Some("FragmentKey")
        );
        assert_eq!(schema.types[2].fields[2].name.as_deref(), Some("Fragment"));
        assert_eq!(
            schema.types[2].fields[2].native_type.as_deref(),
            Some("BaselineableFragment")
        );
    }

    #[test]
    fn merges_message_signature_fields_when_static_report_has_none() {
        let report = json!({
            "registryEntries": [{
                "uuid": "AAAAAAAA-AAAA-AAAA-AAAA-AAAAAAAAAAAA",
                "typeIndex": 77,
                "typeName": "ExampleMsg",
                "fields": []
            }],
            "fieldRegistrationFunctions": []
        });
        let mut schema = NetworkSchema::from_ghidra_static_network_report(&report).expect("schema");

        let merge = schema.merge_message_signatures(
            &[NetworkMessageSignature {
                type_id: Some(uuid!("aaaaaaaa-aaaa-aaaa-aaaa-aaaaaaaaaaaa")),
                type_index: Some(77),
                name: Some("ExampleMsg".to_owned()),
                rust_name: Some("ExampleMsg".to_owned()),
                source: None,
                fields: vec![NetworkMessageFieldSignature {
                    index: Some(0),
                    name: "Payload".to_owned(),
                    rust_type: Some("::nw_network::Payload".to_owned()),
                    native_type: Some("Payload".to_owned()),
                    wire_shape: None,
                }],
            }],
            Some("message-signatures.json".to_owned()),
        );

        assert_eq!(merge.matched_message_count, 1);
        assert_eq!(merge.field_name_filled_count, 1);
        assert_eq!(merge.native_type_filled_count, 1);
        assert_eq!(merge.wire_shape_filled_count, 0);
        assert_eq!(schema.types[0].fields.len(), 1);
        let field = &schema.types[0].fields[0];
        assert_eq!(field.name.as_deref(), Some("Payload"));
        assert_eq!(field.rust_type.as_deref(), Some("::nw_network::Payload"));
        assert_eq!(field.confidence, NetworkConfidence::High);
        assert_eq!(field.evidence[0].kind, NetworkEvidenceKind::MessageSource);
    }

    #[test]
    fn merges_field_overrides_with_source_style_container_types() {
        let report = json!({
            "registryEntries": [{
                "uuid": "BBBBBBBB-BBBB-BBBB-BBBB-BBBBBBBBBBBB",
                "typeIndex": 3362,
                "typeName": "Javelin::SlayerScriptReplicatedState",
                "fields": [{
                    "index": 3,
                    "name": "spawnedEntityIdsBySpawnerId",
                    "nativeType": "MB::ReplicatedMapFieldHandler<AZ::Crc32, AZ::EntityId>",
                    "wireShape": "replicated-container<u32,u64>",
                    "confidence": "register-field-call"
                }]
            }],
            "fieldRegistrationFunctions": []
        });
        let mut schema = NetworkSchema::from_ghidra_static_network_report(&report).expect("schema");
        let overrides = NetworkFieldOverrideFile {
            fields: vec![NetworkFieldOverride {
                type_id: None,
                type_index: Some(3362),
                type_name: None,
                field_index: Some(3),
                field: Some("spawnedEntityIdsBySpawnerId".to_owned()),
                native_type: Some("MB::ReplicatedMapFieldHandler<AZ::Crc32, AZ::EntityId>".to_owned()),
                rust_type: Some("::nw_network::serialize::ReplicatedContainer<::nw_network::serialize::IndexMap<::nw_network::Crc32, ::nw_network::EntityId>, { ::nw_network::serialize::WIRE_VEC_CAP }, ::nw_network::serialize::DefaultMarshaler<::nw_network::Crc32>, ::nw_network::serialize::DefaultMarshaler<::nw_network::EntityId>>".to_owned()),
                wire_shape: Some(NetworkWireShape::ReplicatedContainer(
                    NetworkReplicatedContainerWireShape {
                        key: NetworkWireScalarShape::U32,
                        value: NetworkWireScalarShape::U64,
                    },
                )),
                wire_shape_source: Some("field-overrides".to_owned()),
                confidence: Some(NetworkConfidence::High),
            }],
        };

        let merge = schema
            .merge_field_overrides(&overrides, Some("network-field-overrides.json".to_owned()));

        assert_eq!(merge.source_field_count, 1);
        assert_eq!(merge.matched_field_count, 1);
        assert_eq!(merge.unmatched_type_count, 0);
        assert_eq!(merge.unmatched_field_count, 0);
        assert_eq!(merge.rust_type_updated_count, 1);
        assert_eq!(merge.wire_shape_updated_count, 1);
        assert!(schema.sources.iter().any(|source| {
            source.kind == NetworkSchemaSourceKind::FieldOverrides
                && source.path.as_deref() == Some("network-field-overrides.json")
        }));
        let field = &schema.types[0].fields[0];
        assert!(
            field
                .rust_type
                .as_deref()
                .is_some_and(|rust_type| rust_type.contains("ReplicatedContainer<"))
        );
        assert!(
            field
                .rust_type
                .as_deref()
                .is_some_and(|rust_type| rust_type.contains("IndexMap<"))
        );
        assert_eq!(
            field.evidence.last().map(|evidence| evidence.kind),
            Some(NetworkEvidenceKind::FieldOverride)
        );
    }

    #[test]
    fn merges_typeindex_without_overwriting_conflicts() {
        let report = json!({
            "registryEntries": [
                {
                    "uuid": "8673A3CC-2848-4C87-AA72-CC860589D1B5",
                    "typeName": "ExampleFilled"
                },
                {
                    "uuid": "DA4E5889-A65C-4480-8642-0278160125A7",
                    "typeName": "ExampleConflict",
                    "typeIndex": 9
                }
            ],
            "fieldRegistrationFunctions": []
        });
        let typeindex = json!({
            "typeIndex": [
                "00000000000000000000000000000000",
                "8673A3CC28484C87AA72CC860589D1B5",
                "DA4E5889A65C448086420278160125A7"
            ]
        });

        let mut schema =
            NetworkSchema::from_ghidra_static_network_report(&report).expect("normalized schema");
        let merge = schema
            .merge_typeindex_root(&typeindex, Some("typeindex.json".to_owned()))
            .expect("typeindex merge");

        assert_eq!(merge.source_type_count, 3);
        assert_eq!(merge.matched_type_count, 2);
        assert_eq!(merge.filled_type_index_count, 1);
        assert_eq!(merge.conflicting_type_index_count, 1);
        assert_eq!(schema.types[0].type_index, Some(1));
        assert_eq!(schema.types[1].type_index, Some(9));
        assert_eq!(schema.summary.type_index_evidence_count, 2);
        assert!(schema.sources.iter().any(|source| {
            source.kind == NetworkSchemaSourceKind::TypeIndex
                && source.path.as_deref() == Some("typeindex.json")
        }));
        assert_eq!(
            schema.types[1]
                .evidence
                .last()
                .map(|evidence| evidence.confidence),
            Some(NetworkConfidence::Weak)
        );
    }

    #[test]
    fn merges_serialize_codegen_evidence_and_dependencies() {
        let root_type_id = uuid!("8673a3cc-2848-4c87-aa72-cc860589d1b5");
        let dependency_type_id = uuid!("da4e5889-a65c-4480-8642-0278160125a7");
        let report = json!({
            "registryEntries": [{
                "uuid": root_type_id.to_string(),
                "typeName": "NetworkName"
            }],
            "fieldRegistrationFunctions": []
        });
        let unit = SerializeCodegenUnit {
            items: vec![SerializeCodegenItem {
                source_type_id: root_type_id,
                source_name: "SerializeName".to_owned(),
                role: ReflectedTypeRole::SupportType,
                is_reflection_marker: false,
                is_abstract: Some(false),
                factory: None,
                rtti_base_chain: vec![SerializeCodegenRttiBase {
                    type_id: dependency_type_id,
                    source_name: "Dependency".to_owned(),
                }],
                kind: SerializeCodegenItemKind::Struct,
                enum_underlying_type: None,
                fields: Vec::new(),
                variants: Vec::new(),
            }],
        };

        let mut schema =
            NetworkSchema::from_ghidra_static_network_report(&report).expect("normalized schema");
        let merge = schema.merge_serialize_codegen_unit(&unit, Some("serialize.json".to_owned()));

        assert_eq!(merge.source_type_count, 1);
        assert_eq!(merge.matched_type_count, 1);
        assert_eq!(merge.type_id_matched_count, 1);
        assert_eq!(merge.name_matched_count, 0);
        assert_eq!(merge.filled_name_count, 0);
        assert_eq!(schema.summary.serialize_type_count, 1);
        assert_eq!(schema.summary.serialize_dependency_count, 1);
        let serialize = schema.types[0].serialize.as_ref().expect("serialize merge");
        assert_eq!(serialize.name, "SerializeName");
        assert_eq!(serialize.kind, NetworkSerializeKind::Struct);
        assert_eq!(serialize.role, NetworkSerializeRole::SupportType);
        assert_eq!(
            serialize.direct_dependency_type_ids,
            vec![dependency_type_id]
        );
        assert!(
            schema.types[0]
                .evidence
                .iter()
                .any(|evidence| evidence.kind == NetworkEvidenceKind::SerializeContext)
        );
    }

    #[test]
    fn merges_serialize_codegen_by_unique_source_name_with_inferred_confidence() {
        let network_type_id = uuid!("8673a3cc-2848-4c87-aa72-cc860589d1b5");
        let serialize_type_id = uuid!("da4e5889-a65c-4480-8642-0278160125a7");
        let report = json!({
            "registryEntries": [{
                "uuid": network_type_id.to_string(),
                "typeName": "Example::SharedName"
            }],
            "fieldRegistrationFunctions": []
        });
        let unit = SerializeCodegenUnit {
            items: vec![SerializeCodegenItem {
                source_type_id: serialize_type_id,
                source_name: "Example::SharedName".to_owned(),
                role: ReflectedTypeRole::SupportType,
                is_reflection_marker: false,
                is_abstract: Some(false),
                factory: None,
                rtti_base_chain: Vec::new(),
                kind: SerializeCodegenItemKind::Struct,
                enum_underlying_type: None,
                fields: Vec::new(),
                variants: Vec::new(),
            }],
        };

        let mut schema =
            NetworkSchema::from_ghidra_static_network_report(&report).expect("normalized schema");
        let merge = schema.merge_serialize_codegen_unit(&unit, Some("serialize.json".to_owned()));

        assert_eq!(merge.matched_type_count, 1);
        assert_eq!(merge.type_id_matched_count, 0);
        assert_eq!(merge.name_matched_count, 1);
        assert_eq!(merge.ambiguous_name_match_count, 0);
        assert_eq!(schema.summary.serialize_type_count, 1);
        let evidence = schema.types[0]
            .evidence
            .iter()
            .find(|evidence| evidence.kind == NetworkEvidenceKind::SerializeContext)
            .expect("serialize evidence");
        assert_eq!(evidence.source, "serializeContext:name");
        assert_eq!(evidence.confidence, NetworkConfidence::Inferred);
    }

    #[test]
    fn merges_field_serialize_type_by_nested_type_id() {
        let network_type_id = uuid!("8673a3cc-2848-4c87-aa72-cc860589d1b5");
        let payload_type_id = uuid!("da4e5889-a65c-4480-8642-0278160125a7");
        let report = json!({
            "registryEntries": [{
                "uuid": network_type_id.to_string(),
                "typeName": "Example::PayloadMessage",
                "fields": [{
                    "index": 0,
                    "name": "payload",
                    "nativeType": "PayloadData",
                    "sourceTypeId": payload_type_id.to_string(),
                    "confidence": "message-unmarshal-direct-type",
                    "storageExpression": "param_3 + 0x8",
                    "nestedTypeShape": {
                        "typeId": payload_type_id.to_string(),
                        "typeIdSource": "serialize-context-name",
                        "typeName": "PayloadData",
                        "typeNameFull": "Example::PayloadData",
                        "factory": "NewWorld+0x1234",
                        "members": []
                    }
                }]
            }],
            "fieldRegistrationFunctions": []
        });
        let unit = SerializeCodegenUnit {
            items: vec![SerializeCodegenItem {
                source_type_id: payload_type_id,
                source_name: "Example::PayloadData".to_owned(),
                role: ReflectedTypeRole::SupportType,
                is_reflection_marker: false,
                is_abstract: Some(false),
                factory: Some("NewWorld+0x1234".to_owned()),
                rtti_base_chain: Vec::new(),
                kind: SerializeCodegenItemKind::Struct,
                enum_underlying_type: None,
                fields: Vec::new(),
                variants: Vec::new(),
            }],
        };

        let mut schema =
            NetworkSchema::from_ghidra_static_network_report(&report).expect("normalized schema");
        let merge = schema.merge_serialize_codegen_unit(&unit, Some("serialize.json".to_owned()));

        assert_eq!(merge.matched_type_count, 0);
        assert_eq!(merge.matched_field_type_count, 1);
        assert_eq!(merge.field_type_id_matched_count, 1);
        assert_eq!(schema.summary.serialize_type_count, 0);
        assert_eq!(schema.summary.serialize_field_type_count, 1);
        let field = &schema.types[0].fields[0];
        let serialize = field.serialize.as_ref().expect("field serialize type");
        assert_eq!(serialize.type_id, payload_type_id);
        assert_eq!(serialize.name, "Example::PayloadData");
        assert_eq!(serialize.source, "serializeContext:field-type-id");
        assert_eq!(serialize.confidence, NetworkConfidence::Exact);
    }

    #[test]
    fn merges_field_serialize_type_by_selected_handler_value_type_id() {
        let network_type_id = uuid!("8673a3cc-2848-4c87-aa72-cc860589d1b5");
        let payload_type_id = uuid!("da4e5889-a65c-4480-8642-0278160125a7");
        let report = json!({
            "registryEntries": [{
                "uuid": network_type_id.to_string(),
                "typeName": "Example::ReplicatedState",
                "fields": [{
                    "index": 0,
                    "name": "payloads",
                    "handlerVtable": "NewWorld+0x8123450",
                    "confidence": "register-field-call"
                }]
            }],
            "fieldRegistrationFunctions": [],
            "fieldHandlerVtables": [{
                "address": "NewWorld+0x8123450",
                "fieldCount": 1,
                "valueTypeName": "PayloadData",
                "valueTypeId": payload_type_id.to_string(),
                "valueTypeInfoAddress": "NewWorld+0x8234560",
                "valueTypeInfoCandidates": [{
                    "address": "NewWorld+0x8345670",
                    "name": "NestedMember",
                    "typeId": "11111111-1111-1111-1111-111111111111",
                    "source": "rtti-provider-vtable",
                    "nameSource": "rtti-helper-function-name"
                }]
            }]
        });
        let unit = SerializeCodegenUnit {
            items: vec![SerializeCodegenItem {
                source_type_id: payload_type_id,
                source_name: "PayloadData".to_owned(),
                role: ReflectedTypeRole::SupportType,
                is_reflection_marker: false,
                is_abstract: Some(false),
                factory: None,
                rtti_base_chain: Vec::new(),
                kind: SerializeCodegenItemKind::Struct,
                enum_underlying_type: None,
                fields: Vec::new(),
                variants: Vec::new(),
            }],
        };

        let mut schema =
            NetworkSchema::from_ghidra_static_network_report(&report).expect("normalized schema");
        let merge = schema.merge_serialize_codegen_unit(&unit, Some("serialize.json".to_owned()));

        assert_eq!(merge.matched_type_count, 0);
        assert_eq!(merge.matched_field_type_count, 1);
        assert_eq!(merge.field_type_id_matched_count, 1);
        assert_eq!(schema.summary.serialize_type_count, 0);
        assert_eq!(schema.summary.serialize_field_type_count, 1);
        let field = &schema.types[0].fields[0];
        let serialize = field.serialize.as_ref().expect("field serialize type");
        assert_eq!(serialize.type_id, payload_type_id);
        assert_eq!(serialize.name, "PayloadData");
        assert_eq!(serialize.source, "serializeContext:handler-value-type-id");
        assert_eq!(serialize.confidence, NetworkConfidence::High);
    }

    #[test]
    fn does_not_merge_field_serialize_type_from_provider_candidate_only() {
        let network_type_id = uuid!("8673a3cc-2848-4c87-aa72-cc860589d1b5");
        let payload_type_id = uuid!("da4e5889-a65c-4480-8642-0278160125a7");
        let report = json!({
            "registryEntries": [{
                "uuid": network_type_id.to_string(),
                "typeName": "Example::ReplicatedState",
                "fields": [{
                    "index": 0,
                    "name": "payloads",
                    "handlerVtable": "NewWorld+0x8123450",
                    "confidence": "register-field-call"
                }]
            }],
            "fieldRegistrationFunctions": [],
            "fieldHandlerVtables": [{
                "address": "NewWorld+0x8123450",
                "fieldCount": 1,
                "valueTypeInfoCandidates": [{
                    "address": "NewWorld+0x8234560",
                    "name": "PayloadData",
                    "typeId": payload_type_id.to_string(),
                    "source": "rtti-provider-vtable",
                    "nameSource": "rtti-helper-function-name"
                }]
            }]
        });
        let unit = SerializeCodegenUnit {
            items: vec![SerializeCodegenItem {
                source_type_id: payload_type_id,
                source_name: "PayloadData".to_owned(),
                role: ReflectedTypeRole::SupportType,
                is_reflection_marker: false,
                is_abstract: Some(false),
                factory: None,
                rtti_base_chain: Vec::new(),
                kind: SerializeCodegenItemKind::Struct,
                enum_underlying_type: None,
                fields: Vec::new(),
                variants: Vec::new(),
            }],
        };

        let mut schema =
            NetworkSchema::from_ghidra_static_network_report(&report).expect("normalized schema");
        let merge = schema.merge_serialize_codegen_unit(&unit, Some("serialize.json".to_owned()));

        assert_eq!(merge.matched_field_type_count, 0);
        assert_eq!(merge.field_type_id_matched_count, 0);
        assert_eq!(schema.summary.serialize_field_type_count, 0);
        assert!(schema.types[0].fields[0].serialize.is_none());
    }

    #[test]
    fn skips_ambiguous_serialize_codegen_name_matches() {
        let network_type_id = uuid!("8673a3cc-2848-4c87-aa72-cc860589d1b5");
        let report = json!({
            "registryEntries": [{
                "uuid": network_type_id.to_string(),
                "typeName": "Example::SharedName"
            }],
            "fieldRegistrationFunctions": []
        });
        let unit = SerializeCodegenUnit {
            items: vec![
                SerializeCodegenItem {
                    source_type_id: uuid!("11111111-1111-1111-1111-111111111111"),
                    source_name: "Example::SharedName".to_owned(),
                    role: ReflectedTypeRole::SupportType,
                    is_reflection_marker: false,
                    is_abstract: Some(false),
                    factory: None,
                    rtti_base_chain: Vec::new(),
                    kind: SerializeCodegenItemKind::Struct,
                    enum_underlying_type: None,
                    fields: Vec::new(),
                    variants: Vec::new(),
                },
                SerializeCodegenItem {
                    source_type_id: uuid!("22222222-2222-2222-2222-222222222222"),
                    source_name: "Example::SharedName".to_owned(),
                    role: ReflectedTypeRole::SupportType,
                    is_reflection_marker: false,
                    is_abstract: Some(false),
                    factory: None,
                    rtti_base_chain: Vec::new(),
                    kind: SerializeCodegenItemKind::Struct,
                    enum_underlying_type: None,
                    fields: Vec::new(),
                    variants: Vec::new(),
                },
            ],
        };

        let mut schema =
            NetworkSchema::from_ghidra_static_network_report(&report).expect("normalized schema");
        let merge = schema.merge_serialize_codegen_unit(&unit, Some("serialize.json".to_owned()));

        assert_eq!(merge.matched_type_count, 0);
        assert_eq!(merge.name_matched_count, 0);
        assert_eq!(merge.ambiguous_name_match_count, 1);
        assert_eq!(merge.unmatched_schema_type_count, 1);
        assert_eq!(schema.summary.serialize_type_count, 0);
    }

    #[test]
    fn does_not_merge_serialize_codegen_by_nil_type_id() {
        let report = json!({
            "registryEntries": [{
                "uuid": "00000000-0000-0000-0000-000000000000",
                "typeName": "NullType"
            }],
            "fieldRegistrationFunctions": []
        });
        let unit = SerializeCodegenUnit {
            items: vec![SerializeCodegenItem {
                source_type_id: Uuid::nil(),
                source_name: "WaterDepth".to_owned(),
                role: ReflectedTypeRole::SupportType,
                is_reflection_marker: false,
                is_abstract: Some(false),
                factory: None,
                rtti_base_chain: Vec::new(),
                kind: SerializeCodegenItemKind::Struct,
                enum_underlying_type: None,
                fields: Vec::new(),
                variants: Vec::new(),
            }],
        };

        let mut schema =
            NetworkSchema::from_ghidra_static_network_report(&report).expect("normalized schema");
        let merge = schema.merge_serialize_codegen_unit(&unit, Some("serialize.json".to_owned()));

        assert_eq!(merge.matched_type_count, 0);
        assert_eq!(merge.type_id_matched_count, 0);
        assert_eq!(merge.name_matched_count, 0);
        assert_eq!(merge.unmatched_schema_type_count, 1);
        assert_eq!(schema.summary.serialize_type_count, 0);
    }

    #[test]
    fn structured_container_value_can_split_composite_map_key() {
        let report = json!({
            "registryEntries": [{
                "uuid": "4c6684a9-6988-4a05-94bd-118ce991a7d9",
                "typeIndex": 3312,
                "typeName": "Javelin::GameModeParticipantReplicatedState",
                "fields": [{
                    "index": 0,
                    "name": "activeGameModes",
                    "handlerVtable": "NewWorld+0x81b6e18",
                    "confidence": "register-field-call"
                }]
            }],
            "fieldRegistrationFunctions": [],
            "fieldHandlerVtables": [{
                "address": "NewWorld+0x81b6e18",
                "fieldCount": 1,
                "deltaMarshalShapes": [
                    "vlq-u32",
                    "vlq-u32",
                    "u8",
                    "fixed-bytes-16",
                    "u64",
                    "u64",
                    "sequence-number",
                    "u8",
                    "u32",
                    "u32",
                    "u64",
                    "u64",
                    "vlq-u32"
                ],
                "fullMarshalShapes": [
                    "sequence-number",
                    "vlq-u32",
                    "fixed-bytes-16",
                    "u64",
                    "u64",
                    "u32",
                    "u32",
                    "u64",
                    "u64",
                    "vlq-u32"
                ],
                "valueTypeShape": {
                    "typeName": "Value",
                    "typeNameSource": "synthetic-container-value",
                    "memberNameSource": "synthetic-serialize-type-sequence",
                    "memberNamesProven": false,
                    "validation": "container-value-serialize-type-sequence",
                    "members": [{
                        "index": 0,
                        "name": "remote_typeless_server_facet_ref",
                        "nativeType": "RemoteTypelessServerFacetRef",
                        "wireShape": "composite<fixed-bytes-16,u64,u64>"
                    }, {
                        "index": 1,
                        "name": "stat_multiplier_table_component",
                        "nativeType": "StatMultiplierTableComponent",
                        "wireShape": "composite<u32,u32,u64,u64>"
                    }]
                }
            }]
        });

        let schema =
            NetworkSchema::from_ghidra_static_network_report(&report).expect("normalized schema");
        let container = schema.field_handler_vtables[0]
            .container_shape
            .as_ref()
            .expect("container shape");

        assert_eq!(
            container.storage,
            NetworkReplicatedContainerStorageKind::Map
        );
        assert_eq!(
            container.key_wire_shapes,
            vec![
                NetworkWireScalarShape::FixedBytes(16),
                NetworkWireScalarShape::U64,
                NetworkWireScalarShape::U64
            ]
        );
        assert_eq!(
            container.key_type_name.as_deref(),
            Some("RemoteTypelessServerFacetRef")
        );
        assert_eq!(
            container.value_wire_shapes,
            vec![
                NetworkWireScalarShape::U32,
                NetworkWireScalarShape::U32,
                NetworkWireScalarShape::U64,
                NetworkWireScalarShape::U64
            ]
        );
        assert_eq!(
            container.value_type_name.as_deref(),
            Some("StatMultiplierTableComponent")
        );
        assert_eq!(
            container.source.as_deref(),
            Some("replicated-container-map-structured-key-shape")
        );
    }

    #[test]
    fn structured_container_value_matching_preserves_inner_counted_vecs() {
        let full_shapes = [
            "sequence-number",
            "vlq-u32",
            "u32",
            "f32",
            "f32",
            "f32",
            "vlq-u32",
            "u64",
        ]
        .map(ToOwned::to_owned);
        let value_shapes = replicated_container_full_value_scalar_shapes(&full_shapes);
        assert_eq!(
            value_shapes,
            vec![
                NetworkWireScalarShape::U32,
                NetworkWireScalarShape::F32,
                NetworkWireScalarShape::F32,
                NetworkWireScalarShape::F32,
                NetworkWireScalarShape::VlqU32,
                NetworkWireScalarShape::U64,
            ]
        );

        let shape = NetworkNestedTypeShape {
            type_id: Some(uuid!("fdda118c-1c41-48a4-af1c-b45fd6797fbe")),
            type_id_source: Some("rtti-provider-vtable".to_owned()),
            type_name: Some("ExampleValue".to_owned()),
            type_name_full: Some("ExampleValue".to_owned()),
            type_name_source: Some("az-rtti-provider-table".to_owned()),
            function: None,
            function_name: None,
            factory: None,
            az_rtti_address: Some("NewWorld+0x1000".to_owned()),
            constructor: None,
            vtable: None,
            member_base: None,
            member_name_source: Some("synthetic-pcode-wire-order".to_owned()),
            member_names_proven: Some(false),
            datatype_path: None,
            validation: Some("container-value-pcode-wire-order-native-rtti".to_owned()),
            members: vec![
                nested_type_member(0, "u32"),
                nested_type_member(1, "vec3"),
                nested_type_member(2, "vec<u64>"),
            ],
        };

        assert!(nested_type_shape_matches_wire_shapes(
            &shape,
            &value_shapes,
            &[]
        ));
        assert!(!nested_type_shape_matches_wire_shapes(
            &shape,
            &[
                NetworkWireScalarShape::U32,
                NetworkWireScalarShape::F32,
                NetworkWireScalarShape::F32,
                NetworkWireScalarShape::F32,
                NetworkWireScalarShape::U64,
            ],
            &[]
        ));
    }

    fn nested_type_member(index: u32, wire_shape: &str) -> NetworkNestedTypeMember {
        NetworkNestedTypeMember {
            index: Some(index),
            offset: None,
            native_offset: None,
            name: Some(format!("field_{index}")),
            name_source: Some("synthetic-pcode-wire-order".to_owned()),
            name_proven: Some(false),
            name_evidence: None,
            native_type: None,
            wire_shape: Some(wire_shape.to_owned()),
            byte_width: None,
            evidence_source: Some("test".to_owned()),
            callsite: None,
            target: None,
            target_name: None,
        }
    }
}
