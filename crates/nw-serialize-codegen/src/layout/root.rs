use std::collections::{BTreeMap, BTreeSet};

use uuid::Uuid;

use crate::role::ReflectedTypeRole;

use super::path::concrete_type_scope_segment;
use super::{LayoutAnalysisItem, LayoutAnalysisReport, LayoutFieldOwnerEdge, LayoutScopeReason};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LayoutRootReport {
    pub roots: Vec<LayoutRootItem>,
}

impl LayoutRootReport {
    #[must_use]
    pub fn from_analysis_report(report: &LayoutAnalysisReport) -> Self {
        let mut roots = BTreeMap::<Option<String>, LayoutRootAccumulator>::new();
        for item in &report.items {
            let root_segment = item.emitted_scope_segments.first().cloned();
            let root = roots.entry(root_segment).or_default();
            root.item_count += 1;
            root.reasons.insert(item.emitted_scope_reason);
            if root.sample_source_names.len() < 8 {
                root.sample_source_names.push(item.source_name.clone());
            }
        }

        let roots = roots
            .into_iter()
            .map(|(root_segment, root)| LayoutRootItem {
                root_segment,
                item_count: root.item_count,
                reasons: root.reasons.into_iter().collect(),
                sample_source_names: root.sample_source_names,
            })
            .collect();
        Self { roots }
    }

    #[must_use]
    pub fn root_by_segment(&self, segment: &str) -> Option<&LayoutRootItem> {
        self.roots
            .iter()
            .find(|root| root.root_segment.as_deref() == Some(segment))
    }

    #[must_use]
    pub fn has_root_segment(&self, segment: &str) -> bool {
        self.root_by_segment(segment).is_some()
    }

