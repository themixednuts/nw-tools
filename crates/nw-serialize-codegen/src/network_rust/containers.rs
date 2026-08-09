use super::*;

pub(super) fn replicated_container_vtable_for_field<'a>(
    field: &NetworkField,
    handler_vtables: &'a BTreeMap<&str, &NetworkFieldHandlerVtable>,
) -> Option<&'a NetworkFieldHandlerVtable> {
    field
        .handler_vtable
        .as_deref()
        .and_then(|address| handler_vtables.get(address).copied())
        .filter(|vtable| {
            vtable.full_container_plan.is_some() || vtable.handler_container_type.is_some()
        })
}

pub(super) fn replicated_container_semantic_field_shape(
    field: &NetworkField,
    vtable: &NetworkFieldHandlerVtable,
    wire_shape: Option<&SchemaWireShape>,
    serialize_types: &BTreeMap<Uuid, &NetworkSerializeType>,
) -> Result<RustFieldShape, ContainerPlanError> {
    let plan_error = if let Some(plan) = vtable.full_container_plan.as_ref() {
        match replicated_container_plan_field_shape(field, vtable, plan, serialize_types) {
            Ok(shape) => return Ok(shape),
            Err(ContainerPlanError::NonLinearCodec) => {
                return Err(ContainerPlanError::NonLinearCodec);
            }
            Err(error) => Some(error),
        }
    } else {
        None
    };

    match handler_container_field_shape(vtable, wire_shape) {
        Ok(shape) => Ok(shape),
        Err(error) => Err(plan_error.unwrap_or(error)),
    }
}

fn replicated_container_plan_field_shape(
    field: &NetworkField,
    vtable: &NetworkFieldHandlerVtable,
    plan: &NetworkReplicatedContainerPlan,
    serialize_types: &BTreeMap<Uuid, &NetworkSerializeType>,
) -> Result<RustFieldShape, ContainerPlanError> {
    if plan.has_non_linear_key_codec() {
        return Err(ContainerPlanError::NonLinearCodec);
    }

    let proven_value = container_value_type_from_proven_shape(field, vtable, plan, serialize_types);
    if plan.has_non_linear_value_codec()
        && proven_value.is_none()
        && plan.default_profile_value_wire_shapes().is_none()
    {
        return Err(ContainerPlanError::NonLinearCodec);
    }
    let value = proven_value
        .or_else(|| container_value_type(field, plan, serialize_types))
        .ok_or(ContainerPlanError::MissingValueType)?;
    let key = match plan.storage {
        NetworkReplicatedContainerStorageKind::Map => Some(
            container_key_type(field, plan, serialize_types)
                .ok_or(ContainerPlanError::MissingKeyType)?,
        ),
        NetworkReplicatedContainerStorageKind::Vec => None,
    };
    let collection_type = match plan.storage {
        NetworkReplicatedContainerStorageKind::Map => {
            index_map_type(&key.as_ref().unwrap().rust_type, &value.rust_type)
        }
        NetworkReplicatedContainerStorageKind::Vec => {
            format!("::std::vec::Vec<{}>", value.rust_type)
        }
    };
    let key_marshaler = key.as_ref().map_or_else(
        || "::nw_network::serialize::DefaultMarshaler<::nw_network::serialize::VlqU64>".to_owned(),
        |key| key.marshaler_type.clone(),
    );
    let field_type = format!(
        "::nw_network::serialize::ReplicatedContainer<{collection_type}, {{ ::nw_network::serialize::WIRE_VEC_CAP }}, {key_marshaler}, {}>",
        value.marshaler_type
    );

    Ok(RustFieldShape {
        value_type: collection_type,
        field_type,
        container_key_type_shape: key.as_ref().and_then(|key| key.value_type_shape.clone()),
        container_embedded_key_type_shapes: key
            .as_ref()
            .map(|key| key.embedded_value_type_shapes.clone())
            .unwrap_or_default(),
        container_value_type_shape: value.value_type_shape,
        container_embedded_value_type_shapes: value.embedded_value_type_shapes,
    })
}

