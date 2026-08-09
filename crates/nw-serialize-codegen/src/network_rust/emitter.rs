use super::*;

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
        let handler_vtables = handler_vtables_by_address(schema);
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
            &handler_vtables,
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
            pub struct NetworkPackedPositionWireShape {
                pub minimum_bits: u32,
                pub maximum_bits: u32,
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
                Vec3SmallestThree,
                QuatComp,
                QuatSmallestThree,
                NonUniformScaleComp,
                DeltaVec3(u32),
                RemoteServerGdeRef,
                PackedPosition(NetworkPackedPositionWireShape),
                TransformCompressor,
                PackedSize,
                Mat3,
                Affine3,
                Aabb2d,
                Aabb3d,
                ActorRef,
                EntityRef,
                FixedBytes(u16),
                Bytes,
                String,
            }

            #[derive(Debug, Clone, Copy, PartialEq, Eq)]
            pub struct NetworkReplicatedContainerWireShape {
                pub key: NetworkWireScalarShape,
                pub value: NetworkWireScalarShape,
            }

            #[derive(Debug, Clone, Copy, PartialEq, Eq)]
            pub struct NetworkFixedSequenceWireShape {
                pub element: &'static NetworkWireShape,
                pub capacity: u16,
            }

            #[derive(Debug, Clone, Copy, PartialEq, Eq)]
            pub enum NetworkBitMaskMemberWireShape {
                Required(&'static NetworkWireShape),
                Masked {
                    mask: u8,
                    value: &'static NetworkWireShape,
                },
            }

            #[derive(Debug, Clone, Copy, PartialEq, Eq)]
            pub struct NetworkBitMaskCompositeWireShape {
                pub mask: NetworkWireScalarShape,
                pub members: &'static [NetworkBitMaskMemberWireShape],
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
                Vec3SmallestThree,
                QuatComp,
                QuatSmallestThree,
                NonUniformScaleComp,
                DeltaVec3(u32),
                RemoteServerGdeRef,
                PackedPosition(NetworkPackedPositionWireShape),
                TransformCompressor,
                PackedSize,
                Mat3,
                Affine3,
                Aabb2d,
                Aabb3d,
                ActorRef,
                EntityRef,
                FixedBytes(u16),
                Bytes,
                String,
                ClassValue,
                ActorInstantiationParameters,
                Composite(&'static [NetworkWireShape]),
                Optional(&'static NetworkWireShape),
                DefaultOmitted(&'static [NetworkWireShape]),
                BooleanChoice {
                    false_value: &'static NetworkWireShape,
                    true_value: &'static NetworkWireShape,
                },
                BitMaskComposite(NetworkBitMaskCompositeWireShape),
                Sequence(&'static NetworkWireShape),
                Set(&'static NetworkWireShape),
                Map {
                    key: &'static NetworkWireShape,
                    value: &'static NetworkWireShape,
                },
                ReplicatedContainer(NetworkReplicatedContainerWireShape),
                FixedSequence(NetworkFixedSequenceWireShape),
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
        let handler_vtables = handler_vtables_by_address(schema);
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
                            &handler_vtables,
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
                report.support_type_count += plan
                    .fields
                    .iter()
                    .filter_map(|field| field.fixed_sequence.as_ref())
                    .filter(|sequence| sequence.generates_support_type)
                    .filter_map(|sequence| sequence.element_type_id)
                    .collect::<BTreeSet<_>>()
                    .len();
                modules.push(replicated_state_module_tokens(
                    network_type,
                    plan,
                    &rust_names,
                    &options,
                    &serialize_types,
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
        let serialize_types = serialize_types_by_type_id(schema);
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
                    &serialize_types,
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
                let mut support_names = BTreeSet::new();
                report.support_type_count += plan
                    .fields
                    .iter()
                    .filter_map(|field| {
                        message_field_support_tokens(field, &mut support_names, &serialize_types)
                    })
                    .count();
                modules.push(message_module_tokens(
                    network_type,
                    &plan,
                    &rust_names,
                    &serialize_types,
                ));
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
