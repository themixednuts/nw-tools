use super::*;

pub(super) fn confidence_from_raw(raw: Option<&str>) -> NetworkConfidence {
    match raw {
        Some("exact") => NetworkConfidence::Exact,
        Some("high") => NetworkConfidence::High,
        Some("inferred") => NetworkConfidence::Inferred,
        Some("weak") => NetworkConfidence::Weak,
        Some("unknown") => NetworkConfidence::Unknown,
        Some(
            "register-field-call"
            | "registration-hook"
            | "az-rtti"
            | "message-unmarshal-call"
            | "message-signature-source",
        ) => NetworkConfidence::High,
        Some(value) if value.starts_with("message-unmarshal-") => NetworkConfidence::High,
        Some(value) if value.starts_with("message-marshal-") => NetworkConfidence::High,
        Some(value) if value.starts_with("message-signature-") => NetworkConfidence::High,
        Some(value) if value.starts_with("fixed-field-table-append") => NetworkConfidence::High,
        Some(value) if value.starts_with("fixed-attribute-table-append") => NetworkConfidence::High,
        Some("constructor-match" | "vtable-match") => NetworkConfidence::Inferred,
        Some("hint") => NetworkConfidence::Weak,
        Some(_) => NetworkConfidence::Unknown,
        None => NetworkConfidence::Unknown,
    }
}

pub(super) fn array_values<'a>(
    object: &'a Map<String, Value>,
    key: &str,
) -> impl Iterator<Item = &'a Value> + 'a {
    object
        .get(key)
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
}

pub(super) fn string(object: &Map<String, Value>, key: &str) -> Option<String> {
    string_ref(object, key).map(ToOwned::to_owned)
}

pub(super) fn string_array(object: &Map<String, Value>, key: &str) -> Vec<String> {
    array_values(object, key)
        .filter_map(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
        .collect()
}

pub(super) fn string_ref<'a>(object: &'a Map<String, Value>, key: &str) -> Option<&'a str> {
    object
        .get(key)
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
}

pub(super) fn bool_value(object: &Map<String, Value>, key: &str) -> Option<bool> {
    object.get(key).and_then(Value::as_bool)
}

pub(super) const fn is_false(value: &bool) -> bool {
    !*value
}

pub(super) fn stable_address(object: &Map<String, Value>, key: &str) -> Option<String> {
    string_ref(object, key)
        .filter(|value| value.starts_with("NewWorld+0x"))
        .map(ToOwned::to_owned)
}

pub(super) fn wire_shape(object: &Map<String, Value>, key: &str) -> Option<NetworkWireShape> {
    string_ref(object, key).and_then(parse_network_wire_shape)
}

pub(crate) fn parse_network_wire_shape(value: &str) -> Option<NetworkWireShape> {
    if let Some(arguments) = generic_arguments(value, "optional") {
        let [inner] = arguments.as_slice() else {
            return None;
        };
        return parse_network_wire_shape(inner)
            .map(|shape| NetworkWireShape::Optional(Box::new(shape)));
    }
    if let Some(arguments) = generic_arguments(value, "composite") {
        return arguments
            .into_iter()
            .map(parse_network_wire_shape)
            .collect::<Option<Vec<_>>>()
            .filter(|members| !members.is_empty())
            .map(NetworkWireShape::Composite);
    }
    if let Some(arguments) = generic_arguments(value, "default-omitted") {
        return arguments
            .into_iter()
            .map(parse_network_wire_shape)
            .collect::<Option<Vec<_>>>()
            .filter(|members| (2..=12).contains(&members.len()))
            .map(NetworkWireShape::DefaultOmitted);
    }
    if let Some(arguments) = generic_arguments(value, "boolean-choice") {
        let [false_value, true_value] = arguments.as_slice() else {
            return None;
        };
        return Some(NetworkWireShape::BooleanChoice(
            NetworkBooleanChoiceWireShape {
                false_value: Box::new(parse_network_wire_shape(false_value)?),
                true_value: Box::new(parse_network_wire_shape(true_value)?),
            },
        ));
    }
    if let Some(arguments) = generic_arguments(value, "set") {
        let [inner] = arguments.as_slice() else {
            return None;
        };
        return parse_network_wire_shape(inner)
            .map(Box::new)
            .map(NetworkWireShape::Set);
    }
    if let Some(arguments) = generic_arguments(value, "vec") {
        let [inner] = arguments.as_slice() else {
            return None;
        };
        return parse_network_wire_shape(inner)
            .map(Box::new)
            .map(NetworkWireShape::Sequence);
    }
    if let Some(arguments) = generic_arguments(value, "map") {
        let [key, value] = arguments.as_slice() else {
            return None;
        };
        return Some(NetworkWireShape::Map {
            key: Box::new(parse_network_wire_shape(key)?),
            value: Box::new(parse_network_wire_shape(value)?),
        });
    }
    if let Some(container) = parse_replicated_container_wire_shape(value) {
        return Some(NetworkWireShape::ReplicatedContainer(container));
    }
    if let Some(sequence) = parse_fixed_sequence_wire_shape(value) {
        return Some(NetworkWireShape::FixedSequence(sequence));
    }
    if value == "class-value" {
        return Some(NetworkWireShape::ClassValue);
    }
    parse_network_wire_scalar_shape(value).map(Into::into)
}

