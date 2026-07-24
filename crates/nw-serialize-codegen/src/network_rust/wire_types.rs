use super::*;

pub(super) struct RustFieldShape {
    pub(super) value_type: String,
    pub(super) field_type: String,
    pub(super) container_key_type_shape: Option<crate::network_schema::NetworkNestedTypeShape>,
    pub(super) container_embedded_key_type_shapes:
        Vec<crate::network_schema::NetworkNestedTypeShape>,
    pub(super) container_value_type_shape: Option<crate::network_schema::NetworkNestedTypeShape>,
    pub(super) container_embedded_value_type_shapes:
        Vec<crate::network_schema::NetworkNestedTypeShape>,
}

pub(super) fn rust_field_shape(shape: &SchemaWireShape) -> RustFieldShape {
    match shape {
        SchemaWireShape::Bool => rust_field_shape_static("bool", "ReplicatedFieldHandler<bool>"),
        SchemaWireShape::U8 => rust_field_shape_static("u8", "ReplicatedFieldHandler<u8>"),
        SchemaWireShape::U16 => rust_field_shape_static("u16", "ReplicatedFieldHandler<u16>"),
        SchemaWireShape::U32 => rust_field_shape_static("u32", "ReplicatedFieldHandler<u32>"),
        SchemaWireShape::U64 => rust_field_shape_static("u64", "ReplicatedFieldHandler<u64>"),
        SchemaWireShape::F32 => rust_field_shape_static("f32", "ReplicatedFieldHandler<f32>"),
        SchemaWireShape::F64 => rust_field_shape_static("f64", "ReplicatedFieldHandler<f64>"),
        SchemaWireShape::HalfF32 => {
            rust_field_shape_static("f32", "ReplicatedFieldHandler<f32, HalfF32Marshaler>")
        }
        SchemaWireShape::VlqU32 => {
            rust_field_shape_static("u32", "ReplicatedFieldHandler<u32, VlqU32Marshaler>")
        }
        SchemaWireShape::VlqU64 => {
            rust_field_shape_static("u64", "ReplicatedFieldHandler<u64, VlqU64Marshaler>")
        }
        SchemaWireShape::SequenceNumber => rust_field_shape_static(
            "::nw_network::SequenceNumber",
            "ReplicatedFieldHandler<::nw_network::SequenceNumber>",
        ),
        SchemaWireShape::Vec2 => {
            rust_field_shape_static("::glam::Vec2", "ReplicatedFieldHandler<::glam::Vec2>")
        }
        SchemaWireShape::Vec3 => {
            rust_field_shape_static("::glam::Vec3", "ReplicatedFieldHandler<::glam::Vec3>")
        }
        SchemaWireShape::Vec4 => {
            rust_field_shape_static("::glam::Vec4", "ReplicatedFieldHandler<::glam::Vec4>")
        }
        SchemaWireShape::Quat => {
            rust_field_shape_static("::glam::Quat", "ReplicatedFieldHandler<::glam::Quat>")
        }
        SchemaWireShape::QuatCompNorm => {
            rust_field_shape_static("QuatCompNorm", "ReplicatedFieldHandler<QuatCompNorm>")
        }
        SchemaWireShape::Vec2Comp => rust_field_shape_static(
            "::glam::Vec2",
            "ReplicatedFieldHandler<::glam::Vec2, Vec2CompMarshaler>",
        ),
        SchemaWireShape::Vec3Comp => rust_field_shape_static(
            "::glam::Vec3",
            "ReplicatedFieldHandler<::glam::Vec3, Vec3CompMarshaler>",
        ),
        SchemaWireShape::Vec3CompNorm => rust_field_shape_static(
            "::glam::Vec3",
            "ReplicatedFieldHandler<::glam::Vec3, Vec3CompNormMarshaler>",
        ),
        SchemaWireShape::Vec3SmallestThree => rust_field_shape_static(
            "::glam::Vec3",
            "ReplicatedFieldHandler<::glam::Vec3, PackedNormalizedVec3Marshaller>",
        ),
        SchemaWireShape::QuatComp => rust_field_shape_static(
            "::glam::Quat",
            "ReplicatedFieldHandler<::glam::Quat, QuatCompMarshaler>",
        ),
        SchemaWireShape::QuatSmallestThree => rust_field_shape_static(
            "::glam::Quat",
            "ReplicatedFieldHandler<::glam::Quat, QuatSmallestThreeQuantizedMarshaler>",
        ),
        SchemaWireShape::NonUniformScaleComp => rust_field_shape_static(
            "::glam::Vec3",
            "ReplicatedFieldHandler<::glam::Vec3, NonUniformScaleCompMarshaler>",
        ),
        SchemaWireShape::DeltaVec3(range) => RustFieldShape {
            value_type: "::glam::Vec3".to_owned(),
            field_type: format!(
                "ReplicatedFieldHandler<::glam::Vec3, DeltaMarshaler<{range}, ::glam::Vec3>>"
            ),
            container_key_type_shape: None,
            container_embedded_key_type_shapes: Vec::new(),
            container_value_type_shape: None,
            container_embedded_value_type_shapes: Vec::new(),
        },
        SchemaWireShape::RemoteServerGdeRef => rust_field_shape_static(
            "::nw_network::source::RemoteServerGDERef",
            "ReplicatedFieldHandler<::nw_network::source::RemoteServerGDERef, RemoteServerGdeRefMarshaler>",
        ),
        SchemaWireShape::PackedPosition(shape) => RustFieldShape {
            value_type: "::glam::Vec3".to_owned(),
            field_type: format!(
                "ReplicatedFieldHandler<::glam::Vec3, PackedPositionMarshaller<0x{:08x}, 0x{:08x}>>",
                shape.minimum_bits, shape.maximum_bits
            ),
            container_key_type_shape: None,
            container_embedded_key_type_shapes: Vec::new(),
            container_value_type_shape: None,
            container_embedded_value_type_shapes: Vec::new(),
        },
        SchemaWireShape::TransformCompressor => rust_field_shape_static(
            "::glam::Affine3A",
            "ReplicatedFieldHandler<::glam::Affine3A, TransformCompressor>",
        ),
        SchemaWireShape::PackedSize => {
            rust_field_shape_static("PackedSize", "ReplicatedFieldHandler<PackedSize>")
        }
        SchemaWireShape::Mat3 => {
            rust_field_shape_static("::glam::Mat3", "ReplicatedFieldHandler<::glam::Mat3>")
        }
        SchemaWireShape::Affine3 => rust_field_shape_static(
            "::glam::Affine3A",
            "ReplicatedFieldHandler<::glam::Affine3A>",
        ),
        SchemaWireShape::Aabb2d => rust_field_shape_static(
            "::bevy_math::bounding::Aabb2d",
            "ReplicatedFieldHandler<::bevy_math::bounding::Aabb2d>",
        ),
        SchemaWireShape::Aabb3d => rust_field_shape_static(
            "::bevy_math::bounding::Aabb3d",
            "ReplicatedFieldHandler<::bevy_math::bounding::Aabb3d>",
        ),
        SchemaWireShape::ActorRef => rust_field_shape_static(
            "::nw_network::ActorRef",
            "ReplicatedFieldHandler<::nw_network::ActorRef>",
        ),
        SchemaWireShape::EntityRef => rust_field_shape_static(
            "::nw_network::EntityRef",
            "ReplicatedFieldHandler<::nw_network::EntityRef>",
        ),
        SchemaWireShape::FixedBytes(len) => RustFieldShape {
            value_type: format!("[u8; {len}]"),
            field_type: format!("ReplicatedFieldHandler<[u8; {len}]>"),
            container_key_type_shape: None,
            container_embedded_key_type_shapes: Vec::new(),
            container_value_type_shape: None,
            container_embedded_value_type_shapes: Vec::new(),
        },
        SchemaWireShape::Bytes => {
            rust_field_shape_static("Vec<u8>", "ReplicatedFieldHandler<Vec<u8>>")
        }
        SchemaWireShape::String => {
            rust_field_shape_static("String", "ReplicatedFieldHandler<String>")
        }
        SchemaWireShape::ClassValue => rust_field_shape_static(
            "::nw_network::serialize::ClassValue",
            "ReplicatedFieldHandler<::nw_network::serialize::ClassValue>",
        ),
        SchemaWireShape::Composite(members) => {
            let (value_type, codec) = composite_projection(members);
            let field_type = replicated_field_type(&value_type, codec.as_deref());
            RustFieldShape {
                value_type,
                field_type,
                container_key_type_shape: None,
                container_embedded_key_type_shapes: Vec::new(),
                container_value_type_shape: None,
                container_embedded_value_type_shapes: Vec::new(),
            }
        }
        SchemaWireShape::Optional(inner) => {
            let inner_shape = inner.as_ref();
            let inner = rust_field_shape(inner_shape);
            let value_type = format!("::core::option::Option<{}>", inner.value_type);
            let codec = wire_shape_codec_type(inner_shape);
            let codec =
                codec.map(|codec| format!("::nw_network::serialize::OptionalCodec<{codec}>"));
            let field_type = replicated_field_type(&value_type, codec.as_deref());
            RustFieldShape {
                value_type,
                field_type,
                container_key_type_shape: inner.container_key_type_shape,
                container_embedded_key_type_shapes: inner.container_embedded_key_type_shapes,
                container_value_type_shape: inner.container_value_type_shape,
                container_embedded_value_type_shapes: inner.container_embedded_value_type_shapes,
            }
        }
        SchemaWireShape::DefaultOmitted(members) => {
            let (value_type, codec) = default_omitted_projection(members);
            let field_type = replicated_field_type(&value_type, Some(&codec));
            RustFieldShape {
                value_type,
                field_type,
                container_key_type_shape: None,
                container_embedded_key_type_shapes: Vec::new(),
                container_value_type_shape: None,
                container_embedded_value_type_shapes: Vec::new(),
            }
        }
        SchemaWireShape::BooleanChoice(choice) => {
            let false_value = rust_field_shape(&choice.false_value);
            let true_value = rust_field_shape(&choice.true_value);
            let value_type = format!(
                "::nw_network::serialize::BooleanChoice<{}, {}>",
                false_value.value_type, true_value.value_type
            );
            let codec = format!(
                "::nw_network::serialize::BooleanChoiceCodec<{}, {}>",
                codec_type_or_default(&choice.false_value, &false_value.value_type),
                codec_type_or_default(&choice.true_value, &true_value.value_type)
            );
            let field_type = replicated_field_type(&value_type, Some(&codec));
            RustFieldShape {
                value_type,
                field_type,
                container_key_type_shape: None,
                container_embedded_key_type_shapes: Vec::new(),
                container_value_type_shape: None,
                container_embedded_value_type_shapes: Vec::new(),
            }
        }
        SchemaWireShape::Sequence(inner) => {
            let inner_shape = rust_field_shape(inner);
            let value_type = format!("::std::vec::Vec<{}>", inner_shape.value_type);
            let codec = wire_shape_codec_type(inner)
                .map(|codec| format!("::nw_network::serialize::SequenceCodec<{codec}>"));
            let field_type = replicated_field_type(&value_type, codec.as_deref());
            RustFieldShape {
                value_type,
                field_type,
                container_key_type_shape: None,
                container_embedded_key_type_shapes: Vec::new(),
                container_value_type_shape: None,
                container_embedded_value_type_shapes: Vec::new(),
            }
        }
        SchemaWireShape::Set(inner) => {
            let inner_shape = rust_field_shape(inner);
            let value_type = format!(
                "::nw_network::serialize::IndexSet<{}>",
                inner_shape.value_type
            );
            let codec = wire_shape_codec_type(inner).map(|codec| {
                format!(
                    "::nw_network::serialize::ContainerMarshaler<{}, {codec}>",
                    inner_shape.value_type
                )
            });
            let field_type = replicated_field_type(&value_type, codec.as_deref());
            RustFieldShape {
                value_type,
                field_type,
                container_key_type_shape: None,
                container_embedded_key_type_shapes: Vec::new(),
                container_value_type_shape: None,
                container_embedded_value_type_shapes: Vec::new(),
            }
        }
        SchemaWireShape::Map { key, value } => {
            let key_shape = rust_field_shape(key);
            let value_shape = rust_field_shape(value);
            let value_type = format!(
                "::nw_network::serialize::IndexMap<{}, {}>",
                key_shape.value_type, value_shape.value_type
            );
            let codec = map_sequence_codec_type(key, value, &key_shape, &value_shape);
            let field_type = replicated_field_type(&value_type, codec.as_deref());
            RustFieldShape {
                value_type,
                field_type,
                container_key_type_shape: None,
                container_embedded_key_type_shapes: Vec::new(),
                container_value_type_shape: None,
                container_embedded_value_type_shapes: Vec::new(),
            }
        }
        SchemaWireShape::ReplicatedContainer(container) => {
            replicated_container_field_shape(*container)
        }
        SchemaWireShape::FixedSequence(_) => {
            unreachable!("fixed sequences require their proven handler plan")
        }
    }
}

