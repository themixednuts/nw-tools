use uuid::uuid;

use crate::document::SerializeContextDocument;
use crate::ir::{
    SerializeCodegenField, SerializeCodegenItem, SerializeCodegenItemKind, SerializeCodegenPlanner,
    SerializeCodegenRttiBase, SerializeCodegenSelection, SerializeCodegenVariant,
};
use crate::model::SerializeContextModel;
use crate::role::ReflectedTypeRole;
use crate::types::{ResolvedType, ScalarType};

use super::*;

#[test]
fn scopes_derived_types_by_primary_base_chain() {
    let component = item(
        uuid!("aaaaaaaa-aaaa-aaaa-aaaa-aaaaaaaaaaaa"),
        "AZ::Component",
        Vec::new(),
    );
    let faceted = item(
        uuid!("bbbbbbbb-bbbb-bbbb-bbbb-bbbbbbbbbbbb"),
        "Javelin::FacetedComponent",
        vec![base_field(component.source_type_id, &component.source_name)],
    );
    let action_list = item(
        uuid!("cccccccc-cccc-cccc-cccc-cccccccccccc"),
        "Javelin::ActionListComponent",
        vec![base_field(faceted.source_type_id, &faceted.source_name)],
    );
    let items = BTreeMap::from([
        (component.source_type_id, &component),
        (faceted.source_type_id, &faceted),
        (action_list.source_type_id, &action_list),
    ]);

    assert_eq!(
        inheritance_scope_segments(&action_list, &items),
        vec!["javelin", "components", "faceted_components"]
    );
}

#[test]
fn scopes_rtti_only_component_ancestry_under_component_family() {
    let mut component = item(
        uuid!("aaaaaaaa-aaaa-aaaa-aaaa-aaaaaaaaaaaa"),
        "AZ::Component",
        Vec::new(),
    );
    component.role = ReflectedTypeRole::AzComponent;
    let mut runtime_component = item(
        uuid!("bbbbbbbb-bbbb-bbbb-bbbb-bbbbbbbbbbbb"),
        "RuntimeOnlyComponent",
        Vec::new(),
    );
    runtime_component.role = ReflectedTypeRole::AzComponent;
    runtime_component.rtti_base_chain = vec![SerializeCodegenRttiBase {
        type_id: component.source_type_id,
        source_name: component.source_name.clone(),
    }];
    let items = BTreeMap::from([
        (component.source_type_id, &component),
        (runtime_component.source_type_id, &runtime_component),
    ]);

    assert_eq!(
        inheritance_scope_segments(&runtime_component, &items),
        vec!["components"]
    );
    assert!(
        reflected_base_type_ids(&SerializeCodegenUnit {
            items: vec![component, runtime_component]
        })
        .contains(&uuid!("aaaaaaaa-aaaa-aaaa-aaaa-aaaaaaaaaaaa"))
    );
}

#[test]
fn scopes_rtti_only_entity_ancestry_under_concrete_entity_scope() {
    let mut entity = item(
        uuid!("aaaaaaaa-aaaa-aaaa-aaaa-aaaaaaaaaaaa"),
        "AZ::Entity",
        Vec::new(),
    );
    entity.role = ReflectedTypeRole::AzEntity;
    let mut module_entity = item(
        uuid!("bbbbbbbb-bbbb-bbbb-bbbb-bbbbbbbbbbbb"),
        "ModuleEntity",
        Vec::new(),
    );
    module_entity.role = ReflectedTypeRole::AzEntity;
    module_entity.rtti_base_chain = vec![SerializeCodegenRttiBase {
        type_id: entity.source_type_id,
        source_name: entity.source_name.clone(),
    }];
    let items = BTreeMap::from([
        (entity.source_type_id, &entity),
        (module_entity.source_type_id, &module_entity),
    ]);

    assert_eq!(
        inheritance_scope_segments(&module_entity, &items),
        vec!["az", "entity"]
    );
}

#[test]
fn scopes_base_type_with_its_derived_family() {
    let mut action_condition = item(
        uuid!("aaaaaaaa-aaaa-aaaa-aaaa-aaaaaaaaaaaa"),
        "ActionCondition",
        Vec::new(),
    );
    action_condition.is_abstract = Some(true);
    let derived = item(
        uuid!("bbbbbbbb-bbbb-bbbb-bbbb-bbbbbbbbbbbb"),
        "ActionConditionIfInput",
        vec![base_field(
            action_condition.source_type_id,
            &action_condition.source_name,
        )],
    );
    let items = BTreeMap::from([
        (action_condition.source_type_id, &action_condition),
        (derived.source_type_id, &derived),
    ]);

    assert_eq!(
        inheritance_family_scope_segments(&action_condition, &items),
        vec!["action_conditions"]
    );
    assert_eq!(
        inheritance_scope_segments(&derived, &items),
        vec!["action_conditions"]
    );
}

#[test]
fn orders_items_by_resolved_type_dependencies_before_dependents() {
    let leaf = item(
        uuid!("aaaaaaaa-aaaa-aaaa-aaaa-aaaaaaaaaaaa"),
        "Leaf",
        Vec::new(),
    );
    let dependency = item(
        uuid!("bbbbbbbb-bbbb-bbbb-bbbb-bbbbbbbbbbbb"),
        "Dependency",
        vec![pointer_field(
            "m_leaf",
            leaf.source_type_id,
            &leaf.source_name,
        )],
    );
    let consumer = item(
        uuid!("cccccccc-cccc-cccc-cccc-cccccccccccc"),
        "Consumer",
        vec![pointer_field(
            "m_dependency",
            dependency.source_type_id,
            &dependency.source_name,
        )],
    );

    let unit = SerializeCodegenUnit {
        items: vec![consumer, dependency, leaf],
    };
    let ordered = dependency_ordered_codegen_items(&unit)
        .into_iter()
        .map(|item| item.source_name.as_str())
        .collect::<Vec<_>>();

    assert_eq!(ordered, vec!["Leaf", "Dependency", "Consumer"]);
}

#[test]
fn orders_cycles_deterministically_without_recursive_walks() {
    let a = item(
        uuid!("aaaaaaaa-aaaa-aaaa-aaaa-aaaaaaaaaaaa"),
        "A",
        vec![pointer_field(
            "m_b",
            uuid!("bbbbbbbb-bbbb-bbbb-bbbb-bbbbbbbbbbbb"),
            "B",
        )],
    );
    let b = item(
        uuid!("bbbbbbbb-bbbb-bbbb-bbbb-bbbbbbbbbbbb"),
        "B",
        vec![pointer_field(
            "m_a",
            uuid!("aaaaaaaa-aaaa-aaaa-aaaa-aaaaaaaaaaaa"),
            "A",
        )],
    );

    let unit = SerializeCodegenUnit { items: vec![b, a] };
    let ordered = dependency_ordered_codegen_items(&unit)
        .into_iter()
        .map(|item| item.source_name.as_str())
        .collect::<Vec<_>>();

    assert_eq!(ordered, vec!["B", "A"]);
}

#[test]
fn scopes_slot_descendants_under_the_slot_owner_family() {
    let component_base = item(
        uuid!("aaaaaaaa-aaaa-aaaa-aaaa-aaaaaaaaaaaa"),
        "ComponentBase",
        Vec::new(),
    );
    let faceted_component = item(
        uuid!("bbbbbbbb-bbbb-bbbb-bbbb-bbbbbbbbbbbb"),
        "FacetedComponent",
        vec![
            base_field(component_base.source_type_id, &component_base.source_name),
            pointer_field(
                "client",
                uuid!("cccccccc-cccc-cccc-cccc-cccccccccccc"),
                "ClientFacet",
            ),
        ],
    );
    let client_facet = item(
        uuid!("cccccccc-cccc-cccc-cccc-cccccccccccc"),
        "ClientFacet",
        Vec::new(),
    );
    let mut inventory_component = item(
        uuid!("eeeeeeee-eeee-eeee-eeee-eeeeeeeeeeee"),
        "InventoryComponent",
        vec![base_field(
            faceted_component.source_type_id,
            &faceted_component.source_name,
        )],
    );
    inventory_component.role = ReflectedTypeRole::FacetedComponent;
    let mut concrete_client_facet = item(
        uuid!("dddddddd-dddd-dddd-dddd-dddddddddddd"),
        "InventoryClientFacet",
        vec![base_field(
            client_facet.source_type_id,
            &client_facet.source_name,
        )],
    );
    concrete_client_facet.role = ReflectedTypeRole::ClientFacet;
    let items = BTreeMap::from([
        (component_base.source_type_id, &component_base),
        (faceted_component.source_type_id, &faceted_component),
        (client_facet.source_type_id, &client_facet),
        (inventory_component.source_type_id, &inventory_component),
        (concrete_client_facet.source_type_id, &concrete_client_facet),
    ]);

    assert_eq!(
        inheritance_family_scope_segments(&client_facet, &items),
        vec!["components", "faceted_components"]
    );
    assert_eq!(
        inheritance_scope_segments(&concrete_client_facet, &items),
        vec!["components", "faceted_components", "inventory_component"]
    );
    assert_eq!(
        inheritance_family_scope_segments(&concrete_client_facet, &items),
        vec![
            "components",
            "faceted_components",
            "inventory_component",
            "client_facet"
        ]
    );
    assert!(has_concrete_slot_children(&inventory_component, &items));

    let report = LayoutAnalysisReport::from_codegen_unit(&SerializeCodegenUnit {
        items: vec![
            component_base,
            faceted_component,
            client_facet,
            inventory_component,
            concrete_client_facet,
        ],
    });
    let concrete = report
        .item_by_source_name("InventoryClientFacet")
        .expect("concrete slot descendant analysis");
    assert_eq!(
        concrete.slot_anchor,
        Some(LayoutSlotAnchor {
            owner_type_id: uuid!("bbbbbbbb-bbbb-bbbb-bbbb-bbbbbbbbbbbb"),
            owner_source_name: "FacetedComponent".to_owned(),
            owner_field_name: "client".to_owned(),
            slot_type_id: uuid!("cccccccc-cccc-cccc-cccc-cccccccccccc"),
            slot_source_name: "ClientFacet".to_owned(),
        })
    );
    assert_eq!(
        concrete.concrete_slot_binding,
        Some(LayoutConcreteSlotBinding {
            owner_type_id: uuid!("eeeeeeee-eeee-eeee-eeee-eeeeeeeeeeee"),
            owner_source_name: "InventoryComponent".to_owned(),
            slot_owner_type_id: uuid!("bbbbbbbb-bbbb-bbbb-bbbb-bbbbbbbbbbbb"),
            slot_owner_source_name: "FacetedComponent".to_owned(),
            owner_field_name: "client".to_owned(),
            slot_type_id: uuid!("cccccccc-cccc-cccc-cccc-cccccccccccc"),
            slot_source_name: "ClientFacet".to_owned(),
        })
    );
    assert!(
        report
            .to_text()
            .contains("slot_anchor FacetedComponent.client -> ClientFacet")
    );
    assert!(
        report
            .to_text()
            .contains("concrete_slot InventoryComponent.client -> ClientFacet")
    );
}

