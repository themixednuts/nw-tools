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
    if let Some(arguments) = generic_arguments(value, "bit-mask-composite") {
        let [mask, members @ ..] = arguments.as_slice() else {
            return None;
        };
        let mask = parse_byte_mask_scalar(mask)?;
        let members = members
            .iter()
            .map(|member| parse_bit_mask_member_wire_shape(member))
            .collect::<Option<Vec<_>>>()?;
        if members.is_empty() || members.len() > 11 {
            return None;
        }
        return Some(NetworkWireShape::BitMaskComposite(
            NetworkBitMaskCompositeWireShape { mask, members },
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
    if value == "actor-instantiation-parameters" {
        return Some(NetworkWireShape::ActorInstantiationParameters);
    }
    parse_network_wire_scalar_shape(value).map(Into::into)
}

fn parse_byte_mask_scalar(value: &str) -> Option<NetworkWireScalarShape> {
    match parse_network_wire_scalar_shape(value)? {
        mask @ (NetworkWireScalarShape::U8 | NetworkWireScalarShape::FixedBytes(1)) => Some(mask),
        _ => None,
    }
}

fn parse_bit_mask_member_wire_shape(value: &str) -> Option<NetworkBitMaskMemberWireShape> {
    if let Some(arguments) = generic_arguments(value, "required") {
        let [value] = arguments.as_slice() else {
            return None;
        };
        return Some(NetworkBitMaskMemberWireShape::Required(Box::new(
            parse_network_wire_shape(value)?,
        )));
    }
    let [mask, value] = generic_arguments(value, "masked")?
        .as_slice()
        .try_into()
        .ok()?;
    Some(NetworkBitMaskMemberWireShape::Masked {
        mask: parse_single_byte_mask(mask)?,
        value: Box::new(parse_network_wire_shape(value)?),
    })
}

fn parse_single_byte_mask(value: &str) -> Option<u8> {
    let mask = u8::from_str_radix(value.trim().strip_prefix("0x")?, 16).ok()?;
    mask.is_power_of_two().then_some(mask)
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
    BitMaskComposite {
        mask: NetworkWireScalarShape,
        members: Vec<NetworkMemberBitMaskWireShape<'a>>,
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum NetworkMemberBitMaskWireShape<'a> {
    Required(Box<NetworkMemberWireShape<'a>>),
    Masked {
        mask: u8,
        value: Box<NetworkMemberWireShape<'a>>,
    },
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
    if let Some(arguments) = generic_arguments(value, "bit-mask-composite") {
        let [mask, members @ ..] = arguments.as_slice() else {
            return None;
        };
        let mask = parse_byte_mask_scalar(mask)?;
        let members = members
            .iter()
            .map(|member| parse_network_member_bit_mask_shape(member))
            .collect::<Option<Vec<_>>>()?;
        if members.is_empty() || members.len() > 11 {
            return None;
        }
        return Some(NetworkMemberWireShape::BitMaskComposite { mask, members });
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

fn parse_network_member_bit_mask_shape(value: &str) -> Option<NetworkMemberBitMaskWireShape<'_>> {
    if let Some(arguments) = generic_arguments(value, "required") {
        let [value] = arguments.as_slice() else {
            return None;
        };
        return Some(NetworkMemberBitMaskWireShape::Required(Box::new(
            parse_network_member_wire_shape(value)?,
        )));
    }
    let [mask, value] = generic_arguments(value, "masked")?
        .as_slice()
        .try_into()
        .ok()?;
    Some(NetworkMemberBitMaskWireShape::Masked {
        mask: parse_single_byte_mask(mask)?,
        value: Box::new(parse_network_member_wire_shape(value)?),
    })
}

fn generic_arguments<'a>(value: &'a str, name: &str) -> Option<Vec<&'a str>> {
    let inner = value
        .strip_prefix(name)?
        .strip_prefix('<')?
        .strip_suffix('>')?;
    split_top_level_arguments(inner)
}

pub(crate) fn top_level_composite_members(value: &str) -> Option<Vec<&str>> {
    generic_arguments(value, "composite")
}

/// Collapse `composite<A,B>` when A and B are width-compatible structured
/// spellings of the same payload.
pub(crate) fn collapse_alternate_spelling_wire_product<'a>(
    wire: &'a str,
    nested: Option<&NetworkNestedTypeShape>,
) -> Option<&'a str> {
    let members = top_level_composite_members(wire)?;
    if members.len() != 2 {
        return None;
    }
    let (left, right) = (members[0], members[1]);
    if compatible_alternate_wire_products(left, right)
        && (has_explicit_wire_structure(left) || has_explicit_wire_structure(right))
    {
        return Some(prefer_alternate_wire_product(left, right, nested));
    }
    // Successive helpers sometimes re-append a leading/trailing sub-product
    // already present inside the other product (ActorRequestId pattern).
    // Require nested-shape agreement so legitimate composite<field, same-type>
    // products are not collapsed.
    collapse_redundant_composite_boundary_limb(left, right, nested)
}

fn has_explicit_wire_structure(value: &str) -> bool {
    top_level_composite_members(value).is_some()
        || value.starts_with("vec<")
        || value.starts_with("fixed-vector<")
        || value.starts_with("counted-set<")
        || value.starts_with("counted-map<")
        || value.starts_with("bit-mask-composite<")
}

fn collapse_redundant_composite_boundary_limb<'a>(
    left: &'a str,
    right: &'a str,
    nested: Option<&NetworkNestedTypeShape>,
) -> Option<&'a str> {
    let observed = nested.and_then(|shape| nested_type_shape_wire_shapes(shape, &[]))?;
    let left_matches = nested_member_wire_shapes(left, &[])
        .is_some_and(|product| wire_scalar_products_width_compatible(&product, &observed));
    let right_matches = nested_member_wire_shapes(right, &[])
        .is_some_and(|product| wire_scalar_products_width_compatible(&product, &observed));
    let complete_nested_product = nested.is_some_and(|shape| {
        shape.layout_proven == Some(true)
            && shape.member_coverage_proven == Some(true)
            && shape.wire_order_proven == Some(true)
    });
    match (left_matches, right_matches) {
        (true, false) => {
            (complete_nested_product || boundary_limb_is_redundant(left, right)).then_some(left)
        }
        (false, true) => {
            (complete_nested_product || boundary_limb_is_redundant(right, left)).then_some(right)
        }
        (true, true) => Some(prefer_alternate_wire_product(left, right, nested)),
        (false, false) => None,
    }
}

