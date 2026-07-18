use super::*;

#[derive(Clone)]
pub(super) struct NetworkFieldHandlerProjection<'a> {
    handler_kind: Option<&'a str>,
    handler_kind_source: Option<&'a str>,
    vtable_slots: Option<u32>,
    physical_field_count: Option<u32>,
    wire_shape: Option<NetworkWireShape>,
    wire_shape_source: Option<&'a str>,
    wire_layout: Option<&'a str>,
    wire_layout_source: Option<&'a str>,
    value_type_shape: Option<&'a NetworkNestedTypeShape>,
}

pub(super) fn field_handler_projections(
    vtables: &[NetworkFieldHandlerVtable],
) -> BTreeMap<&str, NetworkFieldHandlerProjection<'_>> {
    vtables
        .iter()
        .filter_map(|vtable| {
            let address = vtable.address.as_deref()?;
            let wire_layout = vtable
                .wire_layout
                .as_deref()
                .or(vtable.full_wire_layout.as_deref());
            let wire_shape = vtable
                .wire_shape
                .clone()
                .or_else(|| wire_layout.and_then(NetworkWireShape::from_self_describing_layout));
            let wire_shape_source = vtable.wire_shape_source.as_deref().or_else(|| {
                wire_shape.as_ref().map(|_| {
                    vtable
                        .wire_layout_source
                        .as_deref()
                        .unwrap_or("field-handler-vtable-layout")
                })
            });
            Some((
                address,
                NetworkFieldHandlerProjection {
                    handler_kind: vtable.handler_kind.as_deref(),
                    handler_kind_source: vtable.handler_kind_source.as_deref(),
                    vtable_slots: vtable.vtable_slots,
                    physical_field_count: vtable.physical_field_count,
                    wire_shape,
                    wire_shape_source,
                    wire_layout,
                    wire_layout_source: wire_layout.map(|_| {
                        vtable
                            .wire_layout_source
                            .as_deref()
                            .unwrap_or("field-handler-vtable-layout")
                    }),
                    value_type_shape: vtable
                        .value_type_shape
                        .as_ref()
                        .filter(|shape| shape.has_exact_identity()),
                },
            ))
        })
        .collect()
}

pub(super) fn enrich_fields_from_handler_projections(
    fields: &mut [NetworkField],
    projections: &BTreeMap<&str, NetworkFieldHandlerProjection<'_>>,
) {
    for field in fields {
        let Some(projection) = field
            .handler_vtable
            .as_deref()
            .and_then(|vtable| projections.get(vtable))
        else {
            continue;
        };

        field.handler_kind = field
            .handler_kind
            .take()
            .or_else(|| projection.handler_kind.map(ToOwned::to_owned));
        field.handler_kind_source = field
            .handler_kind_source
            .take()
            .or_else(|| projection.handler_kind_source.map(ToOwned::to_owned));
        field.handler_vtable_slots = field.handler_vtable_slots.or(projection.vtable_slots);
        field.physical_field_count = field
            .physical_field_count
            .or(projection.physical_field_count);
        if field.wire_shape.is_none() {
            field.wire_shape.clone_from(&projection.wire_shape);
        }
        field.wire_shape_source = field
            .wire_shape_source
            .take()
            .or_else(|| projection.wire_shape_source.map(ToOwned::to_owned));
        field.wire_layout = field
            .wire_layout
            .take()
            .or_else(|| projection.wire_layout.map(ToOwned::to_owned));
        field.wire_layout_source = field
            .wire_layout_source
            .take()
            .or_else(|| projection.wire_layout_source.map(ToOwned::to_owned));
        if field.nested_type_shape.is_none()
            && let Some(shape) = projection.value_type_shape
        {
            field.source_type_name = shape
                .type_name_full
                .clone()
                .or_else(|| shape.type_name.clone());
            field.source_type_id = shape.type_id;
            field.source_type_id_source = shape
                .identity_source
                .clone()
                .or_else(|| shape.type_id_source.clone());
            field.source_type_identity_proven = true;
            field.nested_type_shape = Some(shape.clone());
        }
    }
}