fn handler_container_field_shape(
    vtable: &NetworkFieldHandlerVtable,
    wire_shape: Option<&SchemaWireShape>,
) -> Result<RustFieldShape, ContainerPlanError> {
    let handler = vtable
        .handler_container_type
        .as_ref()
        .ok_or(ContainerPlanError::MissingPlan)?;
    if handler.storage_kind != NetworkReplicatedContainerStorageKind::Map {
        return Err(ContainerPlanError::MissingPlan);
    }
    let container = wire_shape
        .or(vtable.wire_shape.as_ref())
        .and_then(|shape| match shape {
            SchemaWireShape::ReplicatedContainer(container) => Some(*container),
            _ => None,
        })
        .ok_or(ContainerPlanError::MissingPlan)?;
    let key = network_native_scalar_type(
        handler
            .key_native_type
            .as_deref()
            .ok_or(ContainerPlanError::MissingKeyType)?,
    )
    .ok_or(ContainerPlanError::MissingKeyType)?;
    let value = network_native_scalar_type(&handler.value_native_type)
        .ok_or(ContainerPlanError::MissingValueType)?;
    if !container_native_shape_matches(container.key, key.wire_shape) {
        return Err(ContainerPlanError::MissingKeyType);
    }
    if !container_native_shape_matches(container.value, value.wire_shape) {
        return Err(ContainerPlanError::MissingValueType);
    }

    let collection_type = index_map_type(key.rust_type, value.rust_type);
    let key_marshaler = scalar_marshaler_type_for_value(container.key, key.rust_type);
    let value_marshaler = scalar_marshaler_type_for_value(container.value, value.rust_type);
    let field_type = format!(
        "::nw_network::serialize::ReplicatedContainer<{collection_type}, {{ ::nw_network::serialize::WIRE_VEC_CAP }}, {key_marshaler}, {value_marshaler}>"
    );
    Ok(RustFieldShape {
        value_type: collection_type,
        field_type,
        container_key_type_shape: None,
        container_embedded_key_type_shapes: Vec::new(),
        container_value_type_shape: None,
        container_embedded_value_type_shapes: Vec::new(),
    })
}

fn container_native_shape_matches(
    observed: SchemaWireScalarShape,
    native: SchemaWireScalarShape,
) -> bool {
    observed == native
        || matches!(
            (observed, native),
            (SchemaWireScalarShape::Bool, SchemaWireScalarShape::U8)
                | (SchemaWireScalarShape::U8, SchemaWireScalarShape::Bool)
        )
}

#[derive(Debug, Clone)]
pub(super) struct ContainerValueType {
    rust_type: String,
    marshaler_type: String,
    value_type_shape: Option<crate::network_schema::NetworkNestedTypeShape>,
    embedded_value_type_shapes: Vec<crate::network_schema::NetworkNestedTypeShape>,
}

pub(super) fn container_key_type(
    field: &NetworkField,
    plan: &NetworkReplicatedContainerPlan,
    serialize_types: &BTreeMap<Uuid, &NetworkSerializeType>,
) -> Option<ContainerValueType> {
    container_codec_value_type(&plan.key_codecs, serialize_types).or_else(|| {
        wire_sequence_container_value_type(
            field,
            "Key",
            &plan.exact_key_wire_shapes()?,
            serialize_types,
        )
    })
}

pub(super) fn container_value_type(
    field: &NetworkField,
    plan: &NetworkReplicatedContainerPlan,
    serialize_types: &BTreeMap<Uuid, &NetworkSerializeType>,
) -> Option<ContainerValueType> {
    if let Some(value) = container_codec_value_type(&plan.value_codecs, serialize_types) {
        return Some(value);
    }

    let wire_shapes = plan
        .exact_value_wire_shapes()
        .or_else(|| plan.default_profile_value_wire_shapes())?;
    if let Some(serialize) = field.serialize.as_ref()
        && serialize.role == NetworkSerializeRole::SupportType
        && wire_scalar_shapes_match(&wire_shapes, &serialize.wire_shapes)
    {
        return serialize_container_value_type(
            serialize.type_id,
            serialize.kind,
            &serialize.name,
            &wire_shapes,
            serialize_types,
        );
    }

    wire_sequence_container_value_type(field, "Value", &wire_shapes, serialize_types)
}

pub(super) fn container_value_type_from_proven_shape(
    field: &NetworkField,
    vtable: &NetworkFieldHandlerVtable,
    plan: &NetworkReplicatedContainerPlan,
    serialize_types: &BTreeMap<Uuid, &NetworkSerializeType>,
) -> Option<ContainerValueType> {
    let shape = vtable
        .value_type_shape
        .as_ref()
        .filter(|shape| shape.has_exact_identity() || shape.has_proven_anonymous_layout())?;
    container_value_shape_members_are_emittable(
        shape,
        &vtable.embedded_value_type_shapes,
        serialize_types,
    )
    .then_some(())?;
    if let Some(wire_shapes) = plan
        .exact_value_wire_shapes()
        .or_else(|| plan.default_profile_value_wire_shapes())
        && !container_value_shape_matches_with_embedded(
            shape,
            &wire_shapes,
            &vtable.embedded_value_type_shapes,
        )
    {
        return None;
    }
    Some(ContainerValueType {
        rust_type: container_value_shape_rust_type(field, shape, serialize_types)?,
        marshaler_type: container_value_shape_codec_name(field, shape)?,
        value_type_shape: Some(shape.clone()),
        embedded_value_type_shapes: vtable.embedded_value_type_shapes.clone(),
    })
}

