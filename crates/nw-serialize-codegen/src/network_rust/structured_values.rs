use super::*;

#[derive(Debug, Clone)]
pub(super) struct StructuredValueFieldPlan {
    pub(super) value_type: String,
    pub(super) field_type: String,
    pub(super) shape: crate::network_schema::NetworkNestedTypeShape,
    pub(super) embedded_shapes: Vec<crate::network_schema::NetworkNestedTypeShape>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum StructuredValuePlanError {
    MissingSerializeIdentity,
    TypeIdentityMismatch,
    UnprovenMemberNames,
    UnprovenMemberCoverage,
    UnprovenWireOrder,
    MissingMembers,
    InvalidMemberNames,
    MissingSourceType,
    UnsupportedMember,
}

impl StructuredValuePlanError {
    pub(super) const fn as_str(self) -> &'static str {
        match self {
            Self::MissingSerializeIdentity => "missing-structured-value-identity",
            Self::TypeIdentityMismatch => "structured-value-identity-mismatch",
            Self::UnprovenMemberNames => "unproven-structured-value-member-names",
            Self::UnprovenMemberCoverage => "unproven-structured-value-member-coverage",
            Self::UnprovenWireOrder => "unproven-structured-value-wire-order",
            Self::MissingMembers => "missing-structured-value-members",
            Self::InvalidMemberNames => "invalid-structured-value-member-names",
            Self::MissingSourceType => "missing-structured-value-source-type",
            Self::UnsupportedMember => "unsupported-structured-value-member",
        }
    }
}

pub(super) fn structured_value_field_plan(
    field: &NetworkField,
    vtable: Option<&NetworkFieldHandlerVtable>,
    wire_shape: Option<&SchemaWireShape>,
    serialize_types: &BTreeMap<Uuid, &NetworkSerializeType>,
) -> Option<Result<StructuredValueFieldPlan, StructuredValuePlanError>> {
    let shape = field
        .nested_type_shape
        .as_ref()
        .filter(|shape| shape.has_exact_identity() || shape.has_proven_anonymous_layout())
        .or_else(|| {
            vtable
                .and_then(|vtable| vtable.value_type_shape.as_ref())
                .filter(|shape| shape.has_exact_identity() || shape.has_proven_anonymous_layout())
        })?
        .clone();
    let shape = reconcile_serialize_backed_member_names(shape, serialize_types);
    if anonymous_scalar_passthrough(&shape, wire_shape) {
        return None;
    }
    Some(build_structured_value_field_plan(
        field,
        &shape,
        vtable.map_or(&[], |vtable| &vtable.embedded_value_type_shapes),
        serialize_types,
    ))
}

fn reconcile_serialize_backed_member_names(
    shape: crate::network_schema::NetworkNestedTypeShape,
    serialize_types: &BTreeMap<Uuid, &NetworkSerializeType>,
) -> crate::network_schema::NetworkNestedTypeShape {
    let original = shape.clone();
    try_reconcile_serialize_backed_member_names(shape, serialize_types).unwrap_or(original)
}

