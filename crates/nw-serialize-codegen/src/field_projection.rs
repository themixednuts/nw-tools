use std::collections::{BTreeMap, BTreeSet};

use uuid::Uuid;

use crate::ir::{
    MissingReflectedType, SerializeCodegenField, SerializeCodegenItem, SerializeCodegenUnit,
    collect_resolved_named_type_ids,
};
use crate::types::ResolvedType;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CodegenFieldProjection {
    RegularField,
    MaterializedBaseField,
    MarkerBaseField,
    InterfaceBase,
    SkippedBase,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CodegenFieldTypeProjection<'a> {
    Reflected(&'a ResolvedType),
    FixedOpaqueBytes { byte_len: u32 },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CodegenTypeReferenceProjection {
    MaterializedFields,
    MaterializedFieldsAndInterfaceEdges,
    DataFreeAbstractInterfacesAndMaterializedFields,
}

impl CodegenFieldProjection {
    #[must_use]
    pub const fn is_materialized(self) -> bool {
        matches!(
            self,
            Self::RegularField | Self::MaterializedBaseField | Self::MarkerBaseField
        )
    }

    #[must_use]
    pub const fn is_referenced(self) -> bool {
        !matches!(self, Self::SkippedBase)
    }
}

impl CodegenTypeReferenceProjection {
    #[must_use]
    pub fn field_should_reference_type(
        self,
        owner: &SerializeCodegenItem,
        field: &SerializeCodegenField,
        items_by_type_id: &BTreeMap<Uuid, &SerializeCodegenItem>,
    ) -> bool {
        if !field.is_base_class {
            return true;
        }

        match self {
            Self::MaterializedFields => {
                classify_codegen_field(field, items_by_type_id).is_materialized()
            }
            Self::MaterializedFieldsAndInterfaceEdges => {
                classify_codegen_field(field, items_by_type_id).is_referenced()
            }
            Self::DataFreeAbstractInterfacesAndMaterializedFields => {
                if owner.is_abstract == Some(true)
                    && !item_has_materialized_payload(owner, items_by_type_id)
                {
                    base_class_is_abstract(field, items_by_type_id)
                } else {
                    classify_codegen_field(field, items_by_type_id).is_materialized()
                }
            }
        }
    }
}

impl CodegenFieldTypeProjection<'_> {
    #[must_use]
    pub const fn fixed_opaque_byte_len(self) -> Option<u32> {
        match self {
            Self::FixedOpaqueBytes { byte_len } => Some(byte_len),
            Self::Reflected(_) => None,
        }
    }

    #[must_use]
    pub fn references_missing_type(self) -> bool {
        match self {
            Self::FixedOpaqueBytes { .. } => false,
            Self::Reflected(resolved) => resolved_type_references_missing(resolved),
        }
    }

    pub fn collect_missing_type_ids(self, out: &mut Vec<Uuid>) {
        if let Self::Reflected(resolved) = self {
            collect_missing_type_ids(resolved, out);
        }
    }
}

#[must_use]
pub fn classify_codegen_field(
    field: &SerializeCodegenField,
    items_by_type_id: &BTreeMap<Uuid, &SerializeCodegenItem>,
) -> CodegenFieldProjection {
    if !field.is_base_class {
        return CodegenFieldProjection::RegularField;
    }

    let Some(base_item) = base_class_item(field, items_by_type_id) else {
        return CodegenFieldProjection::SkippedBase;
    };
    if base_item.is_reflection_marker {
        return CodegenFieldProjection::SkippedBase;
    }
    if base_class_has_materialized_payload(field, items_by_type_id) {
        CodegenFieldProjection::MaterializedBaseField
    } else if base_item.is_abstract == Some(false) {
        CodegenFieldProjection::MarkerBaseField
    } else {
        CodegenFieldProjection::InterfaceBase
    }
}

#[must_use]
pub fn classify_codegen_field_type(
    field: &SerializeCodegenField,
) -> CodegenFieldTypeProjection<'_> {
    if matches!(field.resolved_type, ResolvedType::Unknown { .. })
        && let Some(byte_len) = field.data_size
    {
        return CodegenFieldTypeProjection::FixedOpaqueBytes { byte_len };
    }
    CodegenFieldTypeProjection::Reflected(&field.resolved_type)
}

