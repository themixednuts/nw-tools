use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use thiserror::Error;
use uuid::Uuid;

use crate::ir::{
    SerializeCodegenIndex, SerializeCodegenItem, SerializeCodegenItemKind, SerializeCodegenUnit,
};
use crate::role::ReflectedTypeRole;

pub const NETWORK_SCHEMA_VERSION: &str = "newworld.network_schema.v1";
pub const NETWORK_STATIC_REPORT_SCHEMA_VERSION: &str = "newworld.network_schema.static.v1";

#[derive(Debug, Error)]
pub enum NetworkSchemaImportError {
    #[error("network schema import expected a JSON object root")]
    ExpectedObjectRoot,
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
    pub serialize_type_count: usize,
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
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NetworkType {
    pub type_id: Option<Uuid>,
    pub type_index: Option<u32>,
    pub registry_index: Option<u32>,
    pub name: Option<String>,
    pub name_source: Option<String>,
    pub root_kinds: Vec<NetworkRootKind>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub storage_address: Option<String>,
    pub base_vtable: Option<String>,
    pub vtable: Option<String>,
    pub handler: Option<NetworkHandler>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub instance: Option<NetworkInstanceLayout>,
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
    pub kind: NetworkSerializeKind,
    pub name: String,
    pub role: NetworkSerializeRole,
    pub field_count: usize,
    pub variant_count: usize,
    pub direct_dependency_type_ids: Vec<Uuid>,
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum NetworkRootKind {
    ReplicatedState,
    Message,
    FieldRegisteredType,
    SupportType,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NetworkField {
    pub index: Option<u32>,
    pub name: Option<String>,
    pub name_address: Option<String>,
    pub group: Option<u32>,
    pub handler_offset: Option<String>,
    pub handler_expression: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub handler_vtable: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub native_type: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rust_type: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub storage_expression: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub storage_offset: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub wire_shape: Option<NetworkWireShape>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub wire_shape_source: Option<String>,
    pub callsite: Option<String>,
    pub confidence: NetworkConfidence,
    pub evidence: Vec<NetworkEvidence>,
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
    pub az_rtti: Option<NetworkAzRtti>,
    pub fields: Vec<NetworkField>,
    pub evidence: Vec<NetworkEvidence>,
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
    pub slots: Vec<NetworkVirtualFunction>,
    pub evidence: Vec<NetworkEvidence>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
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
    Vec2,
    Vec3,
    Vec4,
    Quat,
    QuatCompNorm,
    Mat3,
    Affine3,
    Aabb3d,
    EntityRef,
    #[serde(rename = "fixed-bytes-6", alias = "fixed-bytes6")]
    FixedBytes6,
    #[serde(rename = "fixed-bytes-16", alias = "fixed-bytes16")]
    FixedBytes16,
    String,
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
        let root = report
            .as_object()
            .ok_or(NetworkSchemaImportError::ExpectedObjectRoot)?;
        let types = array_values(root, "registryEntries")
            .filter_map(Value::as_object)
            .map(network_type_from_registry_entry)
            .collect::<Vec<_>>();
        let field_registration_functions = array_values(root, "fieldRegistrationFunctions")
            .filter_map(Value::as_object)
            .map(network_field_registration_function)
            .collect::<Vec<_>>();
        let field_handler_vtables = array_values(root, "fieldHandlerVtables")
            .filter_map(Value::as_object)
            .map(network_field_handler_vtable)
            .collect::<Vec<_>>();
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
            field_registration_functions,
            field_handler_vtables,
        };
        schema.summary = schema.build_summary();
        Ok(schema)
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
        let mut report = NetworkSerializeMergeReport {
            source_type_count: unit.items.len(),
            ..NetworkSerializeMergeReport::default()
        };

        for network_type in &mut self.types {
            let Some((item, confidence, source)) =
                serialize_match(network_type, &index, &name_index, &mut report)
            else {
                continue;
            };
            report.matched_type_count += 1;
            if network_type.name.is_none() {
                network_type.name = Some(item.source_name.clone());
                network_type.name_source = Some(source.clone());
                report.filled_name_count += 1;
            }
            network_type.serialize = Some(network_serialize_type(item));
            network_type.evidence.push(NetworkEvidence {
                kind: NetworkEvidenceKind::SerializeContext,
                source,
                address: None,
                detail: Some(item.source_name.clone()),
                confidence,
            });
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
            serialize_type_count: self
                .types
                .iter()
                .filter(|network_type| network_type.serialize.is_some())
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
        handler_offset: None,
        handler_expression: None,
        handler_vtable: None,
        native_type: signature.native_type.clone(),
        rust_type: signature.rust_type.clone(),
        storage_expression: None,
        storage_offset: None,
        wire_shape: signature.wire_shape,
        wire_shape_source: signature.wire_shape.map(|_| source),
        callsite: None,
        confidence: NetworkConfidence::High,
        evidence,
    }
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

fn network_serialize_type(item: &SerializeCodegenItem) -> NetworkSerializeType {
    let mut direct_dependency_type_ids = item
        .direct_dependency_type_ids()
        .into_iter()
        .collect::<Vec<_>>();
    direct_dependency_type_ids.sort_unstable();
    NetworkSerializeType {
        kind: network_serialize_kind(item.kind),
        name: item.source_name.clone(),
        role: network_serialize_role(item.role),
        field_count: item.fields.len(),
        variant_count: item.variants.len(),
        direct_dependency_type_ids,
        is_abstract: item.is_abstract,
        is_reflection_marker: item.is_reflection_marker,
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
    let root_kinds = root_kinds(name.as_deref(), has_registered_fields);
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

    NetworkType {
        type_id,
        type_index: u32_value(entry, "typeIndex"),
        registry_index: u32_value(entry, "index"),
        name,
        name_source: string(entry, "typeNameSource"),
        root_kinds,
        storage_address,
        base_vtable,
        vtable,
        handler,
        instance,
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

    NetworkFieldRegistrationFunction {
        address: string(function, "address"),
        name: string(function, "name"),
        constructor_type_name: string(function, "constructorTypeName"),
        owner_type_id: az_rtti.as_ref().and_then(|rtti| rtti.type_id),
        owner_type_name: string(function, "constructorTypeName")
            .or_else(|| az_rtti.as_ref().and_then(|rtti| rtti.type_name.clone())),
        instance_vtable: string(function, "instanceVtable"),
        virtual_functions,
        az_rtti,
        fields,
        evidence,
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
    NetworkField {
        index: u32_value(field, "index"),
        name: string(field, "name"),
        name_address: string(field, "nameAddress"),
        group: u32_value(field, "group"),
        handler_offset: string(field, "handlerOffset"),
        handler_expression: string(field, "handlerExpression"),
        handler_vtable: string(field, "handlerVtable"),
        native_type: string(field, "nativeType"),
        rust_type: string(field, "rustType"),
        storage_expression: string(field, "storageExpression"),
        storage_offset: hex_or_decimal_u32(field, "storageOffset"),
        wire_shape: wire_shape(field, "wireShape"),
        wire_shape_source: string(field, "wireShapeSource"),
        callsite: string(field, "callsite"),
        confidence,
        evidence,
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
    NetworkFieldHandlerVtable {
        address: string(vtable, "address"),
        field_count: usize_value(vtable, "fieldCount").unwrap_or_default(),
        marshal: string(vtable, "marshal"),
        marshal_target: string(vtable, "marshalTarget"),
        unmarshal: string(vtable, "unmarshal"),
        unmarshal_target: string(vtable, "unmarshalTarget"),
        wire_shape: wire_shape(vtable, "wireShape"),
        wire_shape_source: string(vtable, "wireShapeSource"),
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

fn root_kinds(name: Option<&str>, has_registered_fields: bool) -> Vec<NetworkRootKind> {
    let mut kinds = Vec::new();
    if name.is_some_and(|name| name.contains("ReplicatedState")) {
        kinds.push(NetworkRootKind::ReplicatedState);
    }
    if name.is_some_and(is_message_name) {
        kinds.push(NetworkRootKind::Message);
    }
    if has_registered_fields {
        kinds.push(NetworkRootKind::FieldRegisteredType);
    }
    if kinds.is_empty() {
        kinds.push(NetworkRootKind::SupportType);
    }
    kinds
}

fn is_message_name(name: &str) -> bool {
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

fn string_ref<'a>(object: &'a Map<String, Value>, key: &str) -> Option<&'a str> {
    object
        .get(key)
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
}

fn stable_address(object: &Map<String, Value>, key: &str) -> Option<String> {
    string_ref(object, key)
        .filter(|value| value.starts_with("NewWorld+0x"))
        .map(ToOwned::to_owned)
}

fn wire_shape(object: &Map<String, Value>, key: &str) -> Option<NetworkWireShape> {
    match string_ref(object, key)? {
        "bool" => Some(NetworkWireShape::Bool),
        "u8" => Some(NetworkWireShape::U8),
        "u16" => Some(NetworkWireShape::U16),
        "u32" => Some(NetworkWireShape::U32),
        "u64" => Some(NetworkWireShape::U64),
        "f32" => Some(NetworkWireShape::F32),
        "f64" => Some(NetworkWireShape::F64),
        "half-f32" => Some(NetworkWireShape::HalfF32),
        "vlq-u32" => Some(NetworkWireShape::VlqU32),
        "vec2" => Some(NetworkWireShape::Vec2),
        "vec3" => Some(NetworkWireShape::Vec3),
        "vec4" => Some(NetworkWireShape::Vec4),
        "quat" => Some(NetworkWireShape::Quat),
        "quat-comp-norm" => Some(NetworkWireShape::QuatCompNorm),
        "mat3" => Some(NetworkWireShape::Mat3),
        "affine3" => Some(NetworkWireShape::Affine3),
        "aabb3d" => Some(NetworkWireShape::Aabb3d),
        "entity-ref" => Some(NetworkWireShape::EntityRef),
        "fixed-bytes-6" => Some(NetworkWireShape::FixedBytes6),
        "fixed-bytes-16" => Some(NetworkWireShape::FixedBytes16),
        "string" => Some(NetworkWireShape::String),
        _ => None,
    }
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
            network_type.root_kinds,
            vec![
                NetworkRootKind::ReplicatedState,
                NetworkRootKind::FieldRegisteredType
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
            Some(NetworkWireShape::FixedBytes6)
        );
        assert_eq!(
            schema.field_handler_vtables[1].wire_shape,
            Some(NetworkWireShape::FixedBytes16)
        );
    }

    #[test]
    fn keeps_message_and_support_type_classification_separate() {
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

        assert_eq!(schema.types[0].root_kinds, vec![NetworkRootKind::Message]);
        assert_eq!(
            schema.types[1].root_kinds,
            vec![NetworkRootKind::SupportType]
        );
    }

    #[test]
    fn imports_message_unmarshal_fields_without_field_registered_kind() {
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
        assert_eq!(schema.types[0].root_kinds, vec![NetworkRootKind::Message]);
        let instance = schema.types[0].instance.as_ref().expect("instance layout");
        assert_eq!(instance.size, Some(0x470));
        assert_eq!(instance.constructor.as_deref(), Some("NewWorld+0x7e37d0"));
        assert_eq!(
            schema.types[0].fields[0].native_type.as_deref(),
            Some("u32")
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
}