pub(crate) fn parse_network_wire_scalar_shape(value: &str) -> Option<NetworkWireScalarShape> {
    if let Some(range) = parse_delta_vec3_wire_shape(value) {
        return Some(NetworkWireScalarShape::DeltaVec3(range));
    }
    if let Some(shape) = parse_packed_position_wire_shape(value) {
        return Some(NetworkWireScalarShape::PackedPosition(shape));
    }
    match value {
        "bool" => Some(NetworkWireScalarShape::Bool),
        "u8" => Some(NetworkWireScalarShape::U8),
        "u16" => Some(NetworkWireScalarShape::U16),
        "u32" => Some(NetworkWireScalarShape::U32),
        "u64" => Some(NetworkWireScalarShape::U64),
        "f32" => Some(NetworkWireScalarShape::F32),
        "f64" => Some(NetworkWireScalarShape::F64),
        "half-f32" => Some(NetworkWireScalarShape::HalfF32),
        "vlq-u32" => Some(NetworkWireScalarShape::VlqU32),
        "vlq-u64" => Some(NetworkWireScalarShape::VlqU64),
        "sequence-number" => Some(NetworkWireScalarShape::SequenceNumber),
        "vec2" => Some(NetworkWireScalarShape::Vec2),
        "vec3" => Some(NetworkWireScalarShape::Vec3),
        "vec4" => Some(NetworkWireScalarShape::Vec4),
        "quat" => Some(NetworkWireScalarShape::Quat),
        "quat-comp-norm" => Some(NetworkWireScalarShape::QuatCompNorm),
        "vec2-comp" => Some(NetworkWireScalarShape::Vec2Comp),
        "vec3-comp" => Some(NetworkWireScalarShape::Vec3Comp),
        "vec3-comp-norm" => Some(NetworkWireScalarShape::Vec3CompNorm),
        "vec3-smallest-three" => Some(NetworkWireScalarShape::Vec3SmallestThree),
        "quat-comp" => Some(NetworkWireScalarShape::QuatComp),
        "quat-smallest-three" => Some(NetworkWireScalarShape::QuatSmallestThree),
        "non-uniform-scale-comp" => Some(NetworkWireScalarShape::NonUniformScaleComp),
        "remote-server-gde-ref" => Some(NetworkWireScalarShape::RemoteServerGdeRef),
        "transform-compressor" => Some(NetworkWireScalarShape::TransformCompressor),
        "packed-size" => Some(NetworkWireScalarShape::PackedSize),
        "mat3" => Some(NetworkWireScalarShape::Mat3),
        "affine3" => Some(NetworkWireScalarShape::Affine3),
        "aabb2d" => Some(NetworkWireScalarShape::Aabb2d),
        "aabb3d" => Some(NetworkWireScalarShape::Aabb3d),
        "actor-ref" => Some(NetworkWireScalarShape::ActorRef),
        "entity-ref" => Some(NetworkWireScalarShape::EntityRef),
        "length-prefixed-bytes" => Some(NetworkWireScalarShape::Bytes),
        "string" => Some(NetworkWireScalarShape::String),
        value => fixed_bytes_wire_shape(value),
    }
}

fn parse_delta_vec3_wire_shape(value: &str) -> Option<u32> {
    let range = value
        .strip_prefix("delta-vec3<")?
        .strip_suffix('>')?
        .parse::<u32>()
        .ok()?;
    (range > 0).then_some(range)
}

pub(super) fn parse_packed_position_wire_shape(
    value: &str,
) -> Option<NetworkPackedPositionWireShape> {
    let inner = value.strip_prefix("packed-position<")?.strip_suffix('>')?;
    let (minimum, maximum) = inner.split_once(',')?;
    let minimum_bits = parse_u32_bits(minimum)?;
    let maximum_bits = parse_u32_bits(maximum)?;
    let minimum = f32::from_bits(minimum_bits);
    let maximum = f32::from_bits(maximum_bits);
    (minimum.is_finite() && maximum.is_finite() && minimum < maximum).then_some(
        NetworkPackedPositionWireShape {
            minimum_bits,
            maximum_bits,
        },
    )
}