fn try_reconcile_serialize_backed_member_names(
    mut shape: crate::network_schema::NetworkNestedTypeShape,
    serialize_types: &BTreeMap<Uuid, &NetworkSerializeType>,
) -> Option<crate::network_schema::NetworkNestedTypeShape> {
    if !shape.has_exact_identity()
        || shape.member_names_proven == Some(true)
        || shape.member_coverage_proven != Some(true)
        || shape.wire_order_proven != Some(true)
    {
        return Some(shape);
    }
    let Some(source) = shape
        .type_id
        .and_then(|type_id| serialize_types.get(&type_id))
    else {
        return Some(shape);
    };
    let source_fields = source
        .fields
        .iter()
        .filter(|field| !field.is_base_class)
        .collect::<Vec<_>>();
    if source_fields.is_empty() {
        return Some(shape);
    }

    let complete_source_product = source_fields
        .iter()
        .map(|field| {
            let wire_shape = resolved_type_wire_shape(
                &field.resolved_type,
                serialize_types,
                &mut BTreeSet::new(),
            )?;
            nested_member_wire_shapes(&wire_shape, &[])
        })
        .collect::<Option<Vec<_>>>()
        .map(|products| products.into_iter().flatten().collect::<Vec<_>>());
    if let [observed] = shape.members.as_slice()
        && source_fields.len() > 1
        && complete_source_product.is_some()
        && nested_member_wire_shapes(
            observed
                .wire_shape
                .as_deref()
                .or(observed.wire_layout.as_deref())?,
            &[],
        ) == complete_source_product
    {
        shape.members = source_fields
            .iter()
            .enumerate()
            .map(|(ordinal, field)| {
                let wire_shape = resolved_type_wire_shape(
                    &field.resolved_type,
                    serialize_types,
                    &mut BTreeSet::new(),
                )?;
                let offset = field.offset.map(|offset| format!("0x{offset:x}"));
                Some(crate::network_schema::NetworkNestedTypeMember {
                    index: u32::try_from(ordinal).ok(),
                    offset: offset.clone(),
                    native_offset: offset,
                    name: Some(field.name.clone()),
                    name_source: Some("serialize-field-wire-product-reconciliation".to_owned()),
                    name_proven: Some(true),
                    name_evidence: Some("exact-type-id+complete-wire-product".to_owned()),
                    native_type: None,
                    type_id: Some(field.type_id),
                    type_id_source: Some("serialize-field-wire-product-reconciliation".to_owned()),
                    type_identity_proven: true,
                    type_identity_source: Some(
                        "serialize-field-wire-product-reconciliation".to_owned(),
                    ),
                    wire_shape: Some(wire_shape.clone()),
                    wire_shape_source: Some(
                        "serialize-field-wire-product-reconciliation".to_owned(),
                    ),
                    wire_layout: None,
                    wire_layout_source: None,
                    byte_width: None,
                    wire_ordinal: u32::try_from(ordinal).ok(),
                    wire_order_source: Some(
                        "serialize-field-wire-product-reconciliation".to_owned(),
                    ),
                    callsite: None,
                    target: None,
                    target_name: None,
                    type_conflict: false,
                })
            })
            .collect::<Option<Vec<_>>>()?;
        shape.member_names_proven = Some(true);
        shape.member_name_source = Some("serialize-field-wire-product-reconciliation".to_owned());
        shape.validation = Some(append_validation_marker(
            shape.validation.as_deref(),
            "serialize-complete-wire-product",
        ));
        return Some(shape);
    }

    let mut reconciled = Vec::with_capacity(shape.members.len());
    for mut member in shape.members.clone() {
        let offset = member
            .native_offset
            .as_deref()
            .or(member.offset.as_deref())
            .and_then(parse_native_member_offset)?;
        let observed_product = member
            .wire_shape
            .as_deref()
            .or(member.wire_layout.as_deref())
            .and_then(|wire_shape| nested_member_wire_shapes(wire_shape, &[]));
        let mut matches = source_fields
            .iter()
            .copied()
            .filter(|field| field.offset == Some(offset));
        let field = matches.next()?;
        if matches.next().is_some() {
            return None;
        }
        let source_wire_shape =
            resolved_type_wire_shape(&field.resolved_type, serialize_types, &mut BTreeSet::new())
                .filter(|source_wire_shape| {
                    nested_member_wire_shapes(source_wire_shape, &[]) == observed_product
                });
        member.name = Some(field.name.clone());
        member.name_source = Some(source_wire_shape.as_ref().map_or_else(
            || "serialize-field-offset-reconciliation".to_owned(),
            |_| "serialize-field-offset-wire-reconciliation".to_owned(),
        ));
        member.name_proven = Some(true);
        member.name_evidence = Some(source_wire_shape.as_ref().map_or_else(
            || "exact-type-id+native-offset".to_owned(),
            |_| "exact-type-id+native-offset+wire-product".to_owned(),
        ));
        member.type_id = Some(field.type_id);
        member.type_id_source = member.name_source.clone();
        member.type_identity_proven = true;
        member.type_identity_source = member.name_source.clone();
        if let Some(source_wire_shape) = source_wire_shape {
            member.wire_shape = Some(source_wire_shape);
            member.wire_shape_source =
                Some("serialize-field-offset-wire-reconciliation".to_owned());
        }
        reconciled.push(member);
    }
    shape.members = reconciled;
    shape.member_names_proven = Some(true);
    shape.member_name_source = Some("serialize-field-offset-wire-reconciliation".to_owned());
    shape.validation = Some(append_validation_marker(
        shape.validation.as_deref(),
        "serialize-offset-wire-product",
    ));
    Some(shape)
}

