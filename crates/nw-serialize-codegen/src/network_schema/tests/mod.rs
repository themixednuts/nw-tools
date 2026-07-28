use crate::ir::{
    SerializeCodegenItem, SerializeCodegenItemKind, SerializeCodegenRttiBase, SerializeCodegenUnit,
};
use crate::role::ReflectedTypeRole;
use serde_json::json;
use uuid::uuid;

use super::*;

mod ingest;
mod merge;
mod message_alignment;

#[test]
fn collapses_alternate_spelling_multi_helper_wire_products() {
    use crate::network_schema::parse::collapse_alternate_spelling_wire_product;

    assert_eq!(
        collapse_alternate_spelling_wire_product(
            "composite<composite<fixed-bytes-8,u64,u8>,composite<u64,u64,u8>>",
            None,
        ),
        Some("composite<u64,u64,u8>")
    );
    assert_eq!(
        collapse_alternate_spelling_wire_product(
            "composite<composite<fixed-bytes-4,fixed-bytes-16,fixed-bytes-16>,actor-ref>",
            None,
        ),
        Some("actor-ref")
    );
    // Distinct successive helpers must stay composed (FilterChat GameChatMessage).
    assert_eq!(
        collapse_alternate_spelling_wire_product(
            "composite<vec<composite<fixed-bytes-4,fixed-bytes-2>>,composite<fixed-bytes-4,string>>",
            None,
        ),
        None
    );
    // Trailing limb already present as the leading limb of the inner product.
    // Requires nested-shape agreement so unrelated composite<field,same-type>
    // products stay intact.
    let nested = nested_shape_from_layouts(&[
        "fixed-bytes-8",
        "composite<fixed-bytes-4,fixed-bytes-16,fixed-bytes-16>",
        "entity-ref",
        "composite<fixed-bytes-4,fixed-bytes-16,fixed-bytes-16>",
        "string",
    ]);
    assert_eq!(
        collapse_alternate_spelling_wire_product(
            "composite<composite<fixed-bytes-8,composite<fixed-bytes-4,fixed-bytes-16,fixed-bytes-16>,entity-ref,composite<fixed-bytes-4,fixed-bytes-16,fixed-bytes-16>,string>,fixed-bytes-8>",
            Some(&nested),
        ),
        Some(
            "composite<fixed-bytes-8,composite<fixed-bytes-4,fixed-bytes-16,fixed-bytes-16>,entity-ref,composite<fixed-bytes-4,fixed-bytes-16,fixed-bytes-16>,string>"
        )
    );
    assert_eq!(
        collapse_alternate_spelling_wire_product(
            "composite<composite<fixed-bytes-8,string>,fixed-bytes-8>",
            None,
        ),
        None
    );
    let nested = nested_shape_from_layouts(&["u64", "u64", "u32", "bool"]);
    assert_eq!(
        collapse_alternate_spelling_wire_product(
            "composite<composite<fixed-bytes-8,u64,u32,bool>,composite<fixed-bytes-8,fixed-bytes-8>>",
            Some(&nested),
        ),
        Some("composite<fixed-bytes-8,u64,u32,bool>")
    );
    let nested = nested_shape_from_layouts(&[
        "fixed-bytes-8",
        "u64",
        "u8",
        "fixed-bytes-12",
        "fixed-bytes-12",
        "fixed-bytes-4",
        "fixed-bytes-4",
        "fixed-bytes-4",
        "composite<fixed-bytes-4,fixed-bytes-4,fixed-bytes-4,fixed-bytes-4>",
        "bool",
        "string",
        "fixed-bytes-4",
        "composite<fixed-bytes-4,fixed-bytes-4,fixed-bytes-4,fixed-bytes-4>",
        "bool",
        "string",
    ]);
    assert_eq!(
        collapse_alternate_spelling_wire_product(
            "composite<composite<fixed-bytes-8,u64,u8,fixed-bytes-12,fixed-bytes-12,fixed-bytes-4,fixed-bytes-4,fixed-bytes-4,composite<fixed-bytes-4,fixed-bytes-4,fixed-bytes-4,fixed-bytes-4>,bool,string,fixed-bytes-4,composite<fixed-bytes-4,fixed-bytes-4,fixed-bytes-4,fixed-bytes-4>,bool,string>,composite<fixed-bytes-8,fixed-bytes-8>>",
            Some(&nested),
        ),
        Some(
            "composite<fixed-bytes-8,u64,u8,fixed-bytes-12,fixed-bytes-12,fixed-bytes-4,fixed-bytes-4,fixed-bytes-4,composite<fixed-bytes-4,fixed-bytes-4,fixed-bytes-4,fixed-bytes-4>,bool,string,fixed-bytes-4,composite<fixed-bytes-4,fixed-bytes-4,fixed-bytes-4,fixed-bytes-4>,bool,string>"
        )
    );
    let nested = nested_shape_from_layouts(&[
        "u64",
        "u64",
        "u8",
        "vec3",
        "vec3",
        "f32",
        "f32",
        "f32",
        "composite<f32,f32,f32,f32>",
        "bool",
        "string",
        "f32",
        "composite<f32,f32,f32,f32>",
        "bool",
        "string",
    ]);
    assert_eq!(
        collapse_alternate_spelling_wire_product(
            "composite<composite<fixed-bytes-8,u64,u8,fixed-bytes-12,fixed-bytes-12,fixed-bytes-4,fixed-bytes-4,fixed-bytes-4,composite<fixed-bytes-4,fixed-bytes-4,fixed-bytes-4,fixed-bytes-4>,bool,string,fixed-bytes-4,composite<fixed-bytes-4,fixed-bytes-4,fixed-bytes-4,fixed-bytes-4>,bool,string>,composite<fixed-bytes-8,fixed-bytes-8>>",
            Some(&nested),
        ),
        Some(
            "composite<fixed-bytes-8,u64,u8,fixed-bytes-12,fixed-bytes-12,fixed-bytes-4,fixed-bytes-4,fixed-bytes-4,composite<fixed-bytes-4,fixed-bytes-4,fixed-bytes-4,fixed-bytes-4>,bool,string,fixed-bytes-4,composite<fixed-bytes-4,fixed-bytes-4,fixed-bytes-4,fixed-bytes-4>,bool,string>"
        )
    );
    let mut nested = nested_shape_from_layouts(&["u32", "fixed-bytes-16", "fixed-bytes-16", "u64"]);
    nested.layout_proven = Some(true);
    nested.member_coverage_proven = Some(true);
    nested.wire_order_proven = Some(true);
    assert_eq!(
        collapse_alternate_spelling_wire_product(
            "composite<composite<fixed-bytes-4,fixed-bytes-16,fixed-bytes-16,fixed-bytes-8>,fixed-bytes-16>",
            Some(&nested),
        ),
        Some("composite<fixed-bytes-4,fixed-bytes-16,fixed-bytes-16,fixed-bytes-8>")
    );
    let mut nested = nested_shape_from_layouts(&["u64", "u64", "u32", "aabb3d"]);
    nested.layout_proven = Some(true);
    nested.member_coverage_proven = Some(true);
    nested.wire_order_proven = Some(true);
    assert_eq!(
        collapse_alternate_spelling_wire_product(
            "composite<composite<fixed-bytes-8,u64,fixed-bytes-4,fixed-bytes-24>,composite<fixed-bytes-8,fixed-bytes-8>>",
            Some(&nested),
        ),
        Some("composite<fixed-bytes-8,u64,fixed-bytes-4,fixed-bytes-24>")
    );
}