pub(super) fn container_codec_value_type(
    codecs: &[NetworkContainerCodec],
    serialize_types: &BTreeMap<Uuid, &NetworkSerializeType>,
) -> Option<ContainerValueType> {
    let [codec] = codecs else {
        return None;
    };
    if !codec.members.is_empty() && !codec.source_type_layout_complete {
        return None;
    }
    let native_type = codec.direct_type_name()?;
    let wire_shapes = codec.exact_wire_shapes().unwrap_or_default();

    if let Some(serialize) = codec
        .type_identity_proven
        .then_some(codec.type_id)
        .flatten()
        .and_then(|type_id| serialize_types.get(&type_id))
    {
        return serialize_container_value_type(
            serialize.type_id,
            serialize.kind,
            &serialize.name,
            &wire_shapes,
            serialize_types,
        );
    }

    let exact_runtime_type = codec
        .type_id
        .and_then(exact_type_id_rust_type)
        .map(ToOwned::to_owned);
    let rust_type = exact_runtime_type.or_else(|| {
        codec
            .type_identity_proven
            .then(|| network_native_type_rust_type(native_type, serialize_types))
            .flatten()
    })?;
    let marshaler_type = match wire_shapes.as_slice() {
        [shape] if scalar_rust_type(*shape) == rust_type => scalar_marshaler_type(*shape),
        _ => format!("::nw_network::serialize::DefaultMarshaler<{rust_type}>"),
    };
    Some(ContainerValueType {
        rust_type,
        marshaler_type,
        value_type_shape: None,
        embedded_value_type_shapes: Vec::new(),
    })
}

pub(super) fn wire_sequence_container_value_type(
    field: &NetworkField,
    role: &str,
    wire_shapes: &[SchemaWireScalarShape],
    serialize_types: &BTreeMap<Uuid, &NetworkSerializeType>,
) -> Option<ContainerValueType> {
    match wire_shapes {
        [] => None,
        [shape] => Some(ContainerValueType {
            rust_type: scalar_rust_type(*shape),
            marshaler_type: scalar_marshaler_type(*shape),
            value_type_shape: None,
            embedded_value_type_shapes: Vec::new(),
        }),
        shapes => {
            let shape = synthetic_container_value_shape(role, shapes);
            Some(ContainerValueType {
                rust_type: container_value_shape_rust_type(field, &shape, serialize_types)?,
                marshaler_type: container_value_shape_codec_name(field, &shape)?,
                value_type_shape: Some(shape),
                embedded_value_type_shapes: Vec::new(),
            })
        }
    }
}

fn synthetic_container_value_shape(
    role: &str,
    wire_shapes: &[SchemaWireScalarShape],
) -> crate::network_schema::NetworkNestedTypeShape {
    let members = wire_shapes
        .iter()
        .copied()
        .enumerate()
        .map(
            |(index, wire_shape)| crate::network_schema::NetworkNestedTypeMember {
                index: u32::try_from(index).ok(),
                offset: None,
                native_offset: None,
                name: Some(format!("field_{index}")),
                name_source: Some("synthetic-wire-ordinal".to_owned()),
                name_proven: Some(false),
                name_evidence: None,
                native_type: None,
                type_id: None,
                type_id_source: None,
                type_identity_proven: false,
                type_identity_source: None,
                wire_shape: Some(wire_shape.wire_string()),
                wire_shape_source: Some("exact-container-codec-sequence".to_owned()),
                wire_layout: None,
                wire_layout_source: None,
                byte_width: None,
                wire_ordinal: u32::try_from(index).ok(),
                wire_order_source: Some("exact-container-codec-sequence".to_owned()),
                callsite: None,
                target: None,
                target_name: None,
                type_conflict: false,
            },
        )
        .collect();
    crate::network_schema::NetworkNestedTypeShape {
        type_id: None,
        type_id_source: None,
        identity_proven: Some(false),
        identity_source: None,
        type_name: Some(role.to_owned()),
        type_name_full: None,
        type_name_source: Some("generated-network-support-type".to_owned()),
        function: None,
        function_name: None,
        factory: None,
        az_rtti_address: None,
        constructor: None,
        vtable: None,
        member_base: Some("element".to_owned()),
        member_name_source: Some("synthetic-wire-ordinal".to_owned()),
        member_names_proven: Some(false),
        layout_proven: Some(true),
        member_coverage_proven: Some(true),
        wire_order_proven: Some(true),
        wire_order_source: Some("exact-container-codec-sequence".to_owned()),
        datatype_path: None,
        validation: Some("complete-wire-layout".to_owned()),
        native_size: None,
        native_size_source: None,
        members,
    }
}