pub(super) fn invalidate_fields_for_handler_vtables(
    fields: &mut [NetworkField],
    addresses: &BTreeSet<String>,
) -> usize {
    let mut count = 0;
    for field in fields {
        if !field
            .handler_vtable
            .as_ref()
            .is_some_and(|address| addresses.contains(address))
        {
            continue;
        }

        field.handler_kind = None;
        field.handler_kind_source = None;
        field.handler_vtable_slots = None;
        field.physical_field_count = None;
        field.wire_shape = None;
        field.wire_shape_source = None;
        field.wire_layout = None;
        field.wire_layout_source = None;
        field.source_type_name = None;
        field.source_type_id = None;
        field.source_type_id_source = None;
        field.source_type_identity_proven = false;
        field.nested_type_shape = None;
        count += 1;
    }
    count
}

pub(super) fn network_fields_from_message_signature(
    fields: &[NetworkMessageFieldSignature],
    source: String,
) -> Vec<NetworkField> {
    fields
        .iter()
        .enumerate()
        .map(|(index, field)| network_field_from_message_signature(index, field, source.clone()))
        .collect()
}

pub(super) fn network_field_from_message_signature(
    fallback_index: usize,
    signature: &NetworkMessageFieldSignature,
    source: String,
) -> NetworkField {
    let index = signature
        .index
        .or_else(|| u32::try_from(fallback_index).ok());
    let evidence = vec![NetworkEvidence {
        kind: NetworkEvidenceKind::MessageSource,
        source: source.clone(),
        address: None,
        detail: Some(signature.name.clone()),
        confidence: NetworkConfidence::High,
    }];
    NetworkField {
        index,
        name: Some(signature.name.clone()),
        name_address: None,
        group: None,
        registration_kind: None,
        filter_group_attribute: None,
        handler_offset: None,
        handler_expression: None,
        handler_vtable: None,
        handler_kind: None,
        handler_kind_source: None,
        handler_vtable_slots: None,
        physical_field_count: None,
        native_type: signature.native_type.clone(),
        source_type_name: None,
        source_type_id: None,
        source_type_id_source: None,
        source_type_identity_proven: false,
        rust_type: signature.rust_type.clone(),
        storage_expression: None,
        storage_base: None,
        storage_base_offset: None,
        storage_offset: None,
        raw_byte_length: None,
        wire_shape: signature.wire_shape.clone(),
        wire_shape_raw: None,
        wire_layout: signature
            .wire_shape
            .as_ref()
            .map(NetworkWireShape::wire_string),
        wire_layout_source: signature.wire_shape.as_ref().map(|_| source.clone()),
        type_conflict: false,
        wire_conflict: false,
        wire_shape_source: signature.wire_shape.as_ref().map(|_| source),
        constructor_writes: Vec::new(),
        unmarshal_evidence: None,
        nested_type_shape: None,
        serialize: None,
        callsite: None,
        confidence: NetworkConfidence::High,
        evidence,
    }
}

pub(super) fn field_override_type_candidates(
    types: &[NetworkType],
    field_override: &NetworkFieldOverride,
) -> Vec<usize> {
    if field_override.type_id.is_none()
        && field_override.type_index.is_none()
        && field_override.type_name.is_none()
    {
        return Vec::new();
    }

    types
        .iter()
        .enumerate()
        .filter_map(|(index, network_type)| {
            field_override_matches_type(network_type, field_override).then_some(index)
        })
        .collect()
}

pub(super) fn field_override_matches_type(
    network_type: &NetworkType,
    field_override: &NetworkFieldOverride,
) -> bool {
    field_override
        .type_id
        .is_none_or(|type_id| network_type.type_id == Some(type_id))
        && field_override
            .type_index
            .is_none_or(|type_index| network_type.type_index == Some(type_index))
        && field_override.type_name.as_deref().is_none_or(|type_name| {
            network_type.name.as_deref() == Some(type_name)
                || network_type.registration_type_name.as_deref() == Some(type_name)
        })
}

pub(super) fn field_override_field_candidates(
    network_type: &NetworkType,
    field_override: &NetworkFieldOverride,
) -> Vec<usize> {
    if field_override.field_index.is_none() && field_override.field.is_none() {
        return Vec::new();
    }

    network_type
        .fields
        .iter()
        .enumerate()
        .filter_map(|(index, field)| {
            field_override_matches_field(field, field_override).then_some(index)
        })
        .collect()
}