#[must_use]
pub fn projected_missing_reflected_types(unit: &SerializeCodegenUnit) -> Vec<MissingReflectedType> {
    let mut missing = Vec::new();
    for item in &unit.items {
        for field in &item.fields {
            let context = MissingReflectedTypeContext {
                owner_name: &item.source_name,
                field_name: &field.source_name,
                is_base_class: field.is_base_class,
            };
            collect_projected_missing_field_types(field, &context, &mut missing);
        }
        if let Some(resolved) = &item.enum_underlying_type {
            collect_projected_missing_types(
                resolved,
                &MissingReflectedTypeContext {
                    owner_name: &item.source_name,
                    field_name: "<enum_underlying>",
                    is_base_class: false,
                },
                &mut missing,
            );
        }
    }
    missing
}

#[must_use]
pub fn projected_missing_reflected_type_reasons(
    unit: &SerializeCodegenUnit,
) -> BTreeMap<Uuid, String> {
    let mut missing = BTreeMap::new();
    for missing_type in projected_missing_reflected_types(unit) {
        missing
            .entry(missing_type.type_id)
            .or_insert(missing_type.reason);
    }
    missing
}

#[must_use]
pub fn codegen_item_references_missing_type(
    item: &SerializeCodegenItem,
    items_by_type_id: &BTreeMap<Uuid, &SerializeCodegenItem>,
    projection: CodegenTypeReferenceProjection,
) -> bool {
    item.fields.iter().any(|field| {
        projection.field_should_reference_type(item, field, items_by_type_id)
            && classify_codegen_field_type(field).references_missing_type()
    }) || item
        .enum_underlying_type
        .as_ref()
        .is_some_and(resolved_type_references_missing)
}

#[must_use]
pub fn codegen_item_missing_type_ids(
    item: &SerializeCodegenItem,
    items_by_type_id: &BTreeMap<Uuid, &SerializeCodegenItem>,
    projection: CodegenTypeReferenceProjection,
) -> Vec<Uuid> {
    let mut type_ids = Vec::new();
    for field in &item.fields {
        if projection.field_should_reference_type(item, field, items_by_type_id) {
            classify_codegen_field_type(field).collect_missing_type_ids(&mut type_ids);
        }
    }
    if let Some(resolved) = &item.enum_underlying_type {
        collect_missing_type_ids(resolved, &mut type_ids);
    }
    type_ids
}

pub fn visit_codegen_item_reference_roots(
    item: &SerializeCodegenItem,
    items_by_type_id: &BTreeMap<Uuid, &SerializeCodegenItem>,
    projection: CodegenTypeReferenceProjection,
    mut visit: impl FnMut(&ResolvedType),
) {
    for field in &item.fields {
        if projection.field_should_reference_type(item, field, items_by_type_id)
            && let CodegenFieldTypeProjection::Reflected(resolved) =
                classify_codegen_field_type(field)
        {
            visit(resolved);
        }
    }
    if let Some(resolved) = &item.enum_underlying_type {
        visit(resolved);
    }
}

#[must_use]
pub fn codegen_item_referenced_type_ids(
    item: &SerializeCodegenItem,
    items_by_type_id: &BTreeMap<Uuid, &SerializeCodegenItem>,
    projection: CodegenTypeReferenceProjection,
) -> BTreeSet<Uuid> {
    let mut type_ids = BTreeSet::new();
    visit_codegen_item_reference_roots(item, items_by_type_id, projection, |resolved| {
        collect_resolved_named_type_ids(resolved, &mut type_ids);
    });
    type_ids
}

#[must_use]
pub fn base_class_is_abstract(
    field: &SerializeCodegenField,
    items_by_type_id: &BTreeMap<Uuid, &SerializeCodegenItem>,
) -> bool {
    base_class_item(field, items_by_type_id).is_some_and(|item| item.is_abstract == Some(true))
}

