use std::collections::{BTreeMap, BTreeSet};

use uuid::Uuid;

use crate::field_projection::{
    CodegenFieldTypeProjection, classify_codegen_field, classify_codegen_field_type,
};
use crate::ir::{SerializeCodegenField, SerializeCodegenItem};
use crate::naming::{rust_field_ident, rust_type_ident};
use crate::rust::derive_plan::{
    rust_type_supports_native_hash_key, rust_type_supports_native_ordering,
};
use crate::rust::enum_plan::is_rust_integer_type;
use crate::rust::integrate::source_index::RustSourceTypeIndex;
use crate::rust::item_plan::{
    RustFieldPlan, RustIntegerRangePlan, RustRttiBasePlan, RustUnresolvedTypePlan,
};
use crate::rust::name_plan::RustNamePlan;
use crate::rust::options::RustCodegenMode;
use crate::rust::types::{RustTypeRenderer, rust_bitset_storage_type};
use crate::types::{MapKind, ResolvedType, ScalarType, SequenceKind};

#[derive(Debug, Clone, Copy)]
pub(super) struct RustFieldPlanner {
    mode: RustCodegenMode,
    rust_types: RustTypeRenderer,
}

impl RustFieldPlanner {
    pub(super) const fn new(mode: RustCodegenMode, rust_types: RustTypeRenderer) -> Self {
        Self { mode, rust_types }
    }

    pub(super) fn plan_struct_fields(
        &self,
        item: &SerializeCodegenItem,
        items_by_type_id: &BTreeMap<Uuid, &SerializeCodegenItem>,
        name_plan: &RustNamePlan,
        source_types: Option<&RustSourceTypeIndex>,
        current_module: &str,
    ) -> Vec<RustFieldPlan> {
        let mut field_counts = BTreeMap::<String, usize>::new();
        item.fields
            .iter()
            .filter(|field| should_materialize_rust_field(field, items_by_type_id))
            .map(|field| {
                let mut plan = self.plan_serialize_field(
                    item,
                    field,
                    name_plan,
                    items_by_type_id,
                    source_types,
                    current_module,
                );
                let count = field_counts.entry(plan.rust_name.clone()).or_default();
                if *count > 0 {
                    plan.rust_name = format!("{}_{}", plan.rust_name, *count + 1);
                }
                *count += 1;
                plan
            })
            .collect()
    }

    pub(super) fn plan_rtti_bases(
        &self,
        item: &SerializeCodegenItem,
        items_by_type_id: &BTreeMap<Uuid, &SerializeCodegenItem>,
        name_plan: &RustNamePlan,
        source_types: Option<&RustSourceTypeIndex>,
        current_module: &str,
    ) -> Vec<RustRttiBasePlan> {
        let mut seen = BTreeSet::new();
        let mut bases = item
            .fields
            .iter()
            .filter(|field| field.is_base_class)
            .filter_map(|field| {
                let ResolvedType::Named {
                    type_id,
                    source_name,
                } = &field.resolved_type
                else {
                    return None;
                };
                if !seen.insert(*type_id) {
                    return None;
                }
                Some(RustRttiBasePlan {
                    source_type_id: *type_id,
                    source_name: source_name.clone(),
                    rust_type: self.rust_type_for_field(
                        field,
                        name_plan,
                        items_by_type_id,
                        source_types,
                        current_module,
                    ),
                })
            })
            .collect::<Vec<_>>();

        if bases.is_empty()
            && let Some(base) = item.rtti_base_chain.last()
        {
            bases.push(RustRttiBasePlan {
                source_type_id: base.type_id,
                source_name: base.source_name.clone(),
                rust_type: name_plan.reference_name(base.type_id, &base.source_name),
            });
        }

        bases
    }

