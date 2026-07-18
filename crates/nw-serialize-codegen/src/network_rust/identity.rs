use super::*;

pub(super) fn identity_tokens(schema: &NetworkSchema) -> Vec<proc_macro2::TokenStream> {
    let names_by_type_index = identity_names_by_type_index(schema);
    schema
        .types
        .iter()
        .filter_map(|network_type| {
            let type_id = network_type.type_id?;
            let type_index = network_type.type_index?;
            let source_name = network_type.name.as_deref()?;
            let rust_name = names_by_type_index.get(&type_index)?;
            let ident = format_ident!("{rust_name}");
            let type_id = type_id_literal(type_id);
            let name = LitStr::new(source_name, proc_macro2::Span::call_site());
            let capabilities =
                capability_slice_tokens(&network_type.capabilities, Some(quote!(super::)));
            Some(quote! {
                #[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
                pub struct #ident;

                impl super::NetworkTypeIdentity for #ident {
                    const TYPE_ID: ::uuid::Uuid = ::uuid::Uuid::from_u128(#type_id);
                    const TYPE_INDEX: u32 = #type_index;
                    const NAME: &'static str = #name;
                    const CAPABILITIES: &'static [super::NetworkTypeCapability] = #capabilities;
                }
            })
        })
        .collect()
}

pub(super) fn identity_names_by_type_index(schema: &NetworkSchema) -> BTreeMap<u32, String> {
    let mut entries_by_candidate = BTreeMap::<String, Vec<&NetworkType>>::new();
    for network_type in &schema.types {
        let (Some(_), Some(name)) = (network_type.type_index, network_type.name.as_deref()) else {
            continue;
        };
        entries_by_candidate
            .entry(rust_type_ident(name))
            .or_default()
            .push(network_type);
    }

    let mut names_by_type_index = BTreeMap::new();
    for (candidate, mut entries) in entries_by_candidate {
        entries.sort_by(|left, right| {
            left.type_index
                .cmp(&right.type_index)
                .then_with(|| left.name.cmp(&right.name))
        });
        if entries.len() == 1 {
            names_by_type_index.insert(
                entries[0]
                    .type_index
                    .expect("single candidate entry has a type index"),
                candidate,
            );
            continue;
        }
        let namespaced_counts = entries
            .iter()
            .filter_map(|network_type| namespaced_identity_candidate(network_type))
            .fold(BTreeMap::<String, usize>::new(), |mut counts, name| {
                *counts.entry(name).or_default() += 1;
                counts
            });
        for network_type in entries {
            let type_index = network_type
                .type_index
                .expect("collision candidate entry has a type index");
            let name = namespaced_identity_candidate(network_type)
                .filter(|name| namespaced_counts.get(name) == Some(&1))
                .unwrap_or_else(|| {
                    format!("{candidate}{}", identity_collision_suffix(network_type))
                });
            names_by_type_index.insert(type_index, name);
        }
    }
    names_by_type_index
}

pub(super) fn namespaced_identity_candidate(network_type: &NetworkType) -> Option<String> {
    let name = network_type.name.as_deref()?;
    if !name.contains("::") {
        return None;
    }
    let candidate = name
        .split("::")
        .filter(|segment| !segment.is_empty())
        .map(rust_type_ident)
        .collect::<String>();
    (!candidate.is_empty() && candidate != rust_type_ident(name)).then_some(candidate)
}

pub(super) fn identity_collision_suffix(network_type: &NetworkType) -> String {
    match network_type.type_id {
        Some(type_id) if !type_id.is_nil() => short_type_id(type_id),
        _ => format!(
            "TypeIndex{}",
            network_type
                .type_index
                .expect("identity collision candidate has a type index")
        ),
    }
}

pub(super) fn short_type_id(type_id: Uuid) -> String {
    type_id
        .simple()
        .to_string()
        .chars()
        .take(8)
        .collect::<String>()
        .to_ascii_uppercase()
}

pub(super) fn identity_name_collision_count(schema: &NetworkSchema) -> usize {
    let mut counts = BTreeMap::<String, usize>::new();
    for network_type in &schema.types {
        let Some(name) = network_type.name.as_deref() else {
            continue;
        };
        *counts.entry(rust_type_ident(name)).or_default() += 1;
    }
    counts.values().filter(|count| **count > 1).count()
}

