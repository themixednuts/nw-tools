use std::collections::{BTreeMap, BTreeSet};

use uuid::Uuid;

use crate::field_projection::item_has_materialized_payload;
use crate::ir::{
    SerializeCodegenField, SerializeCodegenItem, SerializeCodegenUnit,
    collect_resolved_named_type_ids,
};
use crate::naming::{rust_field_ident, rust_type_ident};
use crate::role::ReflectedTypeRole;
use crate::types::ResolvedType;

mod analysis;
mod base;
mod order;
mod path;
mod relation;
mod root;
mod scope;
mod semantic;
mod type_path;

pub use analysis::{LayoutAnalysisItem, LayoutAnalysisReport, LayoutSerializedShape};
pub use base::reflected_base_type_ids;
pub use order::dependency_ordered_codegen_items;
pub use path::{inheritance_scope_segment, sanitize_path_segment, source_namespace_segments};
pub use relation::{
    LayoutBaseEdge, LayoutConcreteSlotBinding, LayoutConcreteSlotCandidate,
    LayoutConcreteSlotMatchKind, LayoutFieldOwnerEdge, LayoutSlotAnchor, LayoutSlotOwnerEdge,
};
pub use root::{
    LayoutRootAudit, LayoutRootFinding, LayoutRootFindingKind, LayoutRootItem, LayoutRootReport,
};
pub use scope::{LayoutScopeDecision, LayoutScopeReason};
pub use type_path::{LayoutPathSet, LayoutTypePath, layout_path_starts_with};

use base::{primary_base_chain, primary_base_chain_edges, primary_base_chains_by_type_id};
use path::{concrete_type_scope_segment, source_scope_segments};
use scope::{
    common_scope_prefix, common_scope_shared_segment, common_scope_suffix_before_segment,
    inherited_namespace_scope_segments,
};
use semantic::{parsed_semantic_wrapper_target, semantic_wrapper_scope_segments};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LayoutIndex {
    pub base_type_ids: BTreeSet<Uuid>,
    pub direct_derived_type_ids_by_base_type_id: BTreeMap<Uuid, Vec<Uuid>>,
    pub slot_owner_edges_by_target_type_id: BTreeMap<Uuid, Vec<LayoutSlotOwnerEdge>>,
    pub field_owner_edges_by_target_type_id: BTreeMap<Uuid, Vec<LayoutFieldOwnerEdge>>,
    pub slot_anchors: BTreeMap<Uuid, LayoutSlotAnchor>,
    pub concrete_slot_candidates: BTreeMap<Uuid, Vec<LayoutConcreteSlotCandidate>>,
    pub concrete_slot_bindings: BTreeMap<Uuid, LayoutConcreteSlotBinding>,
    pub ambiguous_concrete_slot_type_ids: BTreeSet<Uuid>,
    pub concrete_slot_owner_type_ids: BTreeSet<Uuid>,
    slot_family_owner_type_ids: BTreeSet<Uuid>,
    type_ids_by_source_name: BTreeMap<String, Vec<Uuid>>,
    type_ids_by_rust_ident: BTreeMap<String, Vec<Uuid>>,
}

impl LayoutIndex {
    #[must_use]
    pub fn from_codegen_unit(unit: &SerializeCodegenUnit) -> Self {
        let index = unit.index();
        Self::from_items_by_type_id(index.items_by_type_id())
    }

    fn from_items_by_type_id(items_by_type_id: &BTreeMap<Uuid, &SerializeCodegenItem>) -> Self {
        let type_ids_by_source_name = type_ids_by_source_name(items_by_type_id);
        let type_ids_by_rust_ident = type_ids_by_rust_ident(items_by_type_id);
        let primary_base_chains_by_type_id = primary_base_chains_by_type_id(items_by_type_id);
        let direct_derived_type_ids_by_base_type_id =
            direct_derived_type_ids_by_base_type_id(items_by_type_id);
        let base_type_ids = reflected_base_type_ids_from_items(items_by_type_id);
        let slot_owner_edges_by_target_type_id =
            slot_owner_edges_by_target_type_id(items_by_type_id);
        let slot_family_owner_type_ids =
            slot_family_owner_type_ids(&slot_owner_edges_by_target_type_id);
        let field_owner_edges_by_target_type_id =
            field_owner_edges_by_target_type_id(items_by_type_id);
        let target_slot_anchors = target_slot_anchors_by_type_id(
            items_by_type_id,
            &direct_derived_type_ids_by_base_type_id,
            &slot_owner_edges_by_target_type_id,
        );
        let slot_anchors = slot_anchors_by_type_id(
            items_by_type_id.values().copied(),
            &primary_base_chains_by_type_id,
            &target_slot_anchors,
        );
        let concrete_slot_candidates = concrete_slot_candidates_by_type_id(
            items_by_type_id.values().copied(),
            items_by_type_id,
            &primary_base_chains_by_type_id,
            &slot_anchors,
        );
        let (concrete_slot_bindings, ambiguous_concrete_slot_type_ids) =
            resolve_concrete_slot_bindings(&concrete_slot_candidates);
        let concrete_slot_owner_type_ids = concrete_slot_bindings
            .values()
            .map(|binding| binding.owner_type_id)
            .collect();

        Self {
            base_type_ids,
            direct_derived_type_ids_by_base_type_id,
            slot_owner_edges_by_target_type_id,
            field_owner_edges_by_target_type_id,
            slot_anchors,
            concrete_slot_candidates,
            concrete_slot_bindings,
            ambiguous_concrete_slot_type_ids,
            concrete_slot_owner_type_ids,
            slot_family_owner_type_ids,
            type_ids_by_source_name,
            type_ids_by_rust_ident,
        }
    }

    #[must_use]
    pub fn slot_anchor(&self, item: &SerializeCodegenItem) -> Option<&LayoutSlotAnchor> {
        self.slot_anchors.get(&item.source_type_id)
    }

    #[must_use]
    pub fn concrete_slot_binding(
        &self,
        item: &SerializeCodegenItem,
    ) -> Option<&LayoutConcreteSlotBinding> {
        self.concrete_slot_bindings.get(&item.source_type_id)
    }

    #[must_use]
    pub fn concrete_slot_candidates(
        &self,
        item: &SerializeCodegenItem,
    ) -> &[LayoutConcreteSlotCandidate] {
        self.concrete_slot_candidates
            .get(&item.source_type_id)
            .map(Vec::as_slice)
            .unwrap_or_default()
    }

    #[must_use]
    pub fn has_ambiguous_concrete_slot_binding(&self, item: &SerializeCodegenItem) -> bool {
        self.ambiguous_concrete_slot_type_ids
            .contains(&item.source_type_id)
    }

    #[must_use]
    pub fn has_concrete_slot_children(&self, item: &SerializeCodegenItem) -> bool {
        self.concrete_slot_owner_type_ids
            .contains(&item.source_type_id)
    }

    #[must_use]
    pub fn has_inheritance_family_children(&self, item: &SerializeCodegenItem) -> bool {
        self.direct_derived_type_ids_by_base_type_id
            .get(&item.source_type_id)
            .is_some_and(|derived_type_ids| !derived_type_ids.is_empty())
    }

    #[must_use]
    pub fn has_layout_family_descendants(&self, item: &SerializeCodegenItem) -> bool {
        self.has_inheritance_family_children(item) || self.has_concrete_slot_children(item)
    }

    #[must_use]
    pub fn concrete_slot_file_stem(&self, item: &SerializeCodegenItem) -> Option<String> {
        self.concrete_slot_binding(item)
            .map(|binding| concrete_type_scope_segment(&binding.slot_source_name))
    }

