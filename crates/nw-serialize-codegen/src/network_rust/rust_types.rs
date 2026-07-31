use super::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct NetworkNativeScalarType {
    pub(super) rust_type: &'static str,
    pub(super) wire_shape: SchemaWireScalarShape,
}

pub(super) fn serialize_source_rust_type_name(name: &str) -> Option<String> {
    let rust_type = format!("::nw_network::source::{}", rust_type_ident(name));
    syn::parse_str::<syn::Type>(&rust_type).ok()?;
    Some(rust_type)
}

pub(super) fn network_native_type_rust_type(
    native_type: &str,
    serialize_types: &BTreeMap<Uuid, &NetworkSerializeType>,
) -> Option<String> {
    let native_type = normalized_cpp_value_type(native_type)?;
    if let Some(rust_type) = exact_native_runtime_rust_type(&native_type) {
        return Some(rust_type.to_owned());
    }
    if let Some((template, arguments)) = cpp_template(&native_type) {
        return match (template, arguments.as_slice()) {
            ("AZStd::vector" | "std::vector", [element, ..]) => Some(format!(
                "::std::vec::Vec<{}>",
                network_native_type_rust_type(element, serialize_types)?
            )),
            ("AZStd::unordered_set" | "std::unordered_set", [element, ..]) => Some(format!(
                "::nw_network::serialize::IndexSet<{}>",
                network_native_type_rust_type(element, serialize_types)?
            )),
            (
                "AZStd::unordered_map" | "AZStd::unordered_flat_map" | "std::unordered_map",
                [key, value, ..],
            ) => Some(format!(
                "::nw_network::serialize::IndexMap<{}, {}>",
                network_native_type_rust_type(key, serialize_types)?,
                network_native_type_rust_type(value, serialize_types)?,
            )),
            ("AZStd::map" | "std::map", [key, value, ..]) => Some(format!(
                "::std::collections::BTreeMap<{}, {}>",
                network_native_type_rust_type(key, serialize_types)?,
                network_native_type_rust_type(value, serialize_types)?,
            )),
            ("AZStd::pair" | "std::pair", [first, second]) => Some(format!(
                "({}, {})",
                network_native_type_rust_type(first, serialize_types)?,
                network_native_type_rust_type(second, serialize_types)?
            )),
            ("Amazon::Pervasives::Maybe" | "AZStd::optional" | "std::optional", [value]) => {
                Some(format!(
                    "::core::option::Option<{}>",
                    network_native_type_rust_type(value, serialize_types)?
                ))
            }
            ("AZStd::array" | "std::array", [value, len]) => {
                let len = len.parse::<usize>().ok()?;
                Some(format!(
                    "[{}; {len}]",
                    network_native_type_rust_type(value, serialize_types)?
                ))
            }
            _ => None,
        };
    }

    if let Some(scalar) = network_native_scalar_type(&native_type) {
        return Some(scalar.rust_type.to_owned());
    }

    let mut candidates = serialize_types.values().filter(|serialize| {
        serialize.name == native_type
            || type_name_leaf(&serialize.name) == type_name_leaf(&native_type)
    });
    let selected = candidates.next()?;
    if candidates.any(|candidate| candidate.type_id != selected.type_id) {
        return None;
    }
    network_serialize_type_rust_type(selected, serialize_types)
}

pub(super) fn exact_native_runtime_rust_type(native_type: &str) -> Option<&'static str> {
    match normalized_cpp_value_type(native_type)?.as_str() {
        "LoginToken" | "Amazon::REP::LoginToken" => Some("::nw_network::LoginToken"),
        "BaselineableFragment" | "Amazon::Hub::BaselineableFragment" => {
            Some("::nw_network::hub::BaselineableFragment")
        }
        _ => None,
    }
}

