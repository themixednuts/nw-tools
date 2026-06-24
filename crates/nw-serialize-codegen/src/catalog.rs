//! Tooling-only reflected type catalog for New World component generation.
//!
//! `serialize.json` is the source for reflected type shape. Native component
//! descriptor captures add component identity, module, and vtable-slot evidence
//! where they match the reflected catalog. `serialize-porting.json` is parsed
//! as a coverage ledger only; it does not supply missing fields or type shape.

use std::{
    collections::BTreeMap,
    fs, io,
    path::{Path, PathBuf},
};

use nw_objectstream::type_uuid::type_ids;
use serde_json::Value;
use thiserror::Error;
use uuid::Uuid;

use crate::CodegenContext;
use crate::class_registration::{
    ClassRegistrationTraceIndex, ClassRegistrationTraceRecord, class_registration_trace_index,
};
use crate::document::{SerializeContextDocument, SerializeContextDocumentError};
use crate::model::ReflectedClass as ModelClass;
use crate::model::{ReflectedMember as ModelMember, SerializeContextModel};
use crate::module_descriptors::{
    is_module_descriptor_json_name, module_descriptor_capture, module_descriptors_root,
    module_descriptors_root_from_capture, module_name_from_path,
};
use crate::role::{ReflectedTypeRole, SerializeRoleClassifier};
use crate::types::{ResolvedType, TypeResolver};

#[derive(Debug, Clone)]
pub struct ReflectedTypeCatalog {
    types_by_id: BTreeMap<Uuid, ReflectedType>,
    type_ids_by_name: BTreeMap<String, Uuid>,
    component_descriptors_by_id: BTreeMap<Uuid, ComponentDescriptor>,
    component_descriptor_ids_by_name: BTreeMap<String, Vec<Uuid>>,
    class_registration_trace: ClassRegistrationTraceIndex,
    generic_types_by_id: BTreeMap<Uuid, ReflectedGenericType>,
    porting_ledger: Option<SerializePortingLedger>,
}

#[derive(Debug, Clone, Copy, Default)]
pub struct ReflectedTypeCatalogInputs<'a> {
    pub module_descriptors_root: Option<&'a Value>,
    pub serialize_porting_root: Option<&'a Value>,
    pub class_registration_trace_root: Option<&'a Value>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReflectedType {
    pub name: String,
    pub type_id: Uuid,
    pub version: Option<u32>,
    pub role: ReflectedTypeRole,
    pub is_reflection_marker: bool,
    pub fields: Vec<ReflectedField>,
    pub base_type_ids: Vec<Uuid>,
}

impl ReflectedType {
    pub fn authored_fields(&self) -> impl Iterator<Item = &ReflectedField> {
        self.fields.iter().filter(|field| !field.is_base_class)
    }

    pub fn serializable_fields(&self) -> impl Iterator<Item = &ReflectedField> {
        self.fields
            .iter()
            .filter(|field| !field.is_reflection_marker_base_class)
    }

    #[must_use]
    pub const fn is_component_scaffold_type(&self) -> bool {
        !self.is_reflection_marker && self.role.is_az_component_like()
    }