    fn plan_serialize_field(
        &self,
        item: &SerializeCodegenItem,
        field: &SerializeCodegenField,
        name_plan: &RustNamePlan,
        items_by_type_id: &BTreeMap<Uuid, &SerializeCodegenItem>,
        source_types: Option<&RustSourceTypeIndex>,
        current_module: &str,
    ) -> RustFieldPlan {
        let rust_type = rust_storage_override_for_field(self.mode, item, field)
            .map(str::to_owned)
            .or_else(|| {
                self.recursive_base_rust_type_for_field(
                    item,
                    field,
                    name_plan,
                    items_by_type_id,
                    source_types,
                    current_module,
                )
            })
            .unwrap_or_else(|| {
                self.rust_type_for_field(
                    field,
                    name_plan,
                    items_by_type_id,
                    source_types,
                    current_module,
                )
            });
        RustFieldPlan {
            source_name: field.source_name.clone(),
            rust_name: if field.is_base_class {
                base_class_field_name(&field.resolved_type)
            } else {
                rust_field_ident(&field.source_name)
            },
            source_type_id: field.source_type_id,
            rust_type,
            unresolved_type: self.unresolved_type_for_field(field, source_types),
            integer_range: self.plan_integer_range(&field.resolved_type),
            data_size: field.data_size,
            offset: field.offset,
            flags: field.flags,
            is_base_class: field.is_base_class,
        }
    }

    fn rust_type_for_field(
        &self,
        field: &SerializeCodegenField,
        name_plan: &RustNamePlan,
        items_by_type_id: &BTreeMap<Uuid, &SerializeCodegenItem>,
        source_types: Option<&RustSourceTypeIndex>,
        current_module: &str,
    ) -> String {
        match classify_codegen_field_type(field) {
            CodegenFieldTypeProjection::FixedOpaqueBytes { byte_len } => {
                format!("[u8; {byte_len}]")
            }
            CodegenFieldTypeProjection::Reflected(resolved_type)
                if matches!(self.mode, RustCodegenMode::Standalone) =>
            {
                self.rust_type_for_resolved_type(resolved_type, name_plan, items_by_type_id)
            }
            CodegenFieldTypeProjection::Reflected(resolved_type) => self
                .integrated_rust_type_for_resolved_type(
                    resolved_type,
                    name_plan,
                    items_by_type_id,
                    source_types,
                    current_module,
                ),
        }
    }

    fn recursive_base_rust_type_for_field(
        &self,
        item: &SerializeCodegenItem,
        field: &SerializeCodegenField,
        name_plan: &RustNamePlan,
        items_by_type_id: &BTreeMap<Uuid, &SerializeCodegenItem>,
        source_types: Option<&RustSourceTypeIndex>,
        current_module: &str,
    ) -> Option<String> {
        if field.is_base_class {
            return None;
        }

        let (target, is_optional) = recursive_base_field_target(&field.resolved_type)?;
        let target_item = items_by_type_id.get(&target.type_id).copied()?;
        if target_item.is_abstract != Some(true)
            || !target_item.fields.is_empty()
            || !item_inherits_from(item, target.type_id)
        {
            return None;
        }

        let rust_type = match self.mode {
            RustCodegenMode::Standalone => {
                self.rust_type_for_resolved_type(target.resolved, name_plan, items_by_type_id)
            }
            RustCodegenMode::Integrated => self.integrated_rust_type_for_resolved_type(
                target.resolved,
                name_plan,
                items_by_type_id,
                source_types,
                current_module,
            ),
        };

        if is_optional {
            Some(format!("Option<Box<{rust_type}>>"))
        } else {
            Some(format!("Box<{rust_type}>"))
        }
    }

    fn rust_type_for_resolved_type(
        &self,
        resolved: &ResolvedType,
        name_plan: &RustNamePlan,
        items_by_type_id: &BTreeMap<Uuid, &SerializeCodegenItem>,
    ) -> String {
        match resolved {
            ResolvedType::Named {
                type_id,
                source_name,
            } => {
                if matches!(self.mode, RustCodegenMode::Integrated)
                    && let Some(rust_type) = integrated_custom_field_type(source_name)
                {
                    rust_type.to_owned()
                } else {
                    name_plan.reference_name(*type_id, source_name)
                }
            }
            ResolvedType::Sequence {
                kind,
                element,
                capacity,
            } => self.rust_sequence_type(*kind, element, *capacity, name_plan, items_by_type_id),
            ResolvedType::Map { kind, key, value } => {
                self.rust_map_type(*kind, key, value, name_plan, items_by_type_id)
            }
            ResolvedType::ReplicatedField { value } => {
                let value = self.rust_type_for_resolved_type(value, name_plan, items_by_type_id);
                format!("Option<{value}>")
            }
            ResolvedType::RangedInteger { value, .. } => {
                self.rust_type_for_resolved_type(value, name_plan, items_by_type_id)
            }
            ResolvedType::Pair { first, second } => {
                let first = self.rust_type_for_resolved_type(first, name_plan, items_by_type_id);
                let second = self.rust_type_for_resolved_type(second, name_plan, items_by_type_id);
                format!("({first}, {second})")
            }
            ResolvedType::Pointer { target, .. } => {
                let target = self.rust_type_for_resolved_type(target, name_plan, items_by_type_id);
                format!("Option<{target}>")
            }
            ResolvedType::Optional { value } => {
                let value = self.rust_type_for_resolved_type(value, name_plan, items_by_type_id);
                format!("Option<{value}>")
            }
            ResolvedType::Tuple { elements } => {
                let elements = elements
                    .iter()
                    .map(|element| {
                        self.rust_type_for_resolved_type(element, name_plan, items_by_type_id)
                    })
                    .collect::<Vec<_>>();
                match elements.as_slice() {
                    [] => "()".to_owned(),
                    [single] => format!("({single},)"),
                    _ => format!("({})", elements.join(", ")),
                }
            }
            ResolvedType::Scalar(_)
            | ResolvedType::Asset { .. }
            | ResolvedType::Uid { .. }
            | ResolvedType::ByteStream
            | ResolvedType::Unknown { .. } => self
                .rust_types
                .render_with_names(resolved, name_plan.names_by_type_id()),
        }
    }