    #[must_use]
    pub fn inheritance_scope_segments(
        &self,
        item: &SerializeCodegenItem,
        items_by_type_id: &BTreeMap<Uuid, &SerializeCodegenItem>,
    ) -> Vec<String> {
        self.inheritance_scope(item, items_by_type_id).segments
    }

    #[must_use]
    pub fn inheritance_scope(
        &self,
        item: &SerializeCodegenItem,
        items_by_type_id: &BTreeMap<Uuid, &SerializeCodegenItem>,
    ) -> LayoutScopeDecision {
        if let Some(segments) = self.concrete_slot_scope_segments(item, items_by_type_id) {
            return LayoutScopeDecision {
                segments,
                reason: LayoutScopeReason::ConcreteSlotBinding,
            };
        }

        if let Some(segments) =
            self.slot_anchored_scope_segments(item, items_by_type_id, SlotScopeKind::Item)
        {
            return LayoutScopeDecision {
                segments,
                reason: LayoutScopeReason::SlotAnchor,
            };
        }

        if let Some(segments) = self.semantic_target_scope_segments(item, items_by_type_id) {
            return LayoutScopeDecision {
                segments,
                reason: LayoutScopeReason::SemanticWrapperTarget,
            };
        }

        if let Some(segments) = semantic_wrapper_scope_segments(item) {
            return LayoutScopeDecision {
                segments,
                reason: LayoutScopeReason::SemanticWrapper,
            };
        }

        if let Some(segments) =
            self.field_owned_scope_segments(item, items_by_type_id, FieldScopeKind::Item)
        {
            return LayoutScopeDecision {
                segments,
                reason: LayoutScopeReason::FieldOwner,
            };
        }

        self.unanchored_scope_decision(item, items_by_type_id, FieldScopeKind::Item)
    }

    #[must_use]
    pub fn inheritance_family_scope_segments(
        &self,
        item: &SerializeCodegenItem,
        items_by_type_id: &BTreeMap<Uuid, &SerializeCodegenItem>,
    ) -> Vec<String> {
        self.inheritance_family_scope(item, items_by_type_id)
            .segments
    }

    #[must_use]
    pub fn inheritance_family_scope(
        &self,
        item: &SerializeCodegenItem,
        items_by_type_id: &BTreeMap<Uuid, &SerializeCodegenItem>,
    ) -> LayoutScopeDecision {
        if let Some(segments) = self.concrete_slot_family_scope_segments(item, items_by_type_id) {
            return LayoutScopeDecision {
                segments,
                reason: LayoutScopeReason::ConcreteSlotBinding,
            };
        }

        if let Some(segments) =
            self.slot_anchored_scope_segments(item, items_by_type_id, SlotScopeKind::Family)
        {
            return LayoutScopeDecision {
                segments,
                reason: LayoutScopeReason::SlotAnchor,
            };
        }

        if let Some(segments) = self.semantic_target_scope_segments(item, items_by_type_id) {
            return LayoutScopeDecision {
                segments,
                reason: LayoutScopeReason::SemanticWrapperTarget,
            };
        }

        if let Some(segments) = semantic_wrapper_scope_segments(item) {
            return LayoutScopeDecision {
                segments,
                reason: LayoutScopeReason::SemanticWrapper,
            };
        }

        if let Some(segments) =
            self.field_owned_scope_segments(item, items_by_type_id, FieldScopeKind::Family)
        {
            return LayoutScopeDecision {
                segments,
                reason: LayoutScopeReason::FieldOwner,
            };
        }

        if self.base_type_ids.contains(&item.source_type_id)
            && let Some(segments) =
                self.descendant_common_family_scope_segments(item, items_by_type_id)
        {
            return LayoutScopeDecision {
                segments,
                reason: LayoutScopeReason::DescendantCommonFamily,
            };
        }

        self.unanchored_scope_decision(item, items_by_type_id, FieldScopeKind::Family)
    }

    #[must_use]
    pub fn emitted_scope_segments(
        &self,
        item: &SerializeCodegenItem,
        items_by_type_id: &BTreeMap<Uuid, &SerializeCodegenItem>,
    ) -> Vec<String> {
        self.emitted_scope(item, items_by_type_id).segments
    }

    #[must_use]
    pub fn emitted_scope(
        &self,
        item: &SerializeCodegenItem,
        items_by_type_id: &BTreeMap<Uuid, &SerializeCodegenItem>,
    ) -> LayoutScopeDecision {
        if let Some(segments) = self.concrete_slot_scope_segments(item, items_by_type_id) {
            LayoutScopeDecision {
                segments,
                reason: LayoutScopeReason::ConcreteSlotBinding,
            }
        } else if self.has_concrete_slot_children(item) {
            LayoutScopeDecision {
                segments: self.concrete_slot_owner_scope_segments(item, items_by_type_id),
                reason: LayoutScopeReason::ConcreteSlotOwner,
            }
        } else if self.base_type_ids.contains(&item.source_type_id) {
            self.inheritance_family_scope(item, items_by_type_id)
        } else {
            self.inheritance_scope(item, items_by_type_id)
        }
    }

    #[must_use]
    pub fn type_path(
        &self,
        item: &SerializeCodegenItem,
        items_by_type_id: &BTreeMap<Uuid, &SerializeCodegenItem>,
    ) -> LayoutTypePath {
        let scope_segments = self.emitted_scope_segments(item, items_by_type_id);
        let default_file_stem = self
            .concrete_slot_file_stem(item)
            .unwrap_or_else(|| concrete_type_scope_segment(&item.source_name));
        let file_stem = self
            .field_owned_collision_file_stem(item, items_by_type_id, &default_file_stem)
            .unwrap_or(default_file_stem);
        LayoutTypePath::new(scope_segments, file_stem)
    }

    fn concrete_slot_scope_segments(
        &self,
        item: &SerializeCodegenItem,
        items_by_type_id: &BTreeMap<Uuid, &SerializeCodegenItem>,
    ) -> Option<Vec<String>> {
        let binding = self.concrete_slot_binding(item)?;
        let owner = items_by_type_id.get(&binding.owner_type_id)?;
        Some(self.concrete_slot_owner_scope_segments(owner, items_by_type_id))
    }

    fn concrete_slot_family_scope_segments(
        &self,
        item: &SerializeCodegenItem,
        items_by_type_id: &BTreeMap<Uuid, &SerializeCodegenItem>,
    ) -> Option<Vec<String>> {
        let mut segments = self.concrete_slot_scope_segments(item, items_by_type_id)?;
        segments.push(self.concrete_slot_file_stem(item)?);
        segments.dedup();
        Some(segments)
    }

    fn slot_anchored_scope_segments(
        &self,
        item: &SerializeCodegenItem,
        items_by_type_id: &BTreeMap<Uuid, &SerializeCodegenItem>,
        kind: SlotScopeKind,
    ) -> Option<Vec<String>> {
        let mut visiting = BTreeSet::new();
        self.slot_anchored_scope_segments_inner(item, items_by_type_id, kind, &mut visiting)
    }

    fn slot_anchored_scope_segments_inner(
        &self,
        item: &SerializeCodegenItem,
        items_by_type_id: &BTreeMap<Uuid, &SerializeCodegenItem>,
        kind: SlotScopeKind,
        visiting: &mut BTreeSet<Uuid>,
    ) -> Option<Vec<String>> {
        if !visiting.insert(item.source_type_id) {
            return None;
        }

        let exact_slot_scope =
            self.slot_target_family_scope_segments(item, items_by_type_id, visiting);
        let result = if exact_slot_scope.is_some() {
            exact_slot_scope
        } else {
            let mut anchored = None;
            for edge in primary_base_chain_edges(item, items_by_type_id)
                .into_iter()
                .rev()
            {
                let Some(base_item) = items_by_type_id.get(&edge.type_id) else {
                    continue;
                };
                if base_item.source_name != edge.source_name {
                    continue;
                }
                if let Some(mut scope) =
                    self.slot_target_family_scope_segments(base_item, items_by_type_id, visiting)
                {
                    if kind == SlotScopeKind::Family {
                        let family_segment = self.inheritance_family_scope_segment(item, &scope);
                        scope.push(family_segment);
                        scope.dedup();
                    }
                    anchored = Some(scope);
                    break;
                }
            }
            anchored
        };

        visiting.remove(&item.source_type_id);
        result
    }

