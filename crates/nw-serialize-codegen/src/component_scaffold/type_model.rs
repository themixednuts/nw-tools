use std::collections::{BTreeMap, BTreeSet};

use nw_objectstream::{type_uuid::type_ids, types::COMPONENT_ID_VECTOR};
use uuid::Uuid;

use crate::{
    ReflectedField, ReflectedGenericType, ReflectedType, ReflectedTypeCatalog,
    RustDeriveCapabilities, RustSourceTypeIndex, rust_reflected_type_name as rust_type_name,
};

use super::naming::{
    base_class_field_name, is_facet_pointer_field, is_facet_type_name, module_name_for_component,
    pascal_case, snake_field_name,
};

const AZ_S8: Uuid = type_ids::S8;
const AZ_S64: Uuid = type_ids::S64;
const UNSIGNED_CHAR: Uuid = type_ids::U8;
const UNSIGNED_SHORT: Uuid = type_ids::U16;
const UNSIGNED_INT: Uuid = type_ids::U32;
const UNSIGNED_LONG: Uuid = type_ids::ULONG;
const AZ_U64: Uuid = type_ids::U64;
const ASSET: Uuid = type_ids::AZ_DATA_ASSET_REFLECTION;
const ASSET_ID: Uuid = type_ids::AZ_DATA_ASSET_ID;

#[derive(Debug, Clone)]
pub(super) struct RustField {
    pub(super) name: String,
    pub(super) ty: String,
    pub(super) nested_structs: Vec<NestedStruct>,
    pub(super) maps_entities: bool,
    pub(super) derive_capabilities: DeriveCapabilities,
}

#[derive(Debug, Clone)]
pub(super) struct NestedStruct {
    pub(super) name: String,
    pub(super) type_id: Uuid,
    pub(super) fields: Vec<RustField>,
    pub(super) maps_entities: bool,
    pub(super) derive_capabilities: DeriveCapabilities,
}

#[derive(Debug)]
pub(super) struct RustTypeContext<'a> {
    pub(super) component_name: String,
    pub(super) source_types: &'a RustSourceTypeIndex,
    pub(super) used_type_names: BTreeSet<String>,
    pub(super) support_types: BTreeMap<Uuid, String>,
    pub(super) visiting_type_ids: Vec<Uuid>,
    pub(super) nested_structs: Vec<NestedStruct>,
}

impl<'a> RustTypeContext<'a> {
    pub(super) fn new(component_name: &str, source_types: &'a RustSourceTypeIndex) -> Self {
        let mut used_type_names = BTreeSet::new();
        used_type_names.insert(component_name.to_owned());
        Self {
            component_name: component_name.to_owned(),
            source_types,
            used_type_names,
            support_types: BTreeMap::new(),
            visiting_type_ids: Vec::new(),
            nested_structs: Vec::new(),
        }
    }

    fn unique_nested_name(&mut self, name: &str) -> String {
        let mut base = pascal_case(name);
        if base.is_empty() {
            base = format!("{}Field", self.component_name);
        }
        if self.used_type_names.insert(base.clone()) {
            return base;
        }

        for index in 2.. {
            let candidate = format!("{base}{index}");
            if self.used_type_names.insert(candidate.clone()) {
                return candidate;
            }
        }

        unreachable!("unbounded type-name suffix search")
    }
}

#[derive(Debug, Clone)]
struct RustType {
    pub(super) ty: String,
    pub(super) maps_entities: bool,
    pub(super) derive_capabilities: DeriveCapabilities,
}

pub(super) type DeriveCapabilities = RustDeriveCapabilities;