pub(super) fn wire_shape_codec_type(shape: &SchemaWireShape) -> Option<String> {
    match shape {
        SchemaWireShape::HalfF32 => Some("::nw_network::serialize::HalfF32Marshaler".to_owned()),
        SchemaWireShape::VlqU32 => Some("::nw_network::serialize::VlqU32Marshaler".to_owned()),
        SchemaWireShape::VlqU64 => Some("::nw_network::serialize::VlqU64Marshaler".to_owned()),
        SchemaWireShape::Vec2Comp => Some("::nw_network::serialize::Vec2CompMarshaler".to_owned()),
        SchemaWireShape::Vec3Comp => Some("::nw_network::serialize::Vec3CompMarshaler".to_owned()),
        SchemaWireShape::Vec3CompNorm => {
            Some("::nw_network::serialize::Vec3CompNormMarshaler".to_owned())
        }
        SchemaWireShape::Vec3SmallestThree => {
            Some("::nw_network::serialize::PackedNormalizedVec3Marshaller".to_owned())
        }
        SchemaWireShape::QuatComp => Some("::nw_network::serialize::QuatCompMarshaler".to_owned()),
        SchemaWireShape::QuatSmallestThree => {
            Some("::nw_network::serialize::QuatSmallestThreeQuantizedMarshaler".to_owned())
        }
        SchemaWireShape::NonUniformScaleComp => {
            Some("::nw_network::serialize::NonUniformScaleCompMarshaler".to_owned())
        }
        SchemaWireShape::DeltaVec3(range) => Some(format!(
            "::nw_network::serialize::DeltaMarshaler<{range}, ::glam::Vec3>"
        )),
        SchemaWireShape::RemoteServerGdeRef => {
            Some("::nw_network::serialize::RemoteServerGdeRefMarshaler".to_owned())
        }
        SchemaWireShape::PackedPosition(shape) => Some(format!(
            "::nw_network::serialize::PackedPositionMarshaller<0x{:08x}, 0x{:08x}>",
            shape.minimum_bits, shape.maximum_bits
        )),
        SchemaWireShape::TransformCompressor => {
            Some("::nw_network::serialize::TransformCompressor".to_owned())
        }
        SchemaWireShape::Composite(members) => composite_projection(members).1,
        SchemaWireShape::Optional(inner) => wire_shape_codec_type(inner)
            .map(|codec| format!("::nw_network::serialize::OptionalCodec<{codec}>")),
        SchemaWireShape::DefaultOmitted(members) => Some(default_omitted_projection(members).1),
        SchemaWireShape::BooleanChoice(choice) => {
            let false_value = rust_field_shape(&choice.false_value).value_type;
            let true_value = rust_field_shape(&choice.true_value).value_type;
            Some(format!(
                "::nw_network::serialize::BooleanChoiceCodec<{}, {}>",
                codec_type_or_default(&choice.false_value, &false_value),
                codec_type_or_default(&choice.true_value, &true_value)
            ))
        }
        SchemaWireShape::Sequence(inner) => wire_shape_codec_type(inner)
            .map(|codec| format!("::nw_network::serialize::SequenceCodec<{codec}>")),
        SchemaWireShape::Set(inner) => {
            let inner_shape = rust_field_shape(inner);
            wire_shape_codec_type(inner).map(|codec| {
                format!(
                    "::nw_network::serialize::ContainerMarshaler<{}, {codec}>",
                    inner_shape.value_type
                )
            })
        }
        SchemaWireShape::Map { key, value } => {
            let key_shape = rust_field_shape(key);
            let value_shape = rust_field_shape(value);
            map_sequence_codec_type(key, value, &key_shape, &value_shape)
        }
        _ => None,
    }
}

