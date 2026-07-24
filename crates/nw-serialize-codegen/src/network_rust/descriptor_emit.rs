use super::*;

pub(super) fn count_capabilities(
    capabilities: &[NetworkTypeCapability],
    report: &mut NetworkRustGenerationReport,
) {
    if capabilities.contains(&NetworkTypeCapability::ReplicatedState) {
        report.replicated_state_count += 1;
    }
    if capabilities.contains(&NetworkTypeCapability::DirectMessage) {
        report.message_count += 1;
    }
    if capabilities.contains(&NetworkTypeCapability::RegisteredFields) {
        report.field_registered_count += 1;
    }
    if capabilities.contains(&NetworkTypeCapability::SupportData) {
        report.support_type_count += 1;
    }
}

pub(super) fn capability_slice_tokens(
    capabilities: &[NetworkTypeCapability],
    prefix: Option<proc_macro2::TokenStream>,
) -> proc_macro2::TokenStream {
    let capabilities = capabilities
        .iter()
        .copied()
        .map(|kind| {
            let ident = network_type_capability_ident(kind);
            if let Some(prefix) = &prefix {
                quote!(#prefix NetworkTypeCapability::#ident)
            } else {
                quote!(NetworkTypeCapability::#ident)
            }
        })
        .collect::<Vec<_>>();
    quote!(&[#(#capabilities),*])
}

pub(super) fn network_type_capability_ident(kind: NetworkTypeCapability) -> proc_macro2::Ident {
    match kind {
        NetworkTypeCapability::ReplicatedState => format_ident!("ReplicatedState"),
        NetworkTypeCapability::DirectMessage => format_ident!("DirectMessage"),
        NetworkTypeCapability::RegisteredFields => format_ident!("RegisteredFields"),
        NetworkTypeCapability::SupportData => format_ident!("SupportData"),
    }
}

pub(super) fn confidence_ident(confidence: NetworkConfidence) -> proc_macro2::Ident {
    match confidence {
        NetworkConfidence::Exact => format_ident!("Exact"),
        NetworkConfidence::High => format_ident!("High"),
        NetworkConfidence::Inferred => format_ident!("Inferred"),
        NetworkConfidence::Weak => format_ident!("Weak"),
        NetworkConfidence::Unknown => format_ident!("Unknown"),
    }
}

pub(super) fn wire_shape_tokens(shape: &SchemaWireShape) -> proc_macro2::TokenStream {
    match shape {
        SchemaWireShape::Bool => quote!(NetworkWireShape::Bool),
        SchemaWireShape::U8 => quote!(NetworkWireShape::U8),
        SchemaWireShape::U16 => quote!(NetworkWireShape::U16),
        SchemaWireShape::U32 => quote!(NetworkWireShape::U32),
        SchemaWireShape::U64 => quote!(NetworkWireShape::U64),
        SchemaWireShape::F32 => quote!(NetworkWireShape::F32),
        SchemaWireShape::F64 => quote!(NetworkWireShape::F64),
        SchemaWireShape::HalfF32 => quote!(NetworkWireShape::HalfF32),
        SchemaWireShape::VlqU32 => quote!(NetworkWireShape::VlqU32),
        SchemaWireShape::VlqU64 => quote!(NetworkWireShape::VlqU64),
        SchemaWireShape::SequenceNumber => quote!(NetworkWireShape::SequenceNumber),
        SchemaWireShape::Vec2 => quote!(NetworkWireShape::Vec2),
        SchemaWireShape::Vec3 => quote!(NetworkWireShape::Vec3),
        SchemaWireShape::Vec4 => quote!(NetworkWireShape::Vec4),
        SchemaWireShape::Quat => quote!(NetworkWireShape::Quat),
        SchemaWireShape::QuatCompNorm => quote!(NetworkWireShape::QuatCompNorm),
        SchemaWireShape::Vec2Comp => quote!(NetworkWireShape::Vec2Comp),
        SchemaWireShape::Vec3Comp => quote!(NetworkWireShape::Vec3Comp),
        SchemaWireShape::Vec3CompNorm => quote!(NetworkWireShape::Vec3CompNorm),
        SchemaWireShape::Vec3SmallestThree => quote!(NetworkWireShape::Vec3SmallestThree),
        SchemaWireShape::QuatComp => quote!(NetworkWireShape::QuatComp),
        SchemaWireShape::QuatSmallestThree => quote!(NetworkWireShape::QuatSmallestThree),
        SchemaWireShape::NonUniformScaleComp => quote!(NetworkWireShape::NonUniformScaleComp),
        SchemaWireShape::DeltaVec3(range) => quote!(NetworkWireShape::DeltaVec3(#range)),
        SchemaWireShape::RemoteServerGdeRef => {
            quote!(NetworkWireShape::RemoteServerGdeRef)
        }
        SchemaWireShape::PackedPosition(shape) => {
            let shape = packed_position_wire_shape_tokens(*shape);
            quote!(NetworkWireShape::PackedPosition(#shape))
        }
        SchemaWireShape::TransformCompressor => quote!(NetworkWireShape::TransformCompressor),
        SchemaWireShape::PackedSize => quote!(NetworkWireShape::PackedSize),
        SchemaWireShape::Mat3 => quote!(NetworkWireShape::Mat3),
        SchemaWireShape::Affine3 => quote!(NetworkWireShape::Affine3),
        SchemaWireShape::Aabb2d => quote!(NetworkWireShape::Aabb2d),
        SchemaWireShape::Aabb3d => quote!(NetworkWireShape::Aabb3d),
        SchemaWireShape::ActorRef => quote!(NetworkWireShape::ActorRef),
        SchemaWireShape::EntityRef => quote!(NetworkWireShape::EntityRef),
        SchemaWireShape::FixedBytes(len) => quote!(NetworkWireShape::FixedBytes(#len)),
        SchemaWireShape::Bytes => quote!(NetworkWireShape::Bytes),
        SchemaWireShape::String => quote!(NetworkWireShape::String),
        SchemaWireShape::ClassValue => quote!(NetworkWireShape::ClassValue),
        SchemaWireShape::Composite(members) => {
            let members = members.iter().map(wire_shape_tokens);
            quote!(NetworkWireShape::Composite(&[#(#members),*]))
        }
        SchemaWireShape::Optional(value) => {
            let value = wire_shape_tokens(value);
            quote!(NetworkWireShape::Optional(&#value))
        }
        SchemaWireShape::DefaultOmitted(members) => {
            let members = members.iter().map(wire_shape_tokens);
            quote!(NetworkWireShape::DefaultOmitted(&[#(#members),*]))
        }
        SchemaWireShape::BooleanChoice(choice) => {
            let false_value = wire_shape_tokens(&choice.false_value);
            let true_value = wire_shape_tokens(&choice.true_value);
            quote!(NetworkWireShape::BooleanChoice {
                false_value: &#false_value,
                true_value: &#true_value,
            })
        }
        SchemaWireShape::Sequence(value) => {
            let value = wire_shape_tokens(value);
            quote!(NetworkWireShape::Sequence(&#value))
        }
        SchemaWireShape::Set(value) => {
            let value = wire_shape_tokens(value);
            quote!(NetworkWireShape::Set(&#value))
        }
        SchemaWireShape::Map { key, value } => {
            let key = wire_shape_tokens(key);
            let value = wire_shape_tokens(value);
            quote!(NetworkWireShape::Map {
                key: &#key,
                value: &#value,
            })
        }
        SchemaWireShape::ReplicatedContainer(container) => {
            let container = replicated_container_wire_shape_tokens(*container);
            quote!(NetworkWireShape::ReplicatedContainer(#container))
        }
        SchemaWireShape::FixedSequence(sequence) => {
            let element = wire_scalar_shape_tokens(sequence.element);
            let capacity = sequence.capacity;
            quote!(
                NetworkWireShape::FixedSequence(NetworkFixedSequenceWireShape {
                    element: #element,
                    capacity: #capacity,
                })
            )
        }
    }
}

pub(super) fn replicated_container_wire_shape_tokens(
    container: NetworkReplicatedContainerWireShape,
) -> proc_macro2::TokenStream {
    let key = wire_scalar_shape_tokens(container.key);
    let value = wire_scalar_shape_tokens(container.value);
    quote!(NetworkReplicatedContainerWireShape {
        key: #key,
        value: #value,
    })
}

fn packed_position_wire_shape_tokens(
    shape: NetworkPackedPositionWireShape,
) -> proc_macro2::TokenStream {
    let minimum_bits = shape.minimum_bits;
    let maximum_bits = shape.maximum_bits;
    quote!(NetworkPackedPositionWireShape {
        minimum_bits: #minimum_bits,
        maximum_bits: #maximum_bits,
    })
}

pub(super) fn wire_scalar_shape_tokens(shape: SchemaWireScalarShape) -> proc_macro2::TokenStream {
    match shape {
        SchemaWireScalarShape::Bool => quote!(NetworkWireScalarShape::Bool),
        SchemaWireScalarShape::U8 => quote!(NetworkWireScalarShape::U8),
        SchemaWireScalarShape::U16 => quote!(NetworkWireScalarShape::U16),
        SchemaWireScalarShape::U32 => quote!(NetworkWireScalarShape::U32),
        SchemaWireScalarShape::U64 => quote!(NetworkWireScalarShape::U64),
        SchemaWireScalarShape::F32 => quote!(NetworkWireScalarShape::F32),
        SchemaWireScalarShape::F64 => quote!(NetworkWireScalarShape::F64),
        SchemaWireScalarShape::HalfF32 => quote!(NetworkWireScalarShape::HalfF32),
        SchemaWireScalarShape::VlqU32 => quote!(NetworkWireScalarShape::VlqU32),
        SchemaWireScalarShape::VlqU64 => quote!(NetworkWireScalarShape::VlqU64),
        SchemaWireScalarShape::SequenceNumber => quote!(NetworkWireScalarShape::SequenceNumber),
        SchemaWireScalarShape::Vec2 => quote!(NetworkWireScalarShape::Vec2),
        SchemaWireScalarShape::Vec3 => quote!(NetworkWireScalarShape::Vec3),
        SchemaWireScalarShape::Vec4 => quote!(NetworkWireScalarShape::Vec4),
        SchemaWireScalarShape::Quat => quote!(NetworkWireScalarShape::Quat),
        SchemaWireScalarShape::QuatCompNorm => quote!(NetworkWireScalarShape::QuatCompNorm),
        SchemaWireScalarShape::Vec2Comp => quote!(NetworkWireScalarShape::Vec2Comp),
        SchemaWireScalarShape::Vec3Comp => quote!(NetworkWireScalarShape::Vec3Comp),
        SchemaWireScalarShape::Vec3CompNorm => quote!(NetworkWireScalarShape::Vec3CompNorm),
        SchemaWireScalarShape::Vec3SmallestThree => {
            quote!(NetworkWireScalarShape::Vec3SmallestThree)
        }
        SchemaWireScalarShape::QuatComp => quote!(NetworkWireScalarShape::QuatComp),
        SchemaWireScalarShape::QuatSmallestThree => {
            quote!(NetworkWireScalarShape::QuatSmallestThree)
        }
        SchemaWireScalarShape::NonUniformScaleComp => {
            quote!(NetworkWireScalarShape::NonUniformScaleComp)
        }
        SchemaWireScalarShape::DeltaVec3(range) => {
            quote!(NetworkWireScalarShape::DeltaVec3(#range))
        }
        SchemaWireScalarShape::RemoteServerGdeRef => {
            quote!(NetworkWireScalarShape::RemoteServerGdeRef)
        }
        SchemaWireScalarShape::PackedPosition(shape) => {
            let shape = packed_position_wire_shape_tokens(shape);
            quote!(NetworkWireScalarShape::PackedPosition(#shape))
        }
        SchemaWireScalarShape::TransformCompressor => {
            quote!(NetworkWireScalarShape::TransformCompressor)
        }
        SchemaWireScalarShape::PackedSize => quote!(NetworkWireScalarShape::PackedSize),
        SchemaWireScalarShape::Mat3 => quote!(NetworkWireScalarShape::Mat3),
        SchemaWireScalarShape::Affine3 => quote!(NetworkWireScalarShape::Affine3),
        SchemaWireScalarShape::Aabb2d => quote!(NetworkWireScalarShape::Aabb2d),
        SchemaWireScalarShape::Aabb3d => quote!(NetworkWireScalarShape::Aabb3d),
        SchemaWireScalarShape::ActorRef => quote!(NetworkWireScalarShape::ActorRef),
        SchemaWireScalarShape::EntityRef => quote!(NetworkWireScalarShape::EntityRef),
        SchemaWireScalarShape::FixedBytes(len) => {
            quote!(NetworkWireScalarShape::FixedBytes(#len))
        }
        SchemaWireScalarShape::Bytes => quote!(NetworkWireScalarShape::Bytes),
        SchemaWireScalarShape::String => quote!(NetworkWireScalarShape::String),
    }
}

pub(super) fn option_u32_tokens(value: Option<u32>) -> proc_macro2::TokenStream {
    match value {
        Some(value) => quote!(Some(#value)),
        None => quote!(None),
    }
}

pub(super) fn option_str_tokens(value: Option<&str>) -> proc_macro2::TokenStream {
    match value {
        Some(value) => {
            let value = LitStr::new(value, proc_macro2::Span::call_site());
            quote!(Some(#value))
        }
        None => quote!(None),
    }
}

pub(super) fn type_id_literal(type_id: Uuid) -> proc_macro2::TokenStream {
    let literal = crate::uuid_format::uuid_u128_literal_text(type_id);
    let literal = LitInt::new(&literal, proc_macro2::Span::call_site());
    quote!(#literal)
}
