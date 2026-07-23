use super::*;

pub(super) fn unsuffixed_int_lit(value: u16) -> LitInt {
    LitInt::new(&value.to_string(), proc_macro2::Span::call_site())
}

pub(super) fn blocked_state_generation_plan(
    type_index: Option<u32>,
    type_name: Option<String>,
    reason: &str,
) -> NetworkStateGenerationPlanReport {
    NetworkStateGenerationPlanReport {
        type_index,
        type_name,
        fragment_category: None,
        fragment_category_value: None,
        is_metadata_fragment: None,
        field_count: 0,
        attribute_count: 0,
        shaped_field_count: 0,
        supported_field_count: 0,
        missing_wire_shape_count: 0,
        unsupported_wire_shape_count: 0,
        low_confidence_field_count: 0,
        evidence_issues: Vec::new(),
        can_generate: false,
        blocked_reasons: vec![reason.to_owned()],
        fields: Vec::new(),
    }
}

pub(super) fn replicated_state_module_tokens(
    network_type: &NetworkType,
    plan: &NetworkStateGenerationPlanReport,
    rust_names: &BTreeMap<u32, String>,
    options: &NetworkReplicatedStateEmitOptions,
    serialize_types: &BTreeMap<Uuid, &NetworkSerializeType>,
) -> proc_macro2::TokenStream {
    let type_index = network_type
        .type_index
        .expect("generatable replicated state has a type index");
    let type_id = network_type
        .type_id
        .expect("generatable replicated state has a type ID");
    let source_name = network_type
        .name
        .as_deref()
        .expect("generatable replicated state has a name");
    let rust_name = rust_names
        .get(&type_index)
        .cloned()
        .unwrap_or_else(|| rust_type_ident(source_name));
    let module_ident = format_ident!("{}", rust_module_ident(&rust_name));
    let state_ident = format_ident!("{rust_name}");
    let type_id = LitStr::new(
        &type_id.hyphenated().to_string().to_ascii_uppercase(),
        proc_macro2::Span::call_site(),
    );
    let fields = plan
        .fields
        .iter()
        .map(replicated_state_field_tokens)
        .collect::<Vec<_>>();
    let mut support_names = BTreeSet::new();
    let support_items = plan
        .fields
        .iter()
        .flat_map(|field| {
            replicated_state_field_support_tokens(field, &mut support_names, serialize_types)
        })
        .collect::<Vec<_>>();
    let register_fragment = options.registers_type_index(type_index);
    let type_registry_attr = register_fragment.then(|| quote! { #[type_registry(#type_index)] });
    let type_registry_entry_tokens = (!register_fragment).then(|| {
        quote! {
            impl ::nw_network::types::TypeRegistryEntry for #state_ident {
                const TYPE_INDEX: u32 = #type_index;
            }
        }
    });
    let type_registry_import = register_fragment.then(|| quote! { , type_registry });
    let replicated_state_attr =
        replicated_state_attr_tokens(network_type.fragment_metadata.as_ref());

    quote! {
        pub mod #module_ident {
            use ::nw_network::{az_rtti, replicated_state, Marshaler #type_registry_import};

            #(#support_items)*

            #replicated_state_attr
            #[az_rtti(#type_id)]
            #type_registry_attr
            #[derive(Debug, Clone, Default)]
            pub struct #state_ident {
                #(#fields)*
            }

            #type_registry_entry_tokens
        }

        pub use #module_ident::#state_ident;
    }
}

