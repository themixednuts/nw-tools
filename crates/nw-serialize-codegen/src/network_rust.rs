use std::collections::{BTreeMap, BTreeSet};

use quote::{format_ident, quote};
use serde::{Deserialize, Serialize};
use syn::{LitInt, LitStr};
use thiserror::Error;
use uuid::Uuid;

use crate::CodegenContext;
use crate::ir::{SerializeCodegenItem, SerializeCodegenItemKind};
use crate::naming::{rust_field_ident, rust_module_ident, rust_type_ident};
use crate::network_schema::{
    NetworkConfidence, NetworkField, NetworkFragmentMetadata, NetworkNativeTypeInfoEvidence,
    NetworkReplicatedContainerShape, NetworkReplicatedContainerStorageKind,
    NetworkReplicatedContainerWireShape, NetworkSchema, NetworkSerializeFieldType,
    NetworkSerializeKind, NetworkSerializeRole, NetworkSerializeType, NetworkType,
    NetworkTypeCapability, NetworkWireScalarShape as SchemaWireScalarShape,
    NetworkWireShape as SchemaWireShape,
};
use crate::types::{ResolvedType, ScalarType};

pub const NETWORK_RUST_EMITTER_VERSION: &str = "network-rust-v45";

#[derive(Debug, Error)]
pub enum NetworkRustEmitError {
    #[error("generated network Rust source did not parse")]
    Parse(#[from] syn::Error),
    #[error("network Rust source emission was cancelled")]
    Cancelled,
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
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fragment_category: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fragment_category_value: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub is_metadata_fragment: Option<bool>,
    pub field_count: usize,
    #[serde(default)]
    pub attribute_count: usize,
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
    #[serde(skip_serializing_if = "Option::is_none")]
    pub registration_kind: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub filter_group_attribute: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub native_type: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source_type_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source_type_id: Option<Uuid>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub serialize_type_name: Option<String>,
    pub handler_vtable: Option<String>,
    pub wire_shape: Option<SchemaWireShape>,
    pub wire_shape_source: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub value_type_candidates: Vec<NetworkNativeTypeInfoEvidence>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub container_key_type_shape: Option<crate::network_schema::NetworkNestedTypeShape>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub container_embedded_key_type_shapes: Vec<crate::network_schema::NetworkNestedTypeShape>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub container_value_type_shape: Option<crate::network_schema::NetworkNestedTypeShape>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub container_embedded_value_type_shapes: Vec<crate::network_schema::NetworkNestedTypeShape>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub nested_type_shape: Option<crate::network_schema::NetworkNestedTypeShape>,
    pub rust_value_type: Option<String>,
    pub rust_field_type: Option<String>,
    #[serde(default)]
    pub constructor_write_count: usize,
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
    #[serde(default)]
    pub missing_support_type_count: usize,
    #[serde(default)]
    pub missing_composite_support_type_count: usize,
    #[serde(default)]
    pub placeholder_field_name_count: usize,
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
    pub native_type: Option<String>,
    pub source_type_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source_type_id: Option<Uuid>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub serialize_type_name: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub value_type_candidates: Vec<NetworkNativeTypeInfoEvidence>,
    pub rust_value_type: Option<String>,
    pub blocked_reason: Option<String>,
}

#[derive(Debug, Default)]
pub struct NetworkRustEmitter;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NetworkReplicatedStateEmitOptions {
    pub register_fragments: bool,
    pub registered_type_indices: Option<BTreeSet<u32>>,
}

impl Default for NetworkReplicatedStateEmitOptions {
    fn default() -> Self {
        Self {
            register_fragments: true,
            registered_type_indices: None,
        }
    }
}

impl NetworkReplicatedStateEmitOptions {
    pub fn unregistered() -> Self {
        Self {
            register_fragments: false,
            registered_type_indices: None,
        }
    }

    pub fn register_only(type_indices: impl IntoIterator<Item = u32>) -> Self {
        Self {
            register_fragments: true,
            registered_type_indices: Some(type_indices.into_iter().collect()),
        }
    }

    fn registers_type_index(&self, type_index: u32) -> bool {
        self.register_fragments
            && self
                .registered_type_indices
                .as_ref()
                .is_none_or(|type_indices| type_indices.contains(&type_index))
    }
}

impl NetworkRustEmitter {
    pub fn emit_descriptors(
        schema: &NetworkSchema,
    ) -> Result<NetworkRustOutput, NetworkRustEmitError> {
        Self::emit_descriptors_with_context(schema, &CodegenContext::inline())
    }

    pub fn emit_descriptors_with_context(
        schema: &NetworkSchema,
        context: &CodegenContext,
    ) -> Result<NetworkRustOutput, NetworkRustEmitError> {
        let mut report = NetworkRustGenerationReport::default();
        let wire_shapes = wire_shapes_by_handler_vtable(schema);
        let value_type_candidates = value_type_candidates_by_handler_vtable(schema);
        let serialize_types = serialize_types_by_type_id(schema);
        let container_shapes = container_shapes_by_handler_vtable(schema, &serialize_types);
        let descriptors = schema
            .types
            .iter()
            .filter_map(|network_type| descriptor_tokens(network_type, &wire_shapes, &mut report))
            .collect::<Vec<_>>();
        report.descriptor_count = descriptors.len();
        report.identity_name_collision_count = identity_name_collision_count(schema);
        report.state_generation_plans = state_generation_plans(
            schema,
            &wire_shapes,
            &container_shapes,
            &value_type_candidates,
            &serialize_types,
            context,
        )?;
        report.state_generation_plan_count = report.state_generation_plans.len();
        report.generatable_state_count = report
            .state_generation_plans
            .iter()
            .filter(|plan| plan.can_generate)
            .count();
        report.blocked_state_count =
            report.state_generation_plan_count - report.generatable_state_count;
        report.message_generation_plans =
            message_generation_plans(schema, &wire_shapes, &value_type_candidates, context)?;
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
            pub enum NetworkTypeCapability {
                ReplicatedState,
                DirectMessage,
                RegisteredFields,
                SupportData,
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
            pub enum NetworkWireScalarShape {
                Bool,
                U8,
                U16,
                U32,
                U64,
                F32,
                F64,
                HalfF32,
                VlqU32,
                VlqU64,
                SequenceNumber,
                Vec2,
                Vec3,
                Vec4,
                Quat,
                QuatCompNorm,
                Vec2Comp,
                Vec3Comp,
                Vec3CompNorm,
                QuatComp,
                QuatSmallestThree,
                NonUniformScaleComp,
                PositionAnchor,
                TransformCompressor,
                PackedSize,
                Mat3,
                Affine3,
                Aabb2d,
                Aabb3d,
                ActorRef,
                EntityRef,
                FixedBytes(u16),
                String,
            }

            #[derive(Debug, Clone, Copy, PartialEq, Eq)]
            pub struct NetworkReplicatedContainerWireShape {
                pub key: NetworkWireScalarShape,
                pub value: NetworkWireScalarShape,
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
                VlqU64,
                SequenceNumber,
                Vec2,
                Vec3,
                Vec4,
                Quat,
                QuatCompNorm,
                Vec2Comp,
                Vec3Comp,
                Vec3CompNorm,
                QuatComp,
                QuatSmallestThree,
                NonUniformScaleComp,
                PositionAnchor,
                TransformCompressor,
                PackedSize,
                Mat3,
                Affine3,
                Aabb2d,
                Aabb3d,
                ActorRef,
                EntityRef,
                FixedBytes(u16),
                String,
                ReplicatedContainer(NetworkReplicatedContainerWireShape),
            }

            #[derive(Debug, Clone, Copy, PartialEq, Eq)]
            pub struct NetworkFieldDescriptor {
                pub index: u32,
                pub name: &'static str,
                pub group: Option<u32>,
                pub native_type: Option<&'static str>,
                pub source_type_name: Option<&'static str>,
                pub rust_type: Option<&'static str>,
                pub unmarshal_target_name: Option<&'static str>,
                pub storage_offset: Option<u32>,
                pub wire_shape: Option<NetworkWireShape>,
                pub confidence: NetworkFieldConfidence,
            }

            #[derive(Debug, Clone, Copy, PartialEq, Eq)]
            pub struct NetworkTypeDescriptor {
                pub type_id: Uuid,
                pub type_index: u32,
                pub name: Option<&'static str>,
                pub capabilities: &'static [NetworkTypeCapability],
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
                pub fn has_capability(&self, capability: NetworkTypeCapability) -> bool {
                    self.capabilities.contains(&capability)
                }

                #[must_use]
                pub fn is_direct_message(&self) -> bool {
                    self.has_capability(NetworkTypeCapability::DirectMessage)
                }

                #[must_use]
                pub fn is_replicated_state(&self) -> bool {
                    self.has_capability(NetworkTypeCapability::ReplicatedState)
                }

                #[must_use]
                pub fn has_registered_fields(&self) -> bool {
                    self.has_capability(NetworkTypeCapability::RegisteredFields)
                }

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
                const CAPABILITIES: &'static [NetworkTypeCapability];

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
                    .is_some_and(NetworkTypeDescriptor::is_replicated_state)
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
                            .is_some_and(|descriptor| !descriptor.is_replicated_state())
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
        Self::emit_replicated_states_with_options(
            schema,
            type_indices,
            NetworkReplicatedStateEmitOptions::default(),
        )
    }

    pub fn emit_replicated_states_with_options(
        schema: &NetworkSchema,
        type_indices: impl IntoIterator<Item = u32>,
        options: NetworkReplicatedStateEmitOptions,
    ) -> Result<NetworkRustOutput, NetworkRustEmitError> {
        Self::emit_replicated_states_with_options_and_context(
            schema,
            type_indices,
            options,
            &CodegenContext::inline(),
        )
    }

    pub fn emit_replicated_states_with_options_and_context(
        schema: &NetworkSchema,
        type_indices: impl IntoIterator<Item = u32>,
        options: NetworkReplicatedStateEmitOptions,
        context: &CodegenContext,
    ) -> Result<NetworkRustOutput, NetworkRustEmitError> {
        let selected = type_indices.into_iter().collect::<BTreeSet<_>>();
        let wire_shapes = wire_shapes_by_handler_vtable(schema);
        let wire_shape_sources = wire_shape_sources_by_handler_vtable(schema);
        let value_type_candidates = value_type_candidates_by_handler_vtable(schema);
        let serialize_types = serialize_types_by_type_id(schema);
        let container_shapes = container_shapes_by_handler_vtable(schema, &serialize_types);
        let rust_names = identity_names_by_type_index(schema);
        let types_by_type_index = schema
            .types
            .iter()
            .filter_map(|network_type| Some((network_type.type_index?, network_type)))
            .collect::<BTreeMap<_, _>>();
        let state_types = schema
            .types
            .iter()
            .filter(|network_type| {
                network_type
                    .capabilities
                    .contains(&NetworkTypeCapability::ReplicatedState)
            })
            .collect::<Vec<_>>();
        let planned =
            context
                .runner()
                .map_until_cancelled(&state_types, context.cancel(), |network_type| {
                    (
                        network_type.type_index,
                        state_generation_plan(
                            network_type,
                            &wire_shapes,
                            &wire_shape_sources,
                            &container_shapes,
                            &value_type_candidates,
                            &serialize_types,
                        ),
                    )
                });
        if planned.was_cancelled() {
            return Err(NetworkRustEmitError::Cancelled);
        }
        let plans_by_type_index = planned
            .into_completed()
            .into_iter()
            .filter_map(|(type_index, plan)| Some((type_index?, plan)))
            .collect::<BTreeMap<_, _>>();

        let mut report = NetworkRustGenerationReport::default();
        let mut modules = Vec::new();
        for type_index in selected {
            if context.is_cancelled() {
                return Err(NetworkRustEmitError::Cancelled);
            }
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
                    &options,
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
        Self::emit_messages_with_context(schema, &CodegenContext::inline())
    }

    pub fn emit_messages_with_context(
        schema: &NetworkSchema,
        context: &CodegenContext,
    ) -> Result<NetworkRustOutput, NetworkRustEmitError> {
        let wire_shapes = wire_shapes_by_handler_vtable(schema);
        let wire_shape_sources = wire_shape_sources_by_handler_vtable(schema);
        let value_type_candidates = value_type_candidates_by_handler_vtable(schema);
        let support_evidence = message_support_evidence(schema);
        let rust_names = identity_names_by_type_index(schema);
        let message_types = schema
            .types
            .iter()
            .filter(|network_type| {
                network_type
                    .capabilities
                    .contains(&NetworkTypeCapability::DirectMessage)
            })
            .collect::<Vec<_>>();
        let plans = context.runner().map_until_cancelled(
            &message_types,
            context.cancel(),
            |network_type| {
                message_generation_plan(
                    network_type,
                    &wire_shapes,
                    &wire_shape_sources,
                    &value_type_candidates,
                    &support_evidence,
                )
            },
        );
        if plans.was_cancelled() {
            return Err(NetworkRustEmitError::Cancelled);
        }

        let mut report = NetworkRustGenerationReport::default();
        let mut modules = Vec::new();
        for (network_type, plan) in message_types.into_iter().zip(plans.into_completed()) {
            if context.is_cancelled() {
                return Err(NetworkRustEmitError::Cancelled);
            }
            report.message_generation_plans.push(plan.clone());
            if plan.can_generate {
                modules.push(message_module_tokens(network_type, &plan, &rust_names));
            }
        }

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
        Self::emit_marshaler_conversions_with_context(items, &CodegenContext::inline())
    }

    pub fn emit_marshaler_conversions_with_context<'a>(
        items: impl IntoIterator<Item = &'a SerializeCodegenItem>,
        context: &CodegenContext,
    ) -> Result<NetworkRustOutput, NetworkRustEmitError> {
        let items = items.into_iter().collect::<Vec<_>>();
        let items_by_type_id = items
            .iter()
            .map(|item| (item.source_type_id, *item))
            .collect::<BTreeMap<_, _>>();
        let mut report = NetworkRustGenerationReport::default();
        let mut conversions = Vec::new();
        for item in items {
            if context.is_cancelled() {
                return Err(NetworkRustEmitError::Cancelled);
            }
            conversions.extend(enum_marshaler_conversion_tokens(item));
            if let Some(tokens) = struct_native_marshaler_tokens(item, &items_by_type_id) {
                conversions.push(tokens);
            }
        }
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

fn struct_native_marshaler_tokens(
    item: &SerializeCodegenItem,
    items_by_type_id: &BTreeMap<Uuid, &SerializeCodegenItem>,
) -> Option<proc_macro2::TokenStream> {
    if item.kind != SerializeCodegenItemKind::Struct
        || item.is_abstract == Some(true)
        || item.fields.is_empty()
    {
        return None;
    }

    let struct_ident = format_ident!("{}", rust_type_ident(&item.source_name));
    let fields = item
        .fields
        .iter()
        .map(|field| struct_marshaler_field_tokens(field, items_by_type_id))
        .collect::<Option<Vec<_>>>()?;
    let marshal_fields = fields.iter().map(|field| &field.marshal);
    let unmarshal_fields = fields.iter().map(|field| &field.unmarshal);

    Some(quote! {
        impl ::nw_network::serialize::Marshaler for ::nw_network::source::#struct_ident {
            fn marshal(&self, wb: &mut ::nw_network::serialize::WriteBuffer) {
                #(#marshal_fields)*
            }

            fn unmarshal(
                rb: &mut ::nw_network::serialize::ReadBuffer,
            ) -> Result<Self, ::nw_network::serialize::MarshalerError> {
                Ok(Self {
                    #(#unmarshal_fields)*
                })
            }
        }
    })
}

struct StructMarshalerFieldTokens {
    marshal: proc_macro2::TokenStream,
    unmarshal: proc_macro2::TokenStream,
}

fn struct_marshaler_field_tokens(
    field: &crate::ir::SerializeCodegenField,
    items_by_type_id: &BTreeMap<Uuid, &SerializeCodegenItem>,
) -> Option<StructMarshalerFieldTokens> {
    let field_ident = format_ident!("{}", rust_field_ident(&field.source_name));
    if let ResolvedType::Named { type_id, .. } = &field.resolved_type
        && let Some(enum_item) = items_by_type_id.get(type_id)
        && enum_item.kind == SerializeCodegenItemKind::Enum
    {
        return struct_enum_field_marshaler_tokens(&field_ident, enum_item);
    }

    Some(StructMarshalerFieldTokens {
        marshal: quote! {
            ::nw_network::serialize::Marshaler::marshal(&self.#field_ident, wb);
        },
        unmarshal: quote! {
            #field_ident: ::nw_network::serialize::Marshaler::unmarshal(rb)?,
        },
    })
}

fn struct_enum_field_marshaler_tokens(
    field_ident: &proc_macro2::Ident,
    enum_item: &SerializeCodegenItem,
) -> Option<StructMarshalerFieldTokens> {
    let underlying = enum_underlying_scalar(enum_item)?;
    let (min, max) = enum_value_range(enum_item)?;
    let enum_ident = format_ident!("{}", rust_type_ident(&enum_item.source_name));
    let enum_type = quote!(::nw_network::source::#enum_ident);
    let underlying_ty = enum_underlying_rust_type(underlying);
    let min_u64 = u64::try_from(min).unwrap_or(0);
    let max_u64 = u64::try_from(max).ok()?;

    Some(StructMarshalerFieldTokens {
        marshal: quote! {
            let raw = #underlying_ty::from(self.#field_ident);
            ::nw_network::serialize::Marshaler::marshal(&raw, wb);
        },
        unmarshal: quote! {
            #field_ident: {
                let raw = <#underlying_ty as ::nw_network::serialize::Marshaler>::unmarshal(rb)?;
                <#enum_type as ::core::convert::TryFrom<#underlying_ty>>::try_from(raw).map_err(|_| {
                    ::nw_network::serialize::MarshalerError::InvalidRange {
                        value: raw as u64,
                        min: #min_u64,
                        max: #max_u64,
                    }
                })?
            },
        },
    })
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

fn container_shapes_by_handler_vtable<'a>(
    schema: &'a NetworkSchema,
    serialize_types: &BTreeMap<Uuid, &NetworkSerializeType>,
) -> BTreeMap<&'a str, NetworkReplicatedContainerShape> {
    let mut shapes: BTreeMap<&'a str, NetworkReplicatedContainerShape> = BTreeMap::new();
    for vtable in &schema.field_handler_vtables {
        let Some(address) = vtable.address.as_deref() else {
            continue;
        };
        let candidate_shape = candidate_backed_container_shape_from_vtable(vtable, serialize_types);
        let Some(shape) = candidate_shape
            .clone()
            .or_else(|| vtable.container_shape.clone())
        else {
            continue;
        };
        if candidate_shape.is_some()
            || shapes.get(address).is_none_or(|existing| {
                existing.source.as_deref()
                    != Some("replicated-container-map-candidate-value-suffix")
            })
        {
            shapes.insert(address, shape);
        }
    }
    shapes
}

fn candidate_backed_container_shape_from_vtable(
    vtable: &crate::network_schema::NetworkFieldHandlerVtable,
    serialize_types: &BTreeMap<Uuid, &NetworkSerializeType>,
) -> Option<NetworkReplicatedContainerShape> {
    if vtable.handler_kind.as_deref() != Some("replicated-container") {
        return None;
    }

    let full_data_candidates =
        replicated_container_full_data_shape_candidates_from_names(&vtable.full_marshal_shapes);
    if full_data_candidates.is_empty() || vtable.value_type_candidates.is_empty() {
        return None;
    }

    let mut matches = Vec::new();
    for full_data in full_data_candidates {
        let mut seen = BTreeSet::new();
        for candidate in &vtable.value_type_candidates {
            let Some(type_id) = candidate.type_id else {
                continue;
            };
            if !seen.insert(type_id) {
                continue;
            }
            let Some(serialize) = serialize_types.get(&type_id).copied() else {
                continue;
            };
            if serialize.role != NetworkSerializeRole::SupportType
                || serialize.wire_shapes.is_empty()
                || full_data.len() <= serialize.wire_shapes.len()
            {
                continue;
            }
            let split = full_data.len() - serialize.wire_shapes.len();
            if wire_scalar_shapes_match(&full_data[split..], &serialize.wire_shapes) {
                matches.push((serialize, full_data[..split].to_vec()));
            }
        }
    }
    matches.sort_by_key(|(serialize, key_wire_shapes)| {
        (
            serialize.type_id,
            key_wire_shapes
                .iter()
                .map(|shape| wire_scalar_shape_name(*shape))
                .collect::<Vec<_>>(),
        )
    });
    matches.dedup_by(|left, right| left.0.type_id == right.0.type_id && left.1 == right.1);

    let [(serialize, key_wire_shapes)] = matches.as_slice() else {
        return None;
    };
    let key_wire_shape = *key_wire_shapes.first()?;
    let key_type_name = unique_candidate_serialize_type_for_wire_shapes(
        &vtable.value_type_candidates,
        key_wire_shapes,
        serialize_types,
        Some(serialize.type_id),
    )
    .map(|serialize| serialize.name.clone());
    if key_type_name.is_none() && key_wire_shapes.len() > 1 {
        return None;
    }

    Some(NetworkReplicatedContainerShape {
        storage: NetworkReplicatedContainerStorageKind::Map,
        key_wire_shape,
        key_wire_shapes: key_wire_shapes.clone(),
        key_native_type: None,
        key_native_type_source: None,
        key_type_name,
        key_type_shape: None,
        value_wire_shapes: serialize.wire_shapes.clone(),
        delta_value_wire_shapes: Vec::new(),
        value_type_name: Some(serialize.name.clone()),
        value_type_id: Some(serialize.type_id),
        value_type_info_address: None,
        value_type_shape: None,
        embedded_value_type_shapes: Vec::new(),
        source: Some("replicated-container-map-candidate-value-suffix".to_owned()),
    })
}

fn unique_candidate_serialize_type_for_wire_shapes<'a>(
    candidates: &[NetworkNativeTypeInfoEvidence],
    wire_shapes: &[SchemaWireScalarShape],
    serialize_types: &'a BTreeMap<Uuid, &NetworkSerializeType>,
    except_type_id: Option<Uuid>,
) -> Option<&'a NetworkSerializeType> {
    let mut matches = Vec::new();
    let mut seen = BTreeSet::new();
    for candidate in candidates {
        let Some(type_id) = candidate.type_id else {
            continue;
        };
        if Some(type_id) == except_type_id || !seen.insert(type_id) {
            continue;
        }
        let Some(serialize) = serialize_types.get(&type_id).copied() else {
            continue;
        };
        if serialize.role == NetworkSerializeRole::SupportType
            && wire_scalar_shapes_match(wire_shapes, &serialize.wire_shapes)
        {
            matches.push(serialize);
        }
    }
    let [matched] = matches.as_slice() else {
        return None;
    };
    Some(*matched)
}

fn replicated_container_full_data_shape_candidates_from_names(
    shapes: &[String],
) -> Vec<Vec<SchemaWireScalarShape>> {
    let start = shapes
        .iter()
        .position(|shape| shape == "sequence-number")
        .map_or(0, |index| index + 1);
    let mut skipped_outer_count = false;
    let full_data = shapes[start..]
        .iter()
        .filter_map(|shape| {
            let shape = shape.as_str();
            if shape == "sequence-number" {
                return None;
            }
            if !skipped_outer_count && shape == "vlq-u32" {
                skipped_outer_count = true;
                return None;
            }
            wire_scalar_shape_from_name(shape)
        })
        .collect::<Vec<_>>();
    if full_data.len() < 2 {
        return Vec::new();
    }

    let mut candidates = vec![full_data.clone()];
    if full_data.last() == Some(&SchemaWireScalarShape::VlqU32) {
        let mut stripped = full_data;
        stripped.pop();
        if stripped.len() >= 2 {
            candidates.push(stripped);
        }
    }
    candidates
}