pub(super) fn field_override_matches_field(
    field: &NetworkField,
    field_override: &NetworkFieldOverride,
) -> bool {
    field_override
        .field_index
        .is_none_or(|field_index| field.index == Some(field_index))
        && field_override
            .field
            .as_deref()
            .is_none_or(|field_name| field.name.as_deref() == Some(field_name))
}

pub(super) fn field_override_detail(field_override: &NetworkFieldOverride) -> String {
    let type_part = field_override
        .type_name
        .as_deref()
        .map(ToOwned::to_owned)
        .or_else(|| {
            field_override
                .type_index
                .map(|type_index| type_index.to_string())
        })
        .or_else(|| field_override.type_id.map(|type_id| type_id.to_string()))
        .unwrap_or_else(|| "<unknown-type>".to_owned());
    let field_part = field_override
        .field
        .clone()
        .or_else(|| {
            field_override
                .field_index
                .map(|field_index| field_index.to_string())
        })
        .unwrap_or_else(|| "<unknown-field>".to_owned());
    format!("{type_part}.{field_part}")
}

pub(super) fn serialize_items_by_name(
    unit: &SerializeCodegenUnit,
) -> BTreeMap<&str, Vec<&SerializeCodegenItem>> {
    let mut index = BTreeMap::<&str, Vec<&SerializeCodegenItem>>::new();
    for item in &unit.items {
        index.entry(&item.source_name).or_default().push(item);
    }
    index
}

pub(super) fn serialize_match<'a>(
    network_type: &NetworkType,
    type_index: &'a SerializeCodegenIndex<'a>,
    name_index: &'a BTreeMap<&str, Vec<&'a SerializeCodegenItem>>,
    report: &mut NetworkSerializeMergeReport,
) -> Option<(&'a SerializeCodegenItem, NetworkConfidence, String)> {
    if let Some(type_id) = network_type.type_id
        && !type_id.is_nil()
        && let Some(item) = type_index.item_by_type_id(type_id)
    {
        report.type_id_matched_count += 1;
        return Some((item, NetworkConfidence::High, "serializeContext".to_owned()));
    }

    let Some(name) = network_type.name.as_deref() else {
        report.unmatched_schema_type_count += 1;
        return None;
    };
    let Some(candidates) = name_index.get(name) else {
        report.unmatched_schema_type_count += 1;
        return None;
    };
    let [item] = candidates.as_slice() else {
        report.ambiguous_name_match_count += 1;
        report.unmatched_schema_type_count += 1;
        return None;
    };
    report.name_matched_count += 1;
    Some((
        item,
        NetworkConfidence::Inferred,
        "serializeContext:name".to_owned(),
    ))
}

pub(super) fn merge_field_serialize_types(
    network_type: &mut NetworkType,
    type_index: &SerializeCodegenIndex<'_>,
    selected_value_types: &BTreeMap<String, NetworkNativeTypeInfoEvidence>,
    report: &mut NetworkSerializeMergeReport,
) {
    for field in &mut network_type.fields {
        if field.serialize.is_some() {
            continue;
        }
        let Some((item, confidence, source, address)) =
            serialize_field_match(field, type_index, selected_value_types, report)
        else {
            continue;
        };
        report.matched_field_type_count += 1;
        field.serialize = Some(network_serialize_field_type(
            item,
            type_index,
            source.clone(),
            confidence,
        ));
        field.evidence.push(NetworkEvidence {
            kind: NetworkEvidenceKind::SerializeContext,
            source,
            address,
            detail: Some(item.source_name.clone()),
            confidence,
        });
    }
}