#[test]
fn scopes_concrete_data_free_marker_base_at_common_descendant_scope() {
    let component_base = item(
        uuid!("aaaaaaaa-aaaa-aaaa-aaaa-aaaaaaaaaaaa"),
        "AZ::Component",
        Vec::new(),
    );
    let mut facet = item(
        uuid!("bbbbbbbb-bbbb-bbbb-bbbb-bbbbbbbbbbbb"),
        "Facet",
        Vec::new(),
    );
    facet.is_abstract = Some(false);
    let mut client_facet = item(
        uuid!("cccccccc-cccc-cccc-cccc-cccccccccccc"),
        "ClientFacet",
        vec![base_field(facet.source_type_id, &facet.source_name)],
    );
    client_facet.is_abstract = Some(false);
    let mut server_facet = item(
        uuid!("dddddddd-dddd-dddd-dddd-dddddddddddd"),
        "ServerFacet",
        vec![base_field(facet.source_type_id, &facet.source_name)],
    );
    server_facet.is_abstract = Some(false);
    let faceted_component = item(
        uuid!("eeeeeeee-eeee-eeee-eeee-eeeeeeeeeeee"),
        "FacetedComponent",
        vec![
            base_field(component_base.source_type_id, &component_base.source_name),
            pointer_field(
                "m_clientFacetPtr",
                client_facet.source_type_id,
                &client_facet.source_name,
            ),
            pointer_field(
                "m_serverFacetPtr",
                server_facet.source_type_id,
                &server_facet.source_name,
            ),
        ],
    );
    let inventory_component = item(
        uuid!("11111111-1111-1111-1111-111111111111"),
        "InventoryComponent",
        vec![base_field(
            faceted_component.source_type_id,
            &faceted_component.source_name,
        )],
    );
    let inventory_client_facet = item(
        uuid!("22222222-2222-2222-2222-222222222222"),
        "InventoryComponentClientFacet",
        vec![base_field(
            client_facet.source_type_id,
            &client_facet.source_name,
        )],
    );
    let inventory_server_facet = item(
        uuid!("33333333-3333-3333-3333-333333333333"),
        "InventoryComponentServerFacet",
        vec![base_field(
            server_facet.source_type_id,
            &server_facet.source_name,
        )],
    );
    let items = BTreeMap::from([
        (component_base.source_type_id, &component_base),
        (facet.source_type_id, &facet),
        (client_facet.source_type_id, &client_facet),
        (server_facet.source_type_id, &server_facet),
        (faceted_component.source_type_id, &faceted_component),
        (inventory_component.source_type_id, &inventory_component),
        (
            inventory_client_facet.source_type_id,
            &inventory_client_facet,
        ),
        (
            inventory_server_facet.source_type_id,
            &inventory_server_facet,
        ),
    ]);

    assert_eq!(
        inheritance_family_scope_segments(&facet, &items),
        vec!["components", "faceted_components"]
    );
    assert_eq!(
        inheritance_scope_segments(&inventory_client_facet, &items),
        vec!["components", "faceted_components", "inventory_component"]
    );
    assert_eq!(
        inheritance_scope_segments(&inventory_server_facet, &items),
        vec!["components", "faceted_components", "inventory_component"]
    );
}

#[test]
fn type_path_uses_shared_semantic_scope_and_file_stem() {
    let support = item(
        uuid!("aaaaaaaa-aaaa-aaaa-aaaa-aaaaaaaaaaaa"),
        "Example::SupportValue",
        Vec::new(),
    );
    let component_base = item(
        uuid!("bbbbbbbb-bbbb-bbbb-bbbb-bbbbbbbbbbbb"),
        "AZ::Component",
        Vec::new(),
    );
    let facet = item(
        uuid!("cccccccc-cccc-cccc-cccc-cccccccccccc"),
        "ClientFacet",
        Vec::new(),
    );
    let faceted_component = item(
        uuid!("dddddddd-dddd-dddd-dddd-dddddddddddd"),
        "FacetedComponent",
        vec![
            base_field(component_base.source_type_id, &component_base.source_name),
            pointer_field("m_clientFacetPtr", facet.source_type_id, &facet.source_name),
        ],
    );
    let inventory_component = item(
        uuid!("eeeeeeee-eeee-eeee-eeee-eeeeeeeeeeee"),
        "InventoryComponent",
        vec![base_field(
            faceted_component.source_type_id,
            &faceted_component.source_name,
        )],
    );
    let inventory_client_facet = item(
        uuid!("11111111-1111-1111-1111-111111111111"),
        "InventoryComponentClientFacet",
        vec![base_field(facet.source_type_id, &facet.source_name)],
    );
    let items = BTreeMap::from([
        (support.source_type_id, &support),
        (component_base.source_type_id, &component_base),
        (facet.source_type_id, &facet),
        (faceted_component.source_type_id, &faceted_component),
        (inventory_component.source_type_id, &inventory_component),
        (
            inventory_client_facet.source_type_id,
            &inventory_client_facet,
        ),
    ]);
    let index = LayoutIndex::from_items_by_type_id(&items);

    assert_eq!(
        index.type_path(&support, &items),
        LayoutTypePath {
            scope_segments: vec!["example".to_owned()],
            file_stem: "support_value".to_owned(),
        }
    );
    assert_eq!(
        index.type_path(&inventory_component, &items),
        LayoutTypePath {
            scope_segments: vec!["components".to_owned(), "faceted_components".to_owned()],
            file_stem: "inventory_component".to_owned(),
        }
    );
    assert_eq!(
        index.type_path(&inventory_client_facet, &items),
        LayoutTypePath {
            scope_segments: vec![
                "components".to_owned(),
                "faceted_components".to_owned(),
                "inventory_component".to_owned(),
            ],
            file_stem: "client_facet".to_owned(),
        }
    );
}

#[test]
fn scopes_generic_slot_descendants_under_the_concrete_slot_owner_family() {
    let thing_base = item(
        uuid!("aaaaaaaa-aaaa-aaaa-aaaa-aaaaaaaaaaaa"),
        "ThingBase",
        Vec::new(),
    );
    let behavior_host = item(
        uuid!("bbbbbbbb-bbbb-bbbb-bbbb-bbbbbbbbbbbb"),
        "BehaviorHost",
        vec![
            base_field(thing_base.source_type_id, &thing_base.source_name),
            pointer_field(
                "m_runtime",
                uuid!("cccccccc-cccc-cccc-cccc-cccccccccccc"),
                "RuntimeImpl",
            ),
        ],
    );
    let runtime_impl = item(
        uuid!("cccccccc-cccc-cccc-cccc-cccccccccccc"),
        "RuntimeImpl",
        Vec::new(),
    );
    let door_host = item(
        uuid!("dddddddd-dddd-dddd-dddd-dddddddddddd"),
        "DoorHost",
        vec![base_field(
            behavior_host.source_type_id,
            &behavior_host.source_name,
        )],
    );
    let door_runtime = item(
        uuid!("eeeeeeee-eeee-eeee-eeee-eeeeeeeeeeee"),
        "DoorRuntimeImpl",
        vec![base_field(
            runtime_impl.source_type_id,
            &runtime_impl.source_name,
        )],
    );
    let unit = SerializeCodegenUnit {
        items: vec![
            thing_base.clone(),
            behavior_host.clone(),
            runtime_impl.clone(),
            door_host.clone(),
            door_runtime.clone(),
        ],
    };
    let items = unit
        .items
        .iter()
        .map(|item| (item.source_type_id, item))
        .collect::<BTreeMap<_, _>>();
    let index = LayoutIndex::from_codegen_unit(&unit);

    assert_eq!(
        index.inheritance_scope_segments(&door_runtime, &items),
        vec!["thing_base", "behavior_hosts", "door_host"]
    );
    assert_eq!(
        index.inheritance_family_scope_segments(&door_runtime, &items),
        vec!["thing_base", "behavior_hosts", "door_host", "runtime_impl"]
    );
    assert_eq!(
        index.concrete_slot_binding(&door_runtime),
        Some(&LayoutConcreteSlotBinding {
            owner_type_id: door_host.source_type_id,
            owner_source_name: "DoorHost".to_owned(),
            slot_owner_type_id: behavior_host.source_type_id,
            slot_owner_source_name: "BehaviorHost".to_owned(),
            owner_field_name: "m_runtime".to_owned(),
            slot_type_id: runtime_impl.source_type_id,
            slot_source_name: "RuntimeImpl".to_owned(),
        })
    );
    assert_eq!(
        index.concrete_slot_candidates(&door_runtime),
        &[LayoutConcreteSlotCandidate {
            owner_type_id: door_host.source_type_id,
            owner_source_name: "DoorHost".to_owned(),
            slot_owner_type_id: behavior_host.source_type_id,
            slot_owner_source_name: "BehaviorHost".to_owned(),
            owner_field_name: "m_runtime".to_owned(),
            slot_type_id: runtime_impl.source_type_id,
            slot_source_name: "RuntimeImpl".to_owned(),
            match_kind: LayoutConcreteSlotMatchKind::OwnerTrailingSemanticWord,
        }]
    );
}

