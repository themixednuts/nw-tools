use crate::ir::{
    SerializeCodegenItem, SerializeCodegenItemKind, SerializeCodegenRttiBase, SerializeCodegenUnit,
};
use crate::role::ReflectedTypeRole;
use serde_json::json;
use uuid::uuid;

use super::*;

mod ingest;
mod merge;

#[test]
fn parses_nested_collection_member_wire_shapes() {
    use crate::network_schema::parse::nested_member_wire_shapes;

    let shapes = nested_member_wire_shapes(
        "vec<composite<u64,vec<composite<u8,u64,u64,u64>>,u64>>",
        &[],
    )
    .expect("nested collection wire shape");

    assert_eq!(
        shapes,
        vec![
            NetworkWireScalarShape::VlqU32,
            NetworkWireScalarShape::U64,
            NetworkWireScalarShape::VlqU32,
            NetworkWireScalarShape::U8,
            NetworkWireScalarShape::U64,
            NetworkWireScalarShape::U64,
            NetworkWireScalarShape::U64,
            NetworkWireScalarShape::U64,
        ]
    );
}

#[test]
fn parses_conditional_wire_shapes_without_flattening_branch_structure() {
    use crate::network_schema::parse::{nested_member_wire_shapes, parse_network_wire_shape};

    let text = "boolean-choice<default-omitted<vec3,vec3>,u32>";
    let shape = parse_network_wire_shape(text).expect("conditional wire shape");

    assert_eq!(shape.wire_string(), text);
    assert_eq!(nested_member_wire_shapes(text, &[]), None);
}

#[test]
fn parses_counted_set_wire_shapes() {
    use crate::network_schema::parse::{nested_member_wire_shapes, parse_network_wire_shape};

    let text = "set<fixed-bytes-16>";
    let shape = parse_network_wire_shape(text).expect("counted set wire shape");

    assert_eq!(shape.wire_string(), text);
    assert_eq!(
        nested_member_wire_shapes(text, &[]),
        Some(vec![
            NetworkWireScalarShape::VlqU32,
            NetworkWireScalarShape::FixedBytes(16),
        ])
    );
}

#[test]
fn parses_counted_map_wire_shapes() {
    use crate::network_schema::parse::{nested_member_wire_shapes, parse_network_wire_shape};

    let text = "map<u32,composite<entity-ref,u32,bool>>";
    let shape = parse_network_wire_shape(text).expect("counted map wire shape");

    assert_eq!(shape.wire_string(), text);
    assert_eq!(
        nested_member_wire_shapes(text, &[]),
        Some(vec![
            NetworkWireScalarShape::VlqU32,
            NetworkWireScalarShape::U32,
            NetworkWireScalarShape::EntityRef,
            NetworkWireScalarShape::U32,
            NetworkWireScalarShape::Bool,
        ])
    );
}

fn fragment_access_message_signatures() -> Vec<NetworkMessageSignature> {
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
                message_field_signature(0, "TargetRef", "ActorRef"),
                message_field_signature(1, "Key", "FragmentKey"),
                message_field_signature(2, "Fragment", "BaselineableFragment"),
            ],
        },
    ]
}

fn fragment_access_fields() -> Vec<NetworkMessageFieldSignature> {
    vec![
        message_field_signature(0, "ProxyRef", "ActorRef"),
        message_field_signature(1, "Key", "FragmentKey"),
    ]
}

fn message_field_signature(
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
