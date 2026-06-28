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

struct IntegratedRustTypeContext<'a> {
    name_plan: &'a RustNamePlan,
    items_by_type_id: &'a BTreeMap<Uuid, &'a SerializeCodegenItem>,
    source_types: Option<&'a RustSourceTypeIndex>,
    current_module: &'a str,
    polymorphic_value_type_names: Option<&'a BTreeMap<Uuid, String>>,
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
        polymorphic_value_type_names: &BTreeMap<Uuid, String>,
    ) -> Vec<RustFieldPlan> {
        let mut field_counts = BTreeMap::<String, usize>::new();
        let mut seen_fields = Vec::<&SerializeCodegenField>::new();
        item.fields
            .iter()
            .filter(|field| {
                if seen_fields.contains(field) {
                    false
                } else {
                    seen_fields.push(*field);
                    true
                }
            })
            .filter(|field| should_materialize_rust_field(field, items_by_type_id))
            .map(|field| {
                let mut plan = self.plan_serialize_field(
                    item,
                    field,
                    name_plan,
                    items_by_type_id,
                    source_types,
                    current_module,
                    polymorphic_value_type_names,
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
                let type_id = field.source_type_id;
                if !seen.insert(type_id) {
                    return None;
                }
                let source_name = base_source_name(field, items_by_type_id);
                Some(RustRttiBasePlan {
                    source_type_id: type_id,
                    source_name,
                    rust_type: self.rust_type_for_field(
                        field,
                        name_plan,
                        items_by_type_id,
                        source_types,
                        current_module,
                        None,
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
        polymorphic_value_type_names: &BTreeMap<Uuid, String>,
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
                    polymorphic_value_type_names,
                )
            })
            .unwrap_or_else(|| {
                self.rust_type_for_field(
                    field,
                    name_plan,
                    items_by_type_id,
                    source_types,
                    current_module,
                    (!field.is_base_class).then_some(polymorphic_value_type_names),
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
        polymorphic_value_type_names: Option<&BTreeMap<Uuid, String>>,
    ) -> String {
        match classify_codegen_field_type(field) {
            CodegenFieldTypeProjection::FixedOpaqueBytes { byte_len } => {
                format!("[u8; {byte_len}]")
            }
            CodegenFieldTypeProjection::Reflected(resolved_type)
                if matches!(self.mode, RustCodegenMode::Standalone) =>
            {
                self.rust_type_for_resolved_type(
                    resolved_type,
                    name_plan,
                    items_by_type_id,
                    polymorphic_value_type_names,
                )
            }
            CodegenFieldTypeProjection::Reflected(resolved_type) => self
                .integrated_rust_type_for_resolved_type(
                    resolved_type,
                    name_plan,
                    items_by_type_id,
                    source_types,
                    current_module,
                    polymorphic_value_type_names,
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
        polymorphic_value_type_names: &BTreeMap<Uuid, String>,
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
            RustCodegenMode::Standalone => self.rust_type_for_resolved_type(
                target.resolved,
                name_plan,
                items_by_type_id,
                Some(polymorphic_value_type_names),
            ),
            RustCodegenMode::Integrated => self.integrated_rust_type_for_resolved_type(
                target.resolved,
                name_plan,
                items_by_type_id,
                source_types,
                current_module,
                Some(polymorphic_value_type_names),
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
        polymorphic_value_type_names: Option<&BTreeMap<Uuid, String>>,
    ) -> String {
        match resolved {
            ResolvedType::Named {
                type_id,
                source_name,
            } => {
                if let Some(rust_type) =
                    polymorphic_value_type_names.and_then(|type_names| type_names.get(type_id))
                {
                    rust_type.clone()
                } else if matches!(self.mode, RustCodegenMode::Integrated)
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
            } => self.rust_sequence_type(
                *kind,
                element,
                *capacity,
                name_plan,
                items_by_type_id,
                polymorphic_value_type_names,
            ),
            ResolvedType::Map { kind, key, value } => self.rust_map_type(
                *kind,
                key,
                value,
                name_plan,
                items_by_type_id,
                polymorphic_value_type_names,
            ),
            ResolvedType::ReplicatedField { value } => {
                let value = self.rust_type_for_resolved_type(
                    value,
                    name_plan,
                    items_by_type_id,
                    polymorphic_value_type_names,
                );
                format!("Option<{value}>")
            }
            ResolvedType::RangedInteger { value, .. } => self.rust_type_for_resolved_type(
                value,
                name_plan,
                items_by_type_id,
                polymorphic_value_type_names,
            ),
            ResolvedType::Pair { first, second } => {
                let first = self.rust_type_for_resolved_type(
                    first,
                    name_plan,
                    items_by_type_id,
                    polymorphic_value_type_names,
                );
                let second = self.rust_type_for_resolved_type(
                    second,
                    name_plan,
                    items_by_type_id,
                    polymorphic_value_type_names,
                );
                format!("({first}, {second})")
            }
            ResolvedType::Pointer { target, .. } => {
                let target = self.rust_type_for_resolved_type(
                    target,
                    name_plan,
                    items_by_type_id,
                    polymorphic_value_type_names,
                );
                format!("Option<{target}>")
            }
            ResolvedType::Optional { value } => {
                let value = self.rust_type_for_resolved_type(
                    value,
                    name_plan,
                    items_by_type_id,
                    polymorphic_value_type_names,
                );
                format!("Option<{value}>")
            }
            ResolvedType::Tuple { elements } => {
                let elements = elements
                    .iter()
                    .map(|element| {
                        self.rust_type_for_resolved_type(
                            element,
                            name_plan,
                            items_by_type_id,
                            polymorphic_value_type_names,
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
        polymorphic_value_type_names: Option<&BTreeMap<Uuid, String>>,
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
            } => polymorphic_value_type_names
                .and_then(|type_names| type_names.get(type_id))
                .cloned()
                .or_else(|| integrated_custom_field_type(source_name).map(str::to_owned))
                .unwrap_or_else(|| name_plan.reference_name(*type_id, source_name)),
            ResolvedType::Sequence {
                kind,
                element,
                capacity,
            } => self.integrated_rust_sequence_type(
                *kind,
                element,
                *capacity,
                &IntegratedRustTypeContext {
                    name_plan,
                    items_by_type_id,
                    source_types,
                    current_module,
                    polymorphic_value_type_names,
                },
            ),
            ResolvedType::Map { kind, key, value } => self.integrated_rust_map_type(
                *kind,
                key,
                value,
                &IntegratedRustTypeContext {
                    name_plan,
                    items_by_type_id,
                    source_types,
                    current_module,
                    polymorphic_value_type_names,
                },
            ),
            ResolvedType::ReplicatedField { value } => {
                let value = self.integrated_rust_type_for_resolved_type(
                    value,
                    name_plan,
                    items_by_type_id,
                    source_types,
                    current_module,
                    polymorphic_value_type_names,
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
                    polymorphic_value_type_names,
                ),
            ResolvedType::Pair { first, second } => {
                let first = self.integrated_rust_type_for_resolved_type(
                    first,
                    name_plan,
                    items_by_type_id,
                    source_types,
                    current_module,
                    polymorphic_value_type_names,
                );
                let second = self.integrated_rust_type_for_resolved_type(
                    second,
                    name_plan,
                    items_by_type_id,
                    source_types,
                    current_module,
                    polymorphic_value_type_names,
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
                    polymorphic_value_type_names,
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
                    polymorphic_value_type_names,
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
                            polymorphic_value_type_names,
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
        context: &IntegratedRustTypeContext<'_>,
    ) -> String {
        let element_type = self.integrated_rust_type_for_resolved_type(
            element,
            context.name_plan,
            context.items_by_type_id,
            context.source_types,
            context.current_module,
            context.polymorphic_value_type_names,
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
        context: &IntegratedRustTypeContext<'_>,
    ) -> String {
        let key_type = self.integrated_rust_type_for_resolved_type(
            key,
            context.name_plan,
            context.items_by_type_id,
            context.source_types,
            context.current_module,
            context.polymorphic_value_type_names,
        );
        let value_type = self.integrated_rust_type_for_resolved_type(
            value,
            context.name_plan,
            context.items_by_type_id,
            context.source_types,
            context.current_module,
            context.polymorphic_value_type_names,
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
        polymorphic_value_type_names: Option<&BTreeMap<Uuid, String>>,
    ) -> String {
        let element_type = self.rust_type_for_resolved_type(
            element,
            name_plan,
            items_by_type_id,
            polymorphic_value_type_names,
        );
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
        polymorphic_value_type_names: Option<&BTreeMap<Uuid, String>>,
    ) -> String {
        let key_type = self.rust_type_for_resolved_type(key, name_plan, items_by_type_id, None);
        let value_type = self.rust_type_for_resolved_type(
            value,
            name_plan,
            items_by_type_id,
            polymorphic_value_type_names,
        );
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

fn base_source_name(
    field: &SerializeCodegenField,
    items_by_type_id: &BTreeMap<Uuid, &SerializeCodegenItem>,
) -> String {
    items_by_type_id
        .get(&field.source_type_id)
        .map(|item| item.source_name.clone())
        .or_else(|| match &field.resolved_type {
            ResolvedType::Named { source_name, .. } => Some(source_name.clone()),
            _ => None,
        })
        .unwrap_or_else(|| field.source_name.clone())
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
        "Amazon::Hub::ActorRef" => Some("crate::refs::HubActorRef"),
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
    use crate::rust::types::RustTypeOptions;
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
            Some("crate::refs::HubActorRef")
        );
        assert_eq!(
            rust_storage_override_for_field(RustCodegenMode::Standalone, &item, &field),
            None
        );
    }

    #[test]
    fn rtti_base_planning_uses_base_field_source_type_id() {
        let component_type_id = uuid!("edfac2cf-f75d-43be-b26b-f35821b29247");
        let action_list_type_id = uuid!("30ed0ace-51dd-48b9-ba41-2fa6775cd106");
        let base_item = SerializeCodegenItem {
            source_type_id: component_type_id,
            source_name: "AZ::Component".to_owned(),
            role: ReflectedTypeRole::SupportType,
            is_reflection_marker: false,
            is_abstract: Some(true),
            factory: None,
            rtti_base_chain: Vec::new(),
            kind: SerializeCodegenItemKind::Struct,
            enum_underlying_type: None,
            fields: Vec::new(),
            variants: Vec::new(),
        };
        let item = SerializeCodegenItem {
            source_type_id: action_list_type_id,
            source_name: "ActionListComponent".to_owned(),
            role: ReflectedTypeRole::FacetedComponent,
            is_reflection_marker: false,
            is_abstract: Some(false),
            factory: None,
            rtti_base_chain: Vec::new(),
            kind: SerializeCodegenItemKind::Struct,
            enum_underlying_type: None,
            fields: vec![SerializeCodegenField {
                source_name: "BaseClass1".to_owned(),
                source_type_id: component_type_id,
                resolved_type: ResolvedType::Unknown {
                    type_id: component_type_id,
                    reason: "base class resolved through source type id".to_owned(),
                },
                data_size: None,
                offset: Some(0),
                flags: Some(2),
                is_base_class: true,
                is_pointer: false,
                is_dynamic_field: false,
            }],
            variants: Vec::new(),
        };
        let items_by_type_id = BTreeMap::from([
            (base_item.source_type_id, &base_item),
            (item.source_type_id, &item),
        ]);
        let planner = RustFieldPlanner::new(
            RustCodegenMode::Standalone,
            RustTypeRenderer::new(RustTypeOptions::default()),
        );

        let bases = planner.plan_rtti_bases(
            &item,
            &items_by_type_id,
            &RustNamePlan::default(),
            None,
            "types",
        );

        assert_eq!(bases.len(), 1);
        assert_eq!(bases[0].source_type_id, component_type_id);
        assert_eq!(bases[0].source_name, "AZ::Component");
    }

    #[test]
    fn exact_duplicate_fields_are_materialized_once() {
        let duplicate = SerializeCodegenField {
            source_name: "m_rarityLevel".to_owned(),
            source_type_id: uuid!("72039442-eb38-4d42-a1ad-cb68f7e0eef6"),
            resolved_type: ResolvedType::Scalar(ScalarType::I32),
            data_size: Some(4),
            offset: Some(264),
            flags: Some(0),
            is_base_class: false,
            is_pointer: false,
            is_dynamic_field: false,
        };
        let item = SerializeCodegenItem {
            source_type_id: uuid!("72f23ce6-385d-4ff4-a494-c42f9069c686"),
            source_name: "ContractItemSimpleData".to_owned(),
            role: ReflectedTypeRole::SupportType,
            is_reflection_marker: false,
            is_abstract: Some(false),
            factory: None,
            rtti_base_chain: Vec::new(),
            kind: SerializeCodegenItemKind::Struct,
            enum_underlying_type: None,
            fields: vec![duplicate.clone(), duplicate],
            variants: Vec::new(),
        };
        let planner = RustFieldPlanner::new(
            RustCodegenMode::Standalone,
            RustTypeRenderer::new(RustTypeOptions::default()),
        );

        let fields = planner.plan_struct_fields(
            &item,
            &BTreeMap::new(),
            &RustNamePlan::default(),
            None,
            "types",
            &BTreeMap::new(),
        );

        assert_eq!(fields.len(), 1);
        assert_eq!(fields[0].rust_name, "rarity_level");
    }
}
