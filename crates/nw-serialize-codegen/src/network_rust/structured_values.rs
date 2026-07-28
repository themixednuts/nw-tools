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
        })?;
    if anonymous_scalar_passthrough(shape, wire_shape) {
        return None;
    }
    Some(build_structured_value_field_plan(
        field,
        shape,
        vtable.map_or(&[], |vtable| &vtable.embedded_value_type_shapes),
        serialize_types,
    ))
}

fn anonymous_scalar_passthrough(
    shape: &crate::network_schema::NetworkNestedTypeShape,
    wire_shape: Option<&SchemaWireShape>,
) -> bool {
    if shape.has_exact_identity() || !shape.has_proven_anonymous_layout() {
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
    wire_shape == Some(&member_shape.into())
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
