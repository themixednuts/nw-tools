use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::network_schema::parse::{composite_member_wire_shapes, nested_type_shape_wire_shapes};
use crate::network_schema::{NetworkField, NetworkNestedTypeShape, NetworkType};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum NetworkEvidenceIssueKind {
    FieldCountConflict,
    FieldTypeConflict,
    FieldWireConflict,
    NestedMemberTypeConflict,
    DuplicateStorage,
    NestedStorageOverlap,
    NestedWireMismatch,
    NonRootStorage,
    StorageOffsetMismatch,
    UnprovenSourceTypeIdentity,
    UnprovenNestedTypeIdentity,
    UnprovenNestedMemberTypeIdentity,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NetworkEvidenceIssue {
    pub kind: NetworkEvidenceIssueKind,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub field_ordinals: Vec<usize>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub field_indices: Vec<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub storage_offset: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub evidence: Option<String>,
}

pub(super) fn state_evidence_issues(network_type: &NetworkType) -> Vec<NetworkEvidenceIssue> {
    field_evidence_issues(network_type)
}

pub(super) fn message_evidence_issues(network_type: &NetworkType) -> Vec<NetworkEvidenceIssue> {
    let mut issues = field_evidence_issues(network_type);
    if network_type.field_count_conflict {
        issues.push(NetworkEvidenceIssue {
            kind: NetworkEvidenceIssueKind::FieldCountConflict,
            field_ordinals: Vec::new(),
            field_indices: Vec::new(),
            storage_offset: None,
            evidence: Some(network_type.fields.len().to_string()),
        });
    }
    let mut storage = BTreeMap::<(&str, u32), usize>::new();

    for (ordinal, field) in network_type.fields.iter().enumerate() {
        if is_pcode_stack_field(field) && field.storage_base.as_deref() != Some("param_3") {
            issues.push(field_issue(
                NetworkEvidenceIssueKind::NonRootStorage,
                ordinal,
                field,
                field.storage_expression.clone(),
            ));
        }
        if field
            .storage_base_offset
            .zip(field.storage_offset)
            .is_some_and(|(base, storage)| base != storage)
        {
            issues.push(field_issue(
                NetworkEvidenceIssueKind::StorageOffsetMismatch,
                ordinal,
                field,
                field.storage_expression.clone(),
            ));
        }
        if let Some((base, offset)) = field.storage_base.as_deref().zip(field.storage_offset)
            && let Some(previous) = storage.insert((base, offset), ordinal)
        {
            issues.push(pair_issue(
                NetworkEvidenceIssueKind::DuplicateStorage,
                network_type,
                previous,
                ordinal,
                offset,
            ));
        }
    }

    for (parent_ordinal, parent) in network_type.fields.iter().enumerate() {
        let Some((start, end)) = exact_nested_storage_range(parent) else {
            continue;
        };
        for (child_ordinal, child) in network_type.fields.iter().enumerate() {
            if parent_ordinal == child_ordinal || child.storage_base != parent.storage_base {
                continue;
            }
            let Some(offset) = child.storage_offset else {
                continue;
            };
            if offset > start && offset < end {
                issues.push(pair_issue(
                    NetworkEvidenceIssueKind::NestedStorageOverlap,
                    network_type,
                    parent_ordinal,
                    child_ordinal,
                    offset,
                ));
            }
        }
    }

    issues
}

fn field_evidence_issues(network_type: &NetworkType) -> Vec<NetworkEvidenceIssue> {
    let mut issues = Vec::new();
    for (ordinal, field) in network_type.fields.iter().enumerate() {
        if field.type_conflict {
            issues.push(field_issue(
                NetworkEvidenceIssueKind::FieldTypeConflict,
                ordinal,
                field,
                field.native_type.clone(),
            ));
        }
        if field.wire_conflict {
            issues.push(field_issue(
                NetworkEvidenceIssueKind::FieldWireConflict,
                ordinal,
                field,
                field.wire_shape_raw.clone(),
            ));
        }
        if field.source_type_id.is_some() && !field.source_type_identity_proven {
            issues.push(field_issue(
                NetworkEvidenceIssueKind::UnprovenSourceTypeIdentity,
                ordinal,
                field,
                field.source_type_id_source.clone(),
            ));
        }
        if field
            .nested_type_shape
            .as_ref()
            .is_some_and(|shape| shape.members.iter().any(|member| member.type_conflict))
        {
            issues.push(field_issue(
                NetworkEvidenceIssueKind::NestedMemberTypeConflict,
                ordinal,
                field,
                field
                    .nested_type_shape
                    .as_ref()
                    .and_then(|shape| shape.type_name_full.clone().or(shape.type_name.clone())),
            ));
        }
        if let Some(shape) = field.nested_type_shape.as_ref() {
            if shape.type_id.is_some() && !shape.has_exact_identity() {
                issues.push(field_issue(
                    NetworkEvidenceIssueKind::UnprovenNestedTypeIdentity,
                    ordinal,
                    field,
                    shape.type_id_source.clone(),
                ));
            }
            for member in &shape.members {
                let identity_proven = member.type_identity_proven
                    || shape.has_exact_identity()
                        && member.type_id_source.as_deref()
                            == Some("serialize-field-for-proven-type");
                if member.type_id.is_some() && !identity_proven {
                    issues.push(field_issue(
                        NetworkEvidenceIssueKind::UnprovenNestedMemberTypeIdentity,
                        ordinal,
                        field,
                        member.type_id_source.clone(),
                    ));
                }
            }
        }
        if let Some(shape) = field.nested_type_shape.as_ref()
            && !nested_wire_shape_matches(field, shape)
        {
            issues.push(field_issue(
                NetworkEvidenceIssueKind::NestedWireMismatch,
                ordinal,
                field,
                field.wire_shape_raw.clone(),
            ));
        }
    }
    issues
}

fn is_pcode_stack_field(field: &NetworkField) -> bool {
    field
        .evidence
        .iter()
        .any(|evidence| evidence.source.starts_with("message-unmarshal-pcode-stack"))
}

fn nested_wire_shape_matches(field: &NetworkField, shape: &NetworkNestedTypeShape) -> bool {
    let Some(raw) = field.wire_shape_raw.as_deref() else {
        return true;
    };
    let Some(expected) = composite_member_wire_shapes(raw) else {
        return true;
    };
    nested_type_shape_wire_shapes(shape, &[]).is_some_and(|observed| observed == expected)
}

fn exact_nested_storage_range(field: &NetworkField) -> Option<(u32, u32)> {
    let shape = field
        .nested_type_shape
        .as_ref()
        .filter(|shape| shape.has_exact_identity())?;
    let start = field.storage_offset?;
    let size = u32::try_from(shape.native_size?).ok()?;
    Some((start, start.checked_add(size)?))
}

fn field_issue(
    kind: NetworkEvidenceIssueKind,
    ordinal: usize,
    field: &NetworkField,
    evidence: Option<String>,
) -> NetworkEvidenceIssue {
    NetworkEvidenceIssue {
        kind,
        field_ordinals: vec![ordinal],
        field_indices: field.index.into_iter().collect(),
        storage_offset: field.storage_offset,
        evidence,
    }
}

fn pair_issue(
    kind: NetworkEvidenceIssueKind,
    network_type: &NetworkType,
    left: usize,
    right: usize,
    storage_offset: u32,
) -> NetworkEvidenceIssue {
    NetworkEvidenceIssue {
        kind,
        field_ordinals: vec![left, right],
        field_indices: [left, right]
            .into_iter()
            .filter_map(|ordinal| network_type.fields[ordinal].index)
            .collect(),
        storage_offset: Some(storage_offset),
        evidence: None,
    }
}
