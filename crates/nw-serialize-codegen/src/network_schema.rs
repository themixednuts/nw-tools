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
    pub type_index_evidence_count: usize,
    pub serialize_type_count: usize,
    pub serialize_dependency_count: usize,
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
    GhidraReplicatedStateStaticReport,
    TypeRegistry,
    TypeIndex,
    SerializeContext,
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
    pub storage_address: Option<String>,
    pub base_vtable: Option<String>,
    pub vtable: Option<String>,
    pub handler: Option<NetworkHandler>,
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
    pub address: Option<String>,
    pub function: Option<String>,
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
    RegisterField,
    FieldRegistrationFunction,
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
    pub fn from_replicated_state_ghidra_report(
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
        let mut schema = Self {
            schema: NETWORK_SCHEMA_VERSION.to_owned(),
            sources: vec![NetworkSchemaSource {
                kind: NetworkSchemaSourceKind::GhidraReplicatedStateStaticReport,
                path: string(root, "input"),
                schema: string(root, "schema"),
                program: string(root, "program"),
                image_base: string(root, "imageBase"),
            }],
            summary: NetworkSchemaSummary::default(),
            types,
            field_registration_functions,
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
        }
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
    let handler = entry
        .get("handler")
        .and_then(Value::as_object)
        .map(network_handler);
    let az_rtti = entry
        .get("azRtti")
        .and_then(Value::as_object)
        .map(network_az_rtti);
    let registration_hook = entry
        .get("registrationHook")
        .and_then(Value::as_object)
        .map(network_registration_hook);
    let name = registry_entry_name(entry, az_rtti.as_ref(), registration_hook.as_ref());
    let fields = array_values(entry, "fields")
        .filter_map(Value::as_object)
        .map(network_field)
        .collect::<Vec<_>>();
    let root_kinds = root_kinds(name.as_deref(), !fields.is_empty());
    let mut evidence = Vec::new();

    if type_id.is_some() || entry.contains_key("typeIndex") || entry.contains_key("index") {
        evidence.push(NetworkEvidence {
            kind: NetworkEvidenceKind::TypeRegistry,
            source: "registryEntries".to_owned(),
            address: string(entry, "storageAddress"),
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
            address: string(entry, "vtable").or_else(|| string(entry, "baseVtable")),
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
        storage_address: string(entry, "storageAddress"),
        base_vtable: string(entry, "baseVtable"),
        vtable: string(entry, "vtable"),
        handler,
        serialize: None,
        az_rtti,
        registration_type_name: string(entry, "registrationTypeName"),
        registration_hook,
        fields,
        evidence,
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
    let confidence = confidence_from_raw(string_ref(field, "confidence"));
    NetworkField {
        index: u32_value(field, "index"),
        name: string(field, "name"),
        name_address: string(field, "nameAddress"),
        group: u32_value(field, "group"),
        handler_offset: string(field, "handlerOffset"),
        handler_expression: string(field, "handlerExpression"),
        callsite: string(field, "callsite"),
        confidence,
        evidence: vec![NetworkEvidence {
            kind: NetworkEvidenceKind::RegisterField,
            source: string(field, "confidence").unwrap_or_else(|| "field".to_owned()),
            address: string(field, "callsite"),
            detail: string(field, "name"),
            confidence,
        }],
    }
}

fn network_virtual_function(function: &Map<String, Value>) -> NetworkVirtualFunction {
    NetworkVirtualFunction {
        slot: u32_value(function, "slot"),
        slot_offset: string(function, "slotOffset"),
        address: string(function, "address"),
        function: string(function, "function"),
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
        Some("register-field-call" | "registration-hook" | "az-rtti") => NetworkConfidence::High,
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

fn u32_value(object: &Map<String, Value>, key: &str) -> Option<u32> {
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

    #[test]
    fn converts_ghidra_report_to_normalized_network_schema() {
        let report = json!({
            "schema": "newworld.replicated_state_schema.static.v1",
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
            }]
        });

        let schema =
            NetworkSchema::from_replicated_state_ghidra_report(&report).expect("normalized schema");

        assert_eq!(schema.schema, NETWORK_SCHEMA_VERSION);
        assert_eq!(
            schema.sources[0].path.as_deref(),
            Some("E:/Projects/new-world/resources/typeregistry.json")
        );
        assert_eq!(schema.summary.type_count, 1);
        assert_eq!(schema.summary.register_field_function_count, 1);
        assert_eq!(schema.summary.register_field_count, 1);
        assert_eq!(schema.summary.high_confidence_field_count, 1);

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
        assert_eq!(network_type.fields[0].confidence, NetworkConfidence::High);

        let function = &schema.field_registration_functions[0];
        assert_eq!(function.owner_type_id, network_type.type_id);
        assert_eq!(
            function.fields[0].callsite.as_deref(),
            Some("NewWorld+0x3495762")
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
            NetworkSchema::from_replicated_state_ghidra_report(&report).expect("normalized schema");

        assert_eq!(schema.types[0].root_kinds, vec![NetworkRootKind::Message]);
        assert_eq!(
            schema.types[1].root_kinds,
            vec![NetworkRootKind::SupportType]
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
            NetworkSchema::from_replicated_state_ghidra_report(&report).expect("normalized schema");
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
            NetworkSchema::from_replicated_state_ghidra_report(&report).expect("normalized schema");
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
            NetworkSchema::from_replicated_state_ghidra_report(&report).expect("normalized schema");
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
            NetworkSchema::from_replicated_state_ghidra_report(&report).expect("normalized schema");
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
            NetworkSchema::from_replicated_state_ghidra_report(&report).expect("normalized schema");
        let merge = schema.merge_serialize_codegen_unit(&unit, Some("serialize.json".to_owned()));

        assert_eq!(merge.matched_type_count, 0);
        assert_eq!(merge.type_id_matched_count, 0);
        assert_eq!(merge.name_matched_count, 0);
        assert_eq!(merge.unmatched_schema_type_count, 1);
        assert_eq!(schema.summary.serialize_type_count, 0);
    }
}