fn serialize_types_by_type_id(schema: &NetworkSchema) -> BTreeMap<Uuid, &NetworkSerializeType> {
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

fn wire_scalar_shape_name(shape: SchemaWireScalarShape) -> String {
    match shape {
        SchemaWireScalarShape::Bool => "bool".to_owned(),
        SchemaWireScalarShape::U8 => "u8".to_owned(),
        SchemaWireScalarShape::U16 => "u16".to_owned(),
        SchemaWireScalarShape::U32 => "u32".to_owned(),
        SchemaWireScalarShape::U64 => "u64".to_owned(),
        SchemaWireScalarShape::F32 => "f32".to_owned(),
        SchemaWireScalarShape::F64 => "f64".to_owned(),
        SchemaWireScalarShape::HalfF32 => "half-f32".to_owned(),
        SchemaWireScalarShape::VlqU32 => "vlq-u32".to_owned(),
        SchemaWireScalarShape::VlqU64 => "vlq-u64".to_owned(),
        SchemaWireScalarShape::SequenceNumber => "sequence-number".to_owned(),
        SchemaWireScalarShape::Vec2 => "vec2".to_owned(),
        SchemaWireScalarShape::Vec3 => "vec3".to_owned(),
        SchemaWireScalarShape::Vec4 => "vec4".to_owned(),
        SchemaWireScalarShape::Quat => "quat".to_owned(),
        SchemaWireScalarShape::QuatCompNorm => "quat-comp-norm".to_owned(),
        SchemaWireScalarShape::Vec2Comp => "vec2-comp".to_owned(),
        SchemaWireScalarShape::Vec3Comp => "vec3-comp".to_owned(),
        SchemaWireScalarShape::Vec3CompNorm => "vec3-comp-norm".to_owned(),
        SchemaWireScalarShape::QuatComp => "quat-comp".to_owned(),
        SchemaWireScalarShape::QuatSmallestThree => "quat-smallest-three".to_owned(),
        SchemaWireScalarShape::NonUniformScaleComp => "non-uniform-scale-comp".to_owned(),
        SchemaWireScalarShape::PositionAnchor => "position-anchor".to_owned(),
        SchemaWireScalarShape::TransformCompressor => "transform-compressor".to_owned(),
        SchemaWireScalarShape::PackedSize => "packed-size".to_owned(),
        SchemaWireScalarShape::Mat3 => "mat3".to_owned(),
        SchemaWireScalarShape::Affine3 => "affine3".to_owned(),
        SchemaWireScalarShape::Aabb2d => "aabb2d".to_owned(),
        SchemaWireScalarShape::Aabb3d => "aabb3d".to_owned(),
        SchemaWireScalarShape::ActorRef => "actor-ref".to_owned(),
        SchemaWireScalarShape::EntityRef => "entity-ref".to_owned(),
        SchemaWireScalarShape::FixedBytes(len) => format!("fixed-bytes-{len}"),
        SchemaWireScalarShape::String => "string".to_owned(),
    }
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
    container_shapes: &BTreeMap<&str, NetworkReplicatedContainerShape>,
    value_type_candidates: &BTreeMap<&str, Vec<NetworkNativeTypeInfoEvidence>>,
    serialize_types: &BTreeMap<Uuid, &NetworkSerializeType>,
    context: &CodegenContext,
) -> Result<Vec<NetworkStateGenerationPlanReport>, NetworkRustEmitError> {
    let wire_shape_sources = wire_shape_sources_by_handler_vtable(schema);
    let state_types = schema
        .types
        .iter()
        .filter(|network_type| {
            network_type
                .capabilities
                .contains(&NetworkTypeCapability::ReplicatedState)
        })
        .collect::<Vec<_>>();
    let plans =
        context
            .runner()
            .map_until_cancelled(&state_types, context.cancel(), |network_type| {
                state_generation_plan(
                    network_type,
                    wire_shapes,
                    &wire_shape_sources,
                    container_shapes,
                    value_type_candidates,
                    serialize_types,
                )
            });
    if plans.was_cancelled() {
        return Err(NetworkRustEmitError::Cancelled);
    }
    Ok(plans.into_completed())
}

fn message_generation_plans(
    schema: &NetworkSchema,
    wire_shapes: &BTreeMap<&str, SchemaWireShape>,
    value_type_candidates: &BTreeMap<&str, Vec<NetworkNativeTypeInfoEvidence>>,
    context: &CodegenContext,
) -> Result<Vec<NetworkMessageGenerationPlanReport>, NetworkRustEmitError> {
    let wire_shape_sources = wire_shape_sources_by_handler_vtable(schema);
    let support_evidence = message_support_evidence(schema);
    let message_types = schema
        .types
        .iter()
        .filter(|network_type| {
            network_type
                .capabilities
                .contains(&NetworkTypeCapability::DirectMessage)
        })
        .collect::<Vec<_>>();
    let plans =
        context
            .runner()
            .map_until_cancelled(&message_types, context.cancel(), |network_type| {
                message_generation_plan(
                    network_type,
                    wire_shapes,
                    &wire_shape_sources,
                    value_type_candidates,
                    &support_evidence,
                )
            });
    if plans.was_cancelled() {
        return Err(NetworkRustEmitError::Cancelled);
    }
    Ok(plans.into_completed())
}

fn state_generation_plan(
    network_type: &NetworkType,
    wire_shapes: &BTreeMap<&str, SchemaWireShape>,
    wire_shape_sources: &BTreeMap<&str, &str>,
    container_shapes: &BTreeMap<&str, NetworkReplicatedContainerShape>,
    value_type_candidates: &BTreeMap<&str, Vec<NetworkNativeTypeInfoEvidence>>,
    serialize_types: &BTreeMap<Uuid, &NetworkSerializeType>,
) -> NetworkStateGenerationPlanReport {
    let attribute_count = network_type
        .fields
        .iter()
        .filter(|field| is_replicated_state_attribute_field(field))
        .count();
    let mut fields = network_type
        .fields
        .iter()
        .filter(|field| !is_replicated_state_attribute_field(field))
        .map(|field| {
            state_field_shape_report(
                field,
                wire_shapes,
                wire_shape_sources,
                container_shapes,
                value_type_candidates,
                serialize_types,
            )
        })
        .collect::<Vec<_>>();
    disambiguate_report_field_names(&mut fields);
    let field_count = fields.len();
    let shaped_field_count = fields
        .iter()
        .filter(|field| state_field_has_complete_shape(field))
        .count();
    let supported_field_count = fields.iter().filter(|field| field.supported).count();
    let missing_wire_shape_count = fields
        .iter()
        .filter(|field| field.blocked_reason.as_deref() == Some("missing-wire-shape"))
        .count();
    let unsupported_wire_shape_count = fields
        .iter()
        .filter(|field| field.blocked_reason.as_deref() == Some("unsupported-wire-shape"))
        .count();
    let container_codec_only_count = fields
        .iter()
        .filter(|field| field.blocked_reason.as_deref() == Some("container-codec-only"))
        .count();
    let missing_semantic_type_count = fields
        .iter()
        .filter(|field| field.blocked_reason.as_deref() == Some("missing-semantic-type"))
        .count();
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
    let block_counts = FieldBlockCounts {
        field_count,
        missing_wire_shape_count,
        missing_field_type_count: 0,
        missing_support_type_count: 0,
        missing_composite_support_type_count: 0,
        unsupported_wire_shape_count,
        container_codec_only_count,
        missing_semantic_type_count,
        invalid_field_metadata_count,
        low_confidence_field_count,
    };
    let blocked_reasons = state_blocked_reasons(network_type, block_counts);
    NetworkStateGenerationPlanReport {
        type_index: network_type.type_index,
        type_name: network_type.name.clone(),
        fragment_category: network_type
            .fragment_metadata
            .as_ref()
            .and_then(|metadata| metadata.category.clone()),
        fragment_category_value: network_type
            .fragment_metadata
            .as_ref()
            .and_then(|metadata| metadata.category_value),
        is_metadata_fragment: network_type
            .fragment_metadata
            .as_ref()
            .and_then(|metadata| metadata.is_metadata),
        field_count,
        attribute_count,
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

fn is_replicated_state_attribute_field(field: &NetworkField) -> bool {
    field.registration_kind.as_deref() == Some("attribute")
}

fn message_generation_plan(
    network_type: &NetworkType,
    wire_shapes: &BTreeMap<&str, SchemaWireShape>,
    wire_shape_sources: &BTreeMap<&str, &str>,
    value_type_candidates: &BTreeMap<&str, Vec<NetworkNativeTypeInfoEvidence>>,
    support_evidence: &MessageSupportEvidence,
) -> NetworkMessageGenerationPlanReport {
    let mut fields = network_type
        .fields
        .iter()
        .map(|field| {
            message_field_shape_report(
                field,
                wire_shapes,
                wire_shape_sources,
                value_type_candidates,
                support_evidence,
                network_type.name.as_deref(),
            )
        })
        .collect::<Vec<_>>();
    disambiguate_report_field_names(&mut fields);
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
    let missing_support_type_count = fields
        .iter()
        .filter(|field| field.blocked_reason.as_deref() == Some("missing-support-type"))
        .count();
    let missing_composite_support_type_count = fields
        .iter()
        .filter(|field| field.blocked_reason.as_deref() == Some("missing-composite-support-type"))
        .count();
    let unsupported_wire_shape_count = fields
        .iter()
        .filter(|field| field.blocked_reason.as_deref() == Some("unsupported-wire-shape"))
        .count();
    let container_codec_only_count = fields
        .iter()
        .filter(|field| field.blocked_reason.as_deref() == Some("container-codec-only"))
        .count();
    let missing_semantic_type_count = fields
        .iter()
        .filter(|field| field.blocked_reason.as_deref() == Some("missing-semantic-type"))
        .count();
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
        .filter(|field| is_placeholder_report_field_name(field))
        .count();
    let block_counts = FieldBlockCounts {
        field_count,
        missing_wire_shape_count,
        missing_field_type_count,
        missing_support_type_count,
        missing_composite_support_type_count,
        unsupported_wire_shape_count,
        container_codec_only_count,
        missing_semantic_type_count,
        invalid_field_metadata_count,
        low_confidence_field_count,
    };
    let blocked_reasons = message_blocked_reasons(network_type, block_counts);

    NetworkMessageGenerationPlanReport {
        type_index: network_type.type_index,
        type_name: network_type.name.clone(),
        field_count,
        shaped_field_count,
        supported_field_count,
        missing_wire_shape_count,
        missing_field_type_count,
        missing_support_type_count,
        missing_composite_support_type_count,
        placeholder_field_name_count,
        unsupported_wire_shape_count,
        low_confidence_field_count,
        can_generate: blocked_reasons.is_empty(),
        blocked_reasons,
        fields,
    }
}

fn disambiguate_report_field_names(fields: &mut [NetworkStateFieldShapeReport]) {
    let mut seen = BTreeMap::<String, usize>::new();
    for (ordinal, field) in fields.iter_mut().enumerate() {
        let Some(name) = field.field_name.as_deref() else {
            continue;
        };
        let ident = rust_field_ident(name);
        if let std::collections::btree_map::Entry::Vacant(entry) = seen.entry(ident.clone()) {
            entry.insert(1);
            continue;
        }

        let suffix_seed = field.field_index.unwrap_or(ordinal as u32);
        let mut attempt = 0;
        let candidate = loop {
            let suffix = if attempt == 0 {
                suffix_seed.to_string()
            } else {
                format!("{suffix_seed}_{attempt}")
            };
            let candidate = format!("{name}_{suffix}");
            let candidate_ident = rust_field_ident(&candidate);
            if let std::collections::btree_map::Entry::Vacant(entry) = seen.entry(candidate_ident) {
                entry.insert(1);
                break candidate;
            }
            attempt += 1;
        };

        if let Some(count) = seen.get_mut(&ident) {
            *count += 1;
        }
        field.field_name = Some(candidate);
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
        native_type: field.native_type.clone(),
        source_type_name: field.source_type_name.clone(),
        source_type_id: field.source_type_id,
        serialize_type_name: field.serialize_type_name.clone(),
        value_type_candidates: field.value_type_candidates.clone(),
        rust_value_type: field.rust_value_type.clone(),
        blocked_reason: field.blocked_reason.clone(),
    }
}

fn message_field_shape_report(
    field: &NetworkField,
    wire_shapes: &BTreeMap<&str, SchemaWireShape>,
    wire_shape_sources: &BTreeMap<&str, &str>,
    value_type_candidates: &BTreeMap<&str, Vec<NetworkNativeTypeInfoEvidence>>,
    support_evidence: &MessageSupportEvidence,
    message_type_name: Option<&str>,
) -> NetworkStateFieldShapeReport {
    let container_shapes = BTreeMap::new();
    let serialize_types = BTreeMap::new();
    let mut report = state_field_shape_report(
        field,
        wire_shapes,
        wire_shape_sources,
        &container_shapes,
        value_type_candidates,
        &serialize_types,
    );
    let source_type = serialize_field_scalar_source_type(field, report.wire_shape);
    let rust_type = field
        .rust_type
        .as_deref()
        .map(normalize_generated_rust_type)
        .or_else(|| existing_message_support_type(field, support_evidence).map(ToOwned::to_owned))
        .or_else(|| message_serialize_source_rust_type(field))
        .or_else(|| message_nested_shape_rust_type(field, message_type_name))
        .or(source_type)
        .or_else(|| {
            field
                .native_type
                .as_deref()
                .and_then(message_native_type_rust_type)
        })
        .or_else(|| report.rust_value_type.clone());
    report.rust_value_type = rust_type.clone();
    report.rust_field_type = rust_type.clone();
    report.blocked_reason =
        message_field_blocked_reason(field, report.wire_shape, rust_type.as_deref());
    report.supported = report.blocked_reason.is_none();
    report
}

fn state_field_shape_report(
    field: &NetworkField,
    wire_shapes: &BTreeMap<&str, SchemaWireShape>,
    wire_shape_sources: &BTreeMap<&str, &str>,
    container_shapes_by_vtable: &BTreeMap<&str, NetworkReplicatedContainerShape>,
    value_type_candidates_by_vtable: &BTreeMap<&str, Vec<NetworkNativeTypeInfoEvidence>>,
    serialize_types: &BTreeMap<Uuid, &NetworkSerializeType>,
) -> NetworkStateFieldShapeReport {
    let value_type_candidates = field_value_type_candidates(field, value_type_candidates_by_vtable);
    let normalized_rust_type = field
        .rust_type
        .as_deref()
        .map(normalize_generated_rust_type);
    let rust_type = normalized_rust_type
        .as_deref()
        .filter(|rust_type| syn::parse_str::<syn::Type>(rust_type).is_ok());
    let explicit_field_type =
        rust_type.filter(|rust_type| is_replicated_state_field_type(rust_type));
    let shape = if explicit_field_type.is_some() {
        field.wire_shape.or_else(|| {
            field
                .native_type
                .as_deref()
                .and_then(native_type_wire_shape)
        })
    } else {
        field_wire_shape(field, wire_shapes)
    };
    let source_type = serialize_field_scalar_source_type(field, shape);
    let rust_shape = shape.map(rust_field_shape);
    let container_shape = if explicit_field_type.is_none()
        && shape.is_none_or(|shape| shape.is_replicated_container())
    {
        replicated_container_shape_for_field(field, container_shapes_by_vtable)
    } else {
        None
    };
    let container_rust_shape = container_shape.as_ref().and_then(|shape| {
        replicated_container_semantic_field_shape(
            field,
            shape,
            &value_type_candidates,
            serialize_types,
        )
    });
    let generated_rust_field_type = explicit_field_type
        .map(ToOwned::to_owned)
        .or_else(|| {
            rust_type
                .filter(|_| shape.is_some_and(|shape| !shape.is_replicated_container()))
                .map(|rust_type| {
                    replicated_field_handler_type(
                        shape.expect("state value override has a wire shape"),
                        rust_type,
                    )
                })
        })
        .or_else(|| {
            source_type.as_deref().and_then(|source_type| {
                shape
                    .filter(|shape| !shape.is_replicated_container())
                    .map(|shape| replicated_field_handler_type(shape, source_type))
            })
        })
        .or_else(|| {
            container_rust_shape
                .as_ref()
                .map(|shape| shape.field_type.clone())
        })
        .or_else(|| rust_shape.as_ref().map(|shape| shape.field_type.clone()));
    let blocked_reason = state_field_blocked_reason(
        field,
        shape,
        normalized_rust_type.as_deref(),
        explicit_field_type,
        generated_rust_field_type.is_some(),
        !value_type_candidates.is_empty() || container_shape.is_some(),
    );
    NetworkStateFieldShapeReport {
        field_index: field.index,
        field_name: field.name.clone(),
        group: field.group,
        registration_kind: field.registration_kind.clone(),
        filter_group_attribute: field.filter_group_attribute,
        native_type: field.native_type.clone(),
        source_type_name: field.source_type_name.clone(),
        source_type_id: field
            .source_type_id
            .or_else(|| field.serialize.as_ref().map(|serialize| serialize.type_id)),
        serialize_type_name: field
            .serialize
            .as_ref()
            .map(|serialize| serialize.name.clone()),
        handler_vtable: field.handler_vtable.clone(),
        wire_shape: shape,
        wire_shape_source: if explicit_field_type.is_some() && shape.is_none() {
            None
        } else {
            field_wire_shape_source(field, wire_shapes, wire_shape_sources)
        },
        value_type_candidates,
        container_key_type_shape: if explicit_field_type.is_some() {
            None
        } else {
            container_rust_shape
                .as_ref()
                .and_then(|shape| shape.container_key_type_shape.clone())
        },
        container_embedded_key_type_shapes: if explicit_field_type.is_some() {
            Vec::new()
        } else {
            container_rust_shape
                .as_ref()
                .map(|shape| shape.container_embedded_key_type_shapes.clone())
                .unwrap_or_default()
        },
        container_value_type_shape: if explicit_field_type.is_some() {
            None
        } else {
            container_rust_shape
                .as_ref()
                .and_then(|shape| shape.container_value_type_shape.clone())
        },
        container_embedded_value_type_shapes: if explicit_field_type.is_some() {
            Vec::new()
        } else {
            container_rust_shape
                .as_ref()
                .map(|shape| shape.container_embedded_value_type_shapes.clone())
                .unwrap_or_default()
        },
        nested_type_shape: field.nested_type_shape.clone(),
        rust_value_type: if explicit_field_type.is_some() {
            None
        } else {
            rust_type
                .map(ToOwned::to_owned)
                .or_else(|| source_type.clone())
                .or_else(|| {
                    container_rust_shape
                        .as_ref()
                        .map(|shape| shape.value_type.clone())
                })
                .or_else(|| rust_shape.as_ref().map(|shape| shape.value_type.clone()))
        },
        rust_field_type: generated_rust_field_type,
        constructor_write_count: field.constructor_writes.len(),
        confidence: field.confidence,
        supported: blocked_reason.is_none(),
        blocked_reason,
    }
}

fn state_field_has_complete_shape(field: &NetworkStateFieldShapeReport) -> bool {
    field.wire_shape.is_some()
        || field
            .rust_field_type
            .as_deref()
            .is_some_and(is_replicated_state_field_type)
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
        .or_else(|| {
            field
                .source_type_name
                .as_deref()
                .and_then(source_type_name_wire_shape)
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
                .or_else(|| {
                    field
                        .source_type_name
                        .as_deref()
                        .and_then(source_type_name_wire_shape)
                        .map(|_| "source-type-name".to_owned())
                })
        })
    })
}

fn native_type_wire_shape(native_type: &str) -> Option<SchemaWireShape> {
    match native_type.trim() {
        "bool" => Some(SchemaWireShape::Bool),
        "u8" | "uint8_t" | "AZ::u8" | "i8" | "int8_t" | "AZ::s8" => Some(SchemaWireShape::U8),
        "u16" | "uint16_t" | "AZ::u16" | "i16" | "int16_t" | "AZ::s16" => {
            Some(SchemaWireShape::U16)
        }
        "u32"
        | "uint32_t"
        | "AZ::u32"
        | "i32"
        | "int32_t"
        | "AZ::s32"
        | "AZ::Crc32"
        | "FragmentKey"
        | "Amazon::Hub::FragmentKey" => Some(SchemaWireShape::U32),
        "u64"
        | "uint64_t"
        | "AZ::u64"
        | "i64"
        | "int64_t"
        | "AZ::s64"
        | "AZ::EntityId"
        | "TimePoint"
        | "MB::TimePoint"
        | "WallClockTimePoint"
        | "MB::WallClockTimePoint" => Some(SchemaWireShape::U64),
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
        "ActorRef" | "Amazon::Hub::ActorRef" | "HubAddress" | "ProxyAddress" => {
            Some(SchemaWireShape::ActorRef)
        }
        "EntityRef" => Some(SchemaWireShape::EntityRef),
        "AZStd::string" | "std::string" | "string" => Some(SchemaWireShape::String),
        _ => None,
    }
}

fn source_type_name_wire_shape(source_type_name: &str) -> Option<SchemaWireShape> {
    source_type_name
        .split(',')
        .map(str::trim)
        .filter(|part| !part.is_empty() && *part != "composite")
        .find_map(native_type_wire_shape)
}

fn message_native_type_rust_type(native_type: &str) -> Option<String> {
    if let Some(capacity) = fixed_vector_u8_capacity(native_type) {
        return Some(format!("::arrayvec::ArrayVec<u8, {capacity}>"));
    }
    if let Some(vector_type) = native_vector_rust_type(native_type) {
        return Some(vector_type);
    }
    if let Some(map_type) = native_map_rust_type(native_type) {
        return Some(map_type);
    }
    let rust_type = match native_type.trim() {
        "ActorRef" | "Amazon::Hub::ActorRef" | "HubAddress" | "ProxyAddress" => {
            "::nw_network::ActorRef"
        }
        "BaselineableFragment" | "Amazon::Hub::BaselineableFragment" => {
            "::nw_network::hub::BaselineableFragment"
        }
        "FragmentKey" | "Amazon::Hub::FragmentKey" => "::nw_network::hub::FragmentKey",
        "EntityRef" => "::nw_network::EntityRef",
        _ => return None,
    };
    Some(rust_type.to_owned())
}

fn native_vector_rust_type(native_type: &str) -> Option<String> {
    let inner = native_type
        .trim()
        .strip_prefix("AZStd::vector<")
        .or_else(|| native_type.trim().strip_prefix("vector<"))?
        .strip_suffix('>')?;
    let element = first_template_argument(inner)?;
    let element_type = native_vector_element_rust_type(element)?;
    Some(format!("::std::vec::Vec<{element_type}>"))
}

fn native_map_rust_type(native_type: &str) -> Option<String> {
    let inner = native_type
        .trim()
        .strip_prefix("AZStd::unordered_map<")
        .or_else(|| native_type.trim().strip_prefix("std::unordered_map<"))
        .or_else(|| native_type.trim().strip_prefix("unordered_map<"))?
        .strip_suffix('>')?;
    let (key, value) = first_two_template_arguments(inner)?;
    let key_type = native_collection_element_rust_type(key)?;
    let value_type = native_collection_element_rust_type(value)?;
    Some(format!(
        "::nw_network::serialize::IndexMap<{key_type}, {value_type}>"
    ))
}

fn native_vector_element_rust_type(native_type: &str) -> Option<String> {
    native_collection_element_rust_type(native_type)
}

fn native_collection_element_rust_type(native_type: &str) -> Option<String> {
    native_type_wire_shape(native_type)
        .map(|shape| rust_field_shape(shape).value_type)
        .or_else(|| message_native_type_rust_type(native_type))
        .or_else(|| serialize_source_rust_type_name(native_type.rsplit("::").next()?))
}

fn first_template_argument(value: &str) -> Option<&str> {
    let mut depth = 0usize;
    for (index, ch) in value.char_indices() {
        match ch {
            '<' => depth += 1,
            '>' => depth = depth.checked_sub(1)?,
            ',' if depth == 0 => return non_empty_trimmed(&value[..index]),
            _ => {}
        }
    }
    non_empty_trimmed(value)
}

fn first_two_template_arguments(value: &str) -> Option<(&str, &str)> {
    let mut depth = 0usize;
    for (index, ch) in value.char_indices() {
        match ch {
            '<' => depth += 1,
            '>' => depth = depth.checked_sub(1)?,
            ',' if depth == 0 => {
                let first = non_empty_trimmed(&value[..index])?;
                let rest = non_empty_trimmed(&value[index + 1..])?;
                return Some((first, first_template_argument(rest)?));
            }
            _ => {}
        }
    }
    None
}

fn non_empty_trimmed(value: &str) -> Option<&str> {
    let trimmed = value.trim();
    (!trimmed.is_empty()).then_some(trimmed)
}

fn fixed_vector_u8_capacity(native_type: &str) -> Option<usize> {
    let trimmed = native_type.trim();
    let inner = trimmed
        .strip_prefix("AZStd::fixed_vector<")?
        .strip_suffix('>')?;
    let (element, capacity) = inner.split_once(',')?;
    if element.trim() != "AZ::u8" && element.trim() != "u8" {
        return None;
    }
    capacity.trim().parse().ok()
}

fn message_serialize_source_rust_type(field: &NetworkField) -> Option<String> {
    let serialize = field.serialize.as_ref()?;
    runtime_semantic_container_member_type(&serialize.name)
        .map(ToOwned::to_owned)
        .or_else(|| serialize_source_rust_type_name(&serialize.name))
}

fn message_nested_shape_rust_type(
    field: &NetworkField,
    message_type_name: Option<&str>,
) -> Option<String> {
    let shape = field.nested_type_shape.as_ref()?;
    if !message_nested_shape_matches_field(field, shape) {
        return None;
    }
    if message_nested_shape_uses_source_type(shape) {
        return shape
            .type_name_full
            .as_deref()
            .or(shape.type_name.as_deref())
            .and_then(serialize_source_rust_type_name);
    }
    message_nested_shape_support_type_name(shape, message_type_name)
}

fn message_nested_shape_matches_field(
    field: &NetworkField,
    shape: &crate::network_schema::NetworkNestedTypeShape,
) -> bool {
    let Some(shape_name) = shape
        .type_name
        .as_deref()
        .or_else(|| shape.type_name_full.as_deref().map(type_name_leaf))
    else {
        return false;
    };
    field
        .source_type_name
        .as_deref()
        .or(field.native_type.as_deref())
        .and_then(first_source_type_leaf)
        .is_some_and(|source_name| source_name == shape_name)
}

fn first_source_type_leaf(value: &str) -> Option<&str> {
    value
        .split(',')
        .map(str::trim)
        .find(|part| !part.is_empty())
        .map(type_name_leaf)
}

fn message_nested_shape_uses_source_type(
    shape: &crate::network_schema::NetworkNestedTypeShape,
) -> bool {
    shape.member_names_proven == Some(true)
        && shape
            .member_name_source
            .as_deref()
            .is_some_and(|source| source.contains("serialize") || source == "ghidra-datatype")
        && !shape
            .validation
            .as_deref()
            .is_some_and(|validation| validation.contains("native-rtti"))
}

fn message_nested_shape_support_type_name(
    shape: &crate::network_schema::NetworkNestedTypeShape,
    message_type_name: Option<&str>,
) -> Option<String> {
    let mut name = rust_type_ident(shape.type_name.as_deref()?);
    if message_type_name
        .map(rust_type_ident)
        .is_some_and(|message_name| message_name == name)
    {
        name.push_str("Body");
    }
    syn::parse_str::<syn::Type>(&name).ok()?;
    Some(name)
}

fn resolved_field_descriptor_rust_type(field: &NetworkField) -> Option<String> {
    field
        .rust_type
        .as_deref()
        .map(normalize_generated_rust_type)
        .or_else(|| {
            existing_message_support_type(field, &MessageSupportEvidence::default())
                .map(ToOwned::to_owned)
        })
        .or_else(|| message_serialize_source_rust_type(field))
}

fn normalize_generated_rust_type(rust_type: &str) -> String {
    rust_type
        .replace(
            "::std::collections::HashMap<",
            "::nw_network::serialize::IndexMap<",
        )
        .replace(
            "std::collections::HashMap<",
            "::nw_network::serialize::IndexMap<",
        )
}

#[derive(Debug, Default)]
struct MessageSupportEvidence {
    actor_request_id_payload_targets: BTreeSet<String>,
}

fn message_support_evidence(schema: &NetworkSchema) -> MessageSupportEvidence {
    let mut evidence = MessageSupportEvidence::default();
    for field in schema
        .types
        .iter()
        .flat_map(|network_type| &network_type.fields)
    {
        let Some(target_name) = field
            .unmarshal_evidence
            .as_ref()
            .and_then(|unmarshal| unmarshal.target_name.as_deref())
        else {
            continue;
        };
        if field.native_type.as_deref() == Some("composite")
            && field.source_type_name.as_deref().is_some_and(|source| {
                let parts = source
                    .split(',')
                    .map(str::trim)
                    .filter(|part| !part.is_empty())
                    .collect::<Vec<_>>();
                parts.first() == Some(&"ActorRequestIdPayload")
                    && parts.last() == Some(&"ActorRequestId")
            })
            && target_name.ends_with("ActorRequestIdPayload::Unmarshal")
            && has_actor_request_id_shape(field)
        {
            evidence
                .actor_request_id_payload_targets
                .insert(target_name.to_owned());
        }
    }
    evidence
}

fn existing_message_support_type(
    field: &NetworkField,
    support_evidence: &MessageSupportEvidence,
) -> Option<&'static str> {
    if field_native_or_source_type_is(field, "ClientVersionTokenMap")
        || field_native_or_source_type_is(field, "Amazon::Configuration::ClientVersionTokenMap")
    {
        return Some("::nw_network::ClientVersionTokenMap");
    }
    if field_native_or_source_type_is(field, "LoginToken")
        || field_native_or_source_type_is(field, "Amazon::REP::LoginToken")
    {
        return Some("::nw_network::LoginToken");
    }
    if field_native_or_source_type_is(field, "AuthToken")
        || field_native_or_source_type_is(field, "Amazon::REP::AuthToken")
    {
        return Some("::nw_network::AuthToken");
    }
    if field_native_or_source_type_is(field, "ImpersonatedValues")
        || field_native_or_source_type_is(field, "Amazon::REP::ImpersonatedValues")
    {
        return Some("::nw_network::ImpersonatedValues");
    }

    if is_actor_request_id_field(field)
        || is_proven_actor_request_id_payload_target(field, support_evidence)
    {
        return Some("::nw_network::ActorRequestId");
    }
    if is_actor_ref_composite_field(field) {
        return Some("::nw_network::ActorRef");
    }

    None
}

fn field_native_or_source_type_is(field: &NetworkField, expected: &str) -> bool {
    [
        field.native_type.as_deref(),
        field.source_type_name.as_deref(),
    ]
    .into_iter()
    .flatten()
    .any(|value| value.trim() == expected)
}

fn is_proven_actor_request_id_payload_target(
    field: &NetworkField,
    support_evidence: &MessageSupportEvidence,
) -> bool {
    if !field_native_or_source_type_is(field, "ActorRequestIdPayload") {
        return false;
    }
    let Some(evidence) = field.unmarshal_evidence.as_ref() else {
        return false;
    };
    if !evidence
        .target_kind
        .as_deref()
        .is_none_or(|kind| kind == "direct-unmarshal" || kind.contains("direct-type"))
    {
        return false;
    }
    let Some(target_name) = evidence.target_name.as_deref() else {
        return false;
    };
    target_name.ends_with("ActorRequestIdPayload::Unmarshal")
        && support_evidence
            .actor_request_id_payload_targets
            .contains(target_name)
}

fn is_actor_request_id_field(field: &NetworkField) -> bool {
    let native_type = field.native_type.as_deref();
    let source_type = field.source_type_name.as_deref();
    let direct_type = native_type == Some("ActorRequestId")
        && source_type.is_none_or(|source| source == "ActorRequestId");
    let direct_payload_type = field_native_or_source_type_is(field, "ActorRequestIdPayload")
        && has_actor_request_id_payload_shape(field);
    let composite_type = native_type == Some("composite")
        && composite_support_tail(source_type) == Some("ActorRequestId");
    if !direct_type && !direct_payload_type && !composite_type {
        return false;
    }

    let Some(target_name) = field
        .unmarshal_evidence
        .as_ref()
        .and_then(|evidence| evidence.target_name.as_deref())
    else {
        return false;
    };
    let target_kind = field
        .unmarshal_evidence
        .as_ref()
        .and_then(|evidence| evidence.target_kind.as_deref());
    let target_matches = target_name.ends_with("ActorRequestId::Unmarshal")
        || (direct_payload_type && target_name.ends_with("ActorRequestIdPayload::Unmarshal"))
        || (composite_type
            && field
                .nested_type_shape
                .as_ref()
                .and_then(|shape| shape.function_name.as_deref())
                .is_some_and(|function| function.ends_with("ActorRequestId::Unmarshal")));

    target_matches
        && target_kind.is_none_or(|kind| kind == "direct-unmarshal" || kind.contains("direct-type"))
        && (has_actor_request_id_shape(field)
            || (direct_payload_type && has_actor_request_id_payload_shape(field)))
}

fn composite_support_tail(source_type: Option<&str>) -> Option<&str> {
    source_type?
        .split(',')
        .map(str::trim)
        .rfind(|part| !part.is_empty())
}

fn has_actor_request_id_shape(field: &NetworkField) -> bool {
    let Some(shape) = field.nested_type_shape.as_ref() else {
        return false;
    };
    shape.type_name.as_deref() == Some("ActorRequestId")
        && shape
            .type_name_full
            .as_deref()
            .is_some_and(|name| name.ends_with("::ActorRequestId"))
        && shape.type_name_source.as_deref() == Some("ghidra-symbol")
        && has_two_u64_member_shape(field)
}

fn has_actor_request_id_payload_shape(field: &NetworkField) -> bool {
    let Some(shape) = field.nested_type_shape.as_ref() else {
        return false;
    };
    shape.type_name.as_deref() == Some("ActorRequestIdPayload")
        && shape
            .type_name_full
            .as_deref()
            .is_some_and(|name| name.ends_with("::ActorRequestIdPayload"))
        && shape.type_name_source.as_deref() == Some("ghidra-symbol")
        && has_two_u64_member_shape(field)
}

fn has_two_u64_member_shape(field: &NetworkField) -> bool {
    let Some(shape) = field.nested_type_shape.as_ref() else {
        return false;
    };
    matches!(
        shape.validation.as_deref(),
        Some("layout-consistent-two-u64" | "layout-consistent-direct-type")
    ) && shape.member_names_proven.is_some()
        && shape.members.len() == 2
        && actor_request_id_member(&shape.members[0], 0, "0x0")
        && actor_request_id_member(&shape.members[1], 1, "0x8")
}

fn is_actor_ref_composite_field(field: &NetworkField) -> bool {
    field.native_type.as_deref() == Some("composite")
        && field.source_type_name.as_deref() == Some("ProxyAddress,ActorRef")
        && has_proxy_address_actor_ref_shape(field)
}

fn has_proxy_address_actor_ref_shape(field: &NetworkField) -> bool {
    let Some(shape) = field.nested_type_shape.as_ref() else {
        return false;
    };
    shape.type_name.as_deref() == Some("ProxyAddress")
        && shape
            .type_name_full
            .as_deref()
            .is_some_and(|name| name.ends_with("::ProxyAddress"))
        && shape.type_name_source.as_deref() == Some("ghidra-symbol")
        && shape.validation.as_deref() == Some("layout-consistent-direct-type")
        && shape.members.len() == 3
        && actor_ref_member(&shape.members[0], 0, "0x0", "u32", 4)
        && actor_ref_member(&shape.members[1], 1, "0x4", "fixed-bytes-16", 16)
        && actor_ref_member(&shape.members[2], 2, "0x14", "fixed-bytes-16", 16)
}

fn actor_ref_member(
    member: &crate::network_schema::NetworkNestedTypeMember,
    index: u32,
    offset: &str,
    wire_shape: &str,
    byte_width: u32,
) -> bool {
    member.index == Some(index)
        && member.offset.as_deref() == Some(offset)
        && member.wire_shape.as_deref() == Some(wire_shape)
        && member.byte_width == Some(byte_width)
}

fn actor_request_id_member(
    member: &crate::network_schema::NetworkNestedTypeMember,
    index: u32,
    offset: &str,
) -> bool {
    member.index == Some(index)
        && member.offset.as_deref() == Some(offset)
        && member.native_type.as_deref() == Some("u64")
        && member.wire_shape.as_deref() == Some("u64")
        && member.byte_width == Some(8)
        && member.name_proven.is_some()
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

fn value_type_candidates_by_handler_vtable(
    schema: &NetworkSchema,
) -> BTreeMap<&str, Vec<NetworkNativeTypeInfoEvidence>> {
    schema
        .field_handler_vtables
        .iter()
        .filter_map(|vtable| {
            let address = vtable.address.as_deref()?;
            let mut candidates = Vec::new();
            if vtable.value_type_name.is_some()
                || vtable.value_type_id.is_some()
                || vtable.value_type_info_address.is_some()
            {
                candidates.push(NetworkNativeTypeInfoEvidence {
                    address: vtable.value_type_info_address.clone(),
                    name: vtable.value_type_name.clone(),
                    type_id: vtable
                        .value_type_id
                        .as_deref()
                        .and_then(|type_id| Uuid::parse_str(type_id.trim_matches(['{', '}'])).ok()),
                    source: Some("selected-value-type-info".to_owned()),
                    name_source: Some("selected-value-type-info".to_owned()),
                });
            }
            for candidate in &vtable.value_type_candidates {
                push_unique_value_type_candidate(&mut candidates, candidate.clone());
            }
            (!candidates.is_empty()).then_some((address, candidates))
        })
        .collect()
}

fn field_value_type_candidates(
    field: &NetworkField,
    candidates_by_vtable: &BTreeMap<&str, Vec<NetworkNativeTypeInfoEvidence>>,
) -> Vec<NetworkNativeTypeInfoEvidence> {
    field
        .handler_vtable
        .as_deref()
        .and_then(|handler_vtable| candidates_by_vtable.get(handler_vtable))
        .cloned()
        .unwrap_or_default()
}

fn push_unique_value_type_candidate(
    candidates: &mut Vec<NetworkNativeTypeInfoEvidence>,
    candidate: NetworkNativeTypeInfoEvidence,
) {
    let duplicate = candidates.iter().any(|existing| {
        existing
            .type_id
            .as_ref()
            .zip(candidate.type_id.as_ref())
            .is_some_and(|(lhs, rhs)| lhs == rhs)
            || (existing.type_id.is_none()
                && candidate.type_id.is_none()
                && existing.address == candidate.address
                && existing.name == candidate.name)
    });
    if !duplicate {
        candidates.push(candidate);
    }
}

#[derive(Debug, Clone, Copy, Default)]
struct FieldBlockCounts {
    field_count: usize,
    missing_wire_shape_count: usize,
    missing_field_type_count: usize,
    missing_support_type_count: usize,
    missing_composite_support_type_count: usize,
    unsupported_wire_shape_count: usize,
    container_codec_only_count: usize,
    missing_semantic_type_count: usize,
    invalid_field_metadata_count: usize,
    low_confidence_field_count: usize,
}

fn state_blocked_reasons(network_type: &NetworkType, counts: FieldBlockCounts) -> Vec<String> {
    let mut reasons = Vec::new();
    if network_type.type_index.is_none() {
        reasons.push("missing-type-index".to_owned());
    }
    if network_type.name.is_none() {
        reasons.push("missing-type-name".to_owned());
    }
    if counts.field_count == 0 {
        reasons.push("no-registered-fields".to_owned());
    }
    if counts.missing_wire_shape_count != 0 {
        reasons.push(format!(
            "missing-wire-shape:{}",
            counts.missing_wire_shape_count
        ));
    }
    if counts.unsupported_wire_shape_count != 0 {
        reasons.push(format!(
            "unsupported-wire-shape:{}",
            counts.unsupported_wire_shape_count
        ));
    }
    if counts.container_codec_only_count != 0 {
        reasons.push(format!(
            "container-codec-only:{}",
            counts.container_codec_only_count
        ));
    }
    if counts.missing_semantic_type_count != 0 {
        reasons.push(format!(
            "missing-semantic-type:{}",
            counts.missing_semantic_type_count
        ));
    }
    if counts.invalid_field_metadata_count != 0 {
        reasons.push(format!(
            "invalid-field-metadata:{}",
            counts.invalid_field_metadata_count
        ));
    }
    if counts.low_confidence_field_count != 0 {
        reasons.push(format!(
            "low-confidence-field:{}",
            counts.low_confidence_field_count
        ));
    }
    reasons
}

fn message_blocked_reasons(network_type: &NetworkType, counts: FieldBlockCounts) -> Vec<String> {
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
    if counts.missing_field_type_count != 0 {
        reasons.push(format!(
            "missing-field-type:{}",
            counts.missing_field_type_count
        ));
    }
    if counts.missing_support_type_count != 0 {
        reasons.push(format!(
            "missing-support-type:{}",
            counts.missing_support_type_count
        ));
    }
    if counts.missing_composite_support_type_count != 0 {
        reasons.push(format!(
            "missing-composite-support-type:{}",
            counts.missing_composite_support_type_count
        ));
    }
    if counts.unsupported_wire_shape_count != 0 {
        reasons.push(format!(
            "unsupported-wire-shape:{}",
            counts.unsupported_wire_shape_count
        ));
    }
    if counts.container_codec_only_count != 0 {
        reasons.push(format!(
            "container-codec-only:{}",
            counts.container_codec_only_count
        ));
    }
    if counts.missing_semantic_type_count != 0 {
        reasons.push(format!(
            "missing-semantic-type:{}",
            counts.missing_semantic_type_count
        ));
    }
    if counts.invalid_field_metadata_count != 0 {
        reasons.push(format!(
            "invalid-field-metadata:{}",
            counts.invalid_field_metadata_count
        ));
    }
    if counts.low_confidence_field_count != 0 {
        reasons.push(format!(
            "low-confidence-field:{}",
            counts.low_confidence_field_count
        ));
    }
    reasons
}

fn state_field_blocked_reason(
    field: &NetworkField,
    shape: Option<SchemaWireShape>,
    rust_type: Option<&str>,
    explicit_field_type: Option<&str>,
    has_generated_field_type: bool,
    has_value_type_evidence: bool,
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
        && syn::parse_str::<syn::Type>(rust_type).is_err()
    {
        return Some("invalid-rust-field-type".to_owned());
    }
    if shape.is_none() && explicit_field_type.is_none() && !has_generated_field_type {
        if has_value_type_evidence {
            return Some("missing-semantic-type".to_owned());
        }
        return Some("missing-wire-shape".to_owned());
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
        if has_composite_support_type_evidence(field) {
            return Some("missing-composite-support-type".to_owned());
        }
        if has_support_type_evidence(field) {
            return Some("missing-support-type".to_owned());
        }
        return Some("missing-field-type".to_owned());
    }
    if shape.is_some_and(SchemaWireShape::is_replicated_container) {
        return Some("missing-semantic-type".to_owned());
    }
    None
}

fn has_composite_support_type_evidence(field: &NetworkField) -> bool {
    field.native_type.as_deref() == Some("composite")
        || field
            .source_type_name
            .as_deref()
            .is_some_and(|source_type| source_type.contains(','))
}

fn has_support_type_evidence(field: &NetworkField) -> bool {
    field.serialize.is_some()
        || field
            .source_type_name
            .as_deref()
            .is_some_and(is_named_support_type_evidence)
        || field
            .native_type
            .as_deref()
            .is_some_and(is_named_support_type_evidence)
}

fn is_named_support_type_evidence(value: &str) -> bool {
    let value = value.trim();
    !value.is_empty() && value != "unknown" && value != "composite"
}

fn is_placeholder_field_name(value: &str) -> bool {
    value
        .strip_prefix("field_")
        .is_some_and(|suffix| !suffix.is_empty() && suffix.chars().all(|c| c.is_ascii_digit()))
}

fn is_placeholder_report_field_name(field: &NetworkStateFieldShapeReport) -> bool {
    field
        .field_name
        .as_deref()
        .is_some_and(|name| is_placeholder_field_name(name) || is_native_type_field_name(name))
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
    container_key_type_shape: Option<crate::network_schema::NetworkNestedTypeShape>,
    container_embedded_key_type_shapes: Vec<crate::network_schema::NetworkNestedTypeShape>,
    container_value_type_shape: Option<crate::network_schema::NetworkNestedTypeShape>,
    container_embedded_value_type_shapes: Vec<crate::network_schema::NetworkNestedTypeShape>,
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
        SchemaWireShape::PositionAnchor => rust_field_shape_static(
            "(f32, f32, f32)",
            "ReplicatedFieldHandler<(f32, f32, f32), PositionAnchorMarshaler>",
        ),
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
        SchemaWireShape::String => {
            rust_field_shape_static("String", "ReplicatedFieldHandler<String>")
        }
        SchemaWireShape::ReplicatedContainer(container) => {
            replicated_container_field_shape(container)
        }
    }
}