#[test]
fn leaves_generic_slot_binding_unresolved_when_owner_candidates_are_ambiguous() {
    let behavior_host = item(
        uuid!("aaaaaaaa-aaaa-aaaa-aaaa-aaaaaaaaaaaa"),
        "BehaviorHost",
        vec![pointer_field(
            "m_runtime",
            uuid!("bbbbbbbb-bbbb-bbbb-bbbb-bbbbbbbbbbbb"),
            "RuntimeImpl",
        )],
    );
    let runtime_impl = item(
        uuid!("bbbbbbbb-bbbb-bbbb-bbbb-bbbbbbbbbbbb"),
        "RuntimeImpl",
        Vec::new(),
    );
    let a_door_host = item(
        uuid!("cccccccc-cccc-cccc-cccc-cccccccccccc"),
        "A::DoorHost",
        vec![base_field(
            behavior_host.source_type_id,
            &behavior_host.source_name,
        )],
    );
    let b_door_host = item(
        uuid!("dddddddd-dddd-dddd-dddd-dddddddddddd"),
        "B::DoorHost",
        vec![base_field(
            behavior_host.source_type_id,
            &behavior_host.source_name,
        )],
    );
    let door_runtime = item(
        uuid!("eeeeeeee-eeee-eeee-eeee-eeeeeeeeeeee"),
        "DoorRuntimeImpl",
        vec![base_field(
            runtime_impl.source_type_id,
            &runtime_impl.source_name,
        )],
    );
    let unit = SerializeCodegenUnit {
        items: vec![
            behavior_host,
            runtime_impl,
            a_door_host,
            b_door_host,
            door_runtime.clone(),
        ],
    };
    let index = LayoutIndex::from_codegen_unit(&unit);

    assert_eq!(index.concrete_slot_binding(&door_runtime), None);
    assert!(index.has_ambiguous_concrete_slot_binding(&door_runtime));
    assert_eq!(index.concrete_slot_candidates(&door_runtime).len(), 2);
    assert!(
        index
            .concrete_slot_candidates(&door_runtime)
            .iter()
            .all(|candidate| candidate.match_kind
                == LayoutConcreteSlotMatchKind::OwnerTrailingSemanticWord)
    );
}

#[test]
fn scopes_new_world_facets_through_faceted_component_slots() {
    let az_component = item(
        uuid!("edFCb2cf-f75d-43be-b26b-f35821b29247"),
        "AZ::Component",
        Vec::new(),
    );
    let facet = item(
        uuid!("9469c437-6529-489d-8cf8-63eeab723a79"),
        "Facet",
        Vec::new(),
    );
    let client_facet = item(
        uuid!("0643cdc7-b1c9-4721-92ce-7ac02e6175c9"),
        "ClientFacet",
        vec![base_field(facet.source_type_id, &facet.source_name)],
    );
    let server_facet = item(
        uuid!("0392e589-5b61-47cc-835b-c3c254e76493"),
        "ServerFacet",
        vec![base_field(facet.source_type_id, &facet.source_name)],
    );
    let faceted_component = item(
        uuid!("65cd8f3e-73aa-43e9-8d9a-b5ae43f624f9"),
        "FacetedComponent",
        vec![
            base_field(az_component.source_type_id, &az_component.source_name),
            pointer_field(
                "m_clientFacetPtr",
                client_facet.source_type_id,
                &client_facet.source_name,
            ),
            pointer_field(
                "m_serverFacetPtr",
                server_facet.source_type_id,
                &server_facet.source_name,
            ),
        ],
    );
    let action_list_component = item(
        uuid!("30ed0ace-51dd-48b9-ba41-2fa6775cd106"),
        "ActionListComponent",
        vec![base_field(
            faceted_component.source_type_id,
            &faceted_component.source_name,
        )],
    );
    let action_list_client_facet = item(
        uuid!("0f83e947-4111-4e90-a6ad-d5ec0da60307"),
        "ActionListComponentClientFacet",
        vec![base_field(
            client_facet.source_type_id,
            &client_facet.source_name,
        )],
    );
    let action_list_facet = item(
        uuid!("f37ab16d-4c98-4b21-899a-29548f0a788a"),
        "ActionListComponentServerFacet",
        vec![base_field(
            server_facet.source_type_id,
            &server_facet.source_name,
        )],
    );
    let items = BTreeMap::from([
        (az_component.source_type_id, &az_component),
        (facet.source_type_id, &facet),
        (client_facet.source_type_id, &client_facet),
        (server_facet.source_type_id, &server_facet),
        (faceted_component.source_type_id, &faceted_component),
        (action_list_component.source_type_id, &action_list_component),
        (
            action_list_client_facet.source_type_id,
            &action_list_client_facet,
        ),
        (action_list_facet.source_type_id, &action_list_facet),
    ]);

    assert_eq!(
        inheritance_scope_segments(&action_list_facet, &items),
        vec!["components", "faceted_components", "action_list_component"]
    );
    assert_eq!(
        inheritance_family_scope_segments(&facet, &items),
        vec!["components", "faceted_components"]
    );
    assert_eq!(
        inheritance_family_scope_segments(&client_facet, &items),
        vec!["components", "faceted_components"]
    );
    assert_eq!(
        inheritance_family_scope_segments(&server_facet, &items),
        vec!["components", "faceted_components"]
    );
}

#[test]
fn scopes_namespace_less_support_data_under_unique_field_owner_family() {
    let az_component = item(
        uuid!("aaaaaaaa-aaaa-aaaa-aaaa-aaaaaaaaaaaa"),
        "AZ::Component",
        Vec::new(),
    );
    let faceted_component = item(
        uuid!("bbbbbbbb-bbbb-bbbb-bbbb-bbbbbbbbbbbb"),
        "FacetedComponent",
        vec![
            base_field(az_component.source_type_id, &az_component.source_name),
            pointer_field(
                "m_clientFacetPtr",
                uuid!("eeeeeeee-eeee-eeee-eeee-eeeeeeeeeeee"),
                "ClientFacet",
            ),
        ],
    );
    let client_facet = item(
        uuid!("eeeeeeee-eeee-eeee-eeee-eeeeeeeeeeee"),
        "ClientFacet",
        Vec::new(),
    );
    let action_list_component = item(
        uuid!("cccccccc-cccc-cccc-cccc-cccccccccccc"),
        "ActionListComponent",
        vec![
            base_field(
                faceted_component.source_type_id,
                &faceted_component.source_name,
            ),
            value_field(
                "m_scopeData",
                uuid!("dddddddd-dddd-dddd-dddd-dddddddddddd"),
                "ALCScopeData",
            ),
        ],
    );
    let scope_data = item(
        uuid!("dddddddd-dddd-dddd-dddd-dddddddddddd"),
        "ALCScopeData",
        Vec::new(),
    );
    let action_list_client_facet = item(
        uuid!("ffffffff-ffff-ffff-ffff-ffffffffffff"),
        "ActionListComponentClientFacet",
        vec![base_field(
            client_facet.source_type_id,
            &client_facet.source_name,
        )],
    );
    let unit = SerializeCodegenUnit {
        items: vec![
            az_component,
            faceted_component,
            client_facet,
            action_list_component,
            scope_data,
            action_list_client_facet,
        ],
    };
    let items = unit
        .items
        .iter()
        .map(|item| (item.source_type_id, item))
        .collect::<BTreeMap<_, _>>();
    let index = LayoutIndex::from_codegen_unit(&unit);
    let scope_data = items
        .get(&uuid!("dddddddd-dddd-dddd-dddd-dddddddddddd"))
        .copied()
        .expect("scope data");

    assert_eq!(
        index.emitted_scope_segments(scope_data, &items),
        vec!["components", "faceted_components", "action_list_component"]
    );
}

#[test]
fn scopes_shared_support_data_under_common_concrete_owner() {
    let az_component = item(
        uuid!("aaaaaaaa-aaaa-aaaa-aaaa-aaaaaaaaaaaa"),
        "AZ::Component",
        Vec::new(),
    );
    let faceted_component = item(
        uuid!("bbbbbbbb-bbbb-bbbb-bbbb-bbbbbbbbbbbb"),
        "FacetedComponent",
        vec![
            base_field(az_component.source_type_id, &az_component.source_name),
            pointer_field(
                "m_clientFacetPtr",
                uuid!("cccccccc-cccc-cccc-cccc-cccccccccccc"),
                "ClientFacet",
            ),
            pointer_field(
                "m_serverFacetPtr",
                uuid!("dddddddd-dddd-dddd-dddd-dddddddddddd"),
                "ServerFacet",
            ),
        ],
    );
    let client_facet = item(
        uuid!("cccccccc-cccc-cccc-cccc-cccccccccccc"),
        "ClientFacet",
        Vec::new(),
    );
    let server_facet = item(
        uuid!("dddddddd-dddd-dddd-dddd-dddddddddddd"),
        "ServerFacet",
        Vec::new(),
    );
    let action_list_component = item(
        uuid!("eeeeeeee-eeee-eeee-eeee-eeeeeeeeeeee"),
        "ActionListComponent",
        vec![base_field(
            faceted_component.source_type_id,
            &faceted_component.source_name,
        )],
    );
    let shared_data = item(
        uuid!("ffffffff-ffff-ffff-ffff-ffffffffffff"),
        "SharedFacetData",
        Vec::new(),
    );
    let action_list_client_facet = item(
        uuid!("11111111-1111-1111-1111-111111111111"),
        "ActionListComponentClientFacet",
        vec![
            base_field(client_facet.source_type_id, &client_facet.source_name),
            value_field(
                "m_sharedData",
                shared_data.source_type_id,
                &shared_data.source_name,
            ),
        ],
    );
    let action_list_server_facet = item(
        uuid!("22222222-2222-2222-2222-222222222222"),
        "ActionListComponentServerFacet",
        vec![
            base_field(server_facet.source_type_id, &server_facet.source_name),
            value_field(
                "m_sharedData",
                shared_data.source_type_id,
                &shared_data.source_name,
            ),
        ],
    );
    let unit = SerializeCodegenUnit {
        items: vec![
            az_component,
            faceted_component,
            client_facet,
            server_facet,
            action_list_component,
            shared_data,
            action_list_client_facet,
            action_list_server_facet,
        ],
    };
    let items = unit
        .items
        .iter()
        .map(|item| (item.source_type_id, item))
        .collect::<BTreeMap<_, _>>();
    let index = LayoutIndex::from_codegen_unit(&unit);
    let shared_data = items
        .get(&uuid!("ffffffff-ffff-ffff-ffff-ffffffffffff"))
        .copied()
        .expect("shared data");

    assert_eq!(
        index.emitted_scope_segments(shared_data, &items),
        vec!["components", "faceted_components", "action_list_component"]
    );
}