pub(super) fn rust_field_from_reflected_field(
    field: &ReflectedField,
    catalog: &ReflectedTypeCatalog,
    context: &mut RustTypeContext<'_>,
) -> RustField {
    if field.is_base_class {
        let nested_start = context.nested_structs.len();
        let ty = rust_type_from_reflected_type(&field.name, field.type_id, catalog, context);
        let nested_structs = context.nested_structs[nested_start..].to_vec();
        return RustField {
            name: base_class_field_name(field),
            ty: ty.ty,
            nested_structs,
            maps_entities: ty.maps_entities,
            derive_capabilities: ty.derive_capabilities,
        };
    }

    if is_facet_pointer_field(&field.name)
        && let Some(schema_name) = field.type_name.as_deref()
    {
        let facet_name = rust_type_name(schema_name, field.type_id);
        if is_facet_type_name(&facet_name) {
            let name = match field.name.as_str() {
                "m_clientFacetPtr" => "client_facet",
                "m_serverFacetPtr" => "server_facet",
                _ => "facet",
            };
            return RustField {
                name: name.to_owned(),
                ty: facet_name,
                nested_structs: Vec::new(),
                maps_entities: false,
                derive_capabilities: DeriveCapabilities::NONE,
            };
        }
    }

    let nested_start = context.nested_structs.len();
    let ty = rust_type_from_reflected_type(&field.name, field.type_id, catalog, context);
    let nested_structs = context.nested_structs[nested_start..].to_vec();
    RustField {
        name: snake_field_name(&field.name),
        ty: ty.ty,
        nested_structs,
        maps_entities: ty.maps_entities,
        derive_capabilities: ty.derive_capabilities,
    }
}

fn rust_type_from_reflected_type(
    legacy_name: &str,
    type_id: Uuid,
    catalog: &ReflectedTypeCatalog,
    context: &mut RustTypeContext<'_>,
) -> RustType {
    if type_id == type_ids::BOOL {
        return copy_eq_rust_type("bool");
    }
    if matches!(
        type_id,
        type_ids::CHAR
            | type_ids::SIGNED_CHAR
            | AZ_S8
            | type_ids::SHORT
            | type_ids::INT
            | type_ids::LONG
            | AZ_S64
    ) {
        return copy_eq_rust_type(signed_integer_type(type_id));
    }
    if matches!(
        type_id,
        UNSIGNED_CHAR | UNSIGNED_SHORT | UNSIGNED_INT | UNSIGNED_LONG | AZ_U64 | type_ids::CRC32
    ) {
        return copy_eq_rust_type(unsigned_integer_type(type_id));
    }
    if matches!(
        type_id,
        type_ids::FLOAT | type_ids::VECTOR_FLOAT | type_ids::DOUBLE
    ) {
        return copy_rust_type(float_type(type_id));
    }
    if matches!(
        type_id,
        type_ids::VECTOR2
            | type_ids::VECTOR3
            | type_ids::VECTOR4
            | type_ids::COLOR
            | type_ids::COLORF
            | type_ids::QUATERNION
            | type_ids::TRANSFORM
    ) {
        return copy_rust_type(vector_type(type_id, 0));
    }
    if type_id == type_ids::AZ_UUID {
        return copy_eq_rust_type("uuid::Uuid");
    }
    if type_id == type_ids::AZSTD_STRING {
        return eq_rust_type("String");
    }
    if type_id == COMPONENT_ID_VECTOR {
        return eq_rust_type("Vec<az_core::ComponentId>");
    }
    if matches!(type_id, ASSET | ASSET_ID) {
        return eq_rust_type("az_asset::UntypedAssetRef");
    }

    if let Some(generic) = catalog.generic_type(type_id) {
        return rust_type_from_generic(legacy_name, generic, catalog, context);
    }

    let type_name = catalog.type_name(type_id).unwrap_or_default();
    if matches!(type_name, "EntityId" | "AZ::EntityId") || type_id == type_ids::ENTITY_ID {
        return entity_ref_type();
    }
    if type_name == "LocalEntityRef" || type_name.starts_with("LocalComponentRef<") {
        return entity_ref_type();
    }
    if type_name.starts_with("AzFramework::SimpleAssetReference")
        || type_name.contains("AssetReference")
    {
        return eq_rust_type("az_asset::UntypedAssetRef");
    }
    if type_name == "GDEID" {
        return copy_eq_rust_type("crate::generated::GDEID");
    }
    if type_name == "Amazon::Pervasives::UID" {
        return copy_eq_rust_type("crate::refs::Uid");
    }
    if type_name == "AZStd::ranged_int" {
        return rust_type_from_reflected_type(legacy_name, AZ_U64, catalog, context);
    }
    if type_name == "RemoteServerContextRef" {
        return copy_eq_rust_type("crate::generated::RemoteServerContextRef");
    }
    if type_name == "RemoteServerGDERef" {
        return copy_eq_rust_type("crate::generated::RemoteServerGDERef");
    }
    if type_name == "RemoteServerEntityRef" {
        return copy_eq_rust_type("crate::generated::RemoteServerEntityRef");
    }
    if type_name == "RemoteTypelessServerFacetRef" {
        return copy_eq_rust_type("crate::generated::RemoteTypelessServerFacetRef");
    }

    if let Some(reflected) = catalog.type_by_id(type_id)
        && reflected.is_support_type()
    {
        return rust_type_from_support_type(reflected, catalog, context);
    }

    let name = rust_type_name(type_name, type_id);
    rust_type(name)
}