pub(super) fn network_native_scalar_type(native_type: &str) -> Option<NetworkNativeScalarType> {
    let native_type = normalized_cpp_value_type(native_type)?;
    let (rust_type, wire_shape) = match native_type.as_str() {
        "bool" => ("bool", SchemaWireScalarShape::Bool),
        "AZ::s8" | "signed char" | "int8_t" => ("i8", SchemaWireScalarShape::U8),
        "AZ::s16" | "short" | "int16_t" => ("i16", SchemaWireScalarShape::U16),
        "AZ::s32" | "int" | "int32_t" => ("i32", SchemaWireScalarShape::U32),
        "AZ::s64" | "long long" | "int64_t" => ("i64", SchemaWireScalarShape::U64),
        "AZ::u8" | "unsigned char" | "uint8_t" | "u8" => ("u8", SchemaWireScalarShape::U8),
        "AZ::u16" | "unsigned short" | "uint16_t" | "u16" => ("u16", SchemaWireScalarShape::U16),
        "AZ::u32" | "unsigned int" | "uint32_t" | "u32" => ("u32", SchemaWireScalarShape::U32),
        "AZ::u64" | "unsigned long long" | "uint64_t" | "u64" => {
            ("u64", SchemaWireScalarShape::U64)
        }
        "float" | "f32" => ("f32", SchemaWireScalarShape::F32),
        "double" | "f64" => ("f64", SchemaWireScalarShape::F64),
        "AZStd::string" | "std::string" | "string" => {
            ("::std::string::String", SchemaWireScalarShape::String)
        }
        "AZ::Uuid" => ("::uuid::Uuid", SchemaWireScalarShape::FixedBytes(16)),
        "ActorRef" | "Amazon::Hub::ActorRef" | "HubAddress" | "ProxyAddress" => {
            ("::nw_network::ActorRef", SchemaWireScalarShape::ActorRef)
        }
        "EntityRef" => ("::nw_network::EntityRef", SchemaWireScalarShape::EntityRef),
        "FragmentKey" | "Amazon::Hub::FragmentKey" => {
            ("::nw_network::hub::FragmentKey", SchemaWireScalarShape::U32)
        }
        "SequenceNumber" | "Amazon::Hub::SequenceNumber" => (
            "::nw_network::SequenceNumber",
            SchemaWireScalarShape::SequenceNumber,
        ),
        "Amazon::Pervasives::CrcID" => {
            ("::nw_network::CrcId", SchemaWireScalarShape::FixedBytes(16))
        }
        "AZ::Crc32" => ("::nw_network::Crc32", SchemaWireScalarShape::U32),
        "AZ::EntityId" => ("::nw_network::EntityId", SchemaWireScalarShape::U64),
        "AZ::Vector2" => ("::glam::Vec2", SchemaWireScalarShape::Vec2),
        "AZ::Vector3" => ("::glam::Vec3", SchemaWireScalarShape::Vec3),
        "AZ::Vector4" => ("::glam::Vec4", SchemaWireScalarShape::Vec4),
        "AZ::Quaternion" => ("::glam::Quat", SchemaWireScalarShape::Quat),
        "AZ::Matrix3x3" => ("::glam::Mat3", SchemaWireScalarShape::Mat3),
        "AZ::Transform" => ("::glam::Affine3A", SchemaWireScalarShape::Affine3),
        "AZ::Bounds" => (
            "::bevy_math::bounding::Aabb2d",
            SchemaWireScalarShape::Aabb2d,
        ),
        "AZ::Aabb" => (
            "::bevy_math::bounding::Aabb3d",
            SchemaWireScalarShape::Aabb3d,
        ),
        _ => return None,
    };
    Some(NetworkNativeScalarType {
        rust_type,
        wire_shape,
    })
}

pub(super) fn normalized_cpp_value_type(value: &str) -> Option<String> {
    let mut value = value.trim();
    for prefix in ["class ", "struct ", "const "] {
        value = value.strip_prefix(prefix).unwrap_or(value).trim();
    }
    value = value.strip_suffix(" const").unwrap_or(value).trim();
    if value.ends_with('*') {
        return None;
    }
    value = value
        .strip_suffix("&&")
        .or_else(|| value.strip_suffix('&'))
        .unwrap_or(value)
        .trim();
    (!value.is_empty()).then(|| value.to_owned())
}

pub(super) fn cpp_template(value: &str) -> Option<(&str, Vec<&str>)> {
    let open = value.find('<')?;
    let close = value.rfind('>')?;
    if close != value.len() - 1 || open == 0 {
        return None;
    }
    let mut arguments = Vec::new();
    let mut depth = 0usize;
    let mut start = open + 1;
    for (relative, byte) in value[open + 1..close].bytes().enumerate() {
        let index = open + 1 + relative;
        match byte {
            b'<' => depth += 1,
            b'>' => depth = depth.checked_sub(1)?,
            b',' if depth == 0 => {
                arguments.push(value[start..index].trim());
                start = index + 1;
            }
            _ => {}
        }
    }
    if depth != 0 {
        return None;
    }
    arguments.push(value[start..close].trim());
    arguments
        .iter()
        .all(|argument| !argument.is_empty())
        .then_some((value[..open].trim(), arguments))
}