fn boundary_limb_is_redundant(product: &str, limb: &str) -> bool {
    let Some(product) = nested_member_wire_shapes(product, &[]) else {
        return false;
    };
    let Some(limb) = nested_member_wire_shapes(limb, &[]) else {
        return false;
    };
    if limb.is_empty() || limb.len() >= product.len() {
        return false;
    }
    wire_scalar_products_width_compatible(&product[..limb.len()], &limb)
        || wire_scalar_products_width_compatible(&product[product.len() - limb.len()..], &limb)
}

fn compatible_alternate_wire_products(left: &str, right: &str) -> bool {
    let Some(left_product) = nested_member_wire_shapes(left, &[]) else {
        return false;
    };
    let Some(right_product) = nested_member_wire_shapes(right, &[]) else {
        return false;
    };
    wire_scalar_products_width_compatible(&left_product, &right_product)
}

fn prefer_alternate_wire_product<'a>(
    left: &'a str,
    right: &'a str,
    nested: Option<&NetworkNestedTypeShape>,
) -> &'a str {
    if left.starts_with("fixed-vector<") && right.starts_with("vec<") {
        return left;
    }
    if right.starts_with("fixed-vector<") && left.starts_with("vec<") {
        return right;
    }
    if let Some(observed) = nested.and_then(|shape| nested_type_shape_wire_shapes(shape, &[])) {
        for candidate in [left, right] {
            if nested_member_wire_shapes(candidate, &[])
                .is_some_and(|product| wire_scalar_products_width_compatible(&product, &observed))
            {
                // Prefer semantic atoms when both match nested limbs.
                if candidate == "actor-ref" || candidate == "entity-ref" {
                    return candidate;
                }
            }
        }
        for candidate in [right, left] {
            if nested_member_wire_shapes(candidate, &[])
                .is_some_and(|product| wire_scalar_products_width_compatible(&product, &observed))
            {
                return candidate;
            }
        }
    }
    if codec_product_semantic_score(right) > codec_product_semantic_score(left) {
        right
    } else {
        left
    }
}