    fn slot_target_family_scope_segments(
        &self,
        target: &SerializeCodegenItem,
        items_by_type_id: &BTreeMap<Uuid, &SerializeCodegenItem>,
        visiting: &mut BTreeSet<Uuid>,
    ) -> Option<Vec<String>> {
        let owner = self.slot_target_owner(target, items_by_type_id)?;

        let mut scope = self.slot_owner_family_scope_segments(owner, items_by_type_id, visiting);
        scope.dedup();
        Some(scope)
    }

    fn slot_owner_family_scope_segments(
        &self,
        owner: &SerializeCodegenItem,
        items_by_type_id: &BTreeMap<Uuid, &SerializeCodegenItem>,
        visiting: &mut BTreeSet<Uuid>,
    ) -> Vec<String> {
        self.slot_anchored_scope_segments_inner(
            owner,
            items_by_type_id,
            SlotScopeKind::Family,
            visiting,
        )
        .unwrap_or_else(|| {
            self.inheritance_family_scope_segments_without_slot_anchor(owner, items_by_type_id)
        })
    }

    fn slot_target_owner<'a>(
        &self,
        target: &SerializeCodegenItem,
        items_by_type_id: &'a BTreeMap<Uuid, &SerializeCodegenItem>,
    ) -> Option<&'a SerializeCodegenItem> {
        self.direct_derived_type_ids_by_base_type_id
            .contains_key(&target.source_type_id)
            .then_some(())?;

        let owners = self
            .slot_owner_edges_by_target_type_id
            .get(&target.source_type_id)?;
        let [owner] = owners.as_slice() else {
            return None;
        };
        items_by_type_id.get(&owner.owner_type_id).copied()
    }

    fn descendant_common_family_scope_segments(
        &self,
        item: &SerializeCodegenItem,
        items_by_type_id: &BTreeMap<Uuid, &SerializeCodegenItem>,
    ) -> Option<Vec<String>> {
        if !source_namespace_segments(&item.source_name).is_empty() {
            return None;
        }

        let mut descendants = self.direct_derived_items(item, items_by_type_id);
        if descendants.is_empty() {
            descendants = secondary_base_descendant_items(item, items_by_type_id);
        }
        if descendants.is_empty() {
            return None;
        }

        let inherited_segments = inherited_namespace_scope_segments(item, items_by_type_id);
        let own_family_segment = self.inheritance_family_scope_segment(item, &inherited_segments);
        let descendant_scopes = descendants
            .into_iter()
            .map(|descendant| {
                self.descendant_relationship_family_scope(descendant, items_by_type_id)
            })
            .collect::<Vec<_>>();

        if let Some(mut common) = common_scope_prefix(&descendant_scopes) {
            if let Some(index) = common
                .iter()
                .position(|segment| segment == &own_family_segment)
            {
                common.truncate(index + 1);
            } else if self.should_keep_concrete_base_family_segment(item, items_by_type_id, &common)
            {
                common.push(own_family_segment);
            }
            return Some(common);
        }

        if let Some(shared) = common_scope_shared_segment(&descendant_scopes, &own_family_segment) {
            return Some(self.shared_descendant_family_scope_segments(
                item,
                shared,
                own_family_segment,
            ));
        }

        common_scope_suffix_before_segment(&descendant_scopes, &own_family_segment).map(
            |mut parent_scope| {
                parent_scope.push(own_family_segment);
                parent_scope
            },
        )
    }

    fn should_keep_concrete_base_family_segment(
        &self,
        item: &SerializeCodegenItem,
        items_by_type_id: &BTreeMap<Uuid, &SerializeCodegenItem>,
        common_scope: &[String],
    ) -> bool {
        has_concrete_layout_identity(item)
            && item_has_materialized_payload(item, items_by_type_id)
            && !common_scope.is_empty()
            && !item.role.is_az_component_like()
            && !self
                .slot_family_owner_type_ids
                .contains(&item.source_type_id)
    }

    fn shared_descendant_family_scope_segments(
        &self,
        item: &SerializeCodegenItem,
        mut shared_scope: Vec<String>,
        own_family_segment: String,
    ) -> Vec<String> {
        if !(item.role == ReflectedTypeRole::SupportType
            && item.is_abstract == Some(true)
            && shared_scope == ["components"])
        {
            shared_scope.push(own_family_segment);
            shared_scope.dedup();
        }
        shared_scope
    }

    fn descendant_relationship_family_scope(
        &self,
        item: &SerializeCodegenItem,
        items_by_type_id: &BTreeMap<Uuid, &SerializeCodegenItem>,
    ) -> Vec<String> {
        if let Some(segments) = self.concrete_slot_scope_segments(item, items_by_type_id) {
            return segments;
        }
        if let Some(segments) =
            self.slot_anchored_scope_segments(item, items_by_type_id, SlotScopeKind::Family)
        {
            return segments;
        }
        if let Some(segments) = self.semantic_target_scope_segments(item, items_by_type_id) {
            return segments;
        }
        if let Some(segments) = semantic_wrapper_scope_segments(item) {
            return segments;
        }
        if let Some(segments) =
            self.field_owned_scope_segments(item, items_by_type_id, FieldScopeKind::Family)
        {
            return segments;
        }
        if self
            .direct_derived_type_ids_by_base_type_id
            .contains_key(&item.source_type_id)
        {
            self.inheritance_family_scope_segments_without_slot_anchor(item, items_by_type_id)
        } else {
            self.inheritance_scope_segments_without_slot_anchor(item, items_by_type_id)
        }
    }

    fn direct_derived_items<'a>(
        &self,
        base: &SerializeCodegenItem,
        items_by_type_id: &'a BTreeMap<Uuid, &SerializeCodegenItem>,
    ) -> Vec<&'a SerializeCodegenItem> {
        self.direct_derived_type_ids_by_base_type_id
            .get(&base.source_type_id)
            .into_iter()
            .flatten()
            .filter_map(|type_id| items_by_type_id.get(type_id).copied())
            .collect()
    }

    fn semantic_target_scope_segments(
        &self,
        item: &SerializeCodegenItem,
        items_by_type_id: &BTreeMap<Uuid, &SerializeCodegenItem>,
    ) -> Option<Vec<String>> {
        if item.role != ReflectedTypeRole::SupportType {
            return None;
        }

        let target = parsed_semantic_wrapper_target(&item.source_name)?;
        let target_item = self.unique_semantic_target_item(&target.target, items_by_type_id)?;
        if target_item.source_type_id == item.source_type_id {
            return None;
        }

        let mut segments = self.semantic_target_anchor_segments(target_item, items_by_type_id);
        segments.push(target.family_segment(item, items_by_type_id));
        segments.dedup();
        Some(segments)
    }

    fn semantic_target_anchor_segments(
        &self,
        target: &SerializeCodegenItem,
        items_by_type_id: &BTreeMap<Uuid, &SerializeCodegenItem>,
    ) -> Vec<String> {
        let mut segments =
            if let Some(segments) = self.concrete_slot_scope_segments(target, items_by_type_id) {
                segments
            } else if self.has_concrete_slot_children(target) {
                self.concrete_slot_owner_scope_segments(target, items_by_type_id)
            } else if self.base_type_ids.contains(&target.source_type_id) {
                self.inheritance_family_scope_segments_without_slot_anchor(target, items_by_type_id)
            } else {
                self.inheritance_scope_segments_without_slot_anchor(target, items_by_type_id)
            };
        let file_stem = self
            .concrete_slot_file_stem(target)
            .unwrap_or_else(|| concrete_type_scope_segment(&target.source_name));
        segments.push(file_stem);
        segments.dedup();
        segments
    }

    fn unique_semantic_target_item<'a>(
        &self,
        target: &str,
        items_by_type_id: &'a BTreeMap<Uuid, &SerializeCodegenItem>,
    ) -> Option<&'a SerializeCodegenItem> {
        self.unique_indexed_item(target, &self.type_ids_by_source_name, items_by_type_id)
            .or_else(|| {
                let target_ident = rust_type_ident(target);
                self.unique_indexed_item(
                    &target_ident,
                    &self.type_ids_by_rust_ident,
                    items_by_type_id,
                )
            })
    }

    fn unique_indexed_item<'a>(
        &self,
        key: &str,
        index: &BTreeMap<String, Vec<Uuid>>,
        items_by_type_id: &'a BTreeMap<Uuid, &SerializeCodegenItem>,
    ) -> Option<&'a SerializeCodegenItem> {
        let [type_id] = index.get(key)?.as_slice() else {
            return None;
        };
        items_by_type_id.get(type_id).copied()
    }

    fn field_owned_scope_segments(
        &self,
        item: &SerializeCodegenItem,
        items_by_type_id: &BTreeMap<Uuid, &SerializeCodegenItem>,
        kind: FieldScopeKind,
    ) -> Option<Vec<String>> {
        let mut visiting = BTreeSet::new();
        self.field_owned_scope_segments_inner(item, items_by_type_id, kind, &mut visiting)
    }

    fn field_owned_scope_segments_inner(
        &self,
        item: &SerializeCodegenItem,
        items_by_type_id: &BTreeMap<Uuid, &SerializeCodegenItem>,
        kind: FieldScopeKind,
        visiting: &mut BTreeSet<Uuid>,
    ) -> Option<Vec<String>> {
        if !self.can_use_field_owner_scope(item) || !visiting.insert(item.source_type_id) {
            return None;
        }

        let Some(owner_edges) = self
            .field_owner_edges_by_target_type_id
            .get(&item.source_type_id)
        else {
            visiting.remove(&item.source_type_id);
            return None;
        };
        let mut owner_scopes = Vec::new();
        for owner_edge in owner_edges {
            let Some(owner) = items_by_type_id.get(&owner_edge.owner_type_id).copied() else {
                continue;
            };
            if owner.source_type_id == item.source_type_id {
                continue;
            }
            let owner_scope =
                self.field_owner_anchor_scope_segments(item, owner, items_by_type_id, visiting);
            if !owner_scope.is_empty() {
                owner_scopes.push(owner_scope);
            }
        }

        owner_scopes.sort();
        owner_scopes.dedup();
        visiting.remove(&item.source_type_id);
        let owner_scope = common_scope_prefix(&owner_scopes)?;
        Some(self.field_owned_relative_scope_segments(item, items_by_type_id, owner_scope, kind))
    }

    fn can_use_field_owner_scope(&self, item: &SerializeCodegenItem) -> bool {
        item.role == ReflectedTypeRole::SupportType
            && parsed_semantic_wrapper_target(&item.source_name).is_none()
            && semantic_wrapper_scope_segments(item).is_none()
            && source_namespace_segments(&item.source_name).is_empty()
    }

    fn field_owner_anchor_scope_segments(
        &self,
        target: &SerializeCodegenItem,
        owner: &SerializeCodegenItem,
        items_by_type_id: &BTreeMap<Uuid, &SerializeCodegenItem>,
        visiting: &mut BTreeSet<Uuid>,
    ) -> Vec<String> {
        if self.base_type_ids.contains(&owner.source_type_id) {
            return self.field_owner_scope_segments(owner, items_by_type_id, visiting);
        }
        if self.can_climb_through_support_owner(target, owner)
            && let Some(segments) =
                self.field_owner_common_anchor_scope_segments(owner, items_by_type_id, visiting)
        {
            return segments;
        }
        self.field_owner_scope_segments(owner, items_by_type_id, visiting)
    }

    fn can_climb_through_support_owner(
        &self,
        target: &SerializeCodegenItem,
        owner: &SerializeCodegenItem,
    ) -> bool {
        owner.role == ReflectedTypeRole::SupportType
            && parsed_semantic_wrapper_target(&owner.source_name).is_none()
            && semantic_wrapper_scope_segments(owner).is_none()
            && concrete_type_scope_segment(&target.source_name)
                != concrete_type_scope_segment(&owner.source_name)
    }

    fn field_owner_common_anchor_scope_segments(
        &self,
        item: &SerializeCodegenItem,
        items_by_type_id: &BTreeMap<Uuid, &SerializeCodegenItem>,
        visiting: &mut BTreeSet<Uuid>,
    ) -> Option<Vec<String>> {
        if !visiting.insert(item.source_type_id) {
            return None;
        }

        let Some(owner_edges) = self
            .field_owner_edges_by_target_type_id
            .get(&item.source_type_id)
        else {
            visiting.remove(&item.source_type_id);
            return None;
        };
        let mut owner_scopes = Vec::new();
        for owner_edge in owner_edges {
            let Some(owner) = items_by_type_id.get(&owner_edge.owner_type_id).copied() else {
                continue;
            };
            if owner.source_type_id == item.source_type_id {
                continue;
            }
            let owner_scope =
                self.field_owner_anchor_scope_segments(item, owner, items_by_type_id, visiting);
            if !owner_scope.is_empty() {
                owner_scopes.push(owner_scope);
            }
        }

        owner_scopes.sort();
        owner_scopes.dedup();
        visiting.remove(&item.source_type_id);
        common_scope_prefix(&owner_scopes)
    }

    fn field_owner_scope_segments(
        &self,
        owner: &SerializeCodegenItem,
        items_by_type_id: &BTreeMap<Uuid, &SerializeCodegenItem>,
        visiting: &mut BTreeSet<Uuid>,
    ) -> Vec<String> {
        if let Some(segments) = self.semantic_target_scope_segments(owner, items_by_type_id) {
            return segments;
        }
        if let Some(segments) = semantic_wrapper_scope_segments(owner) {
            return segments;
        }
        if let Some(segments) = self.field_owned_scope_segments_inner(
            owner,
            items_by_type_id,
            FieldScopeKind::Family,
            visiting,
        ) {
            return segments;
        }
        self.concrete_field_owner_scope_segments(owner, items_by_type_id)
    }

    fn concrete_field_owner_scope_segments(
        &self,
        owner: &SerializeCodegenItem,
        items_by_type_id: &BTreeMap<Uuid, &SerializeCodegenItem>,
    ) -> Vec<String> {
        if let Some(segments) = self.concrete_slot_scope_segments(owner, items_by_type_id) {
            return segments;
        }
        if self.has_concrete_slot_children(owner) {
            return self.concrete_slot_owner_scope_segments(owner, items_by_type_id);
        }
        if let Some(segments) =
            self.slot_anchored_scope_segments(owner, items_by_type_id, SlotScopeKind::Family)
        {
            return segments;
        }
        if self.base_type_ids.contains(&owner.source_type_id) {
            return self.inheritance_family_scope_segments(owner, items_by_type_id);
        }
        let mut scope =
            self.inheritance_scope_segments_without_slot_anchor(owner, items_by_type_id);
        scope.push(concrete_type_scope_segment(&owner.source_name));
        scope.dedup();
        scope
    }

    fn field_owned_relative_scope_segments(
        &self,
        item: &SerializeCodegenItem,
        items_by_type_id: &BTreeMap<Uuid, &SerializeCodegenItem>,
        mut owner_scope: Vec<String>,
        kind: FieldScopeKind,
    ) -> Vec<String> {
        owner_scope.extend(self.base_chain_scope_segments(item, items_by_type_id));
        if self.should_append_field_owned_family_segment(item, &owner_scope, kind) {
            let family_segment = self.inheritance_family_scope_segment(item, &owner_scope);
            owner_scope.push(family_segment);
        }
        owner_scope.dedup();
        owner_scope
    }

    fn field_owned_collision_file_stem(
        &self,
        item: &SerializeCodegenItem,
        items_by_type_id: &BTreeMap<Uuid, &SerializeCodegenItem>,
        default_file_stem: &str,
    ) -> Option<String> {
        if !self.can_use_field_owner_scope(item) {
            return None;
        }

        let owner_edges = self
            .field_owner_edges_by_target_type_id
            .get(&item.source_type_id)?;
        let mut field_stems = BTreeSet::new();
        for owner_edge in owner_edges {
            let Some(owner) = items_by_type_id.get(&owner_edge.owner_type_id).copied() else {
                continue;
            };
            if owner.source_type_id == item.source_type_id {
                continue;
            }
            if concrete_type_scope_segment(&owner.source_name) == default_file_stem {
                field_stems.insert(rust_field_ident(&owner_edge.field_name));
            }
        }

        let mut field_stems = field_stems.into_iter().collect::<Vec<_>>();
        let [field_stem] = field_stems.as_mut_slice() else {
            return None;
        };
        Some(std::mem::take(field_stem))
    }

    fn should_append_field_owned_family_segment(
        &self,
        item: &SerializeCodegenItem,
        owner_scope: &[String],
        kind: FieldScopeKind,
    ) -> bool {
        if item.role == ReflectedTypeRole::SupportType
            && item.is_abstract == Some(true)
            && owner_scope == ["components"]
        {
            return false;
        }
        kind == FieldScopeKind::Family || self.base_type_ids.contains(&item.source_type_id)
    }

    pub fn concrete_slot_owner_scope_segments(
        &self,
        owner: &SerializeCodegenItem,
        items_by_type_id: &BTreeMap<Uuid, &SerializeCodegenItem>,
    ) -> Vec<String> {
        let mut segments =
            self.inheritance_scope_segments_without_slot_anchor(owner, items_by_type_id);
        segments.push(concrete_type_scope_segment(&owner.source_name));
        segments.dedup();
        segments
    }

    fn inheritance_scope_segments_without_slot_anchor(
        &self,
        item: &SerializeCodegenItem,
        items_by_type_id: &BTreeMap<Uuid, &SerializeCodegenItem>,
    ) -> Vec<String> {
        if let Some(segments) =
            self.component_role_first_scope_segments(item, items_by_type_id, FieldScopeKind::Item)
        {
            return segments;
        }

        let mut segments = inherited_namespace_scope_segments(item, items_by_type_id);
        segments.extend(self.base_chain_scope_segments(item, items_by_type_id));
        segments.dedup();
        segments
    }

    fn inheritance_family_scope_segments_without_slot_anchor(
        &self,
        item: &SerializeCodegenItem,
        items_by_type_id: &BTreeMap<Uuid, &SerializeCodegenItem>,
    ) -> Vec<String> {
        if let Some(segments) =
            self.component_role_first_scope_segments(item, items_by_type_id, FieldScopeKind::Family)
        {
            return segments;
        }

        let mut segments = inherited_namespace_scope_segments(item, items_by_type_id);
        segments.extend(self.base_chain_scope_segments(item, items_by_type_id));
        let family_segment = self.inheritance_family_scope_segment(item, &segments);
        segments.push(family_segment);
        segments.dedup();
        segments
    }

    fn component_role_first_scope_segments(
        &self,
        item: &SerializeCodegenItem,
        items_by_type_id: &BTreeMap<Uuid, &SerializeCodegenItem>,
        kind: FieldScopeKind,
    ) -> Option<Vec<String>> {
        if !item.role.is_az_component_like() {
            return None;
        }

        let mut base_segments = self.component_base_chain_scope_segments(item, items_by_type_id);
        let has_component_family_root = base_segments
            .first()
            .is_some_and(|segment| segment == "components");
        if !has_component_family_root {
            base_segments.insert(0, "components".to_owned());
        }
        let mut segments = inherited_namespace_scope_segments(item, items_by_type_id);
        segments.extend(base_segments);
        if kind == FieldScopeKind::Family || segments.is_empty() {
            let family_segment = self.inheritance_family_scope_segment(item, &segments);
            segments.push(family_segment);
        }
        segments.dedup();
        Some(segments)
    }

    fn component_base_chain_scope_segments(
        &self,
        item: &SerializeCodegenItem,
        items_by_type_id: &BTreeMap<Uuid, &SerializeCodegenItem>,
    ) -> Vec<String> {
        let mut segments = Vec::new();
        for edge in primary_base_chain_edges(item, items_by_type_id) {
            let Some(base_item) = items_by_type_id.get(&edge.type_id).filter(|base_item| {
                edge.matches_reflected_type
                    && base_item.source_name == edge.source_name
                    && base_item.role.is_az_component_like()
            }) else {
                continue;
            };
            let segment = self.inheritance_family_scope_segment(base_item, &segments);
            segments.push(segment);
            segments.dedup();
        }
        segments
    }

    fn base_chain_scope_segments(
        &self,
        item: &SerializeCodegenItem,
        items_by_type_id: &BTreeMap<Uuid, &SerializeCodegenItem>,
    ) -> Vec<String> {
        let mut segments = Vec::new();
        for edge in primary_base_chain_edges(item, items_by_type_id) {
            let segment = items_by_type_id
                .get(&edge.type_id)
                .filter(|base_item| {
                    edge.matches_reflected_type && base_item.source_name == edge.source_name
                })
                .map_or_else(
                    || inheritance_scope_segment(&edge.source_name),
                    |base_item| self.inheritance_family_scope_segment(base_item, &segments),
                );
            segments.push(segment);
            segments.dedup();
        }
        segments
    }

    fn inheritance_family_scope_segment(
        &self,
        item: &SerializeCodegenItem,
        inherited_segments: &[String],
    ) -> String {
        let family_segment = inheritance_scope_segment(&item.source_name);
        if item.role.is_az_component_like() {
            return family_segment;
        }
        if has_concrete_layout_identity(item)
            && !self
                .slot_family_owner_type_ids
                .contains(&item.source_type_id)
            && inherited_segments.last() != Some(&family_segment)
        {
            concrete_type_scope_segment(&item.source_name)
        } else {
            family_segment
        }
    }

    fn unanchored_scope_decision(
        &self,
        item: &SerializeCodegenItem,
        items_by_type_id: &BTreeMap<Uuid, &SerializeCodegenItem>,
        kind: FieldScopeKind,
    ) -> LayoutScopeDecision {
        let segments = match kind {
            FieldScopeKind::Item => {
                self.inheritance_scope_segments_without_slot_anchor(item, items_by_type_id)
            }
            FieldScopeKind::Family => {
                self.inheritance_family_scope_segments_without_slot_anchor(item, items_by_type_id)
            }
        };
        LayoutScopeDecision {
            segments,
            reason: unanchored_scope_reason(item, items_by_type_id, kind),
        }
    }
}

