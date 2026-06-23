use std::collections::BTreeMap;

use uuid::Uuid;

use crate::ir::SerializeCodegenItem;
use crate::layout::base::primary_base_chain_edges;
use crate::layout::path::source_scope_segments;
use crate::role::ReflectedTypeRole;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LayoutScopeDecision {
    pub segments: Vec<String>,
    pub reason: LayoutScopeReason,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum LayoutScopeReason {
    ConcreteSlotBinding,
    ConcreteSlotOwner,
    DescendantCommonFamily,
    FieldOwner,
    InheritanceFamily,
    InheritedNamespace,
    Root,
    SemanticWrapper,
    SemanticWrapperTarget,
    SlotAnchor,
    SourceNamespace,
}

impl LayoutScopeReason {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::ConcreteSlotBinding => "concrete-slot-binding",
            Self::ConcreteSlotOwner => "concrete-slot-owner",
            Self::DescendantCommonFamily => "descendant-common-family",
            Self::FieldOwner => "field-owner",
            Self::InheritanceFamily => "inheritance-family",
            Self::InheritedNamespace => "inherited-namespace",
            Self::Root => "root",
            Self::SemanticWrapper => "semantic-wrapper",
            Self::SemanticWrapperTarget => "semantic-wrapper-target",
            Self::SlotAnchor => "slot-anchor",
            Self::SourceNamespace => "source-namespace",
        }
    }
}

pub(super) fn common_scope_prefix(scopes: &[Vec<String>]) -> Option<Vec<String>> {
    let mut scopes = scopes.iter();
    let mut common = scopes.next()?.clone();
    for scope in scopes {
        common.truncate(
            common
                .iter()
                .zip(scope)
                .take_while(|(left, right)| left == right)
                .count(),
        );
        if common.is_empty() {
            return None;
        }
    }
    (!common.is_empty()).then_some(common)
}

pub(super) fn common_scope_shared_segment(
    scopes: &[Vec<String>],
    excluded_segment: &str,
) -> Option<Vec<String>> {
    let first_scope = scopes.first()?;
    first_scope
        .iter()
        .find(|segment| {
            segment.as_str() != excluded_segment
                && scopes
                    .iter()
                    .all(|scope| scope.iter().any(|candidate| candidate == *segment))
        })
        .map(|segment| vec![segment.clone()])
}

pub(super) fn common_scope_suffix_before_segment(
    scopes: &[Vec<String>],
    segment: &str,
) -> Option<Vec<String>> {
    let mut parents = Vec::new();
    for scope in scopes {
        let index = scope.iter().position(|candidate| candidate == segment)?;
        parents.push(&scope[..index]);
    }
    let mut parents = parents.into_iter();
    let mut common = parents.next()?.to_vec();
    for parent in parents {
        let keep = common
            .iter()
            .rev()
            .zip(parent.iter().rev())
            .take_while(|(left, right)| left == right)
            .count();
        if keep == 0 {
            return None;
        }
        common = common[common.len() - keep..].to_vec();
    }
    (!common.is_empty()).then_some(common)
}

pub(super) fn inherited_namespace_scope_segments(
    item: &SerializeCodegenItem,
    items_by_type_id: &BTreeMap<Uuid, &SerializeCodegenItem>,
) -> Vec<String> {
    let own_segments = source_scope_segments(&item.source_name);
    if !own_segments.is_empty() {
        return own_segments;
    }

    primary_base_chain_edges(item, items_by_type_id)
        .into_iter()
        .filter(|edge| edge.matches_reflected_type)
        .find_map(|edge| {
            let base_item = items_by_type_id.get(&edge.type_id)?;
            if !should_inherit_base_namespace(item, base_item) {
                return None;
            }
            let segments = source_scope_segments(&base_item.source_name);
            (!segments.is_empty()).then_some(segments)
        })
        .unwrap_or_default()
}

fn should_inherit_base_namespace(
    item: &SerializeCodegenItem,
    base_item: &SerializeCodegenItem,
) -> bool {
    item.role == base_item.role
        && item.role != ReflectedTypeRole::SupportType
        && !item.role.is_az_component_like()
}

#[cfg(test)]
mod tests {
    use uuid::uuid;