pub(super) fn descriptor_tokens(
    network_type: &NetworkType,
    wire_shapes: &BTreeMap<&str, &SchemaWireShape>,
    report: &mut NetworkRustGenerationReport,
) -> Option<proc_macro2::TokenStream> {
    let type_id = match network_type.type_id {
        Some(type_id) => type_id_literal(type_id),
        None => {
            report.skipped_missing_type_id += 1;
            return None;
        }
    };
    let type_index = match network_type.type_index {
        Some(type_index) => type_index,
        None => {
            report.skipped_missing_type_index += 1;
            return None;
        }
    };
    if network_type.name.is_none() {
        report.unnamed_descriptor_count += 1;
    }
    let name = option_str_tokens(network_type.name.as_deref());
    let capability_tokens = capability_slice_tokens(&network_type.capabilities, None);
    let instance_size = option_u32_tokens(
        network_type
            .instance
            .as_ref()
            .and_then(|instance| instance.size),
    );
    count_capabilities(&network_type.capabilities, report);
    let fields = network_type
        .fields
        .iter()
        .filter_map(|field| field_tokens(field, wire_shapes, report))
        .collect::<Vec<_>>();
    report.field_descriptor_count += fields.len();

    Some(quote! {
        NetworkTypeDescriptor {
            type_id: Uuid::from_u128(#type_id),
            type_index: #type_index,
            name: #name,
            capabilities: #capability_tokens,
            instance_size: #instance_size,
            fields: &[
                #(#fields),*
            ],
        }
    })
}

pub(super) fn field_tokens(
    field: &NetworkField,
    wire_shapes: &BTreeMap<&str, &SchemaWireShape>,
    report: &mut NetworkRustGenerationReport,
) -> Option<proc_macro2::TokenStream> {
    let index = field.index?;
    let name = field.name.as_deref()?;
    if !field.confidence.is_high_or_exact() {
        report.low_confidence_field_count += 1;
    }
    let name = LitStr::new(name, proc_macro2::Span::call_site());
    let group = option_u32_tokens(field.group);
    let native_type = option_str_tokens(field.native_type.as_deref());
    let source_type_name = option_str_tokens(field.source_type_name.as_deref());
    let rust_type = option_str_tokens(resolved_field_descriptor_rust_type(field).as_deref());
    let unmarshal_target_name = option_str_tokens(
        field
            .unmarshal_evidence
            .as_ref()
            .and_then(|evidence| evidence.target_name.as_deref()),
    );
    let storage_offset = option_u32_tokens(field.storage_offset);
    let wire_shape = field_wire_shape_tokens(field, wire_shapes, report);
    let confidence = confidence_ident(field.confidence);
    Some(quote! {
        NetworkFieldDescriptor {
            index: #index,
            name: #name,
            group: #group,
            native_type: #native_type,
            source_type_name: #source_type_name,
            rust_type: #rust_type,
            unmarshal_target_name: #unmarshal_target_name,
            storage_offset: #storage_offset,
            wire_shape: #wire_shape,
            confidence: NetworkFieldConfidence::#confidence,
        }
    })
}

pub(super) fn wire_shapes_by_handler_vtable(
    schema: &NetworkSchema,
) -> BTreeMap<&str, &SchemaWireShape> {
    schema
        .field_handler_vtables
        .iter()
        .filter_map(|vtable| {
            let address = vtable.address.as_deref()?;
            let shape = vtable.wire_shape.as_ref()?;
            Some((address, shape))
        })
        .collect()
}

pub(super) fn handler_vtables_by_address(
    schema: &NetworkSchema,
) -> BTreeMap<&str, &NetworkFieldHandlerVtable> {
    schema
        .field_handler_vtables
        .iter()
        .filter_map(|vtable| Some((vtable.address.as_deref()?, vtable)))
        .collect()
}

pub(super) fn serialize_types_by_type_id(
    schema: &NetworkSchema,
) -> BTreeMap<Uuid, &NetworkSerializeType> {
    let mut types = schema
        .serialize_types
        .iter()
        .map(|serialize| (serialize.type_id, serialize))
        .collect::<BTreeMap<_, _>>();
    types.extend(schema.types.iter().filter_map(|network_type| {
        let serialize = network_type.serialize.as_ref()?;
        Some((serialize.type_id, serialize))
    }));
    types
}

pub(super) fn field_wire_shape_tokens(
    field: &NetworkField,
    wire_shapes: &BTreeMap<&str, &SchemaWireShape>,
    report: &mut NetworkRustGenerationReport,
) -> proc_macro2::TokenStream {
    if let Some(shape) = field_wire_shape(field, wire_shapes) {
        report.field_wire_shape_count += 1;
        let shape = wire_shape_tokens(shape);
        return quote!(Some(#shape));
    }
    if field.handler_vtable.is_some() {
        report.unresolved_field_wire_shape_count += 1;
    }
    quote!(None)
}
