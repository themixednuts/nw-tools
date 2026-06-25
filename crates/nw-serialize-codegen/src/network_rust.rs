use std::collections::{BTreeMap, BTreeSet};

use quote::{format_ident, quote};
use serde::{Deserialize, Serialize};
use syn::{LitInt, LitStr};
use thiserror::Error;
use uuid::Uuid;

use crate::ir::{SerializeCodegenItem, SerializeCodegenItemKind};
use crate::naming::{rust_field_ident, rust_module_ident, rust_type_ident};
use crate::network_schema::{
    NetworkConfidence, NetworkField, NetworkRootKind, NetworkSchema, NetworkType,
    NetworkWireShape as SchemaWireShape,
};
use crate::types::{ResolvedType, ScalarType};

pub const NETWORK_RUST_EMITTER_VERSION: &str = "network-rust-v23";

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
    pub message_generation_plan_count: usize,
    pub generatable_message_count: usize,
    pub blocked_message_count: usize,
    pub message_generation_plans: Vec<NetworkMessageGenerationPlanReport>,
    #[serde(default)]
    pub message_blocker_summary: NetworkBlockerSummaryReport,
    #[serde(default)]
    pub marshaler_conversion_count: usize,
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

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NetworkMessageGenerationPlanReport {
    pub type_index: Option<u32>,
    pub type_name: Option<String>,
    pub field_count: usize,
    pub shaped_field_count: usize,
    pub supported_field_count: usize,
    pub missing_wire_shape_count: usize,
    #[serde(default)]
    pub missing_field_type_count: usize,
    pub unsupported_wire_shape_count: usize,
    pub low_confidence_field_count: usize,
    pub can_generate: bool,
    pub blocked_reasons: Vec<String>,
    pub fields: Vec<NetworkStateFieldShapeReport>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NetworkBlockerSummaryReport {
    pub total_plan_count: usize,
    pub generatable_count: usize,
    pub blocked_count: usize,
    pub reason_buckets: Vec<NetworkBlockerReasonBucketReport>,
    pub combination_buckets: Vec<NetworkBlockerCombinationBucketReport>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NetworkBlockerReasonBucketReport {
    pub reason: String,
    pub type_count: usize,
    pub blocked_field_count: usize,
    pub examples: Vec<NetworkBlockedTypeExampleReport>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NetworkBlockerCombinationBucketReport {
    pub reasons: Vec<String>,
    pub type_count: usize,
    pub examples: Vec<NetworkBlockedTypeExampleReport>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NetworkBlockedTypeExampleReport {
    pub type_index: Option<u32>,
    pub type_name: Option<String>,
    pub field_count: usize,
    pub blocked_reasons: Vec<String>,
    pub blocked_fields: Vec<NetworkBlockedFieldExampleReport>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NetworkBlockedFieldExampleReport {
    pub field_index: Option<u32>,
    pub field_name: Option<String>,
    pub rust_value_type: Option<String>,
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
        report.message_generation_plans = message_generation_plans(schema, &wire_shapes);
        report.message_generation_plan_count = report.message_generation_plans.len();
        report.generatable_message_count = report
            .message_generation_plans
            .iter()
            .filter(|plan| plan.can_generate)
            .count();
        report.blocked_message_count =
            report.message_generation_plan_count - report.generatable_message_count;
        report.message_blocker_summary = message_blocker_summary(&report.message_generation_plans);
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
                F64,
                HalfF32,
                VlqU32,
                Vec2,
                Vec3,
                Vec4,
                Quat,
                QuatCompNorm,
                Mat3,
                Affine3,
                Aabb2d,
                Aabb3d,
                EntityRef,
                FixedBytes(u16),
                String,
            }

            #[derive(Debug, Clone, Copy, PartialEq, Eq)]
            pub struct NetworkFieldDescriptor {
                pub index: u32,
                pub name: &'static str,
                pub group: Option<u32>,
                pub native_type: Option<&'static str>,
                pub rust_type: Option<&'static str>,
                pub storage_offset: Option<u32>,
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
                pub instance_size: Option<u32>,
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
            #(#modules)*
        };
        let file = syn::parse2(tokens)?;
        Ok(NetworkRustOutput {
            source: prettyplease::unparse(&file),
            report,
        })
    }

    pub fn emit_messages(
        schema: &NetworkSchema,
    ) -> Result<NetworkRustOutput, NetworkRustEmitError> {
        let wire_shapes = wire_shapes_by_handler_vtable(schema);
        let wire_shape_sources = wire_shape_sources_by_handler_vtable(schema);
        let rust_names = identity_names_by_type_index(schema);
        let mut report = NetworkRustGenerationReport::default();
        let modules = schema
            .types
            .iter()
            .filter(|network_type| network_type.root_kinds.contains(&NetworkRootKind::Message))
            .filter_map(|network_type| {
                let plan = message_generation_plan(network_type, &wire_shapes, &wire_shape_sources);
                report.message_generation_plans.push(plan.clone());
                plan.can_generate
                    .then(|| message_module_tokens(network_type, &plan, &rust_names))
            })
            .collect::<Vec<_>>();

        report.message_generation_plan_count = report.message_generation_plans.len();
        report.generatable_message_count = report
            .message_generation_plans
            .iter()
            .filter(|plan| plan.can_generate)
            .count();
        report.blocked_message_count =
            report.message_generation_plan_count - report.generatable_message_count;
        report.message_count = report.generatable_message_count;
        report.message_blocker_summary = message_blocker_summary(&report.message_generation_plans);

        let tokens = quote! {
            #(#modules)*
        };
        let file = syn::parse2(tokens)?;
        Ok(NetworkRustOutput {
            source: prettyplease::unparse(&file),
            report,
        })
    }

    pub fn emit_marshaler_conversions<'a>(
        items: impl IntoIterator<Item = &'a SerializeCodegenItem>,
    ) -> Result<NetworkRustOutput, NetworkRustEmitError> {
        let mut report = NetworkRustGenerationReport::default();
        let conversions = items
            .into_iter()
            .flat_map(enum_marshaler_conversion_tokens)
            .collect::<Vec<_>>();
        report.marshaler_conversion_count = conversions.len();

        let tokens = quote! {
            #(#conversions)*
        };
        let file = syn::parse2(tokens)?;
        Ok(NetworkRustOutput {
            source: prettyplease::unparse(&file),
            report,
        })
    }
}

fn enum_marshaler_conversion_tokens(item: &SerializeCodegenItem) -> Vec<proc_macro2::TokenStream> {
    if item.kind != SerializeCodegenItemKind::Enum {
        return Vec::new();
    }
    let Some(underlying) = enum_underlying_scalar(item) else {
        return Vec::new();
    };
    let Some((min, max)) = enum_value_range(item) else {
        return Vec::new();
    };
    if min < 0 {
        return Vec::new();
    }

    let enum_ident = format_ident!("{}", rust_type_ident(&item.source_name));
    [
        UnsignedConversion::U8,
        UnsignedConversion::U16,
        UnsignedConversion::U32,
    ]
    .into_iter()
    .filter(|conversion| max <= i128::from(conversion.max_value()))
    .map(|conversion| {
        enum_marshaler_conversion_token(&enum_ident, underlying, conversion, min, max)
    })
    .collect()
}

fn enum_underlying_scalar(item: &SerializeCodegenItem) -> Option<ScalarType> {
    match item.enum_underlying_type.as_ref()? {
        ResolvedType::Scalar(scalar) if is_integer_scalar(*scalar) => Some(*scalar),
        _ => None,
    }
}

const fn is_integer_scalar(scalar: ScalarType) -> bool {
    matches!(
        scalar,
        ScalarType::Char
            | ScalarType::SignedChar
            | ScalarType::I8
            | ScalarType::U8
            | ScalarType::I16
            | ScalarType::U16
            | ScalarType::I32
            | ScalarType::U32
            | ScalarType::I64
            | ScalarType::U64
            | ScalarType::UnsignedLong
    )
}

fn enum_value_range(item: &SerializeCodegenItem) -> Option<(i128, i128)> {
    let mut values = item
        .variants
        .iter()
        .map(|variant| {
            variant
                .value_i32
                .map(i128::from)
                .or_else(|| variant.value_u32.map(i128::from))
                .or_else(|| variant.value_u64.map(i128::from))
        })
        .collect::<Option<Vec<_>>>()?;
    values.sort_unstable();
    Some((*values.first()?, *values.last()?))
}

#[derive(Debug, Clone, Copy)]
enum UnsignedConversion {
    U8,
    U16,
    U32,
}

impl UnsignedConversion {
    const fn bit_width(self) -> u8 {
        match self {
            Self::U8 => 8,
            Self::U16 => 16,
            Self::U32 => 32,
        }
    }

    const fn max_value(self) -> u32 {
        match self {
            Self::U8 => u8::MAX as u32,
            Self::U16 => u16::MAX as u32,
            Self::U32 => u32::MAX,
        }
    }

    fn rust_type(self) -> proc_macro2::TokenStream {
        match self {
            Self::U8 => quote!(u8),
            Self::U16 => quote!(u16),
            Self::U32 => quote!(u32),
        }
    }
}

fn enum_marshaler_conversion_token(
    enum_ident: &proc_macro2::Ident,
    underlying: ScalarType,
    conversion: UnsignedConversion,
    min: i128,
    max: i128,
) -> proc_macro2::TokenStream {
    let serialized_ty = conversion.rust_type();
    let underlying_ty = enum_underlying_rust_type(underlying);
    let serialize_value = enum_serialize_value_tokens(underlying, conversion);
    let deserialize_value = enum_deserialize_value_tokens(underlying, conversion, min, max);
    let min_i128 = syn::LitInt::new(&min.to_string(), proc_macro2::Span::call_site());
    let max_i128 = syn::LitInt::new(&max.to_string(), proc_macro2::Span::call_site());
    let min_u64 = u64::try_from(min).expect("unsigned enum conversion has nonnegative min");
    let max_u64 = u64::try_from(max).expect("unsigned enum conversion has nonnegative max");

    quote! {
        impl ::nw_network::serialize::MarshalerConversion<#serialized_ty>
            for ::nw_network::source::#enum_ident
        {
            fn to_serialized(self) -> #serialized_ty {
                let raw = #underlying_ty::from(self);
                let raw_i128 = i128::from(raw);
                debug_assert!((#min_i128..=#max_i128).contains(&raw_i128));
                #serialize_value
            }

            fn try_from_serialized(
                value: #serialized_ty,
            ) -> Result<Self, ::nw_network::serialize::MarshalerError> {
                let raw = #deserialize_value;
                Self::try_from(raw).map_err(|_| {
                    ::nw_network::serialize::MarshalerError::InvalidRange {
                        value: u64::from(value),
                        min: #min_u64,
                        max: #max_u64,
                    }
                })
            }
        }
    }
}

fn enum_serialize_value_tokens(
    underlying: ScalarType,
    conversion: UnsignedConversion,
) -> proc_macro2::TokenStream {
    let serialized_ty = conversion.rust_type();
    if underlying == conversion.scalar_type() {
        return quote!(raw);
    }
    if unsigned_scalar_bit_width(underlying).is_some_and(|bits| bits <= conversion.bit_width()) {
        return quote!(#serialized_ty::from(raw));
    }
    quote! {
        #serialized_ty::try_from(raw)
            .expect("generated enum discriminant fits serialized representation")
    }
}

fn enum_deserialize_value_tokens(
    underlying: ScalarType,
    conversion: UnsignedConversion,
    min: i128,
    max: i128,
) -> proc_macro2::TokenStream {
    let underlying_ty = enum_underlying_rust_type(underlying);
    let min_u64 = u64::try_from(min).expect("unsigned enum conversion has nonnegative min");
    let max_u64 = u64::try_from(max).expect("unsigned enum conversion has nonnegative max");
    if underlying == conversion.scalar_type() {
        return quote!(value);
    }
    if scalar_accepts_all_unsigned_values(underlying, conversion) {
        return quote!(#underlying_ty::from(value));
    }
    quote! {
        #underlying_ty::try_from(value).map_err(|_| {
            ::nw_network::serialize::MarshalerError::InvalidRange {
                value: u64::from(value),
                min: #min_u64,
                max: #max_u64,
            }
        })?
    }
}

impl UnsignedConversion {
    const fn scalar_type(self) -> ScalarType {
        match self {
            Self::U8 => ScalarType::U8,
            Self::U16 => ScalarType::U16,
            Self::U32 => ScalarType::U32,
        }
    }
}

const fn unsigned_scalar_bit_width(scalar: ScalarType) -> Option<u8> {
    match scalar {
        ScalarType::U8 => Some(8),
        ScalarType::U16 => Some(16),
        ScalarType::U32 => Some(32),
        ScalarType::U64 | ScalarType::UnsignedLong => Some(64),
        _ => None,
    }
}

const fn scalar_accepts_all_unsigned_values(
    scalar: ScalarType,
    conversion: UnsignedConversion,
) -> bool {
    match scalar {
        ScalarType::U8 => conversion.bit_width() <= 8,
        ScalarType::U16 => conversion.bit_width() <= 16,
        ScalarType::U32 => conversion.bit_width() <= 32,
        ScalarType::U64 | ScalarType::UnsignedLong => true,
        ScalarType::Char | ScalarType::SignedChar | ScalarType::I8 => {
            conversion.max_value() <= i8::MAX as u32
        }
        ScalarType::I16 => conversion.max_value() <= i16::MAX as u32,
        ScalarType::I32 => conversion.max_value() <= i32::MAX as u32,
        ScalarType::I64 => true,
        _ => false,
    }
}

fn enum_underlying_rust_type(scalar: ScalarType) -> proc_macro2::TokenStream {
    match scalar {
        ScalarType::Char | ScalarType::SignedChar | ScalarType::I8 => quote!(i8),
        ScalarType::U8 => quote!(u8),
        ScalarType::I16 => quote!(i16),
        ScalarType::U16 => quote!(u16),
        ScalarType::I32 => quote!(i32),
        ScalarType::U32 => quote!(u32),
        ScalarType::I64 => quote!(i64),
        ScalarType::U64 | ScalarType::UnsignedLong => quote!(u64),
        _ => unreachable!("non-integer enum underlyings are skipped before emission"),
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

fn namespaced_identity_candidate(network_type: &NetworkType) -> Option<String> {
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
    let instance_size = option_u32_tokens(
        network_type
            .instance
            .as_ref()
            .and_then(|instance| instance.size),
    );
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
            instance_size: #instance_size,
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
    let native_type = option_str_tokens(field.native_type.as_deref());
    let rust_type = option_str_tokens(field.rust_type.as_deref());
    let storage_offset = option_u32_tokens(field.storage_offset);
    let wire_shape = field_wire_shape_tokens(field, wire_shapes, report);
    let confidence = confidence_ident(field.confidence);
    Some(quote! {
        NetworkFieldDescriptor {
            index: #index,
            name: #name,
            group: #group,
            native_type: #native_type,
            rust_type: #rust_type,
            storage_offset: #storage_offset,
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

fn message_generation_plans(
    schema: &NetworkSchema,
    wire_shapes: &BTreeMap<&str, SchemaWireShape>,
) -> Vec<NetworkMessageGenerationPlanReport> {
    let wire_shape_sources = wire_shape_sources_by_handler_vtable(schema);
    schema
        .types
        .iter()
        .filter(|network_type| network_type.root_kinds.contains(&NetworkRootKind::Message))
        .map(|network_type| message_generation_plan(network_type, wire_shapes, &wire_shape_sources))
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

fn message_generation_plan(
    network_type: &NetworkType,
    wire_shapes: &BTreeMap<&str, SchemaWireShape>,
    wire_shape_sources: &BTreeMap<&str, &str>,
) -> NetworkMessageGenerationPlanReport {
    let fields = network_type
        .fields
        .iter()
        .map(|field| message_field_shape_report(field, wire_shapes, wire_shape_sources))
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
    let missing_field_type_count = fields
        .iter()
        .filter(|field| field.blocked_reason.as_deref() == Some("missing-field-type"))
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
    let placeholder_field_name_count = fields
        .iter()
        .filter(|field| field.blocked_reason.as_deref() == Some("placeholder-field-name"))
        .count();
    let blocked_reasons = message_blocked_reasons(
        network_type,
        field_count,
        missing_field_type_count,
        unsupported_wire_shape_count,
        invalid_field_metadata_count,
        low_confidence_field_count,
        placeholder_field_name_count,
    );

    NetworkMessageGenerationPlanReport {
        type_index: network_type.type_index,
        type_name: network_type.name.clone(),
        field_count,
        shaped_field_count,
        supported_field_count,
        missing_wire_shape_count,
        missing_field_type_count,
        unsupported_wire_shape_count,
        low_confidence_field_count,
        can_generate: blocked_reasons.is_empty(),
        blocked_reasons,
        fields,
    }
}

const BLOCKER_EXAMPLE_LIMIT: usize = 8;
const BLOCKED_FIELD_EXAMPLE_LIMIT: usize = 8;

fn message_blocker_summary(
    plans: &[NetworkMessageGenerationPlanReport],
) -> NetworkBlockerSummaryReport {
    let mut reason_buckets = BTreeMap::<String, NetworkBlockerReasonBucketReport>::new();
    let mut combination_buckets =
        BTreeMap::<Vec<String>, NetworkBlockerCombinationBucketReport>::new();

    for plan in plans.iter().filter(|plan| !plan.can_generate) {
        let example = blocked_type_example(plan);
        let reason_families = plan
            .blocked_reasons
            .iter()
            .map(|reason| blocker_reason_family(reason).to_owned())
            .collect::<BTreeSet<_>>();
        for reason in reason_families {
            let bucket = reason_buckets.entry(reason.clone()).or_insert_with(|| {
                NetworkBlockerReasonBucketReport {
                    reason,
                    ..NetworkBlockerReasonBucketReport::default()
                }
            });
            bucket.type_count += 1;
            bucket.blocked_field_count += blocked_field_count_for_reason(plan, &bucket.reason);
            if bucket.examples.len() < BLOCKER_EXAMPLE_LIMIT {
                bucket.examples.push(example.clone());
            }
        }

        let mut reasons = plan.blocked_reasons.clone();
        reasons.sort();
        let bucket = combination_buckets
            .entry(reasons.clone())
            .or_insert_with(|| NetworkBlockerCombinationBucketReport {
                reasons,
                ..NetworkBlockerCombinationBucketReport::default()
            });
        bucket.type_count += 1;
        if bucket.examples.len() < BLOCKER_EXAMPLE_LIMIT {
            bucket.examples.push(example);
        }
    }

    let mut reason_buckets = reason_buckets.into_values().collect::<Vec<_>>();
    reason_buckets.sort_by(|left, right| {
        right
            .type_count
            .cmp(&left.type_count)
            .then_with(|| left.reason.cmp(&right.reason))
    });

    let mut combination_buckets = combination_buckets.into_values().collect::<Vec<_>>();
    combination_buckets.sort_by(|left, right| {
        right
            .type_count
            .cmp(&left.type_count)
            .then_with(|| left.reasons.cmp(&right.reasons))
    });

    NetworkBlockerSummaryReport {
        total_plan_count: plans.len(),
        generatable_count: plans.iter().filter(|plan| plan.can_generate).count(),
        blocked_count: plans.iter().filter(|plan| !plan.can_generate).count(),
        reason_buckets,
        combination_buckets,
    }
}

fn blocker_reason_family(reason: &str) -> &str {
    reason.split_once(':').map_or(reason, |(family, _)| family)
}

fn blocked_field_count_for_reason(
    plan: &NetworkMessageGenerationPlanReport,
    reason: &str,
) -> usize {
    plan.fields
        .iter()
        .filter(|field| {
            field
                .blocked_reason
                .as_deref()
                .is_some_and(|field_reason| blocker_reason_family(field_reason) == reason)
        })
        .count()
}

fn blocked_type_example(
    plan: &NetworkMessageGenerationPlanReport,
) -> NetworkBlockedTypeExampleReport {
    NetworkBlockedTypeExampleReport {
        type_index: plan.type_index,
        type_name: plan.type_name.clone(),
        field_count: plan.field_count,
        blocked_reasons: plan.blocked_reasons.clone(),
        blocked_fields: plan
            .fields
            .iter()
            .filter(|field| field.blocked_reason.is_some())
            .take(BLOCKED_FIELD_EXAMPLE_LIMIT)
            .map(blocked_field_example)
            .collect(),
    }
}

fn blocked_field_example(field: &NetworkStateFieldShapeReport) -> NetworkBlockedFieldExampleReport {
    NetworkBlockedFieldExampleReport {
        field_index: field.field_index,
        field_name: field.field_name.clone(),
        rust_value_type: field.rust_value_type.clone(),
        blocked_reason: field.blocked_reason.clone(),
    }
}

fn message_field_shape_report(
    field: &NetworkField,
    wire_shapes: &BTreeMap<&str, SchemaWireShape>,
    wire_shape_sources: &BTreeMap<&str, &str>,
) -> NetworkStateFieldShapeReport {
    let mut report = state_field_shape_report(field, wire_shapes, wire_shape_sources);
    let rust_type = field
        .rust_type
        .clone()
        .or_else(|| {
            field
                .native_type
                .as_deref()
                .and_then(message_native_type_rust_type)
                .map(ToOwned::to_owned)
        })
        .or_else(|| report.rust_value_type.clone());
    report.rust_value_type = rust_type.clone();
    report.rust_field_type = rust_type.clone();
    report.blocked_reason =
        message_field_blocked_reason(field, report.wire_shape, rust_type.as_deref());
    report.supported = report.blocked_reason.is_none();
    if is_placeholder_message_field_name(field) {
        report.supported = false;
        report.blocked_reason = Some("placeholder-field-name".to_owned());
    }
    report
}

fn state_field_shape_report(
    field: &NetworkField,
    wire_shapes: &BTreeMap<&str, SchemaWireShape>,
    wire_shape_sources: &BTreeMap<&str, &str>,
) -> NetworkStateFieldShapeReport {
    let shape = field_wire_shape(field, wire_shapes);
    let rust_type = field
        .rust_type
        .as_deref()
        .filter(|rust_type| syn::parse_str::<syn::Type>(rust_type).is_ok());
    let rust_shape = shape.map(rust_field_shape);
    let blocked_reason = field_blocked_reason(field, shape, field.rust_type.as_deref());
    NetworkStateFieldShapeReport {
        field_index: field.index,
        field_name: field.name.clone(),
        group: field.group,
        handler_vtable: field.handler_vtable.clone(),
        wire_shape: shape,
        wire_shape_source: field_wire_shape_source(field, wire_shapes, wire_shape_sources),
        rust_value_type: rust_type
            .map(ToOwned::to_owned)
            .or_else(|| rust_shape.as_ref().map(|shape| shape.value_type.clone())),
        rust_field_type: rust_shape.map(|shape| shape.field_type),
        confidence: field.confidence,
        supported: blocked_reason.is_none(),
        blocked_reason,
    }
}

fn field_wire_shape(
    field: &NetworkField,
    wire_shapes: &BTreeMap<&str, SchemaWireShape>,
) -> Option<SchemaWireShape> {
    field
        .wire_shape
        .or_else(|| {
            field
                .handler_vtable
                .as_deref()
                .and_then(|handler_vtable| wire_shapes.get(handler_vtable).copied())
        })
        .or_else(|| {
            field
                .native_type
                .as_deref()
                .and_then(native_type_wire_shape)
        })
}

fn field_wire_shape_source(
    field: &NetworkField,
    wire_shapes: &BTreeMap<&str, SchemaWireShape>,
    wire_shape_sources: &BTreeMap<&str, &str>,
) -> Option<String> {
    field.wire_shape_source.clone().or_else(|| {
        let handler_source = field
            .handler_vtable
            .as_deref()
            .filter(|handler_vtable| wire_shapes.contains_key(*handler_vtable))
            .and_then(|handler_vtable| wire_shape_sources.get(handler_vtable).copied())
            .map(ToOwned::to_owned);
        handler_source.or_else(|| {
            field
                .native_type
                .as_deref()
                .and_then(native_type_wire_shape)
                .map(|_| "native-type".to_owned())
        })
    })
}

fn native_type_wire_shape(native_type: &str) -> Option<SchemaWireShape> {
    match native_type.trim() {
        "bool" => Some(SchemaWireShape::Bool),
        "u8" | "uint8_t" | "AZ::u8" => Some(SchemaWireShape::U8),
        "u16" | "uint16_t" | "AZ::u16" => Some(SchemaWireShape::U16),
        "u32" | "uint32_t" | "AZ::u32" | "FragmentKey" | "Amazon::Hub::FragmentKey" => {
            Some(SchemaWireShape::U32)
        }
        "u64" | "uint64_t" | "AZ::u64" => Some(SchemaWireShape::U64),
        "f32" | "float" => Some(SchemaWireShape::F32),
        "f64" | "double" => Some(SchemaWireShape::F64),
        "AZ::Vector2" => Some(SchemaWireShape::Vec2),
        "AZ::Vector3" => Some(SchemaWireShape::Vec3),
        "AZ::Vector4" => Some(SchemaWireShape::Vec4),
        "AZ::Quaternion" => Some(SchemaWireShape::Quat),
        "AZ::Matrix3x3" => Some(SchemaWireShape::Mat3),
        "AZ::Transform" => Some(SchemaWireShape::Affine3),
        "AZ::Bounds" => Some(SchemaWireShape::Aabb2d),
        "AZ::Aabb" => Some(SchemaWireShape::Aabb3d),
        "EntityRef" => Some(SchemaWireShape::EntityRef),
        "AZStd::string" | "std::string" | "string" => Some(SchemaWireShape::String),
        _ => None,
    }
}

fn message_native_type_rust_type(native_type: &str) -> Option<&'static str> {
    match native_type.trim() {
        "ActorRef" | "Amazon::Hub::ActorRef" | "HubAddress" | "ProxyAddress" => {
            Some("::nw_network::HubAddress")
        }
        "BaselineableFragment" | "Amazon::Hub::BaselineableFragment" => {
            Some("::nw_network::hub::BaselineableFragment")
        }
        "FragmentKey" | "Amazon::Hub::FragmentKey" => Some("::nw_network::hub::FragmentKey"),
        _ => None,
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

fn message_blocked_reasons(
    network_type: &NetworkType,
    field_count: usize,
    missing_field_type_count: usize,
    unsupported_wire_shape_count: usize,
    invalid_field_metadata_count: usize,
    low_confidence_field_count: usize,
    placeholder_field_name_count: usize,
) -> Vec<String> {
    let mut reasons = Vec::new();
    if network_type.type_id.is_none() {
        reasons.push("missing-type-id".to_owned());
    }
    if network_type.type_index.is_none() {
        reasons.push("missing-type-index".to_owned());
    }
    if network_type.name.is_none() {
        reasons.push("missing-type-name".to_owned());
    }
    if field_count == 0 {
        reasons.push("no-message-fields".to_owned());
    }
    if missing_field_type_count != 0 {
        reasons.push(format!("missing-field-type:{missing_field_type_count}"));
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
    if placeholder_field_name_count != 0 {
        reasons.push(format!(
            "placeholder-field-name:{placeholder_field_name_count}"
        ));
    }
    reasons
}

fn field_blocked_reason(
    field: &NetworkField,
    shape: Option<SchemaWireShape>,
    rust_type: Option<&str>,
) -> Option<String> {
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
    if let Some(rust_type) = rust_type
        && syn::parse_str::<syn::Type>(rust_type).is_err()
    {
        return Some("invalid-rust-field-type".to_owned());
    }
    None
}

fn message_field_blocked_reason(
    field: &NetworkField,
    shape: Option<SchemaWireShape>,
    rust_type: Option<&str>,
) -> Option<String> {
    if field.index.is_none() {
        return Some("missing-field-index".to_owned());
    }
    if field.name.is_none() {
        return Some("missing-field-name".to_owned());
    }
    if !field.confidence.is_high_or_exact() {
        return Some("low-confidence-field".to_owned());
    }
    if let Some(rust_type) = rust_type
        && syn::parse_str::<syn::Type>(rust_type).is_ok()
    {
        return None;
    }
    if rust_type.is_some() {
        return Some("invalid-rust-field-type".to_owned());
    }
    if shape.is_none() {
        return Some("missing-field-type".to_owned());
    }
    None
}

fn is_placeholder_field_name(value: &str) -> bool {
    value
        .strip_prefix("field_")
        .is_some_and(|suffix| !suffix.is_empty() && suffix.chars().all(|c| c.is_ascii_digit()))
}

fn is_placeholder_message_field_name(field: &NetworkField) -> bool {
    let Some(name) = field.name.as_deref() else {
        return false;
    };
    is_placeholder_field_name(name) || is_native_type_field_name(name)
}

fn is_native_type_field_name(name: &str) -> bool {
    matches!(
        name.trim(),
        "bool"
            | "u8"
            | "u16"
            | "u32"
            | "u64"
            | "f32"
            | "f64"
            | "float"
            | "double"
            | "String"
            | "Vector2"
            | "Vector3"
            | "Vector4"
            | "Quaternion"
            | "Matrix3x3"
            | "Aabb"
            | "EntityRef"
            | "ActorRef"
            | "HubAddress"
            | "ProxyAddress"
            | "FragmentKey"
            | "BaselineableFragment"
            | "Amazon::Hub::ActorRef"
            | "Amazon::Hub::FragmentKey"
            | "Amazon::Hub::BaselineableFragment"
            | "composite"
    )
}

#[derive(Debug, Clone)]
struct RustFieldShape {
    value_type: String,
    field_type: String,
}

fn rust_field_shape(shape: SchemaWireShape) -> RustFieldShape {
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
        SchemaWireShape::EntityRef => rust_field_shape_static(
            "::nw_network::EntityRef",
            "ReplicatedFieldHandler<::nw_network::EntityRef>",
        ),
        SchemaWireShape::FixedBytes(len) => RustFieldShape {
            value_type: format!("[u8; {len}]"),
            field_type: format!("ReplicatedFieldHandler<[u8; {len}]>"),
        },
        SchemaWireShape::String => {
            rust_field_shape_static("String", "ReplicatedFieldHandler<String>")
        }
    }
}

fn rust_field_shape_static(value_type: &'static str, field_type: &'static str) -> RustFieldShape {
    RustFieldShape {
        value_type: value_type.to_owned(),
        field_type: field_type.to_owned(),
    }
}

fn unsuffixed_int_lit(value: u16) -> LitInt {
    LitInt::new(&value.to_string(), proc_macro2::Span::call_site())
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
    let field_type = replicated_state_field_type_tokens(field);

    quote! {
        #group_attr
        pub #field_ident: #field_type,
    }
}

fn replicated_state_field_type_tokens(
    field: &NetworkStateFieldShapeReport,
) -> proc_macro2::TokenStream {
    let shape = field
        .wire_shape
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
        SchemaWireShape::EntityRef => {
            quote!(::nw_network::serialize::ReplicatedFieldHandler<::nw_network::EntityRef>)
        }
        SchemaWireShape::FixedBytes(len) => {
            let len = unsuffixed_int_lit(len);
            quote!(::nw_network::serialize::ReplicatedFieldHandler<[u8; #len]>)
        }
        SchemaWireShape::String => {
            quote!(::nw_network::serialize::ReplicatedFieldHandler<String>)
        }
    }
}

fn message_module_tokens(
    network_type: &NetworkType,
    plan: &NetworkMessageGenerationPlanReport,
    rust_names: &BTreeMap<u32, String>,
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

    quote! {
        pub mod #module_ident {
            use ::nw_network::{AzRtti, Marshaler, TypeRegistry};

            #[derive(Debug, Clone, Default, PartialEq, Marshaler, AzRtti, TypeRegistry)]
            #[az_rtti(#type_id)]
            #[type_registry(#type_index)]
            pub struct #message_ident {
                #(#fields)*
            }
        }

        pub use #module_ident::#message_ident;
    }
}

fn message_field_tokens(field: &NetworkStateFieldShapeReport) -> proc_macro2::TokenStream {
    let field_name = field
        .field_name
        .as_deref()
        .expect("generatable message field has a name");
    let field_ident = format_ident!("{}", rust_field_ident(field_name));
    let field_type = field
        .rust_field_type
        .as_deref()
        .and_then(|rust_type| syn::parse_str::<syn::Type>(rust_type).ok())
        .map(|rust_type| quote!(#rust_type))
        .unwrap_or_else(|| {
            message_field_type_tokens(
                field
                    .wire_shape
                    .expect("generatable message field has a field type"),
            )
        });
    let marshal_attr = message_field_marshal_attr_tokens(field);

    quote! {
        #marshal_attr
        pub #field_ident: #field_type,
    }
}

fn message_field_type_tokens(shape: SchemaWireShape) -> proc_macro2::TokenStream {
    match shape {
        SchemaWireShape::Bool => quote!(bool),
        SchemaWireShape::U8 => quote!(u8),
        SchemaWireShape::U16 => quote!(u16),
        SchemaWireShape::U32 | SchemaWireShape::VlqU32 => quote!(u32),
        SchemaWireShape::U64 => quote!(u64),
        SchemaWireShape::F64 => quote!(f64),
        SchemaWireShape::F32 | SchemaWireShape::HalfF32 => quote!(f32),
        SchemaWireShape::Vec2 => quote!(::glam::Vec2),
        SchemaWireShape::Vec3 => quote!(::glam::Vec3),
        SchemaWireShape::Vec4 => quote!(::glam::Vec4),
        SchemaWireShape::Quat => quote!(::glam::Quat),
        SchemaWireShape::QuatCompNorm => quote!(::nw_network::serialize::QuatCompNorm),
        SchemaWireShape::Mat3 => quote!(::glam::Mat3),
        SchemaWireShape::Affine3 => quote!(::glam::Affine3A),
        SchemaWireShape::Aabb2d => quote!(::bevy_math::bounding::Aabb2d),
        SchemaWireShape::Aabb3d => quote!(::bevy_math::bounding::Aabb3d),
        SchemaWireShape::EntityRef => quote!(::nw_network::EntityRef),
        SchemaWireShape::FixedBytes(len) => {
            let len = unsuffixed_int_lit(len);
            quote!([u8; #len])
        }
        SchemaWireShape::String => quote!(String),
    }
}

fn message_field_marshal_attr_tokens(
    field: &NetworkStateFieldShapeReport,
) -> proc_macro2::TokenStream {
    if let Some(conversion) = field_conversion_marshal_type_string(field) {
        let conversion = LitStr::new(&conversion, proc_macro2::Span::call_site());
        return quote!(#[marshal(codec = #conversion)]);
    }

    match field.wire_shape {
        Some(shape) => message_wire_shape_marshal_attr_tokens(shape),
        None => quote! {},
    }
}

fn message_wire_shape_marshal_attr_tokens(shape: SchemaWireShape) -> proc_macro2::TokenStream {
    match shape {
        SchemaWireShape::HalfF32 => {
            quote!(#[marshal(as = "::nw_network::serialize::HalfF32")])
        }
        SchemaWireShape::VlqU32 => {
            quote!(#[marshal(as = "::nw_network::serialize::VlqU32")])
        }
        _ => quote! {},
    }
}

fn field_conversion_marshal_type_tokens(
    field: &NetworkStateFieldShapeReport,
) -> Option<proc_macro2::TokenStream> {
    let ty = field_conversion_marshal_type_string(field)?;
    syn::parse_str::<syn::Type>(&ty).ok().map(|ty| quote!(#ty))
}

fn field_conversion_marshal_type_string(field: &NetworkStateFieldShapeReport) -> Option<String> {
    let shape = field.wire_shape?;
    let serialized_type = scalar_conversion_serialized_type(shape)?;
    let rust_type = field.rust_value_type.as_deref()?.trim();
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

fn is_generated_source_type(rust_type: &str) -> bool {
    let rust_type = rust_type.trim_start_matches("::");
    rust_type.starts_with("nw_network::source::")
}

const fn scalar_conversion_serialized_type(shape: SchemaWireShape) -> Option<&'static str> {
    match shape {
        SchemaWireShape::U8 => Some("u8"),
        SchemaWireShape::U16 => Some("u16"),
        SchemaWireShape::U32 => Some("u32"),
        _ => None,
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

fn wire_shape_tokens(shape: SchemaWireShape) -> proc_macro2::TokenStream {
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
        SchemaWireShape::Vec2 => quote!(NetworkWireShape::Vec2),
        SchemaWireShape::Vec3 => quote!(NetworkWireShape::Vec3),
        SchemaWireShape::Vec4 => quote!(NetworkWireShape::Vec4),
        SchemaWireShape::Quat => quote!(NetworkWireShape::Quat),
        SchemaWireShape::QuatCompNorm => quote!(NetworkWireShape::QuatCompNorm),
        SchemaWireShape::Mat3 => quote!(NetworkWireShape::Mat3),
        SchemaWireShape::Affine3 => quote!(NetworkWireShape::Affine3),
        SchemaWireShape::Aabb2d => quote!(NetworkWireShape::Aabb2d),
        SchemaWireShape::Aabb3d => quote!(NetworkWireShape::Aabb3d),
        SchemaWireShape::EntityRef => quote!(NetworkWireShape::EntityRef),
        SchemaWireShape::FixedBytes(len) => quote!(NetworkWireShape::FixedBytes(#len)),
        SchemaWireShape::String => quote!(NetworkWireShape::String),
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
    use uuid::uuid;

    use crate::{
        ir::SerializeCodegenVariant,
        network_schema::{NetworkMessageFieldSignature, NetworkMessageSignature, NetworkSchema},
    };

    use super::*;

    fn fragment_message_signatures() -> Vec<NetworkMessageSignature> {
        vec![
            NetworkMessageSignature {
                type_id: Some(uuid!("96a58e69-7bd5-45c5-86e4-daf9f5eb1e86")),
                type_index: Some(397),
                name: Some("Replicate::RegisterFragmentAccessMsg".to_owned()),
                rust_name: Some("RegisterFragmentAccessMsg".to_owned()),
                source: None,
                fields: fragment_access_fields(),
            },
            NetworkMessageSignature {
                type_id: Some(uuid!("2b7640e0-4204-4e52-998a-c2db02e0a480")),
                type_index: Some(399),
                name: Some("Replicate::UnregisterFragmentAccessMsg".to_owned()),
                rust_name: Some("UnregisterFragmentAccessMsg".to_owned()),
                source: None,
                fields: fragment_access_fields(),
            },
            NetworkMessageSignature {
                type_id: Some(uuid!("951ef3ed-c9a0-4e3d-a6fd-7fe0673d28d2")),
                type_index: Some(422),
                name: Some("ReplicateClient::FragmentUpdateMsg".to_owned()),
                rust_name: Some("FragmentUpdateMsg".to_owned()),
                source: None,
                fields: vec![
                    message_signature_field(0, "TargetRef", "ActorRef"),
                    message_signature_field(1, "Key", "FragmentKey"),
                    message_signature_field(2, "Fragment", "BaselineableFragment"),
                ],
            },
        ]
    }

    fn fragment_access_fields() -> Vec<NetworkMessageFieldSignature> {
        vec![
            message_signature_field(0, "ProxyRef", "ActorRef"),
            message_signature_field(1, "Key", "FragmentKey"),
        ]
    }

    fn message_signature_field(
        index: u32,
        name: &str,
        native_type: &str,
    ) -> NetworkMessageFieldSignature {
        NetworkMessageFieldSignature {
            index: Some(index),
            name: name.to_owned(),
            rust_type: None,
            native_type: Some(native_type.to_owned()),
            wire_shape: None,
        }
    }

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
    fn emits_fixed_byte_replicated_field_handlers() {
        let schema = NetworkSchema::from_ghidra_static_network_report(&json!({
            "registryEntries": [{
                "uuid": "B8B8D08F-3AC4-47E9-8B1A-AD3704D0E001",
                "typeIndex": 702,
                "typeName": "Javelin::GameModeParticipantReplicatedState",
                "fields": [{
                    "index": 0,
                    "name": "flags",
                    "group": 0,
                    "handlerVtable": "NewWorld+0x81b6eb8",
                    "confidence": "register-field-call"
                }, {
                    "index": 1,
                    "name": "groupActivityEligibility",
                    "group": 0,
                    "handlerVtable": "NewWorld+0x80b9830",
                    "confidence": "register-field-call"
                }]
            }],
            "fieldRegistrationFunctions": [],
            "fieldHandlerVtables": [{
                "address": "NewWorld+0x81b6eb8",
                "fieldCount": 1,
                "wireShape": "fixed-bytes-6",
                "wireShapeSource": "marshal-raw-write-length",
                "slots": []
            }, {
                "address": "NewWorld+0x80b9830",
                "fieldCount": 1,
                "wireShape": "fixed-bytes-16",
                "wireShapeSource": "marshal-raw-write-length",
                "slots": []
            }]
        }))
        .expect("schema");

        let descriptor_output =
            NetworkRustEmitter::emit_descriptors(&schema).expect("descriptor source");

        assert_eq!(descriptor_output.report.field_wire_shape_count, 2);
        assert!(
            descriptor_output
                .source
                .contains("NetworkWireShape::FixedBytes(6")
        );
        assert!(
            descriptor_output
                .source
                .contains("NetworkWireShape::FixedBytes(16")
        );

        let state_output =
            NetworkRustEmitter::emit_replicated_states(&schema, [702]).expect("state source");

        assert_eq!(state_output.report.generatable_state_count, 1);
        assert!(
            state_output
                .source
                .contains("pub flags: ::nw_network::serialize::ReplicatedFieldHandler<[u8; 6]>")
        );
        assert!(
            state_output
                .source
                .contains("pub group_activity_eligibility:")
        );
        assert!(state_output.source.contains("[u8; 16]"));
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
    fn emits_message_unmarshal_fields_as_descriptors() {
        let schema = NetworkSchema::from_ghidra_static_network_report(&json!({
            "registryEntries": [{
                "uuid": "0B826B33-89F5-49E0-B8CB-FE4433427778",
                "typeIndex": 19,
                "typeName": "RegistrationRequestV3Msg",
                "messageUnmarshal": {
                    "createInstance": "NewWorld+0x7ce840",
                    "instanceSize": "0x470",
                    "instanceSizeSource": "create-instance-operator-new"
                },
                "fields": [{
                    "index": 0,
                    "name": "StatusCode",
                    "nativeType": "u32",
                    "storageOffset": "0x8",
                    "wireShape": "u32",
                    "wireShapeSource": "message-unmarshal-native-type",
                    "confidence": "message-unmarshal-call"
                }, {
                    "index": 2,
                    "name": "ServerVersion",
                    "nativeType": "AZStd::string",
                    "storageOffset": "0xa0",
                    "wireShape": "string",
                    "wireShapeSource": "message-unmarshal-native-type",
                    "confidence": "message-unmarshal-call"
                }]
            }],
            "fieldRegistrationFunctions": []
        }))
        .expect("schema");

        let output = NetworkRustEmitter::emit_descriptors(&schema).expect("rust source");

        assert_eq!(output.report.descriptor_count, 1);
        assert_eq!(output.report.message_count, 1);
        assert_eq!(output.report.field_registered_count, 0);
        assert_eq!(output.report.field_descriptor_count, 2);
        assert_eq!(output.report.field_wire_shape_count, 2);
        assert!(
            output
                .source
                .contains("pub struct RegistrationRequestV3Msg")
        );
        assert!(output.source.contains("native_type: Some(\"u32\")"));
        assert!(output.source.contains("storage_offset: Some(8u32)"));
        assert!(output.source.contains("instance_size: Some(1136u32)"));
        assert!(
            output
                .source
                .contains("native_type: Some(\"AZStd::string\")")
        );
        assert!(
            output
                .source
                .contains("wire_shape: Some(NetworkWireShape::String)")
        );

        let message_output = NetworkRustEmitter::emit_messages(&schema).expect("message source");

        assert_eq!(message_output.report.message_generation_plan_count, 1);
        assert_eq!(message_output.report.generatable_message_count, 1);
        assert_eq!(message_output.report.blocked_message_count, 0);
        assert!(
            message_output
                .source
                .contains("pub mod registration_request_v3_msg")
        );
        assert!(
            message_output
                .source
                .contains("pub struct RegistrationRequestV3Msg")
        );
        assert!(message_output.source.contains("pub status_code: u32"));
        assert!(message_output.source.contains("pub server_version: String"));
        assert!(message_output.source.contains("Marshaler"));
        assert!(message_output.source.contains("AzRtti"));
        assert!(message_output.source.contains("TypeRegistry"));
    }

    #[test]
    fn reports_message_blocker_summary_with_examples() {
        let schema = NetworkSchema::from_ghidra_static_network_report(&json!({
            "registryEntries": [{
                "uuid": "11111111-1111-1111-1111-111111111111",
                "typeIndex": 1,
                "typeName": "Example::EmptyMsg",
                "rootKinds": ["message"],
                "fields": []
            }, {
                "uuid": "22222222-2222-2222-2222-222222222222",
                "typeIndex": 2,
                "typeName": "Example::PlaceholderMsg",
                "rootKinds": ["message"],
                "fields": [{
                    "index": 0,
                    "name": "ActorRef",
                    "nativeType": "Amazon::Hub::ActorRef",
                    "confidence": "message-unmarshal-helper-wrapper"
                }]
            }, {
                "uuid": "33333333-3333-3333-3333-333333333333",
                "typeIndex": 3,
                "typeName": "Example::ReadyMsg",
                "rootKinds": ["message"],
                "fields": [{
                    "index": 0,
                    "name": "Value",
                    "nativeType": "u32",
                    "confidence": "message-unmarshal-call"
                }]
            }],
            "fieldRegistrationFunctions": []
        }))
        .expect("schema");

        let output = NetworkRustEmitter::emit_messages(&schema).expect("message source");
        let summary = &output.report.message_blocker_summary;

        assert_eq!(summary.total_plan_count, 3);
        assert_eq!(summary.generatable_count, 1);
        assert_eq!(summary.blocked_count, 2);
        assert_eq!(summary.reason_buckets.len(), 2);
        assert_eq!(summary.reason_buckets[0].reason, "no-message-fields");
        assert_eq!(summary.reason_buckets[0].type_count, 1);
        assert_eq!(
            summary.reason_buckets[0].examples[0].type_name.as_deref(),
            Some("Example::EmptyMsg")
        );
        assert_eq!(summary.reason_buckets[1].reason, "placeholder-field-name");
        assert_eq!(summary.reason_buckets[1].type_count, 1);
        assert_eq!(summary.reason_buckets[1].blocked_field_count, 1);
        assert_eq!(
            summary.reason_buckets[1].examples[0].blocked_fields[0]
                .field_name
                .as_deref(),
            Some("ActorRef")
        );
        assert_eq!(summary.combination_buckets.len(), 2);
    }

    #[test]
    fn infers_message_wire_shapes_from_native_types() {
        let schema = NetworkSchema::from_ghidra_static_network_report(&json!({
            "registryEntries": [{
                "uuid": "0B826B33-89F5-49E0-B8CB-FE4433427778",
                "typeIndex": 6179,
                "typeName": "Aoi::PhysicsTrait::ResizeAoiObserverMsg",
                "rootKinds": ["message"],
                "fields": [{
                    "index": 0,
                    "name": "Observer",
                    "nativeType": "EntityRef",
                    "confidence": "message-unmarshal-call"
                }, {
                    "index": 1,
                    "name": "Elapsed",
                    "nativeType": "f32",
                    "confidence": "message-unmarshal-call"
                }, {
                    "index": 2,
                    "name": "Extents",
                    "nativeType": "AZ::Vector2",
                    "confidence": "message-unmarshal-call"
                }, {
                    "index": 3,
                    "name": "Bounds",
                    "nativeType": "AZ::Bounds",
                    "confidence": "message-unmarshal-call"
                }]
            }],
            "fieldRegistrationFunctions": []
        }))
        .expect("schema");

        let descriptor_output =
            NetworkRustEmitter::emit_descriptors(&schema).expect("descriptor source");

        assert_eq!(descriptor_output.report.field_wire_shape_count, 4);
        assert!(
            descriptor_output
                .source
                .contains("wire_shape: Some(NetworkWireShape::EntityRef)")
        );
        assert!(
            descriptor_output
                .source
                .contains("wire_shape: Some(NetworkWireShape::F32)")
        );
        assert!(
            descriptor_output
                .source
                .contains("wire_shape: Some(NetworkWireShape::Vec2)")
        );
        assert!(
            descriptor_output
                .source
                .contains("wire_shape: Some(NetworkWireShape::Aabb2d)")
        );

        let message_output = NetworkRustEmitter::emit_messages(&schema).expect("message source");

        assert_eq!(message_output.report.message_generation_plan_count, 1);
        assert_eq!(message_output.report.generatable_message_count, 1);
        assert_eq!(message_output.report.blocked_message_count, 0);
        let plan = &message_output.report.message_generation_plans[0];
        assert_eq!(plan.missing_wire_shape_count, 0);
        assert_eq!(plan.supported_field_count, 4);
        assert_eq!(
            plan.fields[2].wire_shape_source.as_deref(),
            Some("native-type")
        );
        assert!(
            message_output
                .source
                .contains("pub observer: ::nw_network::EntityRef")
        );
        assert!(message_output.source.contains("pub elapsed: f32"));
        assert!(message_output.source.contains("pub extents: ::glam::Vec2"));
        assert!(
            message_output
                .source
                .contains("pub bounds: ::bevy_math::bounding::Aabb2d")
        );
    }

    #[test]
    fn blocks_message_struct_generation_for_native_type_field_names() {
        let schema = NetworkSchema::from_ghidra_static_network_report(&json!({
            "registryEntries": [{
                "uuid": "77D6477C-F057-4098-A644-58D36C551989",
                "typeIndex": 1444,
                "typeName": "Aoi::PhysicsTrait::ResizeAoiObservableMsg",
                "fields": [{
                    "index": 0,
                    "name": "f32",
                    "nativeType": "f32",
                    "confidence": "message-unmarshal-call"
                }]
            }, {
                "uuid": "1E93F466-CD84-4502-BA28-4632F80DD0FA",
                "typeIndex": 780,
                "typeName": "Amazon::Hub::ScaleTestTrait::SetTargetsMsg",
                "fields": [{
                    "index": 0,
                    "name": "ActorRef",
                    "nativeType": "Amazon::Hub::ActorRef",
                    "confidence": "message-unmarshal-helper-wrapper"
                }]
            }],
            "fieldRegistrationFunctions": []
        }))
        .expect("schema");

        let output = NetworkRustEmitter::emit_messages(&schema).expect("message source");

        assert_eq!(output.report.message_generation_plan_count, 2);
        assert_eq!(output.report.generatable_message_count, 0);
        assert_eq!(output.report.blocked_message_count, 2);
        for plan in &output.report.message_generation_plans {
            assert_eq!(plan.blocked_reasons, vec!["placeholder-field-name:1"]);
        }
        assert!(!output.source.contains("pub struct ResizeAoiObservableMsg"));
        assert!(!output.source.contains("pub struct SetTargetsMsg"));
    }

    #[test]
    fn blocks_message_struct_generation_for_placeholder_field_names() {
        let schema = NetworkSchema::from_ghidra_static_network_report(&json!({
            "registryEntries": [{
                "uuid": "6A379FB8-0BDD-43A1-AB3E-9843D7BE8CD3",
                "typeIndex": 349,
                "typeName": "REPClient::PingMsg",
                "fields": [{
                    "index": 0,
                    "name": "field_0",
                    "nativeType": "u64",
                    "wireShape": "u64",
                    "confidence": "message-unmarshal-call"
                }]
            }],
            "fieldRegistrationFunctions": []
        }))
        .expect("schema");

        let output = NetworkRustEmitter::emit_messages(&schema).expect("message source");

        assert_eq!(output.report.message_generation_plan_count, 1);
        assert_eq!(output.report.generatable_message_count, 0);
        assert_eq!(output.report.blocked_message_count, 1);
        assert_eq!(
            output.report.message_generation_plans[0].blocked_reasons,
            vec!["placeholder-field-name:1"]
        );
        assert!(!output.source.contains("pub struct PingMsg"));
    }

    #[test]
    fn emits_message_fields_from_explicit_rust_types_without_wire_shapes() {
        let schema = NetworkSchema::from_ghidra_static_network_report(&json!({
            "registryEntries": [{
                "uuid": "0B826B33-89F5-49E0-B8CB-FE4433427778",
                "typeIndex": 19,
                "typeName": "RegistrationRequestV3Msg",
                "fields": [{
                    "index": 0,
                    "name": "LoginToken",
                    "nativeType": "LoginToken",
                    "rustType": "::nw_network::LoginToken",
                    "confidence": "message-unmarshal-call"
                }]
            }],
            "fieldRegistrationFunctions": []
        }))
        .expect("schema");

        let output = NetworkRustEmitter::emit_messages(&schema).expect("message source");

        assert_eq!(output.report.message_generation_plan_count, 1);
        assert_eq!(output.report.generatable_message_count, 1);
        assert_eq!(output.report.blocked_message_count, 0);
        let plan = &output.report.message_generation_plans[0];
        assert_eq!(plan.missing_wire_shape_count, 1);
        assert_eq!(plan.missing_field_type_count, 0);
        assert_eq!(plan.supported_field_count, 1);
        assert!(
            output
                .source
                .contains("pub login_token: ::nw_network::LoginToken")
        );
    }

    #[test]
    fn emits_hub_address_for_proxy_address_message_fields() {
        let schema = NetworkSchema::from_ghidra_static_network_report(&json!({
            "registryEntries": [{
                "uuid": "96A58E69-7BD5-45C5-86E4-DAF9F5EB1E86",
                "typeIndex": 397,
                "typeName": "Replicate::RegisterFragmentAccessMsg",
                "fields": [{
                    "index": 0,
                    "name": "ProxyRef",
                    "nativeType": "Amazon::Hub::ActorRef",
                    "confidence": "message-unmarshal-helper-direct-type-call"
                }, {
                    "index": 1,
                    "name": "Key",
                    "nativeType": "FragmentKey",
                    "confidence": "message-signature-source"
                }]
            }],
            "fieldRegistrationFunctions": []
        }))
        .expect("schema");

        let output = NetworkRustEmitter::emit_messages(&schema).expect("message source");

        assert_eq!(output.report.generatable_message_count, 1);
        assert_eq!(output.report.blocked_message_count, 0);
        assert!(
            output
                .source
                .contains("pub proxy_ref: ::nw_network::HubAddress")
        );
        assert!(
            output
                .source
                .contains("pub key: ::nw_network::hub::FragmentKey")
        );
    }

    #[test]
    fn emits_baselineable_fragment_for_baselineable_fragment_message_fields() {
        let schema = NetworkSchema::from_ghidra_static_network_report(&json!({
            "registryEntries": [{
                "uuid": "951EF3ED-C9A0-4E3D-A6FD-7FE0673D28D2",
                "typeIndex": 422,
                "typeName": "ReplicateClient::FragmentUpdateMsg",
                "fields": [{
                    "index": 0,
                    "name": "Fragment",
                    "nativeType": "Amazon::Hub::BaselineableFragment",
                    "confidence": "message-unmarshal-helper-direct-type-call"
                }]
            }],
            "fieldRegistrationFunctions": []
        }))
        .expect("schema");

        let output = NetworkRustEmitter::emit_messages(&schema).expect("message source");

        assert_eq!(output.report.generatable_message_count, 1);
        assert_eq!(output.report.blocked_message_count, 0);
        assert!(
            output
                .source
                .contains("pub fragment: ::nw_network::hub::BaselineableFragment")
        );
    }

    #[test]
    fn emits_fragment_messages_from_source_signature_merge() {
        let mut schema = NetworkSchema::from_ghidra_static_network_report(&json!({
            "registryEntries": [{
                "uuid": "96A58E69-7BD5-45C5-86E4-DAF9F5EB1E86",
                "typeIndex": 397,
                "typeName": "Replicate::RegisterFragmentAccessMsg",
                "fields": [{
                    "index": 0,
                    "name": "field_0",
                    "nativeType": "u32",
                    "storageExpression": "param_3 + 1",
                    "wireShape": "u32",
                    "confidence": "message-unmarshal-helper-wrapper"
                }]
            }, {
                "uuid": "2B7640E0-4204-4E52-998A-C2DB02E0A480",
                "typeIndex": 399,
                "typeName": "Replicate::UnregisterFragmentAccessMsg",
                "fields": [{
                    "index": 0,
                    "name": "field_0",
                    "nativeType": "u32",
                    "storageExpression": "param_3 + 1",
                    "wireShape": "u32",
                    "confidence": "message-unmarshal-helper-wrapper"
                }]
            }, {
                "uuid": "951EF3ED-C9A0-4E3D-A6FD-7FE0673D28D2",
                "typeIndex": 422,
                "typeName": "ReplicateClient::FragmentUpdateMsg",
                "fields": [{
                    "index": 0,
                    "name": "ProxyAddress",
                    "nameSource": "message-native-type-name",
                    "nativeType": "ProxyAddress",
                    "confidence": "message-unmarshal-inline-direct-type-call"
                }, {
                    "index": 1,
                    "name": "field_1",
                    "nativeType": "u32",
                    "wireShape": "u32",
                    "confidence": "message-unmarshal-inline-call"
                }]
            }],
            "fieldRegistrationFunctions": []
        }))
        .expect("schema");
        schema.merge_message_signatures(
            &fragment_message_signatures(),
            Some("message-signatures.json".to_owned()),
        );

        let output = NetworkRustEmitter::emit_messages(&schema).expect("message source");

        assert_eq!(output.report.generatable_message_count, 3);
        assert_eq!(output.report.blocked_message_count, 0);
        assert!(
            output
                .source
                .contains("pub struct RegisterFragmentAccessMsg")
        );
        assert!(
            output
                .source
                .contains("pub struct UnregisterFragmentAccessMsg")
        );
        assert!(output.source.contains("pub struct FragmentUpdateMsg"));
        assert!(
            output
                .source
                .contains("pub proxy_ref: ::nw_network::HubAddress")
        );
        assert!(
            output
                .source
                .contains("pub target_ref: ::nw_network::HubAddress")
        );
        assert!(
            output
                .source
                .contains("pub key: ::nw_network::hub::FragmentKey")
        );
        assert!(
            output
                .source
                .contains("pub fragment: ::nw_network::hub::BaselineableFragment")
        );
    }

    #[test]
    fn emits_conversion_marshaler_for_explicit_message_scalar_types() {
        let schema = NetworkSchema::from_ghidra_static_network_report(&json!({
            "registryEntries": [{
                "uuid": "0B826B33-89F5-49E0-B8CB-FE4433427778",
                "typeIndex": 19,
                "typeName": "GridSideMsg",
                "fields": [{
                    "index": 0,
                    "name": "GridSide",
                    "nativeType": "u8",
                    "rustType": "::nw_network::source::GridSides",
                    "wireShape": "u8",
                    "confidence": "message-unmarshal-call"
                }]
            }],
            "fieldRegistrationFunctions": []
        }))
        .expect("schema");

        let output = NetworkRustEmitter::emit_messages(&schema).expect("message source");

        assert_eq!(output.report.generatable_message_count, 1);
        assert!(
            output
                .source
                .contains("pub grid_side: ::nw_network::source::GridSides")
        );
        assert!(
            output.source.contains("codec =")
                && output.source.contains(
                    "::nw_network::serialize::ConversionMarshaler<u8, ::nw_network::source::GridSides>"
                )
        );
    }

    #[test]
    fn leaves_explicit_self_marshaling_scalar_types_unwrapped() {
        let schema = NetworkSchema::from_ghidra_static_network_report(&json!({
            "registryEntries": [{
                "uuid": "0B826B33-89F5-49E0-B8CB-FE4433427778",
                "typeIndex": 19,
                "typeName": "RegistrationRequestV3Msg",
                "fields": [{
                    "index": 0,
                    "name": "TypeIndexCrc",
                    "nativeType": "AZ::Crc32",
                    "rustType": "::nw_network::TypeIndexCrc",
                    "wireShape": "u32",
                    "confidence": "message-unmarshal-call"
                }]
            }],
            "fieldRegistrationFunctions": []
        }))
        .expect("schema");

        let output = NetworkRustEmitter::emit_messages(&schema).expect("message source");

        assert_eq!(output.report.generatable_message_count, 1);
        assert!(
            output
                .source
                .contains("pub type_index_crc: ::nw_network::TypeIndexCrc")
        );
        assert!(!output.source.contains("ConversionMarshaler"));
        assert!(!output.source.contains("codec ="));
    }

    #[test]
    fn emits_conversion_marshaler_for_explicit_replicated_state_scalar_types() {
        let schema = NetworkSchema::from_ghidra_static_network_report(&json!({
            "registryEntries": [{
                "uuid": "A85DF621-DCE0-409F-8D39-A447EA0807FF",
                "typeIndex": 28,
                "typeName": "Javelin::GridSideReplicatedState",
                "rootKinds": ["replicated-state"],
                "fields": [{
                    "index": 0,
                    "name": "GridSide",
                    "group": 0,
                    "nativeType": "u8",
                    "rustType": "::nw_network::source::GridSides",
                    "wireShape": "u8",
                    "confidence": "exact"
                }]
            }],
            "fieldRegistrationFunctions": []
        }))
        .expect("schema");

        let output =
            NetworkRustEmitter::emit_replicated_states(&schema, [28]).expect("state source");

        assert_eq!(output.report.generatable_state_count, 1);
        assert!(
            output
                .source
                .contains("pub grid_side: ::nw_network::serialize::ReplicatedFieldHandler<")
        );
        assert!(output.source.contains("::nw_network::source::GridSides"));
        assert!(
            output
                .source
                .contains("::nw_network::serialize::ConversionMarshaler<")
        );
        assert!(output.source.contains("u8,"));
    }

    #[test]
    fn emits_marshaler_conversions_for_compact_generated_enums() {
        let item = SerializeCodegenItem {
            source_type_id: Uuid::from_u128(0xffe86b0916b9429e9cd22901adbe8de3),
            source_name: "GridSides".to_owned(),
            role: crate::role::ReflectedTypeRole::SupportType,
            is_reflection_marker: false,
            is_abstract: None,
            factory: None,
            rtti_base_chain: Vec::new(),
            kind: SerializeCodegenItemKind::Enum,
            enum_underlying_type: Some(ResolvedType::Scalar(ScalarType::I32)),
            fields: Vec::new(),
            variants: vec![
                SerializeCodegenVariant {
                    source_name: "InvalidSide".to_owned(),
                    value_u64: Some(0),
                    value_u32: Some(0),
                    value_i32: Some(0),
                },
                SerializeCodegenVariant {
                    source_name: "Left".to_owned(),
                    value_u64: Some(4),
                    value_u32: Some(4),
                    value_i32: Some(4),
                },
            ],
        };

        let output =
            NetworkRustEmitter::emit_marshaler_conversions([&item]).expect("conversion source");

        assert_eq!(output.report.marshaler_conversion_count, 3);
        assert!(
            output
                .source
                .contains("impl ::nw_network::serialize::MarshalerConversion<u8>")
        );
        assert!(
            output
                .source
                .contains("for ::nw_network::source::GridSides")
        );
        assert!(output.source.contains("let raw = i32::from(self);"));
        assert!(output.source.contains("min: 0u64"));
        assert!(output.source.contains("max: 4u64"));
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
    fn qualifies_identity_leaf_name_collisions_with_namespace() {
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
        assert!(output.source.contains("pub struct FirstSharedName"));
        assert!(output.source.contains("pub struct SecondSharedName"));
    }
}