fn append_validation_marker(existing: Option<&str>, marker: &str) -> String {
    existing.map_or_else(
        || marker.to_owned(),
        |existing| format!("{existing}+{marker}"),
    )
}

fn resolved_type_wire_shape(
    resolved: &ResolvedType,
    serialize_types: &BTreeMap<Uuid, &NetworkSerializeType>,
    active: &mut BTreeSet<Uuid>,
) -> Option<String> {
    match resolved {
        ResolvedType::Scalar(scalar) => scalar_resolved_wire_shape(*scalar).map(str::to_owned),
        ResolvedType::Named { type_id, .. } => {
            let serialize = serialize_types.get(type_id)?;
            if !serialize.wire_shapes.is_empty() {
                return Some(wire_product_shape(
                    serialize
                        .wire_shapes
                        .iter()
                        .map(|shape| shape.wire_string()),
                ));
            }
            if !active.insert(*type_id) {
                return None;
            }
            let result = match serialize.resolved_type.as_ref() {
                None => {
                    let members = serialize
                        .fields
                        .iter()
                        .filter(|field| !field.is_base_class)
                        .map(|field| {
                            resolved_type_wire_shape(&field.resolved_type, serialize_types, active)
                        })
                        .collect::<Option<Vec<_>>>()?;
                    (!members.is_empty()).then(|| wire_product_shape(members))
                }
                Some(resolved) => resolved_type_wire_shape(resolved, serialize_types, active),
            };
            active.remove(type_id);
            result
        }
        ResolvedType::Sequence {
            kind,
            element,
            capacity,
        } => {
            let element = resolved_type_wire_shape(element, serialize_types, active)?;
            match kind {
                crate::types::SequenceKind::Array => {
                    Some(format!("fixed-array<{element},{}>", capacity.as_ref()?))
                }
                crate::types::SequenceKind::Set | crate::types::SequenceKind::UnorderedSet => {
                    Some(format!("set<{element}>"))
                }
                crate::types::SequenceKind::BitSet => None,
                crate::types::SequenceKind::FixedVector => {
                    Some(format!("fixed-vector<{element},{}>", capacity.as_ref()?))
                }
                crate::types::SequenceKind::Vector
                | crate::types::SequenceKind::List
                | crate::types::SequenceKind::ForwardList => Some(format!("vec<{element}>")),
            }
        }
        ResolvedType::Map { key, value, .. } => Some(format!(
            "map<{},{}>",
            resolved_type_wire_shape(key, serialize_types, active)?,
            resolved_type_wire_shape(value, serialize_types, active)?
        )),
        ResolvedType::Pair { first, second } => Some(format!(
            "composite<{},{}>",
            resolved_type_wire_shape(first, serialize_types, active)?,
            resolved_type_wire_shape(second, serialize_types, active)?
        )),
        ResolvedType::Tuple { elements } => Some(wire_product_shape(
            elements
                .iter()
                .map(|element| resolved_type_wire_shape(element, serialize_types, active))
                .collect::<Option<Vec<_>>>()?,
        )),
        ResolvedType::Optional { value } => Some(format!(
            "optional<{}>",
            resolved_type_wire_shape(value, serialize_types, active)?
        )),
        ResolvedType::RangedInteger { value, .. } | ResolvedType::ReplicatedField { value } => {
            resolved_type_wire_shape(value, serialize_types, active)
        }
        ResolvedType::Pointer { target, .. } => {
            resolved_type_wire_shape(target, serialize_types, active)
        }
        ResolvedType::Uid { .. } => Some("fixed-bytes-16".to_owned()),
        ResolvedType::Asset { .. } | ResolvedType::ByteStream | ResolvedType::Unknown { .. } => {
            None
        }
    }
}