fn codec_product_semantic_score(product: &str) -> i32 {
    if product == "actor-ref" || product == "entity-ref" {
        return 1_000;
    }
    if let Some(members) = top_level_composite_members(product) {
        return members
            .iter()
            .map(|member| codec_product_semantic_score(member))
            .sum();
    }
    if product.starts_with("fixed-bytes-") {
        0
    } else {
        10
    }
}

pub(crate) fn wire_scalar_products_width_compatible(
    left: &[NetworkWireScalarShape],
    right: &[NetworkWireScalarShape],
) -> bool {
    let mut left_index = 0;
    let mut right_index = 0;
    let mut left_width = 0u32;
    let mut right_width = 0u32;

    loop {
        if left_width == right_width {
            left_width = 0;
            right_width = 0;
            match (left.get(left_index), right.get(right_index)) {
                (None, None) => return true,
                (Some(left), Some(right)) => {
                    match (scalar_wire_width(*left), scalar_wire_width(*right)) {
                        (Some(next_left), Some(next_right)) => {
                            left_width = u32::from(next_left);
                            right_width = u32::from(next_right);
                            left_index += 1;
                            right_index += 1;
                        }
                        (None, None) if left == right => {
                            left_index += 1;
                            right_index += 1;
                        }
                        _ => return false,
                    }
                }
                _ => return false,
            }
        } else if left_width < right_width {
            let Some(next) = left
                .get(left_index)
                .and_then(|shape| scalar_wire_width(*shape))
            else {
                return false;
            };
            let Some(total) = left_width.checked_add(u32::from(next)) else {
                return false;
            };
            left_width = total;
            left_index += 1;
        } else {
            let Some(next) = right
                .get(right_index)
                .and_then(|shape| scalar_wire_width(*shape))
            else {
                return false;
            };
            let Some(total) = right_width.checked_add(u32::from(next)) else {
                return false;
            };
            right_width = total;
            right_index += 1;
        }
    }
}

fn scalar_wire_width(shape: NetworkWireScalarShape) -> Option<u16> {
    match shape {
        NetworkWireScalarShape::Bool | NetworkWireScalarShape::U8 => Some(1),
        NetworkWireScalarShape::U16 | NetworkWireScalarShape::HalfF32 => Some(2),
        NetworkWireScalarShape::U32 | NetworkWireScalarShape::F32 => Some(4),
        NetworkWireScalarShape::U64 | NetworkWireScalarShape::F64 => Some(8),
        NetworkWireScalarShape::FixedBytes(width) => Some(width),
        NetworkWireScalarShape::Vec2 | NetworkWireScalarShape::Vec2Comp => Some(8),
        NetworkWireScalarShape::Vec3
        | NetworkWireScalarShape::Vec3Comp
        | NetworkWireScalarShape::Vec3CompNorm
        | NetworkWireScalarShape::Vec3SmallestThree
        | NetworkWireScalarShape::NonUniformScaleComp => Some(12),
        NetworkWireScalarShape::Vec4
        | NetworkWireScalarShape::Quat
        | NetworkWireScalarShape::QuatComp
        | NetworkWireScalarShape::QuatCompNorm
        | NetworkWireScalarShape::QuatSmallestThree => Some(16),
        NetworkWireScalarShape::Aabb2d => Some(16),
        NetworkWireScalarShape::Aabb3d => Some(24),
        NetworkWireScalarShape::Mat3 => Some(36),
        NetworkWireScalarShape::Affine3 => Some(48),
        NetworkWireScalarShape::ActorRef => Some(36),
        _ => None,
    }
}

