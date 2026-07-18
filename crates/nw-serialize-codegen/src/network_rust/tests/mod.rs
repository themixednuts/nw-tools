use serde_json::json;
use uuid::uuid;

use crate::{
    ir::{SerializeCodegenField, SerializeCodegenUnit, SerializeCodegenVariant},
    network_schema::{NetworkMessageFieldSignature, NetworkMessageSignature, NetworkSchema},
};

use super::*;

mod container;
mod conversion;
mod descriptor;
mod identity;
mod message;
mod state;
mod structured_values;

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