pub(super) fn replicated_state_field_support_tokens(
    field: &NetworkStateFieldShapeReport,
    emitted_names: &mut BTreeSet<String>,
    serialize_types: &BTreeMap<Uuid, &NetworkSerializeType>,
) -> Vec<proc_macro2::TokenStream> {
    let mut items = Vec::new();
    if let Some(sequence) = field.fixed_sequence.as_ref()
        && sequence.generates_support_type
        && let Some(element_type_name) = sequence.element_type_name.as_deref()
        && emitted_names.insert(rust_type_ident(element_type_name))
        && let Some(tokens) = sequence.support_tokens()
    {
        items.push(tokens);
    }
    for shape in &field.nested_embedded_type_shapes {
        if !container_embedded_shape_is_referenced(field.nested_type_shape.as_ref(), shape) {
            continue;
        }
        if let Some(tokens) = replicated_state_shape_support_tokens(
            field,
            shape,
            &field.nested_embedded_type_shapes,
            false,
            emitted_names,
            serialize_types,
        ) {
            items.push(tokens);
        }
    }
    if let Some(shape) = field.nested_type_shape.as_ref()
        && field_references_shape_codec(field, shape)
        && let Some(tokens) = replicated_state_shape_support_tokens(
            field,
            shape,
            &field.nested_embedded_type_shapes,
            false,
            emitted_names,
            serialize_types,
        )
    {
        items.push(tokens);
    }
    for shape in &field.container_embedded_key_type_shapes {
        if !container_embedded_shape_is_referenced(field.container_key_type_shape.as_ref(), shape) {
            continue;
        }
        if let Some(tokens) = replicated_state_shape_support_tokens(
            field,
            shape,
            &field.container_embedded_key_type_shapes,
            true,
            emitted_names,
            serialize_types,
        ) {
            items.push(tokens);
        }
    }
    if let Some(shape) = field.container_key_type_shape.as_ref()
        && field_references_shape_codec(field, shape)
        && let Some(tokens) = replicated_state_shape_support_tokens(
            field,
            shape,
            &field.container_embedded_key_type_shapes,
            true,
            emitted_names,
            serialize_types,
        )
    {
        items.push(tokens);
    }
    for shape in &field.container_embedded_value_type_shapes {
        if !container_embedded_shape_is_referenced(field.container_value_type_shape.as_ref(), shape)
        {
            continue;
        }
        if let Some(tokens) = replicated_state_shape_support_tokens(
            field,
            shape,
            &field.container_embedded_value_type_shapes,
            false,
            emitted_names,
            serialize_types,
        ) {
            items.push(tokens);
        }
    }
    if let Some(shape) = field.container_value_type_shape.as_ref()
        && field_references_shape_codec(field, shape)
        && let Some(tokens) = replicated_state_shape_support_tokens(
            field,
            shape,
            &field.container_embedded_value_type_shapes,
            false,
            emitted_names,
            serialize_types,
        )
    {
        items.push(tokens);
    }
    items
}

pub(super) fn container_embedded_shape_is_referenced(
    parent: Option<&crate::network_schema::NetworkNestedTypeShape>,
    shape: &crate::network_schema::NetworkNestedTypeShape,
) -> bool {
    let Some(parent) = parent else {
        return false;
    };
    parent.members.iter().any(|member| {
        nested_member_wire_shape(member).is_some_and(|wire_shape| {
            nested_shape_by_wire_name(wire_shape, core::slice::from_ref(shape)).is_some()
                || collection_element_wire_shape(wire_shape)
                    .and_then(|element| {
                        nested_shape_by_wire_name(element, core::slice::from_ref(shape))
                    })
                    .is_some()
        })
    })
}

pub(super) fn field_references_shape_codec(
    field: &NetworkStateFieldShapeReport,
    shape: &crate::network_schema::NetworkNestedTypeShape,
) -> bool {
    let Some(codec_name) = container_value_shape_report_codec_name(field, shape) else {
        return false;
    };
    field
        .rust_field_type
        .as_deref()
        .is_some_and(|field_type| field_type.contains(&codec_name))
}

