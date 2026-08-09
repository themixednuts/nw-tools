use super::*;

pub(super) fn message_module_tokens(
    network_type: &NetworkType,
    plan: &NetworkMessageGenerationPlanReport,
    rust_names: &BTreeMap<u32, String>,
    serialize_types: &BTreeMap<Uuid, &NetworkSerializeType>,
) -> proc_macro2::TokenStream {
    let type_index = network_type
        .type_index
        .expect("generatable message has a type index");
    let type_id = network_type
        .type_id
        .expect("generatable message has a type ID");
    let source_name = network_type
        .name
        .as_deref()
        .expect("generatable message has a name");
    let rust_name = rust_names
        .get(&type_index)
        .cloned()
        .unwrap_or_else(|| rust_type_ident(source_name));
    let module_ident = format_ident!("{}", rust_module_ident(&rust_name));
    let message_ident = format_ident!("{rust_name}");
    let type_id = LitStr::new(
        &type_id.hyphenated().to_string().to_ascii_uppercase(),
        proc_macro2::Span::call_site(),
    );
    let fields = plan
        .fields
        .iter()
        .map(message_field_tokens)
        .collect::<Vec<_>>();
    let codec_derive = if plan.supports_unmarshal == Some(false) {
        quote!(Marshal)
    } else {
        quote!(Marshaler)
    };
    let type_registry_attr = if plan.supports_unmarshal == Some(false) {
        quote!(#[type_registry(#type_index)])
    } else {
        quote!(#[type_registry(#type_index, class)])
    };
    let mut support_names = BTreeSet::new();
    let support_items = plan
        .fields
        .iter()
        .filter_map(|field| {
            message_field_support_tokens(field, &mut support_names, serialize_types)
        })
        .collect::<Vec<_>>();

    quote! {
        pub mod #module_ident {
            use ::nw_network::{#codec_derive, az_rtti, type_registry};

            #(#support_items)*

            #[az_rtti(#type_id)]
            #type_registry_attr
            #[derive(Debug, Clone, PartialEq, #codec_derive)]
            #[allow(clippy::type_complexity)]
            pub struct #message_ident {
                #(#fields)*
            }
        }

        pub use #module_ident::#message_ident;
    }
}

pub(super) fn message_field_support_tokens(
    field: &NetworkStateFieldShapeReport,
    emitted_names: &mut BTreeSet<String>,
    serialize_types: &BTreeMap<Uuid, &NetworkSerializeType>,
) -> Option<proc_macro2::TokenStream> {
    let shape = field.nested_type_shape.as_ref()?;
    if message_nested_shape_uses_source_type(shape) {
        // Source-backed nested shapes resolve to `::nw_network::source::…` and
        // must not emit a duplicate local support type. Exact identities that
        // fell back to a local support name still need emission below.
        if field
            .rust_value_type
            .as_deref()
            .is_some_and(|value| value.contains("::"))
        {
            return None;
        }
    }
    if !shape.has_proven_anonymous_layout()
        && !shape.has_proven_symbolic_identity()
        && !(shape.has_exact_identity() && shape.has_proven_layout())
    {
        return None;
    }
    let value_type_string = field.rust_value_type.as_deref()?;
    if value_type_string.starts_with("::") || value_type_string.contains("::") {
        return None;
    }
    let value_type_ident = message_support_type_ident(value_type_string)?;
    if !emitted_names.insert(value_type_string.to_owned()) {
        return None;
    }
    let value_type = syn::parse_str::<syn::Type>(value_type_string).ok()?;
    let codec_name = format!("{}Marshaler", rust_type_ident(value_type_string));
    let codec_ident = format_ident!("{codec_name}");
    let members = nested_type_shape_members_in_wire_order(shape)?
        .into_iter()
        .map(|member| container_value_member_tokens(field, shape, member, &[], serialize_types))
        .collect::<Option<Vec<_>>>()?;
    if members.is_empty() {
        return None;
    }

    let struct_fields = members.iter().map(|member| {
        let field_ident = &member.field_ident;
        let rust_type = &member.rust_type;
        quote! {
            pub #field_ident: #rust_type,
        }
    });
    let marshal_size_terms = members
        .iter()
        .map(|member| {
            let codec = &member.codec_type;
            let ty = &member.rust_type;
            quote!(<#codec as ::nw_network::serialize::Codec<#ty>>::MARSHAL_SIZE)
        })
        .collect::<Vec<_>>();
    let marshal_size = match marshal_size_terms.split_first() {
        Some((first, rest)) => quote!(#first #( + #rest )*),
        None => quote!(0),
    };
    let marshal_fields = members.iter().map(|member| {
        let codec = &member.codec_type;
        let ty = &member.rust_type;
        let access = &member.access;
        quote! {
            <#codec as ::nw_network::serialize::Codec<#ty>>::marshal(&value.#access, wb);
        }
    });
    let decode_fields = members.iter().map(|member| {
        let binding = &member.binding;
        let codec = &member.codec_type;
        let ty = &member.rust_type;
        quote! {
            let #binding = <#codec as ::nw_network::serialize::Codec<#ty>>::unmarshal(rb)?;
        }
    });
    let init_fields = members.iter().map(|member| {
        let field_ident = &member.field_ident;
        let binding = &member.binding;
        quote!(#field_ident: #binding,)
    });

    Some(quote! {
        #[derive(Debug, Clone, PartialEq)]
        pub struct #value_type_ident {
            #(#struct_fields)*
        }

        #[derive(Debug, Clone, Copy, Default)]
        pub struct #codec_ident;

        impl ::nw_network::serialize::Codec<#value_type> for #codec_ident {
            const MARSHAL_SIZE: usize = #marshal_size;

            fn marshal(value: &#value_type, wb: &mut ::nw_network::serialize::WriteBuffer) {
                #(#marshal_fields)*
            }

            fn unmarshal(
                rb: &mut ::nw_network::serialize::ReadBuffer,
            ) -> Result<#value_type, ::nw_network::serialize::MarshalerError> {
                #(#decode_fields)*
                Ok(#value_type {
                    #(#init_fields)*
                })
            }
        }

        impl ::nw_network::serialize::Marshal for #value_type {
            const MARSHAL_SIZE: usize =
                <#codec_ident as ::nw_network::serialize::Codec<#value_type>>::MARSHAL_SIZE;

            fn marshal(&self, wb: &mut ::nw_network::serialize::WriteBuffer) {
                <#codec_ident as ::nw_network::serialize::Codec<#value_type>>::marshal(self, wb);
            }

        }

        impl ::nw_network::serialize::Unmarshal for #value_type {
            fn unmarshal(
                rb: &mut ::nw_network::serialize::ReadBuffer,
            ) -> Result<Self, ::nw_network::serialize::MarshalerError> {
                <#codec_ident as ::nw_network::serialize::Codec<#value_type>>::unmarshal(rb)
            }
        }
    })
}