fn wire_product_shape(parts: impl IntoIterator<Item = String>) -> String {
    let parts = parts.into_iter().collect::<Vec<_>>();
    match parts.as_slice() {
        [single] => single.clone(),
        _ => format!("composite<{}>", parts.join(",")),
    }
}

const fn scalar_resolved_wire_shape(scalar: ScalarType) -> Option<&'static str> {
    match scalar {
        ScalarType::Char | ScalarType::SignedChar | ScalarType::I8 | ScalarType::U8 => Some("u8"),
        ScalarType::I16 | ScalarType::U16 => Some("u16"),
        ScalarType::I32 | ScalarType::U32 | ScalarType::Crc32 => Some("u32"),
        ScalarType::I64 | ScalarType::U64 | ScalarType::UnsignedLong | ScalarType::EntityId => {
            Some("u64")
        }
        ScalarType::F32 => Some("f32"),
        ScalarType::F64 => Some("f64"),
        ScalarType::Bool => Some("bool"),
        ScalarType::Uuid => Some("fixed-bytes-16"),
        ScalarType::Vector2 => Some("vec2"),
        ScalarType::Vector3 => Some("vec3"),
        ScalarType::Vector4 => Some("vec4"),
        ScalarType::Quaternion => Some("quat"),
        ScalarType::Transform => Some("affine3"),
        ScalarType::String => Some("string"),
        ScalarType::AssetId | ScalarType::Color | ScalarType::ColorF | ScalarType::ColorB => None,
    }
}

fn anonymous_scalar_passthrough(
    shape: &crate::network_schema::NetworkNestedTypeShape,
    wire_shape: Option<&SchemaWireShape>,
) -> bool {
    if !shape.has_exact_identity() && !shape.has_proven_anonymous_layout() {
        return false;
    }
    let [member] = shape.members.as_slice() else {
        return false;
    };
    let Some(NetworkMemberWireShape::Scalar(member_shape)) = member
        .wire_shape
        .as_deref()
        .or(member.wire_layout.as_deref())
        .and_then(parse_network_member_wire_shape)
    else {
        return false;
    };
    if wire_shape != Some(&member_shape.into()) {
        return false;
    }
    if !shape.has_exact_identity() {
        return true;
    }
    shape
        .type_id
        .and_then(exact_type_id_rust_type)
        .is_some_and(|identity| identity == scalar_rust_type(member_shape))
}

pub(super) fn exact_serialize_value_rust_type(
    field: &NetworkField,
    vtable: Option<&NetworkFieldHandlerVtable>,
    wire_shape: Option<&SchemaWireShape>,
    serialize_types: &BTreeMap<Uuid, &NetworkSerializeType>,
) -> Option<String> {
    let shape = field
        .nested_type_shape
        .as_ref()
        .filter(|shape| shape.has_exact_identity())
        .or_else(|| {
            vtable
                .and_then(|vtable| vtable.value_type_shape.as_ref())
                .filter(|shape| shape.has_exact_identity())
        })?;
    let type_id = shape.type_id?;
    let field_type_id = field
        .serialize
        .as_ref()
        .map(|serialize| serialize.type_id)
        .or_else(|| {
            field
                .source_type_identity_proven
                .then_some(field.source_type_id)
                .flatten()
        })?;
    if type_id != field_type_id {
        return None;
    }

    let serialize = serialize_types.get(&type_id)?;
    if !serialize.emits_source || serialize.wire_shapes.is_empty() {
        return None;
    }
    let observed = wire_shape
        .and_then(crate::network_schema::parse::wire_shape_scalar_product)
        .or_else(|| crate::network_schema::parse::nested_type_shape_wire_shapes(shape, &[]))?;
    if !wire_scalar_shapes_match(&observed, &serialize.wire_shapes) {
        return None;
    }

    network_serialize_type_rust_type(serialize, serialize_types)
}