pub(crate) fn collapse_field_alternate_spelling_wire_products(field: &mut NetworkField) {
    let raw = field
        .wire_layout
        .as_deref()
        .or(field.wire_shape_raw.as_deref())
        .map(str::to_owned);
    let Some(raw) = raw else {
        return;
    };
    if let Some(shape) = field.nested_type_shape.as_mut() {
        collapse_synthetic_nested_duplicate_wire_product(shape, &raw);
    }
    let Some(preferred) =
        collapse_alternate_spelling_wire_product(&raw, field.nested_type_shape.as_ref())
    else {
        return;
    };
    let preferred = preferred.to_owned();
    collapse_synthetic_nested_alternate_spelling_product(
        field.nested_type_shape.as_mut(),
        &raw,
        &preferred,
    );
    let parsed = parse_network_wire_shape(&preferred);
    field.wire_layout = Some(preferred.clone());
    field.wire_layout_source = Some("cfg-multi-helper-alternate-spelling-collapse".to_owned());
    field.wire_shape_raw = Some(preferred);
    field.wire_shape = parsed;
    field.wire_shape_source = field.wire_layout_source.clone();
}

pub(crate) fn collapse_redundant_message_aggregate_fields(fields: &mut Vec<NetworkField>) {
    let redundant = (0..fields.len())
        .filter(|&field_index| {
            (0..fields.len()).any(|aggregate_index| {
                aggregate_index != field_index
                    && is_redundant_message_aggregate_field(
                        &fields[aggregate_index],
                        &fields[field_index],
                    )
            })
        })
        .collect::<BTreeSet<_>>();
    if redundant.is_empty() {
        return;
    }

    let mut index = 0usize;
    fields.retain(|_| {
        let keep = !redundant.contains(&index);
        index += 1;
        keep
    });
    for (index, field) in fields.iter_mut().enumerate() {
        field.index = u32::try_from(index).ok();
    }
}

pub(crate) fn normalize_proven_message_aggregate_boundary(field: &mut NetworkField) {
    let Some(shape) = field.nested_type_shape.as_ref().filter(|shape| {
        shape.validation.as_deref() == Some("call-frame-output-parameter")
            && shape.has_proven_anonymous_layout()
            && shape.members.len() >= 2
    }) else {
        return;
    };

    let mut members = shape.members.iter().collect::<Vec<_>>();
    members.sort_by_key(|member| member.wire_ordinal);
    if members
        .iter()
        .enumerate()
        .any(|(index, member)| member.wire_ordinal != u32::try_from(index).ok())
    {
        return;
    }
    let Some(child_type) = field.native_type.as_deref().filter(|native_type| {
        members
            .iter()
            .any(|member| member.native_type.as_deref() == Some(*native_type))
    }) else {
        return;
    };
    let child_type = child_type.to_owned();

    let Some(semantic_members) = members
        .iter()
        .map(|member| {
            member
                .wire_shape
                .as_deref()
                .or(member.wire_layout.as_deref())
        })
        .collect::<Option<Vec<_>>>()
    else {
        return;
    };
    let Some(layout_members) = members
        .iter()
        .map(|member| {
            member
                .wire_layout
                .as_deref()
                .or(member.wire_shape.as_deref())
        })
        .collect::<Option<Vec<_>>>()
    else {
        return;
    };
    let semantic = format!("composite<{}>", semantic_members.join(","));
    let layout = format!("composite<{}>", layout_members.join(","));
    let Some(nested_product) = nested_member_wire_shapes(&semantic, &[]) else {
        return;
    };
    let parent_matches = network_field_wire_product(field).is_some_and(|parent_product| {
        wire_products_machine_compatible(&parent_product, &nested_product)
    });
    if !parent_matches {
        let source = "normalized-proven-call-frame-output-product".to_owned();
        field.wire_shape = parse_network_wire_shape(&semantic);
        field.wire_shape_raw = Some(semantic);
        field.wire_shape_source = Some(source.clone());
        field.wire_layout = Some(layout);
        field.wire_layout_source = Some(source);
    }

    field.native_type = None;
    if field.source_type_name.as_deref() == Some(child_type.as_str()) {
        field.source_type_name = None;
    }
    if !field.source_type_identity_proven {
        field.source_type_id = None;
        field.source_type_id_source = None;
    }
    field.rust_type = None;
    field.serialize = None;
}

