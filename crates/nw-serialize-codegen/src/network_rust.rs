use std::collections::{BTreeMap, BTreeSet};

use quote::{format_ident, quote};
use serde::{Deserialize, Serialize};
use syn::LitStr;
use thiserror::Error;
use uuid::Uuid;

use crate::naming::{rust_field_ident, rust_module_ident, rust_type_ident};
use crate::network_schema::{
    NetworkConfidence, NetworkField, NetworkRootKind, NetworkSchema, NetworkType,
    NetworkWireShape as SchemaWireShape,
};

pub const NETWORK_RUST_EMITTER_VERSION: &str = "network-rust-v5";

#[derive(Debug, Error)]
pub enum NetworkRustEmitError {
    #[error("generated network Rust source did not parse")]
    Parse(#[from] syn::Error),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NetworkRustOutput {
    pub source: String,
    pub report: NetworkRustGenerationReport,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NetworkRustGenerationReport {
    pub descriptor_count: usize,
    pub identity_type_count: usize,
    pub identity_name_collision_count: usize,
    pub field_descriptor_count: usize,
    pub unnamed_descriptor_count: usize,
    pub skipped_missing_type_id: usize,
    pub skipped_missing_type_index: usize,
    pub skipped_missing_name: usize,
    pub replicated_state_count: usize,
    pub message_count: usize,
    pub field_registered_count: usize,
    pub support_type_count: usize,
    pub low_confidence_field_count: usize,
    pub field_wire_shape_count: usize,
    pub unresolved_field_wire_shape_count: usize,
    pub state_generation_plan_count: usize,
    pub generatable_state_count: usize,
    pub blocked_state_count: usize,
    pub state_generation_plans: Vec<NetworkStateGenerationPlanReport>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NetworkStateGenerationPlanReport {
    pub type_index: Option<u32>,
    pub type_name: Option<String>,
    pub field_count: usize,
    pub shaped_field_count: usize,
    pub supported_field_count: usize,
    pub missing_wire_shape_count: usize,
    pub unsupported_wire_shape_count: usize,
    pub low_confidence_field_count: usize,
    pub can_generate: bool,
    pub blocked_reasons: Vec<String>,
    pub fields: Vec<NetworkStateFieldShapeReport>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NetworkStateFieldShapeReport {
    pub field_index: Option<u32>,
    pub field_name: Option<String>,
    pub group: Option<u32>,
    pub handler_vtable: Option<String>,
    pub wire_shape: Option<SchemaWireShape>,
    pub wire_shape_source: Option<String>,
    pub rust_value_type: Option<String>,
    pub rust_field_type: Option<String>,
    pub confidence: NetworkConfidence,
    pub supported: bool,
    pub blocked_reason: Option<String>,
}

#[derive(Debug, Default)]
pub struct NetworkRustEmitter;

impl NetworkRustEmitter {
    pub fn emit_descriptors(
        schema: &NetworkSchema,
    ) -> Result<NetworkRustOutput, NetworkRustEmitError> {
        let mut report = NetworkRustGenerationReport::default();
        let wire_shapes = wire_shapes_by_handler_vtable(schema);
        let descriptors = schema
            .types
            .iter()
            .filter_map(|network_type| descriptor_tokens(network_type, &wire_shapes, &mut report))
            .collect::<Vec<_>>();
        report.descriptor_count = descriptors.len();
        report.identity_name_collision_count = identity_name_collision_count(schema);
        report.state_generation_plans = state_generation_plans(schema, &wire_shapes);
        report.state_generation_plan_count = report.state_generation_plans.len();
        report.generatable_state_count = report
            .state_generation_plans
            .iter()
            .filter(|plan| plan.can_generate)
            .count();
        report.blocked_state_count =
            report.state_generation_plan_count - report.generatable_state_count;
        let identities = identity_tokens(schema);
        report.identity_type_count = identities.len();

        let tokens = quote! {
            #![allow(clippy::unreadable_literal)]

            use std::collections::BTreeSet;
            use uuid::Uuid;

            #[derive(Debug, Clone, Copy, PartialEq, Eq)]
            pub enum NetworkTypeKind {
                ReplicatedState,
                Message,
                FieldRegisteredType,
                SupportType,
            }

            #[derive(Debug, Clone, Copy, PartialEq, Eq)]
            pub enum NetworkFieldConfidence {
                Exact,
                High,
                Inferred,
                Weak,
                Unknown,
            }

            #[derive(Debug, Clone, Copy, PartialEq, Eq)]
            pub enum NetworkWireShape {
                Bool,
                U8,
                U16,
                U32,
                U64,
                F32,
                HalfF32,
                VlqU32,
                QuatCompNorm,
            }

            #[derive(Debug, Clone, Copy, PartialEq, Eq)]
            pub struct NetworkFieldDescriptor {
                pub index: u32,
                pub name: &'static str,
                pub group: Option<u32>,
                pub wire_shape: Option<NetworkWireShape>,
                pub confidence: NetworkFieldConfidence,
            }

            #[derive(Debug, Clone, Copy, PartialEq, Eq)]
            pub struct NetworkTypeDescriptor {
                pub type_id: Uuid,
                pub type_index: u32,
                pub name: Option<&'static str>,
                pub kind: NetworkTypeKind,
                pub is_field_registered: bool,
                pub fields: &'static [NetworkFieldDescriptor],
            }

            impl NetworkFieldDescriptor {
                #[must_use]
                pub const fn has_wire_shape(&self) -> bool {
                    self.wire_shape.is_some()
                }
            }

            impl NetworkTypeDescriptor {
                #[must_use]
                pub fn field_by_index(&self, field_index: u32) -> Option<&NetworkFieldDescriptor> {
                    self.fields.iter().find(|field| field.index == field_index)
                }

                #[must_use]
                pub fn has_complete_field_wire_shapes(&self) -> bool {
                    self.fields.iter().all(NetworkFieldDescriptor::has_wire_shape)
                }

                #[must_use]
                pub fn missing_field_wire_shape_count(&self) -> usize {
                    self.fields
                        .iter()
                        .filter(|field| !field.has_wire_shape())
                        .count()
                }
            }

            pub trait NetworkTypeIdentity {
                const TYPE_ID: Uuid;
                const TYPE_INDEX: u32;
                const NAME: &'static str;
                const KIND: NetworkTypeKind;

                #[must_use]
                fn descriptor() -> &'static NetworkTypeDescriptor {
                    type_by_type_index(Self::TYPE_INDEX)
                        .expect("generated network identity must have a descriptor")
                }
            }

            pub mod identity {
                #(#identities)*
            }

            pub const NETWORK_TYPES: &[NetworkTypeDescriptor] = &[
                #(#descriptors),*
            ];

            #[must_use]
            pub fn type_by_type_index(type_index: u32) -> Option<&'static NetworkTypeDescriptor> {
                NETWORK_TYPES
                    .iter()
                    .find(|descriptor| descriptor.type_index == type_index)
            }

            #[must_use]
            pub fn type_by_type_id(type_id: Uuid) -> Option<&'static NetworkTypeDescriptor> {
                NETWORK_TYPES
                    .iter()
                    .find(|descriptor| descriptor.type_id == type_id)
            }

            #[must_use]
            pub fn name_for_type_index(type_index: u32) -> Option<&'static str> {
                type_by_type_index(type_index).and_then(|descriptor| descriptor.name)
            }

            #[must_use]
            pub fn is_known_type_index(type_index: u32) -> bool {
                type_by_type_index(type_index).is_some()
            }

            #[must_use]
            pub fn is_replicated_state_type_index(type_index: u32) -> bool {
                type_by_type_index(type_index)
                    .is_some_and(|descriptor| descriptor.kind == NetworkTypeKind::ReplicatedState)
            }

            #[must_use]
            pub fn fields_for_type_index(
                type_index: u32,
            ) -> Option<&'static [NetworkFieldDescriptor]> {
                type_by_type_index(type_index).map(|descriptor| descriptor.fields)
            }

            #[must_use]
            pub fn field_for_type_index(
                type_index: u32,
                field_index: u32,
            ) -> Option<&'static NetworkFieldDescriptor> {
                type_by_type_index(type_index)
                    .and_then(|descriptor| descriptor.field_by_index(field_index))
            }