fn has_concrete_layout_identity(item: &SerializeCodegenItem) -> bool {
    item.is_abstract == Some(false) || (item.is_abstract.is_none() && item.factory.is_some())
}

fn secondary_base_descendant_items<'a>(
    base: &SerializeCodegenItem,
    items_by_type_id: &'a BTreeMap<Uuid, &SerializeCodegenItem>,
) -> Vec<&'a SerializeCodegenItem> {
    items_by_type_id
        .values()
        .copied()
        .filter(|item| item.source_type_id != base.source_type_id)
        .filter(|item| has_reflected_base_edge(item, base))
        .collect()
}

fn has_reflected_base_edge(item: &SerializeCodegenItem, base: &SerializeCodegenItem) -> bool {
    item.fields.iter().any(|field| {
        if !field.is_base_class {
            return false;
        }
        matches!(
            &field.resolved_type,
            ResolvedType::Named {
                type_id,
                source_name,
            } if *type_id == base.source_type_id && source_name == &base.source_name
        )
    }) || item
        .rtti_base_chain
        .iter()
        .any(|edge| edge.type_id == base.source_type_id && edge.source_name == base.source_name)
}

#[must_use]
pub fn inheritance_scope_segments(
    item: &SerializeCodegenItem,
    items_by_type_id: &BTreeMap<Uuid, &SerializeCodegenItem>,
) -> Vec<String> {
    LayoutIndex::from_items_by_type_id(items_by_type_id)
        .inheritance_scope_segments(item, items_by_type_id)
}