pub(super) fn container_value_shape_matches_with_embedded(
    shape: &crate::network_schema::NetworkNestedTypeShape,
    value_wire_shapes: &[SchemaWireScalarShape],
    embedded_shapes: &[crate::network_schema::NetworkNestedTypeShape],
) -> bool {
    if shape.members.is_empty() || value_wire_shapes.is_empty() {
        return false;
    }
    let Some(members) = nested_type_shape_members_in_wire_order(shape) else {
        return false;
    };
    let mut index = 0;
    for member in members {
        let Some(wire_shape) = nested_member_wire_shape(member) else {
            return false;
        };
        let Some(span) = container_value_member_shape_span(
            wire_shape,
            value_wire_shapes,
            index,
            embedded_shapes,
        ) else {
            return false;
        };
        index += span;
    }
    index == value_wire_shapes.len()
}

pub(super) fn scalar_shapes_match(
    observed: SchemaWireScalarShape,
    expected: SchemaWireScalarShape,
) -> bool {
    observed == expected
        || matches!(
            (observed, expected),
            (SchemaWireScalarShape::Bool, SchemaWireScalarShape::U8)
                | (SchemaWireScalarShape::U8, SchemaWireScalarShape::Bool)
        )
        || fixed_width_scalar_shapes_match(observed, expected)
}

fn fixed_width_scalar_shapes_match(
    observed: SchemaWireScalarShape,
    expected: SchemaWireScalarShape,
) -> bool {
    match (
        fixed_width_scalar_bytes(observed),
        fixed_width_scalar_bytes(expected),
    ) {
        (Some(left), Some(right)) => left == right,
        _ => false,
    }
}

fn fixed_width_scalar_bytes(shape: SchemaWireScalarShape) -> Option<u16> {
    match shape {
        SchemaWireScalarShape::Bool | SchemaWireScalarShape::U8 => Some(1),
        SchemaWireScalarShape::U16 | SchemaWireScalarShape::HalfF32 => Some(2),
        SchemaWireScalarShape::U32 | SchemaWireScalarShape::F32 => Some(4),
        SchemaWireScalarShape::U64 | SchemaWireScalarShape::F64 => Some(8),
        SchemaWireScalarShape::FixedBytes(width) => Some(width),
        _ => None,
    }
}

pub(super) fn container_value_member_shape_span(
    observed: &str,
    expected: &[SchemaWireScalarShape],
    index: usize,
    embedded_shapes: &[crate::network_schema::NetworkNestedTypeShape],
) -> Option<usize> {
    let physical = nested_member_wire_shapes(observed, embedded_shapes)?;
    if let [shape] = physical.as_slice()
        && let Some(float_limbs) = uncompressed_float_limb_count(*shape)
    {
        let end = index.checked_add(float_limbs)?;
        if expected.get(index..end).is_some_and(|expected| {
            expected
                .iter()
                .all(|expected| scalar_shapes_match(*expected, SchemaWireScalarShape::F32))
        }) {
            return Some(float_limbs);
        }
    }
    let end = index.checked_add(physical.len())?;
    physical
        .iter()
        .zip(expected.get(index..end)?)
        .all(|(observed, expected)| scalar_shapes_match(*observed, *expected))
        .then_some(physical.len())
}

fn uncompressed_float_limb_count(shape: SchemaWireScalarShape) -> Option<usize> {
    match shape {
        SchemaWireScalarShape::Vec2 => Some(2),
        SchemaWireScalarShape::Vec3 => Some(3),
        SchemaWireScalarShape::Vec4 | SchemaWireScalarShape::Quat => Some(4),
        _ => None,
    }
}

pub(super) fn wire_scalar_shape_from_name(value: &str) -> Option<SchemaWireScalarShape> {
    crate::network_schema::parse::parse_network_wire_scalar_shape(value)
}