            pub fn unknown_type_indices(
                type_indices: impl IntoIterator<Item = u32>,
            ) -> Vec<u32> {
                type_indices
                    .into_iter()
                    .filter(|type_index| !is_known_type_index(*type_index))
                    .collect::<BTreeSet<_>>()
                    .into_iter()
                    .collect()
            }

            pub fn non_replicated_state_type_indices(
                type_indices: impl IntoIterator<Item = u32>,
            ) -> Vec<u32> {
                type_indices
                    .into_iter()
                    .filter(|type_index| {
                        type_by_type_index(*type_index)
                            .is_some_and(|descriptor| descriptor.kind != NetworkTypeKind::ReplicatedState)
                    })
                    .collect::<BTreeSet<_>>()
                    .into_iter()
                    .collect()
            }

            pub fn type_indices_missing_field_wire_shapes(
                type_indices: impl IntoIterator<Item = u32>,
            ) -> Vec<u32> {
                type_indices
                    .into_iter()
                    .filter(|type_index| {
                        type_by_type_index(*type_index)
                            .is_some_and(|descriptor| descriptor.missing_field_wire_shape_count() > 0)
                    })
                    .collect::<BTreeSet<_>>()
                    .into_iter()
                    .collect()
            }
        };
        let file = syn::parse2(tokens)?;
        Ok(NetworkRustOutput {
            source: prettyplease::unparse(&file),
            report,
        })
    }

    pub fn emit_replicated_states(
        schema: &NetworkSchema,
        type_indices: impl IntoIterator<Item = u32>,
    ) -> Result<NetworkRustOutput, NetworkRustEmitError> {
        let selected = type_indices.into_iter().collect::<BTreeSet<_>>();
        let wire_shapes = wire_shapes_by_handler_vtable(schema);
        let wire_shape_sources = wire_shape_sources_by_handler_vtable(schema);
        let rust_names = identity_names_by_type_index(schema);
        let types_by_type_index = schema
            .types
            .iter()
            .filter_map(|network_type| Some((network_type.type_index?, network_type)))
            .collect::<BTreeMap<_, _>>();
        let plans_by_type_index = schema
            .types
            .iter()
            .filter(|network_type| {
                network_type
                    .root_kinds
                    .contains(&NetworkRootKind::ReplicatedState)
            })
            .filter_map(|network_type| {
                Some((
                    network_type.type_index?,
                    state_generation_plan(network_type, &wire_shapes, &wire_shape_sources),
                ))
            })
            .collect::<BTreeMap<_, _>>();

        let mut report = NetworkRustGenerationReport::default();
        let mut modules = Vec::new();
        for type_index in selected {
            let Some(network_type) = types_by_type_index.get(&type_index).copied() else {
                report
                    .state_generation_plans
                    .push(blocked_state_generation_plan(
                        Some(type_index),
                        None,
                        "missing-network-type",
                    ));
                continue;
            };
            let Some(plan) = plans_by_type_index.get(&type_index) else {
                report
                    .state_generation_plans
                    .push(blocked_state_generation_plan(
                        Some(type_index),
                        network_type.name.clone(),
                        "not-replicated-state",
                    ));
                continue;
            };
            report.state_generation_plans.push(plan.clone());
            if plan.can_generate {
                modules.push(replicated_state_module_tokens(
                    network_type,
                    plan,
                    &rust_names,
                ));
            }
        }

        report.state_generation_plan_count = report.state_generation_plans.len();
        report.generatable_state_count = report
            .state_generation_plans
            .iter()
            .filter(|plan| plan.can_generate)
            .count();
        report.blocked_state_count =
            report.state_generation_plan_count - report.generatable_state_count;
        report.replicated_state_count = report.generatable_state_count;

        let tokens = quote! {
            #![allow(clippy::unreadable_literal)]

            #(#modules)*
        };
        let file = syn::parse2(tokens)?;
        Ok(NetworkRustOutput {
            source: prettyplease::unparse(&file),
            report,
        })
    }
}