    fn integrated_rust_type_for_resolved_type(
        &self,
        resolved: &ResolvedType,
        name_plan: &RustNamePlan,
        items_by_type_id: &BTreeMap<Uuid, &SerializeCodegenItem>,
        source_types: Option<&RustSourceTypeIndex>,
        current_module: &str,
    ) -> String {
        match resolved {
            ResolvedType::Unknown { type_id, .. } => source_types
                .and_then(|source_types| {
                    source_types.reference_for_type_id(*type_id, current_module)
                })
                .unwrap_or_else(|| {
                    self.rust_types
                        .render_with_names(resolved, name_plan.names_by_type_id())
                }),
            ResolvedType::Named {
                type_id,
                source_name,
            } => integrated_custom_field_type(source_name)
                .map(str::to_owned)
                .unwrap_or_else(|| name_plan.reference_name(*type_id, source_name)),
            ResolvedType::Sequence {
                kind,
                element,
                capacity,
            } => self.integrated_rust_sequence_type(
                *kind,
                element,
                *capacity,
                name_plan,
                items_by_type_id,
                source_types,
                current_module,
            ),
            ResolvedType::Map { kind, key, value } => self.integrated_rust_map_type(
                *kind,
                key,
                value,
                name_plan,
                items_by_type_id,
                source_types,
                current_module,
            ),
            ResolvedType::ReplicatedField { value } => {
                let value = self.integrated_rust_type_for_resolved_type(
                    value,
                    name_plan,
                    items_by_type_id,
                    source_types,
                    current_module,
                );
                format!("Option<{value}>")
            }
            ResolvedType::RangedInteger { value, .. } => self
                .integrated_rust_type_for_resolved_type(
                    value,
                    name_plan,
                    items_by_type_id,
                    source_types,
                    current_module,
                ),
            ResolvedType::Pair { first, second } => {
                let first = self.integrated_rust_type_for_resolved_type(
                    first,
                    name_plan,
                    items_by_type_id,
                    source_types,
                    current_module,
                );
                let second = self.integrated_rust_type_for_resolved_type(
                    second,
                    name_plan,
                    items_by_type_id,
                    source_types,
                    current_module,
                );
                format!("({first}, {second})")
            }
            ResolvedType::Pointer { target, .. } => {
                let target = self.integrated_rust_type_for_resolved_type(
                    target,
                    name_plan,
                    items_by_type_id,
                    source_types,
                    current_module,
                );
                format!("Option<{target}>")
            }
            ResolvedType::Optional { value } => {
                let value = self.integrated_rust_type_for_resolved_type(
                    value,
                    name_plan,
                    items_by_type_id,
                    source_types,
                    current_module,
                );
                format!("Option<{value}>")
            }
            ResolvedType::Tuple { elements } => {
                let elements = elements
                    .iter()
                    .map(|element| {
                        self.integrated_rust_type_for_resolved_type(
                            element,
                            name_plan,
                            items_by_type_id,
                            source_types,
                            current_module,
                        )
                    })
                    .collect::<Vec<_>>();
                match elements.as_slice() {
                    [] => "()".to_owned(),
                    [single] => format!("({single},)"),
                    _ => format!("({})", elements.join(", ")),
                }
            }
            ResolvedType::Scalar(_)
            | ResolvedType::Asset { .. }
            | ResolvedType::Uid { .. }
            | ResolvedType::ByteStream => self
                .rust_types
                .render_with_names(resolved, name_plan.names_by_type_id()),
        }
    }