#[must_use]
pub fn base_class_has_materialized_payload(
    field: &SerializeCodegenField,
    items_by_type_id: &BTreeMap<Uuid, &SerializeCodegenItem>,
) -> bool {
    base_class_has_materialized_payload_with_visiting(field, items_by_type_id, &mut BTreeSet::new())
}

#[must_use]
pub fn item_has_materialized_payload(
    item: &SerializeCodegenItem,
    items_by_type_id: &BTreeMap<Uuid, &SerializeCodegenItem>,
) -> bool {
    let mut visiting = BTreeSet::new();
    visiting.insert(item.source_type_id);
    let materialized = item.fields.iter().any(|field| {
        !field.is_base_class
            || base_class_has_materialized_payload_with_visiting(
                field,
                items_by_type_id,
                &mut visiting,
            )
    });
    visiting.remove(&item.source_type_id);
    materialized
}

fn base_class_has_materialized_payload_with_visiting(
    field: &SerializeCodegenField,
    items_by_type_id: &BTreeMap<Uuid, &SerializeCodegenItem>,
    visiting: &mut BTreeSet<Uuid>,
) -> bool {
    let Some(item) = base_class_item(field, items_by_type_id) else {
        return false;
    };
    if item.is_reflection_marker || !visiting.insert(item.source_type_id) {
        return false;
    }
    let materialized = item.fields.iter().any(|field| {
        !field.is_base_class
            || base_class_has_materialized_payload_with_visiting(field, items_by_type_id, visiting)
    });
    visiting.remove(&item.source_type_id);
    materialized
}

fn base_class_item<'a>(
    field: &SerializeCodegenField,
    items_by_type_id: &'a BTreeMap<Uuid, &SerializeCodegenItem>,
) -> Option<&'a SerializeCodegenItem> {
    let ResolvedType::Named {
        type_id,
        source_name,
    } = &field.resolved_type
    else {
        return None;
    };
    items_by_type_id
        .get(type_id)
        .copied()
        .filter(|item| item.source_name == *source_name)
}

fn resolved_type_references_missing(resolved: &ResolvedType) -> bool {
    match resolved {
        ResolvedType::Unknown { .. } => true,
        ResolvedType::Sequence { element, .. }
        | ResolvedType::RangedInteger { value: element, .. }
        | ResolvedType::Pointer {
            target: element, ..
        }
        | ResolvedType::Optional { value: element }
        | ResolvedType::ReplicatedField { value: element } => {
            resolved_type_references_missing(element)
        }
        ResolvedType::Map { key, value, .. }
        | ResolvedType::Pair {
            first: key,
            second: value,
        } => resolved_type_references_missing(key) || resolved_type_references_missing(value),
        ResolvedType::Tuple { elements } => elements.iter().any(resolved_type_references_missing),
        ResolvedType::Scalar(_)
        | ResolvedType::Named { .. }
        | ResolvedType::Asset { .. }
        | ResolvedType::Uid { .. }
        | ResolvedType::ByteStream => false,
    }
}

fn collect_missing_type_ids(resolved: &ResolvedType, out: &mut Vec<Uuid>) {
    match resolved {
        ResolvedType::Unknown { type_id, .. } => out.push(*type_id),
        ResolvedType::Sequence { element, .. }
        | ResolvedType::RangedInteger { value: element, .. }
        | ResolvedType::Pointer {
            target: element, ..
        }
        | ResolvedType::Optional { value: element }
        | ResolvedType::ReplicatedField { value: element } => {
            collect_missing_type_ids(element, out);
        }
        ResolvedType::Map { key, value, .. }
        | ResolvedType::Pair {
            first: key,
            second: value,
        } => {
            collect_missing_type_ids(key, out);
            collect_missing_type_ids(value, out);
        }
        ResolvedType::Tuple { elements } => {
            for element in elements {
                collect_missing_type_ids(element, out);
            }
        }
        ResolvedType::Scalar(_)
        | ResolvedType::Named { .. }
        | ResolvedType::Asset { .. }
        | ResolvedType::Uid { .. }
        | ResolvedType::ByteStream => {}
    }
}