pub(super) fn serialize_field_match<'a>(
    field: &NetworkField,
    type_index: &'a SerializeCodegenIndex<'a>,
    selected_value_types: &BTreeMap<String, NetworkNativeTypeInfoEvidence>,
    report: &mut NetworkSerializeMergeReport,
) -> Option<(
    &'a SerializeCodegenItem,
    NetworkConfidence,
    String,
    Option<String>,
)> {
    if let Some(value_type) = field
        .handler_vtable
        .as_deref()
        .and_then(|handler_vtable| selected_value_types.get(handler_vtable))
        && let Some(type_id) = value_type.type_id
        && !type_id.is_nil()
        && let Some(item) = type_index.item_by_type_id(type_id)
    {
        report.field_type_id_matched_count += 1;
        return Some((
            item,
            NetworkConfidence::High,
            "serializeContext:handler-value-type-id".to_owned(),
            value_type.address.clone(),
        ));
    }

    let exact_field_type_id = field
        .source_type_id
        .filter(|_| field.source_type_identity_proven)
        .or_else(|| {
            field
                .nested_type_shape
                .as_ref()
                .filter(|shape| shape.has_exact_identity())
                .and_then(|shape| shape.type_id)
        });
    if let Some(type_id) = exact_field_type_id
        && !type_id.is_nil()
        && let Some(item) = type_index.item_by_type_id(type_id)
    {
        report.field_type_id_matched_count += 1;
        return Some((
            item,
            NetworkConfidence::Exact,
            "serializeContext:field-type-id".to_owned(),
            None,
        ));
    }

    None
}

pub(super) fn selected_value_type_info_by_handler_vtable(
    vtables: &[NetworkFieldHandlerVtable],
) -> BTreeMap<String, NetworkNativeTypeInfoEvidence> {
    vtables
        .iter()
        .filter_map(|vtable| {
            let address = vtable.address.clone()?;
            let value_type = vtable.value_type_info.as_ref()?;
            let constructed = value_type.source.as_deref().is_some_and(|source| {
                source.starts_with("unmarshal-full-element-vptr+native-size")
                    && value_type.native_size.is_some()
            });
            let shape_validated = vtable.value_type_shape.as_ref().is_some_and(|shape| {
                shape.has_exact_identity() && shape.type_id == value_type.type_id
            });
            (constructed || shape_validated).then(|| (address, value_type.clone()))
        })
        .collect()
}

pub(super) fn message_signature_candidates(
    types: &[NetworkType],
    signature: &NetworkMessageSignature,
) -> Vec<usize> {
    if let Some(type_id) = signature.type_id {
        let matches = types
            .iter()
            .enumerate()
            .filter_map(|(index, network_type)| {
                (network_type.type_id == Some(type_id)).then_some(index)
            })
            .collect::<Vec<_>>();
        if !matches.is_empty() {
            return matches;
        }
    }

    if let Some(type_index) = signature.type_index {
        let matches = types
            .iter()
            .enumerate()
            .filter_map(|(index, network_type)| {
                (network_type.type_index == Some(type_index)).then_some(index)
            })
            .collect::<Vec<_>>();
        if !matches.is_empty() {
            return matches;
        }
    }

    if let Some(name) = signature.name.as_deref() {
        let matches = types
            .iter()
            .enumerate()
            .filter_map(|(index, network_type)| {
                network_type
                    .name
                    .as_deref()
                    .is_some_and(|network_name| {
                        network_name == name || type_leaf_name(network_name) == name
                    })
                    .then_some(index)
            })
            .collect::<Vec<_>>();
        if !matches.is_empty() {
            return matches;
        }
    }

    if let Some(rust_name) = signature.rust_name.as_deref() {
        return types
            .iter()
            .enumerate()
            .filter_map(|(index, network_type)| {
                network_type
                    .name
                    .as_deref()
                    .is_some_and(|network_name| type_leaf_name(network_name) == rust_name)
                    .then_some(index)
            })
            .collect();
    }

    Vec::new()
}

pub(super) fn type_leaf_name(name: &str) -> &str {
    name.rsplit("::").next().unwrap_or(name)
}

pub(super) fn is_placeholder_field_name(value: &str) -> bool {
    value
        .strip_prefix("field_")
        .is_some_and(|suffix| !suffix.is_empty() && suffix.chars().all(|c| c.is_ascii_digit()))
}

pub(super) fn field_has_native_type_name(field: &NetworkField) -> bool {
    field.evidence.iter().any(|evidence| {
        evidence.kind == NetworkEvidenceKind::MessageSource
            && evidence.source == "message-native-type-name"
    })
}