    fn integrated_rust_sequence_type(
        &self,
        kind: SequenceKind,
        element: &ResolvedType,
        capacity: Option<usize>,
        name_plan: &RustNamePlan,
        items_by_type_id: &BTreeMap<Uuid, &SerializeCodegenItem>,
        source_types: Option<&RustSourceTypeIndex>,
        current_module: &str,
    ) -> String {
        let element_type = self.integrated_rust_type_for_resolved_type(
            element,
            name_plan,
            items_by_type_id,
            source_types,
            current_module,
        );
        match (kind, capacity) {
            (SequenceKind::Array, Some(capacity)) => format!("[{element_type}; {capacity}]"),
            (SequenceKind::BitSet, _) => rust_bitset_storage_type(capacity),
            (SequenceKind::FixedVector, Some(capacity)) => {
                format!("smallvec::SmallVec<[{element_type}; {capacity}]>")
            }
            (SequenceKind::Set, _) => format!("std::collections::BTreeSet<{element_type}>"),
            (SequenceKind::UnorderedSet, _) => {
                format!("std::collections::HashSet<{element_type}>")
            }
            _ => format!("Vec<{element_type}>"),
        }
    }

    fn integrated_rust_map_type(
        &self,
        kind: MapKind,
        key: &ResolvedType,
        value: &ResolvedType,
        name_plan: &RustNamePlan,
        items_by_type_id: &BTreeMap<Uuid, &SerializeCodegenItem>,
        source_types: Option<&RustSourceTypeIndex>,
        current_module: &str,
    ) -> String {
        let key_type = self.integrated_rust_type_for_resolved_type(
            key,
            name_plan,
            items_by_type_id,
            source_types,
            current_module,
        );
        let value_type = self.integrated_rust_type_for_resolved_type(
            value,
            name_plan,
            items_by_type_id,
            source_types,
            current_module,
        );
        match kind {
            MapKind::Map => format!("std::collections::BTreeMap<{key_type}, {value_type}>"),
            MapKind::UnorderedMap | MapKind::UnorderedFlatMap => {
                format!("std::collections::HashMap<{key_type}, {value_type}>")
            }
        }
    }

    fn rust_sequence_type(
        &self,
        kind: SequenceKind,
        element: &ResolvedType,
        capacity: Option<usize>,
        name_plan: &RustNamePlan,
        items_by_type_id: &BTreeMap<Uuid, &SerializeCodegenItem>,
    ) -> String {
        let element_type = self.rust_type_for_resolved_type(element, name_plan, items_by_type_id);
        match (kind, capacity) {
            (SequenceKind::Array, Some(capacity)) => format!("[{element_type}; {capacity}]"),
            (SequenceKind::BitSet, _) => rust_bitset_storage_type(capacity),
            (SequenceKind::FixedVector, Some(capacity)) => {
                format!("smallvec::SmallVec<[{element_type}; {capacity}]>")
            }
            (SequenceKind::Set, _)
                if rust_type_supports_native_ordering(element, items_by_type_id) =>
            {
                format!("std::collections::BTreeSet<{element_type}>")
            }
            (SequenceKind::Set, _) => format!("Vec<{element_type}>"),
            (SequenceKind::UnorderedSet, _)
                if rust_type_supports_native_hash_key(element, items_by_type_id) =>
            {
                format!("std::collections::HashSet<{element_type}>")
            }
            (SequenceKind::UnorderedSet, _) => format!("Vec<{element_type}>"),
            _ => format!("Vec<{element_type}>"),
        }
    }

