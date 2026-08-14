use super::*;

pub(super) fn state_generation_plans(
    schema: &NetworkSchema,
    wire_shapes: &BTreeMap<&str, &SchemaWireShape>,
    handler_vtables: &BTreeMap<&str, &NetworkFieldHandlerVtable>,
    value_type_candidates: &BTreeMap<&str, Vec<NetworkNativeTypeInfoEvidence>>,
    serialize_types: &BTreeMap<Uuid, &NetworkSerializeType>,
    context: &CodegenContext,
) -> Result<Vec<NetworkStateGenerationPlanReport>, NetworkRustEmitError> {
    let wire_shape_sources = wire_shape_sources_by_handler_vtable(schema);
    let state_types = schema
        .types
        .iter()
        .filter(|network_type| {
            network_type
                .capabilities
                .contains(&NetworkTypeCapability::ReplicatedState)
        })
        .collect::<Vec<_>>();
    let plans =
        context
            .runner()
            .map_until_cancelled(&state_types, context.cancel(), |network_type| {
                state_generation_plan(
                    network_type,
                    wire_shapes,
                    &wire_shape_sources,
                    handler_vtables,
                    value_type_candidates,
                    serialize_types,
                )
            });
    if plans.was_cancelled() {
        return Err(NetworkRustEmitError::Cancelled);
    }
    Ok(plans.into_completed())
}

pub(super) fn message_generation_plans(
    schema: &NetworkSchema,
    wire_shapes: &BTreeMap<&str, &SchemaWireShape>,
    value_type_candidates: &BTreeMap<&str, Vec<NetworkNativeTypeInfoEvidence>>,
    context: &CodegenContext,
) -> Result<Vec<NetworkMessageGenerationPlanReport>, NetworkRustEmitError> {
    let wire_shape_sources = wire_shape_sources_by_handler_vtable(schema);
    let serialize_types = serialize_types_by_type_id(schema);
    let message_types = schema
        .types
        .iter()
        .filter(|network_type| {
            network_type
                .capabilities
                .contains(&NetworkTypeCapability::DirectMessage)
        })
        .collect::<Vec<_>>();
    let plans =
        context
            .runner()
            .map_until_cancelled(&message_types, context.cancel(), |network_type| {
                message_generation_plan(
                    network_type,
                    wire_shapes,
                    &wire_shape_sources,
                    value_type_candidates,
                    &serialize_types,
                )
            });
    if plans.was_cancelled() {
        return Err(NetworkRustEmitError::Cancelled);
    }
    Ok(plans.into_completed())
}