fn parse_u32_bits(value: &str) -> Option<u32> {
    u32::from_str_radix(value.trim().strip_prefix("0x")?, 16).ok()
}

pub(super) fn fixed_bytes_wire_shape(value: &str) -> Option<NetworkWireScalarShape> {
    let len = value
        .strip_prefix("fixed-bytes-")
        .or_else(|| value.strip_prefix("fixed-bytes"))?
        .parse::<u16>()
        .ok()?;
    (len > 0).then_some(NetworkWireScalarShape::FixedBytes(len))
}

pub(super) fn parse_replicated_container_wire_shape(
    value: &str,
) -> Option<NetworkReplicatedContainerWireShape> {
    let [key, value] = generic_arguments(value, "replicated-container")?
        .as_slice()
        .try_into()
        .ok()?;
    Some(NetworkReplicatedContainerWireShape {
        key: parse_network_wire_scalar_shape(key)?,
        value: parse_network_wire_scalar_shape(value)?,
    })
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum NetworkMemberWireShape<'a> {
    Scalar(NetworkWireScalarShape),
    Composite(Vec<Self>),
    Optional(Box<Self>),
    DefaultOmitted(Vec<Self>),
    BooleanChoice {
        false_value: Box<Self>,
        true_value: Box<Self>,
    },
    Vector(Box<Self>),
    Set(Box<Self>),
    Map {
        key: Box<Self>,
        value: Box<Self>,
    },
    FixedVector {
        element: Box<Self>,
        capacity: u16,
    },
    FixedArray {
        element: Box<Self>,
        capacity: u16,
    },
    Named(&'a str),
}

pub(crate) fn parse_network_member_wire_shape(value: &str) -> Option<NetworkMemberWireShape<'_>> {
    let value = value.trim();
    if let Some(scalar) = parse_network_wire_scalar_shape(value) {
        return Some(NetworkMemberWireShape::Scalar(scalar));
    }
    if let Some(arguments) = generic_arguments(value, "composite") {
        return arguments
            .into_iter()
            .map(parse_network_member_wire_shape)
            .collect::<Option<Vec<_>>>()
            .filter(|members| !members.is_empty())
            .map(NetworkMemberWireShape::Composite);
    }
    if let Some(arguments) = generic_arguments(value, "optional") {
        let [inner] = arguments.as_slice() else {
            return None;
        };
        return parse_network_member_wire_shape(inner)
            .map(Box::new)
            .map(NetworkMemberWireShape::Optional);
    }
    if let Some(arguments) = generic_arguments(value, "default-omitted") {
        return arguments
            .into_iter()
            .map(parse_network_member_wire_shape)
            .collect::<Option<Vec<_>>>()
            .filter(|members| (2..=12).contains(&members.len()))
            .map(NetworkMemberWireShape::DefaultOmitted);
    }
    if let Some(arguments) = generic_arguments(value, "boolean-choice") {
        let [false_value, true_value] = arguments.as_slice() else {
            return None;
        };
        return Some(NetworkMemberWireShape::BooleanChoice {
            false_value: Box::new(parse_network_member_wire_shape(false_value)?),
            true_value: Box::new(parse_network_member_wire_shape(true_value)?),
        });
    }
    if let Some(arguments) = generic_arguments(value, "vec") {
        let [element] = arguments.as_slice().try_into().ok()?;
        return Some(NetworkMemberWireShape::Vector(Box::new(
            parse_network_member_wire_shape(element)?,
        )));
    }
    if let Some(arguments) = generic_arguments(value, "set") {
        let [element] = arguments.as_slice().try_into().ok()?;
        return Some(NetworkMemberWireShape::Set(Box::new(
            parse_network_member_wire_shape(element)?,
        )));
    }
    if let Some(arguments) = generic_arguments(value, "map") {
        let [key, value] = arguments.as_slice().try_into().ok()?;
        return Some(NetworkMemberWireShape::Map {
            key: Box::new(parse_network_member_wire_shape(key)?),
            value: Box::new(parse_network_member_wire_shape(value)?),
        });
    }
    if let Some(arguments) = generic_arguments(value, "fixed-vector") {
        let [element, capacity] = arguments.as_slice().try_into().ok()?;
        return Some(NetworkMemberWireShape::FixedVector {
            element: Box::new(parse_network_member_wire_shape(element)?),
            capacity: parse_collection_capacity(capacity)?,
        });
    }
    if let Some(arguments) = generic_arguments(value, "fixed-array") {
        let [element, capacity] = arguments.as_slice().try_into().ok()?;
        return Some(NetworkMemberWireShape::FixedArray {
            element: Box::new(parse_network_member_wire_shape(element)?),
            capacity: parse_collection_capacity(capacity)?,
        });
    }
    (!value.is_empty() && !value.contains(['<', '>', ',']))
        .then_some(NetworkMemberWireShape::Named(value))
}

