use std::collections::BTreeMap;

use uuid::Uuid;

use crate::ir::{SerializeCodegenItem, SerializeCodegenItemKind, SerializeCodegenUnit};
use crate::role::ReflectedTypeRole;

use super::path::source_scope_segments;
use super::{
    LayoutBaseEdge, LayoutConcreteSlotBinding, LayoutConcreteSlotCandidate, LayoutFieldOwnerEdge,
    LayoutIndex, LayoutRootAudit, LayoutRootReport, LayoutScopeReason, LayoutSlotAnchor,
    primary_base_chain_edges, reflected_base_type_ids,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LayoutAnalysisReport {
    pub items: Vec<LayoutAnalysisItem>,
}

impl LayoutAnalysisReport {
    #[must_use]
    pub fn from_codegen_unit(unit: &SerializeCodegenUnit) -> Self {
        Self::from_codegen_unit_with_context(unit, unit)
    }

    #[must_use]
    pub fn from_codegen_unit_with_context(
        emitted_unit: &SerializeCodegenUnit,
        context_unit: &SerializeCodegenUnit,
    ) -> Self {
        let context_index = context_unit.index();
        let layout_index = LayoutIndex::from_codegen_unit(context_unit);
        let items_by_type_id = context_index.items_by_type_id();
        debug_assert!(
            emitted_unit
                .items
                .iter()
                .all(|item| items_by_type_id.contains_key(&item.source_type_id))
        );
        let base_type_ids = reflected_base_type_ids(context_unit);
        let items = emitted_unit
            .items
            .iter()
            .filter(|item| !item.is_reflection_marker)
            .map(|item| {
                let emitted_scope = layout_index.emitted_scope(item, &items_by_type_id);
                LayoutAnalysisItem {
                    source_type_id: item.source_type_id,
                    source_name: item.source_name.clone(),
                    role: item.role,
                    is_abstract: item.is_abstract,
                    factory: item.factory.clone(),
                    serialized_field_count: item.fields.len(),
                    serialized_base_field_count: serialized_base_field_count(item),
                    serialized_data_field_count: serialized_data_field_count(item),
                    serialized_shape: serialized_shape(item),
                    is_base_family_root: base_type_ids.contains(&item.source_type_id),
                    namespace_segments: source_scope_segments(&item.source_name),
                    primary_base_chain: primary_base_chain_edges(item, &items_by_type_id),
                    direct_derived_source_names: direct_derived_source_names(
                        item,
                        &items_by_type_id,
                        &layout_index,
                    ),
                    slot_anchor: layout_index.slot_anchor(item).cloned(),
                    field_owner_edges: layout_index
                        .field_owner_edges_by_target_type_id
                        .get(&item.source_type_id)
                        .cloned()
                        .unwrap_or_default(),
                    concrete_slot_binding: layout_index.concrete_slot_binding(item).cloned(),
                    concrete_slot_candidates: layout_index
                        .concrete_slot_candidates(item)
                        .to_owned(),
                    has_ambiguous_concrete_slot_binding: layout_index
                        .has_ambiguous_concrete_slot_binding(item),
                    emitted_scope_segments: emitted_scope.segments,
                    emitted_scope_reason: emitted_scope.reason,
                }
            })
            .collect();
        Self { items }
    }

    #[must_use]
    pub fn item_by_source_name(&self, source_name: &str) -> Option<&LayoutAnalysisItem> {
        self.items
            .iter()
            .find(|item| item.source_name == source_name)
    }

    #[must_use]
    pub fn item_by_type_id(&self, type_id: Uuid) -> Option<&LayoutAnalysisItem> {
        self.items
            .iter()
            .find(|item| item.source_type_id == type_id)
    }

    #[must_use]
    pub fn to_text(&self) -> String {
        let mut out = String::new();
        for item in &self.items {
            out.push_str("type ");
            out.push_str(&item.source_name);
            out.push(' ');
            out.push_str(&item.source_type_id.hyphenated().to_string());
            out.push('\n');
            out.push_str("  namespace ");
            out.push_str(&item.namespace_segments.join("/"));
            out.push('\n');
            out.push_str("  shape ");
            out.push_str(item.serialized_shape.as_str());
            out.push_str(" fields=");
            out.push_str(&item.serialized_field_count.to_string());
            out.push_str(" base=");
            out.push_str(&item.serialized_base_field_count.to_string());
            out.push_str(" data=");
            out.push_str(&item.serialized_data_field_count.to_string());
            out.push('\n');
            if let Some(factory) = &item.factory {
                out.push_str("  factory ");
                out.push_str(factory);
                out.push('\n');
            }
            out.push_str("  emitted ");
            out.push_str(&item.emitted_scope_segments.join("/"));
            out.push_str(" reason=");
            out.push_str(item.emitted_scope_reason.as_str());
            out.push('\n');
            out.push_str("  base_family_root ");
            out.push_str(if item.is_base_family_root {
                "true"
            } else {
                "false"
            });
            out.push('\n');
            for edge in &item.primary_base_chain {
                out.push_str("  base ");
                out.push_str(&edge.source_name);
                out.push(' ');
                out.push_str(&edge.type_id.hyphenated().to_string());
                out.push(' ');
                out.push_str(if edge.matches_reflected_type {
                    "matched"
                } else {
                    "edge-local"
                });
                out.push('\n');
            }
            if !item.direct_derived_source_names.is_empty() {
                out.push_str("  direct_derived ");
                out.push_str(&item.direct_derived_source_names.join(", "));
                out.push('\n');
            }
            if let Some(anchor) = &item.slot_anchor {
                out.push_str("  slot_anchor ");
                out.push_str(&anchor.owner_source_name);
                out.push('.');
                out.push_str(&anchor.owner_field_name);
                out.push_str(" -> ");
                out.push_str(&anchor.slot_source_name);
                out.push('\n');
            }
            if !item.field_owner_edges.is_empty() {
                out.push_str("  field_owners ");
                out.push_str(
                    &item
                        .field_owner_edges
                        .iter()
                        .map(|edge| format!("{}.{}", edge.owner_source_name, edge.field_name))
                        .collect::<Vec<_>>()
                        .join(", "),
                );
                out.push('\n');
            }
            if let Some(binding) = &item.concrete_slot_binding {
                out.push_str("  concrete_slot ");
                out.push_str(&binding.owner_source_name);
                out.push('.');
                out.push_str(&binding.owner_field_name);
                out.push_str(" -> ");
                out.push_str(&binding.slot_source_name);
                out.push('\n');
            } else if item.has_ambiguous_concrete_slot_binding {
                out.push_str("  concrete_slot ambiguous candidates=");
                out.push_str(&item.concrete_slot_candidates.len().to_string());
                out.push('\n');
            }
        }
        out
    }

    #[must_use]
    pub fn root_report(&self) -> LayoutRootReport {
        LayoutRootReport::from_analysis_report(self)
    }

    #[must_use]
    pub fn root_audit(&self) -> LayoutRootAudit {
        LayoutRootAudit::from_analysis_report(self)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LayoutAnalysisItem {
    pub source_type_id: Uuid,
    pub source_name: String,
    pub role: ReflectedTypeRole,
    pub is_abstract: Option<bool>,
    pub factory: Option<String>,
    pub serialized_field_count: usize,
    pub serialized_base_field_count: usize,
    pub serialized_data_field_count: usize,
    pub serialized_shape: LayoutSerializedShape,
    pub is_base_family_root: bool,
    pub namespace_segments: Vec<String>,
    pub primary_base_chain: Vec<LayoutBaseEdge>,
    pub direct_derived_source_names: Vec<String>,
    pub slot_anchor: Option<LayoutSlotAnchor>,
    pub field_owner_edges: Vec<LayoutFieldOwnerEdge>,
    pub concrete_slot_binding: Option<LayoutConcreteSlotBinding>,
    pub concrete_slot_candidates: Vec<LayoutConcreteSlotCandidate>,
    pub has_ambiguous_concrete_slot_binding: bool,
    pub emitted_scope_segments: Vec<String>,
    pub emitted_scope_reason: LayoutScopeReason,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LayoutSerializedShape {
    Enum,
    AbstractBaseWithoutData,
    AbstractBaseWithData,
    ConcreteStatelessType,
    ConcreteDataType,
    UnknownAbstractnessWithoutData,
    UnknownAbstractnessWithData,
}

impl LayoutSerializedShape {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Enum => "enum",
            Self::AbstractBaseWithoutData => "abstract-base-without-data",
            Self::AbstractBaseWithData => "abstract-base-with-data",
            Self::ConcreteStatelessType => "concrete-stateless-type",
            Self::ConcreteDataType => "concrete-data-type",
            Self::UnknownAbstractnessWithoutData => "unknown-abstractness-without-data",
            Self::UnknownAbstractnessWithData => "unknown-abstractness-with-data",
        }
    }
}

fn direct_derived_source_names(
    base: &SerializeCodegenItem,
    items_by_type_id: &BTreeMap<Uuid, &SerializeCodegenItem>,
    layout_index: &LayoutIndex,
) -> Vec<String> {
    layout_index
        .direct_derived_type_ids_by_base_type_id
        .get(&base.source_type_id)
        .map(Vec::as_slice)
        .unwrap_or_default()
        .iter()
        .filter_map(|type_id| items_by_type_id.get(type_id))
        .map(|item| item.source_name.clone())
        .collect()
}

fn serialized_shape(item: &SerializeCodegenItem) -> LayoutSerializedShape {
    match item.kind {
        SerializeCodegenItemKind::Enum => LayoutSerializedShape::Enum,
        SerializeCodegenItemKind::Struct => {
            let has_data_fields = serialized_data_field_count(item) > 0;
            match (item.is_abstract, has_data_fields) {
                (Some(true), false) => LayoutSerializedShape::AbstractBaseWithoutData,
                (Some(true), true) => LayoutSerializedShape::AbstractBaseWithData,
                (Some(false), false) => LayoutSerializedShape::ConcreteStatelessType,
                (Some(false), true) => LayoutSerializedShape::ConcreteDataType,
                (None, false) => LayoutSerializedShape::UnknownAbstractnessWithoutData,
                (None, true) => LayoutSerializedShape::UnknownAbstractnessWithData,
            }
        }
    }
}

fn serialized_base_field_count(item: &SerializeCodegenItem) -> usize {
    item.fields
        .iter()
        .filter(|field| field.is_base_class)
        .count()
}

fn serialized_data_field_count(item: &SerializeCodegenItem) -> usize {
    item.fields.len() - serialized_base_field_count(item)
}