fn identity_tokens(schema: &NetworkSchema) -> Vec<proc_macro2::TokenStream> {
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
            let kind_ident = network_type_kind_ident(root_kind(network_type));
            Some(quote! {
                #[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
                pub struct #ident;

                impl super::NetworkTypeIdentity for #ident {
                    const TYPE_ID: ::uuid::Uuid = ::uuid::Uuid::from_u128(#type_id);
                    const TYPE_INDEX: u32 = #type_index;
                    const NAME: &'static str = #name;
                    const KIND: super::NetworkTypeKind = super::NetworkTypeKind::#kind_ident;
                }
            })
        })
        .collect()
}

fn identity_names_by_type_index(schema: &NetworkSchema) -> BTreeMap<u32, String> {
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
        for network_type in entries {
            let type_index = network_type
                .type_index
                .expect("collision candidate entry has a type index");
            names_by_type_index.insert(
                type_index,
                format!("{candidate}{}", identity_collision_suffix(network_type)),
            );
        }
    }
    names_by_type_index
}

fn identity_collision_suffix(network_type: &NetworkType) -> String {
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

fn short_type_id(type_id: Uuid) -> String {
    type_id
        .simple()
        .to_string()
        .chars()
        .take(8)
        .collect::<String>()
        .to_ascii_uppercase()
}

fn identity_name_collision_count(schema: &NetworkSchema) -> usize {
    let mut counts = BTreeMap::<String, usize>::new();
    for network_type in &schema.types {
        let Some(name) = network_type.name.as_deref() else {
            continue;
        };
        *counts.entry(rust_type_ident(name)).or_default() += 1;
    }
    counts.values().filter(|count| **count > 1).count()
}

fn descriptor_tokens(
    network_type: &NetworkType,
    wire_shapes: &BTreeMap<&str, SchemaWireShape>,
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
    let kind = root_kind(network_type);
    let kind_ident = network_type_kind_ident(kind);
    count_kind(kind, report);
    if network_type
        .root_kinds
        .contains(&NetworkRootKind::FieldRegisteredType)
    {
        report.field_registered_count += 1;
    }
    let is_field_registered = network_type
        .root_kinds
        .contains(&NetworkRootKind::FieldRegisteredType);
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
            kind: NetworkTypeKind::#kind_ident,
            is_field_registered: #is_field_registered,
            fields: &[
                #(#fields),*
            ],
        }
    })
}