fn generic_arguments<'a>(value: &'a str, name: &str) -> Option<Vec<&'a str>> {
    let inner = value
        .strip_prefix(name)?
        .strip_prefix('<')?
        .strip_suffix('>')?;
    split_top_level_arguments(inner)
}

fn split_top_level_arguments(value: &str) -> Option<Vec<&str>> {
    let mut arguments = Vec::new();
    let mut depth = 0u32;
    let mut start = 0usize;
    for (index, byte) in value.bytes().enumerate() {
        match byte {
            b'<' => depth = depth.checked_add(1)?,
            b'>' => depth = depth.checked_sub(1)?,
            b',' if depth == 0 => {
                let argument = value.get(start..index)?.trim();
                if argument.is_empty() {
                    return None;
                }
                arguments.push(argument);
                start = index + 1;
            }
            _ => {}
        }
    }
    if depth != 0 {
        return None;
    }
    let argument = value.get(start..)?.trim();
    if argument.is_empty() {
        return None;
    }
    arguments.push(argument);
    Some(arguments)
}

fn parse_collection_capacity(value: &str) -> Option<u16> {
    value.trim().parse().ok().filter(|capacity| *capacity > 0)
}

pub(crate) fn nested_type_shape_wire_shapes(
    shape: &NetworkNestedTypeShape,
    embedded_shapes: &[NetworkNestedTypeShape],
) -> Option<Vec<NetworkWireScalarShape>> {
    if shape.members.is_empty() {
        return None;
    }

    let mut shapes = Vec::new();
    for member in &shape.members {
        let wire_shape = member
            .wire_shape
            .as_deref()
            .or(member.wire_layout.as_deref())?;
        shapes.extend(nested_member_wire_shapes(wire_shape, embedded_shapes)?);
    }
    (!shapes.is_empty()).then_some(shapes)
}

pub(crate) fn wire_shape_scalar_product(
    shape: &NetworkWireShape,
) -> Option<Vec<NetworkWireScalarShape>> {
    nested_member_wire_shapes(&shape.wire_string(), &[])
}

pub(crate) fn nested_member_wire_shapes(
    observed: &str,
    embedded_shapes: &[NetworkNestedTypeShape],
) -> Option<Vec<NetworkWireScalarShape>> {
    flatten_member_wire_shape(&parse_network_member_wire_shape(observed)?, embedded_shapes)
}

fn flatten_member_wire_shape(
    shape: &NetworkMemberWireShape<'_>,
    embedded_shapes: &[NetworkNestedTypeShape],
) -> Option<Vec<NetworkWireScalarShape>> {
    match shape {
        NetworkMemberWireShape::Scalar(shape) => Some(vec![*shape]),
        NetworkMemberWireShape::Composite(members) => {
            let mut shapes = Vec::new();
            for member in members {
                shapes.extend(flatten_member_wire_shape(member, embedded_shapes)?);
            }
            Some(shapes)
        }
        NetworkMemberWireShape::Optional(_)
        | NetworkMemberWireShape::DefaultOmitted(_)
        | NetworkMemberWireShape::BooleanChoice { .. } => None,
        NetworkMemberWireShape::Vector(element)
        | NetworkMemberWireShape::Set(element)
        | NetworkMemberWireShape::FixedVector { element, .. } => {
            let mut shapes = vec![NetworkWireScalarShape::VlqU32];
            shapes.extend(flatten_member_wire_shape(element, embedded_shapes)?);
            Some(shapes)
        }
        NetworkMemberWireShape::Map { key, value } => {
            let mut shapes = vec![NetworkWireScalarShape::VlqU32];
            shapes.extend(flatten_member_wire_shape(key, embedded_shapes)?);
            shapes.extend(flatten_member_wire_shape(value, embedded_shapes)?);
            Some(shapes)
        }
        NetworkMemberWireShape::FixedArray { element, capacity } => Some(
            flatten_member_wire_shape(element, embedded_shapes)?.repeat(usize::from(*capacity)),
        ),
        NetworkMemberWireShape::Named(name) => {
            let embedded = nested_shape_by_wire_name(name, embedded_shapes)?;
            nested_type_shape_wire_shapes(embedded, embedded_shapes)
        }
    }
}