    fn rust_map_type(
        &self,
        kind: MapKind,
        key: &ResolvedType,
        value: &ResolvedType,
        name_plan: &RustNamePlan,
        items_by_type_id: &BTreeMap<Uuid, &SerializeCodegenItem>,
    ) -> String {
        let key_type = self.rust_type_for_resolved_type(key, name_plan, items_by_type_id);
        let value_type = self.rust_type_for_resolved_type(value, name_plan, items_by_type_id);
        match kind {
            MapKind::Map if rust_type_supports_native_ordering(key, items_by_type_id) => {
                format!("std::collections::BTreeMap<{key_type}, {value_type}>")
            }
            MapKind::Map => format!("Vec<({key_type}, {value_type})>"),
            MapKind::UnorderedMap | MapKind::UnorderedFlatMap
                if rust_type_supports_native_hash_key(key, items_by_type_id) =>
            {
                format!("std::collections::HashMap<{key_type}, {value_type}>")
            }
            MapKind::UnorderedMap | MapKind::UnorderedFlatMap => {
                format!("Vec<({key_type}, {value_type})>")
            }
        }
    }

    fn unresolved_type_for_field(
        &self,
        field: &SerializeCodegenField,
        source_types: Option<&RustSourceTypeIndex>,
    ) -> Option<RustUnresolvedTypePlan> {
        let CodegenFieldTypeProjection::Reflected(resolved_type) =
            classify_codegen_field_type(field)
        else {
            return None;
        };
        resolved_type.unresolved().and_then(|unresolved| {
            if matches!(self.mode, RustCodegenMode::Integrated)
                && source_types.is_some_and(|source_types| {
                    source_types
                        .location_for_type_id(unresolved.type_id)
                        .is_some()
                })
            {
                return None;
            }
            Some(RustUnresolvedTypePlan {
                type_id: unresolved.type_id,
                reason: unresolved.reason.to_owned(),
            })
        })
    }

    fn plan_integer_range(&self, resolved: &ResolvedType) -> Option<RustIntegerRangePlan> {
        let ResolvedType::RangedInteger {
            value,
            min: Some(start),
            max: Some(last),
        } = resolved
        else {
            return None;
        };

        let value_type = self.rust_types.render(value);
        if !is_rust_integer_type(&value_type) {
            return None;
        }

        Some(RustIntegerRangePlan {
            rust_type: format!("::core::ops::RangeInclusive<{value_type}>"),
            value_type,
            start: start.to_string(),
            last: last.to_string(),
        })
    }
}

fn should_materialize_rust_field(
    field: &SerializeCodegenField,
    items_by_type_id: &BTreeMap<Uuid, &SerializeCodegenItem>,
) -> bool {
    classify_codegen_field(field, items_by_type_id).is_materialized()
}

#[derive(Debug, Clone, Copy)]
struct RecursiveBaseFieldTarget<'a> {
    type_id: Uuid,
    resolved: &'a ResolvedType,
}

fn recursive_base_field_target(
    resolved: &ResolvedType,
) -> Option<(RecursiveBaseFieldTarget<'_>, bool)> {
    match resolved {
        ResolvedType::Named { type_id, .. } => Some((
            RecursiveBaseFieldTarget {
                type_id: *type_id,
                resolved,
            },
            false,
        )),
        ResolvedType::Optional { value } | ResolvedType::Pointer { target: value, .. } => {
            let ResolvedType::Named { type_id, .. } = value.as_ref() else {
                return None;
            };
            Some((
                RecursiveBaseFieldTarget {
                    type_id: *type_id,
                    resolved: value,
                },
                true,
            ))
        }
        ResolvedType::Sequence { .. }
        | ResolvedType::Map { .. }
        | ResolvedType::ReplicatedField { .. }
        | ResolvedType::RangedInteger { .. }
        | ResolvedType::Pair { .. }
        | ResolvedType::Tuple { .. }
        | ResolvedType::Scalar(_)
        | ResolvedType::Asset { .. }
        | ResolvedType::Uid { .. }
        | ResolvedType::ByteStream
        | ResolvedType::Unknown { .. } => None,
    }
}

fn item_inherits_from(item: &SerializeCodegenItem, target_type_id: Uuid) -> bool {
    item.rtti_base_chain
        .iter()
        .any(|base| base.type_id == target_type_id)
        || item.fields.iter().any(|field| {
            field.is_base_class
                && matches!(
                    field.resolved_type,
                    ResolvedType::Named { type_id, .. } if type_id == target_type_id
                )
        })
}

fn base_class_field_name(resolved: &ResolvedType) -> String {
    let ResolvedType::Named { source_name, .. } = resolved else {
        return "base".to_owned();
    };
    if source_name.contains("::") {
        rust_field_ident(&source_name.replace("::", "_"))
    } else {
        rust_field_ident(&rust_type_ident(source_name))
    }
}

