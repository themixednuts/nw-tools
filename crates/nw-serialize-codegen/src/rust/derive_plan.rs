use std::collections::{BTreeMap, BTreeSet};

use uuid::Uuid;

use crate::field_projection::{
    CodegenFieldTypeProjection, classify_codegen_field, classify_codegen_field_type,
};
use crate::ir::{SerializeCodegenField, SerializeCodegenItem, SerializeCodegenItemKind};
use crate::rust::identity::RustTypeIdentityKind;
use crate::rust::options::RustCodegenMode;
use crate::types::{MapKind, ResolvedType, ScalarType, SequenceKind};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct LocalTypeTraits {
    default: bool,
    copy: bool,
    partial_eq: bool,
    eq: bool,
    ord: bool,
    hash: bool,
    reflect: bool,
    marshaler: bool,
    serde: bool,
}

impl LocalTypeTraits {
    const fn value() -> Self {
        Self {
            default: true,
            copy: true,
            partial_eq: true,
            eq: true,
            ord: true,
            hash: true,
            reflect: true,
            marshaler: true,
            serde: true,
        }
    }

    const fn custom_field_payload() -> Self {
        Self {
            default: true,
            copy: false,
            partial_eq: true,
            eq: false,
            ord: false,
            hash: false,
            reflect: false,
            marshaler: true,
            serde: false,
        }
    }
}