pub(crate) fn nested_shape_by_wire_name<'a>(
    name: &str,
    shapes: &'a [NetworkNestedTypeShape],
) -> Option<&'a NetworkNestedTypeShape> {
    shapes.iter().find(|shape| {
        [shape.type_name.as_deref(), shape.type_name_full.as_deref()]
            .into_iter()
            .flatten()
            .any(|candidate| type_name_leaf(candidate) == name)
    })
}

pub(crate) fn type_name_leaf(name: &str) -> &str {
    name.rsplit("::").next().unwrap_or(name).trim()
}

pub(crate) fn vector_element_wire_shape(value: &str) -> Option<&str> {
    let arguments = generic_arguments(value, "vec")?;
    let [element] = arguments.as_slice().try_into().ok()?;
    Some(element)
}

pub(crate) fn fixed_vector_wire_shape(value: &str) -> Option<(&str, u16)> {
    let arguments = generic_arguments(value, "fixed-vector")?;
    let [element, capacity] = arguments.as_slice().try_into().ok()?;
    Some((element, parse_collection_capacity(capacity)?))
}

pub(crate) fn fixed_array_wire_shape(value: &str) -> Option<(&str, u16)> {
    let arguments = generic_arguments(value, "fixed-array")?;
    let [element, capacity] = arguments.as_slice().try_into().ok()?;
    Some((element, parse_collection_capacity(capacity)?))
}

pub(crate) fn sequence_element_wire_shape(value: &str) -> Option<&str> {
    vector_element_wire_shape(value)
        .or_else(|| fixed_vector_wire_shape(value).map(|(element, _)| element))
}

pub(crate) fn collection_element_wire_shape(value: &str) -> Option<&str> {
    sequence_element_wire_shape(value)
        .or_else(|| fixed_array_wire_shape(value).map(|(element, _)| element))
}

pub(crate) fn composite_member_wire_shapes(value: &str) -> Option<Vec<NetworkWireScalarShape>> {
    let shape = parse_network_member_wire_shape(value)?;
    matches!(shape, NetworkMemberWireShape::Composite(_))
        .then(|| flatten_member_wire_shape(&shape, &[]))?
}

pub(super) fn u32_value(object: &Map<String, Value>, key: &str) -> Option<u32> {
    object.get(key).and_then(|value| match value {
        Value::Number(number) => number.as_u64().and_then(|value| value.try_into().ok()),
        Value::String(value) => value.parse().ok(),
        _ => None,
    })
}

pub(super) fn u64_value(object: &Map<String, Value>, key: &str) -> Option<u64> {
    object.get(key).and_then(|value| match value {
        Value::Number(number) => number.as_u64(),
        Value::String(value) => value.parse().ok(),
        _ => None,
    })
}

pub(super) fn hex_or_decimal_u32(object: &Map<String, Value>, key: &str) -> Option<u32> {
    object.get(key).and_then(|value| match value {
        Value::Number(number) => number.as_u64().and_then(|value| value.try_into().ok()),
        Value::String(value) => {
            let trimmed = value.trim();
            trimmed
                .strip_prefix("0x")
                .or_else(|| trimmed.strip_prefix("0X"))
                .map_or_else(
                    || trimmed.parse().ok(),
                    |hex| u32::from_str_radix(hex, 16).ok(),
                )
        }
        _ => None,
    })
}

pub(super) fn hex_or_decimal_u64(object: &Map<String, Value>, key: &str) -> Option<u64> {
    object.get(key).and_then(|value| match value {
        Value::Number(number) => number.as_u64(),
        Value::String(value) => {
            let trimmed = value.trim();
            trimmed
                .strip_prefix("0x")
                .or_else(|| trimmed.strip_prefix("0X"))
                .map_or_else(
                    || trimmed.parse().ok(),
                    |hex| u64::from_str_radix(hex, 16).ok(),
                )
        }
        _ => None,
    })
}

pub(super) fn usize_value(object: &Map<String, Value>, key: &str) -> Option<usize> {
    object.get(key).and_then(|value| match value {
        Value::Number(number) => number.as_u64().and_then(|value| value.try_into().ok()),
        Value::String(value) => value.parse().ok(),
        _ => None,
    })
}

pub(super) fn uuid(object: &Map<String, Value>, key: &str) -> Option<Uuid> {
    string_ref(object, key).and_then(parse_uuid)
}

pub(super) fn parse_uuid(value: &str) -> Option<Uuid> {
    Uuid::parse_str(value.trim_matches(['{', '}'])).ok()
}