#[must_use]
pub fn inheritance_family_scope_segments(
    item: &SerializeCodegenItem,
    items_by_type_id: &BTreeMap<Uuid, &SerializeCodegenItem>,
) -> Vec<String> {
    LayoutIndex::from_items_by_type_id(items_by_type_id)
        .inheritance_family_scope_segments(item, items_by_type_id)
}

fn unanchored_scope_reason(
    item: &SerializeCodegenItem,
    items_by_type_id: &BTreeMap<Uuid, &SerializeCodegenItem>,
    kind: FieldScopeKind,
) -> LayoutScopeReason {
    if !source_scope_segments(&item.source_name).is_empty() {
        return LayoutScopeReason::SourceNamespace;
    }
    if !inherited_namespace_scope_segments(item, items_by_type_id).is_empty() {
        return LayoutScopeReason::InheritedNamespace;
    }
    if kind == FieldScopeKind::Family || !primary_base_chain(item, items_by_type_id).is_empty() {
        LayoutScopeReason::InheritanceFamily
    } else {
        LayoutScopeReason::Root
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SlotScopeKind {
    Item,
    Family,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum FieldScopeKind {
    Item,
    Family,
}

#[must_use]
pub fn concrete_slot_binding(
    item: &SerializeCodegenItem,
    items_by_type_id: &BTreeMap<Uuid, &SerializeCodegenItem>,
) -> Option<LayoutConcreteSlotBinding> {
    LayoutIndex::from_items_by_type_id(items_by_type_id)
        .concrete_slot_binding(item)
        .cloned()
}

#[must_use]
pub fn has_concrete_slot_children(
    owner: &SerializeCodegenItem,
    items_by_type_id: &BTreeMap<Uuid, &SerializeCodegenItem>,
) -> bool {
    LayoutIndex::from_items_by_type_id(items_by_type_id).has_concrete_slot_children(owner)
}

#[must_use]
pub fn concrete_slot_owner_scope_segments(
    owner: &SerializeCodegenItem,
    items_by_type_id: &BTreeMap<Uuid, &SerializeCodegenItem>,
) -> Vec<String> {
    LayoutIndex::from_items_by_type_id(items_by_type_id)
        .concrete_slot_owner_scope_segments(owner, items_by_type_id)
}

#[must_use]
pub fn concrete_slot_file_stem(
    item: &SerializeCodegenItem,
    items_by_type_id: &BTreeMap<Uuid, &SerializeCodegenItem>,
) -> Option<String> {
    LayoutIndex::from_items_by_type_id(items_by_type_id).concrete_slot_file_stem(item)
}

fn target_slot_anchors_by_type_id(
    items_by_type_id: &BTreeMap<Uuid, &SerializeCodegenItem>,
    direct_derived_type_ids_by_base_type_id: &BTreeMap<Uuid, Vec<Uuid>>,
    slot_owner_edges_by_target_type_id: &BTreeMap<Uuid, Vec<LayoutSlotOwnerEdge>>,
) -> BTreeMap<Uuid, LayoutSlotAnchor> {
    let mut anchors = BTreeMap::new();
    for (slot_type_id, owner_edges) in slot_owner_edges_by_target_type_id {
        if !direct_derived_type_ids_by_base_type_id.contains_key(slot_type_id) {
            continue;
        }
        let [owner_edge] = owner_edges.as_slice() else {
            continue;
        };
        let Some(owner) = items_by_type_id.get(&owner_edge.owner_type_id) else {
            continue;
        };
        let Some(slot) = items_by_type_id.get(slot_type_id) else {
            continue;
        };
        anchors.insert(
            *slot_type_id,
            LayoutSlotAnchor {
                owner_type_id: owner.source_type_id,
                owner_source_name: owner.source_name.clone(),
                owner_field_name: owner_edge.field_name.clone(),
                slot_type_id: slot.source_type_id,
                slot_source_name: slot.source_name.clone(),
            },
        );
    }

    anchors
}

fn type_ids_by_source_name(
    items_by_type_id: &BTreeMap<Uuid, &SerializeCodegenItem>,
) -> BTreeMap<String, Vec<Uuid>> {
    type_ids_by_key(items_by_type_id, |item| item.source_name.clone())
}

fn type_ids_by_rust_ident(
    items_by_type_id: &BTreeMap<Uuid, &SerializeCodegenItem>,
) -> BTreeMap<String, Vec<Uuid>> {
    type_ids_by_key(items_by_type_id, |item| rust_type_ident(&item.source_name))
}

fn type_ids_by_key(
    items_by_type_id: &BTreeMap<Uuid, &SerializeCodegenItem>,
    key: impl Fn(&SerializeCodegenItem) -> String,
) -> BTreeMap<String, Vec<Uuid>> {
    let mut index = BTreeMap::<String, Vec<Uuid>>::new();
    for item in items_by_type_id.values().copied() {
        index
            .entry(key(item))
            .or_default()
            .push(item.source_type_id);
    }
    for type_ids in index.values_mut() {
        type_ids.sort_unstable();
        type_ids.dedup();
    }
    index
}

fn direct_derived_type_ids_by_base_type_id(
    items_by_type_id: &BTreeMap<Uuid, &SerializeCodegenItem>,
) -> BTreeMap<Uuid, Vec<Uuid>> {
    let mut derived_by_base = BTreeMap::<Uuid, Vec<Uuid>>::new();
    for item in items_by_type_id.values().copied() {
        if let Some(base_type_id) = resolved_direct_base_type_id(item, items_by_type_id) {
            derived_by_base
                .entry(base_type_id)
                .or_default()
                .push(item.source_type_id);
        }
    }
    for derived_type_ids in derived_by_base.values_mut() {
        derived_type_ids.sort_by(|left, right| {
            let left_item = items_by_type_id
                .get(left)
                .expect("derived type id came from items_by_type_id");
            let right_item = items_by_type_id
                .get(right)
                .expect("derived type id came from items_by_type_id");
            left_item
                .source_name
                .cmp(&right_item.source_name)
                .then_with(|| left.cmp(right))
        });
    }
    derived_by_base
}

fn reflected_base_type_ids_from_items(
    items_by_type_id: &BTreeMap<Uuid, &SerializeCodegenItem>,
) -> BTreeSet<Uuid> {
    let mut base_type_ids = BTreeSet::new();
    for item in items_by_type_id.values().copied() {
        for field in &item.fields {
            if !field.is_base_class {
                continue;
            }
            let ResolvedType::Named {
                type_id,
                source_name,
            } = &field.resolved_type
            else {
                continue;
            };
            if items_by_type_id
                .get(type_id)
                .is_some_and(|base_item| base_item.source_name == *source_name)
            {
                base_type_ids.insert(*type_id);
            }
        }
        for base in &item.rtti_base_chain {
            if items_by_type_id
                .get(&base.type_id)
                .is_some_and(|base_item| base_item.source_name == base.source_name)
            {
                base_type_ids.insert(base.type_id);
            }
        }
    }
    base_type_ids
}

fn slot_owner_edges_by_target_type_id(
    items_by_type_id: &BTreeMap<Uuid, &SerializeCodegenItem>,
) -> BTreeMap<Uuid, Vec<LayoutSlotOwnerEdge>> {
    let mut edges_by_target = BTreeMap::<Uuid, Vec<LayoutSlotOwnerEdge>>::new();

    for owner in items_by_type_id.values().copied() {
        for field in &owner.fields {
            let Some(target_type_id) =
                resolved_pointer_field_target_type_id(field, items_by_type_id)
            else {
                continue;
            };
            edges_by_target
                .entry(target_type_id)
                .or_default()
                .push(LayoutSlotOwnerEdge {
                    owner_type_id: owner.source_type_id,
                    owner_source_name: owner.source_name.clone(),
                    field_name: field.source_name.clone(),
                });
        }
    }

    for edges in edges_by_target.values_mut() {
        edges.sort_by(|left, right| {
            left.owner_source_name
                .cmp(&right.owner_source_name)
                .then_with(|| left.owner_type_id.cmp(&right.owner_type_id))
                .then_with(|| left.field_name.cmp(&right.field_name))
        });
    }

    edges_by_target
}

fn slot_family_owner_type_ids(
    slot_owner_edges_by_target_type_id: &BTreeMap<Uuid, Vec<LayoutSlotOwnerEdge>>,
) -> BTreeSet<Uuid> {
    slot_owner_edges_by_target_type_id
        .values()
        .flatten()
        .map(|edge| edge.owner_type_id)
        .collect()
}

fn field_owner_edges_by_target_type_id(
    items_by_type_id: &BTreeMap<Uuid, &SerializeCodegenItem>,
) -> BTreeMap<Uuid, Vec<LayoutFieldOwnerEdge>> {
    let mut edges_by_target = BTreeMap::<Uuid, Vec<LayoutFieldOwnerEdge>>::new();

    for owner in items_by_type_id.values().copied() {
        for field in &owner.fields {
            if field.is_base_class {
                continue;
            }

            let mut target_type_ids = BTreeSet::new();
            collect_resolved_named_type_ids(&field.resolved_type, &mut target_type_ids);
            for target_type_id in target_type_ids {
                if target_type_id == owner.source_type_id
                    || !items_by_type_id.contains_key(&target_type_id)
                {
                    continue;
                }

                edges_by_target
                    .entry(target_type_id)
                    .or_default()
                    .push(LayoutFieldOwnerEdge {
                        owner_type_id: owner.source_type_id,
                        owner_source_name: owner.source_name.clone(),
                        field_name: field.source_name.clone(),
                    });
            }
        }
    }

    for edges in edges_by_target.values_mut() {
        edges.sort_by(|left, right| {
            left.owner_source_name
                .cmp(&right.owner_source_name)
                .then_with(|| left.owner_type_id.cmp(&right.owner_type_id))
                .then_with(|| left.field_name.cmp(&right.field_name))
        });
        edges.dedup();
    }

    edges_by_target
}

fn resolved_direct_base_type_id(
    item: &SerializeCodegenItem,
    items_by_type_id: &BTreeMap<Uuid, &SerializeCodegenItem>,
) -> Option<Uuid> {
    resolved_direct_serialized_base_type_id(item, items_by_type_id)
        .or_else(|| resolved_direct_rtti_base_type_id(item, items_by_type_id))
}

fn resolved_direct_serialized_base_type_id(
    item: &SerializeCodegenItem,
    items_by_type_id: &BTreeMap<Uuid, &SerializeCodegenItem>,
) -> Option<Uuid> {
    item.fields
        .iter()
        .filter(|field| field.is_base_class)
        .find_map(|field| {
            let ResolvedType::Named {
                type_id,
                source_name,
            } = &field.resolved_type
            else {
                return None;
            };
            items_by_type_id
                .get(type_id)
                .filter(|base_item| base_item.source_name == *source_name)
                .map(|_| *type_id)
        })
}

fn resolved_direct_rtti_base_type_id(
    item: &SerializeCodegenItem,
    items_by_type_id: &BTreeMap<Uuid, &SerializeCodegenItem>,
) -> Option<Uuid> {
    let base = item.rtti_base_chain.last()?;
    items_by_type_id
        .get(&base.type_id)
        .filter(|base_item| base_item.source_name == base.source_name)
        .map(|_| base.type_id)
}

fn resolved_pointer_field_target_type_id(
    field: &SerializeCodegenField,
    items_by_type_id: &BTreeMap<Uuid, &SerializeCodegenItem>,
) -> Option<Uuid> {
    if field.is_base_class || !field.is_pointer {
        return None;
    }
    let ResolvedType::Named {
        type_id,
        source_name,
    } = &field.resolved_type
    else {
        return None;
    };
    items_by_type_id
        .get(type_id)
        .filter(|target| target.source_name == *source_name)
        .map(|_| *type_id)
}

fn slot_anchors_by_type_id<'a>(
    items: impl IntoIterator<Item = &'a SerializeCodegenItem>,
    primary_base_chains_by_type_id: &BTreeMap<Uuid, Vec<LayoutBaseEdge>>,
    target_slot_anchors: &BTreeMap<Uuid, LayoutSlotAnchor>,
) -> BTreeMap<Uuid, LayoutSlotAnchor> {
    items
        .into_iter()
        .filter_map(|item| {
            resolved_slot_anchor_from_maps(
                item,
                primary_base_chains_by_type_id,
                target_slot_anchors,
            )
            .map(|anchor| (item.source_type_id, anchor))
        })
        .collect()
}

fn resolved_slot_anchor_from_maps(
    item: &SerializeCodegenItem,
    primary_base_chains_by_type_id: &BTreeMap<Uuid, Vec<LayoutBaseEdge>>,
    target_slot_anchors: &BTreeMap<Uuid, LayoutSlotAnchor>,
) -> Option<LayoutSlotAnchor> {
    if let Some(anchor) = target_slot_anchors.get(&item.source_type_id) {
        return Some(anchor.clone());
    }

    primary_base_chains_by_type_id
        .get(&item.source_type_id)?
        .iter()
        .rev()
        .find_map(|edge| {
            target_slot_anchors
                .get(&edge.type_id)
                .filter(|anchor| anchor.slot_source_name == edge.source_name)
                .cloned()
        })
}

fn concrete_slot_candidates_by_type_id<'a>(
    items: impl IntoIterator<Item = &'a SerializeCodegenItem>,
    items_by_type_id: &BTreeMap<Uuid, &SerializeCodegenItem>,
    primary_base_chains_by_type_id: &BTreeMap<Uuid, Vec<LayoutBaseEdge>>,
    slot_anchors: &BTreeMap<Uuid, LayoutSlotAnchor>,
) -> BTreeMap<Uuid, Vec<LayoutConcreteSlotCandidate>> {
    items
        .into_iter()
        .filter(|item| !item.is_reflection_marker)
        .filter_map(|item| {
            let anchor = slot_anchors.get(&item.source_type_id)?;
            if item.source_type_id == anchor.slot_type_id {
                return None;
            }
            let slot = items_by_type_id.get(&anchor.slot_type_id)?;
            let slot_owner = items_by_type_id.get(&anchor.owner_type_id)?;
            let implementation_key = concrete_slot_implementation_key(item, slot)?;
            let candidates = concrete_slot_owner_candidates_with_chains(
                slot_owner,
                items_by_type_id,
                primary_base_chains_by_type_id,
                &implementation_key,
            )
            .into_iter()
            .map(|(match_kind, owner)| LayoutConcreteSlotCandidate {
                owner_type_id: owner.source_type_id,
                owner_source_name: owner.source_name.clone(),
                slot_owner_type_id: anchor.owner_type_id,
                slot_owner_source_name: anchor.owner_source_name.clone(),
                owner_field_name: anchor.owner_field_name.clone(),
                slot_type_id: anchor.slot_type_id,
                slot_source_name: anchor.slot_source_name.clone(),
                match_kind,
            })
            .collect::<Vec<_>>();

            (!candidates.is_empty()).then_some((item.source_type_id, candidates))
        })
        .collect()
}

fn resolve_concrete_slot_bindings(
    candidates_by_type_id: &BTreeMap<Uuid, Vec<LayoutConcreteSlotCandidate>>,
) -> (BTreeMap<Uuid, LayoutConcreteSlotBinding>, BTreeSet<Uuid>) {
    let mut ambiguous_type_ids = BTreeSet::new();
    let mut tentative = BTreeMap::<Uuid, LayoutConcreteSlotCandidate>::new();

    for (implementation_type_id, candidates) in candidates_by_type_id {
        let Some(best_kind) = candidates.first().map(|candidate| candidate.match_kind) else {
            continue;
        };
        let best_candidates = candidates
            .iter()
            .filter(|candidate| candidate.match_kind == best_kind)
            .collect::<Vec<_>>();
        let [candidate] = best_candidates.as_slice() else {
            ambiguous_type_ids.insert(*implementation_type_id);
            continue;
        };
        tentative.insert(*implementation_type_id, (*candidate).clone());
    }

    let mut implementations_by_owner_slot = BTreeMap::<(Uuid, Uuid), Vec<Uuid>>::new();
    for (implementation_type_id, candidate) in &tentative {
        implementations_by_owner_slot
            .entry((candidate.owner_type_id, candidate.slot_type_id))
            .or_default()
            .push(*implementation_type_id);
    }

    for implementation_type_ids in implementations_by_owner_slot.values() {
        if implementation_type_ids.len() > 1 {
            ambiguous_type_ids.extend(implementation_type_ids.iter().copied());
        }
    }

    let bindings = tentative
        .into_iter()
        .filter(|(implementation_type_id, _)| !ambiguous_type_ids.contains(implementation_type_id))
        .map(|(implementation_type_id, candidate)| {
            (
                implementation_type_id,
                LayoutConcreteSlotBinding {
                    owner_type_id: candidate.owner_type_id,
                    owner_source_name: candidate.owner_source_name.clone(),
                    slot_owner_type_id: candidate.slot_owner_type_id,
                    slot_owner_source_name: candidate.slot_owner_source_name.clone(),
                    owner_field_name: candidate.owner_field_name.clone(),
                    slot_type_id: candidate.slot_type_id,
                    slot_source_name: candidate.slot_source_name.clone(),
                },
            )
        })
        .collect();

    (bindings, ambiguous_type_ids)
}

fn concrete_slot_implementation_key(
    item: &SerializeCodegenItem,
    slot: &SerializeCodegenItem,
) -> Option<String> {
    let implementation_name = rust_type_ident(&item.source_name);
    let slot_name = rust_type_ident(&slot.source_name);
    implementation_name
        .strip_suffix(&slot_name)
        .filter(|owner_key| !owner_key.is_empty())
        .map(ToOwned::to_owned)
}

fn concrete_slot_owner_candidates_with_chains<'a>(
    slot_owner: &'a SerializeCodegenItem,
    items_by_type_id: &'a BTreeMap<Uuid, &SerializeCodegenItem>,
    primary_base_chains_by_type_id: &BTreeMap<Uuid, Vec<LayoutBaseEdge>>,
    implementation_key: &str,
) -> Vec<(LayoutConcreteSlotMatchKind, &'a SerializeCodegenItem)> {
    let slot_owner_name = rust_type_ident(&slot_owner.source_name);
    let slot_owner_suffix = trailing_semantic_word(&slot_owner_name);
    let mut matches = items_by_type_id
        .values()
        .copied()
        .filter(|candidate| candidate.source_type_id != slot_owner.source_type_id)
        .filter(|candidate| candidate.is_abstract != Some(true))
        .filter(|candidate| {
            inherits_from_chains(
                candidate,
                slot_owner.source_type_id,
                &slot_owner.source_name,
                primary_base_chains_by_type_id,
            )
        })
        .filter_map(|candidate| {
            concrete_slot_owner_match_rank(candidate, implementation_key, slot_owner_suffix)
                .map(|rank| (rank, candidate))
        })
        .collect::<Vec<_>>();

    matches.sort_by(|(left_rank, left), (right_rank, right)| {
        left_rank
            .cmp(right_rank)
            .then_with(|| left.source_name.cmp(&right.source_name))
            .then_with(|| left.source_type_id.cmp(&right.source_type_id))
    });
    matches
}