pub(super) fn exact_member_rust_type(
    parent: &crate::network_schema::NetworkNestedTypeShape,
    member: &crate::network_schema::NetworkNestedTypeMember,
    serialize_types: &BTreeMap<Uuid, &NetworkSerializeType>,
) -> Option<String> {
    let parent_is_serialize_backed = parent
        .type_id
        .is_some_and(|type_id| serialize_types.contains_key(&type_id));
    if member.type_identity_proven && !parent_is_serialize_backed {
        if let Some(type_id) = member.type_id {
            return exact_type_id_rust_type(type_id)
                .map(ToOwned::to_owned)
                .or_else(|| {
                    serialize_types.get(&type_id).and_then(|serialize| {
                        network_serialize_type_rust_type(serialize, serialize_types)
                    })
                });
        }
        let native_type = member.native_type.as_deref()?;
        let wire_shape = nested_member_wire_shape(member)?;
        let wire_product =
            crate::network_schema::parse::nested_member_wire_shapes(wire_shape, &[])?;
        return exact_symbolic_wire_product_rust_type(native_type, &wire_product)
            .map(ToOwned::to_owned);
    }

    let identity_proven = member.type_identity_proven
        || parent.has_exact_identity()
            && member.type_id_source.as_deref() == Some("serialize-field-for-proven-type");
    if !identity_proven || member.name_proven != Some(true) {
        return None;
    }
    let mut current_type_id = parent.type_id?;
    let member_path = member.name.as_deref()?;
    let member_type_id = member.type_id?;
    let member_offset = parse_native_member_offset(member.offset.as_deref()?)?;
    let path = member_path
        .split('.')
        .map(str::trim)
        .filter(|segment| !segment.is_empty())
        .collect::<Vec<_>>();
    let mut accumulated_offset = 0u32;

    for (index, segment) in path.iter().enumerate() {
        let current_type = serialize_types.get(&current_type_id)?;
        let mut candidates = current_type
            .fields
            .iter()
            .filter(|field| !field.is_base_class && field.name == *segment);
        let field = candidates.next()?;
        if candidates.next().is_some() {
            return None;
        }
        accumulated_offset = accumulated_offset.checked_add(field.offset?)?;
        if index + 1 == path.len() {
            if field.type_id != member_type_id || accumulated_offset != member_offset {
                return None;
            }
            return network_resolved_type_rust_type(&field.resolved_type, serialize_types);
        }
        current_type_id = field.type_id;
    }
    None
}

pub(super) fn exact_symbolic_wire_product_rust_type(
    native_type: &str,
    wire_product: &[SchemaWireScalarShape],
) -> Option<&'static str> {
    match normalized_cpp_value_type(native_type)?.as_str() {
        "ActorRequestId" | "Javelin::ClientMessages::ActorRequestId"
            if wire_product == [SchemaWireScalarShape::U64, SchemaWireScalarShape::U64] =>
        {
            Some("::nw_network::ActorRequestId")
        }
        _ => None,
    }
}

pub(super) fn parse_native_member_offset(value: &str) -> Option<u32> {
    let value = value.trim();
    value.strip_prefix("0x").map_or_else(
        || value.parse().ok(),
        |hex| u32::from_str_radix(hex, 16).ok(),
    )
}

pub(super) fn network_resolved_type_rust_type(
    resolved: &ResolvedType,
    serialize_types: &BTreeMap<Uuid, &NetworkSerializeType>,
) -> Option<String> {
    network_resolved_type_rust_type_inner(resolved, serialize_types, &mut BTreeSet::new())
}

pub(super) fn network_serialize_type_rust_type(
    serialize: &NetworkSerializeType,
    serialize_types: &BTreeMap<Uuid, &NetworkSerializeType>,
) -> Option<String> {
    if let Some(rust_type) = exact_type_id_rust_type(serialize.type_id) {
        return Some(rust_type.to_owned());
    }
    if let Some(resolved) = &serialize.resolved_type {
        let mut resolving = BTreeSet::from([serialize.type_id]);
        if let Some(rust_type) =
            network_resolved_type_rust_type_inner(resolved, serialize_types, &mut resolving)
        {
            return Some(rust_type);
        }
    }
    serialize
        .emits_source
        .then(|| serialize_source_rust_type_name(&serialize.name))
        .flatten()
}

