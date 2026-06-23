use std::collections::{BTreeMap, BTreeSet};

use uuid::Uuid;

use crate::field_projection::projected_missing_reflected_types;
use crate::ir::{SerializeCodegenItem, SerializeCodegenItemKind, SerializeCodegenUnit};
use crate::native::native_reflected_type_name;
use crate::role::ReflectedTypeRole;
use crate::types::ResolvedType;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MissingReflectedBody {
    pub type_id: Uuid,
    pub source_name: Option<String>,
    pub owner_name: String,
    pub field_name: String,
    pub reason: String,
    pub reference_count: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MissingReflectedBodyPlaceholder {
    pub type_id: Uuid,
    pub source_name: String,
    pub owner_name: String,
    pub field_name: String,
    pub reason: String,
    pub reference_count: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompletedCodegenUnits {
    pub emitted: SerializeCodegenUnit,
    pub context: SerializeCodegenUnit,
    pub placeholders: Vec<MissingReflectedBodyPlaceholder>,
}

#[must_use]
pub fn missing_reflected_bodies_by_type(
    unit: &SerializeCodegenUnit,
) -> BTreeMap<Uuid, MissingReflectedBody> {
    let mut missing_by_type = BTreeMap::new();
    for missing in projected_missing_reflected_types(unit) {
        let source_name = native_reflected_type_name(missing.type_id).map(str::to_owned);
        let entry =
            missing_by_type
                .entry(missing.type_id)
                .or_insert_with(|| MissingReflectedBody {
                    type_id: missing.type_id,
                    source_name,
                    owner_name: missing.owner_name,
                    field_name: missing.field_name,
                    reason: missing.reason,
                    reference_count: 0,
                });
        entry.reference_count += 1;
    }
    missing_by_type
}

#[must_use]
pub fn complete_known_missing_reflected_bodies(
    emitted: SerializeCodegenUnit,
    context: SerializeCodegenUnit,
) -> CompletedCodegenUnits {
    let mut placeholders = missing_reflected_bodies_by_type(&emitted)
        .into_values()
        .filter_map(|missing| placeholder_from_missing(&emitted, &context, missing))
        .collect::<Vec<_>>();
    placeholders.sort_by(|a, b| a.type_id.cmp(&b.type_id));

    if placeholders.is_empty() {
        return CompletedCodegenUnits {
            emitted,
            context,
            placeholders,
        };
    }

    let placeholder_names = placeholders
        .iter()
        .map(|placeholder| (placeholder.type_id, placeholder.source_name.clone()))
        .collect::<BTreeMap<_, _>>();

    let mut emitted = emitted;
    rewrite_placeholder_refs(&mut emitted, &placeholder_names);
    append_placeholder_items(&mut emitted, &placeholders);

    let mut context = context;
    rewrite_placeholder_refs(&mut context, &placeholder_names);
    append_placeholder_items(&mut context, &placeholders);

    CompletedCodegenUnits {
        emitted,
        context,
        placeholders,
    }
}

fn placeholder_from_missing(
    emitted: &SerializeCodegenUnit,
    context: &SerializeCodegenUnit,
    missing: MissingReflectedBody,
) -> Option<MissingReflectedBodyPlaceholder> {
    if item_exists(emitted, missing.type_id) || item_exists(context, missing.type_id) {
        return None;
    }
    let source_name = missing.source_name?;
    Some(MissingReflectedBodyPlaceholder {
        type_id: missing.type_id,
        source_name,
        owner_name: missing.owner_name,
        field_name: missing.field_name,
        reason: missing.reason,
        reference_count: missing.reference_count,
    })
}

fn item_exists(unit: &SerializeCodegenUnit, type_id: Uuid) -> bool {
    unit.items.iter().any(|item| item.source_type_id == type_id)
}

fn append_placeholder_items(
    unit: &mut SerializeCodegenUnit,
    placeholders: &[MissingReflectedBodyPlaceholder],
) {
    let mut existing_type_ids = unit
        .items
        .iter()
        .map(|item| item.source_type_id)
        .collect::<BTreeSet<_>>();
    for placeholder in placeholders {
        if existing_type_ids.insert(placeholder.type_id) {
            unit.items.push(placeholder_item(placeholder));
        }
    }
}

fn placeholder_item(placeholder: &MissingReflectedBodyPlaceholder) -> SerializeCodegenItem {
    SerializeCodegenItem {
        source_type_id: placeholder.type_id,
        source_name: placeholder.source_name.clone(),
        role: ReflectedTypeRole::SupportType,
        is_reflection_marker: false,
        is_abstract: Some(false),
        factory: None,
        rtti_base_chain: Vec::new(),
        kind: SerializeCodegenItemKind::Struct,
        enum_underlying_type: None,
        fields: Vec::new(),
        variants: Vec::new(),
    }
}

fn rewrite_placeholder_refs(
    unit: &mut SerializeCodegenUnit,
    names_by_type_id: &BTreeMap<Uuid, String>,
) {
    for item in &mut unit.items {
        for field in &mut item.fields {
            rewrite_placeholder_resolved_type(&mut field.resolved_type, names_by_type_id);
        }
        if let Some(resolved) = &mut item.enum_underlying_type {
            rewrite_placeholder_resolved_type(resolved, names_by_type_id);
        }
    }
}

fn rewrite_placeholder_resolved_type(
    resolved: &mut ResolvedType,
    names_by_type_id: &BTreeMap<Uuid, String>,
) {
    match resolved {
        ResolvedType::Unknown { type_id, .. } => {
            if let Some(source_name) = names_by_type_id.get(type_id) {
                *resolved = ResolvedType::Named {
                    type_id: *type_id,
                    source_name: source_name.clone(),
                };
            }
        }
        ResolvedType::Sequence { element, .. } => {
            rewrite_placeholder_resolved_type(element, names_by_type_id);
        }
        ResolvedType::Map { key, value, .. } => {
            rewrite_placeholder_resolved_type(key, names_by_type_id);
            rewrite_placeholder_resolved_type(value, names_by_type_id);
        }
        ResolvedType::ReplicatedField { value }
        | ResolvedType::RangedInteger { value, .. }
        | ResolvedType::Optional { value } => {
            rewrite_placeholder_resolved_type(value, names_by_type_id);
        }
        ResolvedType::Pair { first, second } => {
            rewrite_placeholder_resolved_type(first, names_by_type_id);
            rewrite_placeholder_resolved_type(second, names_by_type_id);
        }
        ResolvedType::Pointer { target, .. } => {
            rewrite_placeholder_resolved_type(target, names_by_type_id);
        }
        ResolvedType::Tuple { elements } => {
            for element in elements {
                rewrite_placeholder_resolved_type(element, names_by_type_id);
            }
        }
        ResolvedType::Scalar(_)
        | ResolvedType::Named { .. }
        | ResolvedType::Asset { .. }
        | ResolvedType::Uid { .. }
        | ResolvedType::ByteStream => {}
    }
}

#[cfg(test)]
mod tests {
    use uuid::uuid;

    use crate::{
        MapKind, ReflectedTypeRole, ResolvedType, ScalarType, SequenceKind, SerializeCodegenField,
        SerializeCodegenItem, SerializeCodegenItemKind, SerializeCodegenUnit,
        SerializeCodegenVariant,
    };

    use super::*;

    #[test]
    fn completes_known_missing_reflected_body_with_named_placeholder() {
        let known_missing = uuid!("e95d9900-33d1-4696-9b40-4640b1da544c");
        let owner = uuid!("11111111-1111-1111-1111-111111111111");
        let emitted = SerializeCodegenUnit {
            items: vec![item(
                owner,
                "BuildableGridComponentClientFacet",
                vec![field(
                    "m_gridSidesActiveClientView",
                    ResolvedType::Sequence {
                        kind: SequenceKind::Vector,
                        capacity: None,
                        element: Box::new(ResolvedType::Unknown {
                            type_id: known_missing,
                            reason: "reflected type `ClientView` is present on member type metadata but not as a SerializeContext class".to_owned(),
                        }),
                    },
                )],
            )],
        };

        let completed = complete_known_missing_reflected_bodies(
            emitted,
            SerializeCodegenUnit { items: Vec::new() },
        );

        assert_eq!(completed.placeholders.len(), 1);
        assert_eq!(completed.placeholders[0].source_name, "ClientView");
        assert!(
            completed
                .emitted
                .items
                .iter()
                .any(|item| item.source_type_id == known_missing
                    && item.source_name == "ClientView"
                    && item.fields.is_empty())
        );
        let owner = completed
            .emitted
            .items
            .iter()
            .find(|item| item.source_type_id == owner)
            .expect("owner item");
        assert!(matches!(
            &owner.fields[0].resolved_type,
            ResolvedType::Sequence { element, .. }
                if matches!(
                    element.as_ref(),
                    ResolvedType::Named { type_id, source_name }
                    if *type_id == known_missing && source_name == "ClientView"
                )
        ));
    }

    #[test]
    fn leaves_unknown_missing_reflected_body_unresolved() {
        let unknown_missing = uuid!("99999999-9999-9999-9999-999999999999");
        let emitted = SerializeCodegenUnit {
            items: vec![item(
                uuid!("11111111-1111-1111-1111-111111111111"),
                "Owner",
                vec![field(
                    "m_missing",
                    ResolvedType::Unknown {
                        type_id: unknown_missing,
                        reason: "no reflected class body".to_owned(),
                    },
                )],
            )],
        };

        let completed = complete_known_missing_reflected_bodies(
            emitted,
            SerializeCodegenUnit { items: Vec::new() },
        );

        assert!(completed.placeholders.is_empty());
        assert!(matches!(
            completed.emitted.items[0].fields[0].resolved_type,
            ResolvedType::Unknown { type_id, .. } if type_id == unknown_missing
        ));
    }

    #[test]
    fn rewrites_nested_known_missing_refs_inside_maps() {
        let known_missing = uuid!("d43e3d53-10b1-43ec-a503-e83d4208bc30");
        let emitted = SerializeCodegenUnit {
            items: vec![item(
                uuid!("11111111-1111-1111-1111-111111111111"),
                "WaterLevelComponentClientFacet",
                vec![field(
                    "m_unifiedInteractOptions",
                    ResolvedType::Map {
                        kind: MapKind::UnorderedMap,
                        key: Box::new(ResolvedType::Scalar(ScalarType::U32)),
                        value: Box::new(ResolvedType::Unknown {
                            type_id: known_missing,
                            reason: "reflected type `UnifiedInteractOption` is known but has no reflected class body".to_owned(),
                        }),
                    },
                )],
            )],
        };

        let completed = complete_known_missing_reflected_bodies(
            emitted,
            SerializeCodegenUnit { items: Vec::new() },
        );

        assert_eq!(
            completed.placeholders[0].source_name,
            "UnifiedInteractOption"
        );
        assert!(matches!(
            &completed.emitted.items[0].fields[0].resolved_type,
            ResolvedType::Map { value, .. }
                if matches!(
                    value.as_ref(),
                    ResolvedType::Named { type_id, source_name }
                    if *type_id == known_missing && source_name == "UnifiedInteractOption"
                )
        ));
    }

    fn item(
        source_type_id: Uuid,
        source_name: &str,
        fields: Vec<SerializeCodegenField>,
    ) -> SerializeCodegenItem {
        SerializeCodegenItem {
            source_type_id,
            source_name: source_name.to_owned(),
            role: ReflectedTypeRole::SupportType,
            is_reflection_marker: false,
            is_abstract: Some(false),
            factory: None,
            rtti_base_chain: Vec::new(),
            kind: SerializeCodegenItemKind::Struct,
            enum_underlying_type: None,
            fields,
            variants: Vec::<SerializeCodegenVariant>::new(),
        }
    }

    fn field(name: &str, resolved_type: ResolvedType) -> SerializeCodegenField {
        SerializeCodegenField {
            source_name: name.to_owned(),
            source_type_id: uuid!("22222222-2222-2222-2222-222222222222"),
            offset: None,
            data_size: None,
            flags: None,
            is_base_class: false,
            is_pointer: false,
            is_dynamic_field: false,
            resolved_type,
        }
    }
}