pub(super) fn replicated_state_shape_support_tokens(
    field: &NetworkStateFieldShapeReport,
    shape: &crate::network_schema::NetworkNestedTypeShape,
    embedded_shapes: &[crate::network_schema::NetworkNestedTypeShape],
    derive_key_traits: bool,
    emitted_names: &mut BTreeSet<String>,
    serialize_types: &BTreeMap<Uuid, &NetworkSerializeType>,
) -> Option<proc_macro2::TokenStream> {
    let codec_name = container_value_shape_report_codec_name(field, shape)?;
    if !emitted_names.insert(codec_name.clone()) {
        return None;
    }

    let codec_ident = format_ident!("{codec_name}");
    let local_value_type_name = field
        .field_name
        .as_deref()
        .and_then(|field_name| container_value_shape_support_type_name(field_name, shape));
    let uses_source_type = container_value_shape_report_uses_source_type(shape, serialize_types);
    let value_type_string = if uses_source_type {
        shape
            .type_name_full
            .as_deref()
            .or(shape.type_name.as_deref())
            .and_then(serialize_source_rust_type_name)?
    } else {
        local_value_type_name.clone()?
    };
    let value_type = syn::parse_str::<syn::Type>(&value_type_string).ok()?;

    let members = shape
        .members
        .iter()
        .map(|member| {
            container_value_member_tokens(field, shape, member, embedded_shapes, serialize_types)
        })
        .collect::<Option<Vec<_>>>()?;
    if members.is_empty() {
        return None;
    }
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
    let can_initialize_directly = members.iter().all(|member| member.is_flat_field);
    let value_initializer = if can_initialize_directly {
        let init_fields = members.iter().map(|member| {
            let field_ident = &member.field_ident;
            let binding = &member.binding;
            quote!(#field_ident: #binding,)
        });
        if uses_source_type {
            quote! {
                #value_type {
                    #(#init_fields)*
                    ..<#value_type as ::core::default::Default>::default()
                }
            }
        } else {
            quote! {
                #value_type {
                    #(#init_fields)*
                }
            }
        }
    } else {
        let assign_fields = members.iter().map(|member| {
            let binding = &member.binding;
            let access = &member.access;
            quote! {
                value.#access = #binding;
            }
        });
        quote! {{
            let mut value = <#value_type as ::core::default::Default>::default();
            #(#assign_fields)*
            value
        }}
    };
    let support_struct = if uses_source_type {
        quote! {}
    } else {
        let value_type_ident = local_value_type_name
            .as_deref()
            .map(|name| format_ident!("{name}"))?;
        let key_derives = derive_key_traits.then(|| quote! { , Eq, Hash });
        let struct_fields = members.iter().map(|member| {
            let field_ident = &member.field_ident;
            let rust_type = &member.rust_type;
            quote! {
                pub #field_ident: #rust_type,
            }
        });
        quote! {
            #[derive(Debug, Clone, Default, PartialEq #key_derives)]
            pub struct #value_type_ident {
                #(#struct_fields)*
            }
        }
    };
    let marshaler_impl = if uses_source_type {
        quote! {}
    } else {
        quote! {
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
        }
    };

    Some(quote! {
        #support_struct

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
                Ok(#value_initializer)
            }
        }

        #marshaler_impl
    })
}

pub(super) struct ContainerValueMemberTokens {
    pub(super) binding: proc_macro2::Ident,
    pub(super) access: proc_macro2::TokenStream,
    pub(super) field_ident: proc_macro2::Ident,
    pub(super) rust_type: syn::Type,
    pub(super) codec_type: syn::Type,
    pub(super) is_flat_field: bool,
}

pub(super) fn container_value_member_tokens(
    field: &NetworkStateFieldShapeReport,
    parent: &crate::network_schema::NetworkNestedTypeShape,
    member: &crate::network_schema::NetworkNestedTypeMember,
    embedded_shapes: &[crate::network_schema::NetworkNestedTypeShape],
    serialize_types: &BTreeMap<Uuid, &NetworkSerializeType>,
) -> Option<ContainerValueMemberTokens> {
    let name = member.name.as_deref()?;
    let binding = format_ident!("field_{}", rust_field_ident(&name.replace('.', "_")));
    let access = member_access_tokens(name)?;
    let field_ident = format_ident!("{}", rust_field_ident(name));
    let rust_type_string =
        container_value_member_rust_type(field, parent, member, embedded_shapes, serialize_types)?;
    let rust_type = syn::parse_str::<syn::Type>(&rust_type_string).ok()?;
    let codec_type_string = container_value_member_codec_type(
        field,
        member,
        &rust_type_string,
        embedded_shapes,
        serialize_types,
    )?;
    let codec_type = syn::parse_str::<syn::Type>(&codec_type_string).ok()?;
    Some(ContainerValueMemberTokens {
        binding,
        access,
        field_ident,
        rust_type,
        codec_type,
        is_flat_field: !name.contains('.'),
    })
}

pub(super) fn container_value_shape_report_uses_source_type(
    shape: &crate::network_schema::NetworkNestedTypeShape,
    serialize_types: &BTreeMap<Uuid, &NetworkSerializeType>,
) -> bool {
    container_value_shape_uses_source_type(shape, serialize_types)
}

pub(super) fn container_value_shape_report_rust_type(
    field: &NetworkStateFieldShapeReport,
    shape: &crate::network_schema::NetworkNestedTypeShape,
    serialize_types: &BTreeMap<Uuid, &NetworkSerializeType>,
) -> Option<String> {
    if container_value_shape_report_uses_source_type(shape, serialize_types) {
        if let Some(rust_type) = shape.type_id.and_then(exact_type_id_rust_type) {
            return Some(rust_type.to_owned());
        }
        shape
            .type_name_full
            .as_deref()
            .or(shape.type_name.as_deref())
            .and_then(serialize_source_rust_type_name)
    } else {
        field
            .field_name
            .as_deref()
            .and_then(|field_name| container_value_shape_support_type_name(field_name, shape))
    }
}