fn network_resolved_type_rust_type_inner(
    resolved: &ResolvedType,
    serialize_types: &BTreeMap<Uuid, &NetworkSerializeType>,
    resolving: &mut BTreeSet<Uuid>,
) -> Option<String> {
    resolved.unresolved().is_none().then_some(())?;
    let mut type_ids = BTreeSet::new();
    collect_resolved_named_type_ids(resolved, &mut type_ids);
    let names = type_ids
        .into_iter()
        .map(|type_id| {
            let serialize = serialize_types.get(&type_id)?;
            let rust_type = exact_type_id_rust_type(type_id)
                .map(ToOwned::to_owned)
                .or_else(|| {
                    serialize
                        .emits_source
                        .then(|| serialize_source_rust_type_name(&serialize.name))
                        .flatten()
                })
                .or_else(|| {
                    resolving.insert(type_id).then_some(())?;
                    let rust_type = serialize.resolved_type.as_ref().and_then(|resolved| {
                        network_resolved_type_rust_type_inner(resolved, serialize_types, resolving)
                    });
                    resolving.remove(&type_id);
                    rust_type
                })?;
            Some((type_id, rust_type))
        })
        .collect::<Option<BTreeMap<_, _>>>()?;
    let renderer = RustTypeRenderer::new(RustTypeOptions {
        use_support_aliases: true,
        uuid_alias: "::nw_network::source::AzUuid",
        crc32_alias: "::nw_network::Crc32",
        entity_id_alias: "::nw_network::EntityId",
        asset_id_alias: "::nw_network::AssetId",
        asset_alias: "::nw_network::source::Asset",
        uid_alias: "::nw_network::source::AzUuid",
        replicated_field_alias: "::nw_network::serialize::ReplicatedFieldHandler",
    });
    let rust_type = renderer.render_with_names(resolved, &names);
    syn::parse_str::<syn::Type>(&rust_type).ok()?;
    Some(rust_type)
}

pub(super) const fn exact_type_id_rust_type(type_id: Uuid) -> Option<&'static str> {
    match type_id.as_u128() {
        0x3ab0_037f_af8d_48ce_bca0_a170_d18b_2c03 | 0x5842_2c0e_1e47_4854_98e6_3409_8f6f_e12d => {
            Some("i8")
        }
        0xb8a5_6d56_a10d_4dce_9f63_405e_e243_dd3c => Some("i16"),
        0x7203_9442_eb38_4d42_a1ad_cb68_f7e0_eef6 | 0x8f24_b9ad_7c51_46cf_b2f8_2773_5695_7325 => {
            Some("i32")
        }
        0x70d8_a282_a1ea_462d_9d04_51ed_e81f_ac2f => Some("i64"),
        0x72b9_409a_7d1a_4831_9cfe_fcb3_fadd_3426 => Some("u8"),
        0xeca0_b403_c4f8_4b86_95fc_8168_8d04_6e40 => Some("u16"),
        0x43da_906b_7def_4ca8_9790_8541_06d3_f983 | 0x5ec2_d6f7_6859_400f_9215_c106_f5b1_0e53 => {
            Some("u32")
        }
        0xd659_7933_47cd_4fc8_b911_63f3_e2b0_993a => Some("u64"),
        0xea2c_3e90_afbe_44d4_a90d_faaf_79ba_f93d => Some("f32"),
        0x110c_4b14_11a8_4e9d_8638_5051_013a_56ac => Some("f64"),
        0xa0ca_880c_afe4_43cb_926c_59ac_4849_6112 => Some("bool"),
        0xe152_c105_a133_4d03_bbf8_3d4b_2fba_3e2a | 0xdfe5_0973_ea0b_4616_833a_b60b_5e2e_71df => {
            Some("::uuid::Uuid")
        }
        0x3d80_f623_c85c_4741_90d0_e4e6_6164_e6bf => Some("::glam::Vec2"),
        0x8379_eb7d_01fa_4538_b64b_a654_3b4b_e73d => Some("::glam::Vec3"),
        0x0ce9_fa36_1e3a_4c06_9254_b7c7_3a73_2053 => Some("::glam::Vec4"),
        0x7310_3120_3dd3_4873_bab3_9713_fa28_04fb => Some("::glam::Quat"),
        0x15a4_332f_7c3f_4a58_ac35_50e1_ce53_fb9c => Some("::glam::Mat3"),
        0x0e0e_f911_f8e7_4d3c_afaa_68d5_d370_f244 | 0x5d99_58e9_9f1e_4985_b532_fffd_e75f_edfd => {
            Some("::glam::Affine3A")
        }
        0x6383_f1d3_bb27_4e6b_a49a_6409_b205_9eaa => Some("::nw_network::EntityId"),
        0x9f4e_062e_06a0_46d4_85df_e0da_9646_7d3a => Some("::nw_network::Crc32"),
        0x652e_d536_3402_439b_aebe_4a5d_bc55_4085 => Some("::nw_network::AssetId"),
        0x0638_e28c_ab7b_4ba4_84ac_0353_038e_6fdc => Some("::nw_network::ActorRef"),
        0xc148_c555_3264_41f7_a335_e48b_65f9_1728 => Some("::nw_network::ClientRef"),
        0xa54c_2b36_d5b8_46a1_a529_4ebd_bd24_50e7 => Some("::bevy_math::bounding::Aabb3d"),
        _ => None,
    }
}