struct MissingReflectedTypeContext<'a> {
    owner_name: &'a str,
    field_name: &'a str,
    is_base_class: bool,
}

fn collect_projected_missing_field_types(
    field: &SerializeCodegenField,
    context: &MissingReflectedTypeContext<'_>,
    missing: &mut Vec<MissingReflectedType>,
) {
    if let CodegenFieldTypeProjection::Reflected(resolved) = classify_codegen_field_type(field) {
        collect_projected_missing_types(resolved, context, missing);
    }
}

fn collect_projected_missing_types(
    resolved: &ResolvedType,
    context: &MissingReflectedTypeContext<'_>,
    missing: &mut Vec<MissingReflectedType>,
) {
    match resolved {
        ResolvedType::Unknown { type_id, reason } => missing.push(MissingReflectedType {
            owner_name: context.owner_name.to_owned(),
            field_name: context.field_name.to_owned(),
            type_id: *type_id,
            reason: reason.clone(),
            is_base_class: context.is_base_class,
        }),
        ResolvedType::Sequence { element, .. }
        | ResolvedType::RangedInteger { value: element, .. }
        | ResolvedType::Pointer {
            target: element, ..
        }
        | ResolvedType::Optional { value: element }
        | ResolvedType::ReplicatedField { value: element } => {
            collect_projected_missing_types(element, context, missing);
        }
        ResolvedType::Map { key, value, .. }
        | ResolvedType::Pair {
            first: key,
            second: value,
        } => {
            collect_projected_missing_types(key, context, missing);
            collect_projected_missing_types(value, context, missing);
        }
        ResolvedType::Tuple { elements } => {
            for element in elements {
                collect_projected_missing_types(element, context, missing);
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

    use crate::ir::{SerializeCodegenField, SerializeCodegenItem, SerializeCodegenItemKind};
    use crate::role::ReflectedTypeRole;
    use crate::types::{ResolvedType, ScalarType};

    use super::*;

    #[test]
    fn materializes_dataful_base_classes_as_fields() {
        let base = struct_item(
            uuid!("11111111-1111-1111-1111-111111111111"),
            "AZ::Component",
            false,
            vec![regular_field("id", ResolvedType::Scalar(ScalarType::U64))],
        );
        let derived = struct_item(
            uuid!("22222222-2222-2222-2222-222222222222"),
            "UiElementComponent",
            false,
            vec![
                base_field(&base),
                regular_field("id", ResolvedType::Scalar(ScalarType::U32)),
            ],
        );
        let items = items_by_id([&base, &derived]);

        assert_eq!(
            classify_codegen_field(&derived.fields[0], &items),
            CodegenFieldProjection::MaterializedBaseField
        );
        assert!(item_has_materialized_payload(&derived, &items));
    }

    #[test]
    fn keeps_data_free_abstract_bases_as_interface_edges() {
        let base = struct_item(
            uuid!("11111111-1111-1111-1111-111111111111"),
            "IStimulusPayloadBase",
            true,
            Vec::new(),
        );
        let derived = struct_item(
            uuid!("22222222-2222-2222-2222-222222222222"),
            "StimulusPayload",
            false,
            vec![base_field(&base)],
        );
        let items = items_by_id([&base, &derived]);

        assert_eq!(
            classify_codegen_field(&derived.fields[0], &items),
            CodegenFieldProjection::InterfaceBase
        );
        assert!(base_class_is_abstract(&derived.fields[0], &items));
    }

    #[test]
    fn keeps_data_free_concrete_bases_as_marker_fields() {
        let base = struct_item(
            uuid!("11111111-1111-1111-1111-111111111111"),
            "Facet",
            false,
            Vec::new(),
        );
        let derived = struct_item(
            uuid!("22222222-2222-2222-2222-222222222222"),
            "ServerFacet",
            false,
            vec![base_field(&base)],
        );
        let items = items_by_id([&base, &derived]);

        assert_eq!(
            classify_codegen_field(&derived.fields[0], &items),
            CodegenFieldProjection::MarkerBaseField
        );
        assert!(
            CodegenTypeReferenceProjection::MaterializedFields.field_should_reference_type(
                &derived,
                &derived.fields[0],
                &items
            )
        );
    }

    #[test]
    fn skips_reflection_marker_bases() {
        let marker = SerializeCodegenItem {
            source_type_id: uuid!("11111111-1111-1111-1111-111111111111"),
            source_name: "Facet".to_owned(),
            role: ReflectedTypeRole::SupportType,
            is_reflection_marker: true,
            is_abstract: Some(true),
            factory: None,
            rtti_base_chain: Vec::new(),
            kind: SerializeCodegenItemKind::Struct,
            enum_underlying_type: None,
            fields: Vec::new(),
            variants: Vec::new(),
        };
        let derived = struct_item(
            uuid!("22222222-2222-2222-2222-222222222222"),
            "ServerFacet",
            false,
            vec![base_field(&marker)],
        );
        let items = items_by_id([&marker, &derived]);

        assert_eq!(
            classify_codegen_field(&derived.fields[0], &items),
            CodegenFieldProjection::SkippedBase
        );
    }

    #[test]
    fn treats_top_level_unknown_with_size_as_fixed_opaque_bytes() {
        let field = SerializeCodegenField {
            source_name: "payload".to_owned(),
            source_type_id: uuid!("aaaaaaaa-aaaa-aaaa-aaaa-aaaaaaaaaaaa"),
            resolved_type: ResolvedType::Unknown {
                type_id: uuid!("aaaaaaaa-aaaa-aaaa-aaaa-aaaaaaaaaaaa"),
                reason: "type id is not present in SerializeContext".to_owned(),
            },
            data_size: Some(16),
            offset: None,
            flags: None,
            is_base_class: false,
            is_pointer: false,
            is_dynamic_field: false,
        };

        let projection = classify_codegen_field_type(&field);

        assert_eq!(
            projection,
            CodegenFieldTypeProjection::FixedOpaqueBytes { byte_len: 16 }
        );
        assert_eq!(projection.fixed_opaque_byte_len(), Some(16));
        assert!(!projection.references_missing_type());
        let mut missing = Vec::new();
        projection.collect_missing_type_ids(&mut missing);
        assert!(missing.is_empty());
    }

    #[test]
    fn keeps_nested_unknown_types_as_missing_references() {
        let missing_id = uuid!("aaaaaaaa-aaaa-aaaa-aaaa-aaaaaaaaaaaa");
        let field = SerializeCodegenField {
            source_name: "payloads".to_owned(),
            source_type_id: uuid!("bbbbbbbb-bbbb-bbbb-bbbb-bbbbbbbbbbbb"),
            resolved_type: ResolvedType::Sequence {
                kind: crate::types::SequenceKind::Vector,
                element: Box::new(ResolvedType::Unknown {
                    type_id: missing_id,
                    reason: "type id is not present in SerializeContext".to_owned(),
                }),
                capacity: None,
            },
            data_size: Some(16),
            offset: None,
            flags: None,
            is_base_class: false,
            is_pointer: false,
            is_dynamic_field: false,
        };

        let projection = classify_codegen_field_type(&field);
        let mut missing = Vec::new();
        projection.collect_missing_type_ids(&mut missing);

        assert!(projection.references_missing_type());
        assert_eq!(missing, vec![missing_id]);
    }

    #[test]
    fn projected_missing_types_skip_fixed_opaque_fields_but_keep_nested_unknowns() {
        let fixed_id = uuid!("aaaaaaaa-aaaa-aaaa-aaaa-aaaaaaaaaaaa");
        let nested_id = uuid!("bbbbbbbb-bbbb-bbbb-bbbb-bbbbbbbbbbbb");
        let unit = SerializeCodegenUnit {
            items: vec![struct_item(
                uuid!("11111111-1111-1111-1111-111111111111"),
                "Example",
                false,
                vec![
                    SerializeCodegenField {
                        source_name: "payload".to_owned(),
                        source_type_id: fixed_id,
                        resolved_type: ResolvedType::Unknown {
                            type_id: fixed_id,
                            reason: "fixed payload".to_owned(),
                        },
                        data_size: Some(16),
                        offset: None,
                        flags: None,
                        is_base_class: false,
                        is_pointer: false,
                        is_dynamic_field: false,
                    },
                    SerializeCodegenField {
                        source_name: "payloads".to_owned(),
                        source_type_id: nested_id,
                        resolved_type: ResolvedType::Sequence {
                            kind: crate::types::SequenceKind::Vector,
                            element: Box::new(ResolvedType::Unknown {
                                type_id: nested_id,
                                reason: "missing nested payload".to_owned(),
                            }),
                            capacity: None,
                        },
                        data_size: Some(16),
                        offset: None,
                        flags: None,
                        is_base_class: false,
                        is_pointer: false,
                        is_dynamic_field: false,
                    },
                ],
            )],
        };

        let missing = projected_missing_reflected_types(&unit);

        assert_eq!(missing.len(), 1);
        assert_eq!(missing[0].type_id, nested_id);
        assert_eq!(missing[0].field_name, "payloads");
    }

    #[test]
    fn type_reference_projection_distinguishes_interface_and_materialized_shapes() {
        let base = struct_item(
            uuid!("11111111-1111-1111-1111-111111111111"),
            "IStimulusPayloadBase",
            true,
            Vec::new(),
        );
        let abstract_owner = struct_item(
            uuid!("22222222-2222-2222-2222-222222222222"),
            "IStimulusPayload",
            true,
            vec![base_field(&base)],
        );
        let concrete_owner = struct_item(
            uuid!("33333333-3333-3333-3333-333333333333"),
            "StimulusPayload",
            false,
            vec![base_field(&base)],
        );
        let items = items_by_id([&base, &abstract_owner, &concrete_owner]);

        assert!(
            CodegenTypeReferenceProjection::MaterializedFieldsAndInterfaceEdges
                .field_should_reference_type(&concrete_owner, &concrete_owner.fields[0], &items)
        );
        assert!(
            CodegenTypeReferenceProjection::DataFreeAbstractInterfacesAndMaterializedFields
                .field_should_reference_type(&abstract_owner, &abstract_owner.fields[0], &items)
        );
        assert!(
            !CodegenTypeReferenceProjection::DataFreeAbstractInterfacesAndMaterializedFields
                .field_should_reference_type(&concrete_owner, &concrete_owner.fields[0], &items)
        );
        assert!(
            !CodegenTypeReferenceProjection::MaterializedFields.field_should_reference_type(
                &concrete_owner,
                &concrete_owner.fields[0],
                &items
            )
        );
    }

    #[test]
    fn missing_type_ids_follow_projection_and_fixed_opaque_policy() {
        let fixed_id = uuid!("aaaaaaaa-aaaa-aaaa-aaaa-aaaaaaaaaaaa");
        let nested_id = uuid!("bbbbbbbb-bbbb-bbbb-bbbb-bbbbbbbbbbbb");
        let enum_id = uuid!("cccccccc-cccc-cccc-cccc-cccccccccccc");
        let item = SerializeCodegenItem {
            source_type_id: uuid!("11111111-1111-1111-1111-111111111111"),
            source_name: "Example".to_owned(),
            role: ReflectedTypeRole::SupportType,
            is_reflection_marker: false,
            is_abstract: Some(false),
            factory: None,
            rtti_base_chain: Vec::new(),
            kind: SerializeCodegenItemKind::Struct,
            enum_underlying_type: Some(ResolvedType::Unknown {
                type_id: enum_id,
                reason: "missing enum backing".to_owned(),
            }),
            fields: vec![
                SerializeCodegenField {
                    source_name: "payload".to_owned(),
                    source_type_id: fixed_id,
                    resolved_type: ResolvedType::Unknown {
                        type_id: fixed_id,
                        reason: "fixed payload".to_owned(),
                    },
                    data_size: Some(16),
                    offset: None,
                    flags: None,
                    is_base_class: false,
                    is_pointer: false,
                    is_dynamic_field: false,
                },
                SerializeCodegenField {
                    source_name: "payloads".to_owned(),
                    source_type_id: nested_id,
                    resolved_type: ResolvedType::Sequence {
                        kind: crate::types::SequenceKind::Vector,
                        element: Box::new(ResolvedType::Unknown {
                            type_id: nested_id,
                            reason: "missing nested payload".to_owned(),
                        }),
                        capacity: None,
                    },
                    data_size: Some(16),
                    offset: None,
                    flags: None,
                    is_base_class: false,
                    is_pointer: false,
                    is_dynamic_field: false,
                },
            ],
            variants: Vec::new(),
        };
        let items = items_by_id([&item]);

        assert!(codegen_item_references_missing_type(
            &item,
            &items,
            CodegenTypeReferenceProjection::MaterializedFields
        ));
        assert_eq!(
            codegen_item_missing_type_ids(
                &item,
                &items,
                CodegenTypeReferenceProjection::MaterializedFields
            ),
            vec![nested_id, enum_id]
        );
    }

    #[test]
    fn referenced_type_ids_follow_projection_and_nested_named_types() {
        let data_base = struct_item(
            uuid!("11111111-1111-1111-1111-111111111111"),
            "DataBase",
            false,
            vec![regular_field("m_id", ResolvedType::Scalar(ScalarType::U32))],
        );
        let interface_base = struct_item(
            uuid!("22222222-2222-2222-2222-222222222222"),
            "IStimulusPayloadBase",
            true,
            Vec::new(),
        );
        let marker_base = SerializeCodegenItem {
            source_type_id: uuid!("33333333-3333-3333-3333-333333333333"),
            source_name: "ReflectionMarker".to_owned(),
            role: ReflectedTypeRole::SupportType,
            is_reflection_marker: true,
            is_abstract: Some(true),
            factory: None,
            rtti_base_chain: Vec::new(),
            kind: SerializeCodegenItemKind::Struct,
            enum_underlying_type: None,
            fields: Vec::new(),
            variants: Vec::new(),
        };
        let nested_type_id = uuid!("44444444-4444-4444-4444-444444444444");
        let enum_type_id = uuid!("55555555-5555-5555-5555-555555555555");
        let concrete = SerializeCodegenItem {
            source_type_id: uuid!("66666666-6666-6666-6666-666666666666"),
            source_name: "ConcretePayload".to_owned(),
            role: ReflectedTypeRole::SupportType,
            is_reflection_marker: false,
            is_abstract: Some(false),
            factory: None,
            rtti_base_chain: Vec::new(),
            kind: SerializeCodegenItemKind::Struct,
            enum_underlying_type: Some(ResolvedType::Named {
                type_id: enum_type_id,
                source_name: "PayloadKind".to_owned(),
            }),
            fields: vec![
                base_field(&data_base),
                base_field(&interface_base),
                base_field(&marker_base),
                regular_field(
                    "m_nested",
                    ResolvedType::Sequence {
                        kind: crate::types::SequenceKind::Vector,
                        element: Box::new(ResolvedType::Named {
                            type_id: nested_type_id,
                            source_name: "NestedPayload".to_owned(),
                        }),
                        capacity: None,
                    },
                ),
            ],
            variants: Vec::new(),
        };
        let abstract_leaf = struct_item(
            uuid!("77777777-7777-7777-7777-777777777777"),
            "IConcretePayload",
            true,
            vec![base_field(&interface_base)],
        );
        let items = items_by_id([
            &data_base,
            &interface_base,
            &marker_base,
            &concrete,
            &abstract_leaf,
        ]);

        assert_eq!(
            codegen_item_referenced_type_ids(
                &concrete,
                &items,
                CodegenTypeReferenceProjection::MaterializedFields
            ),
            BTreeSet::from([data_base.source_type_id, nested_type_id, enum_type_id])
        );
        assert_eq!(
            codegen_item_referenced_type_ids(
                &concrete,
                &items,
                CodegenTypeReferenceProjection::MaterializedFieldsAndInterfaceEdges
            ),
            BTreeSet::from([
                data_base.source_type_id,
                interface_base.source_type_id,
                nested_type_id,
                enum_type_id,
            ])
        );
        assert_eq!(
            codegen_item_referenced_type_ids(
                &abstract_leaf,
                &items,
                CodegenTypeReferenceProjection::DataFreeAbstractInterfacesAndMaterializedFields
            ),
            BTreeSet::from([interface_base.source_type_id])
        );
        assert!(
            !codegen_item_referenced_type_ids(
                &concrete,
                &items,
                CodegenTypeReferenceProjection::MaterializedFieldsAndInterfaceEdges
            )
            .contains(&marker_base.source_type_id)
        );
    }

    #[test]
    fn projected_missing_type_reasons_dedupe_by_type_id() {
        let missing_id = uuid!("aaaaaaaa-aaaa-aaaa-aaaa-aaaaaaaaaaaa");
        let unit = SerializeCodegenUnit {
            items: vec![struct_item(
                uuid!("11111111-1111-1111-1111-111111111111"),
                "Example",
                false,
                vec![
                    SerializeCodegenField {
                        source_name: "left".to_owned(),
                        source_type_id: missing_id,
                        resolved_type: ResolvedType::Unknown {
                            type_id: missing_id,
                            reason: "first reason wins".to_owned(),
                        },
                        data_size: None,
                        offset: None,
                        flags: None,
                        is_base_class: false,
                        is_pointer: false,
                        is_dynamic_field: false,
                    },
                    SerializeCodegenField {
                        source_name: "right".to_owned(),
                        source_type_id: missing_id,
                        resolved_type: ResolvedType::Unknown {
                            type_id: missing_id,
                            reason: "second reason ignored".to_owned(),
                        },
                        data_size: None,
                        offset: None,
                        flags: None,
                        is_base_class: false,
                        is_pointer: false,
                        is_dynamic_field: false,
                    },
                ],
            )],
        };

        let reasons = projected_missing_reflected_type_reasons(&unit);

        assert_eq!(reasons.len(), 1);
        assert_eq!(
            reasons.get(&missing_id),
            Some(&"first reason wins".to_owned())
        );
    }

    fn items_by_id<'a>(
        items: impl IntoIterator<Item = &'a SerializeCodegenItem>,
    ) -> BTreeMap<Uuid, &'a SerializeCodegenItem> {
        items
            .into_iter()
            .map(|item| (item.source_type_id, item))
            .collect()
    }

    fn struct_item(
        type_id: Uuid,
        source_name: &str,
        is_abstract: bool,
        fields: Vec<SerializeCodegenField>,
    ) -> SerializeCodegenItem {
        SerializeCodegenItem {
            source_type_id: type_id,
            source_name: source_name.to_owned(),
            role: ReflectedTypeRole::SupportType,
            is_reflection_marker: false,
            is_abstract: Some(is_abstract),
            factory: None,
            rtti_base_chain: Vec::new(),
            kind: SerializeCodegenItemKind::Struct,
            enum_underlying_type: None,
            fields,
            variants: Vec::new(),
        }
    }

    fn regular_field(source_name: &str, resolved_type: ResolvedType) -> SerializeCodegenField {
        SerializeCodegenField {
            source_name: source_name.to_owned(),
            source_type_id: uuid!("72039442-eb38-4d42-a1ad-cb68f7e0eef6"),
            resolved_type,
            data_size: None,
            offset: None,
            flags: None,
            is_base_class: false,
            is_pointer: false,
            is_dynamic_field: false,
        }
    }

    fn base_field(item: &SerializeCodegenItem) -> SerializeCodegenField {
        SerializeCodegenField {
            source_name: "BaseClass1".to_owned(),
            source_type_id: item.source_type_id,
            resolved_type: ResolvedType::Named {
                type_id: item.source_type_id,
                source_name: item.source_name.clone(),
            },
            data_size: None,
            offset: None,
            flags: None,
            is_base_class: true,
            is_pointer: false,
            is_dynamic_field: false,
        }
    }
}