fn field_tokens(
    field: &NetworkField,
    wire_shapes: &BTreeMap<&str, SchemaWireShape>,
    report: &mut NetworkRustGenerationReport,
) -> Option<proc_macro2::TokenStream> {
    let index = field.index?;
    let name = field.name.as_deref()?;
    if !field.confidence.is_high_or_exact() {
        report.low_confidence_field_count += 1;
    }
    let name = LitStr::new(name, proc_macro2::Span::call_site());
    let group = option_u32_tokens(field.group);
    let wire_shape = field_wire_shape_tokens(field, wire_shapes, report);
    let confidence = confidence_ident(field.confidence);
    Some(quote! {
        NetworkFieldDescriptor {
            index: #index,
            name: #name,
            group: #group,
            wire_shape: #wire_shape,
            confidence: NetworkFieldConfidence::#confidence,
        }
    })
}

fn wire_shapes_by_handler_vtable(schema: &NetworkSchema) -> BTreeMap<&str, SchemaWireShape> {
    schema
        .field_handler_vtables
        .iter()
        .filter_map(|vtable| {
            let address = vtable.address.as_deref()?;
            let shape = vtable.wire_shape?;
            Some((address, shape))
        })
        .collect()
}

fn field_wire_shape_tokens(
    field: &NetworkField,
    wire_shapes: &BTreeMap<&str, SchemaWireShape>,
    report: &mut NetworkRustGenerationReport,
) -> proc_macro2::TokenStream {
    let Some(handler_vtable) = field.handler_vtable.as_deref() else {
        return quote!(None);
    };
    let Some(shape) = wire_shapes.get(handler_vtable).copied() else {
        report.unresolved_field_wire_shape_count += 1;
        return quote!(None);
    };
    report.field_wire_shape_count += 1;
    let shape = wire_shape_ident(shape);
    quote!(Some(NetworkWireShape::#shape))
}

fn state_generation_plans(
    schema: &NetworkSchema,
    wire_shapes: &BTreeMap<&str, SchemaWireShape>,
) -> Vec<NetworkStateGenerationPlanReport> {
    let wire_shape_sources = wire_shape_sources_by_handler_vtable(schema);
    schema
        .types
        .iter()
        .filter(|network_type| {
            network_type
                .root_kinds
                .contains(&NetworkRootKind::ReplicatedState)
        })
        .map(|network_type| state_generation_plan(network_type, wire_shapes, &wire_shape_sources))
        .collect()
}

fn state_generation_plan(
    network_type: &NetworkType,
    wire_shapes: &BTreeMap<&str, SchemaWireShape>,
    wire_shape_sources: &BTreeMap<&str, &str>,
) -> NetworkStateGenerationPlanReport {
    let fields = network_type
        .fields
        .iter()
        .map(|field| state_field_shape_report(field, wire_shapes, wire_shape_sources))
        .collect::<Vec<_>>();
    let field_count = fields.len();
    let shaped_field_count = fields
        .iter()
        .filter(|field| field.wire_shape.is_some())
        .count();
    let supported_field_count = fields.iter().filter(|field| field.supported).count();
    let missing_wire_shape_count = fields
        .iter()
        .filter(|field| field.wire_shape.is_none())
        .count();
    let unsupported_wire_shape_count = 0;
    let invalid_field_metadata_count = fields
        .iter()
        .filter(|field| {
            matches!(
                field.blocked_reason.as_deref(),
                Some("missing-field-index" | "missing-field-name")
            )
        })
        .count();
    let low_confidence_field_count = fields
        .iter()
        .filter(|field| !field.confidence.is_high_or_exact())
        .count();
    let blocked_reasons = state_blocked_reasons(
        network_type,
        field_count,
        missing_wire_shape_count,
        unsupported_wire_shape_count,
        invalid_field_metadata_count,
        low_confidence_field_count,
    );
    NetworkStateGenerationPlanReport {
        type_index: network_type.type_index,
        type_name: network_type.name.clone(),
        field_count,
        shaped_field_count,
        supported_field_count,
        missing_wire_shape_count,
        unsupported_wire_shape_count,
        low_confidence_field_count,
        can_generate: blocked_reasons.is_empty(),
        blocked_reasons,
        fields,
    }
}

fn state_field_shape_report(
    field: &NetworkField,
    wire_shapes: &BTreeMap<&str, SchemaWireShape>,
    wire_shape_sources: &BTreeMap<&str, &str>,
) -> NetworkStateFieldShapeReport {
    let shape = field
        .handler_vtable
        .as_deref()
        .and_then(|handler_vtable| wire_shapes.get(handler_vtable).copied());
    let rust_shape = shape.map(rust_field_shape);
    let blocked_reason = field_blocked_reason(field, shape);
    NetworkStateFieldShapeReport {
        field_index: field.index,
        field_name: field.name.clone(),
        group: field.group,
        handler_vtable: field.handler_vtable.clone(),
        wire_shape: shape,
        wire_shape_source: field
            .handler_vtable
            .as_deref()
            .and_then(|handler_vtable| wire_shape_sources.get(handler_vtable).copied())
            .map(ToOwned::to_owned),
        rust_value_type: rust_shape.map(|shape| shape.value_type.to_owned()),
        rust_field_type: rust_shape.map(|shape| shape.field_type.to_owned()),
        confidence: field.confidence,
        supported: blocked_reason.is_none(),
        blocked_reason,
    }
}

fn wire_shape_sources_by_handler_vtable(schema: &NetworkSchema) -> BTreeMap<&str, &str> {
    schema
        .field_handler_vtables
        .iter()
        .filter_map(|vtable| {
            Some((
                vtable.address.as_deref()?,
                vtable.wire_shape_source.as_deref()?,
            ))
        })
        .collect()
}

fn state_blocked_reasons(
    network_type: &NetworkType,
    field_count: usize,
    missing_wire_shape_count: usize,
    unsupported_wire_shape_count: usize,
    invalid_field_metadata_count: usize,
    low_confidence_field_count: usize,
) -> Vec<String> {
    let mut reasons = Vec::new();
    if network_type.type_index.is_none() {
        reasons.push("missing-type-index".to_owned());
    }
    if network_type.name.is_none() {
        reasons.push("missing-type-name".to_owned());
    }
    if field_count == 0 {
        reasons.push("no-registered-fields".to_owned());
    }
    if missing_wire_shape_count != 0 {
        reasons.push(format!("missing-wire-shape:{missing_wire_shape_count}"));
    }
    if unsupported_wire_shape_count != 0 {
        reasons.push(format!(
            "unsupported-wire-shape:{unsupported_wire_shape_count}"
        ));
    }
    if invalid_field_metadata_count != 0 {
        reasons.push(format!(
            "invalid-field-metadata:{invalid_field_metadata_count}"
        ));
    }
    if low_confidence_field_count != 0 {
        reasons.push(format!("low-confidence-field:{low_confidence_field_count}"));
    }
    reasons
}

fn field_blocked_reason(field: &NetworkField, shape: Option<SchemaWireShape>) -> Option<String> {
    if field.index.is_none() {
        return Some("missing-field-index".to_owned());
    }
    if field.name.is_none() {
        return Some("missing-field-name".to_owned());
    }
    if !field.confidence.is_high_or_exact() {
        return Some("low-confidence-field".to_owned());
    }
    if shape.is_none() {
        return Some("missing-wire-shape".to_owned());
    }
    None
}

#[derive(Debug, Clone, Copy)]
struct RustFieldShape {
    value_type: &'static str,
    field_type: &'static str,
}

const fn rust_field_shape(shape: SchemaWireShape) -> RustFieldShape {
    match shape {
        SchemaWireShape::Bool => RustFieldShape {
            value_type: "bool",
            field_type: "ReplicatedFieldHandler<bool>",
        },
        SchemaWireShape::U8 => RustFieldShape {
            value_type: "u8",
            field_type: "ReplicatedFieldHandler<u8>",
        },
        SchemaWireShape::U16 => RustFieldShape {
            value_type: "u16",
            field_type: "ReplicatedFieldHandler<u16>",
        },
        SchemaWireShape::U32 => RustFieldShape {
            value_type: "u32",
            field_type: "ReplicatedFieldHandler<u32>",
        },
        SchemaWireShape::U64 => RustFieldShape {
            value_type: "u64",
            field_type: "ReplicatedFieldHandler<u64>",
        },
        SchemaWireShape::F32 => RustFieldShape {
            value_type: "f32",
            field_type: "ReplicatedFieldHandler<f32>",
        },
        SchemaWireShape::HalfF32 => RustFieldShape {
            value_type: "f32",
            field_type: "ReplicatedFieldHandler<f32, HalfF32Marshaler>",
        },
        SchemaWireShape::VlqU32 => RustFieldShape {
            value_type: "u32",
            field_type: "ReplicatedFieldHandler<u32, VlqU32Marshaler>",
        },
        SchemaWireShape::QuatCompNorm => RustFieldShape {
            value_type: "QuatCompNorm",
            field_type: "ReplicatedFieldHandler<QuatCompNorm>",
        },
    }
}

fn blocked_state_generation_plan(
    type_index: Option<u32>,
    type_name: Option<String>,
    reason: &str,
) -> NetworkStateGenerationPlanReport {
    NetworkStateGenerationPlanReport {
        type_index,
        type_name,
        field_count: 0,
        shaped_field_count: 0,
        supported_field_count: 0,
        missing_wire_shape_count: 0,
        unsupported_wire_shape_count: 0,
        low_confidence_field_count: 0,
        can_generate: false,
        blocked_reasons: vec![reason.to_owned()],
        fields: Vec::new(),
    }
}

fn replicated_state_module_tokens(
    network_type: &NetworkType,
    plan: &NetworkStateGenerationPlanReport,
    rust_names: &BTreeMap<u32, String>,
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

    quote! {
        pub mod #module_ident {
            use ::nw_network::{AzRtti, ReplicatedState, TypeRegistry};

            #[derive(Debug, Clone, Default, ReplicatedState, AzRtti, TypeRegistry)]
            #[az_rtti(#type_id)]
            #[type_registry(#type_index)]
            pub struct #state_ident {
                #(#fields)*

                pub hub: ::nw_network::hub::ReplicatedState,
            }
        }

        pub use #module_ident::#state_ident;
    }
}