pub(super) fn network_serialize_type(
    item: &SerializeCodegenItem,
    index: &SerializeCodegenIndex<'_>,
) -> NetworkSerializeType {
    let mut direct_dependency_type_ids = item
        .direct_dependency_type_ids()
        .into_iter()
        .collect::<Vec<_>>();
    direct_dependency_type_ids.sort_unstable();
    NetworkSerializeType {
        type_id: item.source_type_id,
        kind: network_serialize_kind(item.kind),
        name: item.source_name.clone(),
        role: network_serialize_role(item.role),
        resolved_type: None,
        emits_source: true,
        factory: item.factory.clone(),
        field_count: item.fields.len(),
        fields: item
            .fields
            .iter()
            .map(|field| NetworkSerializeField {
                name: field.source_name.clone(),
                type_id: field.source_type_id,
                resolved_type: field.resolved_type.clone(),
                offset: field.offset,
                is_base_class: field.is_base_class,
            })
            .collect(),
        variant_count: item.variants.len(),
        direct_dependency_type_ids,
        wire_shapes: serialize_item_wire_shapes(item, index).unwrap_or_default(),
        is_abstract: item.is_abstract,
        is_reflection_marker: item.is_reflection_marker,
    }
}

pub(super) fn network_serialize_generic_type(
    generic: &crate::catalog::ReflectedGenericType,
) -> NetworkSerializeType {
    let mut direct_dependency_type_ids = BTreeSet::new();
    collect_resolved_named_type_ids(&generic.resolved_type, &mut direct_dependency_type_ids);
    NetworkSerializeType {
        type_id: generic.type_id,
        kind: NetworkSerializeKind::Struct,
        name: generic.display_name.clone(),
        role: NetworkSerializeRole::SupportType,
        resolved_type: Some(generic.resolved_type.clone()),
        emits_source: false,
        factory: None,
        field_count: 0,
        fields: Vec::new(),
        variant_count: 0,
        direct_dependency_type_ids: direct_dependency_type_ids.into_iter().collect(),
        wire_shapes: Vec::new(),
        is_abstract: Some(false),
        is_reflection_marker: false,
    }
}

pub(super) fn network_serialize_field_type(
    item: &SerializeCodegenItem,
    index: &SerializeCodegenIndex<'_>,
    source: String,
    confidence: NetworkConfidence,
) -> NetworkSerializeFieldType {
    let mut direct_dependency_type_ids = item
        .direct_dependency_type_ids()
        .into_iter()
        .collect::<Vec<_>>();
    direct_dependency_type_ids.sort_unstable();
    NetworkSerializeFieldType {
        type_id: item.source_type_id,
        kind: network_serialize_kind(item.kind),
        name: item.source_name.clone(),
        role: network_serialize_role(item.role),
        field_count: item.fields.len(),
        variant_count: item.variants.len(),
        direct_dependency_type_ids,
        wire_shapes: serialize_item_wire_shapes(item, index).unwrap_or_default(),
        source,
        confidence,
    }
}

pub(super) fn serialize_item_wire_shapes(
    item: &SerializeCodegenItem,
    index: &SerializeCodegenIndex<'_>,
) -> Option<Vec<NetworkWireScalarShape>> {
    serialize_item_wire_shapes_with_seen(item, index, &mut BTreeSet::new())
}

pub(super) fn serialize_item_wire_shapes_with_seen(
    item: &SerializeCodegenItem,
    index: &SerializeCodegenIndex<'_>,
    seen: &mut BTreeSet<Uuid>,
) -> Option<Vec<NetworkWireScalarShape>> {
    if !seen.insert(item.source_type_id) {
        return None;
    }
    match item.kind {
        SerializeCodegenItemKind::Struct => {
            let mut shapes = Vec::new();
            for field in item.fields.iter().filter(|field| !field.is_base_class) {
                shapes.extend(resolved_type_wire_shapes(
                    &field.resolved_type,
                    index,
                    seen,
                )?);
            }
            seen.remove(&item.source_type_id);
            Some(shapes)
        }
        SerializeCodegenItemKind::Enum => {
            let shapes = item
                .enum_underlying_type
                .as_ref()
                .and_then(|underlying| resolved_type_wire_shapes(underlying, index, seen));
            seen.remove(&item.source_type_id);
            shapes
        }
    }
}