fn rust_type_from_generic(
    legacy_name: &str,
    generic: &ReflectedGenericType,
    catalog: &ReflectedTypeCatalog,
    context: &mut RustTypeContext<'_>,
) -> RustType {
    match generic.base_name.as_str() {
        "AZStd::string" | "AZStd::basic_string" => eq_rust_type("String"),
        "AZStd::vector" | "AZStd::list" | "AZStd::forward_list" | "AZStd::fixed_vector" => {
            let element_type = generic
                .argument_type_ids
                .first()
                .map(|type_id| {
                    rust_type_from_reflected_type(legacy_name, *type_id, catalog, context)
                })
                .unwrap_or_else(|| rust_type("()"));
            RustType {
                ty: format!("Vec<{}>", element_type.ty),
                maps_entities: element_type.maps_entities,
                derive_capabilities: DeriveCapabilities {
                    copy: false,
                    eq: element_type.derive_capabilities.eq,
                },
            }
        }
        "AZStd::set" | "AZStd::unordered_set" | "AZStd::unordered_flat_set" => {
            let element_type = generic
                .argument_type_ids
                .first()
                .map(|type_id| {
                    rust_type_from_reflected_type(legacy_name, *type_id, catalog, context)
                })
                .unwrap_or_else(|| rust_type("()"));
            RustType {
                ty: format!("std::collections::BTreeSet<{}>", element_type.ty),
                maps_entities: element_type.maps_entities,
                derive_capabilities: DeriveCapabilities {
                    copy: false,
                    eq: element_type.derive_capabilities.eq,
                },
            }
        }
        "AZStd::optional" => {
            let element_type = generic
                .argument_type_ids
                .first()
                .map(|type_id| {
                    rust_type_from_reflected_type(legacy_name, *type_id, catalog, context)
                })
                .unwrap_or_else(|| rust_type("()"));
            RustType {
                ty: format!("Option<{}>", element_type.ty),
                maps_entities: element_type.maps_entities,
                derive_capabilities: element_type.derive_capabilities,
            }
        }
        "AZStd::shared_ptr" | "AZStd::unique_ptr" | "AZStd::intrusive_ptr" => {
            let element_type = generic
                .argument_type_ids
                .first()
                .map(|type_id| {
                    rust_type_from_reflected_type(legacy_name, *type_id, catalog, context)
                })
                .unwrap_or_else(|| rust_type("()"));
            RustType {
                ty: format!("Option<{}>", element_type.ty),
                maps_entities: element_type.maps_entities,
                derive_capabilities: element_type.derive_capabilities,
            }
        }
        "Amazon::Pervasives::UID" => copy_eq_rust_type("crate::refs::Uid"),
        "AZStd::ranged_int" => generic
            .argument_type_ids
            .first()
            .map(|type_id| rust_type_from_reflected_type(legacy_name, *type_id, catalog, context))
            .unwrap_or_else(|| rust_type("u64")),
        "AZStd::array" => {
            let element_type = generic
                .argument_type_ids
                .first()
                .map(|type_id| {
                    rust_type_from_reflected_type(legacy_name, *type_id, catalog, context)
                })
                .unwrap_or_else(|| rust_type("()"));
            if let Some(capacity) = generic.non_type_capacity {
                RustType {
                    ty: format!("[{}; {capacity}]", element_type.ty),
                    maps_entities: element_type.maps_entities,
                    derive_capabilities: element_type.derive_capabilities,
                }
            } else {
                RustType {
                    ty: format!("Vec<{}>", element_type.ty),
                    maps_entities: element_type.maps_entities,
                    derive_capabilities: DeriveCapabilities {
                        copy: false,
                        eq: element_type.derive_capabilities.eq,
                    },
                }
            }
        }
        "AZStd::pair" => {
            let first = generic
                .argument_type_ids
                .first()
                .map(|type_id| {
                    rust_type_from_reflected_type(legacy_name, *type_id, catalog, context)
                })
                .unwrap_or_else(|| rust_type("()"));
            let second = generic
                .argument_type_ids
                .get(1)
                .map(|type_id| {
                    rust_type_from_reflected_type(legacy_name, *type_id, catalog, context)
                })
                .unwrap_or_else(|| rust_type("()"));
            RustType {
                ty: format!("({}, {})", first.ty, second.ty),
                maps_entities: first.maps_entities || second.maps_entities,
                derive_capabilities: DeriveCapabilities {
                    copy: first.derive_capabilities.copy && second.derive_capabilities.copy,
                    eq: first.derive_capabilities.eq && second.derive_capabilities.eq,
                },
            }
        }
        "AZStd::map" | "AZStd::unordered_map" | "AZStd::unordered_flat_map" => {
            let key = generic
                .argument_type_ids
                .first()
                .map(|type_id| {
                    rust_type_from_reflected_type(legacy_name, *type_id, catalog, context)
                })
                .unwrap_or_else(|| rust_type("()"));
            let value = generic
                .argument_type_ids
                .get(1)
                .map(|type_id| {
                    rust_type_from_reflected_type(legacy_name, *type_id, catalog, context)
                })
                .unwrap_or_else(|| rust_type("()"));
            RustType {
                ty: format!("std::collections::BTreeMap<{}, {}>", key.ty, value.ty),
                maps_entities: key.maps_entities || value.maps_entities,
                derive_capabilities: DeriveCapabilities {
                    copy: false,
                    eq: key.derive_capabilities.eq && value.derive_capabilities.eq,
                },
            }
        }
        _ => rust_type(rust_type_name(&generic.display_name, generic.type_id)),
    }
}