fn local_type_traits(source_name: &str) -> Option<LocalTypeTraits> {
    match source_name {
        "Amazon::Hub::ActorRef" => Some(LocalTypeTraits::value()),
        "CritWindow" | "HomePoint" | "HomePointList" => {
            Some(LocalTypeTraits::custom_field_payload())
        }
        _ => None,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct RustDerivePlanner {
    mode: RustCodegenMode,
}

pub(super) struct RustDeriveCaches<'a> {
    pub eq: &'a mut BTreeMap<Uuid, bool>,
    pub default: &'a mut BTreeMap<Uuid, bool>,
    pub hash: &'a mut BTreeMap<Uuid, bool>,
    pub copy: &'a mut BTreeMap<Uuid, bool>,
    pub marshaler: &'a mut BTreeMap<Uuid, bool>,
    pub serde: &'a mut BTreeMap<Uuid, bool>,
}

impl RustDerivePlanner {
    pub(super) const fn new(mode: RustCodegenMode) -> Self {
        Self { mode }
    }

    pub(super) fn plan_struct_derives(
        &self,
        is_bevy_component: bool,
        identity_kind: RustTypeIdentityKind,
        item: &SerializeCodegenItem,
        items_by_type_id: &BTreeMap<Uuid, &SerializeCodegenItem>,
        caches: &mut RustDeriveCaches<'_>,
    ) -> Vec<String> {
        let supports_partial_eq = item_supports_partial_eq(
            item,
            items_by_type_id,
            &mut BTreeMap::new(),
            &mut BTreeSet::new(),
            self.mode,
        );
        let supports_eq = item_supports_eq(
            item,
            items_by_type_id,
            &mut *caches.eq,
            &mut BTreeSet::new(),
            self.mode,
        );
        let supports_ord = item_supports_ord(
            item,
            items_by_type_id,
            &mut BTreeMap::new(),
            &mut BTreeSet::new(),
        );
        let supports_default = item_supports_default(
            item,
            items_by_type_id,
            &mut *caches.default,
            &mut BTreeSet::new(),
            self.mode,
        );
        let supports_hash = item_supports_hash(
            item,
            items_by_type_id,
            &mut *caches.hash,
            &mut BTreeSet::new(),
            self.mode,
        );
        let supports_copy = item_supports_copy(
            item,
            items_by_type_id,
            &mut *caches.copy,
            &mut BTreeSet::new(),
            self.mode,
        );
        let supports_reflect = item_supports_reflect(
            item,
            items_by_type_id,
            &mut BTreeMap::new(),
            &mut BTreeSet::new(),
            self.mode,
        );
        let supports_marshaler = matches!(self.mode, RustCodegenMode::Integrated)
            && item_supports_marshaler(
                item,
                items_by_type_id,
                &mut *caches.marshaler,
                &mut BTreeSet::new(),
            );
        let supports_serde = item_supports_serde(
            item,
            items_by_type_id,
            &mut *caches.serde,
            &mut BTreeSet::new(),
            self.mode,
        );

        match self.mode {
            RustCodegenMode::Integrated => {
                let mut derives = Vec::new();
                if is_bevy_component {
                    derives.push("Component".to_owned());
                }
                derives.push(identity_kind.integrated_derive_name().to_owned());
                derives.push("Debug".to_owned());
                if supports_default {
                    derives.push("Default".to_owned());
                }
                derives.push("Clone".to_owned());
                if supports_copy {
                    derives.push("Copy".to_owned());
                }
                if supports_partial_eq {
                    derives.push("PartialEq".to_owned());
                }
                if supports_eq {
                    derives.push("Eq".to_owned());
                }
                if supports_eq && supports_ord {
                    derives.push("PartialOrd".to_owned());
                    derives.push("Ord".to_owned());
                }
                if supports_hash {
                    derives.push("Hash".to_owned());
                }
                if supports_marshaler {
                    derives.push("Marshaler".to_owned());
                }
                if supports_serde {
                    derives.push("Serialize".to_owned());
                    derives.push("Deserialize".to_owned());
                }
                if supports_reflect {
                    derives.push("Reflect".to_owned());
                }
                derives
            }
            RustCodegenMode::Standalone => {
                let mut derives = vec!["Debug".to_owned()];
                if is_bevy_component {
                    derives.insert(0, "Component".to_owned());
                }
                if supports_default {
                    derives.push("Default".to_owned());
                }
                derives.push("Clone".to_owned());
                if supports_copy {
                    derives.push("Copy".to_owned());
                }
                if supports_partial_eq {
                    derives.push("PartialEq".to_owned());
                }
                if supports_eq {
                    derives.push("Eq".to_owned());
                }
                if supports_eq && supports_ord {
                    derives.push("PartialOrd".to_owned());
                    derives.push("Ord".to_owned());
                }
                if supports_hash {
                    derives.push("Hash".to_owned());
                }
                if supports_serde {
                    derives.push("Serialize".to_owned());
                    derives.push("Deserialize".to_owned());
                }
                if supports_reflect {
                    derives.push("Reflect".to_owned());
                }
                derives
            }
        }
    }

    pub(super) fn plan_enum_derives(&self, item: &SerializeCodegenItem) -> Vec<String> {
        let supports_default = !item.variants.is_empty();
        match self.mode {
            RustCodegenMode::Integrated => {
                let mut derives = vec!["AzTypeInfo".to_owned(), "Debug".to_owned()];
                if supports_default {
                    derives.push("Default".to_owned());
                }
                derives.extend([
                    "Clone".to_owned(),
                    "Copy".to_owned(),
                    "PartialEq".to_owned(),
                    "Eq".to_owned(),
                    "PartialOrd".to_owned(),
                    "Ord".to_owned(),
                    "Hash".to_owned(),
                    "Marshaler".to_owned(),
                    "Serialize".to_owned(),
                    "Deserialize".to_owned(),
                    "Reflect".to_owned(),
                ]);
                derives
            }
            RustCodegenMode::Standalone => {
                let mut derives = vec!["Debug".to_owned()];
                if supports_default {
                    derives.push("Default".to_owned());
                }
                derives.extend([
                    "Clone".to_owned(),
                    "Copy".to_owned(),
                    "PartialEq".to_owned(),
                    "Eq".to_owned(),
                    "PartialOrd".to_owned(),
                    "Ord".to_owned(),
                    "Hash".to_owned(),
                    "Serialize".to_owned(),
                    "Deserialize".to_owned(),
                    "Reflect".to_owned(),
                ]);
                derives
            }
        }
    }

    pub(super) fn plan_raw_enum_derives(&self) -> Vec<String> {
        match self.mode {
            RustCodegenMode::Integrated => vec![
                "AzTypeInfo".to_owned(),
                "Debug".to_owned(),
                "Default".to_owned(),
                "Clone".to_owned(),
                "Copy".to_owned(),
                "PartialEq".to_owned(),
                "Eq".to_owned(),
                "PartialOrd".to_owned(),
                "Ord".to_owned(),
                "Hash".to_owned(),
                "Marshaler".to_owned(),
                "Serialize".to_owned(),
                "Deserialize".to_owned(),
                "Reflect".to_owned(),
            ],
            RustCodegenMode::Standalone => vec![
                "Debug".to_owned(),
                "Default".to_owned(),
                "Clone".to_owned(),
                "Copy".to_owned(),
                "PartialEq".to_owned(),
                "Eq".to_owned(),
                "PartialOrd".to_owned(),
                "Ord".to_owned(),
                "Hash".to_owned(),
                "Serialize".to_owned(),
                "Deserialize".to_owned(),
                "Reflect".to_owned(),
            ],
        }
    }
}

fn item_supports_copy(
    item: &SerializeCodegenItem,
    items_by_type_id: &BTreeMap<Uuid, &SerializeCodegenItem>,
    cache: &mut BTreeMap<Uuid, bool>,
    visiting: &mut BTreeSet<Uuid>,
    mode: RustCodegenMode,
) -> bool {
    if let Some(supports_copy) = cache.get(&item.source_type_id) {
        return *supports_copy;
    }
    if !visiting.insert(item.source_type_id) {
        return false;
    }

    let supports_copy = match item.kind {
        SerializeCodegenItemKind::Enum => true,
        SerializeCodegenItemKind::Struct => item
            .fields
            .iter()
            .filter(|field| should_materialize_rust_field(field, items_by_type_id))
            .all(|field| {
                resolved_type_supports_copy(
                    &field.resolved_type,
                    items_by_type_id,
                    cache,
                    visiting,
                    mode,
                )
            }),
    };
    visiting.remove(&item.source_type_id);
    cache.insert(item.source_type_id, supports_copy);
    supports_copy
}

fn item_supports_partial_eq(
    item: &SerializeCodegenItem,
    items_by_type_id: &BTreeMap<Uuid, &SerializeCodegenItem>,
    cache: &mut BTreeMap<Uuid, bool>,
    visiting: &mut BTreeSet<Uuid>,
    mode: RustCodegenMode,
) -> bool {
    if let Some(supports_partial_eq) = cache.get(&item.source_type_id) {
        return *supports_partial_eq;
    }
    if !visiting.insert(item.source_type_id) {
        return false;
    }

    let supports_partial_eq = match item.kind {
        SerializeCodegenItemKind::Enum => true,
        SerializeCodegenItemKind::Struct => item
            .fields
            .iter()
            .filter(|field| should_materialize_rust_field(field, items_by_type_id))
            .all(|field| field_supports_partial_eq(field, items_by_type_id, cache, visiting, mode)),
    };
    visiting.remove(&item.source_type_id);
    cache.insert(item.source_type_id, supports_partial_eq);
    supports_partial_eq
}

fn item_supports_eq(
    item: &SerializeCodegenItem,
    items_by_type_id: &BTreeMap<Uuid, &SerializeCodegenItem>,
    cache: &mut BTreeMap<Uuid, bool>,
    visiting: &mut BTreeSet<Uuid>,
    mode: RustCodegenMode,
) -> bool {
    if let Some(supports_eq) = cache.get(&item.source_type_id) {
        return *supports_eq;
    }
    if !visiting.insert(item.source_type_id) {
        return false;
    }

    let supports_eq = match item.kind {
        SerializeCodegenItemKind::Enum => true,
        SerializeCodegenItemKind::Struct => item
            .fields
            .iter()
            .filter(|field| should_materialize_rust_field(field, items_by_type_id))
            .all(|field| field_supports_eq(field, items_by_type_id, cache, visiting, mode)),
    };
    visiting.remove(&item.source_type_id);
    cache.insert(item.source_type_id, supports_eq);
    supports_eq
}

fn item_supports_default(
    item: &SerializeCodegenItem,
    items_by_type_id: &BTreeMap<Uuid, &SerializeCodegenItem>,
    cache: &mut BTreeMap<Uuid, bool>,
    visiting: &mut BTreeSet<Uuid>,
    mode: RustCodegenMode,
) -> bool {
    if let Some(supports_default) = cache.get(&item.source_type_id) {
        return *supports_default;
    }
    if !visiting.insert(item.source_type_id) {
        return false;
    }

    let supports_default = match item.kind {
        SerializeCodegenItemKind::Enum => !item.variants.is_empty(),
        SerializeCodegenItemKind::Struct => item
            .fields
            .iter()
            .filter(|field| should_materialize_rust_field(field, items_by_type_id))
            .all(|field| field_supports_default(field, items_by_type_id, cache, visiting, mode)),
    };
    visiting.remove(&item.source_type_id);
    cache.insert(item.source_type_id, supports_default);
    supports_default
}

fn item_supports_hash(
    item: &SerializeCodegenItem,
    items_by_type_id: &BTreeMap<Uuid, &SerializeCodegenItem>,
    cache: &mut BTreeMap<Uuid, bool>,
    visiting: &mut BTreeSet<Uuid>,
    mode: RustCodegenMode,
) -> bool {
    if let Some(supports_hash) = cache.get(&item.source_type_id) {
        return *supports_hash;
    }
    if !visiting.insert(item.source_type_id) {
        return false;
    }

    let supports_hash = match item.kind {
        SerializeCodegenItemKind::Enum => true,
        SerializeCodegenItemKind::Struct => item
            .fields
            .iter()
            .filter(|field| should_materialize_rust_field(field, items_by_type_id))
            .all(|field| {
                resolved_type_supports_hash(
                    &field.resolved_type,
                    items_by_type_id,
                    cache,
                    visiting,
                    mode,
                )
            }),
    };
    visiting.remove(&item.source_type_id);
    cache.insert(item.source_type_id, supports_hash);
    supports_hash
}

fn item_supports_reflect(
    item: &SerializeCodegenItem,
    items_by_type_id: &BTreeMap<Uuid, &SerializeCodegenItem>,
    cache: &mut BTreeMap<Uuid, bool>,
    visiting: &mut BTreeSet<Uuid>,
    mode: RustCodegenMode,
) -> bool {
    if let Some(supports_reflect) = cache.get(&item.source_type_id) {
        return *supports_reflect;
    }
    if !visiting.insert(item.source_type_id) {
        return false;
    }

    let supports_reflect = match item.kind {
        SerializeCodegenItemKind::Enum => true,
        SerializeCodegenItemKind::Struct => item
            .fields
            .iter()
            .filter(|field| should_materialize_rust_field(field, items_by_type_id))
            .all(|field| {
                resolved_type_supports_reflect(
                    &field.resolved_type,
                    items_by_type_id,
                    cache,
                    visiting,
                    mode,
                )
            }),
    };
    visiting.remove(&item.source_type_id);
    cache.insert(item.source_type_id, supports_reflect);
    supports_reflect
}

fn item_supports_marshaler(
    item: &SerializeCodegenItem,
    items_by_type_id: &BTreeMap<Uuid, &SerializeCodegenItem>,
    cache: &mut BTreeMap<Uuid, bool>,
    visiting: &mut BTreeSet<Uuid>,
) -> bool {
    if let Some(supports_marshaler) = cache.get(&item.source_type_id) {
        return *supports_marshaler;
    }
    if !visiting.insert(item.source_type_id) {
        return false;
    }

    let supports_marshaler = match item.kind {
        SerializeCodegenItemKind::Enum => true,
        SerializeCodegenItemKind::Struct => item
            .fields
            .iter()
            .filter(|field| should_materialize_rust_field(field, items_by_type_id))
            .all(|field| field_supports_marshaler(field, items_by_type_id, cache, visiting)),
    };
    visiting.remove(&item.source_type_id);
    cache.insert(item.source_type_id, supports_marshaler);
    supports_marshaler
}

fn item_supports_serde(
    item: &SerializeCodegenItem,
    items_by_type_id: &BTreeMap<Uuid, &SerializeCodegenItem>,
    cache: &mut BTreeMap<Uuid, bool>,
    visiting: &mut BTreeSet<Uuid>,
    mode: RustCodegenMode,
) -> bool {
    if let Some(supports_serde) = cache.get(&item.source_type_id) {
        return *supports_serde;
    }
    if !visiting.insert(item.source_type_id) {
        return false;
    }

    let supports_serde = match item.kind {
        SerializeCodegenItemKind::Enum => true,
        SerializeCodegenItemKind::Struct => item
            .fields
            .iter()
            .filter(|field| should_materialize_rust_field(field, items_by_type_id))
            .all(|field| field_supports_serde(field, items_by_type_id, cache, visiting, mode)),
    };
    visiting.remove(&item.source_type_id);
    cache.insert(item.source_type_id, supports_serde);
    supports_serde
}

fn should_materialize_rust_field(
    field: &SerializeCodegenField,
    items_by_type_id: &BTreeMap<Uuid, &SerializeCodegenItem>,
) -> bool {
    classify_codegen_field(field, items_by_type_id).is_materialized()
}

fn field_supports_partial_eq(
    field: &SerializeCodegenField,
    items_by_type_id: &BTreeMap<Uuid, &SerializeCodegenItem>,
    cache: &mut BTreeMap<Uuid, bool>,
    visiting: &mut BTreeSet<Uuid>,
    mode: RustCodegenMode,
) -> bool {
    resolved_type_supports_partial_eq(
        &field.resolved_type,
        items_by_type_id,
        cache,
        visiting,
        mode,
    )
}

fn field_supports_eq(
    field: &SerializeCodegenField,
    items_by_type_id: &BTreeMap<Uuid, &SerializeCodegenItem>,
    cache: &mut BTreeMap<Uuid, bool>,
    visiting: &mut BTreeSet<Uuid>,
    mode: RustCodegenMode,
) -> bool {
    match classify_codegen_field_type(field) {
        CodegenFieldTypeProjection::FixedOpaqueBytes { .. } => true,
        CodegenFieldTypeProjection::Reflected(resolved_type) => {
            resolved_type_supports_eq(resolved_type, items_by_type_id, cache, visiting, mode)
        }
    }
}

fn field_supports_default(
    field: &SerializeCodegenField,
    items_by_type_id: &BTreeMap<Uuid, &SerializeCodegenItem>,
    cache: &mut BTreeMap<Uuid, bool>,
    visiting: &mut BTreeSet<Uuid>,
    mode: RustCodegenMode,
) -> bool {
    match classify_codegen_field_type(field) {
        CodegenFieldTypeProjection::FixedOpaqueBytes { byte_len } => byte_len <= 32,
        CodegenFieldTypeProjection::Reflected(resolved_type) => {
            resolved_type_supports_default(resolved_type, items_by_type_id, cache, visiting, mode)
        }
    }
}

fn field_supports_marshaler(
    field: &SerializeCodegenField,
    items_by_type_id: &BTreeMap<Uuid, &SerializeCodegenItem>,
    cache: &mut BTreeMap<Uuid, bool>,
    visiting: &mut BTreeSet<Uuid>,
) -> bool {
    match classify_codegen_field_type(field) {
        CodegenFieldTypeProjection::FixedOpaqueBytes { .. } => true,
        CodegenFieldTypeProjection::Reflected(resolved_type) => {
            resolved_type_supports_marshaler(resolved_type, items_by_type_id, cache, visiting)
        }
    }
}

fn field_supports_serde(
    field: &SerializeCodegenField,
    items_by_type_id: &BTreeMap<Uuid, &SerializeCodegenItem>,
    cache: &mut BTreeMap<Uuid, bool>,
    visiting: &mut BTreeSet<Uuid>,
    mode: RustCodegenMode,
) -> bool {
    match classify_codegen_field_type(field) {
        CodegenFieldTypeProjection::FixedOpaqueBytes { byte_len } => byte_len <= 32,
        CodegenFieldTypeProjection::Reflected(resolved_type) => {
            resolved_type_supports_serde(resolved_type, items_by_type_id, cache, visiting, mode)
        }
    }
}

fn resolved_type_supports_partial_eq(
    resolved: &ResolvedType,
    items_by_type_id: &BTreeMap<Uuid, &SerializeCodegenItem>,
    cache: &mut BTreeMap<Uuid, bool>,
    visiting: &mut BTreeSet<Uuid>,
    mode: RustCodegenMode,
) -> bool {
    match resolved {
        ResolvedType::Scalar(_)
        | ResolvedType::Asset { .. }
        | ResolvedType::Uid { .. }
        | ResolvedType::ByteStream => true,
        ResolvedType::ReplicatedField { value } => {
            resolved_type_supports_partial_eq(value, items_by_type_id, cache, visiting, mode)
        }
        ResolvedType::Named {
            type_id,
            source_name,
        } => local_type_traits(source_name).map_or_else(
            || {
                items_by_type_id.get(type_id).is_some_and(|item| {
                    item_supports_partial_eq(item, items_by_type_id, cache, visiting, mode)
                })
            },
            |traits| traits.partial_eq,
        ),
        ResolvedType::Sequence { kind, element, .. } => {
            resolved_type_supports_partial_eq(element, items_by_type_id, cache, visiting, mode)
                && match (mode, kind) {
                    (RustCodegenMode::Integrated, SequenceKind::UnorderedSet) => {
                        rust_type_supports_native_hash_key(element, items_by_type_id)
                    }
                    (RustCodegenMode::Integrated, SequenceKind::Set) => {
                        rust_type_supports_native_ordering(element, items_by_type_id)
                    }
                    _ => true,
                }
        }
        ResolvedType::Map { kind, key, value } => {
            let key_supported = match (mode, kind) {
                (RustCodegenMode::Integrated, MapKind::Map) => {
                    rust_type_supports_native_ordering(key, items_by_type_id)
                }
                (
                    RustCodegenMode::Integrated,
                    MapKind::UnorderedMap | MapKind::UnorderedFlatMap,
                ) => rust_type_supports_native_hash_key(key, items_by_type_id),
                (RustCodegenMode::Standalone, _) => {
                    resolved_type_supports_partial_eq(key, items_by_type_id, cache, visiting, mode)
                }
            };

            key_supported
                && resolved_type_supports_partial_eq(value, items_by_type_id, cache, visiting, mode)
        }
        ResolvedType::Pair { first, second } => {
            resolved_type_supports_partial_eq(first, items_by_type_id, cache, visiting, mode)
                && resolved_type_supports_partial_eq(
                    second,
                    items_by_type_id,
                    cache,
                    visiting,
                    mode,
                )
        }
        ResolvedType::RangedInteger { value, .. } | ResolvedType::Optional { value } => {
            resolved_type_supports_partial_eq(value, items_by_type_id, cache, visiting, mode)
        }
        ResolvedType::Pointer { target, .. } => {
            resolved_type_supports_partial_eq(target, items_by_type_id, cache, visiting, mode)
        }
        ResolvedType::Tuple { elements } => elements.iter().all(|element| {
            resolved_type_supports_partial_eq(element, items_by_type_id, cache, visiting, mode)
        }),
        ResolvedType::Unknown { .. } => matches!(mode, RustCodegenMode::Standalone),
    }
}

fn resolved_type_supports_eq(
    resolved: &ResolvedType,
    items_by_type_id: &BTreeMap<Uuid, &SerializeCodegenItem>,
    cache: &mut BTreeMap<Uuid, bool>,
    visiting: &mut BTreeSet<Uuid>,
    mode: RustCodegenMode,
) -> bool {
    match resolved {
        ResolvedType::Scalar(
            ScalarType::F32
            | ScalarType::F64
            | ScalarType::Vector2
            | ScalarType::Vector3
            | ScalarType::Vector4
            | ScalarType::Quaternion
            | ScalarType::Transform
            | ScalarType::Color
            | ScalarType::ColorF
            | ScalarType::ColorB,
        ) => false,
        ResolvedType::Scalar(_)
        | ResolvedType::Asset { .. }
        | ResolvedType::Uid { .. }
        | ResolvedType::ByteStream => true,
        ResolvedType::ReplicatedField { value } => {
            resolved_type_supports_eq(value, items_by_type_id, cache, visiting, mode)
        }
        ResolvedType::Named {
            type_id,
            source_name,
        } => local_type_traits(source_name).map_or_else(
            || {
                items_by_type_id.get(type_id).is_some_and(|item| {
                    item_supports_eq(item, items_by_type_id, cache, visiting, mode)
                })
            },
            |traits| traits.eq,
        ),
        ResolvedType::Sequence { kind, element, .. } => {
            resolved_type_supports_eq(element, items_by_type_id, cache, visiting, mode)
                && match kind {
                    SequenceKind::UnorderedSet if matches!(mode, RustCodegenMode::Standalone) => {
                        true
                    }
                    SequenceKind::UnorderedSet => {
                        rust_type_supports_native_hash_key(element, items_by_type_id)
                    }
                    SequenceKind::Set if mode == RustCodegenMode::Standalone => true,
                    SequenceKind::Set => {
                        rust_type_supports_native_ordering(element, items_by_type_id)
                    }
                    _ => true,
                }
        }
        ResolvedType::Map {
            kind: MapKind::UnorderedMap | MapKind::UnorderedFlatMap,
            key,
            value,
        } => {
            resolved_type_supports_eq(key, items_by_type_id, cache, visiting, mode)
                && (matches!(mode, RustCodegenMode::Standalone)
                    || rust_type_supports_native_hash_key(key, items_by_type_id))
                && resolved_type_supports_eq(value, items_by_type_id, cache, visiting, mode)
        }
        ResolvedType::Map { key, value, .. }
        | ResolvedType::Pair {
            first: key,
            second: value,
        } => {
            resolved_type_supports_eq(key, items_by_type_id, cache, visiting, mode)
                && resolved_type_supports_eq(value, items_by_type_id, cache, visiting, mode)
        }
        ResolvedType::RangedInteger { value, .. } | ResolvedType::Optional { value } => {
            resolved_type_supports_eq(value, items_by_type_id, cache, visiting, mode)
        }
        ResolvedType::Pointer { target, .. } => {
            resolved_type_supports_eq(target, items_by_type_id, cache, visiting, mode)
        }
        ResolvedType::Tuple { elements } => elements.iter().all(|element| {
            resolved_type_supports_eq(element, items_by_type_id, cache, visiting, mode)
        }),
        ResolvedType::Unknown { .. } => false,
    }
}

fn resolved_type_supports_default(
    resolved: &ResolvedType,
    items_by_type_id: &BTreeMap<Uuid, &SerializeCodegenItem>,
    cache: &mut BTreeMap<Uuid, bool>,
    visiting: &mut BTreeSet<Uuid>,
    mode: RustCodegenMode,
) -> bool {
    match resolved {
        ResolvedType::Scalar(_)
        | ResolvedType::Asset { .. }
        | ResolvedType::Uid { .. }
        | ResolvedType::ReplicatedField { .. }
        | ResolvedType::ByteStream => true,
        ResolvedType::Named {
            type_id,
            source_name,
        } => local_type_traits(source_name).map_or_else(
            || {
                items_by_type_id.get(type_id).is_some_and(|item| {
                    item_supports_default(item, items_by_type_id, cache, visiting, mode)
                })
            },
            |traits| traits.default,
        ),
        ResolvedType::Sequence {
            kind: SequenceKind::Array,
            element,
            capacity: Some(capacity),
        } => {
            *capacity <= 32
                && resolved_type_supports_default(element, items_by_type_id, cache, visiting, mode)
        }
        ResolvedType::Sequence { .. } | ResolvedType::Map { .. } => true,
        ResolvedType::Pair { first, second } => {
            resolved_type_supports_default(first, items_by_type_id, cache, visiting, mode)
                && resolved_type_supports_default(second, items_by_type_id, cache, visiting, mode)
        }
        ResolvedType::RangedInteger { value, .. } | ResolvedType::Optional { value } => {
            resolved_type_supports_default(value, items_by_type_id, cache, visiting, mode)
        }
        ResolvedType::Pointer { target, .. } => {
            resolved_type_supports_default(target, items_by_type_id, cache, visiting, mode)
        }
        ResolvedType::Tuple { elements } => elements.iter().all(|element| {
            resolved_type_supports_default(element, items_by_type_id, cache, visiting, mode)
        }),
        ResolvedType::Unknown { .. } => matches!(mode, RustCodegenMode::Standalone),
    }
}

fn resolved_type_supports_reflect(
    resolved: &ResolvedType,
    items_by_type_id: &BTreeMap<Uuid, &SerializeCodegenItem>,
    cache: &mut BTreeMap<Uuid, bool>,
    visiting: &mut BTreeSet<Uuid>,
    mode: RustCodegenMode,
) -> bool {
    match resolved {
        ResolvedType::Scalar(_)
        | ResolvedType::Asset { .. }
        | ResolvedType::Uid { .. }
        | ResolvedType::ByteStream => true,
        ResolvedType::ReplicatedField { value } => {
            resolved_type_supports_reflect(value, items_by_type_id, cache, visiting, mode)
        }
        ResolvedType::Named {
            type_id,
            source_name,
        } => local_type_traits(source_name).map_or_else(
            || {
                items_by_type_id.get(type_id).is_some_and(|item| {
                    item_supports_reflect(item, items_by_type_id, cache, visiting, mode)
                })
            },
            |traits| traits.reflect,
        ),
        ResolvedType::Sequence { kind, element, .. } => {
            resolved_type_supports_reflect(element, items_by_type_id, cache, visiting, mode)
                && match kind {
                    SequenceKind::Set => {
                        matches!(mode, RustCodegenMode::Standalone)
                            || rust_type_supports_native_ordering(element, items_by_type_id)
                    }
                    SequenceKind::UnorderedSet => {
                        matches!(mode, RustCodegenMode::Standalone)
                            || rust_type_supports_native_hash_key(element, items_by_type_id)
                    }
                    _ => true,
                }
        }
        ResolvedType::Map { kind, key, value } => {
            resolved_type_supports_reflect(key, items_by_type_id, cache, visiting, mode)
                && resolved_type_supports_reflect(value, items_by_type_id, cache, visiting, mode)
                && match kind {
                    MapKind::Map => {
                        matches!(mode, RustCodegenMode::Standalone)
                            || rust_type_supports_native_ordering(key, items_by_type_id)
                    }
                    MapKind::UnorderedMap | MapKind::UnorderedFlatMap => {
                        matches!(mode, RustCodegenMode::Standalone)
                            || rust_type_supports_native_hash_key(key, items_by_type_id)
                    }
                }
        }
        ResolvedType::Pair { first, second } => {
            resolved_type_supports_reflect(first, items_by_type_id, cache, visiting, mode)
                && resolved_type_supports_reflect(second, items_by_type_id, cache, visiting, mode)
        }
        ResolvedType::RangedInteger { value, .. } | ResolvedType::Optional { value } => {
            resolved_type_supports_reflect(value, items_by_type_id, cache, visiting, mode)
        }
        ResolvedType::Pointer { target, .. } => {
            resolved_type_supports_reflect(target, items_by_type_id, cache, visiting, mode)
        }
        ResolvedType::Tuple { elements } => elements.iter().all(|element| {
            resolved_type_supports_reflect(element, items_by_type_id, cache, visiting, mode)
        }),
        ResolvedType::Unknown { .. } => matches!(mode, RustCodegenMode::Standalone),
    }
}

fn resolved_type_supports_marshaler(
    resolved: &ResolvedType,
    items_by_type_id: &BTreeMap<Uuid, &SerializeCodegenItem>,
    cache: &mut BTreeMap<Uuid, bool>,
    visiting: &mut BTreeSet<Uuid>,
) -> bool {
    match resolved {
        ResolvedType::Scalar(scalar) => scalar_supports_marshaler(*scalar),
        ResolvedType::Uid { .. } | ResolvedType::ByteStream => true,
        ResolvedType::Asset { .. } => false,
        ResolvedType::ReplicatedField { value } => {
            resolved_type_supports_marshaler(value, items_by_type_id, cache, visiting)
        }
        ResolvedType::Named {
            type_id,
            source_name,
        } => local_type_traits(source_name).map_or_else(
            || {
                items_by_type_id.get(type_id).is_some_and(|item| {
                    item_supports_marshaler(item, items_by_type_id, cache, visiting)
                })
            },
            |traits| traits.marshaler,
        ),
        ResolvedType::Sequence { kind, element, .. } => {
            resolved_type_supports_marshaler(element, items_by_type_id, cache, visiting)
                && match kind {
                    SequenceKind::Set => {
                        rust_type_supports_native_ordering(element, items_by_type_id)
                    }
                    SequenceKind::UnorderedSet => {
                        rust_type_supports_native_hash_key(element, items_by_type_id)
                    }
                    _ => true,
                }
        }
        ResolvedType::Map { kind, key, value } => {
            resolved_type_supports_marshaler(key, items_by_type_id, cache, visiting)
                && resolved_type_supports_marshaler(value, items_by_type_id, cache, visiting)
                && match kind {
                    MapKind::Map => rust_type_supports_native_ordering(key, items_by_type_id),
                    MapKind::UnorderedMap | MapKind::UnorderedFlatMap => {
                        rust_type_supports_native_hash_key(key, items_by_type_id)
                    }
                }
        }
        ResolvedType::Pair {
            first: key,
            second: value,
        } => {
            resolved_type_supports_marshaler(key, items_by_type_id, cache, visiting)
                && resolved_type_supports_marshaler(value, items_by_type_id, cache, visiting)
        }
        ResolvedType::RangedInteger { value, .. } | ResolvedType::Optional { value } => {
            resolved_type_supports_marshaler(value, items_by_type_id, cache, visiting)
        }
        ResolvedType::Pointer { target, .. } => {
            resolved_type_supports_marshaler(target, items_by_type_id, cache, visiting)
        }
        ResolvedType::Tuple { elements } => {
            matches!(elements.len(), 2 | 3)
                && elements.iter().all(|element| {
                    resolved_type_supports_marshaler(element, items_by_type_id, cache, visiting)
                })
        }
        ResolvedType::Unknown { .. } => false,
    }
}

fn resolved_type_supports_serde(
    resolved: &ResolvedType,
    items_by_type_id: &BTreeMap<Uuid, &SerializeCodegenItem>,
    cache: &mut BTreeMap<Uuid, bool>,
    visiting: &mut BTreeSet<Uuid>,
    mode: RustCodegenMode,
) -> bool {
    match resolved {
        ResolvedType::Scalar(_)
        | ResolvedType::Asset { .. }
        | ResolvedType::Uid { .. }
        | ResolvedType::ByteStream => true,
        ResolvedType::ReplicatedField { value } => {
            resolved_type_supports_serde(value, items_by_type_id, cache, visiting, mode)
        }
        ResolvedType::Named {
            type_id,
            source_name,
        } => local_type_traits(source_name).map_or_else(
            || {
                items_by_type_id.get(type_id).is_some_and(|item| {
                    item_supports_serde(item, items_by_type_id, cache, visiting, mode)
                })
            },
            |traits| traits.serde,
        ),
        ResolvedType::Sequence { kind, element, .. } => {
            resolved_type_supports_serde(element, items_by_type_id, cache, visiting, mode)
                && match kind {
                    SequenceKind::Set => {
                        matches!(mode, RustCodegenMode::Standalone)
                            || rust_type_supports_native_ordering(element, items_by_type_id)
                    }
                    SequenceKind::UnorderedSet => {
                        matches!(mode, RustCodegenMode::Standalone)
                            || rust_type_supports_native_hash_key(element, items_by_type_id)
                    }
                    _ => true,
                }
        }
        ResolvedType::Map { kind, key, value } => {
            resolved_type_supports_serde(key, items_by_type_id, cache, visiting, mode)
                && resolved_type_supports_serde(value, items_by_type_id, cache, visiting, mode)
                && match kind {
                    MapKind::Map => {
                        matches!(mode, RustCodegenMode::Standalone)
                            || rust_type_supports_native_ordering(key, items_by_type_id)
                    }
                    MapKind::UnorderedMap | MapKind::UnorderedFlatMap => {
                        matches!(mode, RustCodegenMode::Standalone)
                            || rust_type_supports_native_hash_key(key, items_by_type_id)
                    }
                }
        }
        ResolvedType::Pair { first, second } => {
            resolved_type_supports_serde(first, items_by_type_id, cache, visiting, mode)
                && resolved_type_supports_serde(second, items_by_type_id, cache, visiting, mode)
        }
        ResolvedType::RangedInteger { value, .. } | ResolvedType::Optional { value } => {
            resolved_type_supports_serde(value, items_by_type_id, cache, visiting, mode)
        }
        ResolvedType::Pointer { target, .. } => {
            resolved_type_supports_serde(target, items_by_type_id, cache, visiting, mode)
        }
        ResolvedType::Tuple { elements } => {
            elements.len() <= 16
                && elements.iter().all(|element| {
                    resolved_type_supports_serde(element, items_by_type_id, cache, visiting, mode)
                })
        }
        ResolvedType::Unknown { .. } => false,
    }
}

fn resolved_type_supports_ord(
    resolved: &ResolvedType,
    items_by_type_id: &BTreeMap<Uuid, &SerializeCodegenItem>,
    cache: &mut BTreeMap<Uuid, bool>,
    visiting: &mut BTreeSet<Uuid>,
) -> bool {
    match resolved {
        ResolvedType::Scalar(scalar) => scalar_supports_ord(*scalar),
        ResolvedType::Uid { .. } | ResolvedType::ByteStream => true,
        ResolvedType::Asset { .. } | ResolvedType::ReplicatedField { .. } => false,
        ResolvedType::Named {
            type_id,
            source_name,
        } => local_type_traits(source_name).map_or_else(
            || {
                items_by_type_id
                    .get(type_id)
                    .is_some_and(|item| item_supports_ord(item, items_by_type_id, cache, visiting))
            },
            |traits| traits.ord,
        ),
        ResolvedType::RangedInteger { value, .. } => {
            resolved_type_supports_ord(value, items_by_type_id, cache, visiting)
        }
        ResolvedType::Pointer { target, .. } | ResolvedType::Optional { value: target } => {
            resolved_type_supports_ord(target, items_by_type_id, cache, visiting)
        }
        ResolvedType::Pair { first, second } => {
            resolved_type_supports_ord(first, items_by_type_id, cache, visiting)
                && resolved_type_supports_ord(second, items_by_type_id, cache, visiting)
        }
        ResolvedType::Tuple { elements } => elements
            .iter()
            .all(|element| resolved_type_supports_ord(element, items_by_type_id, cache, visiting)),
        ResolvedType::Sequence { .. } | ResolvedType::Map { .. } | ResolvedType::Unknown { .. } => {
            false
        }
    }
}

fn item_supports_ord(
    item: &SerializeCodegenItem,
    items_by_type_id: &BTreeMap<Uuid, &SerializeCodegenItem>,
    cache: &mut BTreeMap<Uuid, bool>,
    visiting: &mut BTreeSet<Uuid>,
) -> bool {
    if let Some(supports_ord) = cache.get(&item.source_type_id) {
        return *supports_ord;
    }
    if !visiting.insert(item.source_type_id) {
        return false;
    }

    let supports_ord = match item.kind {
        SerializeCodegenItemKind::Enum => true,
        SerializeCodegenItemKind::Struct => item
            .fields
            .iter()
            .filter(|field| should_materialize_rust_field(field, items_by_type_id))
            .all(|field| field_supports_ord(field, items_by_type_id, cache, visiting)),
    };
    visiting.remove(&item.source_type_id);
    cache.insert(item.source_type_id, supports_ord);
    supports_ord
}

fn field_supports_ord(
    field: &SerializeCodegenField,
    items_by_type_id: &BTreeMap<Uuid, &SerializeCodegenItem>,
    cache: &mut BTreeMap<Uuid, bool>,
    visiting: &mut BTreeSet<Uuid>,
) -> bool {
    match classify_codegen_field_type(field) {
        CodegenFieldTypeProjection::FixedOpaqueBytes { .. } => true,
        CodegenFieldTypeProjection::Reflected(resolved_type) => {
            resolved_type_supports_ord(resolved_type, items_by_type_id, cache, visiting)
        }
    }
}

fn resolved_type_supports_hash(
    resolved: &ResolvedType,
    items_by_type_id: &BTreeMap<Uuid, &SerializeCodegenItem>,
    cache: &mut BTreeMap<Uuid, bool>,
    visiting: &mut BTreeSet<Uuid>,
    mode: RustCodegenMode,
) -> bool {
    match resolved {
        ResolvedType::Scalar(
            ScalarType::F32
            | ScalarType::F64
            | ScalarType::Vector2
            | ScalarType::Vector3
            | ScalarType::Vector4
            | ScalarType::Quaternion
            | ScalarType::Transform
            | ScalarType::Color
            | ScalarType::ColorF
            | ScalarType::ColorB,
        ) => false,
        ResolvedType::Scalar(_)
        | ResolvedType::Asset { .. }
        | ResolvedType::Uid { .. }
        | ResolvedType::ByteStream => true,
        ResolvedType::ReplicatedField { value } => {
            resolved_type_supports_hash(value, items_by_type_id, cache, visiting, mode)
        }
        ResolvedType::Named {
            type_id,
            source_name,
        } => local_type_traits(source_name).map_or_else(
            || {
                items_by_type_id.get(type_id).is_some_and(|item| {
                    item_supports_hash(item, items_by_type_id, cache, visiting, mode)
                })
            },
            |traits| traits.hash,
        ),
        ResolvedType::Sequence { kind, element, .. } => {
            resolved_type_supports_hash(element, items_by_type_id, cache, visiting, mode)
                && match kind {
                    SequenceKind::UnorderedSet => {
                        matches!(mode, RustCodegenMode::Standalone)
                            && !rust_type_supports_native_hash_key(element, items_by_type_id)
                    }
                    _ => true,
                }
        }
        ResolvedType::Map { kind, key, value } => {
            resolved_type_supports_hash(key, items_by_type_id, cache, visiting, mode)
                && resolved_type_supports_hash(value, items_by_type_id, cache, visiting, mode)
                && match kind {
                    MapKind::UnorderedMap | MapKind::UnorderedFlatMap => {
                        matches!(mode, RustCodegenMode::Standalone)
                            && !rust_type_supports_native_hash_key(key, items_by_type_id)
                    }
                    MapKind::Map => true,
                }
        }
        ResolvedType::Pair { first, second } => {
            resolved_type_supports_hash(first, items_by_type_id, cache, visiting, mode)
                && resolved_type_supports_hash(second, items_by_type_id, cache, visiting, mode)
        }
        ResolvedType::RangedInteger { value, .. } | ResolvedType::Optional { value } => {
            resolved_type_supports_hash(value, items_by_type_id, cache, visiting, mode)
        }
        ResolvedType::Pointer { target, .. } => {
            resolved_type_supports_hash(target, items_by_type_id, cache, visiting, mode)
        }
        ResolvedType::Tuple { elements } => elements.iter().all(|element| {
            resolved_type_supports_hash(element, items_by_type_id, cache, visiting, mode)
        }),
        ResolvedType::Unknown { .. } => matches!(mode, RustCodegenMode::Standalone),
    }
}

fn resolved_type_supports_copy(
    resolved: &ResolvedType,
    items_by_type_id: &BTreeMap<Uuid, &SerializeCodegenItem>,
    cache: &mut BTreeMap<Uuid, bool>,
    visiting: &mut BTreeSet<Uuid>,
    mode: RustCodegenMode,
) -> bool {
    match resolved {
        ResolvedType::Scalar(scalar) => scalar_supports_copy(*scalar),
        ResolvedType::Uid { .. } => true,
        ResolvedType::Named {
            type_id,
            source_name,
        } => local_type_traits(source_name).map_or_else(
            || {
                items_by_type_id.get(type_id).is_some_and(|item| {
                    item_supports_copy(item, items_by_type_id, cache, visiting, mode)
                })
            },
            |traits| traits.copy,
        ),
        ResolvedType::RangedInteger { value, .. } | ResolvedType::Optional { value } => {
            resolved_type_supports_copy(value, items_by_type_id, cache, visiting, mode)
        }
        ResolvedType::Pointer { target, .. } => {
            resolved_type_supports_copy(target, items_by_type_id, cache, visiting, mode)
        }
        ResolvedType::Pair { first, second } => {
            resolved_type_supports_copy(first, items_by_type_id, cache, visiting, mode)
                && resolved_type_supports_copy(second, items_by_type_id, cache, visiting, mode)
        }
        ResolvedType::Tuple { elements } => elements.iter().all(|element| {
            resolved_type_supports_copy(element, items_by_type_id, cache, visiting, mode)
        }),
        ResolvedType::Asset { .. }
        | ResolvedType::ByteStream
        | ResolvedType::ReplicatedField { .. }
        | ResolvedType::Sequence { .. }
        | ResolvedType::Map { .. }
        | ResolvedType::Unknown { .. } => false,
    }
}

const fn scalar_supports_copy(scalar: ScalarType) -> bool {
    !matches!(scalar, ScalarType::String)
}

const fn scalar_supports_ord(scalar: ScalarType) -> bool {
    !matches!(
        scalar,
        ScalarType::F32
            | ScalarType::F64
            | ScalarType::Vector2
            | ScalarType::Vector3
            | ScalarType::Vector4
            | ScalarType::Quaternion
            | ScalarType::Transform
            | ScalarType::Color
            | ScalarType::ColorF
            | ScalarType::ColorB
            | ScalarType::EntityId
    )
}

const fn scalar_supports_marshaler(scalar: ScalarType) -> bool {
    !matches!(
        scalar,
        ScalarType::AssetId | ScalarType::Vector4 | ScalarType::Transform
    )
}

pub(super) fn rust_type_supports_native_ordering(
    resolved: &ResolvedType,
    items_by_type_id: &BTreeMap<Uuid, &SerializeCodegenItem>,
) -> bool {
    resolved_type_supports_ord(
        resolved,
        items_by_type_id,
        &mut BTreeMap::new(),
        &mut BTreeSet::new(),
    )
}

pub(super) fn rust_type_supports_native_hash_key(
    resolved: &ResolvedType,
    items_by_type_id: &BTreeMap<Uuid, &SerializeCodegenItem>,
) -> bool {
    resolved_type_supports_eq(
        resolved,
        items_by_type_id,
        &mut BTreeMap::new(),
        &mut BTreeSet::new(),
        RustCodegenMode::Integrated,
    ) && resolved_type_supports_hash(
        resolved,
        items_by_type_id,
        &mut BTreeMap::new(),
        &mut BTreeSet::new(),
        RustCodegenMode::Integrated,
    )
}