pub(super) fn state_generation_plan(
    network_type: &NetworkType,
    wire_shapes: &BTreeMap<&str, &SchemaWireShape>,
    wire_shape_sources: &BTreeMap<&str, &str>,
    handler_vtables: &BTreeMap<&str, &NetworkFieldHandlerVtable>,
    value_type_candidates: &BTreeMap<&str, Vec<NetworkNativeTypeInfoEvidence>>,
    serialize_types: &BTreeMap<Uuid, &NetworkSerializeType>,
) -> NetworkStateGenerationPlanReport {
    let attribute_count = network_type
        .fields
        .iter()
        .filter(|field| is_replicated_state_attribute_field(field))
        .count();
    let mut fields = network_type
        .fields
        .iter()
        .filter(|field| !is_replicated_state_attribute_field(field))
        .map(|field| {
            state_field_shape_report(
                field,
                wire_shapes,
                wire_shape_sources,
                handler_vtables,
                value_type_candidates,
                serialize_types,
            )
        })
        .collect::<Vec<_>>();
    disambiguate_report_field_names(&mut fields);
    canonicalize_shared_anonymous_container_support_types(&mut fields);
    let field_count = fields.len();
    let shaped_field_count = fields
        .iter()
        .filter(|field| state_field_has_complete_shape(field))
        .count();
    let supported_field_count = fields.iter().filter(|field| field.supported).count();
    let missing_wire_shape_count = fields
        .iter()
        .filter(|field| field.blocked_reason.as_deref() == Some("missing-wire-shape"))
        .count();
    let unsupported_wire_shape_count = fields
        .iter()
        .filter(|field| field.blocked_reason.as_deref() == Some("unsupported-wire-shape"))
        .count();
    let low_confidence_field_count = fields
        .iter()
        .filter(|field| !field.confidence.is_high_or_exact())
        .count();
    let evidence_issues = state_evidence_issues(network_type);
    let mut blocked_reasons = state_blocked_reasons(network_type, &fields);
    let fatal_evidence_issue_count = evidence_issues
        .iter()
        .filter(|issue| state_evidence_issue_is_fatal(issue, &fields))
        .count();
    if fatal_evidence_issue_count != 0 {
        blocked_reasons.push(format!("invalid-evidence:{fatal_evidence_issue_count}"));
    }
    NetworkStateGenerationPlanReport {
        type_index: network_type.type_index,
        type_name: network_type.name.clone(),
        fragment_category: network_type
            .fragment_metadata
            .as_ref()
            .and_then(|metadata| metadata.category.clone()),
        fragment_category_value: network_type
            .fragment_metadata
            .as_ref()
            .and_then(|metadata| metadata.category_value),
        is_metadata_fragment: network_type
            .fragment_metadata
            .as_ref()
            .and_then(|metadata| metadata.is_metadata),
        field_count,
        attribute_count,
        shaped_field_count,
        supported_field_count,
        missing_wire_shape_count,
        unsupported_wire_shape_count,
        low_confidence_field_count,
        evidence_issues,
        can_generate: blocked_reasons.is_empty(),
        blocked_reasons,
        fields,
    }
}

fn state_evidence_issue_is_fatal(
    issue: &NetworkEvidenceIssue,
    fields: &[NetworkStateFieldShapeReport],
) -> bool {
    if issue.kind != NetworkEvidenceIssueKind::NestedWireMismatch || issue.field_indices.is_empty()
    {
        return true;
    }
    issue.field_indices.iter().any(|field_index| {
        !fields
            .iter()
            .any(|field| field.field_index == Some(*field_index) && field.supported)
    })
}

pub(super) fn is_replicated_state_attribute_field(field: &NetworkField) -> bool {
    field.registration_kind.as_deref() == Some("attribute")
}

pub(super) fn message_generation_plan(
    network_type: &NetworkType,
    wire_shapes: &BTreeMap<&str, &SchemaWireShape>,
    wire_shape_sources: &BTreeMap<&str, &str>,
    value_type_candidates: &BTreeMap<&str, Vec<NetworkNativeTypeInfoEvidence>>,
    serialize_types: &BTreeMap<Uuid, &NetworkSerializeType>,
) -> NetworkMessageGenerationPlanReport {
    let supports_unmarshal = network_type
        .instance
        .as_ref()
        .and_then(|instance| instance.supports_unmarshal);
    let native_fields =
        if supports_unmarshal == Some(false) && !network_type.marshal_fields.is_empty() {
            &network_type.marshal_fields
        } else {
            &network_type.fields
        };
    let mut fields = native_fields
        .iter()
        .map(|field| {
            message_field_shape_report(
                field,
                wire_shapes,
                wire_shape_sources,
                value_type_candidates,
                serialize_types,
                network_type.name.as_deref(),
            )
        })
        .collect::<Vec<_>>();
    disambiguate_report_field_names(&mut fields);
    let field_count = fields.len();
    let shaped_field_count = fields
        .iter()
        .filter(|field| field.wire_shape.is_some())
        .count();
    let supported_field_count = fields.iter().filter(|field| field.supported).count();
    let missing_wire_shape_count = fields
        .iter()
        .filter(|field| field.wire_shape.is_none())
        .count();
    let missing_field_type_count = fields
        .iter()
        .filter(|field| field.blocked_reason.as_deref() == Some("missing-field-type"))
        .count();
    let missing_support_type_count = fields
        .iter()
        .filter(|field| field.blocked_reason.as_deref() == Some("missing-support-type"))
        .count();
    let missing_composite_support_type_count = fields
        .iter()
        .filter(|field| field.blocked_reason.as_deref() == Some("missing-composite-support-type"))
        .count();
    let unsupported_wire_shape_count = fields
        .iter()
        .filter(|field| field.blocked_reason.as_deref() == Some("unsupported-wire-shape"))
        .count();
    let low_confidence_field_count = fields
        .iter()
        .filter(|field| !field.confidence.is_high_or_exact())
        .count();
    let placeholder_field_name_count = fields
        .iter()
        .filter(|field| is_placeholder_report_field_name(field))
        .count();
    let evidence_issues = message_evidence_issues(network_type);
    let mut blocked_reasons = message_blocked_reasons(network_type, &fields);
    if !evidence_issues.is_empty() {
        blocked_reasons.push(format!("invalid-evidence:{}", evidence_issues.len()));
    }

    NetworkMessageGenerationPlanReport {
        type_index: network_type.type_index,
        type_name: network_type.name.clone(),
        analysis_status: network_type
            .instance
            .as_ref()
            .and_then(|instance| instance.analysis_status),
        empty_wire_proven: network_type
            .instance
            .as_ref()
            .is_some_and(|instance| instance.empty_wire_proven),
        supports_unmarshal,
        field_count,
        shaped_field_count,
        supported_field_count,
        missing_wire_shape_count,
        missing_field_type_count,
        missing_support_type_count,
        missing_composite_support_type_count,
        placeholder_field_name_count,
        unsupported_wire_shape_count,
        low_confidence_field_count,
        evidence_issues,
        can_generate: blocked_reasons.is_empty(),
        blocked_reasons,
        fields,
    }
}