fn map_sequence_codec_type(
    key: &SchemaWireShape,
    value: &SchemaWireShape,
    key_shape: &RustFieldShape,
    value_shape: &RustFieldShape,
) -> Option<String> {
    let key_codec = wire_shape_codec_type(key);
    let value_codec = wire_shape_codec_type(value);
    (key_codec.is_some() || value_codec.is_some()).then(|| {
        format!(
            "::nw_network::serialize::MapSequenceCodec<{}, {}>",
            key_codec.unwrap_or_else(|| format!(
                "::nw_network::serialize::DefaultMarshaler<{}>",
                key_shape.value_type
            )),
            value_codec.unwrap_or_else(|| format!(
                "::nw_network::serialize::DefaultMarshaler<{}>",
                value_shape.value_type
            ))
        )
    })
}

fn default_omitted_projection(members: &[SchemaWireShape]) -> (String, String) {
    assert!(
        (2..=12).contains(&members.len()),
        "a default-omitted tuple contains between two and twelve members"
    );
    let fields = members.iter().map(rust_field_shape).collect::<Vec<_>>();
    let value_type = tuple_rust_type(
        &fields
            .iter()
            .map(|field| field.value_type.clone())
            .collect::<Vec<_>>(),
    );
    let codecs = members
        .iter()
        .zip(&fields)
        .map(|(shape, field)| codec_type_or_default(shape, &field.value_type))
        .collect::<Vec<_>>();
    let codec = format!(
        "::nw_network::serialize::DefaultOmittedTupleCodec<{}>",
        tuple_rust_type(&codecs)
    );
    (value_type, codec)
}