fn rust_field_shape_static(value_type: &'static str, field_type: &'static str) -> RustFieldShape {
    RustFieldShape {
        value_type: value_type.to_owned(),
        field_type: field_type.to_owned(),
        container_key_type_shape: None,
        container_embedded_key_type_shapes: Vec::new(),
        container_value_type_shape: None,
        container_embedded_value_type_shapes: Vec::new(),
    }
}

fn replicated_container_field_shape(
    container: NetworkReplicatedContainerWireShape,
) -> RustFieldShape {
    let key_type = scalar_rust_type(container.key);
    let value_type = scalar_rust_type(container.value);
    let collection_type = keyed_replicated_container_type(&key_type, &value_type);
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

fn replicated_container_shape_for_field<'a>(
    field: &NetworkField,
    container_shapes: &'a BTreeMap<&str, NetworkReplicatedContainerShape>,
) -> Option<&'a NetworkReplicatedContainerShape> {
    field
        .handler_vtable
        .as_deref()
        .and_then(|handler_vtable| container_shapes.get(handler_vtable))
}

fn replicated_container_semantic_field_shape(
    field: &NetworkField,
    container: &NetworkReplicatedContainerShape,
    value_type_candidates: &[NetworkNativeTypeInfoEvidence],
    serialize_types: &BTreeMap<Uuid, &NetworkSerializeType>,
) -> Option<RustFieldShape> {
    let value = container_value_type(field, container, value_type_candidates, serialize_types)?;
    let key = match container.storage {
        NetworkReplicatedContainerStorageKind::Map => {
            Some(container_key_type(field, container, serialize_types)?)
        }
        NetworkReplicatedContainerStorageKind::Vec => None,
    };
    let collection_type = match container.storage {
        NetworkReplicatedContainerStorageKind::Map => {
            let key_type = &key.as_ref()?.rust_type;
            keyed_replicated_container_type(key_type, &value.rust_type)
        }
        NetworkReplicatedContainerStorageKind::Vec => {
            format!("::std::vec::Vec<{}>", value.rust_type)
        }
    };
    let key_marshaler = match container.storage {
        NetworkReplicatedContainerStorageKind::Map => key.as_ref()?.marshaler_type.clone(),
        NetworkReplicatedContainerStorageKind::Vec => {
            "::nw_network::serialize::DefaultMarshaler<::nw_network::serialize::VlqU64>".to_owned()
        }
    };
    let value_marshaler = value.marshaler_type;
    let field_type = format!(
        "::nw_network::serialize::ReplicatedContainer<{collection_type}, {{ ::nw_network::serialize::WIRE_VEC_CAP }}, {key_marshaler}, {value_marshaler}>"
    );
    Some(RustFieldShape {
        value_type: collection_type,
        field_type,
        container_key_type_shape: key.as_ref().and_then(|key| key.value_type_shape.clone()),
        container_embedded_key_type_shapes: key
            .as_ref()
            .map(|key| key.embedded_value_type_shapes.clone())
            .unwrap_or_default(),
        container_value_type_shape: value.value_type_shape,
        container_embedded_value_type_shapes: value.embedded_value_type_shapes,
    })
}

fn container_key_type(
    field: &NetworkField,
    container: &NetworkReplicatedContainerShape,
    serialize_types: &BTreeMap<Uuid, &NetworkSerializeType>,
) -> Option<ContainerValueType> {
    if is_uuid_native_type(container.key_native_type.as_deref()) {
        return Some(ContainerValueType {
            rust_type: "::uuid::Uuid".to_owned(),
            marshaler_type: "::nw_network::serialize::DefaultMarshaler<::uuid::Uuid>".to_owned(),
            value_type_shape: None,
            embedded_value_type_shapes: Vec::new(),
        });
    }
    let key_wire_shapes = container_key_wire_shapes(container);
    if let Some(type_name) = container.key_native_type.as_deref()
        && let Some(value) =
            container_named_source_type(type_name, &key_wire_shapes, serialize_types)
    {
        return Some(value);
    }
    if let Some(type_name) = container.key_type_name.as_deref()
        && let Some(value) =
            container_named_source_type(type_name, &key_wire_shapes, serialize_types)
    {
        return Some(value);
    }
    if let Some(shape) = container.key_type_shape.as_ref()
        && container_value_shape_matches(shape, &key_wire_shapes)
    {
        let rust_type = container_value_shape_rust_type(field, shape, serialize_types)?;
        let marshaler_type = container_value_shape_codec_name(field, shape)?;
        return Some(ContainerValueType {
            rust_type,
            marshaler_type,
            value_type_shape: Some(shape.clone()),
            embedded_value_type_shapes: Vec::new(),
        });
    }
    let [shape] = key_wire_shapes.as_slice() else {
        return None;
    };
    let rust_type = scalar_rust_type(*shape);
    let marshaler_type = scalar_marshaler_type(*shape);
    Some(ContainerValueType {
        rust_type,
        marshaler_type,
        value_type_shape: None,
        embedded_value_type_shapes: Vec::new(),
    })
}

fn container_key_wire_shapes(
    container: &NetworkReplicatedContainerShape,
) -> Vec<SchemaWireScalarShape> {
    if container.key_wire_shapes.is_empty() {
        vec![container.key_wire_shape]
    } else {
        container.key_wire_shapes.clone()
    }
}

fn is_uuid_native_type(native_type: Option<&str>) -> bool {
    matches!(
        native_type.map(str::trim),
        Some("AZ::Uuid" | "Uuid" | "uuid::Uuid" | "::uuid::Uuid")
    )
}

#[derive(Debug, Clone)]
struct ContainerValueType {
    rust_type: String,
    marshaler_type: String,
    value_type_shape: Option<crate::network_schema::NetworkNestedTypeShape>,
    embedded_value_type_shapes: Vec<crate::network_schema::NetworkNestedTypeShape>,
}

fn container_value_type(
    field: &NetworkField,
    container: &NetworkReplicatedContainerShape,
    value_type_candidates: &[NetworkNativeTypeInfoEvidence],
    serialize_types: &BTreeMap<Uuid, &NetworkSerializeType>,
) -> Option<ContainerValueType> {
    if let Some(serialize) = field.serialize.as_ref()
        && container_value_matches_serialize(container, serialize)
    {
        return serialize_container_value_type(
            serialize.kind,
            &serialize.name,
            &container.value_wire_shapes,
        );
    }

    if let Some(serialize) =
        unique_container_value_candidate(container, value_type_candidates, serialize_types)
    {
        return serialize_container_value_type(
            serialize.kind,
            &serialize.name,
            &container.value_wire_shapes,
        );
    }

    if let Some(type_name) = container.value_type_name.as_deref()
        && let Some(value) =
            container_named_source_type(type_name, &container.value_wire_shapes, serialize_types)
    {
        return Some(value);
    }

    if let Some(value) = vector_container_value_type(container, serialize_types) {
        return Some(value);
    }

    if let Some(shape) = container.value_type_shape.as_ref()
        && container_value_shape_matches_with_embedded(
            shape,
            &container.value_wire_shapes,
            &container.embedded_value_type_shapes,
        )
        && container_value_shape_members_are_emittable(shape, &container.embedded_value_type_shapes)
    {
        let rust_type = container_value_shape_rust_type(field, shape, serialize_types)?;
        let marshaler_type = container_value_shape_codec_name(field, shape)?;
        return Some(ContainerValueType {
            rust_type,
            marshaler_type,
            value_type_shape: Some(shape.clone()),
            embedded_value_type_shapes: container.embedded_value_type_shapes.clone(),
        });
    }

    let [shape] = container.value_wire_shapes.as_slice() else {
        return None;
    };
    let rust_type = scalar_rust_type(*shape);
    let marshaler_type = scalar_marshaler_type(*shape);
    Some(ContainerValueType {
        rust_type,
        marshaler_type,
        value_type_shape: None,
        embedded_value_type_shapes: Vec::new(),
    })
}

fn container_value_shape_matches(
    shape: &crate::network_schema::NetworkNestedTypeShape,
    value_wire_shapes: &[SchemaWireScalarShape],
) -> bool {
    container_value_shape_matches_with_embedded(shape, value_wire_shapes, &[])
}

fn container_value_shape_matches_with_embedded(
    shape: &crate::network_schema::NetworkNestedTypeShape,
    value_wire_shapes: &[SchemaWireScalarShape],
    embedded_shapes: &[crate::network_schema::NetworkNestedTypeShape],
) -> bool {
    if shape.members.is_empty() || value_wire_shapes.is_empty() {
        return false;
    }
    let mut index = 0;
    for member in &shape.members {
        let Some(wire_shape) = member.wire_shape.as_deref() else {
            return false;
        };
        let Some(span) = container_value_member_shape_span(
            wire_shape,
            value_wire_shapes,
            index,
            embedded_shapes,
        ) else {
            return false;
        };
        index += span;
    }
    index == value_wire_shapes.len()
}

fn scalar_shape_name_matches(value: &str, expected: SchemaWireScalarShape) -> bool {
    wire_scalar_shape_from_name(value)
        .is_some_and(|observed| scalar_shapes_match(observed, expected))
}

fn scalar_shapes_match(observed: SchemaWireScalarShape, expected: SchemaWireScalarShape) -> bool {
    observed == expected
        || matches!(
            (observed, expected),
            (SchemaWireScalarShape::Bool, SchemaWireScalarShape::U8)
        )
}

fn container_value_member_shape_span(
    observed: &str,
    expected: &[SchemaWireScalarShape],
    index: usize,
    embedded_shapes: &[crate::network_schema::NetworkNestedTypeShape],
) -> Option<usize> {
    let next = *expected.get(index)?;
    if scalar_shape_name_matches(observed, next) {
        return Some(1);
    }
    if let Some(composite) = composite_member_wire_shapes(observed) {
        let end = index.checked_add(composite.len())?;
        let expected = expected.get(index..end)?;
        return composite
            .iter()
            .zip(expected.iter())
            .all(|(observed, expected)| scalar_shapes_match(*observed, *expected))
            .then_some(composite.len());
    }
    if let Some(embedded) = nested_shape_by_wire_name(observed, embedded_shapes) {
        let embedded_shapes = nested_shape_wire_shapes(embedded, embedded_shapes)?;
        let span = embedded_shapes.len();
        let expected = expected.get(index..index.checked_add(span)?)?;
        return embedded_shapes
            .iter()
            .zip(expected.iter())
            .all(|(observed, expected)| scalar_shapes_match(*observed, *expected))
            .then_some(span);
    }
    match observed {
        "vec2" if expected_shape_run(expected, index, SchemaWireScalarShape::F32, 2) => Some(2),
        "vec3" if expected_shape_run(expected, index, SchemaWireScalarShape::F32, 3) => Some(3),
        "vec4" | "quat" if expected_shape_run(expected, index, SchemaWireScalarShape::F32, 4) => {
            Some(4)
        }
        observed => {
            let element = vector_element_wire_shape(observed)?;
            if next != SchemaWireScalarShape::VlqU32 {
                return None;
            }
            if let Some(embedded) = nested_shape_by_wire_name(element, embedded_shapes) {
                let element_shapes = nested_shape_wire_shapes(embedded, embedded_shapes)?;
                let span = 1usize.checked_add(element_shapes.len())?;
                let expected = expected.get(index + 1..index + span)?;
                return element_shapes
                    .iter()
                    .zip(expected.iter())
                    .all(|(observed, expected)| scalar_shapes_match(*observed, *expected))
                    .then_some(span);
            }
            if expected
                .get(index + 1)
                .is_some_and(|shape| scalar_shape_name_matches(element, *shape))
            {
                Some(2)
            } else {
                Some(1)
            }
        }
    }
}

fn nested_shape_by_wire_name<'a>(
    name: &str,
    shapes: &'a [crate::network_schema::NetworkNestedTypeShape],
) -> Option<&'a crate::network_schema::NetworkNestedTypeShape> {
    shapes.iter().find(|shape| {
        [shape.type_name.as_deref(), shape.type_name_full.as_deref()]
            .into_iter()
            .flatten()
            .any(|candidate| type_name_leaf(candidate) == name)
    })
}

fn type_name_leaf(name: &str) -> &str {
    name.rsplit("::").next().unwrap_or(name).trim()
}

fn nested_shape_wire_shapes(
    shape: &crate::network_schema::NetworkNestedTypeShape,
    embedded_shapes: &[crate::network_schema::NetworkNestedTypeShape],
) -> Option<Vec<SchemaWireScalarShape>> {
    if shape.members.is_empty() {
        return None;
    }

    let mut shapes = Vec::new();
    for member in &shape.members {
        let wire_shape = member.wire_shape.as_deref()?;
        shapes.extend(nested_member_wire_shapes(wire_shape, embedded_shapes)?);
    }
    Some(shapes)
}

fn nested_member_wire_shapes(
    observed: &str,
    embedded_shapes: &[crate::network_schema::NetworkNestedTypeShape],
) -> Option<Vec<SchemaWireScalarShape>> {
    if let Some(shape) = wire_scalar_shape_from_name(observed) {
        return Some(vec![shape]);
    }
    if let Some(composite) = composite_member_wire_shapes(observed) {
        return Some(composite);
    }
    if let Some(embedded) = nested_shape_by_wire_name(observed, embedded_shapes) {
        return nested_shape_wire_shapes(embedded, embedded_shapes);
    }
    match observed {
        "vec2" => Some(vec![SchemaWireScalarShape::F32; 2]),
        "vec3" => Some(vec![SchemaWireScalarShape::F32; 3]),
        "vec4" | "quat" => Some(vec![SchemaWireScalarShape::F32; 4]),
        observed => {
            let element = vector_element_wire_shape(observed)?;
            let embedded = nested_shape_by_wire_name(element, embedded_shapes)?;
            let mut shapes = vec![SchemaWireScalarShape::VlqU32];
            shapes.extend(nested_shape_wire_shapes(embedded, embedded_shapes)?);
            Some(shapes)
        }
    }
}

fn expected_shape_run(
    shapes: &[SchemaWireScalarShape],
    start: usize,
    shape: SchemaWireScalarShape,
    count: usize,
) -> bool {
    shapes
        .get(start..start + count)
        .is_some_and(|slice| slice.iter().all(|candidate| *candidate == shape))
}

fn vector_element_wire_shape(value: &str) -> Option<&str> {
    value
        .strip_prefix("vec<")?
        .strip_suffix('>')
        .map(str::trim)
        .filter(|value| !value.is_empty())
}

fn composite_member_wire_shapes(value: &str) -> Option<Vec<SchemaWireScalarShape>> {
    value
        .strip_prefix("composite<")?
        .strip_suffix('>')?
        .split(',')
        .map(str::trim)
        .map(wire_scalar_shape_from_name)
        .collect()
}

fn wire_scalar_shape_from_name(value: &str) -> Option<SchemaWireScalarShape> {
    match value {
        "bool" => Some(SchemaWireScalarShape::Bool),
        "u8" => Some(SchemaWireScalarShape::U8),
        "u16" => Some(SchemaWireScalarShape::U16),
        "u32" => Some(SchemaWireScalarShape::U32),
        "u64" => Some(SchemaWireScalarShape::U64),
        "f32" => Some(SchemaWireScalarShape::F32),
        "f64" => Some(SchemaWireScalarShape::F64),
        "half-f32" => Some(SchemaWireScalarShape::HalfF32),
        "vlq-u32" => Some(SchemaWireScalarShape::VlqU32),
        "vlq-u64" => Some(SchemaWireScalarShape::VlqU64),
        "sequence-number" => Some(SchemaWireScalarShape::SequenceNumber),
        "vec2" => Some(SchemaWireScalarShape::Vec2),
        "vec3" => Some(SchemaWireScalarShape::Vec3),
        "vec4" => Some(SchemaWireScalarShape::Vec4),
        "quat" => Some(SchemaWireScalarShape::Quat),
        "quat-comp-norm" => Some(SchemaWireScalarShape::QuatCompNorm),
        "vec2-comp" => Some(SchemaWireScalarShape::Vec2Comp),
        "vec3-comp" => Some(SchemaWireScalarShape::Vec3Comp),
        "vec3-comp-norm" => Some(SchemaWireScalarShape::Vec3CompNorm),
        "quat-comp" => Some(SchemaWireScalarShape::QuatComp),
        "quat-smallest-three" => Some(SchemaWireScalarShape::QuatSmallestThree),
        "non-uniform-scale-comp" => Some(SchemaWireScalarShape::NonUniformScaleComp),
        "position-anchor" => Some(SchemaWireScalarShape::PositionAnchor),
        "transform-compressor" => Some(SchemaWireScalarShape::TransformCompressor),
        "packed-size" => Some(SchemaWireScalarShape::PackedSize),
        "mat3" => Some(SchemaWireScalarShape::Mat3),
        "affine3" => Some(SchemaWireScalarShape::Affine3),
        "aabb2d" => Some(SchemaWireScalarShape::Aabb2d),
        "aabb3d" => Some(SchemaWireScalarShape::Aabb3d),
        "actor-ref" => Some(SchemaWireScalarShape::ActorRef),
        "entity-ref" => Some(SchemaWireScalarShape::EntityRef),
        "string" => Some(SchemaWireScalarShape::String),
        value => value
            .strip_prefix("fixed-bytes-")
            .and_then(|len| len.parse::<u16>().ok())
            .map(SchemaWireScalarShape::FixedBytes),
    }
}

fn container_value_shape_codec_name(
    field: &NetworkField,
    shape: &crate::network_schema::NetworkNestedTypeShape,
) -> Option<String> {
    let field_name = field.name.as_deref()?;
    let type_name = shape.type_name.as_deref()?;
    let field_name = rust_field_ident(field_name);
    Some(format!(
        "{}{}Marshaler",
        rust_type_ident(&field_name),
        rust_type_ident(type_name)
    ))
}

fn container_value_shape_rust_type(
    field: &NetworkField,
    shape: &crate::network_schema::NetworkNestedTypeShape,
    serialize_types: &BTreeMap<Uuid, &NetworkSerializeType>,
) -> Option<String> {
    if let Some(type_name) = shape
        .type_name_full
        .as_deref()
        .or(shape.type_name.as_deref())
        && let Some(rust_type) = runtime_semantic_container_member_type(type_name)
    {
        return Some(rust_type.to_owned());
    }
    if container_value_shape_uses_source_type(shape, serialize_types) {
        return shape
            .type_name_full
            .as_deref()
            .or(shape.type_name.as_deref())
            .and_then(serialize_source_rust_type_name);
    }
    container_value_shape_support_type_name(field.name.as_deref()?, shape)
}

fn container_value_shape_uses_source_type(
    shape: &crate::network_schema::NetworkNestedTypeShape,
    _serialize_types: &BTreeMap<Uuid, &NetworkSerializeType>,
) -> bool {
    shape.member_names_proven == Some(true)
        && shape
            .member_name_source
            .as_deref()
            .is_some_and(|source| source.contains("serialize") || source == "ghidra-datatype")
        && !shape
            .validation
            .as_deref()
            .is_some_and(|validation| validation.contains("native-rtti"))
}

fn container_value_shape_support_type_name(
    field_name: &str,
    shape: &crate::network_schema::NetworkNestedTypeShape,
) -> Option<String> {
    let type_name = shape.type_name.as_deref()?;
    Some(format!(
        "{}{}",
        rust_type_ident(&rust_field_ident(field_name)),
        rust_type_ident(type_name)
    ))
}

fn keyed_replicated_container_type(key_type: &str, value_type: &str) -> String {
    format!("::nw_network::serialize::IndexMap<{key_type}, {value_type}>")
}

fn unique_container_value_candidate<'a>(
    container: &NetworkReplicatedContainerShape,
    value_type_candidates: &[NetworkNativeTypeInfoEvidence],
    serialize_types: &'a BTreeMap<Uuid, &NetworkSerializeType>,
) -> Option<&'a NetworkSerializeType> {
    let mut matches = Vec::new();
    let mut seen = BTreeSet::new();
    for candidate in value_type_candidates {
        let Some(type_id) = candidate.type_id else {
            continue;
        };
        if !seen.insert(type_id) {
            continue;
        }
        let Some(serialize) = serialize_types.get(&type_id).copied() else {
            continue;
        };
        if serialize.role == NetworkSerializeRole::SupportType
            && wire_scalar_shapes_match(&container.value_wire_shapes, &serialize.wire_shapes)
        {
            matches.push(serialize);
        }
    }
    let [matched] = matches.as_slice() else {
        return None;
    };
    Some(*matched)
}

fn serialize_container_value_type(
    kind: NetworkSerializeKind,
    name: &str,
    wire_shapes: &[SchemaWireScalarShape],
) -> Option<ContainerValueType> {
    let rust_type = runtime_semantic_container_member_type(name)
        .map(ToOwned::to_owned)
        .or_else(|| serialize_source_rust_type_name(name))?;
    let marshaler_type = if kind == NetworkSerializeKind::Enum && wire_shapes.len() == 1 {
        conversion_marshal_type_string_for(wire_shapes[0].into(), &rust_type)
            .unwrap_or_else(|| format!("::nw_network::serialize::DefaultMarshaler<{rust_type}>"))
    } else {
        format!("::nw_network::serialize::DefaultMarshaler<{rust_type}>")
    };
    Some(ContainerValueType {
        rust_type,
        marshaler_type,
        value_type_shape: None,
        embedded_value_type_shapes: Vec::new(),
    })
}

fn vector_container_value_type(
    container: &NetworkReplicatedContainerShape,
    serialize_types: &BTreeMap<Uuid, &NetworkSerializeType>,
) -> Option<ContainerValueType> {
    let vector_shape_element = container_source_vector_shape_element(container);
    if container.value_wire_shapes.first() != Some(&SchemaWireScalarShape::VlqU32)
        && vector_shape_element.is_none()
    {
        return None;
    }
    let type_name = container
        .value_type_name
        .as_deref()
        .or_else(|| {
            container
                .value_type_shape
                .as_ref()?
                .type_name_full
                .as_deref()
        })
        .or_else(|| container.value_type_shape.as_ref()?.type_name.as_deref())?;
    let element_name = azstd_vector_inner_type(type_name).or(vector_shape_element)?;
    let serialize = serialize_types
        .values()
        .copied()
        .find(|candidate| candidate.name == element_name)?;
    let element_rust_type = serialize_source_rust_type_name(&serialize.name)?;
    let rust_type = format!("::std::vec::Vec<{element_rust_type}>");
    let marshaler_type = format!("::nw_network::serialize::DefaultMarshaler<{rust_type}>");
    Some(ContainerValueType {
        rust_type,
        marshaler_type,
        value_type_shape: None,
        embedded_value_type_shapes: Vec::new(),
    })
}

fn container_source_vector_shape_element(
    container: &NetworkReplicatedContainerShape,
) -> Option<&str> {
    let shape = container.value_type_shape.as_ref()?;
    if !shape
        .validation
        .as_deref()
        .is_some_and(|validation| validation.contains("serialize-type-sequence"))
    {
        return None;
    }
    let [member] = shape.members.as_slice() else {
        return None;
    };
    member
        .wire_shape
        .as_deref()
        .and_then(vector_element_wire_shape)
}

fn azstd_vector_inner_type(type_name: &str) -> Option<&str> {
    type_name
        .trim()
        .strip_prefix("AZStd::vector<")
        .or_else(|| type_name.trim().strip_prefix("vector<"))
        .or_else(|| type_name.trim().strip_prefix("Vec<"))?
        .strip_suffix('>')
        .map(str::trim)
        .filter(|name| !name.is_empty())
}

fn container_value_matches_serialize(
    container: &NetworkReplicatedContainerShape,
    serialize: &NetworkSerializeFieldType,
) -> bool {
    container
        .value_type_id
        .is_none_or(|type_id| type_id == serialize.type_id)
        && serialize.role == NetworkSerializeRole::SupportType
        && wire_scalar_shapes_match(&container.value_wire_shapes, &serialize.wire_shapes)
}

fn container_named_source_type(
    type_name: &str,
    wire_shapes: &[SchemaWireScalarShape],
    serialize_types: &BTreeMap<Uuid, &NetworkSerializeType>,
) -> Option<ContainerValueType> {
    let leaf_name = type_name.rsplit("::").next().unwrap_or(type_name).trim();
    if let Some(value) = runtime_semantic_container_type(leaf_name, wire_shapes) {
        return Some(value);
    }
    let serialize = serialize_types.values().copied().find(|serialize| {
        serialize.role == NetworkSerializeRole::SupportType
            && serialize.name == leaf_name
            && wire_scalar_shapes_match(wire_shapes, &serialize.wire_shapes)
    })?;
    serialize_container_value_type(serialize.kind, &serialize.name, wire_shapes)
}

fn container_value_shape_members_are_emittable(
    shape: &crate::network_schema::NetworkNestedTypeShape,
    embedded_shapes: &[crate::network_schema::NetworkNestedTypeShape],
) -> bool {
    shape.members.iter().all(|member| {
        member.wire_shape.as_deref().is_some_and(|wire_shape| {
            container_value_member_wire_shape_is_emittable(
                wire_shape,
                member.native_type.as_deref(),
                embedded_shapes,
            )
        })
    })
}

fn container_value_member_wire_shape_is_emittable(
    wire_shape: &str,
    native_type: Option<&str>,
    embedded_shapes: &[crate::network_schema::NetworkNestedTypeShape],
) -> bool {
    if wire_scalar_shape_from_name(wire_shape).is_some() {
        return true;
    }
    if native_type
        .and_then(container_member_source_rust_type)
        .is_some()
    {
        return true;
    }
    if let Some(shapes) = composite_member_wire_shapes(wire_shape) {
        return composite_member_rust_type(native_type, &shapes).is_some();
    }
    if let Some(element) = vector_element_wire_shape(wire_shape) {
        return wire_scalar_shape_from_name(element).is_some()
            || nested_shape_by_wire_name(element, embedded_shapes).is_some_and(|shape| {
                container_value_shape_members_are_emittable(shape, embedded_shapes)
            });
    }
    nested_shape_by_wire_name(wire_shape, embedded_shapes)
        .is_some_and(|shape| container_value_shape_members_are_emittable(shape, embedded_shapes))
}

fn runtime_semantic_container_type(
    leaf_name: &str,
    wire_shapes: &[SchemaWireScalarShape],
) -> Option<ContainerValueType> {
    let rust_type = match leaf_name {
        "RemoteServerGDERef" | "RemoteServerGdeRef"
            if wire_shapes
                == [
                    SchemaWireScalarShape::FixedBytes(16),
                    SchemaWireScalarShape::U64,
                ] =>
        {
            "::nw_network::RemoteServerGdeRef"
        }
        "RemoteTypelessServerFacetRef"
            if wire_shapes
                == [
                    SchemaWireScalarShape::FixedBytes(16),
                    SchemaWireScalarShape::U64,
                    SchemaWireScalarShape::U64,
                ] =>
        {
            "::nw_network::RemoteTypelessServerFacetRef"
        }
        _ => return None,
    };
    Some(ContainerValueType {
        rust_type: rust_type.to_owned(),
        marshaler_type: format!("::nw_network::serialize::DefaultMarshaler<{rust_type}>"),
        value_type_shape: None,
        embedded_value_type_shapes: Vec::new(),
    })
}

fn wire_scalar_shapes_match(
    observed: &[SchemaWireScalarShape],
    expected: &[SchemaWireScalarShape],
) -> bool {
    observed.len() == expected.len()
        && observed
            .iter()
            .zip(expected)
            .all(|(observed, expected)| scalar_shapes_match(*observed, *expected))
}

fn serialize_source_rust_type_name(name: &str) -> Option<String> {
    let rust_type = format!("::nw_network::source::{}", rust_type_ident(name));
    syn::parse_str::<syn::Type>(&rust_type).ok()?;
    Some(rust_type)
}

fn scalar_rust_type(shape: SchemaWireScalarShape) -> String {
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
        | SchemaWireScalarShape::NonUniformScaleComp => "::glam::Vec3".to_owned(),
        SchemaWireScalarShape::QuatComp | SchemaWireScalarShape::QuatSmallestThree => {
            "::glam::Quat".to_owned()
        }
        SchemaWireScalarShape::PositionAnchor => "(f32, f32, f32)".to_owned(),
        SchemaWireScalarShape::TransformCompressor => "::glam::Affine3A".to_owned(),
        SchemaWireScalarShape::PackedSize => "::nw_network::serialize::PackedSize".to_owned(),
        SchemaWireScalarShape::Mat3 => "::glam::Mat3".to_owned(),
        SchemaWireScalarShape::Affine3 => "::glam::Affine3A".to_owned(),
        SchemaWireScalarShape::Aabb2d => "::bevy_math::bounding::Aabb2d".to_owned(),
        SchemaWireScalarShape::Aabb3d => "::bevy_math::bounding::Aabb3d".to_owned(),
        SchemaWireScalarShape::ActorRef => "::nw_network::ActorRef".to_owned(),
        SchemaWireScalarShape::EntityRef => "::nw_network::EntityRef".to_owned(),
        SchemaWireScalarShape::FixedBytes(len) => format!("[u8; {len}]"),
        SchemaWireScalarShape::String => "String".to_owned(),
    }
}

fn scalar_marshaler_type(shape: SchemaWireScalarShape) -> String {
    match shape {
        SchemaWireScalarShape::HalfF32 => "::nw_network::serialize::HalfF32Marshaler".to_owned(),
        SchemaWireScalarShape::VlqU32 => "::nw_network::serialize::VlqU32Marshaler".to_owned(),
        SchemaWireScalarShape::VlqU64 => "::nw_network::serialize::VlqU64Marshaler".to_owned(),
        SchemaWireScalarShape::Vec2Comp => "::nw_network::serialize::Vec2CompMarshaler".to_owned(),
        SchemaWireScalarShape::Vec3Comp => "::nw_network::serialize::Vec3CompMarshaler".to_owned(),
        SchemaWireScalarShape::Vec3CompNorm => {
            "::nw_network::serialize::Vec3CompNormMarshaler".to_owned()
        }
        SchemaWireScalarShape::QuatComp => "::nw_network::serialize::QuatCompMarshaler".to_owned(),
        SchemaWireScalarShape::QuatSmallestThree => {
            "::nw_network::serialize::QuatSmallestThreeQuantizedMarshaler".to_owned()
        }
        SchemaWireScalarShape::NonUniformScaleComp => {
            "::nw_network::serialize::NonUniformScaleCompMarshaler".to_owned()
        }
        SchemaWireScalarShape::PositionAnchor => {
            "::nw_network::serialize::PositionAnchorMarshaler".to_owned()
        }
        SchemaWireScalarShape::TransformCompressor => {
            "::nw_network::serialize::TransformCompressor".to_owned()
        }
        _ => {
            let rust_type = scalar_rust_type(shape);
            format!("::nw_network::serialize::DefaultMarshaler<{rust_type}>")
        }
    }
}

fn replicated_field_handler_type(shape: SchemaWireShape, rust_type: &str) -> String {
    if let Some(conversion) = conversion_marshal_type_string_for(shape, rust_type) {
        return format!(
            "::nw_network::serialize::ReplicatedFieldHandler<{rust_type}, {conversion}>"
        );
    }
    format!("::nw_network::serialize::ReplicatedFieldHandler<{rust_type}>")
}

fn is_replicated_state_field_type(rust_type: &str) -> bool {
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
        can_generate: false,
        blocked_reasons: vec![reason.to_owned()],
        fields: Vec::new(),
    }
}

fn replicated_state_module_tokens(
    network_type: &NetworkType,
    plan: &NetworkStateGenerationPlanReport,
    rust_names: &BTreeMap<u32, String>,
    options: &NetworkReplicatedStateEmitOptions,
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
        .flat_map(|field| replicated_state_field_support_tokens(field, &mut support_names))
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
            use ::nw_network::{az_rtti, replicated_state #type_registry_import};

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

fn replicated_state_field_support_tokens(
    field: &NetworkStateFieldShapeReport,
    emitted_names: &mut BTreeSet<String>,
) -> Vec<proc_macro2::TokenStream> {
    let mut items = Vec::new();
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
        ) {
            items.push(tokens);
        }
    }
    if let Some(shape) = field.container_key_type_shape.as_ref()
        && field_references_container_shape_codec(field, shape)
        && let Some(tokens) = replicated_state_shape_support_tokens(
            field,
            shape,
            &field.container_embedded_key_type_shapes,
            true,
            emitted_names,
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
        ) {
            items.push(tokens);
        }
    }
    if let Some(shape) = field.container_value_type_shape.as_ref()
        && field_references_container_shape_codec(field, shape)
        && let Some(tokens) = replicated_state_shape_support_tokens(
            field,
            shape,
            &field.container_embedded_value_type_shapes,
            false,
            emitted_names,
        )
    {
        items.push(tokens);
    }
    items
}

fn container_embedded_shape_is_referenced(
    parent: Option<&crate::network_schema::NetworkNestedTypeShape>,
    shape: &crate::network_schema::NetworkNestedTypeShape,
) -> bool {
    let Some(parent) = parent else {
        return false;
    };
    parent.members.iter().any(|member| {
        member.wire_shape.as_deref().is_some_and(|wire_shape| {
            nested_shape_by_wire_name(wire_shape, core::slice::from_ref(shape)).is_some()
                || vector_element_wire_shape(wire_shape)
                    .and_then(|element| {
                        nested_shape_by_wire_name(element, core::slice::from_ref(shape))
                    })
                    .is_some()
        })
    })
}