pub(super) fn disambiguate_report_field_names(fields: &mut [NetworkStateFieldShapeReport]) {
    let mut seen = BTreeMap::<String, usize>::new();
    for (ordinal, field) in fields.iter_mut().enumerate() {
        let Some(name) = field.field_name.as_deref() else {
            continue;
        };
        let ident = rust_field_ident(name);
        if let std::collections::btree_map::Entry::Vacant(entry) = seen.entry(ident.clone()) {
            entry.insert(1);
            continue;
        }

        let suffix_seed = field.field_index.unwrap_or(ordinal as u32);
        let mut attempt = 0;
        let candidate = loop {
            let suffix = if attempt == 0 {
                suffix_seed.to_string()
            } else {
                format!("{suffix_seed}_{attempt}")
            };
            let candidate = format!("{name}_{suffix}");
            let candidate_ident = rust_field_ident(&candidate);
            if let std::collections::btree_map::Entry::Vacant(entry) = seen.entry(candidate_ident) {
                entry.insert(1);
                break candidate;
            }
            attempt += 1;
        };

        if let Some(count) = seen.get_mut(&ident) {
            *count += 1;
        }
        field.field_name = Some(candidate);
    }
}

fn canonicalize_shared_anonymous_container_support_types(
    fields: &mut [NetworkStateFieldShapeReport],
) {
    let mut groups = Vec::<Vec<usize>>::new();
    for field_index in 0..fields.len() {
        let Some(shape) = fields[field_index].container_value_type_shape.as_ref() else {
            continue;
        };
        if !shape.has_proven_anonymous_layout() {
            continue;
        }
        if let Some(group) = groups.iter_mut().find(|group| {
            shared_anonymous_container_projection(&fields[group[0]], &fields[field_index])
        }) {
            group.push(field_index);
        } else {
            groups.push(vec![field_index]);
        }
    }

    for group in groups.into_iter().filter(|group| group.len() > 1) {
        let representative = group[0];
        let Some(default_field_name) = fields[representative].field_name.clone() else {
            continue;
        };
        let shared_field_name = common_numbered_field_name(fields, &group)
            .filter(|candidate| {
                shared_support_name_is_available(fields, &group, representative, candidate)
            })
            .unwrap_or(default_field_name);

        for field_index in group {
            canonicalize_container_support_names(&mut fields[field_index], &shared_field_name);
        }
    }
}

