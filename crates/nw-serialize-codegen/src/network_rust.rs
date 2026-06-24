use quote::{format_ident, quote};
use serde::{Deserialize, Serialize};
use syn::LitStr;
use thiserror::Error;
use uuid::Uuid;

use crate::network_schema::{
    NetworkConfidence, NetworkField, NetworkRootKind, NetworkSchema, NetworkType,
};

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
    pub field_descriptor_count: usize,
    pub skipped_missing_type_id: usize,
    pub skipped_missing_type_index: usize,
    pub skipped_missing_name: usize,
    pub replicated_state_count: usize,
    pub message_count: usize,
    pub field_registered_count: usize,
    pub support_type_count: usize,
    pub low_confidence_field_count: usize,
}

#[derive(Debug, Default)]
pub struct NetworkRustEmitter;

impl NetworkRustEmitter {
    pub fn emit_descriptors(
        schema: &NetworkSchema,
    ) -> Result<NetworkRustOutput, NetworkRustEmitError> {
        let mut report = NetworkRustGenerationReport::default();
        let descriptors = schema
            .types
            .iter()
            .filter_map(|network_type| descriptor_tokens(network_type, &mut report))
            .collect::<Vec<_>>();
        report.descriptor_count = descriptors.len();

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
            pub struct NetworkFieldDescriptor {
                pub index: u32,
                pub name: &'static str,
                pub group: Option<u32>,
                pub confidence: NetworkFieldConfidence,
            }

            #[derive(Debug, Clone, Copy, PartialEq, Eq)]
            pub struct NetworkTypeDescriptor {
                pub type_id: Uuid,
                pub type_index: u32,
                pub name: &'static str,
                pub kind: NetworkTypeKind,
                pub is_field_registered: bool,
                pub fields: &'static [NetworkFieldDescriptor],
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
                type_by_type_index(type_index).map(|descriptor| descriptor.name)
            }

            #[must_use]
            pub fn is_known_type_index(type_index: u32) -> bool {
                type_by_type_index(type_index).is_some()
            }

            #[must_use]
            pub fn fields_for_type_index(
                type_index: u32,
            ) -> Option<&'static [NetworkFieldDescriptor]> {
                type_by_type_index(type_index).map(|descriptor| descriptor.fields)
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
        };
        let file = syn::parse2(tokens)?;
        Ok(NetworkRustOutput {
            source: prettyplease::unparse(&file),
            report,
        })
    }
}

fn descriptor_tokens(
    network_type: &NetworkType,
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
    let name = match network_type.name.as_deref() {
        Some(name) => LitStr::new(name, proc_macro2::Span::call_site()),
        None => {
            report.skipped_missing_name += 1;
            return None;
        }
    };
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
        .filter_map(|field| field_tokens(field, report))
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
    report: &mut NetworkRustGenerationReport,
) -> Option<proc_macro2::TokenStream> {
    let index = field.index?;
    let name = field.name.as_deref()?;
    if !field.confidence.is_high_or_exact() {
        report.low_confidence_field_count += 1;
    }
    let name = LitStr::new(name, proc_macro2::Span::call_site());
    let group = option_u32_tokens(field.group);
    let confidence = confidence_ident(field.confidence);
    Some(quote! {
        NetworkFieldDescriptor {
            index: #index,
            name: #name,
            group: #group,
            confidence: NetworkFieldConfidence::#confidence,
        }
    })
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

fn option_u32_tokens(value: Option<u32>) -> proc_macro2::TokenStream {
    match value {
        Some(value) => quote!(Some(#value)),
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
        let schema = NetworkSchema::from_replicated_state_ghidra_report(&json!({
            "registryEntries": [{
                "uuid": "A85DF621-DCE0-409F-8D39-A447EA0807FF",
                "typeIndex": 28,
                "typeName": "Javelin::RaidDataComponentReplicatedState",
                "fields": [{
                    "index": 0,
                    "name": "raidId",
                    "group": 0,
                    "confidence": "register-field-call"
                }]
            }],
            "fieldRegistrationFunctions": []
        }))
        .expect("schema");

        let output = NetworkRustEmitter::emit_descriptors(&schema).expect("rust source");

        assert_eq!(output.report.descriptor_count, 1);
        assert_eq!(output.report.field_descriptor_count, 1);
        assert_eq!(output.report.replicated_state_count, 1);
        assert!(
            output
                .source
                .contains("pub const NETWORK_TYPES: &[NetworkTypeDescriptor]")
        );
        assert!(
            output
                .source
                .contains("Javelin::RaidDataComponentReplicatedState")
        );
        assert!(output.source.contains("raidId"));
        assert!(output.source.contains("unknown_type_indices"));
    }
}