fn field_references_container_shape_codec(
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

fn replicated_state_shape_support_tokens(
    field: &NetworkStateFieldShapeReport,
    shape: &crate::network_schema::NetworkNestedTypeShape,
    embedded_shapes: &[crate::network_schema::NetworkNestedTypeShape],
    derive_key_traits: bool,
    emitted_names: &mut BTreeSet<String>,
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
    let value_type_string = if container_value_shape_report_uses_source_type(shape) {
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
        .map(|member| container_value_member_tokens(field, member, embedded_shapes))
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
        if container_value_shape_report_uses_source_type(shape) {
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
    let support_struct = if container_value_shape_report_uses_source_type(shape) {
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
    let marshaler_impl = if container_value_shape_report_uses_source_type(shape) {
        quote! {}
    } else {
        quote! {
            impl ::nw_network::serialize::Marshaler for #value_type {
                const MARSHAL_SIZE: usize =
                    <#codec_ident as ::nw_network::serialize::Codec<#value_type>>::MARSHAL_SIZE;

                fn marshal(&self, wb: &mut ::nw_network::serialize::WriteBuffer) {
                    <#codec_ident as ::nw_network::serialize::Codec<#value_type>>::marshal(self, wb);
                }

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

struct ContainerValueMemberTokens {
    binding: proc_macro2::Ident,
    access: proc_macro2::TokenStream,
    field_ident: proc_macro2::Ident,
    rust_type: syn::Type,
    codec_type: syn::Type,
    is_flat_field: bool,
}

fn container_value_member_tokens(
    field: &NetworkStateFieldShapeReport,
    member: &crate::network_schema::NetworkNestedTypeMember,
    embedded_shapes: &[crate::network_schema::NetworkNestedTypeShape],
) -> Option<ContainerValueMemberTokens> {
    let name = member.name.as_deref()?;
    let binding = format_ident!("field_{}", rust_field_ident(&name.replace('.', "_")));
    let access = member_access_tokens(name)?;
    let field_ident = format_ident!("{}", rust_field_ident(name));
    let rust_type_string = container_value_member_rust_type(field, member, embedded_shapes)?;
    let rust_type = syn::parse_str::<syn::Type>(&rust_type_string).ok()?;
    let codec_type_string = container_value_member_codec_type(member, &rust_type_string)?;
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

fn container_value_shape_report_uses_source_type(
    shape: &crate::network_schema::NetworkNestedTypeShape,
) -> bool {
    shape.member_names_proven == Some(true)
        && shape
            .member_name_source
            .as_deref()
            .is_some_and(|source| source.contains("serialize") || source == "ghidra-datatype")
        && !shape
            .validation
            .as_deref()
            .is_some_and(|validation| validation.contains("native-rtti"))
}

fn container_value_shape_report_rust_type(
    field: &NetworkStateFieldShapeReport,
    shape: &crate::network_schema::NetworkNestedTypeShape,
) -> Option<String> {
    if let Some(type_name) = shape
        .type_name_full
        .as_deref()
        .or(shape.type_name.as_deref())
        && let Some(rust_type) = runtime_semantic_container_member_type(type_name)
    {
        return Some(rust_type.to_owned());
    }
    if container_value_shape_report_uses_source_type(shape) {
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

fn member_access_tokens(name: &str) -> Option<proc_macro2::TokenStream> {
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

fn container_value_member_rust_type(
    field: &NetworkStateFieldShapeReport,
    member: &crate::network_schema::NetworkNestedTypeMember,
    embedded_shapes: &[crate::network_schema::NetworkNestedTypeShape],
) -> Option<String> {
    let shape = member.wire_shape.as_deref()?;
    if let Some(shape) = nested_shape_by_wire_name(shape, embedded_shapes) {
        return container_value_shape_report_rust_type(field, shape);
    }
    if let Some(native_type) = member.native_type.as_deref()
        && let Some(rust_type) = container_member_source_rust_type(native_type)
    {
        return Some(rust_type);
    }
    if let Some(shapes) = composite_member_wire_shapes(shape) {
        return composite_member_rust_type(member.native_type.as_deref(), &shapes);
    }
    if let Some(element_shape) = vector_element_wire_shape(shape) {
        let element_type = if let Some(element_shape) = wire_scalar_shape_from_name(element_shape) {
            scalar_rust_type(element_shape)
        } else {
            let shape = nested_shape_by_wire_name(element_shape, embedded_shapes)?;
            container_value_shape_report_rust_type(field, shape)?
        };
        return Some(format!("::std::vec::Vec<{element_type}>"));
    }
    let shape = wire_scalar_shape_from_name(shape)?;
    Some(scalar_rust_type(shape))
}

fn composite_member_rust_type(
    native_type: Option<&str>,
    shapes: &[SchemaWireScalarShape],
) -> Option<String> {
    let [first, rest @ ..] = shapes else {
        return None;
    };
    if !rest.iter().all(|shape| shape == first) {
        return None;
    }
    let scalar = *first;
    let mut element_type = scalar_rust_type(scalar);
    if native_type
        .map(str::trim)
        .is_some_and(|native_type| native_type == "AZStd::array")
        && scalar == SchemaWireScalarShape::U16
    {
        element_type = "i16".to_owned();
    }
    Some(format!("[{element_type}; {}]", shapes.len()))
}

fn container_member_source_rust_type(native_type: &str) -> Option<String> {
    let native_type = native_type.trim();
    if native_type == "bool" {
        return Some("bool".to_owned());
    }
    if let Some(rust_type) = runtime_semantic_container_member_type(native_type) {
        return Some(rust_type.to_owned());
    }
    let normalized = native_type.replace(' ', "");
    if matches!(
        normalized.as_str(),
        "AZStd::array<short,3>"
            | "AZStd::array<AZ::s16,3>"
            | "AZStd::array<i16,3>"
            | "std::array<i16,3>"
    ) {
        return Some("[i16; 3]".to_owned());
    }
    if matches!(native_type, "Quaternion" | "AZ::Quaternion") {
        return Some("::glam::Quat".to_owned());
    }
    None
}

fn runtime_semantic_container_member_type(native_type: &str) -> Option<&'static str> {
    match native_type.trim() {
        "AZ::Vector2" | "Vector2" | "Vec2" => Some("::glam::Vec2"),
        "AZ::Vector3" | "Vector3" | "Vec3" => Some("::glam::Vec3"),
        "AZ::Vector4" | "Vector4" | "Vec4" => Some("::glam::Vec4"),
        "AZ::Quaternion" | "Quaternion" | "Quat" => Some("::glam::Quat"),
        "AZ::Matrix3x3" | "Matrix3x3" | "Mat3" => Some("::glam::Mat3"),
        "AZ::Transform" | "Transform" => Some("::glam::Affine3A"),
        "ActorRef" | "Amazon::Hub::ActorRef" | "HubAddress" | "ProxyAddress" => {
            Some("::nw_network::ActorRef")
        }
        "BaselineableFragment" | "Amazon::Hub::BaselineableFragment" => {
            Some("::nw_network::hub::BaselineableFragment")
        }
        "FragmentKey" | "Amazon::Hub::FragmentKey" => Some("::nw_network::hub::FragmentKey"),
        "EntityRef" => Some("::nw_network::EntityRef"),
        "AZ::Uuid" | "Uuid" | "uuid::Uuid" | "::uuid::Uuid" | "GuildId" | "WarId" | "RaidId" => {
            Some("::uuid::Uuid")
        }
        "Amazon::Pervasives::UID" | "UID" => Some("::uuid::Uuid"),
        "TimePoint" | "MB::TimePoint" => Some("::nw_network::TimePoint"),
        "WallClockTimePoint" | "MB::WallClockTimePoint" => Some("::nw_network::WallClockTimePoint"),
        "Duration" | "AZStd::chrono::duration" => Some("::nw_network::Duration"),
        "GDEID" | "GdeId" => Some("::nw_network::GdeId"),
        "RemoteServerGDERef" | "RemoteServerGdeRef" => Some("::nw_network::RemoteServerGdeRef"),
        "RemoteServerContextRef" => Some("::nw_network::RemoteServerContextRef"),
        "RemoteTypelessServerFacetRef" => Some("::nw_network::RemoteTypelessServerFacetRef"),
        "AssetId" | "AZ::Data::AssetId" => Some("::nw_network::AssetId"),
        _ => None,
    }
}

fn container_value_member_codec_type(
    member: &crate::network_schema::NetworkNestedTypeMember,
    rust_type: &str,
) -> Option<String> {
    let wire_shape = member.wire_shape.as_deref()?;
    if vector_element_wire_shape(wire_shape).is_some()
        || composite_member_wire_shapes(wire_shape).is_some()
    {
        return Some(format!(
            "::nw_network::serialize::DefaultMarshaler<{rust_type}>"
        ));
    }
    if !wire_shape.is_empty() && wire_scalar_shape_from_name(wire_shape).is_none() {
        return Some(format!(
            "::nw_network::serialize::DefaultMarshaler<{rust_type}>"
        ));
    }
    let shape = wire_scalar_shape_from_name(wire_shape)?;
    if let Some(conversion) = conversion_marshal_type_string_for(shape.into(), rust_type) {
        return Some(conversion);
    }
    if scalar_shape_uses_custom_codec(shape) {
        return Some(scalar_marshaler_type(shape));
    }
    Some(format!(
        "::nw_network::serialize::DefaultMarshaler<{rust_type}>"
    ))
}

fn scalar_shape_uses_custom_codec(shape: SchemaWireScalarShape) -> bool {
    matches!(
        shape,
        SchemaWireScalarShape::HalfF32
            | SchemaWireScalarShape::VlqU32
            | SchemaWireScalarShape::VlqU64
            | SchemaWireScalarShape::Vec2Comp
            | SchemaWireScalarShape::Vec3Comp
            | SchemaWireScalarShape::Vec3CompNorm
            | SchemaWireScalarShape::QuatComp
            | SchemaWireScalarShape::QuatSmallestThree
            | SchemaWireScalarShape::NonUniformScaleComp
            | SchemaWireScalarShape::PositionAnchor
            | SchemaWireScalarShape::TransformCompressor
    )
}

fn container_value_shape_report_codec_name(
    field: &NetworkStateFieldShapeReport,
    shape: &crate::network_schema::NetworkNestedTypeShape,
) -> Option<String> {
    let field_name = field.field_name.as_deref()?;
    let type_name = shape.type_name.as_deref()?;
    let field_name = rust_field_ident(field_name);
    Some(format!(
        "{}{}Marshaler",
        rust_type_ident(&field_name),
        rust_type_ident(type_name)
    ))
}

fn replicated_state_attr_tokens(
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

fn fragment_category_attr_name(category: &str) -> Option<&'static str> {
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
        SchemaWireShape::PositionAnchor => {
            quote!(
                ::nw_network::serialize::ReplicatedFieldHandler<
                    (f32, f32, f32),
                    ::nw_network::serialize::PositionAnchorMarshaler,
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
            let len = unsuffixed_int_lit(len);
            quote!(::nw_network::serialize::ReplicatedFieldHandler<[u8; #len]>)
        }
        SchemaWireShape::String => {
            quote!(::nw_network::serialize::ReplicatedFieldHandler<String>)
        }
        SchemaWireShape::ReplicatedContainer(_) => {
            unreachable!("container fields require an explicit ReplicatedContainer type")
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
    let mut support_names = BTreeSet::new();
    let support_items = plan
        .fields
        .iter()
        .filter_map(|field| message_field_support_tokens(field, &mut support_names))
        .collect::<Vec<_>>();

    quote! {
        pub mod #module_ident {
            use ::nw_network::{Marshaler, az_rtti, type_registry};

            #(#support_items)*

            #[az_rtti(#type_id)]
            #[type_registry(#type_index)]
            #[derive(Debug, Clone, PartialEq, Marshaler)]
            pub struct #message_ident {
                #(#fields)*
            }
        }

        pub use #module_ident::#message_ident;
    }
}

fn message_field_support_tokens(
    field: &NetworkStateFieldShapeReport,
    emitted_names: &mut BTreeSet<String>,
) -> Option<proc_macro2::TokenStream> {
    let shape = field.nested_type_shape.as_ref()?;
    if message_nested_shape_uses_source_type(shape) {
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
    let members = shape
        .members
        .iter()
        .map(|member| container_value_member_tokens(field, member, &[]))
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
        #[derive(Debug, Clone, Default, PartialEq)]
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

        impl ::nw_network::serialize::Marshaler for #value_type {
            const MARSHAL_SIZE: usize =
                <#codec_ident as ::nw_network::serialize::Codec<#value_type>>::MARSHAL_SIZE;

            fn marshal(&self, wb: &mut ::nw_network::serialize::WriteBuffer) {
                <#codec_ident as ::nw_network::serialize::Codec<#value_type>>::marshal(self, wb);
            }

            fn unmarshal(
                rb: &mut ::nw_network::serialize::ReadBuffer,
            ) -> Result<Self, ::nw_network::serialize::MarshalerError> {
                <#codec_ident as ::nw_network::serialize::Codec<#value_type>>::unmarshal(rb)
            }
        }
    })
}

fn message_support_type_ident(value: &str) -> Option<syn::Ident> {
    let ident = syn::parse_str::<syn::Ident>(value).ok()?;
    let ident_text = ident.to_string();
    if ident_text != value || is_builtin_rust_type_ident(&ident_text) {
        return None;
    }
    Some(ident)
}

fn is_builtin_rust_type_ident(value: &str) -> bool {
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

fn message_field_tokens(field: &NetworkStateFieldShapeReport) -> proc_macro2::TokenStream {
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
                    .expect("generatable message field has a field type"),
            )
        });
    let marshal_attr = message_field_marshal_attr_tokens(field);

    quote! {
        #marshal_attr
        pub #field_ident: #field_type,
    }
}

fn message_field_ident(field: &NetworkStateFieldShapeReport) -> String {
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

fn message_field_type_tokens(shape: SchemaWireShape) -> proc_macro2::TokenStream {
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
        | SchemaWireShape::NonUniformScaleComp => quote!(::glam::Vec3),
        SchemaWireShape::QuatComp | SchemaWireShape::QuatSmallestThree => quote!(::glam::Quat),
        SchemaWireShape::PositionAnchor => quote!((f32, f32, f32)),
        SchemaWireShape::TransformCompressor => quote!(::glam::Affine3A),
        SchemaWireShape::PackedSize => quote!(::nw_network::serialize::PackedSize),
        SchemaWireShape::Mat3 => quote!(::glam::Mat3),
        SchemaWireShape::Affine3 => quote!(::glam::Affine3A),
        SchemaWireShape::Aabb2d => quote!(::bevy_math::bounding::Aabb2d),
        SchemaWireShape::Aabb3d => quote!(::bevy_math::bounding::Aabb3d),
        SchemaWireShape::ActorRef => quote!(::nw_network::ActorRef),
        SchemaWireShape::EntityRef => quote!(::nw_network::EntityRef),
        SchemaWireShape::FixedBytes(len) => {
            let len = unsuffixed_int_lit(len);
            quote!([u8; #len])
        }
        SchemaWireShape::String => quote!(String),
        SchemaWireShape::ReplicatedContainer(_) => {
            unreachable!("container message fields require an explicit semantic type")
        }
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
        SchemaWireShape::QuatComp => {
            quote!(#[marshal(codec = "::nw_network::serialize::QuatCompMarshaler")])
        }
        SchemaWireShape::QuatSmallestThree => {
            quote!(#[marshal(codec = "::nw_network::serialize::QuatSmallestThreeQuantizedMarshaler")])
        }
        SchemaWireShape::NonUniformScaleComp => {
            quote!(#[marshal(codec = "::nw_network::serialize::NonUniformScaleCompMarshaler")])
        }
        SchemaWireShape::PositionAnchor => {
            quote!(#[marshal(codec = "::nw_network::serialize::PositionAnchorMarshaler")])
        }
        SchemaWireShape::TransformCompressor => {
            quote!(#[marshal(codec = "::nw_network::serialize::TransformCompressor")])
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
    let rust_type = field.rust_value_type.as_deref()?.trim();
    conversion_marshal_type_string_for(shape, rust_type)
}

fn serialize_field_scalar_source_type(
    field: &NetworkField,
    shape: Option<SchemaWireShape>,
) -> Option<String> {
    let serialize = field.serialize.as_ref()?;
    if serialize.kind != NetworkSerializeKind::Enum {
        return None;
    }
    scalar_conversion_serialized_type(shape?)?;
    let rust_type = format!("::nw_network::source::{}", rust_type_ident(&serialize.name));
    syn::parse_str::<syn::Type>(&rust_type).ok()?;
    Some(rust_type)
}

fn conversion_marshal_type_string_for(shape: SchemaWireShape, rust_type: &str) -> Option<String> {
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

fn count_capabilities(
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

fn capability_slice_tokens(
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

fn network_type_capability_ident(kind: NetworkTypeCapability) -> proc_macro2::Ident {
    match kind {
        NetworkTypeCapability::ReplicatedState => format_ident!("ReplicatedState"),
        NetworkTypeCapability::DirectMessage => format_ident!("DirectMessage"),
        NetworkTypeCapability::RegisteredFields => format_ident!("RegisteredFields"),
        NetworkTypeCapability::SupportData => format_ident!("SupportData"),
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
        SchemaWireShape::QuatComp => quote!(NetworkWireShape::QuatComp),
        SchemaWireShape::QuatSmallestThree => quote!(NetworkWireShape::QuatSmallestThree),
        SchemaWireShape::NonUniformScaleComp => quote!(NetworkWireShape::NonUniformScaleComp),
        SchemaWireShape::PositionAnchor => quote!(NetworkWireShape::PositionAnchor),
        SchemaWireShape::TransformCompressor => quote!(NetworkWireShape::TransformCompressor),
        SchemaWireShape::PackedSize => quote!(NetworkWireShape::PackedSize),
        SchemaWireShape::Mat3 => quote!(NetworkWireShape::Mat3),
        SchemaWireShape::Affine3 => quote!(NetworkWireShape::Affine3),
        SchemaWireShape::Aabb2d => quote!(NetworkWireShape::Aabb2d),
        SchemaWireShape::Aabb3d => quote!(NetworkWireShape::Aabb3d),
        SchemaWireShape::ActorRef => quote!(NetworkWireShape::ActorRef),
        SchemaWireShape::EntityRef => quote!(NetworkWireShape::EntityRef),
        SchemaWireShape::FixedBytes(len) => quote!(NetworkWireShape::FixedBytes(#len)),
        SchemaWireShape::String => quote!(NetworkWireShape::String),
        SchemaWireShape::ReplicatedContainer(container) => {
            let container = replicated_container_wire_shape_tokens(container);
            quote!(NetworkWireShape::ReplicatedContainer(#container))
        }
    }
}

fn replicated_container_wire_shape_tokens(
    container: NetworkReplicatedContainerWireShape,
) -> proc_macro2::TokenStream {
    let key = wire_scalar_shape_tokens(container.key);
    let value = wire_scalar_shape_tokens(container.value);
    quote!(NetworkReplicatedContainerWireShape {
        key: #key,
        value: #value,
    })
}

fn wire_scalar_shape_tokens(shape: SchemaWireScalarShape) -> proc_macro2::TokenStream {
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
        SchemaWireScalarShape::QuatComp => quote!(NetworkWireScalarShape::QuatComp),
        SchemaWireScalarShape::QuatSmallestThree => {
            quote!(NetworkWireScalarShape::QuatSmallestThree)
        }
        SchemaWireScalarShape::NonUniformScaleComp => {
            quote!(NetworkWireScalarShape::NonUniformScaleComp)
        }
        SchemaWireScalarShape::PositionAnchor => quote!(NetworkWireScalarShape::PositionAnchor),
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
        SchemaWireScalarShape::String => quote!(NetworkWireScalarShape::String),
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
    let literal = crate::uuid_format::uuid_u128_literal_text(type_id);
    let literal = LitInt::new(&literal, proc_macro2::Span::call_site());
    quote!(#literal)
}

#[cfg(test)]
mod tests {
    use serde_json::json;
    use uuid::uuid;

    use crate::{
        ir::{SerializeCodegenField, SerializeCodegenUnit, SerializeCodegenVariant},
        network_schema::{
            NetworkMessageFieldSignature, NetworkMessageSignature, NetworkSchema,
            NetworkWireScalarShape,
        },
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
        assert!(
            output
                .source
                .contains("0xA85DF621_DCE0_409F_8D39_A447EA0807FF")
        );
        assert!(
            !output
                .source
                .contains("0xA85D_F621_DCE0_409F_8D39_A447_EA08_07FF")
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
        assert!(state_output.source.contains("#[replicated_state]"));
        assert!(!state_output.source.contains("Default, ReplicatedState"));
        assert!(
            !state_output
                .source
                .contains("pub hub: ::nw_network::hub::ReplicatedState")
        );
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

        let unregistered_state_output = NetworkRustEmitter::emit_replicated_states_with_options(
            &schema,
            [28],
            NetworkReplicatedStateEmitOptions::unregistered(),
        )
        .expect("unregistered state source");

        assert!(
            unregistered_state_output
                .source
                .contains("pub struct RaidDataComponentReplicatedState")
        );
        assert!(
            unregistered_state_output
                .source
                .contains("impl ::nw_network::types::TypeRegistryEntry")
        );
        assert!(!unregistered_state_output.source.contains("#[type_registry"));
        assert!(
            !unregistered_state_output
                .source
                .contains("AzRtti, ReplicatedState, TypeRegistry")
        );
    }

    #[test]
    fn emits_single_generated_state_module_with_registration_allowlist() {
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
            }, {
                "uuid": "F9E72714-96F5-4092-8F90-136DCB98BDB3",
                "typeIndex": 29,
                "typeName": "Javelin::RaidGroupComponentReplicatedState",
                "fields": [{
                    "index": 0,
                    "name": "groupId",
                    "group": 0,
                    "handlerVtable": "NewWorld+0x81dad80",
                    "confidence": "register-field-call"
                }]
            }],
            "fieldRegistrationFunctions": [],
            "fieldHandlerVtables": [{
                "address": "NewWorld+0x81dad80",
                "fieldCount": 1,
                "wireShape": "u64",
                "wireShapeSource": "marshal-call:marshal-function-name",
                "slots": []
            }]
        }))
        .expect("schema");

        let output = NetworkRustEmitter::emit_replicated_states_with_options(
            &schema,
            [28, 29],
            NetworkReplicatedStateEmitOptions::register_only([28]),
        )
        .expect("allowlisted state source");

        assert_eq!(output.report.generatable_state_count, 2);
        assert!(
            output
                .source
                .contains("pub struct RaidDataComponentReplicatedState")
        );
        assert!(
            output
                .source
                .contains("pub struct RaidGroupComponentReplicatedState")
        );
        assert_eq!(output.source.matches("#[type_registry").count(), 1);
        assert!(output.source.contains("#[type_registry(28u32)]"));
        assert!(!output.source.contains("#[type_registry(29u32)]"));
    }

    #[test]
    fn emits_native_fragment_category_attribute_from_schema_evidence() {
        let schema = NetworkSchema::from_ghidra_static_network_report(&json!({
            "registryEntries": [{
                "uuid": "39B4C919-3A6D-46B5-92D0-3B4ACB284B1D",
                "typeIndex": 16,
                "typeName": "MB::ProjectileReplicatedState",
                "constructorMatches": [{
                    "fragmentMetadata": {
                        "source": "i-fragment-vtable",
                        "isMetadataSlot": 12,
                        "isMetadataFunction": "NewWorld+0x294910",
                        "isMetadata": false,
                        "categorySlot": 13,
                        "categoryFunction": "NewWorld+0x6840000",
                        "categoryValue": 5,
                        "category": "Projectile"
                    },
                    "fields": []
                }],
                "fields": [{
                    "index": 0,
                    "name": "projectileId",
                    "group": 0,
                    "handlerVtable": "NewWorld+0x81dad80",
                    "confidence": "register-field-call"
                }]
            }],
            "fieldRegistrationFunctions": [],
            "fieldHandlerVtables": [{
                "address": "NewWorld+0x81dad80",
                "fieldCount": 1,
                "wireShape": "u32",
                "wireShapeSource": "marshal-call:marshal-function-name",
                "slots": []
            }]
        }))
        .expect("schema");

        let output =
            NetworkRustEmitter::emit_replicated_states(&schema, [16]).expect("state source");
        let plan = &output.report.state_generation_plans[0];
        assert_eq!(plan.fragment_category.as_deref(), Some("Projectile"));
        assert_eq!(plan.fragment_category_value, Some(5));
        assert_eq!(plan.is_metadata_fragment, Some(false));
        assert!(
            output
                .source
                .contains("#[replicated_state(category = \"projectile\")]")
        );
    }

    #[test]
    fn replicated_state_attributes_are_not_emitted_as_normal_fields() {
        let schema = NetworkSchema::from_ghidra_static_network_report(&json!({
            "registryEntries": [{
                "uuid": "203DC8C7-0C60-454B-A46F-566114314B84",
                "typeIndex": 10,
                "typeName": "MB::GdeMetadataReplicatedState",
                "fields": [{
                    "index": 0,
                    "name": "AssetId",
                    "registrationKind": "field",
                    "handlerVtable": "NewWorld+0x8041098",
                    "confidence": "fixed-field-table-append"
                }, {
                    "index": 1,
                    "name": "ReplicationCategory",
                    "registrationKind": "attribute",
                    "handlerVtable": "NewWorld+0x8041028",
                    "confidence": "fixed-attribute-table-append"
                }]
            }],
            "fieldRegistrationFunctions": [],
            "fieldHandlerVtables": [{
                "address": "NewWorld+0x8041098",
                "fieldCount": 1,
                "wireShape": "u32",
                "wireShapeSource": "handler-template-type",
                "slots": []
            }, {
                "address": "NewWorld+0x8041028",
                "fieldCount": 1,
                "wireShape": "u8",
                "wireShapeSource": "handler-template-type",
                "slots": []
            }]
        }))
        .expect("schema");

        assert_eq!(
            schema.types[0].fields[1].registration_kind.as_deref(),
            Some("attribute")
        );

        let output =
            NetworkRustEmitter::emit_replicated_states(&schema, [10]).expect("state source");
        let plan = &output.report.state_generation_plans[0];
        assert_eq!(plan.field_count, 1);
        assert_eq!(plan.attribute_count, 1);
        assert!(output.source.contains("pub asset_id:"));
        assert!(!output.source.contains("pub replication_category:"));
    }

    #[test]
    fn disambiguates_repeated_replicated_state_field_labels_by_field_index() {
        let schema = NetworkSchema::from_ghidra_static_network_report(&json!({
            "registryEntries": [{
                "uuid": "01B0664B-3AB6-44A6-87E3-8C69D40E0365",
                "typeIndex": 11,
                "typeName": "MB::ALCReplicatedState",
                "capabilities": ["replicated-state"],
                "fields": [{
                    "index": 0,
                    "name": "Value",
                    "wireShape": "u8",
                    "confidence": "fixed-field-table-append"
                }, {
                    "index": 1,
                    "name": "Value",
                    "wireShape": "u8",
                    "confidence": "fixed-field-table-append"
                }, {
                    "index": 2,
                    "name": "Value",
                    "wireShape": "u8",
                    "confidence": "fixed-field-table-append"
                }]
            }],
            "fieldRegistrationFunctions": []
        }))
        .expect("schema");

        let output =
            NetworkRustEmitter::emit_replicated_states(&schema, [11]).expect("state source");
        let plan = &output.report.state_generation_plans[0];

        assert_eq!(plan.fields[0].field_name.as_deref(), Some("Value"));
        assert_eq!(plan.fields[1].field_name.as_deref(), Some("Value_1"));
        assert_eq!(plan.fields[2].field_name.as_deref(), Some("Value_2"));
        assert!(output.source.contains("pub value:"));
        assert!(output.source.contains("pub value_1:"));
        assert!(output.source.contains("pub value_2:"));
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
    fn replicated_state_rust_type_override_wraps_value_type() {
        let schema = NetworkSchema::from_ghidra_static_network_report(&json!({
            "registryEntries": [{
                "uuid": "B4DB39E2-5054-4604-9855-9A4DC75BDDE4",
                "typeIndex": 3362,
                "typeName": "MB::SlayerScriptReplicatedState",
                "fields": [{
                    "index": 0,
                    "name": "curScriptStateId",
                    "group": 0,
                    "nativeType": "AZ::s8",
                    "rustType": "i8",
                    "wireShape": "u8",
                    "wireShapeSource": "source:field-override",
                    "confidence": "register-field-call"
                }]
            }],
            "fieldRegistrationFunctions": []
        }))
        .expect("schema");

        let output =
            NetworkRustEmitter::emit_replicated_states(&schema, [3362]).expect("state source");
        let plan = &output.report.state_generation_plans[0];

        assert!(plan.can_generate, "{plan:#?}");
        assert_eq!(plan.missing_wire_shape_count, 0);
        assert_eq!(plan.fields[0].rust_value_type.as_deref(), Some("i8"));
        assert_eq!(
            plan.fields[0].rust_field_type.as_deref(),
            Some("::nw_network::serialize::ReplicatedFieldHandler<i8>")
        );
        assert!(output.source.contains(
            "pub cur_script_state_id: ::nw_network::serialize::ReplicatedFieldHandler<i8>"
        ));
    }

    #[test]
    fn replicated_state_rust_type_override_can_be_complete_field_type() {
        let schema = NetworkSchema::from_ghidra_static_network_report(&json!({
            "registryEntries": [{
                "uuid": "B4DB39E2-5054-4604-9855-9A4DC75BDDE4",
                "typeIndex": 3362,
                "typeName": "MB::SlayerScriptReplicatedState",
                "fields": [{
                    "index": 2,
                    "name": "spawnedEntityIdsBySpawnerId",
                    "group": 0,
                    "handlerVtable": "NewWorld+0x81bf3d0",
                    "nativeType": "MB::ReplicatedMapFieldHandler<AZ::Crc32, AZ::EntityId>",
                    "rustType": "::nw_network::serialize::ReplicatedContainer<::nw_network::serialize::IndexMap<::nw_network::Crc32, ::nw_network::EntityId>, { ::nw_network::serialize::WIRE_VEC_CAP }, ::nw_network::serialize::DefaultMarshaler<::nw_network::Crc32>, ::nw_network::serialize::DefaultMarshaler<::nw_network::EntityId>>",
                    "confidence": "register-field-call"
                }]
            }],
            "fieldRegistrationFunctions": [],
            "fieldHandlerVtables": [{
                "address": "NewWorld+0x81bf3d0",
                "fieldCount": 1,
                "wireShape": "vlq-u32",
                "wireShapeSource": "marshal-call:ambiguous-container-helper",
                "slots": []
            }]
        }))
        .expect("schema");

        let output =
            NetworkRustEmitter::emit_replicated_states(&schema, [3362]).expect("state source");
        let plan = &output.report.state_generation_plans[0];

        assert!(plan.can_generate, "{plan:#?}");
        assert_eq!(plan.shaped_field_count, 1);
        assert_eq!(plan.missing_wire_shape_count, 0);
        assert_eq!(plan.fields[0].wire_shape, None);
        assert_eq!(plan.fields[0].rust_value_type, None);
        assert_eq!(
            plan.fields[0].rust_field_type.as_deref(),
            Some(
                "::nw_network::serialize::ReplicatedContainer<::nw_network::serialize::IndexMap<::nw_network::Crc32, ::nw_network::EntityId>, { ::nw_network::serialize::WIRE_VEC_CAP }, ::nw_network::serialize::DefaultMarshaler<::nw_network::Crc32>, ::nw_network::serialize::DefaultMarshaler<::nw_network::EntityId>>"
            )
        );
        assert!(output.source.contains("ReplicatedContainer"));
        assert!(!output.source.contains("ReplicatedMap<"));
        assert!(output.source.contains("IndexMap"));
        assert!(output.source.contains("::nw_network::Crc32"));
        assert!(output.source.contains("::nw_network::EntityId"));
    }

    #[test]
    fn inferred_container_wire_shapes_emit_replicated_container_fields() {
        let schema = NetworkSchema::from_ghidra_static_network_report(&json!({
            "registryEntries": [{
                "uuid": "B4DB39E2-5054-4604-9855-9A4DC75BDDE4",
                "typeIndex": 3362,
                "typeName": "MB::SlayerScriptReplicatedState",
                "fields": [{
                    "index": 2,
                    "name": "spawnedEntityIdsBySpawnerId",
                    "group": 0,
                    "handlerVtable": "NewWorld+0x81bf3d0",
                    "confidence": "register-field-call"
                }]
            }],
            "fieldRegistrationFunctions": [],
            "fieldHandlerVtables": [{
                "address": "NewWorld+0x81bf3d0",
                "fieldCount": 1,
                "wireShape": "replicated-container<u32,vlq-u64>",
                "wireShapeSource": "replicated-container-marshal-calls",
                "slots": []
            }]
        }))
        .expect("schema");

        let descriptor_output =
            NetworkRustEmitter::emit_descriptors(&schema).expect("descriptor source");
        let plan = &descriptor_output.report.state_generation_plans[0];

        assert!(plan.can_generate, "{plan:#?}");
        assert_eq!(plan.shaped_field_count, 1);
        assert_eq!(plan.supported_field_count, 1);
        assert!(plan.blocked_reasons.is_empty());
        assert_eq!(
            plan.fields[0].wire_shape,
            Some(SchemaWireShape::ReplicatedContainer(
                NetworkReplicatedContainerWireShape {
                    key: SchemaWireScalarShape::U32,
                    value: SchemaWireScalarShape::VlqU64,
                }
            ))
        );
        assert_eq!(plan.fields[0].blocked_reason, None);
        assert_eq!(
            plan.fields[0].rust_value_type.as_deref(),
            Some("::nw_network::serialize::IndexMap<u32, u64>")
        );
        assert_eq!(
            plan.fields[0].rust_field_type.as_deref(),
            Some(
                "::nw_network::serialize::ReplicatedContainer<::nw_network::serialize::IndexMap<u32, u64>, { ::nw_network::serialize::WIRE_VEC_CAP }, ::nw_network::serialize::DefaultMarshaler<u32>, ::nw_network::serialize::VlqU64Marshaler>"
            )
        );
        assert!(
            descriptor_output
                .source
                .contains("NetworkWireShape::ReplicatedContainer")
        );
        assert!(
            descriptor_output
                .source
                .contains("NetworkWireScalarShape::VlqU64")
        );

        let state_output =
            NetworkRustEmitter::emit_replicated_states(&schema, [3362]).expect("state source");
        assert!(state_output.source.contains("ReplicatedContainer"));
        assert!(state_output.source.contains("IndexMap"));
        assert!(state_output.source.contains("u32"));
        assert!(state_output.source.contains("u64"));
    }

    #[test]
    fn selected_struct_container_shape_emits_full_replicated_container_type() {
        let value_type_id = uuid!("da4e5889-a65c-4480-8642-0278160125a7");
        let mut schema = NetworkSchema::from_ghidra_static_network_report(&json!({
            "registryEntries": [{
                "uuid": "B4DB39E2-5054-4604-9855-9A4DC75BDDE4",
                "typeIndex": 3362,
                "typeName": "MB::StructuredMapReplicatedState",
                "fields": [{
                    "index": 0,
                    "name": "valuesById",
                    "group": 0,
                    "handlerVtable": "NewWorld+0x81bf3d0",
                    "confidence": "register-field-call"
                }]
            }],
            "fieldRegistrationFunctions": [],
            "fieldHandlerVtables": [{
                "address": "NewWorld+0x81bf3d0",
                "fieldCount": 1,
                "wireShape": "replicated-container<u32,u8>",
                "wireShapeSource": "replicated-container-marshal-calls",
                "deltaMarshalShapes": ["vlq-u32", "vlq-u32"],
                "fullMarshalShapes": ["sequence-number", "vlq-u32", "u32", "u8", "u64"],
                "valueTypeName": "ExampleValue",
                "valueTypeId": value_type_id.to_string(),
                "valueTypeInfoAddress": "NewWorld+0x8123450",
                "slots": []
            }]
        }))
        .expect("schema");
        schema.merge_serialize_codegen_unit(
            &SerializeCodegenUnit {
                items: vec![example_value_item(
                    value_type_id,
                    [ScalarType::U8, ScalarType::U64],
                )],
            },
            Some("selection.json".to_owned()),
        );

        let output =
            NetworkRustEmitter::emit_replicated_states(&schema, [3362]).expect("state source");
        let plan = &output.report.state_generation_plans[0];
        let field = &plan.fields[0];

        assert!(plan.can_generate, "{plan:#?}");
        assert_eq!(field.wire_shape, None);
        assert_eq!(field.serialize_type_name.as_deref(), Some("ExampleValue"));
        assert_eq!(
            field.rust_value_type.as_deref(),
            Some("::nw_network::serialize::IndexMap<u32, ::nw_network::source::ExampleValue>")
        );
        assert!(
            field
                .rust_field_type
                .as_deref()
                .is_some_and(|ty| ty.contains("ReplicatedContainer<"))
        );
        assert!(
            output
                .source
                .contains("IndexMap<u32, ::nw_network::source::ExampleValue>")
        );
        assert!(output.source.contains(
            "::nw_network::serialize::DefaultMarshaler<::nw_network::source::ExampleValue>"
        ));
    }

    #[test]
    fn selected_struct_container_shape_mismatch_stays_blocked() {
        let value_type_id = uuid!("da4e5889-a65c-4480-8642-0278160125a7");
        let mut schema = NetworkSchema::from_ghidra_static_network_report(&json!({
            "registryEntries": [{
                "uuid": "B4DB39E2-5054-4604-9855-9A4DC75BDDE4",
                "typeIndex": 3362,
                "typeName": "MB::StructuredMapReplicatedState",
                "fields": [{
                    "index": 0,
                    "name": "valuesById",
                    "group": 0,
                    "handlerVtable": "NewWorld+0x81bf3d0",
                    "confidence": "register-field-call"
                }]
            }],
            "fieldRegistrationFunctions": [],
            "fieldHandlerVtables": [{
                "address": "NewWorld+0x81bf3d0",
                "fieldCount": 1,
                "deltaMarshalShapes": ["vlq-u32", "u32", "sequence-number", "u8", "u64"],
                "fullMarshalShapes": ["sequence-number", "vlq-u32", "u32", "u8", "u64"],
                "valueTypeName": "ExampleValue",
                "valueTypeId": value_type_id.to_string(),
                "slots": []
            }]
        }))
        .expect("schema");
        schema.merge_serialize_codegen_unit(
            &SerializeCodegenUnit {
                items: vec![example_value_item(
                    value_type_id,
                    [ScalarType::U64, ScalarType::U8],
                )],
            },
            Some("selection.json".to_owned()),
        );

        let output =
            NetworkRustEmitter::emit_replicated_states(&schema, [3362]).expect("state source");
        let plan = &output.report.state_generation_plans[0];
        let field = &plan.fields[0];

        assert!(!plan.can_generate);
        assert_eq!(plan.blocked_reasons, vec!["missing-semantic-type:1"]);
        assert_eq!(field.serialize_type_name.as_deref(), Some("ExampleValue"));
        assert_eq!(field.rust_field_type, None);
        assert_eq!(
            field.blocked_reason.as_deref(),
            Some("missing-semantic-type")
        );
    }

    #[test]
    fn container_value_type_shape_emits_order_specific_codec() {
        let value_type_id = uuid!("022d0c83-ee04-4e4d-9776-4dfbdaa90923");
        let schema = NetworkSchema::from_ghidra_static_network_report(&json!({
            "registryEntries": [{
                "uuid": "FD24C20B-FB95-49F8-9BB0-DEC472F0B6EA",
                "typeIndex": 205,
                "typeName": "MB::CraftingComponentReplicatedState",
                "fields": [{
                    "index": 0,
                    "name": "m_cooldowns",
                    "group": 0,
                    "handlerVtable": "NewWorld+0x8153630",
                    "confidence": "register-field-call"
                }]
            }],
            "fieldRegistrationFunctions": [],
            "fieldHandlerVtables": [{
                "address": "NewWorld+0x8153630",
                "fieldCount": 1,
                "deltaMarshalShapes": ["vlq-u32", "u32", "sequence-number", "u8", "u64"],
                "fullMarshalShapes": ["sequence-number", "vlq-u32", "u32", "u8", "u64"],
                "valueTypeInfoCandidates": [{
                    "address": "NewWorld+0x814f838",
                    "name": "RecipeCooldownData",
                    "typeId": value_type_id.to_string(),
                    "source": "rtti-provider-vtable",
                    "nameSource": "rtti-helper-function-name"
                }],
                "valueTypeShape": {
                    "typeId": value_type_id.to_string(),
                    "typeIdSource": "rtti-provider-vtable",
                    "typeName": "RecipeCooldownData",
                    "typeNameFull": "RecipeCooldownData",
                    "typeNameSource": "rtti-helper-function-name",
                    "azRttiAddress": "NewWorld+0x814f838",
                    "memberNameSource": "ghidra-datatype",
                    "memberNamesProven": true,
                    "datatypePath": "/RecipeCooldownData",
                    "validation": "container-value-datatype-layout",
                    "members": [{
                        "index": 0,
                        "offset": "0x8",
                        "name": "m_count",
                        "nameSource": "ghidra-datatype",
                        "nameProven": true,
                        "nativeType": "unsigned_char",
                        "wireShape": "u8",
                        "byteWidth": 1,
                        "evidenceSource": "container-value-datatype-member"
                    }, {
                        "index": 1,
                        "offset": "0x10",
                        "nativeOffset": "0x18",
                        "name": "m_cooldownEnd",
                        "nameSource": "ghidra-datatype",
                        "nameProven": true,
                        "nameEvidence": "m_nanosecondsSinceEpoc",
                        "nativeType": "WallClockTimePoint",
                        "wireShape": "u64",
                        "byteWidth": 16,
                        "evidenceSource": "container-value-nested-datatype-member"
                    }]
                },
                "slots": []
            }]
        }))
        .expect("schema");

        let output =
            NetworkRustEmitter::emit_replicated_states(&schema, [205]).expect("state source");
        let plan = &output.report.state_generation_plans[0];
        let field = &plan.fields[0];

        assert!(plan.can_generate, "{plan:#?}");
        assert_eq!(
            field.rust_value_type.as_deref(),
            Some(
                "::nw_network::serialize::IndexMap<u32, ::nw_network::source::RecipeCooldownData>"
            )
        );
        assert!(
            field
                .rust_field_type
                .as_deref()
                .is_some_and(|ty| ty.contains("CooldownsRecipeCooldownDataMarshaler"))
        );
        assert_eq!(
            field
                .container_value_type_shape
                .as_ref()
                .map(|shape| shape.validation.as_deref()),
            Some(Some("container-value-datatype-layout"))
        );
        assert!(
            output
                .source
                .contains("pub struct CooldownsRecipeCooldownDataMarshaler")
        );
        let compact_source = output.source.split_whitespace().collect::<String>();
        assert!(output.source.contains("value.count"));
        assert!(output.source.contains("value.cooldown_end"));
        assert!(compact_source.contains("::nw_network::WallClockTimePoint"));
        assert!(
            compact_source
                .contains("as::nw_network::serialize::Codec<::nw_network::WallClockTimePoint")
        );
        assert!(!compact_source.contains(
            "::nw_network::serialize::DefaultMarshaler<::nw_network::source::RecipeCooldownData>"
        ));
    }

    #[test]
    fn container_key_type_shape_emits_key_codec() {
        let schema = NetworkSchema::from_ghidra_static_network_report(&json!({
            "registryEntries": [{
                "uuid": "5E1977B4-E4C7-4F2A-8337-4BE775A9014C",
                "typeIndex": 3312,
                "typeName": "Javelin::GameModeParticipantReplicatedState",
                "fields": [{
                    "index": 0,
                    "name": "activeGameModes",
                    "group": 0,
                    "handlerVtable": "NewWorld+0x81b6fc8",
                    "confidence": "register-field-call"
                }]
            }],
            "fieldRegistrationFunctions": [],
            "fieldHandlerVtables": [{
                "address": "NewWorld+0x81b6fc8",
                "fieldCount": 1,
                "wireShape": "replicated-container<u32,u64>",
                "wireShapeSource": "replicated-container-marshal-calls",
                "containerShape": {
                    "storage": "map",
                    "keyWireShape": "u32",
                    "keyWireShapes": ["u32", "u8"],
                    "keyTypeShape": {
                        "typeName": "Key",
                        "typeNameSource": "container-structured-member-split",
                        "memberNameSource": "container-value-wire-sequence",
                        "memberNamesProven": true,
                        "validation": "custom-replicated-container-value-shape",
                        "members": [{
                            "index": 0,
                            "name": "game_mode_id",
                            "nameProven": true,
                            "wireShape": "u32"
                        }, {
                            "index": 1,
                            "name": "queue_index",
                            "nameProven": true,
                            "wireShape": "u8"
                        }]
                    },
                    "valueWireShapes": ["u64"],
                    "source": "replicated-container-map-structured-key-shape"
                },
                "slots": []
            }]
        }))
        .expect("schema");

        let output =
            NetworkRustEmitter::emit_replicated_states(&schema, [3312]).expect("state source");
        let plan = &output.report.state_generation_plans[0];
        let field = &plan.fields[0];

        assert!(plan.can_generate, "{plan:#?}");
        assert_eq!(
            field.rust_value_type.as_deref(),
            Some("::nw_network::serialize::IndexMap<ActiveGameModesKey, u64>")
        );
        assert!(field.rust_field_type.as_deref().is_some_and(|ty| {
            ty.contains("ActiveGameModesKeyMarshaler")
                && ty.contains("::nw_network::serialize::DefaultMarshaler<u64>")
        }));
        assert!(
            field
                .container_key_type_shape
                .as_ref()
                .is_some_and(|shape| shape.type_name.as_deref() == Some("Key"))
        );
        assert!(output.source.contains("pub struct ActiveGameModesKey"));
        assert!(
            output
                .source
                .contains("pub struct ActiveGameModesKeyMarshaler")
        );
        let compact_source = output.source.split_whitespace().collect::<String>();
        assert!(compact_source.contains("#[derive(Debug,Clone,Default,PartialEq,Eq,Hash)]"));
    }

    #[test]
    fn single_member_container_value_shape_is_not_flattened_to_scalar() {
        let value_type_id = uuid!("24fbf222-8cf9-4539-b313-34726b8fc675");
        let schema = NetworkSchema::from_ghidra_static_network_report(&json!({
            "registryEntries": [{
                "uuid": "5E1977B4-E4C7-4F2A-8337-4BE775A9014C",
                "typeIndex": 3312,
                "typeName": "Javelin::GameModeParticipantReplicatedState",
                "fields": [{
                    "index": 0,
                    "name": "queueEligibleTimesForGameModes",
                    "group": 0,
                    "handlerVtable": "NewWorld+0x81b6fc8",
                    "confidence": "register-field-call"
                }]
            }],
            "fieldRegistrationFunctions": [],
            "fieldHandlerVtables": [{
                "address": "NewWorld+0x81b6fc8",
                "fieldCount": 1,
                "wireShape": "replicated-container<u32,u64>",
                "wireShapeSource": "replicated-container-marshal-calls",
                "deltaMarshalShapes": ["vlq-u32", "vlq-u32"],
                "fullMarshalShapes": ["sequence-number", "vlq-u32", "u32", "u64", "vlq-u32"],
                "valueTypeInfoCandidates": [{
                    "address": "NewWorld+0x802f940",
                    "name": "WallClockTimePoint",
                    "typeId": value_type_id.to_string(),
                    "source": "rtti-provider-vtable",
                    "nameSource": "rtti-helper-function-name"
                }],
                "valueTypeShape": {
                    "typeId": value_type_id.to_string(),
                    "typeIdSource": "rtti-provider-vtable",
                    "typeName": "WallClockTimePoint",
                    "typeNameFull": "WallClockTimePoint",
                    "typeNameSource": "rtti-helper-function-name",
                    "azRttiAddress": "NewWorld+0x802f940",
                    "memberNameSource": "serialize-json-offset-match",
                    "memberNamesProven": true,
                    "validation": "container-value-pcode-wire-order-serialize-layout",
                    "members": [{
                        "index": 0,
                        "offset": "0x8",
                        "nativeOffset": "0x8",
                        "name": "m_nanosecondsSinceEpoc",
                        "nameSource": "serialize-json-offset-match",
                        "nameProven": true,
                        "nativeType": "AZStd::ranged_int",
                        "wireShape": "u64",
                        "byteWidth": 8,
                        "evidenceSource": "container-value-pcode-call-scalar-output-store+serialize-json-field"
                    }]
                },
                "slots": []
            }]
        }))
        .expect("schema");

        let output =
            NetworkRustEmitter::emit_replicated_states(&schema, [3312]).expect("state source");
        let plan = &output.report.state_generation_plans[0];
        let field = &plan.fields[0];

        assert!(plan.can_generate, "{plan:#?}");
        assert_eq!(
            field.rust_value_type.as_deref(),
            Some("::nw_network::serialize::IndexMap<u32, ::nw_network::WallClockTimePoint>")
        );
        assert!(field.rust_field_type.as_deref().is_some_and(|ty| {
            ty.contains("QueueEligibleTimesForGameModesWallClockTimePointMarshaler")
        }));
        assert!(
            field
                .container_value_type_shape
                .as_ref()
                .is_some_and(|shape| shape.type_name.as_deref() == Some("WallClockTimePoint"))
        );
        assert!(!output.source.contains("IndexMap<u32, u64>"));
    }

    #[test]
    fn bool_backed_container_value_member_keeps_bool_rust_field() {
        let value_type_id = uuid!("b715b520-5fc0-4245-84e7-7d974b8410f8");
        let schema = NetworkSchema::from_ghidra_static_network_report(&json!({
            "registryEntries": [{
                "uuid": "C5021E27-01D8-4E31-87E8-51E00506E07B",
                "typeIndex": 1525,
                "typeName": "MB::StatMultiplierTableComponentReplicatedState",
                "fields": [{
                    "index": 0,
                    "name": "multiplierTable",
                    "group": 1,
                    "handlerVtable": "NewWorld+0x82f3038",
                    "confidence": "register-field-call"
                }]
            }],
            "fieldRegistrationFunctions": [],
            "fieldHandlerVtables": [{
                "address": "NewWorld+0x82f3038",
                "fieldCount": 2,
                "deltaMarshalShapes": ["vlq-u32", "vlq-u32"],
                "fullMarshalShapes": ["sequence-number", "vlq-u32", "u8", "u32", "u8", "vlq-u32"],
                "valueTypeInfoCandidates": [{
                    "address": "NewWorld+0x82f2710",
                    "name": "StatMultiplierData",
                    "typeId": value_type_id.to_string(),
                    "source": "rtti-provider-vtable",
                    "nameSource": "rtti-helper-function-name"
                }],
                "valueTypeShape": {
                    "typeId": value_type_id.to_string(),
                    "typeIdSource": "rtti-provider-vtable",
                    "typeName": "StatMultiplierData",
                    "typeNameFull": "StatMultiplierData",
                    "typeNameSource": "rtti-helper-function-name",
                    "azRttiAddress": "NewWorld+0x82f2710",
                    "memberNameSource": "serialize-json-offset-match",
                    "memberNamesProven": true,
                    "validation": "container-value-pcode-wire-order-serialize-layout",
                    "members": [{
                        "index": 0,
                        "offset": "0xc",
                        "nativeOffset": "0xc",
                        "name": "m_value",
                        "nameSource": "serialize-json-offset-match",
                        "nameProven": true,
                        "nativeType": "int",
                        "wireShape": "u32",
                        "byteWidth": 4,
                        "evidenceSource": "container-value-pcode-call+serialize-json-field"
                    }, {
                        "index": 1,
                        "offset": "0x8",
                        "nativeOffset": "0x8",
                        "name": "m_syncVitals",
                        "nameSource": "serialize-json-offset-match",
                        "nameProven": true,
                        "nativeType": "bool",
                        "wireShape": "u8",
                        "byteWidth": 1,
                        "evidenceSource": "container-value-pcode-call+serialize-json-field"
                    }]
                },
                "slots": []
            }]
        }))
        .expect("schema");

        let output =
            NetworkRustEmitter::emit_replicated_states(&schema, [1525]).expect("state source");
        let plan = &output.report.state_generation_plans[0];
        let field = &plan.fields[0];

        assert!(plan.can_generate, "{plan:#?}");
        assert_eq!(
            field.rust_value_type.as_deref(),
            Some("::nw_network::serialize::IndexMap<u8, ::nw_network::source::StatMultiplierData>")
        );
        assert!(
            field
                .rust_field_type
                .as_deref()
                .is_some_and(|ty| { ty.contains("MultiplierTableStatMultiplierDataMarshaler") })
        );
        let compact_source = output.source.split_whitespace().collect::<String>();
        assert!(compact_source.contains("value.sync_vitals"));
        assert!(compact_source.contains("DefaultMarshaler<bool"));
        assert!(compact_source.contains("value:field_value"));
        assert!(compact_source.contains("sync_vitals:field_sync_vitals"));
    }

    #[test]
    fn single_member_source_container_value_keeps_projected_source_type() {
        let value_type_id = uuid!("b715b520-5fc0-4245-84e7-7d974b8410f8");
        let schema = NetworkSchema::from_ghidra_static_network_report(&json!({
            "registryEntries": [{
                "uuid": "C5021E27-01D8-4E31-87E8-51E00506E07B",
                "typeIndex": 1525,
                "typeName": "MB::StatMultiplierTableComponentReplicatedState",
                "fields": [{
                    "index": 0,
                    "name": "multiplierTable",
                    "group": 1,
                    "handlerVtable": "NewWorld+0x82f3038",
                    "confidence": "register-field-call"
                }]
            }],
            "fieldRegistrationFunctions": [],
            "fieldHandlerVtables": [{
                "address": "NewWorld+0x82f3038",
                "fieldCount": 2,
                "handlerKind": "replicated-container",
                "deltaMarshalShapes": ["bool", "vlq-u32"],
                "fullMarshalShapes": ["sequence-number", "bool", "vlq-u32"],
                "valueTypeInfoCandidates": [{
                    "address": "NewWorld+0x82f2710",
                    "name": "StatMultiplierData",
                    "typeId": value_type_id.to_string(),
                    "source": "rtti-provider-vtable",
                    "nameSource": "rtti-helper-function-name"
                }],
                "valueTypeShape": {
                    "typeId": value_type_id.to_string(),
                    "typeIdSource": "rtti-provider-vtable",
                    "typeName": "StatMultiplierData",
                    "typeNameFull": "StatMultiplierData",
                    "typeNameSource": "rtti-helper-function-name",
                    "azRttiAddress": "NewWorld+0x82f2710",
                    "memberNameSource": "ghidra-datatype",
                    "memberNamesProven": true,
                    "validation": "container-value-datatype-layout",
                    "members": [{
                        "index": 0,
                        "offset": "0x8",
                        "name": "m_syncVitals",
                        "nameSource": "ghidra-datatype",
                        "nameProven": true,
                        "nativeType": "bool",
                        "wireShape": "bool",
                        "byteWidth": 1,
                        "evidenceSource": "container-value-datatype-member"
                    }]
                },
                "slots": []
            }]
        }))
        .expect("schema");

        let output =
            NetworkRustEmitter::emit_replicated_states(&schema, [1525]).expect("state source");
        let plan = &output.report.state_generation_plans[0];
        let field = &plan.fields[0];

        assert!(plan.can_generate, "{plan:#?}");
        assert_eq!(
            field.rust_value_type.as_deref(),
            Some("::std::vec::Vec<::nw_network::source::StatMultiplierData>")
        );
        assert!(
            field
                .rust_field_type
                .as_deref()
                .is_some_and(|ty| { ty.contains("MultiplierTableStatMultiplierDataMarshaler") })
        );
        let compact_source = output.source.split_whitespace().collect::<String>();
        assert!(compact_source.contains("value.sync_vitals"));
        assert!(compact_source.contains("sync_vitals:field_sync_vitals"));
        assert!(compact_source.contains("::core::default::Default>::default()"));
        assert!(
            !compact_source.contains("DefaultMarshaler<::nw_network::source::StatMultiplierData>")
        );
    }

    #[test]
    fn vector_container_shape_emits_vec_storage() {
        let schema = NetworkSchema::from_ghidra_static_network_report(&json!({
            "registryEntries": [{
                "uuid": "B4DB39E2-5054-4604-9855-9A4DC75BDDE4",
                "typeIndex": 3362,
                "typeName": "MB::ScalarVecReplicatedState",
                "fields": [{
                    "index": 0,
                    "name": "values",
                    "group": 0,
                    "handlerVtable": "NewWorld+0x81bf3d0",
                    "confidence": "register-field-call"
                }]
            }],
            "fieldRegistrationFunctions": [],
            "fieldHandlerVtables": [{
                "address": "NewWorld+0x81bf3d0",
                "fieldCount": 1,
                "wireShape": "replicated-container<vlq-u64,u64>",
                "wireShapeSource": "replicated-container-marshal-calls",
                "deltaMarshalShapes": ["vlq-u32", "vlq-u64", "sequence-number", "u64"],
                "fullMarshalShapes": ["sequence-number", "vlq-u32", "u64"],
                "slots": []
            }]
        }))
        .expect("schema");

        let output =
            NetworkRustEmitter::emit_replicated_states(&schema, [3362]).expect("state source");
        let plan = &output.report.state_generation_plans[0];
        let field = &plan.fields[0];

        assert!(plan.can_generate, "{plan:#?}");
        assert_eq!(field.wire_shape, None);
        assert_eq!(
            field.rust_value_type.as_deref(),
            Some("::std::vec::Vec<u64>")
        );
        assert!(
            field
                .rust_field_type
                .as_deref()
                .is_some_and(|ty| ty.contains("ReplicatedContainer<::std::vec::Vec<u64>"))
        );
    }

    #[test]
    fn native_only_structured_vector_container_emits_local_value_type() {
        let value_type_id = uuid!("fdda118c-1c41-48a4-af1c-b45fd6797fbe");
        let beam_value_shapes = [
            "u8", "u32", "u64", "f32", "f32", "f32", "f32", "f32", "f32", "f32", "f32", "f32",
            "f32", "f32", "f32", "f32", "f32", "f32", "vlq-u32",
        ];
        let dynamic_value_shapes = [
            "u32", "u8", "u32", "u64", "f32", "f32", "f32", "f32", "f32", "f32", "f32", "f32",
            "f32", "f32", "f32", "f32", "f32", "f32", "f32", "vlq-u32",
        ];
        let mut beam_delta_shapes = vec!["vlq-u32", "vlq-u64", "sequence-number"];
        beam_delta_shapes.extend(beam_value_shapes);
        let mut beam_full_shapes = vec!["sequence-number", "vlq-u32"];
        beam_full_shapes.extend(beam_value_shapes);
        let mut dynamic_delta_shapes = vec!["vlq-u32", "vlq-u64", "sequence-number"];
        dynamic_delta_shapes.extend(dynamic_value_shapes);
        let mut dynamic_full_shapes = vec!["sequence-number", "vlq-u32"];
        dynamic_full_shapes.extend(dynamic_value_shapes);
        let schema = NetworkSchema::from_ghidra_static_network_report(&json!({
            "registryEntries": [{
                "uuid": "CB9D9BE8-9C90-494C-9324-F17689B1B635",
                "typeIndex": 2947,
                "typeName": "MB::BeamAttackComponentReplicatedState",
                "fragmentMetadata": {
                    "category": "Uncategorized",
                    "categoryValue": 0,
                    "isMetadata": false
                },
                "fields": [{
                    "index": 0,
                    "name": "beamData",
                    "group": 0,
                    "handlerVtable": "NewWorld+0x854e440",
                    "confidence": "register-field-call"
                }, {
                    "index": 1,
                    "name": "dynamicBeamData",
                    "group": 0,
                    "handlerVtable": "NewWorld+0x854e4e0",
                    "confidence": "register-field-call"
                }, {
                    "index": 2,
                    "name": "aiTargetGDERef",
                    "group": 0,
                    "handlerVtable": "NewWorld+0x818cee8",
                    "confidence": "register-field-call"
                }]
            }],
            "fieldRegistrationFunctions": [],
            "fieldHandlerVtables": [{
                "address": "NewWorld+0x854e440",
                "fieldCount": 1,
                "handlerKind": "replicated-container",
                "deltaMarshalShapes": beam_delta_shapes,
                "fullMarshalShapes": beam_full_shapes,
                "valueTypeInfoCandidates": [{
                    "address": "NewWorld+0x8540380",
                    "name": "BeamAttackData_Replicated",
                    "typeId": value_type_id.to_string(),
                    "source": "rtti-provider-vtable",
                    "nameSource": "az-rtti-provider-table"
                }],
                "valueTypeShape": {
                    "typeId": value_type_id.to_string(),
                    "typeIdSource": "rtti-provider-vtable",
                    "typeName": "BeamAttackData_Replicated",
                    "typeNameFull": "BeamAttackData_Replicated",
                    "typeNameSource": "az-rtti-provider-table",
                    "azRttiAddress": "NewWorld+0x8540380",
                    "memberNameSource": "synthetic-pcode-wire-order",
                    "memberNamesProven": false,
                    "validation": "container-value-pcode-wire-order-native-rtti",
                    "members": [{
                        "index": 0,
                        "offset": "0x10",
                        "nativeOffset": "0x10",
                        "name": "field_0",
                        "nameSource": "synthetic-pcode-wire-order",
                        "nameProven": false,
                        "nativeType": "bool",
                        "wireShape": "u8",
                        "byteWidth": 1,
                        "evidenceSource": "container-value-pcode-call+native-rtti-synthetic-field"
                    }, {
                        "index": 1,
                        "offset": "0x20",
                        "nativeOffset": "0x20",
                        "name": "field_1",
                        "nameSource": "synthetic-pcode-wire-order",
                        "nameProven": false,
                        "nativeType": "AZ::u32",
                        "wireShape": "u32",
                        "byteWidth": 4,
                        "evidenceSource": "container-value-pcode-call+native-rtti-synthetic-field"
                    }, {
                        "index": 2,
                        "offset": "0x18",
                        "nativeOffset": "0x18",
                        "name": "field_2",
                        "nameSource": "synthetic-pcode-wire-order",
                        "nameProven": false,
                        "nativeType": "AZ::u64",
                        "wireShape": "u64",
                        "byteWidth": 8,
                        "evidenceSource": "container-value-pcode-call+native-rtti-synthetic-field"
                    }, {
                        "index": 3,
                        "offset": "0x30",
                        "nativeOffset": "0x30",
                        "name": "field_3",
                        "nameSource": "synthetic-pcode-wire-order",
                        "nameProven": false,
                        "nativeType": "AZ::Vector3",
                        "wireShape": "vec3",
                        "byteWidth": 12,
                        "evidenceSource": "container-value-pcode-call+native-rtti-synthetic-field"
                    }, {
                        "index": 4,
                        "offset": "0x40",
                        "nativeOffset": "0x40",
                        "name": "field_4",
                        "nameSource": "synthetic-pcode-wire-order",
                        "nameProven": false,
                        "nativeType": "AZ::Vector3",
                        "wireShape": "vec3",
                        "byteWidth": 12,
                        "evidenceSource": "container-value-pcode-call+native-rtti-synthetic-field"
                    }, {
                        "index": 5,
                        "offset": "0x50",
                        "nativeOffset": "0x50",
                        "name": "field_5",
                        "nameSource": "synthetic-pcode-wire-order",
                        "nameProven": false,
                        "nativeType": "AZ::Vector3",
                        "wireShape": "vec3",
                        "byteWidth": 12,
                        "evidenceSource": "container-value-pcode-call+native-rtti-synthetic-field"
                    }, {
                        "index": 6,
                        "offset": "0x60",
                        "nativeOffset": "0x60",
                        "name": "field_6",
                        "nameSource": "synthetic-pcode-wire-order",
                        "nameProven": false,
                        "nativeType": "AZ::Vector3",
                        "wireShape": "vec3",
                        "byteWidth": 12,
                        "evidenceSource": "container-value-pcode-call+native-rtti-synthetic-field"
                    }, {
                        "index": 7,
                        "offset": "0x70",
                        "nativeOffset": "0x70",
                        "name": "field_7",
                        "nameSource": "synthetic-pcode-wire-order",
                        "nameProven": false,
                        "nativeType": "AZ::Vector3",
                        "wireShape": "vec3",
                        "byteWidth": 12,
                        "evidenceSource": "container-value-pcode-call+native-rtti-synthetic-field"
                    }, {
                        "index": 8,
                        "offset": "0x80",
                        "nativeOffset": "0x80",
                        "name": "field_8",
                        "nameSource": "synthetic-pcode-wire-order",
                        "nameProven": false,
                        "nativeType": "AZStd::vector<AZ::u64>",
                        "wireShape": "vec<u64>",
                        "byteWidth": 24,
                        "evidenceSource": "container-value-pcode-collection-output+native-rtti-synthetic-field"
                    }]
                },
                "slots": []
            }, {
                "address": "NewWorld+0x854e4e0",
                "fieldCount": 1,
                "handlerKind": "replicated-container",
                "deltaMarshalShapes": dynamic_delta_shapes,
                "fullMarshalShapes": dynamic_full_shapes,
                "valueTypeInfoCandidates": [{
                    "address": "NewWorld+0x8540380",
                    "name": "BeamAttackData_Replicated",
                    "typeId": value_type_id.to_string(),
                    "source": "rtti-provider-vtable",
                    "nameSource": "az-rtti-provider-table"
                }],
                "valueTypeShape": {
                    "typeId": value_type_id.to_string(),
                    "typeIdSource": "rtti-provider-vtable",
                    "typeName": "BeamAttackData_Replicated",
                    "typeNameFull": "BeamAttackData_Replicated",
                    "typeNameSource": "az-rtti-provider-table",
                    "azRttiAddress": "NewWorld+0x8540380",
                    "memberNameSource": "synthetic-pcode-wire-order",
                    "memberNamesProven": false,
                    "validation": "container-value-pcode-wire-order-native-rtti",
                    "members": [{
                        "index": 0,
                        "offset": "0x0",
                        "nativeOffset": "0x0",
                        "name": "field_0",
                        "nameSource": "synthetic-pcode-wire-order",
                        "nameProven": false,
                        "nativeType": "AZ::u32",
                        "wireShape": "u32",
                        "byteWidth": 4,
                        "evidenceSource": "container-value-pcode-call+native-rtti-synthetic-field"
                    }, {
                        "index": 1,
                        "offset": "0x10",
                        "nativeOffset": "0x10",
                        "name": "field_1",
                        "nameSource": "synthetic-pcode-wire-order",
                        "nameProven": false,
                        "nativeType": "bool",
                        "wireShape": "u8",
                        "byteWidth": 1,
                        "evidenceSource": "container-value-pcode-call+native-rtti-synthetic-field"
                    }, {
                        "index": 2,
                        "offset": "0x20",
                        "nativeOffset": "0x20",
                        "name": "field_2",
                        "nameSource": "synthetic-pcode-wire-order",
                        "nameProven": false,
                        "nativeType": "AZ::u32",
                        "wireShape": "u32",
                        "byteWidth": 4,
                        "evidenceSource": "container-value-pcode-call+native-rtti-synthetic-field"
                    }, {
                        "index": 3,
                        "offset": "0x18",
                        "nativeOffset": "0x18",
                        "name": "field_3",
                        "nameSource": "synthetic-pcode-wire-order",
                        "nameProven": false,
                        "nativeType": "AZ::u64",
                        "wireShape": "u64",
                        "byteWidth": 8,
                        "evidenceSource": "container-value-pcode-call+native-rtti-synthetic-field"
                    }, {
                        "index": 4,
                        "offset": "0x30",
                        "nativeOffset": "0x30",
                        "name": "field_4",
                        "nameSource": "synthetic-pcode-wire-order",
                        "nameProven": false,
                        "nativeType": "AZ::Vector3",
                        "wireShape": "vec3",
                        "byteWidth": 12,
                        "evidenceSource": "container-value-pcode-call+native-rtti-synthetic-field"
                    }, {
                        "index": 5,
                        "offset": "0x40",
                        "nativeOffset": "0x40",
                        "name": "field_5",
                        "nameSource": "synthetic-pcode-wire-order",
                        "nameProven": false,
                        "nativeType": "AZ::Vector3",
                        "wireShape": "vec3",
                        "byteWidth": 12,
                        "evidenceSource": "container-value-pcode-call+native-rtti-synthetic-field"
                    }, {
                        "index": 6,
                        "offset": "0x50",
                        "nativeOffset": "0x50",
                        "name": "field_6",
                        "nameSource": "synthetic-pcode-wire-order",
                        "nameProven": false,
                        "nativeType": "AZ::Vector3",
                        "wireShape": "vec3",
                        "byteWidth": 12,
                        "evidenceSource": "container-value-pcode-call+native-rtti-synthetic-field"
                    }, {
                        "index": 7,
                        "offset": "0x60",
                        "nativeOffset": "0x60",
                        "name": "field_7",
                        "nameSource": "synthetic-pcode-wire-order",
                        "nameProven": false,
                        "nativeType": "AZ::Vector3",
                        "wireShape": "vec3",
                        "byteWidth": 12,
                        "evidenceSource": "container-value-pcode-call+native-rtti-synthetic-field"
                    }, {
                        "index": 8,
                        "offset": "0x70",
                        "nativeOffset": "0x70",
                        "name": "field_8",
                        "nameSource": "synthetic-pcode-wire-order",
                        "nameProven": false,
                        "nativeType": "AZ::Vector3",
                        "wireShape": "vec3",
                        "byteWidth": 12,
                        "evidenceSource": "container-value-pcode-call+native-rtti-synthetic-field"
                    }, {
                        "index": 9,
                        "offset": "0x80",
                        "nativeOffset": "0x80",
                        "name": "field_9",
                        "nameSource": "synthetic-pcode-wire-order",
                        "nameProven": false,
                        "nativeType": "AZStd::vector<AZ::u64>",
                        "wireShape": "vec<u64>",
                        "byteWidth": 24,
                        "evidenceSource": "container-value-pcode-collection-output+native-rtti-synthetic-field"
                    }]
                },
                "slots": []
            }, {
                "address": "NewWorld+0x818cee8",
                "fieldCount": 1,
                "wireShape": "u64",
                "wireShapeSource": "marshal-call:marshal-function-name",
                "slots": []
            }]
        }))
        .expect("schema");

        let output =
            NetworkRustEmitter::emit_replicated_states(&schema, [2947]).expect("state source");
        let plan = &output.report.state_generation_plans[0];

        assert!(plan.can_generate, "{plan:#?}");
        assert_eq!(plan.fields[0].field_name.as_deref(), Some("beamData"));
        assert_eq!(
            plan.fields[0].rust_value_type.as_deref(),
            Some("::std::vec::Vec<BeamDataBeamAttackDataReplicated>")
        );
        assert_eq!(
            plan.fields[1].rust_value_type.as_deref(),
            Some("::std::vec::Vec<DynamicBeamDataBeamAttackDataReplicated>")
        );
        let compact_source = output.source.split_whitespace().collect::<String>();
        assert!(
            output
                .source
                .contains("pub struct BeamDataBeamAttackDataReplicated")
        );
        assert!(
            output
                .source
                .contains("pub struct DynamicBeamDataBeamAttackDataReplicated")
        );
        assert!(compact_source.contains("pubfield_3:::glam::Vec3"));
        assert!(compact_source.contains("pubfield_8:::std::vec::Vec<u64>"));
        assert!(compact_source.contains("DefaultMarshaler<::std::vec::Vec<u64>"));
        assert!(!output.source.contains("[f32; 3]"));
        assert!(!compact_source.contains("[f32;3]"));
        assert!(
            !output
                .source
                .contains("::nw_network::source::BeamAttackDataReplicated")
        );
        assert!(
            compact_source
                .contains("ReplicatedContainer<::std::vec::Vec<BeamDataBeamAttackDataReplicated>")
        );
    }

    #[test]
    fn provider_value_type_candidates_do_not_force_container_generation() {
        let recipe_cooldown_id = uuid!("022d0c83-ee04-4e4d-9776-4dfbdaa90923");
        let schema = NetworkSchema::from_ghidra_static_network_report(&json!({
            "registryEntries": [{
                "uuid": "B4DB39E2-5054-4604-9855-9A4DC75BDDE4",
                "typeIndex": 3362,
                "typeName": "MB::CraftingComponentReplicatedState",
                "fields": [{
                    "index": 0,
                    "name": "recipeCooldowns",
                    "group": 0,
                    "handlerVtable": "NewWorld+0x81bf3d0",
                    "confidence": "register-field-call"
                }]
            }],
            "fieldRegistrationFunctions": [],
            "fieldHandlerVtables": [{
                "address": "NewWorld+0x81bf3d0",
                "fieldCount": 1,
                "wireShape": "replicated-container<u32,u8>",
                "wireShapeSource": "replicated-container-marshal-calls",
                "deltaMarshalShapes": ["vlq-u32", "u32", "sequence-number", "u8"],
                "fullMarshalShapes": ["sequence-number", "vlq-u32", "u32", "u8", "u64", "u16"],
                "valueTypeInfoCandidates": [{
                    "address": "NewWorld+0x8123450",
                    "name": "RecipeCooldownData",
                    "typeId": recipe_cooldown_id.to_string(),
                    "source": "rtti-provider-vtable"
                }],
                "slots": []
            }]
        }))
        .expect("schema");

        let output =
            NetworkRustEmitter::emit_replicated_states(&schema, [3362]).expect("state source");
        let plan = &output.report.state_generation_plans[0];
        let field = &plan.fields[0];

        assert!(!plan.can_generate);
        assert_eq!(plan.blocked_reasons, vec!["missing-semantic-type:1"]);
        assert_eq!(field.wire_shape, None);
        assert_eq!(
            field.blocked_reason.as_deref(),
            Some("missing-semantic-type")
        );
        assert_eq!(field.value_type_candidates.len(), 1);
        assert_eq!(
            field.value_type_candidates[0].name.as_deref(),
            Some("RecipeCooldownData")
        );
        assert_eq!(
            field.value_type_candidates[0].type_id,
            Some(recipe_cooldown_id)
        );
        assert_eq!(field.rust_value_type, None);
        assert_eq!(field.rust_field_type, None);
        assert!(!output.source.contains("CraftingComponentReplicatedState"));
    }

    #[test]
    fn provider_value_type_candidate_matching_serialize_shape_emits_container_generation() {
        let recipe_cooldown_id = uuid!("022d0c83-ee04-4e4d-9776-4dfbdaa90923");
        let mut schema = NetworkSchema::from_ghidra_static_network_report(&json!({
            "registryEntries": [{
                "uuid": "B4DB39E2-5054-4604-9855-9A4DC75BDDE4",
                "typeIndex": 3362,
                "typeName": "MB::CraftingComponentReplicatedState",
                "fields": [{
                    "index": 0,
                    "name": "recipeCooldowns",
                    "group": 0,
                    "handlerVtable": "NewWorld+0x81bf3d0",
                    "confidence": "register-field-call"
                }]
            }],
            "fieldRegistrationFunctions": [],
            "fieldHandlerVtables": [{
                "address": "NewWorld+0x81bf3d0",
                "fieldCount": 1,
                "deltaMarshalShapes": ["vlq-u32", "u32", "sequence-number", "u8", "u64"],
                "fullMarshalShapes": ["sequence-number", "vlq-u32", "u32", "u8", "u64"],
                "valueTypeInfoCandidates": [{
                    "address": "NewWorld+0x8123450",
                    "name": "RecipeCooldownData",
                    "typeId": recipe_cooldown_id.to_string(),
                    "source": "rtti-provider-vtable"
                }],
                "slots": []
            }]
        }))
        .expect("schema");
        schema.merge_serialize_codegen_unit(
            &SerializeCodegenUnit {
                items: vec![named_value_item(
                    recipe_cooldown_id,
                    "RecipeCooldownData",
                    [ScalarType::U8, ScalarType::U64],
                )],
            },
            Some("selection.json".to_owned()),
        );

        let output =
            NetworkRustEmitter::emit_replicated_states(&schema, [3362]).expect("state source");
        let plan = &output.report.state_generation_plans[0];
        let field = &plan.fields[0];

        assert!(plan.can_generate);
        assert_eq!(
            field.rust_value_type.as_deref(),
            Some(
                "::nw_network::serialize::IndexMap<u32, ::nw_network::source::RecipeCooldownData>"
            )
        );
        assert!(field.rust_field_type.as_deref().is_some_and(|ty| {
            ty.contains("DefaultMarshaler<::nw_network::source::RecipeCooldownData>")
        }));
    }

    #[test]
    fn provider_candidates_can_split_key_value_around_terminal_count() {
        let task_id = uuid!("e1838273-034d-47fb-b535-95ff1d52d8ee");
        let time_id = uuid!("24fbf222-8cf9-4539-b313-34726b8fc675");
        let mut schema = NetworkSchema::from_ghidra_static_network_report(&json!({
            "registryEntries": [{
                "uuid": "AEFEDE43-4D48-42ED-81F8-7FF1E8D4D120",
                "typeIndex": 3857,
                "typeName": "Javelin::ObjectivesComponentReplicatedState",
                "fields": [{
                    "index": 0,
                    "name": "taskStartTimes",
                    "group": 0,
                    "handlerVtable": "NewWorld+0x8258560",
                    "confidence": "register-field-call"
                }]
            }],
            "fieldRegistrationFunctions": [],
            "fieldHandlerVtables": [{
                "address": "NewWorld+0x8258560",
                "fieldCount": 1,
                "handlerKind": "replicated-container",
                "fullMarshalShapes": [
                    "sequence-number",
                    "vlq-u32",
                    "u64",
                    "u8",
                    "u64",
                    "vlq-u32"
                ],
                "valueTypeInfoCandidates": [{
                    "address": "NewWorld+0x802f940",
                    "name": "WallClockTimePoint",
                    "typeId": time_id.to_string(),
                    "source": "rtti-provider-vtable"
                }, {
                    "address": "NewWorld+0x80cb690",
                    "name": "ObjectiveTaskInstanceId",
                    "typeId": task_id.to_string(),
                    "source": "rtti-provider-vtable"
                }],
                "slots": []
            }]
        }))
        .expect("schema");
        schema.merge_serialize_codegen_unit(
            &SerializeCodegenUnit {
                items: vec![
                    named_value_item(
                        task_id,
                        "ObjectiveTaskInstanceId",
                        [ScalarType::U64, ScalarType::U8],
                    ),
                    named_value_item(time_id, "WallClockTimePoint", [ScalarType::U64]),
                ],
            },
            Some("selection.json".to_owned()),
        );
        let serialize_types = serialize_types_by_type_id(&schema);
        let vtable = schema
            .field_handler_vtables
            .iter()
            .find(|vtable| vtable.address.as_deref() == Some("NewWorld+0x8258560"))
            .expect("handler vtable");
        let inferred = candidate_backed_container_shape_from_vtable(vtable, &serialize_types)
            .expect("candidate-backed container shape");
        assert_eq!(
            inferred.key_type_name.as_deref(),
            Some("ObjectiveTaskInstanceId")
        );
        assert_eq!(
            inferred.value_type_name.as_deref(),
            Some("WallClockTimePoint")
        );

        let output =
            NetworkRustEmitter::emit_replicated_states(&schema, [3857]).expect("state source");
        let plan = &output.report.state_generation_plans[0];
        let field = &plan.fields[0];

        assert!(plan.can_generate, "{plan:#?}");
        assert_eq!(
            field.rust_value_type.as_deref(),
            Some(
                "::nw_network::serialize::IndexMap<::nw_network::source::ObjectiveTaskInstanceId, ::nw_network::WallClockTimePoint>"
            )
        );
        assert!(field.rust_field_type.as_deref().is_some_and(|ty| {
            ty.contains("DefaultMarshaler<::nw_network::source::ObjectiveTaskInstanceId>")
                && ty.contains("DefaultMarshaler<::nw_network::WallClockTimePoint>")
        }));
    }

    #[test]
    fn selected_structured_container_with_partial_delta_uses_full_value_codec() {
        let value_type_id = uuid!("0dc02dd0-993e-48c0-8b60-5715d4383b0d");
        let mut schema = NetworkSchema::from_ghidra_static_network_report(&json!({
            "registryEntries": [{
                "uuid": "111AEBB0-4F23-4914-B732-A349CCBD82D4",
                "typeIndex": 3780,
                "typeName": "Javelin::GlobalMapDataManagerComponentReplicatedState",
                "fields": [{
                    "index": 0,
                    "name": "globalMapData",
                    "group": 0,
                    "handlerVtable": "NewWorld+0x8223838",
                    "confidence": "register-field-call"
                }]
            }],
            "fieldRegistrationFunctions": [],
            "fieldHandlerVtables": [{
                "address": "NewWorld+0x8223838",
                "fieldCount": 1,
                "deltaMarshalShapes": ["vlq-u32", "u64", "sequence-number", "u8"],
                "fullMarshalShapes": [
                    "sequence-number",
                    "vlq-u32",
                    "u64",
                    "vec2",
                    "u16",
                    "u32"
                ],
                "valueTypeName": "GlobalMapData",
                "valueTypeId": value_type_id.to_string(),
                "slots": []
            }]
        }))
        .expect("schema");
        schema.merge_serialize_codegen_unit(
            &SerializeCodegenUnit {
                items: vec![named_value_item(
                    value_type_id,
                    "GlobalMapData",
                    [ScalarType::Vector2, ScalarType::U16, ScalarType::U32],
                )],
            },
            Some("selection.json".to_owned()),
        );

        let output =
            NetworkRustEmitter::emit_replicated_states(&schema, [3780]).expect("state source");
        let plan = &output.report.state_generation_plans[0];
        let field = &plan.fields[0];

        assert!(plan.can_generate);
        assert!(plan.blocked_reasons.is_empty());
        assert_eq!(
            field.rust_value_type.as_deref(),
            Some("::nw_network::serialize::IndexMap<u64, ::nw_network::source::GlobalMapData>")
        );
        assert!(field.rust_field_type.as_deref().is_some_and(|ty| {
            ty.contains("DefaultMarshaler<::nw_network::source::GlobalMapData>")
        }));
    }

    #[test]
    fn fixed_key_container_with_source_vector_value_uses_vec_codec() {
        let persistent_item_data_id = uuid!("1be36174-fd4f-4a1c-8e52-7c28d50eec5a");
        let mut schema = NetworkSchema::from_ghidra_static_network_report(&json!({
            "registryEntries": [{
                "uuid": "393D9FE0-8E0F-41E9-8FE0-A2C33EF9C7C2",
                "typeIndex": 2938,
                "typeName": "MB::GlobalStorageComponentReplicatedState",
                "fields": [{
                    "index": 0,
                    "name": "m_globalItemMap",
                    "group": 0,
                    "handlerVtable": "NewWorld+0x813bb88",
                    "confidence": "register-field-call"
                }]
            }],
            "fieldRegistrationFunctions": [],
            "fieldHandlerVtables": [{
                "address": "NewWorld+0x813bb88",
                "fieldCount": 1,
                "deltaMarshalShapes": [
                    "vlq-u32",
                    "u8",
                    "fixed-bytes-16",
                    "sequence-number",
                    "vlq-u32",
                    "u64"
                ],
                "fullMarshalShapes": [
                    "sequence-number",
                    "vlq-u32",
                    "fixed-bytes-16",
                    "vlq-u32",
                    "u64"
                ],
                "valueTypeShape": {
                    "typeName": "AZStd::vector<PersistentItemData>",
                    "typeNameFull": "AZStd::vector<PersistentItemData>",
                    "typeNameSource": "marshal-helper-callgraph",
                    "memberNameSource": "container-value-shape",
                    "memberNamesProven": false,
                    "validation": "custom-container-value-pcode-serialize-type-sequence-persistent-item-data-vector",
                    "members": [{
                        "index": 0,
                        "offset": "0x0",
                        "nativeOffset": "0x0",
                        "name": "items",
                        "nameSource": "synthetic-offset",
                        "nameProven": false,
                        "nativeType": "AZStd::vector<PersistentItemData>",
                        "wireShape": "vec<PersistentItemData>",
                        "evidenceSource": "persistent-item-vector-container-slot"
                    }]
                },
                "valueTypeInfoCandidates": [{
                    "address": "NewWorld+0x9f34228",
                    "name": "PersistentItemData",
                    "typeId": persistent_item_data_id.to_string(),
                    "source": "serialize-registration+marshal-helper-callgraph"
                }],
                "slots": []
            }]
        }))
        .expect("schema");
        schema.merge_serialize_codegen_unit(
            &SerializeCodegenUnit {
                items: vec![named_value_item::<0>(
                    persistent_item_data_id,
                    "PersistentItemData",
                    [],
                )],
            },
            Some("selection.json".to_owned()),
        );

        let output =
            NetworkRustEmitter::emit_replicated_states(&schema, [2938]).expect("state source");
        let plan = &output.report.state_generation_plans[0];
        let field = &plan.fields[0];

        assert!(plan.can_generate, "{plan:#?}");
        assert_eq!(
            field.rust_value_type.as_deref(),
            Some(
                "::nw_network::serialize::IndexMap<[u8; 16], ::std::vec::Vec<::nw_network::source::PersistentItemData>>"
            )
        );
        assert!(field.rust_field_type.as_deref().is_some_and(|ty| {
            ty.contains(
                "DefaultMarshaler<::std::vec::Vec<::nw_network::source::PersistentItemData>>",
            )
        }));
    }

    #[test]
    fn embedded_vector_value_shape_emits_nested_local_marshaler() {
        let outer_id = uuid!("2c011c80-9a5e-46fb-bf92-dac57d0cc07d");
        let schema = NetworkSchema::from_ghidra_static_network_report(&json!({
            "registryEntries": [{
                "uuid": "26F9BF16-816C-4B55-9443-3337124D4490",
                "typeIndex": 7000,
                "typeName": "MB::NestedVectorComponentReplicatedState",
                "fields": [{
                    "index": 0,
                    "name": "nestedValues",
                    "group": 0,
                    "handlerVtable": "NewWorld+0x9000000",
                    "confidence": "register-field-call"
                }]
            }],
            "fieldRegistrationFunctions": [],
            "fieldHandlerVtables": [{
                "address": "NewWorld+0x9000000",
                "fieldCount": 1,
                "deltaMarshalShapes": [
                    "vlq-u32",
                    "vlq-u64",
                    "sequence-number",
                    "u32",
                    "vlq-u32",
                    "u8",
                    "u16"
                ],
                "fullMarshalShapes": [
                    "sequence-number",
                    "vlq-u32",
                    "u32",
                    "vlq-u32",
                    "u8",
                    "u16"
                ],
                "valueTypeShape": {
                    "typeId": outer_id.to_string(),
                    "typeIdSource": "rtti-provider-vtable",
                    "typeName": "OuterValue",
                    "typeNameFull": "OuterValue",
                    "typeNameSource": "az-rtti-provider-table",
                    "memberNameSource": "synthetic-pcode-wire-order",
                    "memberNamesProven": false,
                    "validation": "container-value-pcode-wire-order-native-rtti",
                    "members": [{
                        "index": 0,
                        "offset": "0x0",
                        "nativeOffset": "0x0",
                        "name": "field_0",
                        "nameSource": "synthetic-pcode-wire-order",
                        "nameProven": false,
                        "nativeType": "AZ::u32",
                        "wireShape": "u32",
                        "byteWidth": 4,
                        "evidenceSource": "container-value-pcode-call+native-rtti-synthetic-field"
                    }, {
                        "index": 1,
                        "offset": "0x8",
                        "nativeOffset": "0x8",
                        "name": "field_1",
                        "nameSource": "synthetic-pcode-wire-order",
                        "nameProven": false,
                        "nativeType": "AZStd::vector<InnerValue>",
                        "wireShape": "vec<InnerValue>",
                        "byteWidth": 24,
                        "evidenceSource": "container-value-pcode-collection-output+native-rtti-synthetic-field"
                    }]
                },
                "embeddedValueTypeShapes": [{
                    "typeName": "InnerValue",
                    "typeNameFull": "InnerValue",
                    "typeNameSource": "nested-pcode-call",
                    "memberNameSource": "synthetic-pcode-wire-order",
                    "memberNamesProven": false,
                    "validation": "embedded-container-value-pcode-wire-sequence",
                    "members": [{
                        "index": 0,
                        "offset": "0x0",
                        "nativeOffset": "0x0",
                        "name": "field_0",
                        "nameSource": "synthetic-pcode-wire-order",
                        "nameProven": false,
                        "nativeType": "AZ::u8",
                        "wireShape": "u8",
                        "byteWidth": 1,
                        "evidenceSource": "embedded-container-value-pcode-call"
                    }, {
                        "index": 1,
                        "offset": "0x2",
                        "nativeOffset": "0x2",
                        "name": "field_1",
                        "nameSource": "synthetic-pcode-wire-order",
                        "nameProven": false,
                        "nativeType": "AZ::u16",
                        "wireShape": "u16",
                        "byteWidth": 2,
                        "evidenceSource": "embedded-container-value-pcode-call"
                    }]
                }],
                "slots": []
            }]
        }))
        .expect("schema");

        let output =
            NetworkRustEmitter::emit_replicated_states(&schema, [7000]).expect("state source");
        let plan = &output.report.state_generation_plans[0];
        let field = &plan.fields[0];

        assert!(plan.can_generate, "{plan:#?}");
        assert_eq!(
            field.rust_value_type.as_deref(),
            Some("::std::vec::Vec<NestedValuesOuterValue>")
        );
        assert_eq!(field.container_embedded_value_type_shapes.len(), 1);
        let inner_index = output
            .source
            .find("pub struct NestedValuesInnerValue")
            .expect("inner support type");
        let outer_index = output
            .source
            .find("pub struct NestedValuesOuterValue")
            .expect("outer support type");
        assert!(inner_index < outer_index);
        let compact_source = output.source.split_whitespace().collect::<String>();
        assert!(
            compact_source
                .contains("impl::nw_network::serialize::MarshalerforNestedValuesInnerValue")
        );
        assert!(compact_source.contains("pubfield_1:::std::vec::Vec<NestedValuesInnerValue>"));
    }

    #[test]
    fn nested_progression_value_shape_emits_two_vector_members() {
        let schema = NetworkSchema::from_ghidra_static_network_report(&json!({
            "registryEntries": [{
                "uuid": "ABBA1776-6E4C-4BA6-A831-6F4052AFC9C0",
                "typeIndex": 3086,
                "typeName": "MB::LandClaimManagerComponentReplicatedState",
                "fields": [{
                    "index": 15,
                    "name": "replicatedProgression",
                    "group": 0,
                    "handlerVtable": "NewWorld+0x81685d0",
                    "confidence": "register-field-call"
                }]
            }],
            "fieldRegistrationFunctions": [],
            "fieldHandlerVtables": [{
                "address": "NewWorld+0x81685d0",
                "fieldCount": 1,
                "deltaMarshalShapes": [
                    "vlq-u32",
                    "vlq-u32",
                    "u8",
                    "vlq-u64",
                    "sequence-number",
                    "u8"
                ],
                "fullMarshalShapes": [
                    "sequence-number",
                    "vlq-u32",
                    "vlq-u32",
                    "u32",
                    "u8",
                    "vlq-u32",
                    "u32",
                    "u8",
                    "u32",
                    "u8"
                ],
                "valueTypeShape": {
                    "typeName": "Value",
                    "typeNameSource": "container-slot-shape",
                    "memberNameSource": "ghidra-stack-wire-sequence",
                    "memberNamesProven": false,
                    "validation": "custom-container-value-pcode-wire-sequence-land-claim-progression",
                    "members": [{
                        "index": 0,
                        "offset": "0x0",
                        "nativeOffset": "0x0",
                        "name": "field_00",
                        "nameSource": "synthetic-offset",
                        "nameProven": false,
                        "nativeType": "AZStd::vector<FirstElement>",
                        "wireShape": "vec<FirstElement>",
                        "evidenceSource": "land-claim-progression-container-slot"
                    }, {
                        "index": 1,
                        "offset": "0x20",
                        "nativeOffset": "0x20",
                        "name": "field_20",
                        "nameSource": "synthetic-offset",
                        "nameProven": false,
                        "nativeType": "AZStd::vector<SecondElement>",
                        "wireShape": "vec<SecondElement>",
                        "evidenceSource": "land-claim-progression-container-slot"
                    }, {
                        "index": 2,
                        "offset": "0x40",
                        "nativeOffset": "0x40",
                        "name": "field_40",
                        "nameSource": "synthetic-offset",
                        "nameProven": false,
                        "nativeType": "AZ::u8",
                        "wireShape": "u8",
                        "byteWidth": 1,
                        "evidenceSource": "land-claim-progression-container-slot"
                    }]
                },
                "embeddedValueTypeShapes": [{
                    "typeName": "FirstElement",
                    "typeNameSource": "container-slot-shape",
                    "memberNameSource": "ghidra-stack-wire-sequence",
                    "memberNamesProven": false,
                    "validation": "custom-container-value-pcode-wire-sequence-land-claim-progression-first-element",
                    "members": [{
                        "index": 0,
                        "offset": "0x0",
                        "nativeOffset": "0x0",
                        "name": "field_00",
                        "nameSource": "synthetic-offset",
                        "nameProven": false,
                        "nativeType": "AZ::u32",
                        "wireShape": "u32",
                        "byteWidth": 4,
                        "evidenceSource": "land-claim-progression-first-vector-slot"
                    }, {
                        "index": 1,
                        "offset": "0x4",
                        "nativeOffset": "0x4",
                        "name": "field_04",
                        "nameSource": "synthetic-offset",
                        "nameProven": false,
                        "nativeType": "AZ::u8",
                        "wireShape": "u8",
                        "byteWidth": 1,
                        "evidenceSource": "land-claim-progression-first-vector-slot"
                    }]
                }, {
                    "typeName": "SecondElement",
                    "typeNameSource": "container-slot-shape",
                    "memberNameSource": "ghidra-stack-wire-sequence",
                    "memberNamesProven": false,
                    "validation": "custom-container-value-pcode-wire-sequence-land-claim-progression-second-element",
                    "members": [{
                        "index": 0,
                        "offset": "0x0",
                        "nativeOffset": "0x0",
                        "name": "field_00",
                        "nameSource": "synthetic-offset",
                        "nameProven": false,
                        "nativeType": "AZ::u32",
                        "wireShape": "u32",
                        "byteWidth": 4,
                        "evidenceSource": "land-claim-progression-second-vector-slot"
                    }, {
                        "index": 1,
                        "offset": "0x4",
                        "nativeOffset": "0x4",
                        "name": "field_04",
                        "nameSource": "synthetic-offset",
                        "nameProven": false,
                        "nativeType": "AZ::u8",
                        "wireShape": "u8",
                        "byteWidth": 1,
                        "evidenceSource": "land-claim-progression-second-vector-slot"
                    }, {
                        "index": 2,
                        "offset": "0x8",
                        "nativeOffset": "0x8",
                        "name": "field_08",
                        "nameSource": "synthetic-offset",
                        "nameProven": false,
                        "nativeType": "AZ::u32",
                        "wireShape": "u32",
                        "byteWidth": 4,
                        "evidenceSource": "land-claim-progression-second-vector-slot"
                    }]
                }],
                "slots": []
            }]
        }))
        .expect("schema");

        let output =
            NetworkRustEmitter::emit_replicated_states(&schema, [3086]).expect("state source");
        let plan = &output.report.state_generation_plans[0];
        let field = &plan.fields[0];

        assert!(plan.can_generate, "{plan:#?}");
        assert_eq!(
            field.rust_value_type.as_deref(),
            Some("::std::vec::Vec<ReplicatedProgressionValue>")
        );
        assert_eq!(field.container_embedded_value_type_shapes.len(), 2);
        let compact_source = output.source.split_whitespace().collect::<String>();
        assert!(
            compact_source
                .contains("pubfield_00:::std::vec::Vec<ReplicatedProgressionFirstElement>")
        );
        assert!(
            compact_source
                .contains("pubfield_20:::std::vec::Vec<ReplicatedProgressionSecondElement>")
        );
        assert!(compact_source.contains("pubfield_40:u8"));
    }

    #[test]
    fn source_backed_container_value_with_composite_member_emits_codec() {
        let persistent_mount_data_id = uuid!("e6f4d231-af72-47b6-a817-ab1a3e413216");
        let schema = NetworkSchema::from_ghidra_static_network_report(&json!({
            "registryEntries": [{
                "uuid": "BD597A98-A64F-4538-94B2-2479C35CB6BF",
                "typeIndex": 5620,
                "typeName": "MB::MountComponentReplicatedState",
                "fields": [{
                    "index": 0,
                    "name": "m_persistentMountData",
                    "group": 0,
                    "handlerVtable": "NewWorld+0x82378f0",
                    "confidence": "register-field-call"
                }]
            }],
            "fieldRegistrationFunctions": [],
            "fieldHandlerVtables": [{
                "address": "NewWorld+0x82378f0",
                "fieldCount": 1,
                "deltaMarshalShapes": [
                    "vlq-u32",
                    "u8",
                    "u32",
                    "sequence-number",
                    "u8",
                    "u8",
                    "u8",
                    "u8",
                    "string"
                ],
                "fullMarshalShapes": [
                    "sequence-number",
                    "vlq-u32",
                    "u32",
                    "u8",
                    "u8",
                    "u8",
                    "u8",
                    "string"
                ],
                "valueTypeShape": {
                    "typeId": persistent_mount_data_id.to_string(),
                    "typeName": "PersistentMountData",
                    "typeNameFull": "PersistentMountData",
                    "typeNameSource": "serialize-json-class-name",
                    "memberNameSource": "serialize-json-field",
                    "memberNamesProven": true,
                    "validation": "custom-container-value-pcode-serialize-type-sequence-persistent-mount-data",
                    "members": [{
                        "index": 0,
                        "offset": "0x8",
                        "nativeOffset": "0x8",
                        "name": "m_dyeData",
                        "nameSource": "serialize-json-offset-match",
                        "nameProven": true,
                        "nativeType": "DyeData",
                        "wireShape": "composite<u8,u8,u8,u8>",
                        "evidenceSource": "persistent-mount-data-container-slot"
                    }, {
                        "index": 1,
                        "offset": "0x18",
                        "nativeOffset": "0x18",
                        "name": "m_name",
                        "nameSource": "serialize-json-offset-match",
                        "nameProven": true,
                        "nativeType": "AZStd::string",
                        "wireShape": "string",
                        "evidenceSource": "persistent-mount-data-container-slot"
                    }]
                },
                "valueTypeInfoCandidates": [{
                    "address": "NewWorld+0x9ee6838",
                    "name": "PersistentMountData",
                    "typeId": persistent_mount_data_id.to_string(),
                    "source": "serialize-registration+container-slot-shape"
                }],
                "slots": []
            }]
        }))
        .expect("schema");

        let output =
            NetworkRustEmitter::emit_replicated_states(&schema, [5620]).expect("state source");
        let plan = &output.report.state_generation_plans[0];
        let field = &plan.fields[0];

        assert!(plan.can_generate, "{plan:#?}");
        assert_eq!(
            field.rust_value_type.as_deref(),
            Some(
                "::nw_network::serialize::IndexMap<u32, ::nw_network::source::PersistentMountData>"
            )
        );
        assert!(
            field
                .rust_field_type
                .as_deref()
                .is_some_and(|ty| { ty.contains("PersistentMountDataMarshaler") })
        );
        assert!(output.source.contains("value.dye_data"));
        assert!(output.source.contains("value.name"));
    }

    #[test]
    fn affliction_hot_data_container_uses_source_type_subset_codec() {
        let affliction_data_id = uuid!("99a32353-e595-4d5c-86cb-dc80318228d1");
        let schema = NetworkSchema::from_ghidra_static_network_report(&json!({
            "registryEntries": [{
                "uuid": "F7662A54-8F1D-4F4A-A7F7-2A1B08E7AB99",
                "typeIndex": 15,
                "typeName": "MB::VitalsComponentReplicatedState",
                "fields": [{
                    "index": 3,
                    "name": "replicatedAfflictionsHotData",
                    "group": 0,
                    "handlerVtable": "NewWorld+0x855eab0",
                    "confidence": "register-field-call"
                }]
            }],
            "fieldRegistrationFunctions": [],
            "fieldHandlerVtables": [{
                "address": "NewWorld+0x855eab0",
                "fieldCount": 1,
                "deltaMarshalShapes": [
                    "vlq-u32",
                    "u8",
                    "u8",
                    "sequence-number",
                    "half-f32",
                    "half-f32",
                    "u64",
                    "u64"
                ],
                "fullMarshalShapes": [
                    "sequence-number",
                    "vlq-u32",
                    "u8",
                    "half-f32",
                    "half-f32",
                    "u64",
                    "u64"
                ],
                "valueTypeShape": {
                    "typeId": affliction_data_id.to_string(),
                    "typeName": "AfflictionData",
                    "typeNameFull": "AfflictionData",
                    "typeNameSource": "serialize-json-class-name",
                    "memberNameSource": "serialize-json-field-subset",
                    "memberNamesProven": true,
                    "validation": "custom-container-value-pcode-serialize-type-subset-affliction-hot-data",
                    "members": [{
                        "index": 0,
                        "offset": "0x8",
                        "nativeOffset": "0x8",
                        "name": "m_lastAmount",
                        "nameSource": "serialize-json-field-subset",
                        "nameProven": true,
                        "nativeType": "float",
                        "wireShape": "half-f32",
                        "byteWidth": 4,
                        "evidenceSource": "affliction-hot-data-container-slot"
                    }, {
                        "index": 1,
                        "offset": "0x20",
                        "nativeOffset": "0x20",
                        "name": "m_targetAmount",
                        "nameSource": "serialize-json-field-subset",
                        "nameProven": true,
                        "nativeType": "float",
                        "wireShape": "half-f32",
                        "byteWidth": 4,
                        "evidenceSource": "affliction-hot-data-container-slot"
                    }, {
                        "index": 2,
                        "offset": "0x10",
                        "nativeOffset": "0x10",
                        "name": "m_lastAmountTimePoint",
                        "nameSource": "serialize-json-field-subset",
                        "nameProven": true,
                        "nativeType": "TimePoint",
                        "wireShape": "u64",
                        "byteWidth": 16,
                        "evidenceSource": "affliction-hot-data-container-slot"
                    }, {
                        "index": 3,
                        "offset": "0x28",
                        "nativeOffset": "0x28",
                        "name": "m_targetAmountTimePoint",
                        "nameSource": "serialize-json-field-subset",
                        "nameProven": true,
                        "nativeType": "TimePoint",
                        "wireShape": "u64",
                        "byteWidth": 16,
                        "evidenceSource": "affliction-hot-data-container-slot"
                    }]
                },
                "valueTypeInfoCandidates": [{
                    "address": "NewWorld+0x9f34550",
                    "name": "AfflictionData",
                    "typeId": affliction_data_id.to_string(),
                    "source": "serialize-registration+container-slot-shape"
                }],
                "slots": []
            }]
        }))
        .expect("schema");

        let output =
            NetworkRustEmitter::emit_replicated_states(&schema, [15]).expect("state source");
        let plan = &output.report.state_generation_plans[0];
        let field = &plan.fields[0];

        assert!(plan.can_generate, "{plan:#?}");
        assert_eq!(
            field.rust_value_type.as_deref(),
            Some("::nw_network::serialize::IndexMap<u8, ::nw_network::source::AfflictionData>")
        );
        assert!(field.rust_field_type.as_deref().is_some_and(|ty| {
            ty.contains("ReplicatedAfflictionsHotDataAfflictionDataMarshaler")
        }));
        assert!(output.source.contains("value.last_amount"));
        assert!(output.source.contains("value.target_amount"));
        assert!(output.source.contains("value.last_amount_time_point"));
        assert!(output.source.contains("value.target_amount_time_point"));
        assert!(output.source.contains("HalfF32Marshaler"));
    }

    #[test]
    fn projected_source_container_value_emits_array_and_smallest_three_codec() {
        let housing_item_server_data_id = uuid!("d65749ab-07d4-4401-b2d4-d9282475ce59");
        let schema = NetworkSchema::from_ghidra_static_network_report(&json!({
            "registryEntries": [{
                "uuid": "9E4B23C7-4BCB-4D98-9A4D-7D1805B43C74",
                "typeIndex": 3663,
                "typeName": "Javelin::HouseDataReplicatedState",
                "fields": [{
                    "index": 0,
                    "name": "housingItems",
                    "group": 0,
                    "handlerVtable": "NewWorld+0x81ebc78",
                    "confidence": "register-field-call"
                }]
            }],
            "fieldRegistrationFunctions": [],
            "fieldHandlerVtables": [{
                "address": "NewWorld+0x81ebc78",
                "fieldCount": 1,
                "deltaMarshalShapes": [
                    "vlq-u32",
                    "u8",
                    "vlq-u64",
                    "sequence-number",
                    "u16",
                    "u16",
                    "u16",
                    "quat-smallest-three",
                    "u32",
                    "u8"
                ],
                "fullMarshalShapes": [
                    "sequence-number",
                    "vlq-u32",
                    "u16",
                    "u16",
                    "u16",
                    "quat-smallest-three",
                    "u32",
                    "u8"
                ],
                "valueTypeShape": {
                    "typeId": housing_item_server_data_id.to_string(),
                    "typeName": "HousingItemServerData",
                    "typeNameFull": "HousingItemServerData",
                    "typeNameSource": "serialize-json-class-name",
                    "memberNameSource": "serialize-json-offset+ghidra-helper-offset",
                    "memberNamesProven": true,
                    "validation": "custom-container-value-pcode-serialize-type-sequence-housing-item-server-data",
                    "members": [{
                        "index": 0,
                        "offset": "0x20",
                        "nativeOffset": "0x20",
                        "name": "m_positionOffset",
                        "nameSource": "serialize-json-offset-match",
                        "nameProven": true,
                        "nativeType": "AZStd::array<short, 3>",
                        "wireShape": "composite<u16,u16,u16>",
                        "evidenceSource": "housing-item-server-data-container-slot"
                    }, {
                        "index": 1,
                        "offset": "0x10",
                        "nativeOffset": "0x10",
                        "name": "m_rotation",
                        "nameSource": "serialize-json-offset-match",
                        "nameProven": true,
                        "nativeType": "Quaternion",
                        "wireShape": "quat-smallest-three",
                        "evidenceSource": "housing-item-server-data-container-slot"
                    }, {
                        "index": 2,
                        "offset": "0x30",
                        "nativeOffset": "0x30",
                        "name": "m_itemIndex",
                        "nameSource": "serialize-json-offset-match",
                        "nameProven": true,
                        "nativeType": "AZ::u32",
                        "wireShape": "u32",
                        "evidenceSource": "housing-item-server-data-container-slot"
                    }, {
                        "index": 3,
                        "offset": "0x2c",
                        "nativeOffset": "0x2c",
                        "name": "m_state",
                        "nameSource": "serialize-json-offset-match",
                        "nameProven": true,
                        "nativeType": "AZ::u8",
                        "wireShape": "u8",
                        "evidenceSource": "housing-item-server-data-container-slot"
                    }]
                },
                "valueTypeInfoCandidates": [{
                    "address": "NewWorld+0x9ed8bb8",
                    "name": "HousingItemServerData",
                    "typeId": housing_item_server_data_id.to_string(),
                    "source": "serialize-registration+container-slot-shape"
                }],
                "slots": []
            }]
        }))
        .expect("schema");

        let output =
            NetworkRustEmitter::emit_replicated_states(&schema, [3663]).expect("state source");
        let plan = &output.report.state_generation_plans[0];
        let field = &plan.fields[0];

        assert!(plan.can_generate, "{plan:#?}");
        assert_eq!(
            field.rust_value_type.as_deref(),
            Some("::std::vec::Vec<::nw_network::source::HousingItemServerData>")
        );
        assert!(
            field
                .rust_field_type
                .as_deref()
                .is_some_and(|ty| { ty.contains("HousingItemServerDataMarshaler") })
        );
        assert!(output.source.contains("value.position_offset"));
        assert!(output.source.contains("value.rotation"));
        assert!(
            output
                .source
                .contains("QuatSmallestThreeQuantizedMarshaler")
        );
    }

    #[test]
    fn native_serializer_container_value_without_fields_stays_local_support_type() {
        let loot_limit_data_id = uuid!("ec6027f0-84b8-46f1-9683-b850c37348ee");
        let schema = NetworkSchema::from_ghidra_static_network_report(&json!({
            "registryEntries": [{
                "uuid": "A24F4B9E-71E0-4D10-8BB6-42476298BB80",
                "typeIndex": 982,
                "typeName": "MB::LootTrackerComponentReplicatedState",
                "fields": [{
                    "index": 0,
                    "name": "m_lootLimitDataMap",
                    "group": 0,
                    "handlerVtable": "NewWorld+0x8219138",
                    "confidence": "register-field-call"
                }]
            }],
            "fieldRegistrationFunctions": [],
            "fieldHandlerVtables": [{
                "address": "NewWorld+0x8219138",
                "fieldCount": 1,
                "deltaMarshalShapes": [
                    "vlq-u32",
                    "u8",
                    "u32",
                    "sequence-number",
                    "u64",
                    "u64",
                    "u16",
                    "u8"
                ],
                "fullMarshalShapes": [
                    "sequence-number",
                    "vlq-u32",
                    "u32",
                    "u64",
                    "u64",
                    "u16",
                    "u8"
                ],
                "valueTypeShape": {
                    "typeId": loot_limit_data_id.to_string(),
                    "typeIdSource": "serialize-registration+container-slot-shape",
                    "typeName": "LootLimitData",
                    "typeNameFull": "LootLimitData",
                    "typeNameSource": "serialize-json-class-name",
                    "factory": "NewWorld+0x9f34270",
                    "memberNameSource": "synthetic-pcode-wire-order",
                    "memberNamesProven": false,
                    "validation": "custom-container-value-pcode-native-type-sequence-loot-limit-data",
                    "members": [{
                        "index": 0,
                        "offset": "0x10",
                        "nativeOffset": "0x10",
                        "name": "field_0",
                        "nameSource": "synthetic-pcode-wire-order",
                        "nameProven": false,
                        "nativeType": "WallClockTimePoint",
                        "wireShape": "u64",
                        "byteWidth": 16,
                        "evidenceSource": "loot-limit-data-container-slot"
                    }, {
                        "index": 1,
                        "offset": "0x20",
                        "nativeOffset": "0x20",
                        "name": "field_1",
                        "nameSource": "synthetic-pcode-wire-order",
                        "nameProven": false,
                        "nativeType": "WallClockTimePoint",
                        "wireShape": "u64",
                        "byteWidth": 16,
                        "evidenceSource": "loot-limit-data-container-slot"
                    }, {
                        "index": 2,
                        "offset": "0x28",
                        "nativeOffset": "0x28",
                        "name": "field_2",
                        "nameSource": "synthetic-pcode-wire-order",
                        "nameProven": false,
                        "nativeType": "AZ::u16",
                        "wireShape": "u16",
                        "byteWidth": 2,
                        "evidenceSource": "loot-limit-data-container-slot"
                    }, {
                        "index": 3,
                        "offset": "0x2a",
                        "nativeOffset": "0x2a",
                        "name": "field_3",
                        "nameSource": "synthetic-pcode-wire-order",
                        "nameProven": false,
                        "nativeType": "AZ::u8",
                        "wireShape": "u8",
                        "byteWidth": 1,
                        "evidenceSource": "loot-limit-data-container-slot"
                    }]
                },
                "valueTypeInfoCandidates": [{
                    "address": "NewWorld+0x9f34270",
                    "name": "LootLimitData",
                    "typeId": loot_limit_data_id.to_string(),
                    "source": "serialize-registration+container-slot-shape"
                }],
                "slots": []
            }]
        }))
        .expect("schema");

        let output =
            NetworkRustEmitter::emit_replicated_states(&schema, [982]).expect("state source");
        let plan = &output.report.state_generation_plans[0];
        let field = &plan.fields[0];

        assert!(plan.can_generate, "{plan:#?}");
        assert_eq!(
            field.rust_value_type.as_deref(),
            Some("::nw_network::serialize::IndexMap<u32, LootLimitDataMapLootLimitData>")
        );
        assert!(
            output
                .source
                .contains("pub struct LootLimitDataMapLootLimitData")
        );
        assert!(!output.source.contains("source::LootLimitData"));
        assert!(output.source.contains("::nw_network::WallClockTimePoint"));
    }

    #[test]
    fn replicated_vector_container_value_without_source_type_emits_vec_support_type() {
        let schema = NetworkSchema::from_ghidra_static_network_report(&json!({
            "registryEntries": [{
                "uuid": "3EF03794-7831-4123-A770-821C53F29C81",
                "typeIndex": 3681,
                "typeName": "Javelin::ProgressionPointComponentReplicatedState",
                "fields": [{
                    "index": 0,
                    "name": "pointEntries",
                    "group": 0,
                    "handlerVtable": "NewWorld+0x82ae9d0",
                    "confidence": "register-field-call"
                }]
            }],
            "fieldRegistrationFunctions": [],
            "fieldHandlerVtables": [{
                "address": "NewWorld+0x82ae9d0",
                "fieldCount": 1,
                "deltaMarshalShapes": [
                    "vlq-u32",
                    "u8",
                    "vlq-u64",
                    "sequence-number",
                    "u32",
                    "u32",
                    "u16"
                ],
                "fullMarshalShapes": [
                    "sequence-number",
                    "vlq-u32",
                    "u32",
                    "u32",
                    "u16"
                ],
                "valueTypeShape": {
                    "typeName": "PointEntry",
                    "typeNameFull": "PointEntry",
                    "typeNameSource": "field-name+container-slot-shape",
                    "memberNameSource": "synthetic-pcode-wire-order",
                    "memberNamesProven": false,
                    "validation": "custom-container-value-pcode-wire-sequence-progression-point-entry",
                    "members": [{
                        "index": 0,
                        "offset": "0x8",
                        "nativeOffset": "0x8",
                        "name": "field_0",
                        "nameSource": "synthetic-pcode-wire-order",
                        "nameProven": false,
                        "nativeType": "AZ::u32",
                        "wireShape": "u32",
                        "byteWidth": 4,
                        "evidenceSource": "progression-point-entry-container-slot"
                    }, {
                        "index": 1,
                        "offset": "0xc",
                        "nativeOffset": "0xc",
                        "name": "field_1",
                        "nameSource": "synthetic-pcode-wire-order",
                        "nameProven": false,
                        "nativeType": "AZ::u32",
                        "wireShape": "u32",
                        "byteWidth": 4,
                        "evidenceSource": "progression-point-entry-container-slot"
                    }, {
                        "index": 2,
                        "offset": "0x10",
                        "nativeOffset": "0x10",
                        "name": "field_2",
                        "nameSource": "synthetic-pcode-wire-order",
                        "nameProven": false,
                        "nativeType": "AZ::u16",
                        "wireShape": "u16",
                        "byteWidth": 2,
                        "evidenceSource": "progression-point-entry-container-slot"
                    }]
                },
                "slots": []
            }]
        }))
        .expect("schema");

        let output =
            NetworkRustEmitter::emit_replicated_states(&schema, [3681]).expect("state source");
        let plan = &output.report.state_generation_plans[0];
        let field = &plan.fields[0];

        assert!(plan.can_generate, "{plan:#?}");
        assert_eq!(
            field.rust_value_type.as_deref(),
            Some("::std::vec::Vec<PointEntriesPointEntry>")
        );
        assert!(output.source.contains("pub struct PointEntriesPointEntry"));
        assert!(output.source.contains("PointEntriesPointEntryMarshaler"));
    }

    #[test]
    fn replicated_vector_container_value_can_use_runtime_semantic_composite_type() {
        let schema = NetworkSchema::from_ghidra_static_network_report(&json!({
            "registryEntries": [{
                "uuid": "F0D09F5A-A67D-4B8C-88F1-FA96442165F2",
                "typeIndex": 5980,
                "typeName": "MB::TransformLinkComponentReplicatedState",
                "fields": [{
                    "index": 0,
                    "name": "m_childGDEVector",
                    "group": 0,
                    "handlerVtable": "NewWorld+0x847c718",
                    "confidence": "register-field-call"
                }]
            }],
            "fieldRegistrationFunctions": [],
            "fieldHandlerVtables": [{
                "address": "NewWorld+0x847c718",
                "fieldCount": 1,
                "valueTypeName": "RemoteServerGDERef",
                "deltaMarshalShapes": [
                    "vlq-u32",
                    "u8",
                    "vlq-u64",
                    "sequence-number",
                    "fixed-bytes-16",
                    "u64"
                ],
                "fullMarshalShapes": [
                    "sequence-number",
                    "vlq-u32",
                    "fixed-bytes-16",
                    "u64"
                ],
                "valueTypeShape": {
                    "typeName": "Value",
                    "typeNameSource": "synthetic-container-value",
                    "memberNameSource": "synthetic-serialize-type-sequence",
                    "memberNamesProven": false,
                    "validation": "container-value-serialize-type-sequence",
                    "members": [{
                        "index": 0,
                        "name": "remote_server_gderef",
                        "nativeType": "RemoteServerGDERef",
                        "wireShape": "composite<fixed-bytes-16,u64>"
                    }]
                },
                "slots": []
            }]
        }))
        .expect("schema");

        let output =
            NetworkRustEmitter::emit_replicated_states(&schema, [5980]).expect("state source");
        let plan = &output.report.state_generation_plans[0];
        let field = &plan.fields[0];

        assert!(plan.can_generate, "{plan:#?}");
        assert_eq!(
            field.rust_value_type.as_deref(),
            Some("::std::vec::Vec<::nw_network::RemoteServerGdeRef>")
        );
        assert!(!output.source.contains("pub struct ChildGdevectorValue"));
    }

    #[test]
    fn replicated_vector_container_value_support_type_can_hold_runtime_semantic_composites() {
        let schema = NetworkSchema::from_ghidra_static_network_report(&json!({
            "registryEntries": [{
                "uuid": "E2D1863A-2BBE-4434-AFFB-D75BD68BDE5B",
                "typeIndex": 3451,
                "typeName": "Javelin::GroupDataComponentReplicatedState",
                "fields": [{
                    "index": 43,
                    "name": "groupMemberHouseIds",
                    "group": 0,
                    "handlerVtable": "NewWorld+0x81ce260",
                    "confidence": "register-field-call"
                }, {
                    "index": 50,
                    "name": "groupMemberHouseIds",
                    "group": 1,
                    "handlerVtable": "NewWorld+0x81ce260",
                    "confidence": "register-field-call"
                }]
            }],
            "fieldRegistrationFunctions": [],
            "fieldHandlerVtables": [{
                "address": "NewWorld+0x81ce260",
                "fieldCount": 1,
                "deltaMarshalShapes": [
                    "vlq-u32",
                    "u8",
                    "vlq-u64",
                    "sequence-number",
                    "fixed-bytes-16",
                    "u64",
                    "u64",
                    "fixed-bytes-16",
                    "u64"
                ],
                "fullMarshalShapes": [
                    "sequence-number",
                    "vlq-u32",
                    "fixed-bytes-16",
                    "u64",
                    "u64",
                    "fixed-bytes-16",
                    "u64"
                ],
                "valueTypeShape": {
                    "typeName": "Value",
                    "typeNameSource": "synthetic-container-value",
                    "memberNameSource": "ghidra-stack-serialize-type-sequence",
                    "memberNamesProven": false,
                    "validation": "container-value-pcode-stack-wire-order-serialize-type-sequence",
                    "members": [{
                        "index": 0,
                        "name": "remote_typeless_server_facet_ref",
                        "nativeType": "RemoteTypelessServerFacetRef",
                        "wireShape": "composite<fixed-bytes-16,u64,u64>"
                    }, {
                        "index": 1,
                        "name": "remote_server_gderef",
                        "nativeType": "RemoteServerGDERef",
                        "wireShape": "composite<fixed-bytes-16,u64>"
                    }]
                },
                "slots": []
            }]
        }))
        .expect("schema");

        let output =
            NetworkRustEmitter::emit_replicated_states(&schema, [3451]).expect("state source");
        let plan = &output.report.state_generation_plans[0];
        let field = &plan.fields[0];

        assert!(plan.can_generate, "{plan:#?}");
        assert_eq!(
            field.rust_value_type.as_deref(),
            Some("::std::vec::Vec<GroupMemberHouseIdsValue>")
        );
        assert!(
            output
                .source
                .contains("pub struct GroupMemberHouseIdsValue")
        );
        assert!(
            !output
                .source
                .contains("pub struct GroupMemberHouseIds50Value")
        );
        assert!(
            output
                .source
                .contains("::nw_network::RemoteTypelessServerFacetRef")
        );
        assert!(output.source.contains("::nw_network::RemoteServerGdeRef"));
    }

    #[test]
    fn projectile_piercing_hits_emit_vector_support_type() {
        let schema = NetworkSchema::from_ghidra_static_network_report(&json!({
            "registryEntries": [{
                "uuid": "AA4DB620-9D7A-43A0-8E51-6D8D83A7EE16",
                "typeIndex": 16,
                "typeName": "MB::ProjectileReplicatedState",
                "fields": [{
                    "index": 16,
                    "name": "m_piercingHits",
                    "group": 0,
                    "handlerVtable": "NewWorld+0x8549bd0",
                    "confidence": "register-field-call"
                }]
            }],
            "fieldRegistrationFunctions": [],
            "fieldHandlerVtables": [{
                "address": "NewWorld+0x8549bd0",
                "fieldCount": 1,
                "deltaMarshalShapes": [
                    "vlq-u32",
                    "u8",
                    "vlq-u64",
                    "sequence-number",
                    "u64",
                    "u8",
                    "u16"
                ],
                "fullMarshalShapes": [
                    "sequence-number",
                    "vlq-u32",
                    "u64",
                    "u8",
                    "u16"
                ],
                "valueTypeShape": {
                    "typeName": "PiercingHitData",
                    "typeNameFull": "PiercingHitData",
                    "typeNameSource": "field-name+container-slot-shape",
                    "memberNameSource": "synthetic-pcode-wire-order",
                    "memberNamesProven": false,
                    "validation": "custom-container-value-pcode-wire-sequence-piercing-hit-data",
                    "members": [{
                        "index": 0,
                        "offset": "0x8",
                        "name": "field_0",
                        "nativeType": "AZ::u64",
                        "wireShape": "u64",
                        "byteWidth": 8,
                        "evidenceSource": "piercing-hit-data-container-slot"
                    }, {
                        "index": 1,
                        "offset": "0x10",
                        "name": "field_1",
                        "nativeType": "AZ::u8",
                        "wireShape": "u8",
                        "byteWidth": 1,
                        "evidenceSource": "piercing-hit-data-container-slot"
                    }, {
                        "index": 2,
                        "offset": "0x12",
                        "name": "field_2",
                        "nativeType": "AZ::u16",
                        "wireShape": "u16",
                        "byteWidth": 2,
                        "evidenceSource": "piercing-hit-data-container-slot"
                    }]
                },
                "slots": []
            }]
        }))
        .expect("schema");

        let output =
            NetworkRustEmitter::emit_replicated_states(&schema, [16]).expect("state source");
        let plan = &output.report.state_generation_plans[0];
        let field = &plan.fields[0];

        assert!(plan.can_generate, "{plan:#?}");
        assert_eq!(
            field.rust_value_type.as_deref(),
            Some("::std::vec::Vec<PiercingHitsPiercingHitData>")
        );
        assert!(
            output
                .source
                .contains("pub struct PiercingHitsPiercingHitData")
        );
        assert!(
            output
                .source
                .contains("PiercingHitsPiercingHitDataMarshaler")
        );
        assert!(
            !output
                .source
                .contains("IndexMap<u64, PiercingHitsPiercingHitData>")
        );
    }

    #[test]
    fn group_finder_applications_emit_fixed_uuid_keyed_map_support_type() {
        let schema = NetworkSchema::from_ghidra_static_network_report(&json!({
            "registryEntries": [{
                "uuid": "CE526687-CA4B-4647-A599-EC026FDC0C6D",
                "typeIndex": 1994,
                "typeName": "Javelin::GroupsComponentReplicatedState",
                "fields": [{
                    "index": 7,
                    "name": "groupFinderApplications",
                    "group": 1,
                    "handlerVtable": "NewWorld+0x81dadf0",
                    "confidence": "register-field-call"
                }]
            }],
            "fieldRegistrationFunctions": [],
            "fieldHandlerVtables": [{
                "address": "NewWorld+0x81dadf0",
                "fieldCount": 1,
                "keyNativeType": "AZ::Uuid",
                "keyNativeTypeSource": "replicated-container-key-marshal-shape",
                "deltaMarshalShapes": [
                    "vlq-u32",
                    "u8",
                    "fixed-bytes-16",
                    "sequence-number",
                    "u8",
                    "u8"
                ],
                "fullMarshalShapes": [
                    "sequence-number",
                    "vlq-u32",
                    "fixed-bytes-16",
                    "u8",
                    "u8"
                ],
                "valueTypeShape": {
                    "typeName": "GroupFinderApplication",
                    "typeNameFull": "GroupFinderApplication",
                    "typeNameSource": "field-name+container-slot-shape",
                    "memberNameSource": "synthetic-pcode-wire-order",
                    "memberNamesProven": false,
                    "validation": "custom-container-value-pcode-wire-sequence-group-finder-application",
                    "members": [{
                        "index": 0,
                        "offset": "0x0",
                        "name": "field_0",
                        "nativeType": "AZ::u8",
                        "wireShape": "u8",
                        "byteWidth": 1,
                        "evidenceSource": "group-finder-application-container-slot"
                    }, {
                        "index": 1,
                        "offset": "0x1",
                        "name": "field_1",
                        "nativeType": "AZ::u8",
                        "wireShape": "u8",
                        "byteWidth": 1,
                        "evidenceSource": "group-finder-application-container-slot"
                    }]
                },
                "slots": []
            }]
        }))
        .expect("schema");

        let output =
            NetworkRustEmitter::emit_replicated_states(&schema, [1994]).expect("state source");
        let container_shape = schema.field_handler_vtables[0]
            .container_shape
            .as_ref()
            .expect("container shape");
        let plan = &output.report.state_generation_plans[0];
        let field = &plan.fields[0];

        assert_eq!(
            container_shape.key_wire_shape,
            NetworkWireScalarShape::FixedBytes(16)
        );
        assert_eq!(container_shape.key_native_type.as_deref(), Some("AZ::Uuid"));
        assert!(plan.can_generate, "{plan:#?}");
        assert_eq!(
            field.rust_value_type.as_deref(),
            Some(
                "::nw_network::serialize::IndexMap<::uuid::Uuid, GroupFinderApplicationsGroupFinderApplication>"
            )
        );
        assert!(
            output
                .source
                .contains("pub struct GroupFinderApplicationsGroupFinderApplication")
        );
        assert!(
            output
                .source
                .contains("GroupFinderApplicationsGroupFinderApplicationMarshaler")
        );
    }

    #[test]
    fn replicated_map_container_value_groups_vec3_from_f32_lanes() {
        let schema = NetworkSchema::from_ghidra_static_network_report(&json!({
            "registryEntries": [{
                "uuid": "F467C4C2-B2AF-4AE6-A022-3167DE100779",
                "typeIndex": 4103,
                "typeName": "Javelin::SiegeWarfareDataComponentReplicatedState",
                "fields": [{
                    "index": 0,
                    "name": "fortMajorStructureStates",
                    "group": 0,
                    "handlerVtable": "NewWorld+0x82cb0a8",
                    "confidence": "register-field-call"
                }]
            }],
            "fieldRegistrationFunctions": [],
            "fieldHandlerVtables": [{
                "address": "NewWorld+0x82cb0a8",
                "fieldCount": 1,
                "deltaMarshalShapes": [
                    "vlq-u32",
                    "u8",
                    "u64",
                    "sequence-number",
                    "f32",
                    "f32",
                    "f32",
                    "u32",
                    "u8",
                    "u8"
                ],
                "fullMarshalShapes": [
                    "sequence-number",
                    "vlq-u32",
                    "u64",
                    "f32",
                    "f32",
                    "f32",
                    "u32",
                    "u8",
                    "u8"
                ],
                "valueTypeShape": {
                    "typeName": "FortMajorStructureState",
                    "typeNameFull": "FortMajorStructureState",
                    "typeNameSource": "field-name+container-slot-shape",
                    "memberNameSource": "synthetic-pcode-wire-order",
                    "memberNamesProven": false,
                    "validation": "custom-container-value-pcode-wire-sequence-fort-major-structure-state",
                    "members": [{
                        "index": 0,
                        "offset": "0x10",
                        "nativeOffset": "0x10",
                        "name": "field_0",
                        "nameSource": "synthetic-pcode-wire-order",
                        "nameProven": false,
                        "nativeType": "AZ::Vector3",
                        "wireShape": "vec3",
                        "byteWidth": 12,
                        "evidenceSource": "fort-major-structure-state-container-slot"
                    }, {
                        "index": 1,
                        "offset": "0x20",
                        "nativeOffset": "0x20",
                        "name": "field_1",
                        "nameSource": "synthetic-pcode-wire-order",
                        "nameProven": false,
                        "nativeType": "AZ::u32",
                        "wireShape": "u32",
                        "byteWidth": 4,
                        "evidenceSource": "fort-major-structure-state-container-slot"
                    }, {
                        "index": 2,
                        "offset": "0x24",
                        "nativeOffset": "0x24",
                        "name": "field_2",
                        "nameSource": "synthetic-pcode-wire-order",
                        "nameProven": false,
                        "nativeType": "AZ::u8",
                        "wireShape": "u8",
                        "byteWidth": 1,
                        "evidenceSource": "fort-major-structure-state-container-slot"
                    }, {
                        "index": 3,
                        "offset": "0x25",
                        "nativeOffset": "0x25",
                        "name": "field_3",
                        "nameSource": "synthetic-pcode-wire-order",
                        "nameProven": false,
                        "nativeType": "AZ::u8",
                        "wireShape": "u8",
                        "byteWidth": 1,
                        "evidenceSource": "fort-major-structure-state-container-slot"
                    }]
                },
                "slots": []
            }]
        }))
        .expect("schema");

        let output =
            NetworkRustEmitter::emit_replicated_states(&schema, [4103]).expect("state source");
        let plan = &output.report.state_generation_plans[0];
        let field = &plan.fields[0];

        assert!(plan.can_generate, "{plan:#?}");
        assert_eq!(
            field.rust_value_type.as_deref(),
            Some(
                "::nw_network::serialize::IndexMap<u64, FortMajorStructureStatesFortMajorStructureState>"
            )
        );
        assert!(output.source.contains("pub field_0: ::glam::Vec3"));
        assert!(
            output
                .source
                .contains("FortMajorStructureStatesFortMajorStructureStateMarshaler")
        );
    }

    #[test]
    fn replicated_map_container_can_use_fixed_bytes_key() {
        let schema = NetworkSchema::from_ghidra_static_network_report(&json!({
            "registryEntries": [{
                "uuid": "A24F4B9E-71E0-4D10-8BB6-42476298BB80",
                "typeIndex": 982,
                "typeName": "MB::LootTrackerComponentReplicatedState",
                "fields": [{
                    "index": 0,
                    "name": "m_slayerScriptDataMap",
                    "group": 0,
                    "handlerVtable": "NewWorld+0x8218ff8",
                    "confidence": "register-field-call"
                }]
            }],
            "fieldRegistrationFunctions": [],
            "fieldHandlerVtables": [{
                "address": "NewWorld+0x8218ff8",
                "fieldCount": 1,
                "deltaMarshalShapes": [
                    "vlq-u32",
                    "u8",
                    "fixed-bytes-16",
                    "sequence-number",
                    "u8",
                    "u64"
                ],
                "fullMarshalShapes": [
                    "sequence-number",
                    "vlq-u32",
                    "fixed-bytes-16",
                    "u8",
                    "u64"
                ],
                "valueTypeShape": {
                    "typeName": "SlayerScriptDataValue",
                    "typeNameFull": "SlayerScriptDataValue",
                    "typeNameSource": "field-name+container-slot-shape",
                    "memberNameSource": "synthetic-pcode-wire-order",
                    "memberNamesProven": false,
                    "validation": "custom-container-value-pcode-wire-sequence-loot-tracker-slayer-script-data",
                    "members": [{
                        "index": 0,
                        "offset": "0x0",
                        "nativeOffset": "0x0",
                        "name": "field_0",
                        "nameSource": "synthetic-pcode-wire-order",
                        "nameProven": false,
                        "nativeType": "AZ::u8",
                        "wireShape": "u8",
                        "byteWidth": 1,
                        "evidenceSource": "loot-tracker-slayer-script-data-container-slot"
                    }, {
                        "index": 1,
                        "offset": "0x8",
                        "nativeOffset": "0x8",
                        "name": "field_1",
                        "nameSource": "synthetic-pcode-wire-order",
                        "nameProven": false,
                        "nativeType": "WallClockTimePoint",
                        "wireShape": "u64",
                        "byteWidth": 16,
                        "evidenceSource": "loot-tracker-slayer-script-data-container-slot"
                    }]
                },
                "slots": []
            }]
        }))
        .expect("schema");

        let output =
            NetworkRustEmitter::emit_replicated_states(&schema, [982]).expect("state source");
        let plan = &output.report.state_generation_plans[0];
        let field = &plan.fields[0];

        assert!(plan.can_generate, "{plan:#?}");
        assert_eq!(
            field.rust_value_type.as_deref(),
            Some(
                "::nw_network::serialize::IndexMap<[u8; 16], SlayerScriptDataMapSlayerScriptDataValue>"
            )
        );
        assert!(output.source.contains("::nw_network::WallClockTimePoint"));
        assert!(
            output
                .source
                .contains("SlayerScriptDataMapSlayerScriptDataValueMarshaler")
        );
    }

    #[test]
    fn objective_task_state_vector_emits_local_support_type() {
        let schema = NetworkSchema::from_ghidra_static_network_report(&json!({
            "registryEntries": [{
                "uuid": "240B9B03-D3F9-497E-BF87-7056500137B4",
                "typeIndex": 3857,
                "typeName": "MB::ObjectivesComponentReplicatedState",
                "fields": [{
                    "index": 0,
                    "name": "taskStates",
                    "group": 0,
                    "handlerVtable": "NewWorld+0x82587e0",
                    "confidence": "register-field-call"
                }]
            }],
            "fieldRegistrationFunctions": [],
            "fieldHandlerVtables": [{
                "address": "NewWorld+0x82587e0",
                "fieldCount": 1,
                "deltaMarshalShapes": [
                    "vlq-u32",
                    "u8",
                    "vlq-u64",
                    "sequence-number",
                    "u64",
                    "u8",
                    "u32",
                    "u32",
                    "u8"
                ],
                "fullMarshalShapes": [
                    "sequence-number",
                    "vlq-u32",
                    "u64",
                    "u8",
                    "u32",
                    "u32",
                    "u8"
                ],
                "valueTypeShape": {
                    "typeName": "TaskState",
                    "typeNameFull": "TaskState",
                    "typeNameSource": "field-name+container-slot-shape",
                    "memberNameSource": "synthetic-pcode-wire-order",
                    "memberNamesProven": false,
                    "validation": "custom-container-value-pcode-wire-sequence-objective-task-state",
                    "members": [{
                        "index": 0,
                        "offset": "0x10",
                        "nativeOffset": "0x10",
                        "name": "field_0",
                        "nameSource": "synthetic-pcode-wire-order",
                        "nameProven": false,
                        "nativeType": "AZ::u64",
                        "wireShape": "u64",
                        "byteWidth": 8,
                        "evidenceSource": "objective-task-state-container-slot"
                    }, {
                        "index": 1,
                        "offset": "0x18",
                        "nativeOffset": "0x18",
                        "name": "field_1",
                        "nameSource": "synthetic-pcode-wire-order",
                        "nameProven": false,
                        "nativeType": "AZ::u8",
                        "wireShape": "u8",
                        "byteWidth": 1,
                        "evidenceSource": "objective-task-state-container-slot"
                    }, {
                        "index": 2,
                        "offset": "0x1c",
                        "nativeOffset": "0x1c",
                        "name": "field_2",
                        "nameSource": "synthetic-pcode-wire-order",
                        "nameProven": false,
                        "nativeType": "AZ::u32",
                        "wireShape": "u32",
                        "byteWidth": 4,
                        "evidenceSource": "objective-task-state-container-slot"
                    }, {
                        "index": 3,
                        "offset": "0x20",
                        "nativeOffset": "0x20",
                        "name": "field_3",
                        "nameSource": "synthetic-pcode-wire-order",
                        "nameProven": false,
                        "nativeType": "AZ::u32",
                        "wireShape": "u32",
                        "byteWidth": 4,
                        "evidenceSource": "objective-task-state-container-slot"
                    }, {
                        "index": 4,
                        "offset": "0x24",
                        "nativeOffset": "0x24",
                        "name": "field_4",
                        "nameSource": "synthetic-pcode-wire-order",
                        "nameProven": false,
                        "nativeType": "AZ::u8",
                        "wireShape": "u8",
                        "byteWidth": 1,
                        "evidenceSource": "objective-task-state-container-slot"
                    }]
                },
                "slots": []
            }]
        }))
        .expect("schema");

        let output =
            NetworkRustEmitter::emit_replicated_states(&schema, [3857]).expect("state source");
        let plan = &output.report.state_generation_plans[0];
        let field = &plan.fields[0];

        assert!(plan.can_generate, "{plan:#?}");
        assert_eq!(
            field.rust_value_type.as_deref(),
            Some("::std::vec::Vec<TaskStatesTaskState>")
        );
        assert!(output.source.contains("pub struct TaskStatesTaskState"));
        assert!(output.source.contains("TaskStatesTaskStateMarshaler"));
    }

    #[test]
    fn cooldown_timer_entry_map_emits_local_support_type() {
        let schema = NetworkSchema::from_ghidra_static_network_report(&json!({
            "registryEntries": [{
                "uuid": "DD33E4A7-3D79-4BF7-B925-57134858BE9F",
                "typeIndex": 2932,
                "typeName": "MB::CooldownTimersComponentReplicatedState",
                "fields": [{
                    "index": 0,
                    "name": "ccdmap",
                    "group": 0,
                    "handlerVtable": "NewWorld+0x855d080",
                    "confidence": "register-field-call"
                }]
            }],
            "fieldRegistrationFunctions": [],
            "fieldHandlerVtables": [{
                "address": "NewWorld+0x855d080",
                "fieldCount": 1,
                "deltaMarshalShapes": [
                    "vlq-u32",
                    "u8",
                    "u32",
                    "sequence-number",
                    "u64",
                    "u32",
                    "u32"
                ],
                "fullMarshalShapes": [
                    "sequence-number",
                    "vlq-u32",
                    "u32",
                    "u64",
                    "u32",
                    "u32"
                ],
                "valueTypeShape": {
                    "typeName": "CooldownTimerEntry",
                    "typeNameFull": "CooldownTimerEntry",
                    "typeNameSource": "field-name+container-slot-shape",
                    "memberNameSource": "synthetic-pcode-wire-order",
                    "memberNamesProven": false,
                    "validation": "custom-container-value-pcode-wire-sequence-cooldown-timer-entry",
                    "members": [{
                        "index": 0,
                        "offset": "0x20",
                        "nativeOffset": "0x20",
                        "name": "field_0",
                        "nameSource": "synthetic-pcode-wire-order",
                        "nameProven": false,
                        "nativeType": "WallClockTimePoint",
                        "wireShape": "u64",
                        "byteWidth": 16,
                        "evidenceSource": "cooldown-timer-entry-container-slot"
                    }, {
                        "index": 1,
                        "offset": "0x1c",
                        "nativeOffset": "0x1c",
                        "name": "field_1",
                        "nameSource": "synthetic-pcode-wire-order",
                        "nameProven": false,
                        "nativeType": "AZ::u32",
                        "wireShape": "u32",
                        "byteWidth": 4,
                        "evidenceSource": "cooldown-timer-entry-container-slot"
                    }, {
                        "index": 2,
                        "offset": "0x18",
                        "nativeOffset": "0x18",
                        "name": "field_2",
                        "nameSource": "synthetic-pcode-wire-order",
                        "nameProven": false,
                        "nativeType": "AZ::u32",
                        "wireShape": "u32",
                        "byteWidth": 4,
                        "evidenceSource": "cooldown-timer-entry-container-slot"
                    }]
                },
                "slots": []
            }]
        }))
        .expect("schema");

        let output =
            NetworkRustEmitter::emit_replicated_states(&schema, [2932]).expect("state source");
        let plan = &output.report.state_generation_plans[0];
        let field = &plan.fields[0];

        assert!(plan.can_generate, "{plan:#?}");
        assert_eq!(
            field.rust_value_type.as_deref(),
            Some("::nw_network::serialize::IndexMap<u32, CcdmapCooldownTimerEntry>")
        );
        assert!(output.source.contains("::nw_network::WallClockTimePoint"));
        assert!(output.source.contains("CcdmapCooldownTimerEntryMarshaler"));
    }

    #[test]
    fn ambiguous_provider_value_type_shape_matches_stay_blocked() {
        let first_type_id = uuid!("022d0c83-ee04-4e4d-9776-4dfbdaa90923");
        let second_type_id = uuid!("80a9e3d4-2cf6-44b1-b05e-c44a6f36b5db");
        let mut schema = NetworkSchema::from_ghidra_static_network_report(&json!({
            "registryEntries": [{
                "uuid": "B4DB39E2-5054-4604-9855-9A4DC75BDDE4",
                "typeIndex": 3362,
                "typeName": "MB::CraftingComponentReplicatedState",
                "fields": [{
                    "index": 0,
                    "name": "recipeCooldowns",
                    "group": 0,
                    "handlerVtable": "NewWorld+0x81bf3d0",
                    "confidence": "register-field-call"
                }]
            }],
            "fieldRegistrationFunctions": [],
            "fieldHandlerVtables": [{
                "address": "NewWorld+0x81bf3d0",
                "fieldCount": 1,
                "deltaMarshalShapes": ["vlq-u32", "u32", "sequence-number", "u8", "u64"],
                "fullMarshalShapes": ["sequence-number", "vlq-u32", "u32", "u8", "u64"],
                "valueTypeInfoCandidates": [{
                    "address": "NewWorld+0x8123450",
                    "name": "RecipeCooldownData",
                    "typeId": first_type_id.to_string(),
                    "source": "rtti-provider-vtable"
                }, {
                    "address": "NewWorld+0x8123460",
                    "name": "OtherCooldownData",
                    "typeId": second_type_id.to_string(),
                    "source": "rtti-provider-vtable"
                }],
                "slots": []
            }]
        }))
        .expect("schema");
        schema.merge_serialize_codegen_unit(
            &SerializeCodegenUnit {
                items: vec![
                    named_value_item(
                        first_type_id,
                        "RecipeCooldownData",
                        [ScalarType::U8, ScalarType::U64],
                    ),
                    named_value_item(
                        second_type_id,
                        "OtherCooldownData",
                        [ScalarType::U8, ScalarType::U64],
                    ),
                ],
            },
            Some("selection.json".to_owned()),
        );

        let output =
            NetworkRustEmitter::emit_replicated_states(&schema, [3362]).expect("state source");
        let plan = &output.report.state_generation_plans[0];
        let field = &plan.fields[0];

        assert!(!plan.can_generate);
        assert_eq!(plan.blocked_reasons, vec!["missing-semantic-type:1"]);
        assert_eq!(field.rust_field_type, None);
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
        assert!(message_output.source.contains("az_rtti"));
        assert!(message_output.source.contains("type_registry"));
    }

    #[test]
    fn reports_message_blocker_summary_with_examples() {
        let schema = NetworkSchema::from_ghidra_static_network_report(&json!({
            "registryEntries": [{
                "uuid": "11111111-1111-1111-1111-111111111111",
                "typeIndex": 1,
                "typeName": "Example::EmptyMsg",
                "capabilities": ["direct-message"],
                "fields": []
            }, {
                "uuid": "22222222-2222-2222-2222-222222222222",
                "typeIndex": 2,
                "typeName": "Example::PlaceholderMsg",
                "capabilities": ["direct-message"],
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
                "capabilities": ["direct-message"],
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
        assert_eq!(summary.generatable_count, 3);
        assert_eq!(summary.blocked_count, 0);
        assert!(summary.reason_buckets.is_empty());
        assert!(summary.combination_buckets.is_empty());
        assert!(output.source.contains("pub struct EmptyMsg"));
        let placeholder_plan = output
            .report
            .message_generation_plans
            .iter()
            .find(|plan| plan.type_name.as_deref() == Some("Example::PlaceholderMsg"))
            .expect("placeholder message plan");
        assert_eq!(placeholder_plan.placeholder_field_name_count, 1);
        assert!(placeholder_plan.can_generate);
        assert_eq!(
            placeholder_plan.fields[0].field_name.as_deref(),
            Some("ActorRef")
        );
    }

    #[test]
    fn infers_message_wire_shapes_from_native_types() {
        let schema = NetworkSchema::from_ghidra_static_network_report(&json!({
            "registryEntries": [{
                "uuid": "0B826B33-89F5-49E0-B8CB-FE4433427778",
                "typeIndex": 6179,
                "typeName": "Aoi::PhysicsTrait::ResizeAoiObserverMsg",
                "capabilities": ["direct-message"],
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
    fn infers_time_point_wrapper_native_types_as_u64() {
        let schema = NetworkSchema::from_ghidra_static_network_report(&json!({
            "registryEntries": [{
                "uuid": "0B826B33-89F5-49E0-B8CB-FE4433427778",
                "typeIndex": 6179,
                "typeName": "Example::TimerMsg",
                "capabilities": ["direct-message"],
                "fields": [{
                    "index": 0,
                    "name": "StartedAt",
                    "nativeType": "MB::TimePoint",
                    "confidence": "message-unmarshal-call"
                }, {
                    "index": 1,
                    "name": "WallClock",
                    "nativeType": "MB::WallClockTimePoint",
                    "confidence": "message-unmarshal-call"
                }]
            }],
            "fieldRegistrationFunctions": []
        }))
        .expect("schema");

        let output = NetworkRustEmitter::emit_messages(&schema).expect("message source");

        assert_eq!(output.report.generatable_message_count, 1);
        let plan = &output.report.message_generation_plans[0];
        assert_eq!(plan.missing_wire_shape_count, 0);
        assert_eq!(plan.fields[0].wire_shape, Some(SchemaWireShape::U64));
        assert_eq!(plan.fields[1].wire_shape, Some(SchemaWireShape::U64));
        assert!(output.source.contains("pub started_at: u64"));
        assert!(output.source.contains("pub wall_clock: u64"));
    }

    #[test]
    fn emits_message_structs_with_native_type_field_names() {
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
        assert_eq!(output.report.generatable_message_count, 2);
        assert_eq!(output.report.blocked_message_count, 0);
        for plan in &output.report.message_generation_plans {
            assert_eq!(plan.placeholder_field_name_count, 1);
            assert!(plan.blocked_reasons.is_empty());
        }
        assert!(output.source.contains("pub struct ResizeAoiObservableMsg"));
        assert!(output.source.contains("pub struct SetTargetsMsg"));
    }

    #[test]
    fn emits_message_structs_with_placeholder_field_names() {
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
        assert_eq!(output.report.generatable_message_count, 1);
        assert_eq!(output.report.blocked_message_count, 0);
        assert_eq!(
            output.report.message_generation_plans[0].placeholder_field_name_count,
            1
        );
        assert!(
            output.report.message_generation_plans[0]
                .blocked_reasons
                .is_empty()
        );
        assert!(output.source.contains("pub struct PingMsg"));
        assert!(output.source.contains("pub field_0: u64"));
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
    fn resolves_existing_message_support_types_from_unmarshal_evidence() {
        let schema = NetworkSchema::from_ghidra_static_network_report(&json!({
            "registryEntries": [{
                "uuid": "57735773-5773-4773-9773-577357735773",
                "typeIndex": 5773,
                "typeName": "Javelin::ClientMessages::InventoriesComponentServerFacet_UpdateItemBatch",
                "capabilities": ["direct-message"],
                "fields": [{
                    "index": 0,
                    "name": "ActorRequestId",
                    "nativeType": "ActorRequestId",
                    "sourceTypeName": "ActorRequestId",
                    "unmarshalEvidence": {
                        "callsite": "NewWorld+0x35f48ef",
                        "targetName": "Javelin::ClientMessages::ActorRequestId::Unmarshal",
                        "targetKind": "direct-unmarshal",
                        "evidenceSource": "message-unmarshal-pcode-call"
                    },
                    "nestedTypeShape": {
                        "typeName": "ActorRequestId",
                        "typeNameFull": "Javelin::ClientMessages::ActorRequestId",
                        "typeNameSource": "ghidra-symbol",
                        "function": "NewWorld+0x35f4000",
                        "functionName": "Javelin::ClientMessages::ActorRequestId::Unmarshal",
                        "memberBase": "param_1",
                        "memberNameSource": "synthetic-offset",
                        "memberNamesProven": false,
                        "validation": "layout-consistent-two-u64",
                        "members": [{
                            "index": 0,
                            "offset": "0x0",
                            "name": "_0",
                            "nameSource": "synthetic-offset",
                            "nameProven": false,
                            "nativeType": "u64",
                            "wireShape": "u64",
                            "byteWidth": 8,
                            "evidenceSource": "pcode-call"
                        }, {
                            "index": 1,
                            "offset": "0x8",
                            "name": "_1",
                            "nameSource": "synthetic-offset",
                            "nameProven": false,
                            "nativeType": "u64",
                            "wireShape": "u64",
                            "byteWidth": 8,
                            "evidenceSource": "pcode-call"
                        }]
                    },
                    "confidence": "message-unmarshal-pcode-call"
                }]
            }, {
                "uuid": "57745774-5774-4774-9774-577457745774",
                "typeIndex": 5774,
                "typeName": "Javelin::ClientMessages::InventoriesComponentServerFacet_UpdateItemBatchWithoutEvidence",
                "capabilities": ["direct-message"],
                "fields": [{
                    "index": 0,
                    "name": "ActorRequestId",
                    "nativeType": "ActorRequestId",
                    "sourceTypeName": "ActorRequestId",
                    "confidence": "message-unmarshal-whole-helper-direct-type"
                }]
            }, {
                "uuid": "34773477-3477-4477-9477-347734773477",
                "typeIndex": 3477,
                "typeName": "GroupsComponentClientFacet_OnGroupFinderAddMemberSuccessMsg",
                "capabilities": ["direct-message"],
                "fields": [{
                    "index": 0,
                    "name": "ActorRequestIdPayload",
                    "nativeType": "composite",
                    "sourceTypeName": "ActorRequestIdPayload,ActorRequestId",
                    "unmarshalEvidence": {
                        "callsite": "NewWorld+0x340e9b1",
                        "targetName": "Javelin::ClientMessages::ActorRequestIdPayload::Unmarshal",
                        "targetKind": "direct-unmarshal",
                        "evidenceSource": "message-unmarshal-pcode-call"
                    },
                    "nestedTypeShape": {
                        "typeName": "ActorRequestId",
                        "typeNameFull": "Javelin::ClientMessages::ActorRequestId",
                        "typeNameSource": "ghidra-symbol",
                        "functionName": "Javelin::ClientMessages::ActorRequestId::Unmarshal",
                        "memberNamesProven": false,
                        "validation": "layout-consistent-two-u64",
                        "members": [{
                            "index": 0,
                            "offset": "0x0",
                            "nativeType": "u64",
                            "wireShape": "u64",
                            "byteWidth": 8,
                            "nameProven": true
                        }, {
                            "index": 1,
                            "offset": "0x8",
                            "nativeType": "u64",
                            "wireShape": "u64",
                            "byteWidth": 8,
                            "nameProven": false
                        }]
                    },
                    "confidence": "message-unmarshal-pcode-call"
                }]
            }, {
                "uuid": "34783478-3478-4478-9478-347834783478",
                "typeIndex": 3478,
                "typeName": "GroupsComponentClientFacet_OnGroupFinderClearMemberSuccessMsg",
                "capabilities": ["direct-message"],
                "fields": [{
                    "index": 0,
                    "name": "ActorRequestIdPayload",
                    "nativeType": "ActorRequestIdPayload",
                    "sourceTypeName": "ActorRequestIdPayload",
                    "unmarshalEvidence": {
                        "callsite": "NewWorld+0x340e9e1",
                        "targetName": "Javelin::ClientMessages::ActorRequestIdPayload::Unmarshal",
                        "targetKind": "direct-unmarshal",
                        "evidenceSource": "message-unmarshal-pcode-call"
                    },
                    "confidence": "message-unmarshal-pcode-call"
                }]
            }],
            "fieldRegistrationFunctions": []
        }))
        .expect("schema");

        let output = NetworkRustEmitter::emit_messages(&schema).expect("message source");

        assert_eq!(output.report.message_generation_plan_count, 4);
        assert_eq!(output.report.generatable_message_count, 3);
        assert_eq!(output.report.blocked_message_count, 1);

        let resolved_plan = output
            .report
            .message_generation_plans
            .iter()
            .find(|plan| plan.type_index == Some(5773))
            .expect("resolved support type plan");
        assert_eq!(resolved_plan.missing_support_type_count, 0);
        assert!(resolved_plan.blocked_reasons.is_empty());
        assert_eq!(
            resolved_plan.fields[0].rust_value_type.as_deref(),
            Some("::nw_network::ActorRequestId")
        );
        assert_eq!(resolved_plan.fields[0].blocked_reason, None);
        assert!(
            output
                .source
                .contains("pub actor_request_id: ::nw_network::ActorRequestId")
        );

        let unresolved_plan = output
            .report
            .message_generation_plans
            .iter()
            .find(|plan| plan.type_index == Some(5774))
            .expect("support type plan");
        assert_eq!(unresolved_plan.missing_support_type_count, 1);
        assert_eq!(unresolved_plan.missing_composite_support_type_count, 0);
        assert_eq!(
            unresolved_plan.blocked_reasons,
            vec!["missing-support-type:1"]
        );
        assert_eq!(
            unresolved_plan.fields[0].source_type_name.as_deref(),
            Some("ActorRequestId")
        );
        assert_eq!(
            unresolved_plan.fields[0].blocked_reason.as_deref(),
            Some("missing-support-type")
        );

        let composite_plan = output
            .report
            .message_generation_plans
            .iter()
            .find(|plan| plan.type_index == Some(3477))
            .expect("composite type plan");
        assert_eq!(composite_plan.missing_support_type_count, 0);
        assert_eq!(composite_plan.missing_composite_support_type_count, 0);
        assert!(composite_plan.blocked_reasons.is_empty());
        assert_eq!(
            composite_plan.fields[0].source_type_name.as_deref(),
            Some("ActorRequestIdPayload,ActorRequestId")
        );
        assert_eq!(
            composite_plan.fields[0].rust_value_type.as_deref(),
            Some("::nw_network::ActorRequestId")
        );
        assert_eq!(composite_plan.fields[0].blocked_reason, None);
        assert!(
            output
                .source
                .contains("pub actor_request_id_payload: ::nw_network::ActorRequestId")
        );

        let direct_payload_plan = output
            .report
            .message_generation_plans
            .iter()
            .find(|plan| plan.type_index == Some(3478))
            .expect("direct payload plan");
        assert_eq!(direct_payload_plan.missing_support_type_count, 0);
        assert_eq!(direct_payload_plan.missing_composite_support_type_count, 0);
        assert!(direct_payload_plan.blocked_reasons.is_empty());
        assert_eq!(
            direct_payload_plan.fields[0].rust_value_type.as_deref(),
            Some("::nw_network::ActorRequestId")
        );

        let support_bucket = output
            .report
            .message_blocker_summary
            .reason_buckets
            .iter()
            .find(|bucket| bucket.reason == "missing-support-type")
            .expect("support blocker bucket");
        assert_eq!(
            support_bucket.examples[0].blocked_fields[0]
                .source_type_name
                .as_deref(),
            Some("ActorRequestId")
        );
        assert_eq!(
            support_bucket.examples[0].blocked_fields[0]
                .native_type
                .as_deref(),
            Some("ActorRequestId")
        );
    }

    #[test]
    fn emits_message_support_structs_from_proven_nested_shapes() {
        let schema = NetworkSchema::from_ghidra_static_network_report(&json!({
            "registryEntries": [{
                "uuid": "49334933-4933-4933-8933-493349334933",
                "typeIndex": 4933,
                "typeName": "Javelin::ClientMessages::RewardTrackComponentServerFacet_DebugRefreshRewards",
                "capabilities": ["direct-message"],
                "fields": [{
                    "index": 0,
                    "name": "ActorRequestIdBoolPayload",
                    "nativeType": "ActorRequestIdBoolPayload",
                    "sourceTypeName": "ActorRequestIdBoolPayload,ActorRequestId",
                    "unmarshalEvidence": {
                        "callsite": "NewWorld+0x37021b5",
                        "targetName": "Javelin::ClientMessages::ActorRequestIdBoolPayload::Unmarshal",
                        "targetKind": "direct-unmarshal",
                        "evidenceSource": "message-unmarshal-pcode-call"
                    },
                    "nestedTypeShape": {
                        "typeName": "ActorRequestIdBoolPayload",
                        "typeNameFull": "Javelin::ClientMessages::ActorRequestIdBoolPayload",
                        "typeNameSource": "ghidra-symbol",
                        "function": "NewWorld+0x25a2110",
                        "functionName": "Javelin::ClientMessages::ActorRequestIdBoolPayload::Unmarshal",
                        "memberBase": "param_1",
                        "memberNameSource": "synthetic-offset",
                        "memberNamesProven": false,
                        "validation": "layout-consistent-direct-type",
                        "members": [{
                            "index": 0,
                            "offset": "0x0",
                            "name": "_0",
                            "nameSource": "synthetic-offset",
                            "nameProven": false,
                            "nativeType": "u64",
                            "wireShape": "u64",
                            "byteWidth": 8
                        }, {
                            "index": 1,
                            "offset": "0x8",
                            "name": "_1",
                            "nameSource": "synthetic-offset",
                            "nameProven": false,
                            "nativeType": "u64",
                            "wireShape": "u64",
                            "byteWidth": 8
                        }, {
                            "index": 2,
                            "offset": "0x20",
                            "name": "value",
                            "nameSource": "synthetic-offset",
                            "nameProven": false,
                            "nativeType": "bool",
                            "wireShape": "bool",
                            "byteWidth": 1
                        }]
                    },
                    "confidence": "message-unmarshal-pcode-call"
                }]
            }],
            "fieldRegistrationFunctions": []
        }))
        .expect("schema");

        let output = NetworkRustEmitter::emit_messages(&schema).expect("message source");

        assert_eq!(output.report.generatable_message_count, 1);
        assert_eq!(output.report.blocked_message_count, 0);
        let plan = &output.report.message_generation_plans[0];
        assert_eq!(plan.fields[0].blocked_reason, None);
        assert_eq!(
            plan.fields[0].rust_value_type.as_deref(),
            Some("ActorRequestIdBoolPayload")
        );
        assert!(
            output
                .source
                .contains("pub struct ActorRequestIdBoolPayload")
        );
        assert!(
            output
                .source
                .contains("pub actor_request_id_bool_payload: ActorRequestIdBoolPayload")
        );
        assert!(
            output
                .source
                .contains("impl ::nw_network::serialize::Marshaler for ActorRequestIdBoolPayload")
        );
    }

    #[test]
    fn emits_bounded_u8_fixed_vector_message_fields() {
        let schema = NetworkSchema::from_ghidra_static_network_report(&json!({
            "registryEntries": [{
                "uuid": "18141814-1814-4814-9814-181418141814",
                "typeIndex": 1814,
                "typeName": "Javelin::ClientMessages::ObjectivesComponentServerFacet_AddObjectiveFromRecipe",
                "capabilities": ["direct-message"],
                "fields": [{
                    "index": 0,
                    "name": "field_1",
                    "nativeType": "AZStd::fixed_vector<AZ::u8,64>",
                    "unmarshalEvidence": {
                        "callsite": "NewWorld+0x39e937d",
                        "targetName": "GridMate::Marshaler<AZStd::fixed_vector<AZ::u8,64>>::Unmarshal",
                        "targetKind": "whole-helper-marshaler",
                        "evidenceSource": "message-unmarshal-whole-helper-marshaler"
                    },
                    "confidence": "message-unmarshal-whole-helper-marshaler"
                }]
            }],
            "fieldRegistrationFunctions": []
        }))
        .expect("schema");

        let output = NetworkRustEmitter::emit_messages(&schema).expect("message source");

        assert_eq!(output.report.generatable_message_count, 1);
        assert_eq!(output.report.blocked_message_count, 0);
        let plan = &output.report.message_generation_plans[0];
        assert_eq!(plan.fields[0].blocked_reason, None);
        assert_eq!(
            plan.fields[0].rust_value_type.as_deref(),
            Some("::arrayvec::ArrayVec<u8, 64>")
        );
    }

    #[test]
    fn emits_actor_ref_for_proxy_address_message_fields() {
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
            }, {
                "uuid": "17117117-1711-4711-9711-171171171171",
                "typeIndex": 171,
                "typeName": "ConfigOverridesDebugTrait::SendConfigOverridesMsg",
                "fields": [{
                    "index": 0,
                    "name": "ProxyAddress",
                    "nativeType": "composite",
                    "sourceTypeName": "ProxyAddress,ActorRef",
                    "confidence": "message-unmarshal-pcode-call",
                    "nestedTypeShape": {
                        "typeName": "ProxyAddress",
                        "typeNameFull": "Amazon::Hub::ProxyAddress",
                        "typeNameSource": "ghidra-symbol",
                        "validation": "layout-consistent-direct-type",
                        "members": [{
                            "index": 0,
                            "offset": "0x0",
                            "nativeType": "u32",
                            "wireShape": "u32",
                            "byteWidth": 4
                        }, {
                            "index": 1,
                            "offset": "0x4",
                            "nativeType": "fixed-bytes-16",
                            "wireShape": "fixed-bytes-16",
                            "byteWidth": 16
                        }, {
                            "index": 2,
                            "offset": "0x14",
                            "nativeType": "fixed-bytes-16",
                            "wireShape": "fixed-bytes-16",
                            "byteWidth": 16
                        }]
                    }
                }]
            }],
            "fieldRegistrationFunctions": []
        }))
        .expect("schema");

        let output = NetworkRustEmitter::emit_messages(&schema).expect("message source");

        assert_eq!(output.report.generatable_message_count, 2);
        assert_eq!(output.report.blocked_message_count, 0);
        assert!(
            output
                .source
                .contains("pub proxy_ref: ::nw_network::ActorRef")
        );
        assert!(
            output
                .source
                .contains("pub field_0: ::nw_network::ActorRef")
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
                .contains("pub proxy_ref: ::nw_network::ActorRef")
        );
        assert!(
            output
                .source
                .contains("pub target_ref: ::nw_network::ActorRef")
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
    fn emits_selected_serialize_enum_message_field_from_source_type_id() {
        let grid_sides_type_id = uuid!("ffe86b09-16b9-429e-9cd2-2901adbe8de3");
        let mut schema = NetworkSchema::from_ghidra_static_network_report(&json!({
            "registryEntries": [{
                "uuid": "0B826B33-89F5-49E0-B8CB-FE4433427778",
                "typeIndex": 19,
                "typeName": "GridSideMsg",
                "capabilities": ["direct-message"],
                "fields": [{
                    "index": 0,
                    "name": "GridSide",
                    "sourceTypeId": grid_sides_type_id.to_string(),
                    "wireShape": "u8",
                    "confidence": "message-unmarshal-call"
                }]
            }],
            "fieldRegistrationFunctions": []
        }))
        .expect("schema");
        let unit = SerializeCodegenUnit {
            items: vec![grid_sides_enum_item(grid_sides_type_id)],
        };
        schema.merge_serialize_codegen_unit(&unit, Some("selection.json".to_owned()));

        let output = NetworkRustEmitter::emit_messages(&schema).expect("message source");

        assert_eq!(output.report.generatable_message_count, 1);
        let field = &output.report.message_generation_plans[0].fields[0];
        assert_eq!(field.source_type_id, Some(grid_sides_type_id));
        assert_eq!(field.serialize_type_name.as_deref(), Some("GridSides"));
        assert_eq!(
            field.rust_value_type.as_deref(),
            Some("::nw_network::source::GridSides")
        );
        assert!(
            output
                .source
                .contains("pub grid_side: ::nw_network::source::GridSides")
        );
        assert!(output.source.contains(
            "::nw_network::serialize::ConversionMarshaler<u8, ::nw_network::source::GridSides>"
        ));
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
                "capabilities": ["replicated-state"],
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
    fn emits_selected_serialize_enum_replicated_state_field_from_source_type_id() {
        let grid_sides_type_id = uuid!("ffe86b09-16b9-429e-9cd2-2901adbe8de3");
        let mut schema = NetworkSchema::from_ghidra_static_network_report(&json!({
            "registryEntries": [{
                "uuid": "A85DF621-DCE0-409F-8D39-A447EA0807FF",
                "typeIndex": 28,
                "typeName": "Javelin::GridSideReplicatedState",
                "capabilities": ["replicated-state"],
                "fields": [{
                    "index": 0,
                    "name": "GridSide",
                    "group": 0,
                    "sourceTypeId": grid_sides_type_id.to_string(),
                    "wireShape": "u8",
                    "confidence": "exact"
                }]
            }],
            "fieldRegistrationFunctions": []
        }))
        .expect("schema");
        let unit = SerializeCodegenUnit {
            items: vec![grid_sides_enum_item(grid_sides_type_id)],
        };
        schema.merge_serialize_codegen_unit(&unit, Some("selection.json".to_owned()));

        let output =
            NetworkRustEmitter::emit_replicated_states(&schema, [28]).expect("state source");

        assert_eq!(output.report.generatable_state_count, 1);
        let field = &output.report.state_generation_plans[0].fields[0];
        assert_eq!(field.source_type_id, Some(grid_sides_type_id));
        assert_eq!(field.serialize_type_name.as_deref(), Some("GridSides"));
        assert_eq!(
            field.rust_value_type.as_deref(),
            Some("::nw_network::source::GridSides")
        );
        assert!(output.source.contains("ReplicatedFieldHandler<"));
        assert!(output.source.contains("::nw_network::source::GridSides"));
        assert!(output.source.contains("ConversionMarshaler<"));
        assert!(output.source.contains("u8,"));
    }

    #[test]
    fn emits_selected_serialize_struct_message_field_from_source_type_id() {
        let payload_type_id = uuid!("da4e5889-a65c-4480-8642-0278160125a7");
        let mut schema = NetworkSchema::from_ghidra_static_network_report(&json!({
            "registryEntries": [{
                "uuid": "0B826B33-89F5-49E0-B8CB-FE4433427778",
                "typeIndex": 19,
                "typeName": "PayloadMsg",
                "capabilities": ["direct-message"],
                "fields": [{
                    "index": 0,
                    "name": "Payload",
                    "nativeType": "PayloadData",
                    "sourceTypeName": "PayloadData",
                    "sourceTypeId": payload_type_id.to_string(),
                    "confidence": "message-unmarshal-direct-type"
                }]
            }],
            "fieldRegistrationFunctions": []
        }))
        .expect("schema");
        let unit = SerializeCodegenUnit {
            items: vec![SerializeCodegenItem {
                source_type_id: payload_type_id,
                source_name: "PayloadData".to_owned(),
                role: crate::role::ReflectedTypeRole::SupportType,
                is_reflection_marker: false,
                is_abstract: Some(false),
                factory: None,
                rtti_base_chain: Vec::new(),
                kind: SerializeCodegenItemKind::Struct,
                enum_underlying_type: None,
                fields: Vec::new(),
                variants: Vec::new(),
            }],
        };
        schema.merge_serialize_codegen_unit(&unit, Some("selection.json".to_owned()));

        let output = NetworkRustEmitter::emit_messages(&schema).expect("message source");

        assert_eq!(output.report.generatable_message_count, 1);
        let field = &output.report.message_generation_plans[0].fields[0];
        assert_eq!(field.source_type_id, Some(payload_type_id));
        assert_eq!(field.serialize_type_name.as_deref(), Some("PayloadData"));
        assert_eq!(
            field.rust_value_type.as_deref(),
            Some("::nw_network::source::PayloadData")
        );
        assert_eq!(field.blocked_reason, None);
        assert_eq!(
            field.rust_field_type.as_deref(),
            Some("::nw_network::source::PayloadData")
        );
        assert!(output.source.contains("pub struct PayloadMsg"));
        assert!(
            output
                .source
                .contains("pub payload: ::nw_network::source::PayloadData")
        );
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
    fn emits_struct_marshaler_for_signed_enum_fields() {
        let enum_type_id = uuid!("99ffbb9b-34a3-44a1-a576-1d13d732b0aa");
        let enum_item = SerializeCodegenItem {
            source_type_id: enum_type_id,
            source_name: "SettlementProgressionCategory".to_owned(),
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
                    source_name: "None".to_owned(),
                    value_u64: None,
                    value_u32: None,
                    value_i32: Some(-1),
                },
                SerializeCodegenVariant {
                    source_name: "Blacksmithing".to_owned(),
                    value_u64: Some(0),
                    value_u32: Some(0),
                    value_i32: Some(0),
                },
            ],
        };
        let struct_item = SerializeCodegenItem {
            source_type_id: uuid!("27362f56-9317-40ce-8caa-69d5d8f75450"),
            source_name: "TerritoryUpgradeData".to_owned(),
            role: crate::role::ReflectedTypeRole::SupportType,
            is_reflection_marker: false,
            is_abstract: Some(false),
            factory: None,
            rtti_base_chain: Vec::new(),
            kind: SerializeCodegenItemKind::Struct,
            enum_underlying_type: None,
            fields: vec![
                SerializeCodegenField {
                    source_name: "m_category".to_owned(),
                    source_type_id: enum_type_id,
                    resolved_type: ResolvedType::Named {
                        type_id: enum_type_id,
                        source_name: "SettlementProgressionCategory".to_owned(),
                    },
                    data_size: None,
                    offset: None,
                    flags: None,
                    is_base_class: false,
                    is_pointer: false,
                    is_dynamic_field: false,
                },
                SerializeCodegenField {
                    source_name: "m_level".to_owned(),
                    source_type_id: Uuid::nil(),
                    resolved_type: ResolvedType::Scalar(ScalarType::U8),
                    data_size: None,
                    offset: None,
                    flags: None,
                    is_base_class: false,
                    is_pointer: false,
                    is_dynamic_field: false,
                },
            ],
            variants: Vec::new(),
        };

        let output = NetworkRustEmitter::emit_marshaler_conversions([&enum_item, &struct_item])
            .expect("conversion source");

        assert!(output.source.contains(
            "impl ::nw_network::serialize::Marshaler for ::nw_network::source::TerritoryUpgradeData"
        ));
        assert!(
            output
                .source
                .contains("let raw = i32::from(self.category);")
        );
        assert!(output.source.contains("min: 0u64"));
        assert!(output.source.contains("max: 0u64"));
    }

    fn grid_sides_enum_item(type_id: Uuid) -> SerializeCodegenItem {
        SerializeCodegenItem {
            source_type_id: type_id,
            source_name: "GridSides".to_owned(),
            role: crate::role::ReflectedTypeRole::SupportType,
            is_reflection_marker: false,
            is_abstract: Some(false),
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
        }
    }

    fn example_value_item<const N: usize>(
        type_id: Uuid,
        fields: [ScalarType; N],
    ) -> SerializeCodegenItem {
        named_value_item(type_id, "ExampleValue", fields)
    }

    fn named_value_item<const N: usize>(
        type_id: Uuid,
        source_name: &str,
        fields: [ScalarType; N],
    ) -> SerializeCodegenItem {
        SerializeCodegenItem {
            source_type_id: type_id,
            source_name: source_name.to_owned(),
            role: crate::role::ReflectedTypeRole::SupportType,
            is_reflection_marker: false,
            is_abstract: Some(false),
            factory: None,
            rtti_base_chain: Vec::new(),
            kind: SerializeCodegenItemKind::Struct,
            enum_underlying_type: None,
            fields: fields
                .into_iter()
                .enumerate()
                .map(|(index, scalar)| SerializeCodegenField {
                    source_name: format!("m_field{index}"),
                    source_type_id: Uuid::nil(),
                    resolved_type: ResolvedType::Scalar(scalar),
                    data_size: None,
                    offset: None,
                    flags: None,
                    is_base_class: false,
                    is_pointer: false,
                    is_dynamic_field: false,
                })
                .collect(),
            variants: Vec::new(),
        }
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