fn replicated_state_field_tokens(field: &NetworkStateFieldShapeReport) -> proc_macro2::TokenStream {
    let field_name = field
        .field_name
        .as_deref()
        .expect("generatable replicated state field has a name");
    let field_ident = format_ident!("{}", rust_field_ident(field_name));
    let group_attr = match field.group {
        Some(0) | None => quote! {},
        Some(group) => quote! { #[replicated_state(group = #group)] },
    };
    let field_type = replicated_state_field_type_tokens(
        field
            .wire_shape
            .expect("generatable replicated state field has a wire shape"),
    );

    quote! {
        #group_attr
        pub #field_ident: #field_type,
    }
}

fn replicated_state_field_type_tokens(shape: SchemaWireShape) -> proc_macro2::TokenStream {
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
        SchemaWireShape::QuatCompNorm => {
            quote!(
                ::nw_network::serialize::ReplicatedFieldHandler<
                    ::nw_network::serialize::QuatCompNorm,
                >
            )
        }
    }
}

fn root_kind(network_type: &NetworkType) -> NetworkRootKind {
    if network_type
        .root_kinds
        .contains(&NetworkRootKind::ReplicatedState)
    {
        NetworkRootKind::ReplicatedState
    } else if network_type.root_kinds.contains(&NetworkRootKind::Message) {
        NetworkRootKind::Message
    } else if network_type
        .root_kinds
        .contains(&NetworkRootKind::FieldRegisteredType)
    {
        NetworkRootKind::FieldRegisteredType
    } else {
        NetworkRootKind::SupportType
    }
}