#[test]
fn scopes_shared_support_data_under_lowest_common_owner_family() {
    let az_component = item(
        uuid!("aaaaaaaa-aaaa-aaaa-aaaa-aaaaaaaaaaaa"),
        "AZ::Component",
        Vec::new(),
    );
    let faceted_component = item(
        uuid!("bbbbbbbb-bbbb-bbbb-bbbb-bbbbbbbbbbbb"),
        "FacetedComponent",
        vec![
            base_field(az_component.source_type_id, &az_component.source_name),
            pointer_field(
                "m_clientFacetPtr",
                uuid!("cccccccc-cccc-cccc-cccc-cccccccccccc"),
                "ClientFacet",
            ),
        ],
    );
    let client_facet = item(
        uuid!("cccccccc-cccc-cccc-cccc-cccccccccccc"),
        "ClientFacet",
        Vec::new(),
    );
    let action_list_component = item(
        uuid!("dddddddd-dddd-dddd-dddd-dddddddddddd"),
        "ActionListComponent",
        vec![base_field(
            faceted_component.source_type_id,
            &faceted_component.source_name,
        )],
    );
    let territory_component = item(
        uuid!("eeeeeeee-eeee-eeee-eeee-eeeeeeeeeeee"),
        "TerritoryComponent",
        vec![base_field(
            faceted_component.source_type_id,
            &faceted_component.source_name,
        )],
    );
    let shared_id = item(
        uuid!("ffffffff-ffff-ffff-ffff-ffffffffffff"),
        "SharedRuntimeId",
        Vec::new(),
    );
    let action_list_client_facet = item(
        uuid!("11111111-1111-1111-1111-111111111111"),
        "ActionListComponentClientFacet",
        vec![
            base_field(client_facet.source_type_id, &client_facet.source_name),
            value_field(
                "m_sharedId",
                shared_id.source_type_id,
                &shared_id.source_name,
            ),
        ],
    );
    let territory_client_facet = item(
        uuid!("22222222-2222-2222-2222-222222222222"),
        "TerritoryComponentClientFacet",
        vec![
            base_field(client_facet.source_type_id, &client_facet.source_name),
            value_field(
                "m_sharedId",
                shared_id.source_type_id,
                &shared_id.source_name,
            ),
        ],
    );
    let unit = SerializeCodegenUnit {
        items: vec![
            az_component,
            faceted_component,
            client_facet,
            action_list_component,
            territory_component,
            shared_id,
            action_list_client_facet,
            territory_client_facet,
        ],
    };
    let items = unit
        .items
        .iter()
        .map(|item| (item.source_type_id, item))
        .collect::<BTreeMap<_, _>>();
    let index = LayoutIndex::from_codegen_unit(&unit);
    let shared_id = items
        .get(&uuid!("ffffffff-ffff-ffff-ffff-ffffffffffff"))
        .copied()
        .expect("shared runtime id");

    assert_eq!(
        index.emitted_scope_segments(shared_id, &items),
        vec!["components", "faceted_components"]
    );
    assert_eq!(
        index.emitted_scope(shared_id, &items).reason,
        LayoutScopeReason::FieldOwner
    );
}

#[test]
fn scopes_namespace_less_az_scalar_wrappers_by_semantic_scalar_family() {
    let entity_ref = item(
        uuid!("aaaaaaaa-aaaa-aaaa-aaaa-aaaaaaaaaaaa"),
        "LocalEntityRef",
        vec![scalar_field("EntityId", ScalarType::EntityId)],
    );
    let crc_wrapper = item(
        uuid!("bbbbbbbb-bbbb-bbbb-bbbb-bbbbbbbbbbbb"),
        "EditCrc",
        vec![scalar_field("m_value", ScalarType::Crc32)],
    );
    let plain_wrapper = item(
        uuid!("cccccccc-cccc-cccc-cccc-cccccccccccc"),
        "PlainId",
        vec![scalar_field("m_value", ScalarType::U64)],
    );
    let owned_crc_wrapper = item(
        uuid!("dddddddd-dddd-dddd-dddd-dddddddddddd"),
        "OwnedEditCrc",
        vec![scalar_field("m_value", ScalarType::Crc32)],
    );
    let owner = item(
        uuid!("eeeeeeee-eeee-eeee-eeee-eeeeeeeeeeee"),
        "Example::Owner",
        vec![value_field(
            "m_crc",
            owned_crc_wrapper.source_type_id,
            &owned_crc_wrapper.source_name,
        )],
    );
    let multi_field = item(
        uuid!("99999999-9999-9999-9999-999999999999"),
        "EventData",
        vec![
            scalar_field("EntityId", ScalarType::EntityId),
            scalar_field("m_type", ScalarType::I32),
        ],
    );
    let items = BTreeMap::from([
        (entity_ref.source_type_id, &entity_ref),
        (crc_wrapper.source_type_id, &crc_wrapper),
        (plain_wrapper.source_type_id, &plain_wrapper),
        (owned_crc_wrapper.source_type_id, &owned_crc_wrapper),
        (owner.source_type_id, &owner),
        (multi_field.source_type_id, &multi_field),
    ]);
    let index = LayoutIndex::from_items_by_type_id(&items);

    let entity_ref_scope = index.emitted_scope(&entity_ref, &items);
    assert_eq!(entity_ref_scope.segments, vec!["az", "entity"]);
    assert_eq!(entity_ref_scope.reason, LayoutScopeReason::SemanticWrapper);
    assert_eq!(
        index.emitted_scope_segments(&crc_wrapper, &items),
        vec!["az", "crc"]
    );
    let owned_crc_scope = index.emitted_scope(&owned_crc_wrapper, &items);
    assert_eq!(owned_crc_scope.segments, vec!["az", "crc"]);
    assert_eq!(owned_crc_scope.reason, LayoutScopeReason::SemanticWrapper);
    assert_eq!(
        index.emitted_scope(&plain_wrapper, &items).reason,
        LayoutScopeReason::Root
    );
    assert_eq!(
        index.emitted_scope(&multi_field, &items).reason,
        LayoutScopeReason::Root
    );
}

#[test]
fn keeps_interface_base_scope_singular() {
    let base = item(
        uuid!("aaaaaaaa-aaaa-aaaa-aaaa-aaaaaaaaaaaa"),
        "IStimulusPayloadBase",
        Vec::new(),
    );
    let payload = item(
        uuid!("bbbbbbbb-bbbb-bbbb-bbbb-bbbbbbbbbbbb"),
        "Javelin::StimulusPayload",
        vec![base_field(base.source_type_id, &base.source_name)],
    );
    let items = BTreeMap::from([
        (base.source_type_id, &base),
        (payload.source_type_id, &payload),
    ]);

    assert_eq!(
        inheritance_scope_segments(&payload, &items),
        vec!["javelin", "i_stimulus_payload_base"]
    );
}

#[test]
fn keeps_namespace_as_the_root_scope_before_inheritance() {
    let mut component = item(
        uuid!("aaaaaaaa-aaaa-aaaa-aaaa-aaaaaaaaaaaa"),
        "AZ::Component",
        Vec::new(),
    );
    component.is_abstract = Some(true);
    let mut az_memory = item(
        uuid!("dddddddd-dddd-dddd-dddd-dddddddddddd"),
        "AZ::MemoryComponent",
        vec![base_field(component.source_type_id, &component.source_name)],
    );
    az_memory.role = ReflectedTypeRole::AzComponent;
    let mut system = item(
        uuid!("bbbbbbbb-bbbb-bbbb-bbbb-bbbbbbbbbbbb"),
        "AzFramework::InputSystemComponent",
        vec![base_field(component.source_type_id, &component.source_name)],
    );
    system.role = ReflectedTypeRole::AzComponent;
    let mut asset_catalog = item(
        uuid!("eeeeeeee-eeee-eeee-eeee-eeeeeeeeeeee"),
        "AssetCatalogComponent",
        vec![base_field(component.source_type_id, &component.source_name)],
    );
    asset_catalog.role = ReflectedTypeRole::AzComponent;
    let support = item(
        uuid!("cccccccc-cccc-cccc-cccc-cccccccccccc"),
        "AzFramework::InputDeviceId",
        Vec::new(),
    );
    let items = BTreeMap::from([
        (component.source_type_id, &component),
        (az_memory.source_type_id, &az_memory),
        (system.source_type_id, &system),
        (asset_catalog.source_type_id, &asset_catalog),
        (support.source_type_id, &support),
    ]);
    let index = LayoutIndex::from_items_by_type_id(&items);

    assert_eq!(
        index.emitted_scope_segments(&component, &items),
        vec!["az", "components"]
    );
    assert_eq!(
        inheritance_family_scope_segments(&component, &items),
        vec!["az", "components"]
    );

    assert_eq!(
        inheritance_scope_segments(&az_memory, &items),
        vec!["az", "components"]
    );
    assert_eq!(
        index.emitted_scope_segments(&az_memory, &items),
        vec!["az", "components"]
    );
    assert_eq!(
        inheritance_scope_segments(&system, &items),
        vec!["az_framework", "components"]
    );
    assert_eq!(
        index.emitted_scope_segments(&system, &items),
        vec!["az_framework", "components"]
    );
    assert_eq!(
        inheritance_scope_segments(&asset_catalog, &items),
        vec!["components"]
    );
    assert_eq!(
        index.emitted_scope_segments(&asset_catalog, &items),
        vec!["components"]
    );
    assert_eq!(
        inheritance_scope_segments(&support, &items),
        vec!["az_framework"]
    );
}

#[test]
fn component_role_scope_skips_support_bases_between_component_descendants() {
    let component = item(
        uuid!("aaaaaaaa-aaaa-aaaa-aaaa-aaaaaaaaaaaa"),
        "Component",
        Vec::new(),
    );
    let mut net_bindable = item(
        uuid!("bbbbbbbb-bbbb-bbbb-bbbb-bbbbbbbbbbbb"),
        "NetBindable",
        vec![scalar_field("m_isNetSyncEnabled", ScalarType::Bool)],
    );
    net_bindable.is_abstract = Some(true);
    let mut transform_component = item(
        uuid!("cccccccc-cccc-cccc-cccc-cccccccccccc"),
        "TransformComponent",
        vec![base_field(
            net_bindable.source_type_id,
            &net_bindable.source_name,
        )],
    );
    transform_component.role = ReflectedTypeRole::AzComponent;
    transform_component.rtti_base_chain = vec![
        SerializeCodegenRttiBase {
            type_id: net_bindable.source_type_id,
            source_name: net_bindable.source_name.clone(),
        },
        SerializeCodegenRttiBase {
            type_id: component.source_type_id,
            source_name: component.source_name.clone(),
        },
    ];
    let mut script_component = item(
        uuid!("dddddddd-dddd-dddd-dddd-dddddddddddd"),
        "AzFramework::ScriptComponent",
        vec![base_field(
            net_bindable.source_type_id,
            &net_bindable.source_name,
        )],
    );
    script_component.role = ReflectedTypeRole::AzComponent;
    script_component.rtti_base_chain = transform_component.rtti_base_chain.clone();
    let items = BTreeMap::from([
        (component.source_type_id, &component),
        (net_bindable.source_type_id, &net_bindable),
        (transform_component.source_type_id, &transform_component),
        (script_component.source_type_id, &script_component),
    ]);
    let index = LayoutIndex::from_items_by_type_id(&items);

    assert_eq!(
        index.emitted_scope_segments(&transform_component, &items),
        vec!["components"]
    );
    assert_eq!(
        index.emitted_scope_segments(&script_component, &items),
        vec!["az_framework", "components"]
    );
    assert_eq!(
        index.emitted_scope_segments(&net_bindable, &items),
        vec!["components"]
    );
}