fn is_redundant_message_aggregate_field(aggregate: &NetworkField, field: &NetworkField) -> bool {
    let Some(shape) = aggregate.nested_type_shape.as_ref().filter(|shape| {
        shape.validation.as_deref() == Some("call-frame-output-parameter")
            && shape.has_proven_layout()
    }) else {
        return false;
    };
    let (Some(aggregate_base), Some(field_base)) = (
        aggregate.storage_base.as_deref(),
        field.storage_base.as_deref(),
    ) else {
        return false;
    };
    if aggregate_base != field_base {
        return false;
    }
    let (Some(aggregate_offset), Some(field_offset)) =
        (aggregate.storage_offset, field.storage_offset)
    else {
        return false;
    };
    let Some(relative_offset) = field_offset.checked_sub(aggregate_offset) else {
        return false;
    };
    let Some(field_callsite) = field.callsite.as_deref() else {
        return false;
    };
    let Some(field_product) = network_field_wire_product(field) else {
        return false;
    };

    if shape.members.iter().any(|member| {
        nested_member_offset(member) == Some(relative_offset)
            && member.callsite.as_deref() == Some(field_callsite)
            && nested_member_wire_product(member).is_some_and(|member_product| {
                wire_products_machine_compatible(&field_product, &member_product)
            })
    }) {
        return true;
    }

    let same_callsite_members = shape
        .members
        .iter()
        .skip_while(|member| nested_member_offset(member) != Some(relative_offset))
        .take_while(|member| member.callsite.as_deref() == Some(field_callsite))
        .collect::<Vec<_>>();
    if same_callsite_members.is_empty() {
        return false;
    }
    let Some(member_products) = same_callsite_members
        .into_iter()
        .map(nested_member_wire_product)
        .collect::<Option<Vec<_>>>()
    else {
        return false;
    };
    let member_product = member_products.into_iter().flatten().collect::<Vec<_>>();
    field_product.len() == member_product.len() + 1
        && wire_scalar_products_width_compatible(&field_product[1..], &member_product)
}

fn network_field_wire_product(field: &NetworkField) -> Option<Vec<NetworkWireScalarShape>> {
    let product = field
        .wire_shape
        .as_ref()
        .map(NetworkWireShape::wire_string)
        .or_else(|| field.wire_layout.clone())?;
    nested_member_wire_shapes(&product, &[])
}

fn nested_member_wire_product(
    member: &NetworkNestedTypeMember,
) -> Option<Vec<NetworkWireScalarShape>> {
    let product = member
        .wire_shape
        .as_deref()
        .or(member.wire_layout.as_deref())?;
    nested_member_wire_shapes(product, &[])
}

fn wire_products_machine_compatible(
    left: &[NetworkWireScalarShape],
    right: &[NetworkWireScalarShape],
) -> bool {
    wire_scalar_products_width_compatible(left, right)
}

fn nested_member_offset(member: &NetworkNestedTypeMember) -> Option<u32> {
    let value = member
        .native_offset
        .as_deref()
        .or(member.offset.as_deref())?;
    value
        .strip_prefix("0x")
        .or_else(|| value.strip_prefix("0X"))
        .map_or_else(
            || value.parse().ok(),
            |value| u32::from_str_radix(value, 16).ok(),
        )
}

fn collapse_synthetic_nested_alternate_spelling_product(
    shape: Option<&mut NetworkNestedTypeShape>,
    raw: &str,
    preferred: &str,
) {
    let Some(shape) = shape else {
        return;
    };
    if !is_proven_synthetic_nested_wire_product(shape) {
        return;
    }

    let Some(alternatives) = top_level_composite_members(raw) else {
        return;
    };
    if alternatives.len() != 2 || shape.members.len() != alternatives.len() {
        return;
    }
    let all_members_are_alternate_spellings =
        shape
            .members
            .iter()
            .zip(&alternatives)
            .all(|(member, alternative)| {
                let Some(member_product) = member
                    .wire_shape
                    .as_deref()
                    .or(member.wire_layout.as_deref())
                else {
                    return false;
                };
                compatible_alternate_wire_products(member_product, alternative)
                    && compatible_alternate_wire_products(member_product, preferred)
            });
    if !all_members_are_alternate_spellings {
        return;
    }

    replace_synthetic_nested_wire_product(shape, preferred);
}