fn shared_anonymous_container_projection(
    left: &NetworkStateFieldShapeReport,
    right: &NetworkStateFieldShapeReport,
) -> bool {
    left.handler_vtable.is_some()
        && left.handler_vtable == right.handler_vtable
        && support_shape_options_match(
            left.container_key_type_shape.as_ref(),
            right.container_key_type_shape.as_ref(),
        )
        && support_shape_slices_match(
            &left.container_embedded_key_type_shapes,
            &right.container_embedded_key_type_shapes,
        )
        && support_shape_options_match(
            left.container_value_type_shape.as_ref(),
            right.container_value_type_shape.as_ref(),
        )
        && support_shape_slices_match(
            &left.container_embedded_value_type_shapes,
            &right.container_embedded_value_type_shapes,
        )
        && normalized_container_types_match(
            left,
            left.rust_value_type.as_deref(),
            right,
            right.rust_value_type.as_deref(),
        )
        && normalized_container_types_match(
            left,
            left.rust_field_type.as_deref(),
            right,
            right.rust_field_type.as_deref(),
        )
}

fn normalized_container_types_match(
    left: &NetworkStateFieldShapeReport,
    left_type: Option<&str>,
    right: &NetworkStateFieldShapeReport,
    right_type: Option<&str>,
) -> bool {
    normalized_container_type(left, left_type)
        .zip(normalized_container_type(right, right_type))
        .is_some_and(|(left, right)| left == right)
}

fn normalized_container_type(
    field: &NetworkStateFieldShapeReport,
    rust_type: Option<&str>,
) -> Option<String> {
    let shape = field.container_value_type_shape.as_ref()?;
    let field_name = field.field_name.as_deref()?;
    let value_type = container_value_shape_support_type_name(field_name, shape)?;
    let codec_type = structured_value_codec_name(field_name, shape)?;
    Some(
        rust_type?
            .replace(&codec_type, "{ANONYMOUS_CODEC}")
            .replace(&value_type, "{ANONYMOUS_VALUE}"),
    )
}

fn support_shape_options_match(
    left: Option<&crate::network_schema::NetworkNestedTypeShape>,
    right: Option<&crate::network_schema::NetworkNestedTypeShape>,
) -> bool {
    match (left, right) {
        (Some(left), Some(right)) => support_shapes_match(left, right),
        (None, None) => true,
        _ => false,
    }
}

fn support_shape_slices_match(
    left: &[crate::network_schema::NetworkNestedTypeShape],
    right: &[crate::network_schema::NetworkNestedTypeShape],
) -> bool {
    left.len() == right.len()
        && left
            .iter()
            .zip(right)
            .all(|(left, right)| support_shapes_match(left, right))
}

fn support_shapes_match(
    left: &crate::network_schema::NetworkNestedTypeShape,
    right: &crate::network_schema::NetworkNestedTypeShape,
) -> bool {
    if left.has_exact_identity() || right.has_exact_identity() {
        return left.has_exact_identity()
            && right.has_exact_identity()
            && left.type_id == right.type_id;
    }
    let Some(left_members) = nested_type_shape_members_in_wire_order(left) else {
        return false;
    };
    let Some(right_members) = nested_type_shape_members_in_wire_order(right) else {
        return false;
    };
    left.native_size == right.native_size
        && left_members.len() == right_members.len()
        && left_members
            .into_iter()
            .zip(right_members)
            .all(|(left, right)| {
                left.offset == right.offset
                    && left.native_offset == right.native_offset
                    && left.name == right.name
                    && left.native_type == right.native_type
                    && left.type_id == right.type_id
                    && left.type_identity_proven == right.type_identity_proven
                    && left.wire_shape == right.wire_shape
                    && left.wire_layout == right.wire_layout
                    && left.byte_width == right.byte_width
                    && left.wire_ordinal == right.wire_ordinal
                    && left.type_conflict == right.type_conflict
            })
}

fn common_numbered_field_name(
    fields: &[NetworkStateFieldShapeReport],
    group: &[usize],
) -> Option<String> {
    let mut names = group
        .iter()
        .map(|index| fields[*index].field_name.as_deref())
        .collect::<Option<Vec<_>>>()?
        .into_iter();
    let first = strip_numeric_field_suffix(names.next()?);
    (!first.is_empty() && names.all(|name| strip_numeric_field_suffix(name) == first))
        .then(|| first.to_owned())
}