#[test]
fn rootless_support_climbs_through_namespaced_support_owners_to_runtime_anchor() {
    let component = item(
        uuid!("aaaaaaaa-aaaa-aaaa-aaaa-aaaaaaaaaaaa"),
        "Component",
        Vec::new(),
    );
    let faceted_component = item(
        uuid!("bbbbbbbb-bbbb-bbbb-bbbb-bbbbbbbbbbbb"),
        "FacetedComponent",
        vec![
            base_field(component.source_type_id, &component.source_name),
            pointer_field(
                "m_clientFacetPtr",
                uuid!("cccccccc-cccc-cccc-cccc-cccccccccccc"),
                "ClientFacet",
            ),
        ],
    );
    let client_facet = item(
        uuid!("cccccccc-cccc-cccc-cccc-cccccccccccc"),
        "ClientFacet",
        Vec::new(),
    );
    let cutscene_component = item(
        uuid!("dddddddd-dddd-dddd-dddd-dddddddddddd"),
        "CutsceneEventListenerComponent",
        vec![base_field(
            faceted_component.source_type_id,
            &faceted_component.source_name,
        )],
    );
    let event_data = item(
        uuid!("eeeeeeee-eeee-eeee-eeee-eeeeeeeeeeee"),
        "EventData",
        Vec::new(),
    );
    let cutscene_event = item(
        uuid!("ffffffff-ffff-ffff-ffff-ffffffffffff"),
        "Javelin::CutsceneEntityEvent",
        vec![value_field(
            "m_cutsceneEvent",
            event_data.source_type_id,
            &event_data.source_name,
        )],
    );
    let cutscene_client_facet = item(
        uuid!("11111111-1111-1111-1111-111111111111"),
        "CutsceneEventListenerComponentClientFacet",
        vec![
            base_field(client_facet.source_type_id, &client_facet.source_name),
            value_field(
                "m_onStartEntityEvent",
                cutscene_event.source_type_id,
                &cutscene_event.source_name,
            ),
        ],
    );
    let unit = SerializeCodegenUnit {
        items: vec![
            component,
            faceted_component,
            client_facet,
            cutscene_component,
            event_data,
            cutscene_event,
            cutscene_client_facet,
        ],
    };
    let items = unit
        .items
        .iter()
        .map(|item| (item.source_type_id, item))
        .collect::<BTreeMap<_, _>>();
    let index = LayoutIndex::from_codegen_unit(&unit);
    let event_data = items
        .get(&uuid!("eeeeeeee-eeee-eeee-eeee-eeeeeeeeeeee"))
        .copied()
        .expect("EventData");
    let cutscene_event = items
        .get(&uuid!("ffffffff-ffff-ffff-ffff-ffffffffffff"))
        .copied()
        .expect("CutsceneEntityEvent");

    assert_eq!(
        index.emitted_scope(cutscene_event, &items),
        LayoutScopeDecision {
            segments: vec!["javelin".to_owned()],
            reason: LayoutScopeReason::SourceNamespace,
        }
    );
    assert_eq!(
        index.emitted_scope(event_data, &items),
        LayoutScopeDecision {
            segments: vec![
                "components".to_owned(),
                "faceted_components".to_owned(),
                "cutscene_event_listener_component".to_owned(),
            ],
            reason: LayoutScopeReason::FieldOwner,
        }
    );
}

#[test]
fn scopes_rootless_entity_descendants_under_concrete_entity_scope() {
    let mut entity = item(
        uuid!("aaaaaaaa-aaaa-aaaa-aaaa-aaaaaaaaaaaa"),
        "AZ::Entity",
        Vec::new(),
    );
    entity.role = ReflectedTypeRole::AzEntity;
    let mut module_entity = item(
        uuid!("bbbbbbbb-bbbb-bbbb-bbbb-bbbbbbbbbbbb"),
        "ModuleEntity",
        vec![base_field(entity.source_type_id, &entity.source_name)],
    );
    module_entity.role = ReflectedTypeRole::AzEntity;
    let items = BTreeMap::from([
        (entity.source_type_id, &entity),
        (module_entity.source_type_id, &module_entity),
    ]);

    assert_eq!(
        inheritance_scope_segments(&module_entity, &items),
        vec!["az", "entity"]
    );
    assert_eq!(
        inheritance_family_scope_segments(&entity, &items),
        vec!["az", "entity"]
    );
}

#[test]
fn scopes_field_owned_base_families_under_unique_owner() {
    let component = item(
        uuid!("aaaaaaaa-aaaa-aaaa-aaaa-aaaaaaaaaaaa"),
        "Component",
        Vec::new(),
    );
    let faceted_component = item(
        uuid!("bbbbbbbb-bbbb-bbbb-bbbb-bbbbbbbbbbbb"),
        "FacetedComponent",
        vec![
            base_field(component.source_type_id, &component.source_name),
            pointer_field(
                "m_clientFacetPtr",
                uuid!("cccccccc-cccc-cccc-cccc-cccccccccccc"),
                "ClientFacet",
            ),
        ],
    );
    let client_facet = item(
        uuid!("cccccccc-cccc-cccc-cccc-cccccccccccc"),
        "ClientFacet",
        Vec::new(),
    );
    let camera_lock_component = item(
        uuid!("dddddddd-dddd-dddd-dddd-dddddddddddd"),
        "CameraLockComponent",
        vec![base_field(
            faceted_component.source_type_id,
            &faceted_component.source_name,
        )],
    );
    let base = item(
        uuid!("eeeeeeee-eeee-eeee-eeee-eeeeeeeeeeee"),
        "Base",
        Vec::new(),
    );
    let mut wedge = item(
        uuid!("ffffffff-ffff-ffff-ffff-ffffffffffff"),
        "Wedge",
        vec![base_field(base.source_type_id, &base.source_name)],
    );
    wedge.is_abstract = Some(true);
    let cylinder = item(
        uuid!("11111111-1111-1111-1111-111111111111"),
        "Cylinder",
        vec![base_field(wedge.source_type_id, &wedge.source_name)],
    );
    let obb = item(
        uuid!("22222222-2222-2222-2222-222222222222"),
        "OBB",
        vec![base_field(base.source_type_id, &base.source_name)],
    );
    let camera_lock_client_facet = item(
        uuid!("33333333-3333-3333-3333-333333333333"),
        "CameraLockComponentClientFacet",
        vec![
            base_field(client_facet.source_type_id, &client_facet.source_name),
            value_field("m_midRangeScan", wedge.source_type_id, &wedge.source_name),
            value_field(
                "m_closeRangeScan",
                cylinder.source_type_id,
                &cylinder.source_name,
            ),
            value_field("m_obb", obb.source_type_id, &obb.source_name),
        ],
    );
    let unit = SerializeCodegenUnit {
        items: vec![
            component,
            faceted_component,
            client_facet,
            camera_lock_component,
            base,
            wedge,
            cylinder,
            obb,
            camera_lock_client_facet,
        ],
    };
    let items = unit
        .items
        .iter()
        .map(|item| (item.source_type_id, item))
        .collect::<BTreeMap<_, _>>();
    let index = LayoutIndex::from_codegen_unit(&unit);

    let base = items
        .get(&uuid!("eeeeeeee-eeee-eeee-eeee-eeeeeeeeeeee"))
        .copied()
        .expect("base");
    let wedge = items
        .get(&uuid!("ffffffff-ffff-ffff-ffff-ffffffffffff"))
        .copied()
        .expect("wedge");
    let cylinder = items
        .get(&uuid!("11111111-1111-1111-1111-111111111111"))
        .copied()
        .expect("cylinder");
    let obb = items
        .get(&uuid!("22222222-2222-2222-2222-222222222222"))
        .copied()
        .expect("obb");

    assert_eq!(
        index.inheritance_family_scope_segments(base, &items),
        vec![
            "components",
            "faceted_components",
            "camera_lock_component",
            "base"
        ]
    );
    assert_eq!(
        index.inheritance_family_scope_segments(wedge, &items),
        vec![
            "components",
            "faceted_components",
            "camera_lock_component",
            "base",
            "wedges"
        ]
    );
    assert_eq!(
        index.inheritance_scope_segments(cylinder, &items),
        vec![
            "components",
            "faceted_components",
            "camera_lock_component",
            "base",
            "wedges"
        ]
    );
    assert_eq!(
        index.inheritance_scope_segments(obb, &items),
        vec![
            "components",
            "faceted_components",
            "camera_lock_component",
            "base"
        ]
    );
}

#[test]
fn field_owned_support_base_uses_concrete_owner_before_descendant_family() {
    let component = item(
        uuid!("aaaaaaaa-aaaa-aaaa-aaaa-aaaaaaaaaaaa"),
        "AZ::Component",
        Vec::new(),
    );
    let mesh_component = item(
        uuid!("bbbbbbbb-bbbb-bbbb-bbbb-bbbbbbbbbbbb"),
        "MeshComponent",
        vec![
            base_field(component.source_type_id, &component.source_name),
            value_field(
                "m_renderNode",
                uuid!("cccccccc-cccc-cccc-cccc-cccccccccccc"),
                "MeshComponentRenderNode",
            ),
        ],
    );
    let render_node = item(
        uuid!("cccccccc-cccc-cccc-cccc-cccccccccccc"),
        "MeshComponentRenderNode",
        vec![value_field(
            "m_options",
            uuid!("dddddddd-dddd-dddd-dddd-dddddddddddd"),
            "MeshRenderOptions",
        )],
    );
    let options = item(
        uuid!("dddddddd-dddd-dddd-dddd-dddddddddddd"),
        "MeshRenderOptions",
        Vec::new(),
    );
    let instanced_mesh_component = item(
        uuid!("eeeeeeee-eeee-eeee-eeee-eeeeeeeeeeee"),
        "InstancedMeshComponent",
        vec![
            base_field(component.source_type_id, &component.source_name),
            value_field(
                "m_renderNode",
                uuid!("ffffffff-ffff-ffff-ffff-ffffffffffff"),
                "InstancedMeshComponentRenderNode",
            ),
        ],
    );
    let instanced_render_node = item(
        uuid!("ffffffff-ffff-ffff-ffff-ffffffffffff"),
        "InstancedMeshComponentRenderNode",
        vec![base_field(
            render_node.source_type_id,
            &render_node.source_name,
        )],
    );
    let unit = SerializeCodegenUnit {
        items: vec![
            component,
            mesh_component,
            render_node,
            options,
            instanced_mesh_component,
            instanced_render_node,
        ],
    };
    let items = unit
        .items
        .iter()
        .map(|item| (item.source_type_id, item))
        .collect::<BTreeMap<_, _>>();
    let index = LayoutIndex::from_codegen_unit(&unit);
    let render_node = items
        .get(&uuid!("cccccccc-cccc-cccc-cccc-cccccccccccc"))
        .copied()
        .expect("render node");
    let options = items
        .get(&uuid!("dddddddd-dddd-dddd-dddd-dddddddddddd"))
        .copied()
        .expect("render options");
    let instanced_render_node = items
        .get(&uuid!("ffffffff-ffff-ffff-ffff-ffffffffffff"))
        .copied()
        .expect("instanced render node");

    assert_eq!(
        index.inheritance_family_scope_segments(render_node, &items),
        vec![
            "components",
            "mesh_component",
            "mesh_component_render_nodes"
        ]
    );
    assert_eq!(
        index.inheritance_scope_segments(options, &items),
        vec![
            "components",
            "mesh_component",
            "mesh_component_render_nodes"
        ]
    );
    assert_eq!(
        index.inheritance_scope_segments(instanced_render_node, &items),
        vec![
            "components",
            "instanced_mesh_component",
            "mesh_component_render_nodes"
        ]
    );
}