    #[must_use]
    pub fn to_text(&self) -> String {
        let mut out = String::new();
        for root in &self.roots {
            out.push_str("root ");
            out.push_str(root.root_segment.as_deref().unwrap_or("<types>"));
            out.push_str(" items=");
            out.push_str(&root.item_count.to_string());
            out.push_str(" reasons=");
            out.push_str(
                &root
                    .reasons
                    .iter()
                    .map(|reason| reason.as_str())
                    .collect::<Vec<_>>()
                    .join(","),
            );
            if !root.sample_source_names.is_empty() {
                out.push_str(" sample=");
                out.push_str(&root.sample_source_names.join(", "));
            }
            out.push('\n');
        }
        out
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LayoutRootItem {
    pub root_segment: Option<String>,
    pub item_count: usize,
    pub reasons: Vec<LayoutScopeReason>,
    pub sample_source_names: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LayoutRootAudit {
    pub findings: Vec<LayoutRootFinding>,
}

impl LayoutRootAudit {
    #[must_use]
    pub fn from_analysis_report(report: &LayoutAnalysisReport) -> Self {
        let items_by_type_id = report
            .items
            .iter()
            .map(|item| (item.source_type_id, item))
            .collect::<BTreeMap<_, _>>();
        let mut findings = Vec::new();
        for item in &report.items {
            if let Some(finding) = unique_owned_support_root_finding(item, &items_by_type_id) {
                findings.push(finding);
            }
            if let Some(finding) = ambiguous_shared_support_root_finding(item) {
                findings.push(finding);
            }
        }
        Self { findings }
    }

    #[must_use]
    pub fn is_clean(&self) -> bool {
        self.findings.is_empty()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LayoutRootFinding {
    pub kind: LayoutRootFindingKind,
    pub source_type_id: Uuid,
    pub source_name: String,
    pub root_segment: String,
    pub owner_edges: Vec<LayoutFieldOwnerEdge>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LayoutRootFindingKind {
    AmbiguousSharedSupportRoot,
    UniqueOwnedSupportRoot,
}

fn unique_owned_support_root_finding(
    item: &LayoutAnalysisItem,
    items_by_type_id: &BTreeMap<Uuid, &LayoutAnalysisItem>,
) -> Option<LayoutRootFinding> {
    if !is_namespace_less_support_root(item) {
        return None;
    }
    let [owner_edge] = item.field_owner_edges.as_slice() else {
        return None;
    };
    let owner = items_by_type_id.get(&owner_edge.owner_type_id)?;
    if owner.emitted_scope_segments.is_empty() {
        return None;
    }
    Some(LayoutRootFinding {
        kind: LayoutRootFindingKind::UniqueOwnedSupportRoot,
        source_type_id: item.source_type_id,
        source_name: item.source_name.clone(),
        root_segment: root_segment(item),
        owner_edges: vec![owner_edge.clone()],
    })
}

fn ambiguous_shared_support_root_finding(item: &LayoutAnalysisItem) -> Option<LayoutRootFinding> {
    if !is_namespace_less_support_root(item) || item.field_owner_edges.len() < 2 {
        return None;
    }

    Some(LayoutRootFinding {
        kind: LayoutRootFindingKind::AmbiguousSharedSupportRoot,
        source_type_id: item.source_type_id,
        source_name: item.source_name.clone(),
        root_segment: root_segment(item),
        owner_edges: item.field_owner_edges.clone(),
    })
}

fn is_namespace_less_support_root(item: &LayoutAnalysisItem) -> bool {
    item.role == ReflectedTypeRole::SupportType
        && item.namespace_segments.is_empty()
        && item.emitted_scope_reason == LayoutScopeReason::Root
        && item.emitted_scope_segments.len() <= 1
}

fn root_segment(item: &LayoutAnalysisItem) -> String {
    item.emitted_scope_segments
        .first()
        .cloned()
        .unwrap_or_else(|| concrete_type_scope_segment(&item.source_name))
}

#[derive(Debug, Default)]
struct LayoutRootAccumulator {
    item_count: usize,
    reasons: BTreeSet<LayoutScopeReason>,
    sample_source_names: Vec<String>,
}

#[cfg(test)]
mod tests {
    use uuid::uuid;

    use crate::layout::{
        LayoutAnalysisItem, LayoutBaseEdge, LayoutConcreteSlotCandidate, LayoutFieldOwnerEdge,
        LayoutRootAudit, LayoutRootFindingKind, LayoutScopeReason, LayoutSerializedShape,
    };
    use crate::role::ReflectedTypeRole;

    use super::LayoutAnalysisReport;

    #[test]
    fn audit_flags_unique_owned_namespace_less_support_root() {
        let owner_type_id = uuid!("aaaaaaaa-aaaa-aaaa-aaaa-aaaaaaaaaaaa");
        let support_type_id = uuid!("bbbbbbbb-bbbb-bbbb-bbbb-bbbbbbbbbbbb");
        let report = LayoutAnalysisReport {
            items: vec![
                analysis_item(
                    owner_type_id,
                    "MeshComponent",
                    ReflectedTypeRole::AzComponent,
                    vec!["components", "mesh_component"],
                    LayoutScopeReason::InheritanceFamily,
                    Vec::new(),
                ),
                analysis_item(
                    support_type_id,
                    "MeshRenderOptions",
                    ReflectedTypeRole::SupportType,
                    vec!["mesh_render_options"],
                    LayoutScopeReason::Root,
                    vec![LayoutFieldOwnerEdge {
                        owner_type_id,
                        owner_source_name: "MeshComponent".to_owned(),
                        field_name: "m_options".to_owned(),
                    }],
                ),
            ],
        };

        let audit = LayoutRootAudit::from_analysis_report(&report);

        assert_eq!(audit.findings.len(), 1);
        assert_eq!(
            audit.findings[0].kind,
            LayoutRootFindingKind::UniqueOwnedSupportRoot
        );
        assert_eq!(audit.findings[0].source_name, "MeshRenderOptions");
        assert_eq!(
            audit.findings[0].owner_edges[0].owner_source_name,
            "MeshComponent"
        );
    }

    #[test]
    fn audit_flags_shared_namespace_less_support_roots() {
        let first_owner_type_id = uuid!("aaaaaaaa-aaaa-aaaa-aaaa-aaaaaaaaaaaa");
        let second_owner_type_id = uuid!("bbbbbbbb-bbbb-bbbb-bbbb-bbbbbbbbbbbb");
        let support_type_id = uuid!("cccccccc-cccc-cccc-cccc-cccccccccccc");
        let report = LayoutAnalysisReport {
            items: vec![
                analysis_item(
                    first_owner_type_id,
                    "GatherableControllerComponentServerFacet",
                    ReflectedTypeRole::ServerFacet,
                    vec![
                        "components",
                        "faceted_components",
                        "gatherable_controller_component",
                    ],
                    LayoutScopeReason::ConcreteSlotBinding,
                    Vec::new(),
                ),
                analysis_item(
                    second_owner_type_id,
                    "TerritoryInterfaceComponentServerFacet",
                    ReflectedTypeRole::ServerFacet,
                    vec![
                        "components",
                        "faceted_components",
                        "territory_interface_component",
                    ],
                    LayoutScopeReason::ConcreteSlotBinding,
                    Vec::new(),
                ),
                analysis_item(
                    support_type_id,
                    "RemoteTypelessServerFacetRef",
                    ReflectedTypeRole::SupportType,
                    vec!["remote_typeless_server_facet_ref"],
                    LayoutScopeReason::Root,
                    vec![
                        LayoutFieldOwnerEdge {
                            owner_type_id: first_owner_type_id,
                            owner_source_name: "GatherableControllerComponentServerFacet"
                                .to_owned(),
                            field_name: "m_obstructions".to_owned(),
                        },
                        LayoutFieldOwnerEdge {
                            owner_type_id: second_owner_type_id,
                            owner_source_name: "TerritoryInterfaceComponentServerFacet".to_owned(),
                            field_name: "m_territoryGovernanceRef".to_owned(),
                        },
                    ],
                ),
            ],
        };

        let audit = LayoutRootAudit::from_analysis_report(&report);

        assert_eq!(audit.findings.len(), 1);
        assert_eq!(
            audit.findings[0].kind,
            LayoutRootFindingKind::AmbiguousSharedSupportRoot
        );
        assert_eq!(
            audit.findings[0].source_name,
            "RemoteTypelessServerFacetRef"
        );
        assert_eq!(audit.findings[0].owner_edges.len(), 2);
    }

    fn analysis_item(
        source_type_id: uuid::Uuid,
        source_name: &str,
        role: ReflectedTypeRole,
        emitted_scope_segments: Vec<&str>,
        emitted_scope_reason: LayoutScopeReason,
        field_owner_edges: Vec<LayoutFieldOwnerEdge>,
    ) -> LayoutAnalysisItem {
        LayoutAnalysisItem {
            source_type_id,
            source_name: source_name.to_owned(),
            role,
            is_abstract: Some(false),
            factory: None,
            serialized_field_count: 0,
            serialized_base_field_count: 0,
            serialized_data_field_count: 0,
            serialized_shape: LayoutSerializedShape::ConcreteDataType,
            is_base_family_root: false,
            namespace_segments: Vec::new(),
            primary_base_chain: Vec::<LayoutBaseEdge>::new(),
            direct_derived_source_names: Vec::new(),
            slot_anchor: None,
            field_owner_edges,
            concrete_slot_binding: None,
            concrete_slot_candidates: Vec::<LayoutConcreteSlotCandidate>::new(),
            has_ambiguous_concrete_slot_binding: false,
            emitted_scope_segments: emitted_scope_segments
                .into_iter()
                .map(str::to_owned)
                .collect(),
            emitted_scope_reason,
        }
    }
}