fn nested_shape_from_layouts(layouts: &[&str]) -> crate::network_schema::NetworkNestedTypeShape {
    crate::network_schema::NetworkNestedTypeShape {
        type_id: None,
        type_id_source: None,
        identity_proven: None,
        identity_source: None,
        type_name: None,
        type_name_full: None,
        type_name_source: None,
        function: None,
        function_name: None,
        factory: None,
        az_rtti_address: None,
        constructor: None,
        vtable: None,
        member_base: None,
        member_name_source: None,
        member_names_proven: None,
        layout_proven: None,
        member_coverage_proven: None,
        wire_order_proven: None,
        wire_order_source: None,
        datatype_path: None,
        validation: None,
        native_size: None,
        native_size_source: None,
        members: layouts
            .iter()
            .enumerate()
            .map(
                |(index, layout)| crate::network_schema::NetworkNestedTypeMember {
                    index: Some(index as u32),
                    offset: None,
                    native_offset: None,
                    name: None,
                    name_source: None,
                    name_proven: None,
                    name_evidence: None,
                    native_type: None,
                    type_id: None,
                    type_id_source: None,
                    type_identity_proven: false,
                    type_identity_source: None,
                    wire_shape: None,
                    wire_shape_source: None,
                    wire_layout: Some((*layout).to_owned()),
                    wire_layout_source: None,
                    byte_width: None,
                    wire_ordinal: None,
                    wire_order_source: None,
                    callsite: None,
                    target: None,
                    target_name: None,
                    type_conflict: false,
                },
            )
            .collect(),
    }
}

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