fn rust_type_from_support_type(
    reflected: &ReflectedType,
    catalog: &ReflectedTypeCatalog,
    context: &mut RustTypeContext<'_>,
) -> RustType {
    let rust_name = rust_type_name(&reflected.name, reflected.type_id);
    if let Some(reference) = support_type_reference(&rust_name, reflected.type_id, context) {
        return reference;
    }

    if let Some(name) = context.support_types.get(&reflected.type_id) {
        let maps_entities = context
            .nested_structs
            .iter()
            .find(|nested| nested.name == *name)
            .is_some_and(|nested| nested.maps_entities);
        return RustType {
            ty: name.clone(),
            maps_entities,
            derive_capabilities: context
                .nested_structs
                .iter()
                .find(|nested| nested.name == *name)
                .map(|nested| nested.derive_capabilities)
                .unwrap_or(DeriveCapabilities::NONE),
        };
    }

    let rust_name = context.unique_nested_name(&rust_name);
    context
        .support_types
        .insert(reflected.type_id, rust_name.clone());
    if context.visiting_type_ids.contains(&reflected.type_id) {
        return RustType {
            ty: rust_name,
            maps_entities: false,
            derive_capabilities: DeriveCapabilities::NONE,
        };
    }

    context.visiting_type_ids.push(reflected.type_id);
    let mut fields = reflected
        .serializable_fields()
        .map(|field| rust_field_from_reflected_field(field, catalog, context))
        .collect::<Vec<_>>();
    context.visiting_type_ids.pop();
    dedupe_rust_fields(&mut fields);
    let maps_entities = rust_fields_map_entities(&fields);
    let derive_capabilities = derive_capabilities_for_fields(&fields);
    context.nested_structs.push(NestedStruct {
        name: rust_name.clone(),
        type_id: reflected.type_id,
        fields,
        maps_entities,
        derive_capabilities,
    });
    RustType {
        ty: rust_name,
        maps_entities,
        derive_capabilities,
    }
}