fn strip_numeric_field_suffix(name: &str) -> &str {
    name.trim_end_matches(|character: char| character.is_ascii_digit())
        .trim_end_matches('_')
}

fn shared_support_name_is_available(
    fields: &[NetworkStateFieldShapeReport],
    group: &[usize],
    representative: usize,
    candidate: &str,
) -> bool {
    let Some(shape) = fields[representative].container_value_type_shape.as_ref() else {
        return false;
    };
    let Some(candidate_value) = container_value_shape_support_type_name(candidate, shape) else {
        return false;
    };
    let Some(candidate_codec) = structured_value_codec_name(candidate, shape) else {
        return false;
    };
    fields.iter().enumerate().all(|(index, field)| {
        if group.contains(&index) {
            return true;
        }
        let Some(shape) = field.container_value_type_shape.as_ref() else {
            return true;
        };
        let Some(field_name) = field.field_name.as_deref() else {
            return true;
        };
        container_value_shape_support_type_name(field_name, shape)
            .is_none_or(|name| name != candidate_value)
            && structured_value_codec_name(field_name, shape)
                .is_none_or(|name| name != candidate_codec)
    })
}

fn canonicalize_container_support_names(
    field: &mut NetworkStateFieldShapeReport,
    shared_field_name: &str,
) {
    let Some(shape) = field.container_value_type_shape.as_ref() else {
        return;
    };
    let Some(field_name) = field.field_name.as_deref() else {
        return;
    };
    let Some(old_value) = container_value_shape_support_type_name(field_name, shape) else {
        return;
    };
    let Some(old_codec) = structured_value_codec_name(field_name, shape) else {
        return;
    };
    let Some(new_value) = container_value_shape_support_type_name(shared_field_name, shape) else {
        return;
    };
    let Some(new_codec) = structured_value_codec_name(shared_field_name, shape) else {
        return;
    };
    for rust_type in [&mut field.rust_value_type, &mut field.rust_field_type]
        .into_iter()
        .flatten()
    {
        *rust_type = rust_type
            .replace(&old_codec, "{SHARED_CONTAINER_CODEC}")
            .replace(&old_value, "{SHARED_CONTAINER_VALUE}")
            .replace("{SHARED_CONTAINER_CODEC}", &new_codec)
            .replace("{SHARED_CONTAINER_VALUE}", &new_value);
    }
    field.support_type_field_name = Some(shared_field_name.to_owned());
}

pub(super) const BLOCKER_EXAMPLE_LIMIT: usize = 8;
pub(super) const BLOCKED_FIELD_EXAMPLE_LIMIT: usize = 8;

pub(super) fn state_blocker_summary(
    plans: &[NetworkStateGenerationPlanReport],
) -> NetworkBlockerSummaryReport {
    let mut reason_buckets = BTreeMap::<String, NetworkBlockerReasonBucketReport>::new();
    let mut combination_buckets =
        BTreeMap::<Vec<String>, NetworkBlockerCombinationBucketReport>::new();

    for plan in plans.iter().filter(|plan| !plan.can_generate) {
        let example = blocked_state_type_example(plan);
        let reason_families = plan
            .blocked_reasons
            .iter()
            .map(|reason| blocker_reason_family(reason).to_owned())
            .collect::<BTreeSet<_>>();
        for reason in reason_families {
            let bucket = reason_buckets.entry(reason.clone()).or_insert_with(|| {
                NetworkBlockerReasonBucketReport {
                    reason,
                    ..NetworkBlockerReasonBucketReport::default()
                }
            });
            bucket.type_count += 1;
            bucket.blocked_field_count +=
                blocked_state_field_count_for_reason(plan, &bucket.reason);
            if bucket.examples.len() < BLOCKER_EXAMPLE_LIMIT {
                bucket.examples.push(example.clone());
            }
        }

        let mut reasons = plan.blocked_reasons.clone();
        reasons.sort();
        let bucket = combination_buckets
            .entry(reasons.clone())
            .or_insert_with(|| NetworkBlockerCombinationBucketReport {
                reasons,
                ..NetworkBlockerCombinationBucketReport::default()
            });
        bucket.type_count += 1;
        if bucket.examples.len() < BLOCKER_EXAMPLE_LIMIT {
            bucket.examples.push(example);
        }
    }

    let mut reason_buckets = reason_buckets.into_values().collect::<Vec<_>>();
    reason_buckets.sort_by(|left, right| {
        right
            .type_count
            .cmp(&left.type_count)
            .then_with(|| left.reason.cmp(&right.reason))
    });
    let mut combination_buckets = combination_buckets.into_values().collect::<Vec<_>>();
    combination_buckets.sort_by(|left, right| {
        right
            .type_count
            .cmp(&left.type_count)
            .then_with(|| left.reasons.cmp(&right.reasons))
    });

    NetworkBlockerSummaryReport {
        total_plan_count: plans.len(),
        generatable_count: plans.iter().filter(|plan| plan.can_generate).count(),
        blocked_count: plans.iter().filter(|plan| !plan.can_generate).count(),
        reason_buckets,
        combination_buckets,
    }
}