pub(super) fn container_value_shape_codec_name(
    field: &NetworkField,
    shape: &crate::network_schema::NetworkNestedTypeShape,
) -> Option<String> {
    structured_value_codec_name(field.name.as_deref()?, shape)
}

pub(super) fn container_value_shape_rust_type(
    field: &NetworkField,
    shape: &crate::network_schema::NetworkNestedTypeShape,
    serialize_types: &BTreeMap<Uuid, &NetworkSerializeType>,
) -> Option<String> {
    if container_value_shape_uses_source_type(shape, serialize_types) {
        if let Some(rust_type) = shape.type_id.and_then(exact_type_id_rust_type) {
            return Some(rust_type.to_owned());
        }
        return shape
            .type_name_full
            .as_deref()
            .or(shape.type_name.as_deref())
            .and_then(serialize_source_rust_type_name);
    }
    container_value_shape_support_type_name(field.name.as_deref()?, shape)
}

pub(super) fn container_value_shape_uses_source_type(
    shape: &crate::network_schema::NetworkNestedTypeShape,
    serialize_types: &BTreeMap<Uuid, &NetworkSerializeType>,
) -> bool {
    let Some(source_type) = shape
        .has_exact_identity()
        .then(|| {
            shape
                .type_id
                .and_then(|type_id| serialize_types.get(&type_id))
        })
        .flatten()
    else {
        return false;
    };

    source_type.emits_source
        && match source_type.kind {
            NetworkSerializeKind::Enum => true,
            NetworkSerializeKind::Struct => !source_type.fields.is_empty(),
        }
}

pub(super) fn container_value_shape_support_type_name(
    field_name: &str,
    shape: &crate::network_schema::NetworkNestedTypeShape,
) -> Option<String> {
    let type_name = rust_type_ident(
        shape
            .type_name_full
            .as_deref()
            .or(shape.type_name.as_deref())?,
    );
    if shape.has_exact_identity() {
        return Some(type_name);
    }
    let field_name = rust_type_ident(&rust_field_ident(field_name));
    if field_name == type_name {
        Some(type_name)
    } else {
        Some(format!("{field_name}{type_name}"))
    }
}

pub(super) fn index_map_type(key_type: &str, value_type: &str) -> String {
    format!("::nw_network::serialize::IndexMap<{key_type}, {value_type}>")
}

pub(super) fn serialize_container_value_type(
    type_id: Uuid,
    kind: NetworkSerializeKind,
    name: &str,
    wire_shapes: &[SchemaWireScalarShape],
    serialize_types: &BTreeMap<Uuid, &NetworkSerializeType>,
) -> Option<ContainerValueType> {
    let rust_type = serialize_types
        .get(&type_id)
        .and_then(|serialize| network_serialize_type_rust_type(serialize, serialize_types))
        .or_else(|| exact_type_id_rust_type(type_id).map(ToOwned::to_owned))
        .or_else(|| {
            serialize_types
                .get(&type_id)
                .is_none()
                .then(|| serialize_source_rust_type_name(name))
                .flatten()
        })?;
    let marshaler_type = if kind == NetworkSerializeKind::Enum && wire_shapes.len() == 1 {
        let wire_shape = wire_shapes[0].into();
        conversion_marshal_type_string_for(&wire_shape, &rust_type)
            .unwrap_or_else(|| format!("::nw_network::serialize::DefaultMarshaler<{rust_type}>"))
    } else {
        format!("::nw_network::serialize::DefaultMarshaler<{rust_type}>")
    };
    Some(ContainerValueType {
        rust_type,
        marshaler_type,
        value_type_shape: None,
        embedded_value_type_shapes: Vec::new(),
    })
}

pub(super) fn container_value_shape_members_are_emittable(
    shape: &crate::network_schema::NetworkNestedTypeShape,
    embedded_shapes: &[crate::network_schema::NetworkNestedTypeShape],
    serialize_types: &BTreeMap<Uuid, &NetworkSerializeType>,
) -> bool {
    let uses_source_type = container_value_shape_uses_source_type(shape, serialize_types);
    let Some(members) = nested_type_shape_members_in_wire_order(shape) else {
        return false;
    };
    container_value_shape_member_names_are_emittable(shape, serialize_types)
        && shape.member_coverage_proven == Some(true)
        && members.into_iter().all(|member| {
            (!uses_source_type || exact_member_rust_type(shape, member, serialize_types).is_some())
                && container_value_member_wire_shape_is_emittable(
                    shape,
                    member,
                    embedded_shapes,
                    serialize_types,
                )
        })
}