fn build_structured_value_field_plan(
    field: &NetworkField,
    shape: &crate::network_schema::NetworkNestedTypeShape,
    embedded_shapes: &[crate::network_schema::NetworkNestedTypeShape],
    serialize_types: &BTreeMap<Uuid, &NetworkSerializeType>,
) -> Result<StructuredValueFieldPlan, StructuredValuePlanError> {
    if shape.has_exact_identity() {
        let shape_type_id = shape
            .type_id
            .ok_or(StructuredValuePlanError::MissingSerializeIdentity)?;
        let field_type_id = field
            .serialize
            .as_ref()
            .map(|serialize| serialize.type_id)
            .or_else(|| {
                field
                    .source_type_identity_proven
                    .then_some(field.source_type_id)
                    .flatten()
            });
        if field_type_id.is_some_and(|field_type_id| field_type_id != shape_type_id) {
            return Err(StructuredValuePlanError::TypeIdentityMismatch);
        }
    } else if !shape.has_proven_anonymous_layout() {
        return Err(StructuredValuePlanError::MissingSerializeIdentity);
    }
    let uses_source_type = container_value_shape_uses_source_type(shape, serialize_types);
    if uses_source_type
        && (shape.member_names_proven != Some(true)
            || shape
                .members
                .iter()
                .any(|member| member.name_proven != Some(true)))
    {
        return Err(StructuredValuePlanError::UnprovenMemberNames);
    }
    if shape.member_coverage_proven != Some(true) {
        return Err(StructuredValuePlanError::UnprovenMemberCoverage);
    }
    if nested_type_shape_members_in_wire_order(shape).is_none() {
        return Err(StructuredValuePlanError::UnprovenWireOrder);
    }
    if shape.members.is_empty() {
        return Err(StructuredValuePlanError::MissingMembers);
    }
    if !container_value_shape_member_names_are_emittable(shape, serialize_types) {
        return Err(StructuredValuePlanError::InvalidMemberNames);
    }
    if !container_value_shape_members_are_emittable(shape, embedded_shapes, serialize_types) {
        return Err(StructuredValuePlanError::UnsupportedMember);
    }

    let value_type = container_value_shape_rust_type(field, shape, serialize_types)
        .ok_or(StructuredValuePlanError::MissingSourceType)?;
    let codec_name = structured_value_codec_name(
        field
            .name
            .as_deref()
            .ok_or(StructuredValuePlanError::MissingSourceType)?,
        shape,
    )
    .ok_or(StructuredValuePlanError::MissingSourceType)?;
    let field_type =
        format!("::nw_network::serialize::ReplicatedFieldHandler<{value_type}, {codec_name}>");
    Ok(StructuredValueFieldPlan {
        value_type,
        field_type,
        shape: shape.clone(),
        embedded_shapes: embedded_shapes.to_vec(),
    })
}

pub(super) fn structured_value_codec_name(
    field_name: &str,
    shape: &crate::network_schema::NetworkNestedTypeShape,
) -> Option<String> {
    let type_name = shape
        .type_name_full
        .as_deref()
        .or(shape.type_name.as_deref())?;
    if shape.has_exact_identity() {
        return Some(format!("{}Marshaler", rust_type_ident(type_name)));
    }
    Some(format!(
        "{}{}Marshaler",
        rust_type_ident(&rust_field_ident(field_name)),
        rust_type_ident(type_name)
    ))
}

#[cfg(test)]
mod tests;