#[test]
fn scopes_multi_owner_support_under_lowest_common_component_family() {
    let component = item(
        uuid!("aaaaaaaa-aaaa-aaaa-aaaa-aaaaaaaaaaaa"),
        "AZ::Component",
        Vec::new(),
    );
    let navigation_profile = item(
        uuid!("bbbbbbbb-bbbb-bbbb-bbbb-bbbbbbbbbbbb"),
        "NavigationProfile",
        Vec::new(),
    );
    let pathing_component = item(
        uuid!("cccccccc-cccc-cccc-cccc-cccccccccccc"),
        "PathingComponent",
        vec![
            base_field(component.source_type_id, &component.source_name),
            value_field(
                "m_navigationProfile",
                navigation_profile.source_type_id,
                &navigation_profile.source_name,
            ),
        ],
    );
    let client_pathing_component = item(
        uuid!("dddddddd-dddd-dddd-dddd-dddddddddddd"),
        "ClientPathingComponent",
        vec![
            base_field(component.source_type_id, &component.source_name),
            value_field(
                "m_navigationProfile",
                navigation_profile.source_type_id,
                &navigation_profile.source_name,
            ),
        ],
    );
    let unit = SerializeCodegenUnit {
        items: vec![
            component,
            navigation_profile,
            pathing_component,
            client_pathing_component,
        ],
    };
    let items = unit
        .items
        .iter()
        .map(|item| (item.source_type_id, item))
        .collect::<BTreeMap<_, _>>();
    let index = LayoutIndex::from_codegen_unit(&unit);
    let navigation_profile = items
        .get(&uuid!("bbbbbbbb-bbbb-bbbb-bbbb-bbbbbbbbbbbb"))
        .copied()
        .expect("navigation profile");

    assert_eq!(
        index.inheritance_scope_segments(navigation_profile, &items),
        vec!["components"]
    );
    assert_eq!(
        index.emitted_scope(navigation_profile, &items).reason,
        LayoutScopeReason::FieldOwner
    );
}

#[test]
fn concrete_shared_base_keeps_own_family_for_owned_fields() {
    let component = item(
        uuid!("aaaaaaaa-aaaa-aaaa-aaaa-aaaaaaaaaaaa"),
        "AZ::Component",
        Vec::new(),
    );
    let width_modifier = item(
        uuid!("bbbbbbbb-bbbb-bbbb-bbbb-bbbbbbbbbbbb"),
        "SplineGeometryWidthModifier",
        Vec::new(),
    );
    let spline_geometry = item(
        uuid!("cccccccc-cccc-cccc-cccc-cccccccccccc"),
        "SplineGeometry",
        vec![value_field(
            "Width",
            width_modifier.source_type_id,
            &width_modifier.source_name,
        )],
    );
    let road = item(
        uuid!("dddddddd-dddd-dddd-dddd-dddddddddddd"),
        "Road",
        vec![base_field(
            spline_geometry.source_type_id,
            &spline_geometry.source_name,
        )],
    );
    let river = item(
        uuid!("eeeeeeee-eeee-eeee-eeee-eeeeeeeeeeee"),
        "River",
        vec![base_field(
            spline_geometry.source_type_id,
            &spline_geometry.source_name,
        )],
    );
    let mut road_component = item(
        uuid!("ffffffff-ffff-ffff-ffff-ffffffffffff"),
        "RoadComponent",
        vec![
            base_field(component.source_type_id, &component.source_name),
            value_field("Road", road.source_type_id, &road.source_name),
        ],
    );
    road_component.role = ReflectedTypeRole::AzComponent;
    let mut river_component = item(
        uuid!("11111111-1111-1111-1111-111111111111"),
        "RiverComponent",
        vec![
            base_field(component.source_type_id, &component.source_name),
            value_field("River", river.source_type_id, &river.source_name),
        ],
    );
    river_component.role = ReflectedTypeRole::AzComponent;
    let unit = SerializeCodegenUnit {
        items: vec![
            component,
            width_modifier,
            spline_geometry,
            road,
            river,
            road_component,
            river_component,
        ],
    };
    let items = unit
        .items
        .iter()
        .map(|item| (item.source_type_id, item))
        .collect::<BTreeMap<_, _>>();
    let index = LayoutIndex::from_codegen_unit(&unit);
    let spline_geometry = items
        .get(&uuid!("cccccccc-cccc-cccc-cccc-cccccccccccc"))
        .copied()
        .expect("spline geometry");
    let width_modifier = items
        .get(&uuid!("bbbbbbbb-bbbb-bbbb-bbbb-bbbbbbbbbbbb"))
        .copied()
        .expect("width modifier");

    assert_eq!(
        index.emitted_scope_segments(spline_geometry, &items),
        vec!["components", "spline_geometry"]
    );
    assert_eq!(
        index.emitted_scope_segments(width_modifier, &items),
        vec!["components", "spline_geometry"]
    );
}

#[test]
fn scopes_local_component_refs_by_get_type_name_target_namespace() {
    let base = item(
        uuid!("aaaaaaaa-aaaa-aaaa-aaaa-aaaaaaaaaaaa"),
        "LocalComponentRefBase",
        Vec::new(),
    );
    let local_ref = item(
        uuid!("bbbbbbbb-bbbb-bbbb-bbbb-bbbbbbbbbbbb"),
        "LocalComponentRef<InterfaceType><const char *__cdecl MB::GetTypeName<class Javelin::ActionConditionCacheComponent>(void)>",
        vec![base_field(base.source_type_id, &base.source_name)],
    );
    let items = BTreeMap::from([
        (base.source_type_id, &base),
        (local_ref.source_type_id, &local_ref),
    ]);

    assert_eq!(
        inheritance_scope_segments(&local_ref, &items),
        vec!["javelin", "local_component_ref_base"]
    );
}

#[test]
fn scopes_local_component_refs_under_resolved_target_layout() {
    let component_base = item(
        uuid!("aaaaaaaa-aaaa-aaaa-aaaa-aaaaaaaaaaaa"),
        "AZ::Component",
        Vec::new(),
    );
    let base = item(
        uuid!("bbbbbbbb-bbbb-bbbb-bbbb-bbbbbbbbbbbb"),
        "LocalComponentRefBase",
        Vec::new(),
    );
    let mut faceted_component = item(
        uuid!("cccccccc-cccc-cccc-cccc-cccccccccccc"),
        "FacetedComponent",
        vec![base_field(
            component_base.source_type_id,
            &component_base.source_name,
        )],
    );
    faceted_component.role = ReflectedTypeRole::FacetedComponent;
    let mut target = item(
        uuid!("dddddddd-dddd-dddd-dddd-dddddddddddd"),
        "ActionConditionCacheComponent",
        vec![base_field(
            faceted_component.source_type_id,
            &faceted_component.source_name,
        )],
    );
    target.role = ReflectedTypeRole::FacetedComponent;
    let local_ref = item(
        uuid!("eeeeeeee-eeee-eeee-eeee-eeeeeeeeeeee"),
        "LocalComponentRef<InterfaceType><const char *__cdecl MB::GetTypeName<class Javelin::ActionConditionCacheComponent>(void)>",
        vec![base_field(base.source_type_id, &base.source_name)],
    );
    let items = BTreeMap::from([
        (component_base.source_type_id, &component_base),
        (base.source_type_id, &base),
        (faceted_component.source_type_id, &faceted_component),
        (target.source_type_id, &target),
        (local_ref.source_type_id, &local_ref),
    ]);
    let index = LayoutIndex::from_items_by_type_id(&items);

    assert_eq!(
        index.inheritance_scope(&local_ref, &items),
        LayoutScopeDecision {
            segments: vec![
                "components".to_owned(),
                "faceted_components".to_owned(),
                "action_condition_cache_component".to_owned(),
                "local_component_ref_base".to_owned(),
            ],
            reason: LayoutScopeReason::SemanticWrapperTarget,
        }
    );
}

#[test]
fn scopes_template_wrappers_by_wrapper_family() {
    let wrapper = item(
        uuid!("aaaaaaaa-aaaa-aaaa-aaaa-aaaaaaaaaaaa"),
        "RemoteServerFacetRef<PlayerComponentServerFacet >",
        Vec::new(),
    );

    assert_eq!(
        inheritance_scope_segments(&wrapper, &BTreeMap::new()),
        vec!["remote_server_facet_refs"]
    );
}

#[test]
fn scopes_namespaced_template_wrappers_by_wrapper_namespace_and_family() {
    let base = item(
        uuid!("aaaaaaaa-aaaa-aaaa-aaaa-aaaaaaaaaaaa"),
        "SimpleAssetReferenceBase",
        Vec::new(),
    );
    let wrapper = item(
        uuid!("bbbbbbbb-bbbb-bbbb-bbbb-bbbbbbbbbbbb"),
        "AzFramework::SimpleAssetReference<MB::DataSheetAsset>",
        vec![base_field(base.source_type_id, &base.source_name)],
    );
    let items = BTreeMap::from([
        (base.source_type_id, &base),
        (wrapper.source_type_id, &wrapper),
    ]);

    assert_eq!(
        inheritance_family_scope_segments(&base, &items),
        vec![
            "az_framework",
            "simple_asset_references",
            "simple_asset_reference_base"
        ]
    );
    assert_eq!(
        inheritance_scope_segments(&wrapper, &items),
        vec![
            "az_framework",
            "simple_asset_references",
            "simple_asset_reference_base"
        ]
    );
}