pub(super) fn container_value_shape_member_names_are_emittable(
    shape: &crate::network_schema::NetworkNestedTypeShape,
    serialize_types: &BTreeMap<Uuid, &NetworkSerializeType>,
) -> bool {
    if container_value_shape_uses_source_type(shape, serialize_types) {
        shape.member_names_proven == Some(true)
            && shape
                .members
                .iter()
                .all(|member| member.name_proven == Some(true))
    } else {
        let mut names = BTreeSet::new();
        shape.members.iter().all(|member| {
            member.name.as_deref().is_some_and(|name| {
                !name.is_empty() && !name.contains('.') && names.insert(rust_field_ident(name))
            })
        })
    }
}

pub(super) fn container_value_member_wire_shape_is_emittable(
    parent: &crate::network_schema::NetworkNestedTypeShape,
    member: &crate::network_schema::NetworkNestedTypeMember,
    embedded_shapes: &[crate::network_schema::NetworkNestedTypeShape],
    serialize_types: &BTreeMap<Uuid, &NetworkSerializeType>,
) -> bool {
    let Some(wire_shape) = nested_member_wire_shape(member) else {
        return false;
    };
    if wire_scalar_shape_from_name(wire_shape).is_some() {
        return true;
    }
    if exact_member_rust_type(parent, member, serialize_types).is_some() {
        return true;
    }
    let Some(shape) = parse_network_member_wire_shape(wire_shape) else {
        return false;
    };
    member_wire_shape_is_emittable(&shape, embedded_shapes, serialize_types)
}

fn member_wire_shape_is_emittable(
    shape: &NetworkMemberWireShape<'_>,
    embedded_shapes: &[crate::network_schema::NetworkNestedTypeShape],
    serialize_types: &BTreeMap<Uuid, &NetworkSerializeType>,
) -> bool {
    match shape {
        NetworkMemberWireShape::Scalar(_) => true,
        NetworkMemberWireShape::Composite(members)
        | NetworkMemberWireShape::DefaultOmitted(members) => members
            .iter()
            .all(|member| member_wire_shape_is_emittable(member, embedded_shapes, serialize_types)),
        NetworkMemberWireShape::Optional(inner) => {
            member_wire_shape_is_emittable(inner, embedded_shapes, serialize_types)
        }
        NetworkMemberWireShape::BooleanChoice {
            false_value,
            true_value,
        } => {
            member_wire_shape_is_emittable(false_value, embedded_shapes, serialize_types)
                && member_wire_shape_is_emittable(true_value, embedded_shapes, serialize_types)
        }
        NetworkMemberWireShape::BitMaskComposite { members, .. } => members.iter().all(|member| {
            let value = match member {
                NetworkMemberBitMaskWireShape::Required(value)
                | NetworkMemberBitMaskWireShape::Masked { value, .. } => value,
            };
            member_wire_shape_is_emittable(value, embedded_shapes, serialize_types)
        }),
        NetworkMemberWireShape::Vector(element)
        | NetworkMemberWireShape::Set(element)
        | NetworkMemberWireShape::FixedVector { element, .. }
        | NetworkMemberWireShape::FixedArray { element, .. } => {
            member_wire_shape_is_emittable(element, embedded_shapes, serialize_types)
        }
        NetworkMemberWireShape::Map { key, value } => {
            member_wire_shape_is_emittable(key, embedded_shapes, serialize_types)
                && member_wire_shape_is_emittable(value, embedded_shapes, serialize_types)
        }
        NetworkMemberWireShape::Named("class-value") => true,
        NetworkMemberWireShape::Named(name) => nested_shape_by_wire_name(name, embedded_shapes)
            .is_some_and(|shape| {
                container_value_shape_members_are_emittable(shape, embedded_shapes, serialize_types)
            }),
    }
}

pub(super) fn nested_member_wire_shape(
    member: &crate::network_schema::NetworkNestedTypeMember,
) -> Option<&str> {
    member
        .wire_shape
        .as_deref()
        .or(member.wire_layout.as_deref())
}

pub(super) fn wire_scalar_shapes_match(
    observed: &[SchemaWireScalarShape],
    expected: &[SchemaWireScalarShape],
) -> bool {
    observed.len() == expected.len()
        && observed
            .iter()
            .zip(expected)
            .all(|(observed, expected)| scalar_shapes_match(*observed, *expected))
}