fn codec_type_or_default(shape: &SchemaWireShape, value_type: &str) -> String {
    wire_shape_codec_type(shape)
        .unwrap_or_else(|| format!("::nw_network::serialize::DefaultMarshaler<{value_type}>"))
}

fn composite_projection(members: &[SchemaWireShape]) -> (String, Option<String>) {
    let members = members
        .iter()
        .map(|shape| {
            let field = rust_field_shape(shape);
            (field.value_type, wire_shape_codec_type(shape))
        })
        .collect::<Vec<_>>();
    tuple_projection(&members)
}

pub(super) fn tuple_projection(members: &[(String, Option<String>)]) -> (String, Option<String>) {
    assert!(
        members.len() >= 2,
        "a proven composite wire shape has at least two members"
    );
    let grouped;
    let members = if members.len() <= 12 {
        members
    } else {
        grouped = members[..11]
            .iter()
            .cloned()
            .chain(std::iter::once(tuple_projection(&members[11..])))
            .collect::<Vec<_>>();
        &grouped
    };
    let value_type = tuple_rust_type(
        &members
            .iter()
            .map(|(value_type, _)| value_type.clone())
            .collect::<Vec<_>>(),
    );
    let has_custom_codec = members.iter().any(|(_, codec)| codec.is_some());
    let codec = has_custom_codec.then(|| {
        let codecs = members
            .iter()
            .map(|(value_type, codec)| {
                codec.clone().unwrap_or_else(|| {
                    format!("::nw_network::serialize::DefaultMarshaler<{value_type}>")
                })
            })
            .collect::<Vec<_>>();
        format!(
            "::nw_network::serialize::TupleCodec<{}>",
            tuple_rust_type(&codecs)
        )
    });
    (value_type, codec)
}