pub(super) fn scalar_rust_type(shape: SchemaWireScalarShape) -> String {
    match shape {
        SchemaWireScalarShape::Bool => "bool".to_owned(),
        SchemaWireScalarShape::U8 => "u8".to_owned(),
        SchemaWireScalarShape::U16 => "u16".to_owned(),
        SchemaWireScalarShape::U32 | SchemaWireScalarShape::VlqU32 => "u32".to_owned(),
        SchemaWireScalarShape::U64 | SchemaWireScalarShape::VlqU64 => "u64".to_owned(),
        SchemaWireScalarShape::F32 | SchemaWireScalarShape::HalfF32 => "f32".to_owned(),
        SchemaWireScalarShape::F64 => "f64".to_owned(),
        SchemaWireScalarShape::SequenceNumber => "::nw_network::SequenceNumber".to_owned(),
        SchemaWireScalarShape::Vec2 => "::glam::Vec2".to_owned(),
        SchemaWireScalarShape::Vec3 => "::glam::Vec3".to_owned(),
        SchemaWireScalarShape::Vec4 => "::glam::Vec4".to_owned(),
        SchemaWireScalarShape::Quat => "::glam::Quat".to_owned(),
        SchemaWireScalarShape::QuatCompNorm => "::nw_network::serialize::QuatCompNorm".to_owned(),
        SchemaWireScalarShape::Vec2Comp => "::glam::Vec2".to_owned(),
        SchemaWireScalarShape::Vec3Comp
        | SchemaWireScalarShape::Vec3CompNorm
        | SchemaWireScalarShape::Vec3SmallestThree
        | SchemaWireScalarShape::NonUniformScaleComp
        | SchemaWireScalarShape::DeltaVec3(_) => "::glam::Vec3".to_owned(),
        SchemaWireScalarShape::RemoteServerGdeRef => {
            "::nw_network::source::RemoteServerGDERef".to_owned()
        }
        SchemaWireScalarShape::QuatComp | SchemaWireScalarShape::QuatSmallestThree => {
            "::glam::Quat".to_owned()
        }
        SchemaWireScalarShape::PackedPosition(_) => "::glam::Vec3".to_owned(),
        SchemaWireScalarShape::TransformCompressor => "::glam::Affine3A".to_owned(),
        SchemaWireScalarShape::PackedSize => "::nw_network::serialize::PackedSize".to_owned(),
        SchemaWireScalarShape::Mat3 => "::glam::Mat3".to_owned(),
        SchemaWireScalarShape::Affine3 => "::glam::Affine3A".to_owned(),
        SchemaWireScalarShape::Aabb2d => "::bevy_math::bounding::Aabb2d".to_owned(),
        SchemaWireScalarShape::Aabb3d => "::bevy_math::bounding::Aabb3d".to_owned(),
        SchemaWireScalarShape::ActorRef => "::nw_network::ActorRef".to_owned(),
        SchemaWireScalarShape::EntityRef => "::nw_network::EntityRef".to_owned(),
        SchemaWireScalarShape::FixedBytes(len) => format!("[u8; {len}]"),
        SchemaWireScalarShape::Bytes => "Vec<u8>".to_owned(),
        SchemaWireScalarShape::String => "String".to_owned(),
    }
}