pub(super) fn resolved_type_wire_shapes(
    resolved: &ResolvedType,
    index: &SerializeCodegenIndex<'_>,
    seen: &mut BTreeSet<Uuid>,
) -> Option<Vec<NetworkWireScalarShape>> {
    match resolved {
        ResolvedType::Scalar(scalar) => scalar_wire_shape(*scalar).map(|shape| vec![shape]),
        ResolvedType::Named { type_id, .. } => {
            let item = index.item_by_type_id(*type_id)?;
            serialize_item_wire_shapes_with_seen(item, index, seen)
        }
        ResolvedType::RangedInteger { value, .. } => resolved_type_wire_shapes(value, index, seen),
        ResolvedType::Tuple { elements } => {
            let mut shapes = Vec::new();
            for element in elements {
                shapes.extend(resolved_type_wire_shapes(element, index, seen)?);
            }
            Some(shapes)
        }
        ResolvedType::Sequence { .. }
        | ResolvedType::Map { .. }
        | ResolvedType::Asset { .. }
        | ResolvedType::Uid { .. }
        | ResolvedType::ReplicatedField { .. }
        | ResolvedType::ByteStream
        | ResolvedType::Pair { .. }
        | ResolvedType::Pointer { .. }
        | ResolvedType::Optional { .. }
        | ResolvedType::Unknown { .. } => None,
    }
}

pub(super) const fn scalar_wire_shape(scalar: ScalarType) -> Option<NetworkWireScalarShape> {
    match scalar {
        ScalarType::Char | ScalarType::SignedChar | ScalarType::I8 | ScalarType::U8 => {
            Some(NetworkWireScalarShape::U8)
        }
        ScalarType::I16 | ScalarType::U16 => Some(NetworkWireScalarShape::U16),
        ScalarType::I32 | ScalarType::U32 | ScalarType::Crc32 => Some(NetworkWireScalarShape::U32),
        ScalarType::I64 | ScalarType::U64 | ScalarType::UnsignedLong | ScalarType::EntityId => {
            Some(NetworkWireScalarShape::U64)
        }
        ScalarType::F32 => Some(NetworkWireScalarShape::F32),
        ScalarType::F64 => Some(NetworkWireScalarShape::F64),
        ScalarType::Bool => Some(NetworkWireScalarShape::Bool),
        ScalarType::Uuid => Some(NetworkWireScalarShape::FixedBytes(16)),
        ScalarType::Vector2 => Some(NetworkWireScalarShape::Vec2),
        ScalarType::Vector3 => Some(NetworkWireScalarShape::Vec3),
        ScalarType::Vector4 => Some(NetworkWireScalarShape::Vec4),
        ScalarType::Quaternion => Some(NetworkWireScalarShape::Quat),
        ScalarType::Transform => Some(NetworkWireScalarShape::Affine3),
        ScalarType::String => Some(NetworkWireScalarShape::String),
        ScalarType::AssetId | ScalarType::Color | ScalarType::ColorF | ScalarType::ColorB => None,
    }
}

pub(super) const fn network_serialize_kind(kind: SerializeCodegenItemKind) -> NetworkSerializeKind {
    match kind {
        SerializeCodegenItemKind::Struct => NetworkSerializeKind::Struct,
        SerializeCodegenItemKind::Enum => NetworkSerializeKind::Enum,
    }
}

pub(super) const fn network_serialize_role(role: ReflectedTypeRole) -> NetworkSerializeRole {
    match role {
        ReflectedTypeRole::FacetedComponent => NetworkSerializeRole::FacetedComponent,
        ReflectedTypeRole::AzComponent => NetworkSerializeRole::AzComponent,
        ReflectedTypeRole::ClientFacet => NetworkSerializeRole::ClientFacet,
        ReflectedTypeRole::ServerFacet => NetworkSerializeRole::ServerFacet,
        ReflectedTypeRole::AzEntity => NetworkSerializeRole::AzEntity,
        ReflectedTypeRole::SupportType => NetworkSerializeRole::SupportType,
    }
}
