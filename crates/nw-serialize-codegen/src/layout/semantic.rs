use std::collections::BTreeMap;

use uuid::Uuid;

use crate::ir::{SerializeCodegenField, SerializeCodegenItem};
use crate::layout::base::primary_base_chain_edges;
use crate::layout::path::{inheritance_scope_segment, source_namespace_segments};
use crate::naming::{ParsedSourceName, SourceNameKind};
use crate::role::ReflectedTypeRole;
use crate::types::{ResolvedType, ScalarType};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct SemanticWrapperTarget {
    pub wrapper: String,
    pub target: String,
}

impl SemanticWrapperTarget {
    pub(super) fn family_segment(
        &self,
        item: &SerializeCodegenItem,
        items_by_type_id: &BTreeMap<Uuid, &SerializeCodegenItem>,
    ) -> String {
        primary_base_chain_edges(item, items_by_type_id)
            .into_iter()
            .last()
            .map_or_else(
                || inheritance_scope_segment(&self.wrapper),
                |base| inheritance_scope_segment(&base.source_name),
            )
    }
}

pub(super) fn parsed_semantic_wrapper_target(source_name: &str) -> Option<SemanticWrapperTarget> {
    match ParsedSourceName::parse(source_name).kind {
        SourceNameKind::TemplateWrapper { wrapper, target } => {
            Some(SemanticWrapperTarget { wrapper, target })
        }
        SourceNameKind::LocalComponentRef { target, .. } => Some(SemanticWrapperTarget {
            wrapper: "LocalComponentRef".to_owned(),
            target,
        }),
        SourceNameKind::Plain
        | SourceNameKind::EditEnum { .. }
        | SourceNameKind::GetTypeNameFunction(_) => None,
    }
}

pub(super) fn semantic_wrapper_scope_segments(item: &SerializeCodegenItem) -> Option<Vec<String>> {
    if item.role != ReflectedTypeRole::SupportType
        || item.is_abstract != Some(false)
        || !source_namespace_segments(&item.source_name).is_empty()
    {
        return None;
    }

    let mut data_fields = item.fields.iter().filter(|field| !field.is_base_class);
    let field = data_fields.next()?;
    if data_fields.next().is_some() {
        return None;
    }

    if field.is_pointer || field.is_dynamic_field {
        return None;
    }

    semantic_scalar_scope(field)
}

fn semantic_scalar_scope(field: &SerializeCodegenField) -> Option<Vec<String>> {
    match field.resolved_type {
        ResolvedType::Scalar(ScalarType::AssetId) => {
            Some(vec!["az".to_owned(), "asset".to_owned()])
        }
        ResolvedType::Scalar(ScalarType::Crc32) => Some(vec!["az".to_owned(), "crc".to_owned()]),
        ResolvedType::Scalar(ScalarType::EntityId) => {
            Some(vec!["az".to_owned(), "entity".to_owned()])
        }
        ResolvedType::Scalar(ScalarType::Uuid) => Some(vec!["az".to_owned(), "uuid".to_owned()]),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use uuid::uuid;

    use crate::ir::{SerializeCodegenItemKind, SerializeCodegenRttiBase};

    use super::*;

    #[test]
    fn local_component_ref_names_target_the_referenced_component_type() {
        let target =
            parsed_semantic_wrapper_target("LocalComponentRef<TransformComponent>::GetTypeName")
                .expect("local component ref target");

        assert_eq!(target.wrapper, "LocalComponentRef");
        assert_eq!(target.target, "TransformComponent");
    }

    #[test]
    fn scalar_wrappers_scope_to_az_semantic_modules() {
        let entity_ref = item(
            "LocalEntityRef",
            vec![field(
                "EntityId",
                ResolvedType::Scalar(ScalarType::EntityId),
            )],
        );
        let crc_ref = item(
            "OwnedCrc",
            vec![field("m_value", ResolvedType::Scalar(ScalarType::Crc32))],
        );

        assert_eq!(
            semantic_wrapper_scope_segments(&entity_ref),
            Some(vec!["az".to_owned(), "entity".to_owned()])
        );
        assert_eq!(
            semantic_wrapper_scope_segments(&crc_ref),
            Some(vec!["az".to_owned(), "crc".to_owned()])
        );
    }

    #[test]
    fn scalar_wrapper_scope_requires_one_plain_data_field() {
        let pointer_wrapper = item(
            "PointerEntityRef",
            vec![SerializeCodegenField {
                is_pointer: true,
                ..field("EntityId", ResolvedType::Scalar(ScalarType::EntityId))
            }],
        );
        let multi_field_wrapper = item(
            "CompoundRef",
            vec![
                field("EntityId", ResolvedType::Scalar(ScalarType::EntityId)),
                field("Kind", ResolvedType::Scalar(ScalarType::I32)),
            ],
        );

        assert_eq!(semantic_wrapper_scope_segments(&pointer_wrapper), None);
        assert_eq!(semantic_wrapper_scope_segments(&multi_field_wrapper), None);
    }

    fn item(source_name: &str, fields: Vec<SerializeCodegenField>) -> SerializeCodegenItem {
        SerializeCodegenItem {
            source_type_id: uuid!("11111111-1111-1111-1111-111111111111"),
            source_name: source_name.to_owned(),
            role: ReflectedTypeRole::SupportType,
            is_reflection_marker: false,
            is_abstract: Some(false),
            factory: None,
            rtti_base_chain: Vec::<SerializeCodegenRttiBase>::new(),
            kind: SerializeCodegenItemKind::Struct,
            enum_underlying_type: None,
            fields,
            variants: Vec::new(),
        }
    }

    fn field(source_name: &str, resolved_type: ResolvedType) -> SerializeCodegenField {
        SerializeCodegenField {
            source_name: source_name.to_owned(),
            source_type_id: uuid!("22222222-2222-2222-2222-222222222222"),
            resolved_type,
            data_size: None,
            offset: None,
            flags: None,
            is_base_class: false,
            is_pointer: false,
            is_dynamic_field: false,
        }
    }
}