fn tuple_rust_type(members: &[String]) -> String {
    assert!(
        (2..=12).contains(&members.len()),
        "a projected tuple contains between two and twelve members"
    );
    format!("({})", members.join(", "))
}

fn replicated_field_type(value_type: &str, codec: Option<&str>) -> String {
    codec.map_or_else(
        || format!("::nw_network::serialize::ReplicatedFieldHandler<{value_type}>"),
        |codec| format!("::nw_network::serialize::ReplicatedFieldHandler<{value_type}, {codec}>"),
    )
}

pub(super) fn rust_field_shape_static(
    value_type: &'static str,
    field_type: &'static str,
) -> RustFieldShape {
    RustFieldShape {
        value_type: value_type.to_owned(),
        field_type: field_type.to_owned(),
        container_key_type_shape: None,
        container_embedded_key_type_shapes: Vec::new(),
        container_value_type_shape: None,
        container_embedded_value_type_shapes: Vec::new(),
    }
}

pub(super) fn replicated_container_field_shape(
    container: NetworkReplicatedContainerWireShape,
) -> RustFieldShape {
    let key_type = scalar_rust_type(container.key);
    let value_type = scalar_rust_type(container.value);
    let collection_type = index_map_type(&key_type, &value_type);
    let key_marshaler = scalar_marshaler_type(container.key);
    let value_marshaler = scalar_marshaler_type(container.value);
    let field_type = format!(
        "::nw_network::serialize::ReplicatedContainer<{collection_type}, {{ ::nw_network::serialize::WIRE_VEC_CAP }}, {key_marshaler}, {value_marshaler}>"
    );
    RustFieldShape {
        value_type: collection_type,
        field_type,
        container_key_type_shape: None,
        container_embedded_key_type_shapes: Vec::new(),
        container_value_type_shape: None,
        container_embedded_value_type_shapes: Vec::new(),
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum ContainerPlanError {
    MissingPlan,
    NonLinearCodec,
    MissingKeyType,
    MissingValueType,
}

impl ContainerPlanError {
    pub(super) const fn as_str(self) -> &'static str {
        match self {
            Self::MissingPlan => "missing-container-plan",
            Self::NonLinearCodec => "non-linear-container-codec",
            Self::MissingKeyType => "missing-container-key-type",
            Self::MissingValueType => "missing-container-value-type",
        }
    }
}