#[test]
fn scopes_base_edges_by_edge_name_when_type_id_is_reused() {
    let action_condition = item(
        uuid!("401ea5b5-dde2-4848-be17-fd45660ff8c5"),
        "ActionCondition",
        Vec::new(),
    );
    let activity_data = item(
        uuid!("d490719f-8531-4f82-a64e-ee29dc6aea50"),
        "SetMannequinTagData",
        vec![base_field(action_condition.source_type_id, "ActivityData")],
    );
    let items = BTreeMap::from([
        (action_condition.source_type_id, &action_condition),
        (activity_data.source_type_id, &activity_data),
    ]);

    assert_eq!(
        inheritance_scope_segments(&activity_data, &items),
        vec!["activity_datas"]
    );
    assert!(
        !reflected_base_type_ids(&SerializeCodegenUnit {
            items: vec![action_condition, activity_data]
        })
        .contains(&uuid!("401ea5b5-dde2-4848-be17-fd45660ff8c5"))
    );
}

#[test]
fn layout_analysis_explains_action_condition_family_scope() {
    let mut action_condition = item(
        uuid!("401ea5b5-dde2-4848-be17-fd45660ff8c5"),
        "ActionCondition",
        Vec::new(),
    );
    action_condition.is_abstract = Some(true);
    let child = item(
        uuid!("84c87fc2-e60d-4ff3-9003-7e02ec84bbbb"),
        "ActionConditionIfInput",
        vec![base_field(
            action_condition.source_type_id,
            &action_condition.source_name,
        )],
    );

    let report = LayoutAnalysisReport::from_codegen_unit(&SerializeCodegenUnit {
        items: vec![action_condition, child],
    });
    let base = report
        .item_by_source_name("ActionCondition")
        .expect("ActionCondition layout");
    let child = report
        .item_by_source_name("ActionConditionIfInput")
        .expect("ActionConditionIfInput layout");

    assert_eq!(base.namespace_segments, Vec::<String>::new());
    assert!(base.is_abstract == Some(true));
    assert_eq!(
        base.serialized_shape,
        LayoutSerializedShape::AbstractBaseWithoutData
    );
    assert_eq!(base.serialized_field_count, 0);
    assert!(base.is_base_family_root);
    assert_eq!(
        base.direct_derived_source_names,
        vec!["ActionConditionIfInput"]
    );
    assert_eq!(base.emitted_scope_segments, vec!["action_conditions"]);
    assert_eq!(child.namespace_segments, Vec::<String>::new());
    assert_eq!(
        child.serialized_shape,
        LayoutSerializedShape::ConcreteStatelessType
    );
    assert_eq!(child.serialized_field_count, 1);
    assert_eq!(child.serialized_base_field_count, 1);
    assert_eq!(child.serialized_data_field_count, 0);
    assert!(!child.is_base_family_root);
    assert_eq!(child.emitted_scope_segments, vec!["action_conditions"]);
    assert_eq!(
        child.primary_base_chain,
        vec![LayoutBaseEdge {
            type_id: uuid!("401ea5b5-dde2-4848-be17-fd45660ff8c5"),
            source_name: "ActionCondition".to_owned(),
            matches_reflected_type: true,
        }]
    );
}

#[test]
fn layout_analysis_explains_az_std_chrono_namespace_scope() {
    let time_point = item(
        uuid!("5c48fd59-7267-405d-9c06-1ea31379fe82"),
        "AZStd::chrono::system_clock::time_point",
        Vec::new(),
    );

    let report = LayoutAnalysisReport::from_codegen_unit(&SerializeCodegenUnit {
        items: vec![time_point],
    });
    let item = report
        .item_by_source_name("AZStd::chrono::system_clock::time_point")
        .expect("time_point layout");

    assert_eq!(
        item.namespace_segments,
        vec!["az_std", "chrono", "system_clock"]
    );
    assert_eq!(
        item.serialized_shape,
        LayoutSerializedShape::ConcreteStatelessType
    );
    assert_eq!(item.serialized_field_count, 0);
    assert!(!item.is_base_family_root);
    assert!(item.primary_base_chain.is_empty());
    assert_eq!(
        item.emitted_scope_segments,
        vec!["az_std", "chrono", "system_clock"]
    );
}