pub(super) fn message_blocker_summary(
    plans: &[NetworkMessageGenerationPlanReport],
) -> NetworkBlockerSummaryReport {
    let mut reason_buckets = BTreeMap::<String, NetworkBlockerReasonBucketReport>::new();
    let mut combination_buckets =
        BTreeMap::<Vec<String>, NetworkBlockerCombinationBucketReport>::new();

    for plan in plans.iter().filter(|plan| !plan.can_generate) {
        let example = blocked_type_example(plan);
        let reason_families = plan
            .blocked_reasons
            .iter()
            .map(|reason| blocker_reason_family(reason).to_owned())
            .collect::<BTreeSet<_>>();
        for reason in reason_families {
            let bucket = reason_buckets.entry(reason.clone()).or_insert_with(|| {
                NetworkBlockerReasonBucketReport {
                    reason,
                    ..NetworkBlockerReasonBucketReport::default()
                }
            });
            bucket.type_count += 1;
            bucket.blocked_field_count += blocked_field_count_for_reason(plan, &bucket.reason);
            if bucket.examples.len() < BLOCKER_EXAMPLE_LIMIT {
                bucket.examples.push(example.clone());
            }
        }

        let mut reasons = plan.blocked_reasons.clone();
        reasons.sort();
        let bucket = combination_buckets
            .entry(reasons.clone())
            .or_insert_with(|| NetworkBlockerCombinationBucketReport {
                reasons,
                ..NetworkBlockerCombinationBucketReport::default()
            });
        bucket.type_count += 1;
        if bucket.examples.len() < BLOCKER_EXAMPLE_LIMIT {
            bucket.examples.push(example);
        }
    }

    let mut reason_buckets = reason_buckets.into_values().collect::<Vec<_>>();
    reason_buckets.sort_by(|left, right| {
        right
            .type_count
            .cmp(&left.type_count)
            .then_with(|| left.reason.cmp(&right.reason))
    });

    let mut combination_buckets = combination_buckets.into_values().collect::<Vec<_>>();
    combination_buckets.sort_by(|left, right| {
        right
            .type_count
            .cmp(&left.type_count)
            .then_with(|| left.reasons.cmp(&right.reasons))
    });

    NetworkBlockerSummaryReport {
        total_plan_count: plans.len(),
        generatable_count: plans.iter().filter(|plan| plan.can_generate).count(),
        blocked_count: plans.iter().filter(|plan| !plan.can_generate).count(),
        reason_buckets,
        combination_buckets,
    }
}

pub(super) fn blocker_reason_family(reason: &str) -> &str {
    reason.split_once(':').map_or(reason, |(family, _)| family)
}

fn blocked_state_field_count_for_reason(
    plan: &NetworkStateGenerationPlanReport,
    reason: &str,
) -> usize {
    plan.fields
        .iter()
        .filter(|field| {
            field
                .blocked_reason
                .as_deref()
                .is_some_and(|field_reason| blocker_reason_family(field_reason) == reason)
        })
        .count()
}