    use crate::ir::{SerializeCodegenField, SerializeCodegenItemKind, SerializeCodegenRttiBase};
    use crate::types::ResolvedType;

    use super::*;

    #[test]
    fn common_scope_prefix_returns_shared_leading_scope() {
        let scopes = vec![
            path(["components", "faceted_components", "player"]),
            path(["components", "faceted_components", "inventory"]),
            path(["components", "faceted_components", "groups"]),
        ];

        assert_eq!(
            common_scope_prefix(&scopes),
            Some(path(["components", "faceted_components"]))
        );
    }

    #[test]
    fn common_scope_shared_segment_ignores_excluded_family_segment() {
        let scopes = vec![
            path(["components", "player_component", "client_facet"]),
            path(["components", "inventory_component", "client_facet"]),
        ];

        assert_eq!(
            common_scope_shared_segment(&scopes, "client_facet"),
            Some(path(["components"]))
        );
    }

    #[test]
    fn common_scope_suffix_before_segment_returns_shared_parent_tail() {
        let scopes = vec![
            path(["az", "components", "faceted_components", "facet"]),
            path(["nw", "components", "faceted_components", "facet"]),
        ];

        assert_eq!(
            common_scope_suffix_before_segment(&scopes, "facet"),
            Some(path(["components", "faceted_components"]))
        );
    }

    #[test]
    fn inherited_namespace_uses_own_namespace_first() {
        let item = item(
            uuid!("aaaaaaaa-aaaa-aaaa-aaaa-aaaaaaaaaaaa"),
            "Example::Child",
            ReflectedTypeRole::AzEntity,
            Vec::new(),
        );

        assert_eq!(
            inherited_namespace_scope_segments(&item, &BTreeMap::new()),
            path(["example"])
        );
    }

    #[test]
    fn inherited_namespace_follows_matching_non_component_base_namespace() {
        let base = item(
            uuid!("aaaaaaaa-aaaa-aaaa-aaaa-aaaaaaaaaaaa"),
            "Example::EntityBase",
            ReflectedTypeRole::AzEntity,
            Vec::new(),
        );
        let child = item(
            uuid!("bbbbbbbb-bbbb-bbbb-bbbb-bbbbbbbbbbbb"),
            "EntityChild",
            ReflectedTypeRole::AzEntity,
            vec![base_field(base.source_type_id, &base.source_name)],
        );
        let items = BTreeMap::from([(base.source_type_id, &base), (child.source_type_id, &child)]);

        assert_eq!(
            inherited_namespace_scope_segments(&child, &items),
            path(["example"])
        );
    }

    #[test]
    fn inherited_namespace_does_not_leak_support_base_namespaces() {
        let base = item(
            uuid!("aaaaaaaa-aaaa-aaaa-aaaa-aaaaaaaaaaaa"),
            "Example::SupportBase",
            ReflectedTypeRole::SupportType,
            Vec::new(),
        );
        let child = item(
            uuid!("bbbbbbbb-bbbb-bbbb-bbbb-bbbbbbbbbbbb"),
            "SupportChild",
            ReflectedTypeRole::SupportType,
            vec![base_field(base.source_type_id, &base.source_name)],
        );
        let items = BTreeMap::from([(base.source_type_id, &base), (child.source_type_id, &child)]);

        assert_eq!(
            inherited_namespace_scope_segments(&child, &items),
            Vec::<String>::new()
        );
    }

    fn path<const N: usize>(segments: [&str; N]) -> Vec<String> {
        segments.into_iter().map(str::to_owned).collect()
    }

    fn item(
        source_type_id: Uuid,
        source_name: &str,
        role: ReflectedTypeRole,
        fields: Vec<SerializeCodegenField>,
    ) -> SerializeCodegenItem {
        SerializeCodegenItem {
            source_type_id,
            source_name: source_name.to_owned(),
            role,
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

    fn base_field(type_id: Uuid, source_name: &str) -> SerializeCodegenField {
        SerializeCodegenField {
            source_name: source_name.to_owned(),
            source_type_id: type_id,
            resolved_type: ResolvedType::Named {
                type_id,
                source_name: source_name.to_owned(),
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