    #[must_use]
    pub const fn is_support_type(&self) -> bool {
        matches!(self.role, ReflectedTypeRole::SupportType)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReflectedField {
    pub name: String,
    pub name_crc: Option<u32>,
    pub type_id: Uuid,
    pub type_name: Option<String>,
    pub data_size: Option<u32>,
    pub offset: Option<u32>,
    pub flags: Option<u32>,
    pub is_base_class: bool,
    pub is_reflection_marker_base_class: bool,
    pub is_pointer: bool,
    pub is_dynamic_field: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReflectedGenericType {
    pub type_id: Uuid,
    pub base_name: String,
    pub argument_type_ids: Vec<Uuid>,
    pub non_type_capacity: Option<usize>,
    pub display_name: String,
    pub resolved_type: ResolvedType,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ComponentDescriptor {
    pub name: String,
    pub type_id: Uuid,
    pub module_name: Option<String>,
    pub vtable: Option<String>,
    pub address: Option<String>,
    pub vtable_slots: Vec<ComponentDescriptorVtableSlot>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ComponentDescriptorVtableSlot {
    pub slot: u32,
    pub expected: Option<String>,
    pub address: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SerializePortingLedger {
    pub component_modules_by_name: BTreeMap<String, PathBuf>,
    pub support_modules_by_type_name: BTreeMap<String, PathBuf>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ReflectedTypeCatalogSummary {
    pub reflected_types: usize,
    pub generic_types: usize,
    pub component_descriptors: usize,
    pub component_descriptor_name_collisions: usize,
    pub class_registration_records: usize,
    pub class_registration_type_ids: usize,
    pub class_registration_duplicate_type_ids: usize,
    pub porting_components: usize,
    pub porting_support_types: usize,
    pub faceted_components: usize,
    pub az_components: usize,
    pub client_facets: usize,
    pub server_facets: usize,
}

#[derive(Debug, Error)]
pub enum ReflectedTypeCatalogError {
    #[error("failed to read {path}: {source}")]
    Read {
        path: PathBuf,
        #[source]
        source: io::Error,
    },
    #[error("failed to parse {path}: {source}")]
    Parse {
        path: PathBuf,
        #[source]
        source: serde_json::Error,
    },
    #[error(transparent)]
    Document(#[from] SerializeContextDocumentError),
}

impl ReflectedTypeCatalog {
    pub fn from_paths(
        serialize_context: impl AsRef<Path>,
        module_descriptors: Option<impl AsRef<Path>>,
        serialize_porting: Option<impl AsRef<Path>>,
        context: &CodegenContext,
    ) -> Result<Self, ReflectedTypeCatalogError> {
        let serialize_context = serialize_context.as_ref().to_path_buf();
        let module_descriptors = module_descriptors
            .as_ref()
            .map(|path| path.as_ref().to_path_buf());
        let serialize_porting = serialize_porting
            .as_ref()
            .map(|path| path.as_ref().to_path_buf());

        let ((serialize_context_document, module_descriptors_root), serialize_porting_root) =
            context.runner().join(
                || {
                    context.runner().join(
                        || SerializeContextDocument::from_path(&serialize_context),
                        || read_optional_module_descriptors(module_descriptors.as_deref(), context),
                    )
                },
                || read_optional_json(serialize_porting.as_deref()),
            );

        let serialize_context_document = serialize_context_document?;
        let module_descriptors_root = module_descriptors_root?;
        let serialize_porting_root = serialize_porting_root?;

        Ok(Self::from_document(
            &serialize_context_document,
            module_descriptors_root.as_ref(),
            serialize_porting_root.as_ref(),
            context,
        ))
    }

    #[must_use]
    pub fn from_json_roots(
        serialize_context_root: &Value,
        module_descriptors_root: Option<&Value>,
        serialize_porting_root: Option<&Value>,
        context: &CodegenContext,
    ) -> Self {
        let model = SerializeContextModel::from_root(serialize_context_root);
        Self::from_model(
            &model,
            module_descriptors_root,
            serialize_porting_root,
            context,
        )
    }

    #[must_use]
    pub fn from_document(
        document: &SerializeContextDocument,
        module_descriptors_root: Option<&Value>,
        serialize_porting_root: Option<&Value>,
        context: &CodegenContext,
    ) -> Self {
        let model = SerializeContextModel::from_document(document);
        Self::from_model(
            &model,
            module_descriptors_root,
            serialize_porting_root,
            context,
        )
    }

    #[must_use]
    pub fn from_model(
        model: &SerializeContextModel,
        module_descriptors_root: Option<&Value>,
        serialize_porting_root: Option<&Value>,
        context: &CodegenContext,
    ) -> Self {
        Self::from_model_with_inputs(
            model,
            ReflectedTypeCatalogInputs {
                module_descriptors_root,
                serialize_porting_root,
                class_registration_trace_root: None,
            },
            context,
        )
    }

    #[must_use]
    pub fn from_model_with_inputs(
        model: &SerializeContextModel,
        inputs: ReflectedTypeCatalogInputs<'_>,
        context: &CodegenContext,
    ) -> Self {
        let mut class_names_by_id = primitive_names();

        for class in model.classes.values() {
            class_names_by_id
                .entry(class.type_id)
                .or_insert_with(|| class.name.clone());
            if let Some(map_key_type_id) = class.map_key_type_id {
                class_names_by_id
                    .entry(map_key_type_id)
                    .or_insert_with(|| class.name.clone());
            }
        }
        let mut generic_types_by_id = BTreeMap::new();
        let generic_ids = model.generic_classes.keys().copied().collect::<Vec<_>>();
        let generic_types = context
            .runner()
            .map(&generic_ids, |generic_id| {
                build_model_generic_type(*generic_id, model, &class_names_by_id, &mut Vec::new())
            })
            .into_iter()
            .flatten()
            .collect::<Vec<_>>();
        for generic_type in generic_types {
            class_names_by_id
                .entry(generic_type.type_id)
                .or_insert_with(|| generic_type.display_name.clone());
            generic_types_by_id.insert(generic_type.type_id, generic_type);
        }

        let role_classifier = SerializeRoleClassifier::from_model(model);
        let mut types_by_id = BTreeMap::new();
        let mut type_ids_by_name = BTreeMap::new();
        {
            let classes = model.classes.values().collect::<Vec<_>>();
            let mut reflected_types = context.runner().map(&classes, |class| {
                let class = *class;
                let fields = reflected_fields_from_model(
                    class,
                    &class_names_by_id,
                    role_classifier.reflection_marker_type_ids(),
                );
                let base_type_ids = fields
                    .iter()
                    .filter(|field| field.is_base_class)
                    .map(|field| field.type_id)
                    .collect::<Vec<_>>();
                ReflectedType {
                    name: class.name.clone(),
                    type_id: class.type_id,
                    version: class.version,
                    role: ReflectedTypeRole::SupportType,
                    is_reflection_marker: role_classifier.is_reflection_marker(class.type_id),
                    fields,
                    base_type_ids,
                }
            });

            for reflected in &mut reflected_types {
                reflected.role = role_classifier.classify(reflected.type_id);
            }

            for reflected in reflected_types {
                type_ids_by_name
                    .entry(reflected.name.clone())
                    .or_insert(reflected.type_id);
                types_by_id.insert(reflected.type_id, reflected);
            }
        }

        let (component_descriptors_by_id, component_descriptor_ids_by_name) = inputs
            .module_descriptors_root
            .map(parse_component_descriptors)
            .unwrap_or_default();
        let class_registration_trace =
            class_registration_trace_index(inputs.class_registration_trace_root);
        let porting_ledger = inputs
            .serialize_porting_root
            .map(parse_serialize_porting_ledger);

        Self {
            types_by_id,
            type_ids_by_name,
            component_descriptors_by_id,
            component_descriptor_ids_by_name,
            class_registration_trace,
            generic_types_by_id,
            porting_ledger,
        }
    }

    #[must_use]
    pub fn summary(&self) -> ReflectedTypeCatalogSummary {
        let mut summary = ReflectedTypeCatalogSummary {
            reflected_types: self.types_by_id.len(),
            generic_types: self.generic_types_by_id.len(),
            component_descriptors: self.component_descriptors_by_id.len(),
            component_descriptor_name_collisions: self
                .component_descriptor_ids_by_name
                .values()
                .filter(|type_ids| type_ids.len() > 1)
                .count(),
            class_registration_records: self.class_registration_trace.record_count,
            class_registration_type_ids: self.class_registration_trace.len(),
            class_registration_duplicate_type_ids: self.class_registration_trace.duplicates_len(),
            porting_components: self
                .porting_ledger
                .as_ref()
                .map_or(0, |ledger| ledger.component_modules_by_name.len()),
            porting_support_types: self
                .porting_ledger
                .as_ref()
                .map_or(0, |ledger| ledger.support_modules_by_type_name.len()),
            faceted_components: 0,
            az_components: 0,
            client_facets: 0,
            server_facets: 0,
        };

        for ty in self.types_by_id.values() {
            match ty.role {
                ReflectedTypeRole::FacetedComponent => summary.faceted_components += 1,
                ReflectedTypeRole::AzComponent => summary.az_components += 1,
                ReflectedTypeRole::ClientFacet => summary.client_facets += 1,
                ReflectedTypeRole::ServerFacet => summary.server_facets += 1,
                ReflectedTypeRole::AzEntity | ReflectedTypeRole::SupportType => {}
            }
        }

        summary
    }

    #[must_use]
    pub fn type_by_id(&self, type_id: Uuid) -> Option<&ReflectedType> {
        self.types_by_id.get(&type_id)
    }

    #[must_use]
    pub fn type_id_by_name(&self, name: &str) -> Option<Uuid> {
        self.type_ids_by_name.get(name).copied()
    }

    #[must_use]
    pub fn type_name(&self, type_id: Uuid) -> Option<&str> {
        self.types_by_id
            .get(&type_id)
            .map(|ty| ty.name.as_str())
            .or_else(|| {
                self.generic_types_by_id
                    .get(&type_id)
                    .map(|ty| ty.display_name.as_str())
            })
    }

    #[must_use]
    pub fn generic_type(&self, type_id: Uuid) -> Option<&ReflectedGenericType> {
        self.generic_types_by_id.get(&type_id)
    }

    pub fn component_scaffold_types(&self) -> impl Iterator<Item = &ReflectedType> {
        self.types_by_id
            .values()
            .filter(|ty| ty.is_component_scaffold_type())
    }

    pub fn reflected_types(&self) -> impl Iterator<Item = &ReflectedType> {
        self.types_by_id.values()
    }

    #[must_use]
    pub fn component_type_ids_by_name(&self) -> BTreeMap<String, Uuid> {
        let mut evidence = BTreeMap::new();
        for ty in self.types_by_id.values() {
            if ty.role.is_az_component_like() {
                evidence.insert(ty.name.clone(), ty.type_id);
            }
        }
        for descriptor in self.component_descriptors_by_id.values() {
            if self.type_ids_by_name.contains_key(&descriptor.name) {
                evidence
                    .entry(descriptor.name.clone())
                    .or_insert(descriptor.type_id);
            }
        }
        evidence
    }

    #[must_use]
    pub fn descriptor_by_type_id(&self, type_id: Uuid) -> Option<&ComponentDescriptor> {
        self.component_descriptors_by_id.get(&type_id)
    }

    #[must_use]
    pub fn class_registration_by_type_id(
        &self,
        type_id: Uuid,
    ) -> Option<&ClassRegistrationTraceRecord> {
        self.class_registration_trace.record_by_type_id(type_id)
    }

    #[must_use]
    pub fn class_registration_trace(&self) -> &ClassRegistrationTraceIndex {
        &self.class_registration_trace
    }

    #[must_use]
    pub fn descriptor_type_id_by_name(&self, name: &str) -> Option<Uuid> {
        let type_ids = self.component_descriptor_ids_by_name.get(name)?;
        let [type_id] = type_ids.as_slice() else {
            return None;
        };
        Some(*type_id)
    }

    #[must_use]
    pub fn descriptor_type_ids_by_name(&self, name: &str) -> &[Uuid] {
        self.component_descriptor_ids_by_name
            .get(name)
            .map_or(&[], Vec::as_slice)
    }

    #[must_use]
    pub fn porting_ledger(&self) -> Option<&SerializePortingLedger> {
        self.porting_ledger.as_ref()
    }
}

fn read_json(path: &Path) -> Result<Value, ReflectedTypeCatalogError> {
    let bytes = fs::read(path).map_err(|source| ReflectedTypeCatalogError::Read {
        path: path.to_path_buf(),
        source,
    })?;
    serde_json::from_slice(&bytes).map_err(|source| ReflectedTypeCatalogError::Parse {
        path: path.to_path_buf(),
        source,
    })
}

fn read_optional_json(path: Option<&Path>) -> Result<Option<Value>, ReflectedTypeCatalogError> {
    path.map(read_json).transpose()
}

fn read_optional_module_descriptors(
    path: Option<&Path>,
    context: &CodegenContext,
) -> Result<Option<Value>, ReflectedTypeCatalogError> {
    path.map(|path| read_module_descriptors(path, context))
        .transpose()
}

fn read_module_descriptors(
    path: &Path,
    context: &CodegenContext,
) -> Result<Value, ReflectedTypeCatalogError> {
    if path.is_dir() {
        return read_module_descriptor_directory(path, context);
    }

    let root = read_json(path)?;
    Ok(module_descriptors_root_from_capture(
        module_name_from_path(path),
        root,
    ))
}

fn read_module_descriptor_directory(
    path: &Path,
    context: &CodegenContext,
) -> Result<Value, ReflectedTypeCatalogError> {
    let mut entries = fs::read_dir(path)
        .map_err(|source| ReflectedTypeCatalogError::Read {
            path: path.to_path_buf(),
            source,
        })?
        .map(|entry| {
            entry
                .map(|entry| entry.path())
                .map_err(|source| ReflectedTypeCatalogError::Read {
                    path: path.to_path_buf(),
                    source,
                })
        })
        .collect::<Result<Vec<_>, _>>()?;
    entries.retain(|path| {
        path.file_name()
            .and_then(|name| name.to_str())
            .is_some_and(is_module_descriptor_json_name)
    });
    entries.sort();

    let modules = context.runner().try_map(&entries, |entry| {
        read_json(entry).map(|root| module_descriptor_capture(module_name_from_path(entry), root))
    })?;

    Ok(module_descriptors_root(modules))
}

fn reflected_fields_from_model(
    class: &ModelClass,
    names_by_id: &BTreeMap<Uuid, String>,
    reflection_marker_type_ids: &std::collections::BTreeSet<Uuid>,
) -> Vec<ReflectedField> {
    class
        .members
        .iter()
        .map(|member| reflected_field_from_model(member, names_by_id, reflection_marker_type_ids))
        .collect()
}

fn reflected_field_from_model(
    member: &ModelMember,
    names_by_id: &BTreeMap<Uuid, String>,
    reflection_marker_type_ids: &std::collections::BTreeSet<Uuid>,
) -> ReflectedField {
    ReflectedField {
        name: member.name.clone(),
        name_crc: member.name_crc,
        type_id: member.type_id,
        type_name: names_by_id.get(&member.type_id).cloned(),
        data_size: member.data_size,
        offset: member.offset,
        flags: member.flags,
        is_base_class: member.is_base_class,
        is_reflection_marker_base_class: member.is_base_class
            && reflection_marker_type_ids.contains(&member.type_id),
        is_pointer: member.is_pointer,
        is_dynamic_field: member.is_dynamic_field,
    }
}

fn build_model_generic_type(
    type_id: Uuid,
    model: &SerializeContextModel,
    class_names_by_id: &BTreeMap<Uuid, String>,
    visiting: &mut Vec<Uuid>,
) -> Option<ReflectedGenericType> {
    if visiting.contains(&type_id) {
        return None;
    }
    visiting.push(type_id);

    let Some(generic) = model.generic_class(type_id) else {
        visiting.pop();
        return None;
    };
    let Some(base_name) = generic.class_name.clone() else {
        visiting.pop();
        return None;
    };
    let argument_type_ids = generic.argument_type_ids();
    let display_name = model_generic_display_name(
        &base_name,
        &argument_type_ids,
        model,
        class_names_by_id,
        visiting,
    );
    let non_type_capacity = generic.non_type_capacity();

    visiting.pop();
    Some(ReflectedGenericType {
        type_id,
        base_name,
        argument_type_ids,
        non_type_capacity,
        display_name,
        resolved_type: TypeResolver::new(model).resolve(type_id),
    })
}

fn model_generic_display_name(
    base_name: &str,
    argument_type_ids: &[Uuid],
    model: &SerializeContextModel,
    class_names_by_id: &BTreeMap<Uuid, String>,
    visiting: &mut Vec<Uuid>,
) -> String {
    if matches!(base_name, "AZStd::basic_string" | "AZStd::string") {
        return "AZStd::string".to_owned();
    }
    if argument_type_ids.is_empty() {
        return base_name.to_owned();
    }

    let arguments = argument_type_ids
        .iter()
        .map(|type_id| {
            class_names_by_id
                .get(type_id)
                .cloned()
                .or_else(|| {
                    build_model_generic_type(*type_id, model, class_names_by_id, visiting)
                        .map(|generic| generic.display_name)
                })
                .unwrap_or_else(|| format!("Type{}", short_type_id(*type_id)))
        })
        .collect::<Vec<_>>();

    format!("{base_name}<{}>", arguments.join(", "))
}

fn parse_component_descriptors(
    root: &Value,
) -> (
    BTreeMap<Uuid, ComponentDescriptor>,
    BTreeMap<String, Vec<Uuid>>,
) {
    let mut by_id = BTreeMap::new();
    let mut by_name = BTreeMap::new();

    if let Some(modules) = root.get("modules").and_then(Value::as_array) {
        for module in modules {
            parse_component_descriptor_module(module, &mut by_id, &mut by_name);
        }
    } else {
        parse_component_descriptor_module(root, &mut by_id, &mut by_name);
    }

    for type_ids in by_name.values_mut() {
        type_ids.sort_unstable();
        type_ids.dedup();
    }

    (by_id, by_name)
}

fn parse_component_descriptor_module(
    module: &Value,
    by_id: &mut BTreeMap<Uuid, ComponentDescriptor>,
    by_name: &mut BTreeMap<String, Vec<Uuid>>,
) {
    let module_name = module
        .get("moduleName")
        .or_else(|| module.get("name"))
        .and_then(non_empty_str)
        .map(str::to_owned);
    let Some(descriptors) = module.get("descriptors").and_then(Value::as_array) else {
        return;
    };

    for descriptor in descriptors {
        let Some(name) = descriptor.get("componentName").and_then(non_empty_str) else {
            continue;
        };
        let Some(type_id) = descriptor
            .get("componentUuid")
            .and_then(non_empty_str)
            .and_then(parse_uuid)
        else {
            continue;
        };
        by_name.entry(name.to_owned()).or_default().push(type_id);
        by_id.entry(type_id).or_insert_with(|| ComponentDescriptor {
            name: name.to_owned(),
            type_id,
            module_name: module_name.clone(),
            vtable: descriptor
                .get("vftable")
                .and_then(non_empty_str)
                .map(str::to_owned),
            address: descriptor
                .get("addr")
                .and_then(non_empty_str)
                .map(str::to_owned),
            vtable_slots: parse_component_descriptor_vtable_slots(descriptor),
        });
    }
}

fn parse_component_descriptor_vtable_slots(
    descriptor: &Value,
) -> Vec<ComponentDescriptorVtableSlot> {
    let Some(slots) = descriptor.get("vtableSlots").and_then(Value::as_array) else {
        return Vec::new();
    };

    slots
        .iter()
        .filter_map(|slot| {
            let slot_index = slot.get("slot").and_then(value_as_u32)?;
            Some(ComponentDescriptorVtableSlot {
                slot: slot_index,
                expected: slot
                    .get("expected")
                    .and_then(non_empty_str)
                    .map(str::to_owned),
                address: slot
                    .get("address")
                    .and_then(non_empty_str)
                    .map(str::to_owned),
            })
        })
        .collect()
}

fn parse_serialize_porting_ledger(root: &Value) -> SerializePortingLedger {
    let mut ledger = SerializePortingLedger::default();

    if let Some(components) = root.get("components").and_then(Value::as_array) {
        for component in components {
            let (Some(name), Some(module)) = (
                component.get("component").and_then(non_empty_str),
                component.get("module").and_then(non_empty_str),
            ) else {
                continue;
            };
            ledger
                .component_modules_by_name
                .insert(name.to_owned(), PathBuf::from(module));
        }
    }

    if let Some(support_owners) = root.get("support_owners").and_then(Value::as_array) {
        for owner in support_owners {
            let Some(module) = owner.get("module").and_then(non_empty_str) else {
                continue;
            };
            let Some(owned_types) = owner.get("owned_types").and_then(Value::as_array) else {
                continue;
            };
            for ty in owned_types {
                let Some(name) = ty.as_str().filter(|value| !value.is_empty()) else {
                    continue;
                };
                ledger
                    .support_modules_by_type_name
                    .insert(name.to_owned(), PathBuf::from(module));
            }
        }
    }

    ledger
}

fn primitive_names() -> BTreeMap<Uuid, String> {
    [
        (type_ids::CHAR, "char"),
        (type_ids::SIGNED_CHAR, "signed char"),
        (type_ids::S8, "s8"),
        (type_ids::U8, "u8"),
        (type_ids::SHORT, "s16"),
        (type_ids::U16, "u16"),
        (type_ids::INT, "s32"),
        (type_ids::U32, "u32"),
        (type_ids::LONG, "s64"),
        (type_ids::S64, "s64"),
        (type_ids::U64, "u64"),
        (type_ids::ULONG, "unsigned long"),
        (type_ids::FLOAT, "float"),
        (type_ids::DOUBLE, "double"),
        (type_ids::BOOL, "bool"),
        (type_ids::AZ_UUID, "AZ::Uuid"),
        (type_ids::CRC32, "AZ::Crc32"),
        (type_ids::ENTITY_ID, "AZ::EntityId"),
        (type_ids::AZ_DATA_ASSET_ID, "AZ::Data::AssetId"),
        (type_ids::AZSTD_STRING, "AZStd::string"),
    ]
    .into_iter()
    .map(|(uuid, name)| (uuid, name.to_owned()))
    .collect()
}

fn non_empty_str(value: &Value) -> Option<&str> {
    value.as_str().filter(|value| !value.is_empty())
}

fn value_as_u32(value: &Value) -> Option<u32> {
    let value = value.as_u64()?;
    u32::try_from(value).ok()
}

fn parse_uuid(value: &str) -> Option<Uuid> {
    Uuid::parse_str(value.trim_matches(['{', '}'])).ok()
}

fn short_type_id(type_id: Uuid) -> String {
    type_id
        .simple()
        .to_string()
        .chars()
        .take(8)
        .collect::<String>()
        .to_ascii_uppercase()
}