pub(super) fn message_support_type_ident(value: &str) -> Option<syn::Ident> {
    let ident = syn::parse_str::<syn::Ident>(value).ok()?;
    let ident_text = ident.to_string();
    if ident_text != value || is_builtin_rust_type_ident(&ident_text) {
        return None;
    }
    Some(ident)
}

pub(super) fn is_builtin_rust_type_ident(value: &str) -> bool {
    matches!(
        value,
        "bool"
            | "char"
            | "str"
            | "String"
            | "u8"
            | "u16"
            | "u32"
            | "u64"
            | "u128"
            | "usize"
            | "i8"
            | "i16"
            | "i32"
            | "i64"
            | "i128"
            | "isize"
            | "f32"
            | "f64"
    )
}

pub(super) fn message_field_tokens(
    field: &NetworkStateFieldShapeReport,
) -> proc_macro2::TokenStream {
    let field_ident = format_ident!("{}", message_field_ident(field));
    let field_type = field
        .rust_field_type
        .as_deref()
        .and_then(|rust_type| syn::parse_str::<syn::Type>(rust_type).ok())
        .map(|rust_type| quote!(#rust_type))
        .unwrap_or_else(|| {
            message_field_type_tokens(
                field
                    .wire_shape
                    .as_ref()
                    .expect("generatable message field has a field type"),
            )
        });
    let marshal_attr = message_field_marshal_attr_tokens(field);

    quote! {
        #marshal_attr
        pub #field_ident: #field_type,
    }
}

pub(super) fn message_field_ident(field: &NetworkStateFieldShapeReport) -> String {
    let field_name = field
        .field_name
        .as_deref()
        .expect("generatable message field has a name");
    if is_placeholder_report_field_name(field)
        && let Some(index) = field.field_index
    {
        return format!("field_{index}");
    }
    rust_field_ident(field_name)
}

pub(super) fn message_field_type_tokens(shape: &SchemaWireShape) -> proc_macro2::TokenStream {
    match shape {
        SchemaWireShape::Bool => quote!(bool),
        SchemaWireShape::U8 => quote!(u8),
        SchemaWireShape::U16 => quote!(u16),
        SchemaWireShape::U32 | SchemaWireShape::VlqU32 => quote!(u32),
        SchemaWireShape::U64 | SchemaWireShape::VlqU64 => quote!(u64),
        SchemaWireShape::SequenceNumber => quote!(::nw_network::SequenceNumber),
        SchemaWireShape::F64 => quote!(f64),
        SchemaWireShape::F32 | SchemaWireShape::HalfF32 => quote!(f32),
        SchemaWireShape::Vec2 => quote!(::glam::Vec2),
        SchemaWireShape::Vec3 => quote!(::glam::Vec3),
        SchemaWireShape::Vec4 => quote!(::glam::Vec4),
        SchemaWireShape::Quat => quote!(::glam::Quat),
        SchemaWireShape::QuatCompNorm => quote!(::nw_network::serialize::QuatCompNorm),
        SchemaWireShape::Vec2Comp => quote!(::glam::Vec2),
        SchemaWireShape::Vec3Comp
        | SchemaWireShape::Vec3CompNorm
        | SchemaWireShape::Vec3SmallestThree
        | SchemaWireShape::NonUniformScaleComp
        | SchemaWireShape::DeltaVec3(_) => quote!(::glam::Vec3),
        SchemaWireShape::RemoteServerGdeRef => {
            quote!(::nw_network::source::RemoteServerGDERef)
        }
        SchemaWireShape::QuatComp | SchemaWireShape::QuatSmallestThree => quote!(::glam::Quat),
        SchemaWireShape::PackedPosition(_) => quote!(::glam::Vec3),
        SchemaWireShape::TransformCompressor => quote!(::glam::Affine3A),
        SchemaWireShape::PackedSize => quote!(::nw_network::serialize::PackedSize),
        SchemaWireShape::Mat3 => quote!(::glam::Mat3),
        SchemaWireShape::Affine3 => quote!(::glam::Affine3A),
        SchemaWireShape::Aabb2d => quote!(::bevy_math::bounding::Aabb2d),
        SchemaWireShape::Aabb3d => quote!(::bevy_math::bounding::Aabb3d),
        SchemaWireShape::ActorRef => quote!(::nw_network::ActorRef),
        SchemaWireShape::EntityRef => quote!(::nw_network::EntityRef),
        SchemaWireShape::FixedBytes(len) => {
            let len = unsuffixed_int_lit(*len);
            quote!([u8; #len])
        }
        SchemaWireShape::Bytes => quote!(Vec<u8>),
        SchemaWireShape::String => quote!(String),
        SchemaWireShape::ClassValue => quote!(::nw_network::serialize::ClassValue),
        SchemaWireShape::ActorInstantiationParameters => {
            quote!(::nw_network::ActorInstantiationParameters)
        }
        SchemaWireShape::Composite(members) => {
            let members = members.iter().map(message_field_type_tokens);
            quote!((#(#members,)*))
        }
        SchemaWireShape::Optional(inner) => {
            let inner = message_field_type_tokens(inner);
            quote!(::core::option::Option<#inner>)
        }
        SchemaWireShape::DefaultOmitted(_)
        | SchemaWireShape::BooleanChoice(_)
        | SchemaWireShape::BitMaskComposite(_) => {
            let value_type = rust_field_shape(shape).value_type;
            syn::parse_str::<syn::Type>(&value_type)
                .map(|value_type| quote!(#value_type))
                .expect("recursive wire shape produces a valid Rust value type")
        }
        SchemaWireShape::Sequence(element) => {
            let element = message_field_type_tokens(element);
            quote!(::std::vec::Vec<#element>)
        }
        SchemaWireShape::Set(element) => {
            let element = message_field_type_tokens(element);
            quote!(::nw_network::serialize::IndexSet<#element>)
        }
        SchemaWireShape::Map { key, value } => {
            let key = message_field_type_tokens(key);
            let value = message_field_type_tokens(value);
            quote!(::nw_network::serialize::IndexMap<#key, #value>)
        }
        SchemaWireShape::ReplicatedContainer(_) => {
            unreachable!("container message fields require an explicit semantic type")
        }
        SchemaWireShape::FixedSequence(sequence) => {
            let element = message_field_type_tokens(&sequence.element);
            let capacity = syn::LitInt::new(
                &sequence.capacity.to_string(),
                proc_macro2::Span::call_site(),
            );
            quote!(::arrayvec::ArrayVec<#element, #capacity>)
        }
    }
}

pub(super) fn message_field_marshal_attr_tokens(
    field: &NetworkStateFieldShapeReport,
) -> proc_macro2::TokenStream {
    if let Some(conversion) = field_conversion_marshal_type_string(field) {
        let conversion = LitStr::new(&conversion, proc_macro2::Span::call_site());
        return quote!(#[marshal(codec = #conversion)]);
    }
    if field.nested_type_shape.is_some()
        && field.rust_value_type.as_deref().is_some_and(|rust_type| {
            is_generated_source_type(rust_type) || message_support_type_ident(rust_type).is_some()
        })
    {
        return quote! {};
    }

    match field.wire_shape.as_ref() {
        Some(shape) => message_wire_shape_marshal_attr_tokens(shape),
        None => quote! {},
    }
}

pub(super) fn message_wire_shape_marshal_attr_tokens(
    shape: &SchemaWireShape,
) -> proc_macro2::TokenStream {
    match shape {
        SchemaWireShape::HalfF32 => {
            quote!(#[marshal(as = "::nw_network::serialize::HalfF32")])
        }
        SchemaWireShape::VlqU32 => {
            quote!(#[marshal(as = "::nw_network::serialize::VlqU32")])
        }
        SchemaWireShape::VlqU64 => {
            quote!(#[marshal(as = "::nw_network::serialize::VlqU64")])
        }
        SchemaWireShape::Vec2Comp => {
            quote!(#[marshal(codec = "::nw_network::serialize::Vec2CompMarshaler")])
        }
        SchemaWireShape::Vec3Comp => {
            quote!(#[marshal(codec = "::nw_network::serialize::Vec3CompMarshaler")])
        }
        SchemaWireShape::Vec3CompNorm => {
            quote!(#[marshal(codec = "::nw_network::serialize::Vec3CompNormMarshaler")])
        }
        SchemaWireShape::Vec3SmallestThree => {
            quote!(#[marshal(codec = "::nw_network::serialize::PackedNormalizedVec3Marshaller")])
        }
        SchemaWireShape::QuatComp => {
            quote!(#[marshal(codec = "::nw_network::serialize::QuatCompMarshaler")])
        }
        SchemaWireShape::QuatSmallestThree => {
            quote!(#[marshal(codec = "::nw_network::serialize::QuatSmallestThreeQuantizedMarshaler")])
        }
        SchemaWireShape::NonUniformScaleComp => {
            quote!(#[marshal(codec = "::nw_network::serialize::NonUniformScaleCompMarshaler")])
        }
        SchemaWireShape::DeltaVec3(range) => {
            let codec = format!("::nw_network::serialize::DeltaMarshaler<{range}, ::glam::Vec3>");
            quote!(#[marshal(codec = #codec)])
        }
        SchemaWireShape::RemoteServerGdeRef => {
            quote!(#[marshal(codec = "::nw_network::serialize::RemoteServerGdeRefMarshaler")])
        }
        SchemaWireShape::PackedPosition(shape) => {
            let minimum_bits = shape.minimum_bits;
            let maximum_bits = shape.maximum_bits;
            let codec = format!(
                "::nw_network::serialize::PackedPositionMarshaller<{minimum_bits}, {maximum_bits}>"
            );
            quote!(#[marshal(codec = #codec)])
        }
        SchemaWireShape::TransformCompressor => {
            quote!(#[marshal(codec = "::nw_network::serialize::TransformCompressor")])
        }
        SchemaWireShape::Composite(_)
        | SchemaWireShape::Optional(_)
        | SchemaWireShape::DefaultOmitted(_)
        | SchemaWireShape::BooleanChoice(_)
        | SchemaWireShape::BitMaskComposite(_)
        | SchemaWireShape::Sequence(_)
        | SchemaWireShape::Map { .. }
        | SchemaWireShape::Set(_)
        | SchemaWireShape::FixedSequence(_) => {
            let Some(codec) = wire_shape_codec_type(shape) else {
                return quote! {};
            };
            quote!(#[marshal(codec = #codec)])
        }
        _ => quote! {},
    }
}

pub(super) fn field_conversion_marshal_type_tokens(
    field: &NetworkStateFieldShapeReport,
) -> Option<proc_macro2::TokenStream> {
    let ty = field_conversion_marshal_type_string(field)?;
    syn::parse_str::<syn::Type>(&ty).ok().map(|ty| quote!(#ty))
}

pub(super) fn field_conversion_marshal_type_string(
    field: &NetworkStateFieldShapeReport,
) -> Option<String> {
    let shape = field.wire_shape.as_ref()?;
    let rust_type = field.rust_value_type.as_deref()?.trim();
    conversion_marshal_type_string_for(shape, rust_type)
}

pub(super) fn serialize_field_scalar_source_type(
    field: &NetworkField,
    shape: Option<&SchemaWireShape>,
    serialize_types: &BTreeMap<Uuid, &NetworkSerializeType>,
) -> Option<String> {
    let serialize = field.serialize.as_ref()?;
    if serialize.kind != NetworkSerializeKind::Enum
        || !serialize_types
            .get(&serialize.type_id)
            .is_some_and(|serialize| serialize.emits_source)
    {
        return None;
    }
    scalar_conversion_serialized_type(shape?)?;
    let source_type = serialize_types.get(&serialize.type_id)?;
    network_serialize_type_rust_type(source_type, serialize_types)
        .filter(|rust_type| is_generated_source_type(rust_type))
}

pub(super) fn conversion_marshal_type_string_for(
    shape: &SchemaWireShape,
    rust_type: &str,
) -> Option<String> {
    let serialized_type = scalar_conversion_serialized_type(shape)?;
    let rust_type = rust_type.trim();
    if rust_type == serialized_type {
        return None;
    }
    if !is_generated_source_type(rust_type) {
        return None;
    }
    Some(format!(
        "::nw_network::serialize::ConversionMarshaler<{serialized_type}, {rust_type}>"
    ))
}

pub(super) fn is_generated_source_type(rust_type: &str) -> bool {
    let rust_type = rust_type.trim_start_matches("::");
    rust_type.starts_with("nw_network::source::")
}

pub(super) const fn scalar_conversion_serialized_type(
    shape: &SchemaWireShape,
) -> Option<&'static str> {
    match shape {
        SchemaWireShape::U8 => Some("u8"),
        SchemaWireShape::U16 => Some("u16"),
        SchemaWireShape::U32 => Some("u32"),
        _ => None,
    }
}