fn count_kind(kind: NetworkRootKind, report: &mut NetworkRustGenerationReport) {
    match kind {
        NetworkRootKind::ReplicatedState => report.replicated_state_count += 1,
        NetworkRootKind::Message => report.message_count += 1,
        NetworkRootKind::FieldRegisteredType => {}
        NetworkRootKind::SupportType => report.support_type_count += 1,
    }
}

fn network_type_kind_ident(kind: NetworkRootKind) -> proc_macro2::Ident {
    match kind {
        NetworkRootKind::ReplicatedState => format_ident!("ReplicatedState"),
        NetworkRootKind::Message => format_ident!("Message"),
        NetworkRootKind::FieldRegisteredType => format_ident!("FieldRegisteredType"),
        NetworkRootKind::SupportType => format_ident!("SupportType"),
    }
}

fn confidence_ident(confidence: NetworkConfidence) -> proc_macro2::Ident {
    match confidence {
        NetworkConfidence::Exact => format_ident!("Exact"),
        NetworkConfidence::High => format_ident!("High"),
        NetworkConfidence::Inferred => format_ident!("Inferred"),
        NetworkConfidence::Weak => format_ident!("Weak"),
        NetworkConfidence::Unknown => format_ident!("Unknown"),
    }
}

fn wire_shape_ident(shape: SchemaWireShape) -> proc_macro2::Ident {
    match shape {
        SchemaWireShape::Bool => format_ident!("Bool"),
        SchemaWireShape::U8 => format_ident!("U8"),
        SchemaWireShape::U16 => format_ident!("U16"),
        SchemaWireShape::U32 => format_ident!("U32"),
        SchemaWireShape::U64 => format_ident!("U64"),
        SchemaWireShape::F32 => format_ident!("F32"),
        SchemaWireShape::HalfF32 => format_ident!("HalfF32"),
        SchemaWireShape::VlqU32 => format_ident!("VlqU32"),
        SchemaWireShape::QuatCompNorm => format_ident!("QuatCompNorm"),
    }
}