fn support_type_reference(
    rust_name: &str,
    type_id: Uuid,
    context: &RustTypeContext<'_>,
) -> Option<RustType> {
    let current_module = module_name_for_component(&context.component_name);
    let location = context
        .source_types
        .location_for(rust_name, type_id, &current_module)?;
    Some(RustType {
        ty: location.reference_from(&current_module),
        maps_entities: location.maps_entities,
        derive_capabilities: location.derive_capabilities,
    })
}

fn rust_type(value: impl Into<String>) -> RustType {
    RustType {
        ty: value.into(),
        maps_entities: false,
        derive_capabilities: DeriveCapabilities::NONE,
    }
}

fn entity_ref_type() -> RustType {
    RustType {
        ty: "Option<bevy::ecs::entity::Entity>".to_owned(),
        maps_entities: true,
        derive_capabilities: DeriveCapabilities::COPY_EQ,
    }
}

fn copy_rust_type(value: impl Into<String>) -> RustType {
    let mut ty = rust_type(value);
    ty.derive_capabilities = DeriveCapabilities::COPY_ONLY;
    ty
}

fn eq_rust_type(value: impl Into<String>) -> RustType {
    let mut ty = rust_type(value);
    ty.derive_capabilities = DeriveCapabilities::EQ_ONLY;
    ty
}

fn copy_eq_rust_type(value: impl Into<String>) -> RustType {
    let mut ty = rust_type(value);
    ty.derive_capabilities = DeriveCapabilities::COPY_EQ;
    ty
}

fn signed_integer_type(type_id: Uuid) -> String {
    match type_id {
        type_ids::CHAR | type_ids::SIGNED_CHAR | AZ_S8 => "i8",
        type_ids::SHORT => "i16",
        type_ids::INT => "i32",
        _ => "i64",
    }
    .to_owned()
}

fn unsigned_integer_type(type_id: Uuid) -> String {
    match type_id {
        UNSIGNED_CHAR => "u8",
        UNSIGNED_SHORT => "u16",
        UNSIGNED_INT | type_ids::CRC32 => "u32",
        _ => "u64",
    }
    .to_owned()
}

fn float_type(type_id: Uuid) -> String {
    match type_id {
        type_ids::FLOAT | type_ids::VECTOR_FLOAT => "f32",
        _ => "f64",
    }
    .to_owned()
}

fn vector_type(type_id: Uuid, len: usize) -> String {
    match type_id {
        type_ids::VECTOR2 => "bevy::prelude::Vec2",
        type_ids::VECTOR3 => "bevy::prelude::Vec3",
        type_ids::VECTOR4 | type_ids::COLOR | type_ids::COLORF => "bevy::prelude::Vec4",
        type_ids::QUATERNION => "bevy::prelude::Quat",
        type_ids::TRANSFORM => "bevy::prelude::Transform",
        _ if len == 2 => "[f64; 2]",
        _ if len == 3 => "[f64; 3]",
        _ if len == 4 => "[f64; 4]",
        _ => "Vec<f64>",
    }
    .to_owned()
}

pub(super) fn dedupe_rust_fields(fields: &mut [RustField]) {
    let mut used = BTreeSet::new();
    for field in fields {
        field.name = unique_field_name(&mut used, &field.name);
    }
}

pub(super) fn rust_fields_map_entities(fields: &[RustField]) -> bool {
    fields.iter().any(|field| field.maps_entities)
}

pub(super) fn derive_capabilities_for_fields(fields: &[RustField]) -> DeriveCapabilities {
    DeriveCapabilities {
        copy: fields.iter().all(|field| field.derive_capabilities.copy),
        eq: fields.iter().all(|field| field.derive_capabilities.eq),
    }
}

pub(super) fn unique_field_name(used: &mut BTreeSet<String>, name: &str) -> String {
    if used.insert(name.to_owned()) {
        return name.to_owned();
    }

    for index in 2.. {
        let candidate = format!("{name}_{index}");
        if used.insert(candidate.clone()) {
            return candidate;
        }
    }

    unreachable!("unbounded field-name suffix search")
}