#[test]
fn layout_analysis_explains_real_action_condition_and_chrono_schema_facts() {
    let path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("resources")
        .join("serialize.json");
    let document = SerializeContextDocument::from_path(path).expect("project serialize.json");
    let model = SerializeContextModel::from_document(&document);
    let unit = SerializeCodegenPlanner::plan_model(&model);
    let report = LayoutAnalysisReport::from_codegen_unit(&unit);

    let action_condition = report
        .item_by_source_name("ActionCondition")
        .expect("ActionCondition analysis");
    assert_eq!(
        action_condition.serialized_shape,
        LayoutSerializedShape::AbstractBaseWithoutData
    );
    assert_eq!(
        action_condition.factory.as_deref(),
        Some("NewWorld+0x9eb4750")
    );
    assert!(
        action_condition
            .direct_derived_source_names
            .iter()
            .any(|name| name == "ActionConditionIfInput")
    );
    assert!(
        action_condition.direct_derived_source_names.len() > 100,
        "expected ActionCondition to be a broad reflected behavior family"
    );

    let input_condition = report
        .item_by_source_name("ActionConditionIfInput")
        .expect("ActionConditionIfInput analysis");
    assert_eq!(
        input_condition.serialized_shape,
        LayoutSerializedShape::ConcreteStatelessType
    );
    assert_eq!(
        input_condition.factory.as_deref(),
        Some("NewWorld+0x9eb6a88")
    );
    assert_eq!(input_condition.serialized_field_count, 1);
    assert_eq!(input_condition.serialized_base_field_count, 1);
    assert_eq!(input_condition.serialized_data_field_count, 0);
    assert_eq!(
        input_condition.primary_base_chain,
        vec![LayoutBaseEdge {
            type_id: uuid!("401ea5b5-dde2-4848-be17-fd45660ff8c5"),
            source_name: "ActionCondition".to_owned(),
            matches_reflected_type: true,
        }]
    );
    assert_eq!(
        input_condition.emitted_scope_segments,
        vec!["action_conditions"]
    );

    let time_point = report
        .item_by_source_name("AZStd::chrono::system_clock::time_point")
        .expect("time_point analysis");
    assert_eq!(
        time_point.serialized_shape,
        LayoutSerializedShape::UnknownAbstractnessWithoutData
    );
    assert_eq!(time_point.factory.as_deref(), Some("NewWorld+0x9e8b450"));
    assert_eq!(
        time_point.emitted_scope_segments,
        vec!["az_std", "chrono", "system_clock"]
    );

    let simple_asset_reference_base = report
        .item_by_source_name("SimpleAssetReferenceBase")
        .expect("SimpleAssetReferenceBase analysis");
    assert_eq!(
        simple_asset_reference_base.emitted_scope_segments,
        vec!["simple_asset_references", "simple_asset_reference_base"]
    );

    let data_sheet_reference = report
        .item_by_source_name("AzFramework::SimpleAssetReference<MB::DataSheetAsset>")
        .expect("SimpleAssetReference<MB::DataSheetAsset> analysis");
    assert_eq!(
        data_sheet_reference.emitted_scope_segments,
        vec![
            "az_framework",
            "simple_asset_references",
            "simple_asset_reference_base"
        ]
    );

    let action_list_server_facet = report
        .item_by_source_name("ActionListComponentServerFacet")
        .expect("ActionListComponentServerFacet analysis");
    assert_eq!(
        action_list_server_facet.emitted_scope_segments,
        vec!["components", "faceted_components", "action_list_component"]
    );
    assert_eq!(
        action_list_server_facet.slot_anchor,
        Some(LayoutSlotAnchor {
            owner_type_id: uuid!("65cd8f3e-73aa-43e9-8d9a-b5ae43f624f9"),
            owner_source_name: "FacetedComponent".to_owned(),
            owner_field_name: "m_serverFacetPtr".to_owned(),
            slot_type_id: uuid!("0392e589-5b61-47cc-835b-c3c254e76493"),
            slot_source_name: "ServerFacet".to_owned(),
        })
    );
    assert_eq!(
        action_list_server_facet.concrete_slot_binding,
        Some(LayoutConcreteSlotBinding {
            owner_type_id: uuid!("30ed0ace-51dd-48b9-ba41-2fa6775cd106"),
            owner_source_name: "ActionListComponent".to_owned(),
            slot_owner_type_id: uuid!("65cd8f3e-73aa-43e9-8d9a-b5ae43f624f9"),
            slot_owner_source_name: "FacetedComponent".to_owned(),
            owner_field_name: "m_serverFacetPtr".to_owned(),
            slot_type_id: uuid!("0392e589-5b61-47cc-835b-c3c254e76493"),
            slot_source_name: "ServerFacet".to_owned(),
        })
    );

    let action_list_component = report
        .item_by_source_name("ActionListComponent")
        .expect("ActionListComponent analysis");
    assert!(action_list_component.slot_anchor.is_none());
    assert!(action_list_component.concrete_slot_binding.is_none());
    assert_eq!(
        action_list_component.emitted_scope_segments,
        vec!["components", "faceted_components", "action_list_component"]
    );

    let item_family = report
        .item_by_type_id(uuid!("b9f3747d-192b-5eda-606d-737d339a9679"))
        .expect("Item family analysis");
    let item_record = report
        .item_by_type_id(uuid!("a6d8db05-cc68-4fbe-8002-55c0c7b1fd08"))
        .expect("Item record analysis");
    assert_eq!(item_family.source_name, "Item");
    assert_eq!(item_record.source_name, "Item");
    assert_eq!(item_family.emitted_scope_segments, vec!["item"]);
    assert_eq!(item_record.emitted_scope_segments, Vec::<String>::new());
    assert_eq!(item_record.emitted_scope_reason, LayoutScopeReason::Root);
    assert_ne!(
        item_record.emitted_scope_segments, item_family.emitted_scope_segments,
        "distinct reflected Item roots should not collapse into the same family scope:\nfamily={item_family:#?}\nrecord={item_record:#?}"
    );

    let magic_local_ref = report
            .item_by_source_name(
                "LocalComponentRef<InterfaceType><const char *__cdecl MB::GetTypeName<class Javelin::MagicComponent>(void)>",
            )
            .expect("LocalComponentRef<MagicComponent> analysis");
    assert_eq!(magic_local_ref.role, ReflectedTypeRole::SupportType);
    assert_eq!(
        magic_local_ref.emitted_scope_segments,
        vec![
            "components",
            "faceted_components",
            "magic_component",
            "local_component_ref_base"
        ]
    );
    assert_eq!(
        magic_local_ref.emitted_scope_reason,
        LayoutScopeReason::SemanticWrapperTarget
    );

    let runtime_report = LayoutAnalysisReport::from_codegen_unit(
        &unit.select(SerializeCodegenSelection::RuntimeRoots),
    );
    let runtime_simple_asset_reference_base = runtime_report
        .item_by_source_name("SimpleAssetReferenceBase")
        .expect("runtime SimpleAssetReferenceBase analysis");
    assert_eq!(
        runtime_simple_asset_reference_base.emitted_scope_segments,
        vec![
            "az_framework",
            "simple_asset_references",
            "simple_asset_reference_base"
        ]
    );
    let runtime_navigation_profile = runtime_report
        .item_by_source_name("NavigationProfile")
        .expect("runtime NavigationProfile analysis");
    assert_eq!(
        runtime_navigation_profile.emitted_scope_segments,
        vec!["components", "faceted_components"]
    );
    let runtime_slayer_script_data_container = runtime_report
        .item_by_source_name("SlayerScriptDataContainer")
        .expect("runtime SlayerScriptDataContainer analysis");
    assert_eq!(
        runtime_slayer_script_data_container.emitted_scope_segments,
        vec!["components", "faceted_components"]
    );
    let runtime_remote_typeless_server_facet_ref = runtime_report
        .item_by_source_name("RemoteTypelessServerFacetRef")
        .expect("runtime RemoteTypelessServerFacetRef analysis");
    assert_eq!(
        runtime_remote_typeless_server_facet_ref.emitted_scope_segments,
        vec!["components", "faceted_components"]
    );
    assert!(
            runtime_report
                .item_by_source_name(
                    "LocalComponentRef<InterfaceType><const char *__cdecl MB::GetTypeName<class Javelin::MagicComponent>(void)>",
                )
                .is_none(),
            "runtime roots should not select LocalComponentRef<MagicComponent> unless a selected reflected field owns it"
        );
    let runtime_local_component_ref_base = runtime_report
        .item_by_source_name("LocalComponentRefBase")
        .expect("runtime LocalComponentRefBase analysis");
    assert_eq!(
        runtime_local_component_ref_base.emitted_scope_segments,
        vec![
            "components",
            "faceted_components",
            "local_component_ref_base"
        ]
    );
    let runtime_transform_component = runtime_report
        .item_by_source_name("TransformComponent")
        .expect("runtime TransformComponent analysis");
    assert_eq!(
        runtime_transform_component.emitted_scope_segments,
        vec!["components"],
        "{runtime_transform_component:#?}"
    );
    let root_report = runtime_report.root_report();
    let root_audit = runtime_report.root_audit();
    for (source_name, scope) in [
        ("AZ::Component", vec!["az", "components"]),
        ("AZ::MemoryComponent", vec!["az", "components"]),
        ("AZ::JobManagerComponent", vec!["az", "components"]),
        ("AZ::ObjectStreamComponent", vec!["az", "components"]),
        (
            "AZ::Debug::FrameProfilerComponent",
            vec!["az", "debug", "components"],
        ),
    ] {
        let item = runtime_report
            .item_by_source_name(source_name)
            .unwrap_or_else(|| panic!("{source_name} analysis"));
        assert_eq!(
            item.emitted_scope_segments, scope,
            "{source_name} should keep its source namespace before its component role"
        );
    }
    assert!(
        runtime_report.items.iter().all(|item| {
            !item
                .emitted_scope_segments
                .starts_with(&["components".to_owned(), "az".to_owned()])
        }),
        "AZ namespace should not be nested under components:\n{}",
        runtime_report.to_text()
    );
    assert!(
        !root_audit
            .findings
            .iter()
            .any(|finding| finding.kind == LayoutRootFindingKind::UniqueOwnedSupportRoot),
        "runtime roots should not contain namespace-less support roots with one scoped owner: {:#?}",
        root_audit.findings
    );
    let runtime_event_data = runtime_report
        .item_by_source_name("EventData")
        .expect("runtime EventData analysis");
    assert_eq!(
        runtime_event_data.emitted_scope_segments,
        vec!["components"]
    );
    assert_eq!(
        runtime_event_data.emitted_scope_reason,
        LayoutScopeReason::FieldOwner
    );
    assert!(
        !root_audit
            .findings
            .iter()
            .any(|finding| finding.source_name == "LocalEntityRef"),
        "`LocalEntityRef` should be scoped by its AZ::EntityId wrapper shape, not reported as root:\n{:#?}",
        root_audit.findings
    );
    let runtime_local_entity_ref = runtime_report
        .item_by_source_name("LocalEntityRef")
        .expect("runtime LocalEntityRef analysis");
    assert_eq!(
        runtime_local_entity_ref.emitted_scope_segments,
        vec!["az", "entity"]
    );
    assert_eq!(
        runtime_local_entity_ref.emitted_scope_reason,
        LayoutScopeReason::SemanticWrapper
    );
    let runtime_net_bindable = runtime_report
        .item_by_source_name("NetBindable")
        .expect("runtime NetBindable analysis");
    assert_eq!(
        runtime_net_bindable.emitted_scope_segments,
        vec!["components"],
        "{runtime_net_bindable:#?}"
    );
    assert!(
        root_report.has_root_segment("components"),
        "runtime roots should keep AZ/LY component families under components:\n{}",
        root_report.to_text()
    );
    for accidental_bucket in [
        "global",
        "bases",
        "entities",
        "facets",
        "simple_asset_reference_base",
        "lmbr_central",
        "ly_shine",
        "mb",
        "texture_atlas_namespace",
    ] {
        assert!(
            !root_report.has_root_segment(accidental_bucket),
            "`{accidental_bucket}` should not be a top-level fallback bucket:\n{}",
            root_report.to_text()
        );
    }
    let components = root_report
        .root_by_segment("components")
        .expect("components root");
    assert!(
        components
            .reasons
            .iter()
            .any(|reason| *reason == LayoutScopeReason::ConcreteSlotOwner),
        "components root should include concrete slot owner evidence:\n{}",
        root_report.to_text()
    );
    assert!(
        components
            .reasons
            .iter()
            .any(|reason| *reason == LayoutScopeReason::ConcreteSlotBinding),
        "components root should include concrete slot binding evidence:\n{}",
        root_report.to_text()
    );
}

#[test]
fn pluralizes_child_family_scopes_as_children() {
    let action_condition = item(
        uuid!("aaaaaaaa-aaaa-aaaa-aaaa-aaaaaaaaaaaa"),
        "ActionCondition",
        Vec::new(),
    );
    let single_child = item(
        uuid!("bbbbbbbb-bbbb-bbbb-bbbb-bbbbbbbbbbbb"),
        "ActionConditionSingleChild",
        vec![base_field(
            action_condition.source_type_id,
            &action_condition.source_name,
        )],
    );
    let items = BTreeMap::from([
        (action_condition.source_type_id, &action_condition),
        (single_child.source_type_id, &single_child),
    ]);

    assert_eq!(
        inheritance_family_scope_segments(&single_child, &items),
        vec!["action_conditions", "action_condition_single_children"]
    );
}

fn item(type_id: Uuid, name: &str, fields: Vec<SerializeCodegenField>) -> SerializeCodegenItem {
    let mut item = SerializeCodegenItem {
        source_type_id: type_id,
        source_name: name.to_owned(),
        role: ReflectedTypeRole::SupportType,
        is_reflection_marker: false,
        is_abstract: Some(false),
        factory: None,
        rtti_base_chain: Vec::new(),
        kind: SerializeCodegenItemKind::Struct,
        enum_underlying_type: None,
        fields,
        variants: Vec::<SerializeCodegenVariant>::new(),
    };
    apply_test_reflection_defaults(&mut item);
    item
}

fn apply_test_reflection_defaults(item: &mut SerializeCodegenItem) {
    match item.source_name.as_str() {
        "AZ::Component" => {
            item.role = ReflectedTypeRole::AzComponent;
            item.is_abstract = Some(true);
        }
        "Component" => {
            item.role = ReflectedTypeRole::AzComponent;
        }
        "FacetedComponent" | "Javelin::FacetedComponent" => {
            item.role = ReflectedTypeRole::FacetedComponent;
        }
        "ClientFacet" => {
            item.role = ReflectedTypeRole::ClientFacet;
            item.is_abstract = Some(true);
        }
        "ServerFacet" => {
            item.role = ReflectedTypeRole::ServerFacet;
            item.is_abstract = Some(true);
        }
        "Facet" | "ActionCondition" | "ActionConditionSingleChild" => {
            item.is_abstract = Some(true);
        }
        "MeshComponentRenderNode" => {
            item.is_abstract = None;
        }
        _ => {}
    }
}

fn base_field(type_id: Uuid, source_name: &str) -> SerializeCodegenField {
    SerializeCodegenField {
        source_name: "BaseClass1".to_owned(),
        source_type_id: type_id,
        resolved_type: ResolvedType::Named {
            type_id,
            source_name: source_name.to_owned(),
        },
        data_size: None,
        offset: None,
        flags: None,
        is_base_class: true,
        is_pointer: false,
        is_dynamic_field: false,
    }
}

fn pointer_field(name: &str, type_id: Uuid, source_name: &str) -> SerializeCodegenField {
    SerializeCodegenField {
        source_name: name.to_owned(),
        source_type_id: type_id,
        resolved_type: ResolvedType::Named {
            type_id,
            source_name: source_name.to_owned(),
        },
        data_size: None,
        offset: None,
        flags: None,
        is_base_class: false,
        is_pointer: true,
        is_dynamic_field: false,
    }
}

fn value_field(name: &str, type_id: Uuid, source_name: &str) -> SerializeCodegenField {
    SerializeCodegenField {
        source_name: name.to_owned(),
        source_type_id: type_id,
        resolved_type: ResolvedType::Named {
            type_id,
            source_name: source_name.to_owned(),
        },
        data_size: None,
        offset: None,
        flags: None,
        is_base_class: false,
        is_pointer: false,
        is_dynamic_field: false,
    }
}

fn scalar_field(name: &str, scalar: ScalarType) -> SerializeCodegenField {
    SerializeCodegenField {
        source_name: name.to_owned(),
        source_type_id: Uuid::from_u128(0),
        resolved_type: ResolvedType::Scalar(scalar),
        data_size: None,
        offset: None,
        flags: None,
        is_base_class: false,
        is_pointer: false,
        is_dynamic_field: false,
    }
}