fn option_u32_tokens(value: Option<u32>) -> proc_macro2::TokenStream {
    match value {
        Some(value) => quote!(Some(#value)),
        None => quote!(None),
    }
}

fn option_str_tokens(value: Option<&str>) -> proc_macro2::TokenStream {
    match value {
        Some(value) => {
            let value = LitStr::new(value, proc_macro2::Span::call_site());
            quote!(Some(#value))
        }
        None => quote!(None),
    }
}

fn type_id_literal(type_id: Uuid) -> proc_macro2::TokenStream {
    format!("0x{:032x}", type_id.as_u128())
        .parse()
        .expect("formatted UUID u128 literal is valid Rust")
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use crate::network_schema::NetworkSchema;

    use super::*;

    #[test]
    fn emits_compile_ready_descriptor_module() {
        let schema = NetworkSchema::from_ghidra_static_network_report(&json!({
            "registryEntries": [{
                "uuid": "A85DF621-DCE0-409F-8D39-A447EA0807FF",
                "typeIndex": 28,
                "typeName": "Javelin::RaidDataComponentReplicatedState",
                "fields": [{
                    "index": 0,
                    "name": "raidId",
                    "group": 0,
                    "handlerVtable": "NewWorld+0x81dad80",
                    "confidence": "register-field-call"
                }]
            }],
            "fieldRegistrationFunctions": [],
            "fieldHandlerVtables": [{
                "address": "NewWorld+0x81dad80",
                "fieldCount": 1,
                "marshal": "NewWorld+0x344a700",
                "marshalTarget": "NewWorld+0x17266c0",
                "unmarshal": "NewWorld+0x3464830",
                "wireShape": "u64",
                "wireShapeSource": "marshal-call:marshal-function-name",
                "slots": []
            }]
        }))
        .expect("schema");

        let output = NetworkRustEmitter::emit_descriptors(&schema).expect("rust source");

        assert_eq!(output.report.descriptor_count, 1);
        assert_eq!(output.report.identity_type_count, 1);
        assert_eq!(output.report.field_descriptor_count, 1);
        assert_eq!(output.report.field_wire_shape_count, 1);
        assert_eq!(output.report.unresolved_field_wire_shape_count, 0);
        assert_eq!(output.report.state_generation_plan_count, 1);
        assert_eq!(output.report.generatable_state_count, 1);
        assert_eq!(output.report.blocked_state_count, 0);
        assert_eq!(output.report.replicated_state_count, 1);
        let state_plan = &output.report.state_generation_plans[0];
        assert!(state_plan.can_generate);
        assert_eq!(
            state_plan.type_name.as_deref(),
            Some("Javelin::RaidDataComponentReplicatedState")
        );
        assert_eq!(state_plan.field_count, 1);
        assert_eq!(state_plan.shaped_field_count, 1);
        assert_eq!(state_plan.supported_field_count, 1);
        assert_eq!(
            state_plan.fields[0].rust_field_type.as_deref(),
            Some("ReplicatedFieldHandler<u64>")
        );
        assert!(output.source.contains("pub trait NetworkTypeIdentity"));
        assert!(output.source.contains("pub mod identity"));
        assert!(output.source.contains("pub enum NetworkWireShape"));
        assert!(output.source.contains("pub fn field_by_index"));
        assert!(output.source.contains("pub fn field_for_type_index"));
        assert!(
            output
                .source
                .contains("pub fn type_indices_missing_field_wire_shapes")
        );
        assert!(
            output
                .source
                .contains("pub struct RaidDataComponentReplicatedState")
        );
        assert!(
            output
                .source
                .contains("pub const NETWORK_TYPES: &[NetworkTypeDescriptor]")
        );
        assert!(output.source.contains("is_replicated_state_type_index"));
        assert!(output.source.contains("non_replicated_state_type_indices"));
        assert!(
            output
                .source
                .contains("Javelin::RaidDataComponentReplicatedState")
        );
        assert!(
            output
                .source
                .contains("name: Some(\"Javelin::RaidDataComponentReplicatedState\")")
        );
        assert!(output.source.contains("raidId"));
        assert!(
            output
                .source
                .contains("wire_shape: Some(NetworkWireShape::U64)")
        );
        assert!(output.source.contains("unknown_type_indices"));

        let state_output =
            NetworkRustEmitter::emit_replicated_states(&schema, [28]).expect("state source");

        assert_eq!(state_output.report.state_generation_plan_count, 1);
        assert_eq!(state_output.report.generatable_state_count, 1);
        assert_eq!(state_output.report.blocked_state_count, 0);
        assert!(
            state_output
                .source
                .contains("pub mod raid_data_component_replicated_state")
        );
        assert!(
            state_output
                .source
                .contains("pub struct RaidDataComponentReplicatedState")
        );
        assert!(state_output.source.contains("pub raid_id:"));
        assert!(
            state_output
                .source
                .contains("#[az_rtti(\"A85DF621-DCE0-409F-8D39-A447EA0807FF\")]")
        );
        assert!(state_output.source.contains("type_registry"));
        assert!(state_output.source.contains("28"));
        assert!(
            state_output
                .source
                .contains("pub use raid_data_component_replicated_state")
        );
    }

    #[test]
    fn reports_selected_replicated_states_that_cannot_be_generated() {
        let schema = NetworkSchema::from_ghidra_static_network_report(&json!({
            "registryEntries": [{
                "uuid": "A85DF621-DCE0-409F-8D39-A447EA0807FF",
                "typeIndex": 28,
                "typeName": "Javelin::RaidDataComponentReplicatedState",
                "fields": []
            }],
            "fieldRegistrationFunctions": []
        }))
        .expect("schema");

        let output =
            NetworkRustEmitter::emit_replicated_states(&schema, [28, 29]).expect("state source");

        assert_eq!(output.report.state_generation_plan_count, 2);
        assert_eq!(output.report.generatable_state_count, 0);
        assert_eq!(output.report.blocked_state_count, 2);
        assert_eq!(
            output.report.state_generation_plans[0].blocked_reasons,
            vec!["no-registered-fields"]
        );
        assert_eq!(
            output.report.state_generation_plans[1].blocked_reasons,
            vec!["missing-network-type"]
        );
        assert!(
            !output
                .source
                .contains("pub struct RaidDataComponentReplicatedState")
        );
    }

    #[test]
    fn emits_unnamed_registry_entries_as_descriptors() {
        let schema = NetworkSchema::from_ghidra_static_network_report(&json!({
            "registryEntries": [{
                "uuid": "6C735DB3-871C-4762-A02C-1DA6B5DAB7E9",
                "typeIndex": 67
            }],
            "fieldRegistrationFunctions": []
        }))
        .expect("schema");

        let output = NetworkRustEmitter::emit_descriptors(&schema).expect("rust source");

        assert_eq!(output.report.descriptor_count, 1);
        assert_eq!(output.report.identity_type_count, 0);
        assert_eq!(output.report.unnamed_descriptor_count, 1);
        assert_eq!(output.report.skipped_missing_name, 0);
        assert!(output.source.contains("type_index: 67"));
        assert!(output.source.contains("name: None"));
    }

    #[test]
    fn emits_identity_for_nil_uuid_descriptor() {
        let schema = NetworkSchema::from_ghidra_static_network_report(&json!({
            "registryEntries": [{
                "uuid": "00000000-0000-0000-0000-000000000000",
                "typeIndex": 0,
                "typeName": "NullType",
                "fields": []
            }],
            "fieldRegistrationFunctions": []
        }))
        .expect("schema");

        let output = NetworkRustEmitter::emit_descriptors(&schema).expect("rust source");

        assert_eq!(output.report.descriptor_count, 1);
        assert_eq!(output.report.identity_type_count, 1);
        assert!(output.source.contains("pub struct NullType"));
    }

    #[test]
    fn suffixes_identity_leaf_name_collisions() {
        let schema = NetworkSchema::from_ghidra_static_network_report(&json!({
            "registryEntries": [
                {
                    "uuid": "11111111-1111-1111-1111-111111111111",
                    "typeIndex": 10,
                    "typeName": "First::SharedName",
                    "fields": []
                },
                {
                    "uuid": "22222222-2222-2222-2222-222222222222",
                    "typeIndex": 11,
                    "typeName": "Second::SharedName",
                    "fields": []
                }
            ],
            "fieldRegistrationFunctions": []
        }))
        .expect("schema");

        let output = NetworkRustEmitter::emit_descriptors(&schema).expect("rust source");

        assert_eq!(output.report.identity_name_collision_count, 1);
        assert_eq!(output.report.identity_type_count, 2);
        assert!(output.source.contains("pub struct SharedName11111111"));
        assert!(output.source.contains("pub struct SharedName22222222"));
    }
}