pub(super) fn scalar_marshaler_type(shape: SchemaWireScalarShape) -> String {
    match shape {
        SchemaWireScalarShape::HalfF32 => "::nw_network::serialize::HalfF32Marshaler".to_owned(),
        SchemaWireScalarShape::VlqU32 => "::nw_network::serialize::VlqU32Marshaler".to_owned(),
        SchemaWireScalarShape::VlqU64 => "::nw_network::serialize::VlqU64Marshaler".to_owned(),
        SchemaWireScalarShape::Vec2Comp => "::nw_network::serialize::Vec2CompMarshaler".to_owned(),
        SchemaWireScalarShape::Vec3Comp => "::nw_network::serialize::Vec3CompMarshaler".to_owned(),
        SchemaWireScalarShape::Vec3CompNorm => {
            "::nw_network::serialize::Vec3CompNormMarshaler".to_owned()
        }
        SchemaWireScalarShape::Vec3SmallestThree => {
            "::nw_network::serialize::PackedNormalizedVec3Marshaller".to_owned()
        }
        SchemaWireScalarShape::QuatComp => "::nw_network::serialize::QuatCompMarshaler".to_owned(),
        SchemaWireScalarShape::QuatSmallestThree => {
            "::nw_network::serialize::QuatSmallestThreeQuantizedMarshaler".to_owned()
        }
        SchemaWireScalarShape::NonUniformScaleComp => {
            "::nw_network::serialize::NonUniformScaleCompMarshaler".to_owned()
        }
        SchemaWireScalarShape::DeltaVec3(range) => {
            format!("::nw_network::serialize::DeltaMarshaler<{range}, ::glam::Vec3>")
        }
        SchemaWireScalarShape::RemoteServerGdeRef => {
            "::nw_network::serialize::RemoteServerGdeRefMarshaler".to_owned()
        }
        SchemaWireScalarShape::PackedPosition(shape) => format!(
            "::nw_network::serialize::PackedPositionMarshaller<0x{:08x}, 0x{:08x}>",
            shape.minimum_bits, shape.maximum_bits
        ),
        SchemaWireScalarShape::TransformCompressor => {
            "::nw_network::serialize::TransformCompressor".to_owned()
        }
        _ => {
            let rust_type = scalar_rust_type(shape);
            format!("::nw_network::serialize::DefaultMarshaler<{rust_type}>")
        }
    }
}

pub(super) fn scalar_marshaler_type_for_value(
    shape: SchemaWireScalarShape,
    rust_type: &str,
) -> String {
    match shape {
        SchemaWireScalarShape::Bool
        | SchemaWireScalarShape::U8
        | SchemaWireScalarShape::U16
        | SchemaWireScalarShape::U32
        | SchemaWireScalarShape::U64
        | SchemaWireScalarShape::F32
        | SchemaWireScalarShape::F64
        | SchemaWireScalarShape::Vec2
        | SchemaWireScalarShape::Vec3
        | SchemaWireScalarShape::Vec4
        | SchemaWireScalarShape::Quat
        | SchemaWireScalarShape::Mat3
        | SchemaWireScalarShape::Affine3
        | SchemaWireScalarShape::Aabb2d
        | SchemaWireScalarShape::Aabb3d
        | SchemaWireScalarShape::ActorRef
        | SchemaWireScalarShape::EntityRef
        | SchemaWireScalarShape::FixedBytes(_)
        | SchemaWireScalarShape::Bytes
        | SchemaWireScalarShape::String => {
            format!("::nw_network::serialize::DefaultMarshaler<{rust_type}>")
        }
        _ => scalar_marshaler_type(shape),
    }
}

pub(super) fn replicated_field_handler_type(shape: &SchemaWireShape, rust_type: &str) -> String {
    if let Some(conversion) = conversion_marshal_type_string_for(shape, rust_type) {
        return format!(
            "::nw_network::serialize::ReplicatedFieldHandler<{rust_type}, {conversion}>"
        );
    }
    format!("::nw_network::serialize::ReplicatedFieldHandler<{rust_type}>")
}

pub(super) fn is_replicated_state_field_type(rust_type: &str) -> bool {
    if syn::parse_str::<syn::Type>(rust_type).is_err() {
        return false;
    }
    let rust_type = rust_type.trim().trim_start_matches("::");
    [
        "nw_network::serialize::ReplicatedFieldHandler",
        "nw_network::serialize::ReplicatedContainer",
    ]
    .into_iter()
    .any(|prefix| rust_type == prefix || rust_type.starts_with(&format!("{prefix}<")))
}