fn blocked_state_type_example(
    plan: &NetworkStateGenerationPlanReport,
) -> NetworkBlockedTypeExampleReport {
    NetworkBlockedTypeExampleReport {
        type_index: plan.type_index,
        type_name: plan.type_name.clone(),
        field_count: plan.field_count,
        blocked_reasons: plan.blocked_reasons.clone(),
        blocked_fields: plan
            .fields
            .iter()
            .filter(|field| field.blocked_reason.is_some())
            .take(BLOCKED_FIELD_EXAMPLE_LIMIT)
            .map(blocked_field_example)
            .collect(),
    }
}

pub(super) fn blocked_field_count_for_reason(
    plan: &NetworkMessageGenerationPlanReport,
    reason: &str,
) -> usize {
    plan.fields
        .iter()
        .filter(|field| {
            field
                .blocked_reason
                .as_deref()
                .is_some_and(|field_reason| blocker_reason_family(field_reason) == reason)
        })
        .count()
}

pub(super) fn blocked_type_example(
    plan: &NetworkMessageGenerationPlanReport,
) -> NetworkBlockedTypeExampleReport {
    NetworkBlockedTypeExampleReport {
        type_index: plan.type_index,
        type_name: plan.type_name.clone(),
        field_count: plan.field_count,
        blocked_reasons: plan.blocked_reasons.clone(),
        blocked_fields: plan
            .fields
            .iter()
            .filter(|field| field.blocked_reason.is_some())
            .take(BLOCKED_FIELD_EXAMPLE_LIMIT)
            .map(blocked_field_example)
            .collect(),
    }
}

pub(super) fn blocked_field_example(
    field: &NetworkStateFieldShapeReport,
) -> NetworkBlockedFieldExampleReport {
    NetworkBlockedFieldExampleReport {
        field_index: field.field_index,
        field_name: field.field_name.clone(),
        native_type: field.native_type.clone(),
        source_type_name: field.source_type_name.clone(),
        source_type_id: field.source_type_id,
        serialize_type_name: field.serialize_type_name.clone(),
        wire_layout: field.wire_layout.clone(),
        wire_layout_source: field.wire_layout_source.clone(),
        value_type_candidates: field.value_type_candidates.clone(),
        rust_value_type: field.rust_value_type.clone(),
        blocked_reason: field.blocked_reason.clone(),
    }
}

#[cfg(test)]
mod evidence_severity_tests {
    use serde_json::json;

    use super::*;

    fn field(field_index: u32, supported: bool) -> NetworkStateFieldShapeReport {
        serde_json::from_value(json!({
            "fieldIndex": field_index,
            "fieldName": "value",
            "group": 0,
            "handlerVtable": null,
            "wireShape": "u32",
            "wireShapeSource": "test",
            "valueTypeCandidates": [],
            "rustValueType": "u32",
            "rustFieldType": "ReplicatedFieldHandler<u32>",
            "confidence": "high",
            "supported": supported,
            "blockedReason": if supported { None::<&str> } else { Some("missing-semantic-type") }
        }))
        .expect("state field report")
    }

    #[test]
    fn nested_wire_mismatch_is_advisory_after_the_field_is_supported() {
        let issue = NetworkEvidenceIssue {
            kind: NetworkEvidenceIssueKind::NestedWireMismatch,
            field_ordinals: vec![0],
            field_indices: vec![7],
            storage_offset: None,
            evidence: Some("stale-directional-layout".to_owned()),
        };

        assert!(!state_evidence_issue_is_fatal(&issue, &[field(7, true)]));
        assert!(state_evidence_issue_is_fatal(&issue, &[field(7, false)]));
    }

    #[test]
    fn non_wire_mismatch_evidence_remains_fatal() {
        let issue = NetworkEvidenceIssue {
            kind: NetworkEvidenceIssueKind::FieldTypeConflict,
            field_ordinals: vec![0],
            field_indices: vec![7],
            storage_offset: None,
            evidence: None,
        };

        assert!(state_evidence_issue_is_fatal(&issue, &[field(7, true)]));
    }
}