pub(super) fn member_access_tokens(name: &str) -> Option<proc_macro2::TokenStream> {
    let mut parts = name
        .split('.')
        .map(str::trim)
        .filter(|part| !part.is_empty())
        .map(rust_field_ident)
        .map(|part| format_ident!("{part}"))
        .collect::<Vec<_>>();
    let first = parts.first()?.clone();
    parts.remove(0);
    Some(quote!(#first #(.#parts)*))
}

pub(super) fn container_value_member_rust_type(
    field: &NetworkStateFieldShapeReport,
    parent: &crate::network_schema::NetworkNestedTypeShape,
    member: &crate::network_schema::NetworkNestedTypeMember,
    embedded_shapes: &[crate::network_schema::NetworkNestedTypeShape],
    serialize_types: &BTreeMap<Uuid, &NetworkSerializeType>,
) -> Option<String> {
    let shape = nested_member_wire_shape(member)?;
    if let Some(shape) = nested_shape_by_wire_name(shape, embedded_shapes) {
        return container_value_shape_report_rust_type(field, shape, serialize_types);
    }
    if let Some(rust_type) = exact_member_rust_type(parent, member, serialize_types) {
        return Some(rust_type);
    }
    member_wire_shape_rust_type(
        field,
        &parse_network_member_wire_shape(shape)?,
        embedded_shapes,
        serialize_types,
    )
}

fn member_wire_shape_rust_type(
    field: &NetworkStateFieldShapeReport,
    shape: &NetworkMemberWireShape<'_>,
    embedded_shapes: &[crate::network_schema::NetworkNestedTypeShape],
    serialize_types: &BTreeMap<Uuid, &NetworkSerializeType>,
) -> Option<String> {
    member_wire_shape_projection(field, shape, embedded_shapes, serialize_types)
        .map(|projection| projection.rust_type)
}

struct MemberWireProjection {
    rust_type: String,
    codec_type: Option<String>,
}

fn member_wire_shape_projection(
    field: &NetworkStateFieldShapeReport,
    shape: &NetworkMemberWireShape<'_>,
    embedded_shapes: &[crate::network_schema::NetworkNestedTypeShape],
    serialize_types: &BTreeMap<Uuid, &NetworkSerializeType>,
) -> Option<MemberWireProjection> {
    match shape {
        NetworkMemberWireShape::Scalar(shape) => Some(MemberWireProjection {
            rust_type: scalar_rust_type(*shape),
            codec_type: scalar_shape_uses_custom_codec(*shape)
                .then(|| scalar_marshaler_type(*shape)),
        }),
        NetworkMemberWireShape::Composite(members) => {
            let members = members
                .iter()
                .map(|member| {
                    member_wire_shape_projection(field, member, embedded_shapes, serialize_types)
                })
                .collect::<Option<Vec<_>>>()?;
            composite_member_projection(&members)
        }
        NetworkMemberWireShape::Optional(inner) => {
            let inner =
                member_wire_shape_projection(field, inner, embedded_shapes, serialize_types)?;
            Some(MemberWireProjection {
                rust_type: format!("::core::option::Option<{}>", inner.rust_type),
                codec_type: inner
                    .codec_type
                    .map(|codec| format!("::nw_network::serialize::OptionalCodec<{codec}>")),
            })
        }
        NetworkMemberWireShape::DefaultOmitted(members) => {
            let members = members
                .iter()
                .map(|member| {
                    member_wire_shape_projection(field, member, embedded_shapes, serialize_types)
                })
                .collect::<Option<Vec<_>>>()?;
            let rust_type = tuple_projection(
                &members
                    .iter()
                    .map(|member| (member.rust_type.clone(), None))
                    .collect::<Vec<_>>(),
            )
            .0;
            let codecs = members
                .iter()
                .map(member_projection_codec_type)
                .collect::<Vec<_>>();
            Some(MemberWireProjection {
                rust_type,
                codec_type: Some(format!(
                    "::nw_network::serialize::DefaultOmittedTupleCodec<{}>",
                    tuple_projection(
                        &codecs
                            .into_iter()
                            .map(|codec| (codec, None))
                            .collect::<Vec<_>>()
                    )
                    .0
                )),
            })
        }
        NetworkMemberWireShape::BooleanChoice {
            false_value,
            true_value,
        } => {
            let false_value =
                member_wire_shape_projection(field, false_value, embedded_shapes, serialize_types)?;
            let true_value =
                member_wire_shape_projection(field, true_value, embedded_shapes, serialize_types)?;
            let false_codec = member_projection_codec_type(&false_value);
            let true_codec = member_projection_codec_type(&true_value);
            Some(MemberWireProjection {
                rust_type: format!(
                    "::nw_network::serialize::BooleanChoice<{}, {}>",
                    false_value.rust_type, true_value.rust_type
                ),
                codec_type: Some(format!(
                    "::nw_network::serialize::BooleanChoiceCodec<{false_codec}, {true_codec}>"
                )),
            })
        }
        NetworkMemberWireShape::Vector(element) => {
            let element =
                member_wire_shape_projection(field, element, embedded_shapes, serialize_types)?;
            Some(MemberWireProjection {
                rust_type: format!("::std::vec::Vec<{}>", element.rust_type),
                codec_type: element
                    .codec_type
                    .map(|codec| format!("::nw_network::serialize::SequenceCodec<{codec}>")),
            })
        }
        NetworkMemberWireShape::Set(element) => {
            let element =
                member_wire_shape_projection(field, element, embedded_shapes, serialize_types)?;
            Some(MemberWireProjection {
                rust_type: format!("::nw_network::serialize::IndexSet<{}>", element.rust_type),
                codec_type: element.codec_type.map(|codec| {
                    format!(
                        "::nw_network::serialize::ContainerMarshaler<{}, {codec}>",
                        element.rust_type
                    )
                }),
            })
        }
        NetworkMemberWireShape::Map { key, value } => {
            let key = member_wire_shape_projection(field, key, embedded_shapes, serialize_types)?;
            let value =
                member_wire_shape_projection(field, value, embedded_shapes, serialize_types)?;
            let codec_type = (key.codec_type.is_some() || value.codec_type.is_some()).then(|| {
                format!(
                    "::nw_network::serialize::MapSequenceCodec<{}, {}>",
                    member_projection_codec_type(&key),
                    member_projection_codec_type(&value)
                )
            });
            Some(MemberWireProjection {
                rust_type: format!(
                    "::nw_network::serialize::IndexMap<{}, {}>",
                    key.rust_type, value.rust_type
                ),
                codec_type,
            })
        }
        NetworkMemberWireShape::FixedVector { element, capacity } => {
            let element =
                member_wire_shape_projection(field, element, embedded_shapes, serialize_types)?;
            Some(MemberWireProjection {
                rust_type: format!("::arrayvec::ArrayVec<{}, {capacity}>", element.rust_type),
                codec_type: element
                    .codec_type
                    .map(|codec| format!("::nw_network::serialize::SequenceCodec<{codec}>")),
            })
        }
        NetworkMemberWireShape::FixedArray { element, capacity } => {
            let element =
                member_wire_shape_projection(field, element, embedded_shapes, serialize_types)?;
            Some(MemberWireProjection {
                rust_type: format!("[{}; {capacity}]", element.rust_type),
                codec_type: element
                    .codec_type
                    .map(|codec| format!("::nw_network::serialize::ArrayCodec<{codec}>")),
            })
        }
        NetworkMemberWireShape::Named(name) => {
            let shape = nested_shape_by_wire_name(name, embedded_shapes)?;
            Some(MemberWireProjection {
                rust_type: container_value_shape_report_rust_type(field, shape, serialize_types)?,
                codec_type: Some(structured_value_codec_name(
                    field.field_name.as_deref()?,
                    shape,
                )?),
            })
        }
    }
}

fn member_projection_codec_type(projection: &MemberWireProjection) -> String {
    projection.codec_type.clone().unwrap_or_else(|| {
        format!(
            "::nw_network::serialize::DefaultMarshaler<{}>",
            projection.rust_type
        )
    })
}

fn composite_member_projection(members: &[MemberWireProjection]) -> Option<MemberWireProjection> {
    let [first, rest @ ..] = members else {
        return None;
    };
    if rest
        .iter()
        .all(|member| member.rust_type == first.rust_type && member.codec_type == first.codec_type)
    {
        return Some(MemberWireProjection {
            rust_type: format!("[{}; {}]", first.rust_type, members.len()),
            codec_type: first
                .codec_type
                .as_ref()
                .map(|codec| format!("::nw_network::serialize::ArrayCodec<{codec}>")),
        });
    }
    let members = members
        .iter()
        .map(|member| (member.rust_type.clone(), member.codec_type.clone()))
        .collect::<Vec<_>>();
    let (rust_type, codec_type) = tuple_projection(&members);
    Some(MemberWireProjection {
        rust_type,
        codec_type,
    })
}

pub(super) fn container_value_member_codec_type(
    field: &NetworkStateFieldShapeReport,
    member: &crate::network_schema::NetworkNestedTypeMember,
    rust_type: &str,
    embedded_shapes: &[crate::network_schema::NetworkNestedTypeShape],
    serialize_types: &BTreeMap<Uuid, &NetworkSerializeType>,
) -> Option<String> {
    let wire_shape = nested_member_wire_shape(member)?;
    if let Some(shape) = nested_shape_by_wire_name(wire_shape, embedded_shapes) {
        return structured_value_codec_name(field.field_name.as_deref()?, shape);
    }
    let parsed = parse_network_member_wire_shape(wire_shape);
    if let Some(shape) = parsed
        .as_ref()
        .filter(|shape| !matches!(shape, NetworkMemberWireShape::Scalar(_)))
    {
        let projection =
            member_wire_shape_projection(field, shape, embedded_shapes, serialize_types)?;
        return Some(
            projection.codec_type.unwrap_or_else(|| {
                format!("::nw_network::serialize::DefaultMarshaler<{rust_type}>")
            }),
        );
    }
    if !wire_shape.is_empty() && wire_scalar_shape_from_name(wire_shape).is_none() {
        return Some(format!(
            "::nw_network::serialize::DefaultMarshaler<{rust_type}>"
        ));
    }
    let shape = wire_scalar_shape_from_name(wire_shape)?;
    let wire_shape = shape.into();
    if let Some(conversion) = conversion_marshal_type_string_for(&wire_shape, rust_type) {
        return Some(conversion);
    }
    if scalar_shape_uses_custom_codec(shape) {
        return Some(scalar_marshaler_type(shape));
    }
    Some(format!(
        "::nw_network::serialize::DefaultMarshaler<{rust_type}>"
    ))
}

pub(super) fn scalar_shape_uses_custom_codec(shape: SchemaWireScalarShape) -> bool {
    matches!(
        shape,
        SchemaWireScalarShape::HalfF32
            | SchemaWireScalarShape::VlqU32
            | SchemaWireScalarShape::VlqU64
            | SchemaWireScalarShape::Vec2Comp
            | SchemaWireScalarShape::Vec3Comp
            | SchemaWireScalarShape::Vec3CompNorm
            | SchemaWireScalarShape::Vec3SmallestThree
            | SchemaWireScalarShape::QuatComp
            | SchemaWireScalarShape::QuatSmallestThree
            | SchemaWireScalarShape::NonUniformScaleComp
            | SchemaWireScalarShape::DeltaVec3(_)
            | SchemaWireScalarShape::RemoteServerGdeRef
            | SchemaWireScalarShape::PackedPosition(_)
            | SchemaWireScalarShape::TransformCompressor
    )
}

pub(super) fn container_value_shape_report_codec_name(
    field: &NetworkStateFieldShapeReport,
    shape: &crate::network_schema::NetworkNestedTypeShape,
) -> Option<String> {
    structured_value_codec_name(field.field_name.as_deref()?, shape)
}

pub(super) fn replicated_state_attr_tokens(
    fragment_metadata: Option<&NetworkFragmentMetadata>,
) -> proc_macro2::TokenStream {
    let Some(category) = fragment_metadata
        .and_then(|metadata| metadata.category.as_deref())
        .and_then(fragment_category_attr_name)
    else {
        return quote! { #[replicated_state] };
    };
    quote! { #[replicated_state(category = #category)] }
}

pub(super) fn fragment_category_attr_name(category: &str) -> Option<&'static str> {
    match category {
        "Uncategorized" | "NumCategories" => None,
        "PlayerCharacter" => Some("player_character"),
        "NonPlayerCharacter" => Some("non_player_character"),
        "ImportantNonPlayerCharacter" => Some("important_non_player_character"),
        "Spell" => Some("spell"),
        "Projectile" => Some("projectile"),
        "Buildable" => Some("buildable"),
        _ => None,
    }
}

pub(super) fn replicated_state_field_tokens(
    field: &NetworkStateFieldShapeReport,
) -> proc_macro2::TokenStream {
    let field_name = field
        .field_name
        .as_deref()
        .expect("generatable replicated state field has a name");
    let field_ident = format_ident!("{}", rust_field_ident(field_name));
    let group_attr = match field.group {
        Some(0) | None => quote! {},
        Some(group) => quote! { #[replicated_state(group = #group)] },
    };
    let field_type = replicated_state_field_type_tokens(field);

    quote! {
        #group_attr
        pub #field_ident: #field_type,
    }
}

pub(super) fn replicated_state_field_type_tokens(
    field: &NetworkStateFieldShapeReport,
) -> proc_macro2::TokenStream {
    if let Some(field_type) = field
        .rust_field_type
        .as_deref()
        .filter(|rust_type| is_replicated_state_field_type(rust_type))
        .and_then(|rust_type| syn::parse_str::<syn::Type>(rust_type).ok())
    {
        return quote!(#field_type);
    }

    let shape = field
        .wire_shape
        .as_ref()
        .expect("generatable replicated state field has a wire shape");
    if let Some(conversion) = field_conversion_marshal_type_tokens(field) {
        let rust_type = field
            .rust_value_type
            .as_deref()
            .and_then(|rust_type| syn::parse_str::<syn::Type>(rust_type).ok())
            .expect("converted replicated state field has a valid Rust type");
        return quote!(
            ::nw_network::serialize::ReplicatedFieldHandler<
                #rust_type,
                #conversion,
            >
        );
    }

    match shape {
        SchemaWireShape::Bool => {
            quote!(::nw_network::serialize::ReplicatedFieldHandler<bool>)
        }
        SchemaWireShape::U8 => {
            quote!(::nw_network::serialize::ReplicatedFieldHandler<u8>)
        }
        SchemaWireShape::U16 => {
            quote!(::nw_network::serialize::ReplicatedFieldHandler<u16>)
        }
        SchemaWireShape::U32 => {
            quote!(::nw_network::serialize::ReplicatedFieldHandler<u32>)
        }
        SchemaWireShape::U64 => {
            quote!(::nw_network::serialize::ReplicatedFieldHandler<u64>)
        }
        SchemaWireShape::F32 => {
            quote!(::nw_network::serialize::ReplicatedFieldHandler<f32>)
        }
        SchemaWireShape::F64 => {
            quote!(::nw_network::serialize::ReplicatedFieldHandler<f64>)
        }
        SchemaWireShape::HalfF32 => {
            quote!(
                ::nw_network::serialize::ReplicatedFieldHandler<
                    f32,
                    ::nw_network::serialize::HalfF32Marshaler,
                >
            )
        }
        SchemaWireShape::VlqU32 => {
            quote!(
                ::nw_network::serialize::ReplicatedFieldHandler<
                    u32,
                    ::nw_network::serialize::VlqU32Marshaler,
                >
            )
        }
        SchemaWireShape::VlqU64 => {
            quote!(
                ::nw_network::serialize::ReplicatedFieldHandler<
                    u64,
                    ::nw_network::serialize::VlqU64Marshaler,
                >
            )
        }
        SchemaWireShape::SequenceNumber => {
            quote!(::nw_network::serialize::ReplicatedFieldHandler<::nw_network::SequenceNumber>)
        }
        SchemaWireShape::Vec2 => {
            quote!(::nw_network::serialize::ReplicatedFieldHandler<::glam::Vec2>)
        }
        SchemaWireShape::Vec3 => {
            quote!(::nw_network::serialize::ReplicatedFieldHandler<::glam::Vec3>)
        }
        SchemaWireShape::Vec4 => {
            quote!(::nw_network::serialize::ReplicatedFieldHandler<::glam::Vec4>)
        }
        SchemaWireShape::Quat => {
            quote!(::nw_network::serialize::ReplicatedFieldHandler<::glam::Quat>)
        }
        SchemaWireShape::QuatCompNorm => {
            quote!(
                ::nw_network::serialize::ReplicatedFieldHandler<
                    ::nw_network::serialize::QuatCompNorm,
                >
            )
        }
        SchemaWireShape::Vec2Comp => {
            quote!(
                ::nw_network::serialize::ReplicatedFieldHandler<
                    ::glam::Vec2,
                    ::nw_network::serialize::Vec2CompMarshaler,
                >
            )
        }
        SchemaWireShape::Vec3Comp => {
            quote!(
                ::nw_network::serialize::ReplicatedFieldHandler<
                    ::glam::Vec3,
                    ::nw_network::serialize::Vec3CompMarshaler,
                >
            )
        }
        SchemaWireShape::Vec3CompNorm => {
            quote!(
                ::nw_network::serialize::ReplicatedFieldHandler<
                    ::glam::Vec3,
                    ::nw_network::serialize::Vec3CompNormMarshaler,
                >
            )
        }
        SchemaWireShape::Vec3SmallestThree => {
            quote!(
                ::nw_network::serialize::ReplicatedFieldHandler<
                    ::glam::Vec3,
                    ::nw_network::serialize::PackedNormalizedVec3Marshaller,
                >
            )
        }
        SchemaWireShape::QuatComp => {
            quote!(
                ::nw_network::serialize::ReplicatedFieldHandler<
                    ::glam::Quat,
                    ::nw_network::serialize::QuatCompMarshaler,
                >
            )
        }
        SchemaWireShape::QuatSmallestThree => {
            quote!(
                ::nw_network::serialize::ReplicatedFieldHandler<
                    ::glam::Quat,
                    ::nw_network::serialize::QuatSmallestThreeQuantizedMarshaler,
                >
            )
        }
        SchemaWireShape::NonUniformScaleComp => {
            quote!(
                ::nw_network::serialize::ReplicatedFieldHandler<
                    ::glam::Vec3,
                    ::nw_network::serialize::NonUniformScaleCompMarshaler,
                >
            )
        }
        SchemaWireShape::DeltaVec3(range) => {
            let range = proc_macro2::Literal::u32_unsuffixed(*range);
            quote!(
                ::nw_network::serialize::ReplicatedFieldHandler<
                    ::glam::Vec3,
                    ::nw_network::serialize::DeltaMarshaler<#range, ::glam::Vec3>,
                >
            )
        }
        SchemaWireShape::RemoteServerGdeRef => {
            quote!(
                ::nw_network::serialize::ReplicatedFieldHandler<
                    ::nw_network::source::RemoteServerGDERef,
                    ::nw_network::serialize::RemoteServerGdeRefMarshaler,
                >
            )
        }
        SchemaWireShape::PackedPosition(shape) => {
            let minimum_bits = proc_macro2::Literal::u32_unsuffixed(shape.minimum_bits);
            let maximum_bits = proc_macro2::Literal::u32_unsuffixed(shape.maximum_bits);
            quote!(
                ::nw_network::serialize::ReplicatedFieldHandler<
                    ::glam::Vec3,
                    ::nw_network::serialize::PackedPositionMarshaller<
                        #minimum_bits,
                        #maximum_bits,
                    >,
                >
            )
        }
        SchemaWireShape::TransformCompressor => {
            quote!(
                ::nw_network::serialize::ReplicatedFieldHandler<
                    ::glam::Affine3A,
                    ::nw_network::serialize::TransformCompressor,
                >
            )
        }
        SchemaWireShape::PackedSize => {
            quote!(
                ::nw_network::serialize::ReplicatedFieldHandler<
                    ::nw_network::serialize::PackedSize,
                >
            )
        }
        SchemaWireShape::Mat3 => {
            quote!(::nw_network::serialize::ReplicatedFieldHandler<::glam::Mat3>)
        }
        SchemaWireShape::Affine3 => {
            quote!(::nw_network::serialize::ReplicatedFieldHandler<::glam::Affine3A>)
        }
        SchemaWireShape::Aabb2d => {
            quote!(
                ::nw_network::serialize::ReplicatedFieldHandler<
                    ::bevy_math::bounding::Aabb2d,
                >
            )
        }
        SchemaWireShape::Aabb3d => {
            quote!(
                ::nw_network::serialize::ReplicatedFieldHandler<
                    ::bevy_math::bounding::Aabb3d,
                >
            )
        }
        SchemaWireShape::ActorRef => {
            quote!(::nw_network::serialize::ReplicatedFieldHandler<::nw_network::ActorRef>)
        }
        SchemaWireShape::EntityRef => {
            quote!(::nw_network::serialize::ReplicatedFieldHandler<::nw_network::EntityRef>)
        }
        SchemaWireShape::FixedBytes(len) => {
            let len = unsuffixed_int_lit(*len);
            quote!(::nw_network::serialize::ReplicatedFieldHandler<[u8; #len]>)
        }
        SchemaWireShape::Bytes => {
            quote!(::nw_network::serialize::ReplicatedFieldHandler<Vec<u8>>)
        }
        SchemaWireShape::String => {
            quote!(::nw_network::serialize::ReplicatedFieldHandler<String>)
        }
        SchemaWireShape::Composite(_)
        | SchemaWireShape::Optional(_)
        | SchemaWireShape::DefaultOmitted(_)
        | SchemaWireShape::BooleanChoice(_)
        | SchemaWireShape::Sequence(_)
        | SchemaWireShape::Map { .. }
        | SchemaWireShape::Set(_) => {
            let field_type = rust_field_shape(shape).field_type;
            let field_type = syn::parse_str::<syn::Type>(&field_type)
                .expect("recursive wire shape produces a valid Rust field type");
            quote!(#field_type)
        }
        SchemaWireShape::ReplicatedContainer(_) => {
            unreachable!("container fields require an explicit ReplicatedContainer type")
        }
        SchemaWireShape::FixedSequence(_) => {
            unreachable!("fixed-sequence fields require an explicit ArrayVec type")
        }
    }
}
