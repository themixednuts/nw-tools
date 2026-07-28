use super::*;

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
    #[error("incompatible Ghidra network-schema overlay: {0}")]
    IncompatibleOverlay(String),
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
    #[serde(default)]
    pub message_marshal_field_count: usize,
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
    pub field_name_matched_count: usize,
    #[serde(default)]
    pub ambiguous_field_name_match_count: usize,
    pub filled_name_count: usize,
    pub unmatched_schema_type_count: usize,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NetworkSerializeCatalogMergeReport {
    pub required_type_count: usize,
    pub matched_generic_type_count: usize,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NetworkMessageSignatureMergeReport {
    pub source_message_count: usize,
    pub matched_message_count: usize,
    pub ambiguous_message_count: usize,
    pub unmatched_message_count: usize,
    pub field_count_mismatch_count: usize,
    #[serde(default)]
    pub field_grouped_count: usize,
    #[serde(default)]
    pub field_reordered_count: usize,
    pub field_index_mismatch_count: usize,
    pub field_name_filled_count: usize,
    pub field_name_conflict_count: usize,
    pub native_type_filled_count: usize,
    pub native_type_conflict_count: usize,
    pub wire_shape_filled_count: usize,
    pub wire_shape_conflict_count: usize,
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
    pub field_name_updated_count: usize,
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
    pub name: Option<String>,
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
    #[serde(skip_serializing_if = "Option::is_none")]
    pub replicated_state_abi: Option<NetworkReplicatedStateAbiEvidence>,
    pub serialize: Option<NetworkSerializeType>,
    pub az_rtti: Option<NetworkAzRtti>,
    pub registration_type_name: Option<String>,
    pub registration_hook: Option<NetworkRegistrationHook>,
    pub fields: Vec<NetworkField>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub marshal_fields: Vec<NetworkField>,
    #[serde(default, skip_serializing_if = "is_false")]
    pub signature_field_count_conflict: bool,
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
    pub resolved_type: Option<ResolvedType>,
    #[serde(default = "default_true", skip_serializing_if = "is_true")]
    pub emits_source: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub factory: Option<String>,
    pub field_count: usize,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub fields: Vec<NetworkSerializeField>,
    pub variant_count: usize,
    pub direct_dependency_type_ids: Vec<Uuid>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub wire_shapes: Vec<NetworkWireScalarShape>,
    pub is_abstract: Option<bool>,
    pub is_reflection_marker: bool,
}

const fn default_true() -> bool {
    true
}

const fn is_true(value: &bool) -> bool {
    *value
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NetworkSerializeField {
    pub name: String,
    pub type_id: Uuid,
    pub resolved_type: ResolvedType,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub offset: Option<u32>,
    #[serde(default, skip_serializing_if = "is_false")]
    pub is_base_class: bool,
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
    pub direct_dependency_type_ids: Vec<Uuid>,
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
    pub handler_kind_source: Option<String>,
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
    pub source_type_id_source: Option<String>,
    #[serde(default, skip_serializing_if = "is_false")]
    pub source_type_identity_proven: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rust_type: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub storage_expression: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub storage_base: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub storage_base_offset: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub storage_offset: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub raw_byte_length: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub wire_shape: Option<NetworkWireShape>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub wire_shape_raw: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub wire_layout: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub wire_layout_source: Option<String>,
    #[serde(default, skip_serializing_if = "is_false")]
    pub type_conflict: bool,
    #[serde(default, skip_serializing_if = "is_false")]
    pub signature_type_conflict: bool,
    #[serde(default, skip_serializing_if = "is_false")]
    pub signature_wire_conflict: bool,
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
    #[serde(skip_serializing_if = "Option::is_none")]
    pub identity_proven: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub identity_source: Option<String>,
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
    #[serde(skip_serializing_if = "Option::is_none")]
    pub layout_proven: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub member_coverage_proven: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub wire_order_proven: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub wire_order_source: Option<String>,
    pub datatype_path: Option<String>,
    pub validation: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub native_size: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub native_size_source: Option<String>,
    pub members: Vec<NetworkNestedTypeMember>,
}

impl NetworkNestedTypeShape {
    pub(crate) fn has_exact_identity(&self) -> bool {
        self.type_id.is_some()
            && self
                .type_name_full
                .as_deref()
                .or(self.type_name.as_deref())
                .is_some_and(|name| !name.trim().is_empty())
            && self.identity_proven == Some(true)
    }

    pub(crate) fn has_proven_layout(&self) -> bool {
        self.layout_proven == Some(true)
            && self.member_coverage_proven == Some(true)
            && self.wire_order_proven == Some(true)
            && !self.members.is_empty()
    }

    pub(crate) fn has_proven_symbolic_identity(&self) -> bool {
        self.type_id.is_none()
            && self
                .type_name_full
                .as_deref()
                .or(self.type_name.as_deref())
                .is_some_and(|name| !name.trim().is_empty())
            && self.identity_proven == Some(true)
            && self.has_proven_layout()
    }

    pub(crate) fn has_proven_anonymous_layout(&self) -> bool {
        self.type_id.is_none() && self.identity_proven != Some(true) && self.has_proven_layout()
    }
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
    #[serde(skip_serializing_if = "Option::is_none")]
    pub type_id: Option<Uuid>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub type_id_source: Option<String>,
    #[serde(default, skip_serializing_if = "is_false")]
    pub type_identity_proven: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub type_identity_source: Option<String>,
    pub wire_shape: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub wire_shape_source: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub wire_layout: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub wire_layout_source: Option<String>,
    pub byte_width: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub wire_ordinal: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub wire_order_source: Option<String>,
    pub callsite: Option<String>,
    pub target: Option<String>,
    pub target_name: Option<String>,
    #[serde(default, skip_serializing_if = "is_false")]
    pub type_conflict: bool,
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
    #[serde(skip_serializing_if = "Option::is_none")]
    pub replicated_state_abi: Option<NetworkReplicatedStateAbiEvidence>,
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
pub struct NetworkHandlerContainerType {
    pub storage_kind: NetworkReplicatedContainerStorageKind,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub key_native_type: Option<String>,
    pub value_native_type: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub storage_native_type: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub key_marshaler_type: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub value_marshaler_type: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NetworkFieldHandlerVtable {
    pub address: Option<String>,
    pub field_count: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub handler_kind: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub handler_kind_source: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub handler_type_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub handler_type_source: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub handler_container_type: Option<NetworkHandlerContainerType>,
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
    pub wire_layout: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub wire_layout_source: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub delta_wire_shape: Option<NetworkWireShape>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub full_wire_shape: Option<NetworkWireShape>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub delta_wire_layout: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub full_wire_layout: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub key_native_type: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub key_native_type_source: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub delta_marshal_shapes: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub full_marshal_shapes: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub delta_marshal_layouts: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub full_marshal_layouts: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub value_type_info: Option<NetworkNativeTypeInfoEvidence>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub value_type_candidates: Vec<NetworkNativeTypeInfoEvidence>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub value_type_shape: Option<NetworkNestedTypeShape>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub embedded_value_type_shapes: Vec<NetworkNestedTypeShape>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub full_container_plan: Option<NetworkReplicatedContainerPlan>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub full_container_plan_diagnostics: Vec<NetworkContainerPlanDiagnostic>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fixed_sequence_shape: Option<NetworkFixedSequenceShape>,
    pub slots: Vec<NetworkVirtualFunction>,
    pub evidence: Vec<NetworkEvidence>,
}

impl NetworkFieldHandlerVtable {
    fn has_structured_container_value(&self) -> bool {
        self.value_type_shape.is_some()
            || self.full_container_plan.as_ref().is_some_and(|plan| {
                let [codec] = plan.value_codecs.as_slice() else {
                    return true;
                };
                codec.wire_shape.is_none() || !codec.members.is_empty()
            })
    }

    pub(super) fn should_suppress_replicated_container_wire_shape(&self) -> bool {
        if !matches!(
            self.wire_shape,
            Some(NetworkWireShape::ReplicatedContainer(_))
        ) {
            return false;
        }
        self.full_container_plan.as_ref().is_some_and(|plan| {
            plan.storage != NetworkReplicatedContainerStorageKind::Map
                || plan
                    .exact_value_wire_shapes()
                    .is_none_or(|shapes| shapes.len() != 1)
        }) || self.has_structured_container_value()
    }
}