pub(crate) fn collapse_synthetic_nested_duplicate_wire_product(
    shape: &mut NetworkNestedTypeShape,
    preferred: &str,
) -> bool {
    if !is_proven_synthetic_nested_wire_product(shape) || shape.members.len() != 2 {
        return false;
    }

    let Some(callsite) = shape
        .members
        .first()
        .and_then(|member| member.callsite.clone())
    else {
        return false;
    };
    if !shape
        .members
        .iter()
        .all(|member| member.callsite.as_ref() == Some(&callsite))
    {
        return false;
    }
    if !shape.members.iter().all(|member| {
        member
            .wire_shape
            .as_deref()
            .or(member.wire_layout.as_deref())
            .is_some_and(|product| compatible_alternate_wire_products(product, preferred))
    }) {
        return false;
    }

    replace_synthetic_nested_wire_product(shape, preferred);
    true
}

fn is_proven_synthetic_nested_wire_product(shape: &NetworkNestedTypeShape) -> bool {
    shape.validation.as_deref()
        == Some("message-unmarshal-constructor-vptr+az-rtti+typeregistry-type-name")
        && shape.member_name_source.as_deref() == Some("synthetic-offset")
        && shape.wire_order_source.as_deref() == Some("cfg-ordered-multi-helper-wire-product")
        && shape.layout_proven == Some(true)
        && shape.member_coverage_proven == Some(true)
        && shape.wire_order_proven == Some(true)
}

fn replace_synthetic_nested_wire_product(shape: &mut NetworkNestedTypeShape, preferred: &str) {
    let member_products = top_level_composite_members(preferred).unwrap_or_else(|| vec![preferred]);
    let callsite = shape
        .members
        .iter()
        .find_map(|member| member.callsite.clone());
    shape.members = member_products
        .into_iter()
        .enumerate()
        .map(|(index, product)| NetworkNestedTypeMember {
            index: u32::try_from(index).ok(),
            offset: Some(format!("0x{index:x}")),
            native_offset: None,
            name: Some(format!("_{index}")),
            name_source: Some("synthetic-offset".to_owned()),
            name_proven: Some(false),
            name_evidence: None,
            native_type: None,
            type_id: None,
            type_id_source: None,
            type_identity_proven: false,
            type_identity_source: None,
            wire_shape: Some(product.to_owned()),
            wire_shape_source: Some("cfg-multi-helper-alternate-spelling-collapse".to_owned()),
            wire_layout: Some(product.to_owned()),
            wire_layout_source: Some("cfg-multi-helper-alternate-spelling-collapse".to_owned()),
            byte_width: None,
            wire_ordinal: u32::try_from(index).ok(),
            wire_order_source: Some("cfg-multi-helper-alternate-spelling-collapse".to_owned()),
            callsite: callsite.clone(),
            target: None,
            target_name: None,
            type_conflict: false,
        })
        .collect();
    shape.wire_order_source = Some("cfg-multi-helper-alternate-spelling-collapse".to_owned());
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

pub(crate) fn nested_type_shape_members_in_wire_order(
    shape: &NetworkNestedTypeShape,
) -> Option<Vec<&crate::network_schema::NetworkNestedTypeMember>> {
    if shape.wire_order_proven != Some(true) {
        return None;
    }

    let mut members = shape.members.iter().collect::<Vec<_>>();
    if members.iter().all(|member| member.wire_ordinal.is_none()) {
        return Some(members);
    }
    members.sort_by_key(|member| member.wire_ordinal);
    members
        .iter()
        .enumerate()
        .all(|(index, member)| member.wire_ordinal == u32::try_from(index).ok())
        .then_some(members)
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
        | NetworkMemberWireShape::BooleanChoice { .. }
        | NetworkMemberWireShape::BitMaskComposite { .. } => None,
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