fn rust_storage_override_for_field(
    mode: RustCodegenMode,
    item: &SerializeCodegenItem,
    field: &SerializeCodegenField,
) -> Option<&'static str> {
    if matches!(mode, RustCodegenMode::Integrated)
        && matches!(
            &field.resolved_type,
            ResolvedType::Named { source_name, .. } if integrated_custom_field_type(source_name).is_some()
        )
    {
        let ResolvedType::Named { source_name, .. } = &field.resolved_type else {
            unreachable!("matched named type");
        };
        return integrated_custom_field_type(source_name);
    }

    if item.source_name == "PrefabSpawnerComponent"
        && field.source_name == "m_sliceVariant"
        && matches!(
            field.resolved_type,
            ResolvedType::Scalar(ScalarType::String)
        )
    {
        return Some("Vec<u8>");
    }

    None
}

pub(super) fn integrated_custom_field_type(source_name: &str) -> Option<&'static str> {
    match source_name {
        "Amazon::Hub::ActorRef" => Some("crate::refs::ClientActorRef"),
        "CritWindow" => Some("crate::combat::damage_receiver::messages::CritWindowData"),
        "HomePoint" => Some("crate::housing::player_home::HomePointReplicatedState"),
        "HomePointList" => Some(
            "::gridmate::serialize::ReplicatedVec<crate::housing::player_home::HomePointReplicatedState>",
        ),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use uuid::uuid;

    use crate::ir::{SerializeCodegenField, SerializeCodegenItem, SerializeCodegenItemKind};
    use crate::role::ReflectedTypeRole;
    use crate::types::{ResolvedType, ScalarType};

    use super::*;

    #[test]
    fn prefab_slice_variant_uses_byte_storage() {
        let item = SerializeCodegenItem {
            source_type_id: uuid!("11111111-1111-1111-1111-111111111111"),
            source_name: "PrefabSpawnerComponent".to_owned(),
            role: ReflectedTypeRole::SupportType,
            is_reflection_marker: false,
            is_abstract: Some(false),
            factory: None,
            rtti_base_chain: Vec::new(),
            kind: SerializeCodegenItemKind::Struct,
            enum_underlying_type: None,
            fields: Vec::new(),
            variants: Vec::new(),
        };
        let field = SerializeCodegenField {
            source_name: "m_sliceVariant".to_owned(),
            source_type_id: uuid!("22222222-2222-2222-2222-222222222222"),
            resolved_type: ResolvedType::Scalar(ScalarType::String),
            data_size: None,
            offset: None,
            flags: None,
            is_base_class: false,
            is_pointer: false,
            is_dynamic_field: false,
        };

        assert_eq!(
            rust_storage_override_for_field(RustCodegenMode::Integrated, &item, &field),
            Some("Vec<u8>")
        );
    }

    #[test]
    fn integrated_actor_ref_fields_use_runtime_serializer_type() {
        let item = SerializeCodegenItem {
            source_type_id: uuid!("11111111-1111-1111-1111-111111111111"),
            source_name: "ClientRef".to_owned(),
            role: ReflectedTypeRole::SupportType,
            is_reflection_marker: false,
            is_abstract: Some(false),
            factory: None,
            rtti_base_chain: Vec::new(),
            kind: SerializeCodegenItemKind::Struct,
            enum_underlying_type: None,
            fields: Vec::new(),
            variants: Vec::new(),
        };
        let field = SerializeCodegenField {
            source_name: "m_clientRef".to_owned(),
            source_type_id: uuid!("0638e28c-ab7b-4ba4-84ac-0353038e6fdc"),
            resolved_type: ResolvedType::Named {
                type_id: uuid!("0638e28c-ab7b-4ba4-84ac-0353038e6fdc"),
                source_name: "Amazon::Hub::ActorRef".to_owned(),
            },
            data_size: None,
            offset: None,
            flags: None,
            is_base_class: false,
            is_pointer: false,
            is_dynamic_field: false,
        };

        assert_eq!(
            rust_storage_override_for_field(RustCodegenMode::Integrated, &item, &field),
            Some("crate::refs::ClientActorRef")
        );
        assert_eq!(
            rust_storage_override_for_field(RustCodegenMode::Standalone, &item, &field),
            None
        );
    }
}