fn concrete_slot_owner_match_rank(
    owner: &SerializeCodegenItem,
    implementation_key: &str,
    slot_owner_suffix: Option<&str>,
) -> Option<LayoutConcreteSlotMatchKind> {
    let owner_name = rust_type_ident(&owner.source_name);
    if implementation_key == owner_name {
        return Some(LayoutConcreteSlotMatchKind::ExactOwnerName);
    }
    let suffix = slot_owner_suffix?;
    owner_name
        .strip_suffix(suffix)
        .filter(|owner_key| !owner_key.is_empty())
        .filter(|owner_key| *owner_key == implementation_key)
        .map(|_| LayoutConcreteSlotMatchKind::OwnerTrailingSemanticWord)
}

fn trailing_semantic_word(name: &str) -> Option<&str> {
    let mut last_uppercase = None;
    for (index, ch) in name.char_indices() {
        if ch.is_ascii_uppercase() {
            last_uppercase = Some(index);
        }
    }
    let index = last_uppercase?;
    (index > 0).then_some(&name[index..])
}

fn inherits_from_chains(
    item: &SerializeCodegenItem,
    base_type_id: Uuid,
    base_source_name: &str,
    primary_base_chains_by_type_id: &BTreeMap<Uuid, Vec<LayoutBaseEdge>>,
) -> bool {
    primary_base_chains_by_type_id
        .get(&item.source_type_id)
        .is_some_and(|chain| {
            chain.iter().any(|edge| {
                edge.matches_reflected_type
                    && edge.type_id == base_type_id
                    && edge.source_name == base_source_name
            })
        })
}

#[must_use]
pub fn emitted_scope_segments(
    item: &SerializeCodegenItem,
    items_by_type_id: &BTreeMap<Uuid, &SerializeCodegenItem>,
    base_type_ids: &BTreeSet<Uuid>,
) -> Vec<String> {
    let _ = base_type_ids;
    LayoutIndex::from_items_by_type_id(items_by_type_id)
        .emitted_scope_segments(item, items_by_type_id)
}

#[cfg(test)]
mod tests;
