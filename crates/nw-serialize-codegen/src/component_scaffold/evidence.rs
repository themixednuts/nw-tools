use std::collections::BTreeMap;

use uuid::Uuid;

use super::FacetOwnerEvidence;
use crate::{
    LayoutIndex, ReflectedField, ReflectedTypeCatalog, SerializeCodegenUnit,
    rust_reflected_type_name as rust_type_name,
};

#[derive(Debug, Clone)]
pub(super) struct ComponentEvidence {
    pub(super) component_name: String,
    pub(super) type_id: Uuid,
    pub(super) fields: Vec<ReflectedField>,
    pub(super) facet_owner: Option<String>,
}

pub(super) fn collect_reflected_component_evidence(
    catalog: &ReflectedTypeCatalog,
    facet_owner_evidence: &BTreeMap<String, FacetOwnerEvidence>,
) -> BTreeMap<Uuid, ComponentEvidence> {
    let mut evidence = BTreeMap::new();
    for ty in catalog.component_scaffold_types() {
        let component_name = rust_type_name(&ty.name, ty.type_id);
        let facet_owner = facet_owner_evidence
            .get(&component_name)
            .map(|evidence| evidence.owner_name.clone());
        evidence.insert(
            ty.type_id,
            ComponentEvidence {
                component_name,
                type_id: ty.type_id,
                fields: ty.serializable_fields().cloned().collect(),
                facet_owner,
            },
        );
    }
    evidence
}

#[must_use]
pub fn facet_owner_evidence_from_layout(unit: &SerializeCodegenUnit) -> Vec<FacetOwnerEvidence> {
    let index = unit.index();
    let items_by_type_id = index.items_by_type_id();
    let layout = LayoutIndex::from_codegen_unit(unit);
    let mut evidence = BTreeMap::new();

    for (facet_type_id, binding) in &layout.concrete_slot_bindings {
        let Some(facet) = items_by_type_id.get(facet_type_id) else {
            continue;
        };
        let Some(owner) = items_by_type_id.get(&binding.owner_type_id) else {
            continue;
        };
        let facet_name = rust_type_name(&facet.source_name, *facet_type_id);
        evidence.insert(
            facet_name.clone(),
            FacetOwnerEvidence {
                facet_name,
                facet_type_id: *facet_type_id,
                owner_name: rust_type_name(&owner.source_name, owner.source_type_id),
                owner_type_id: owner.source_type_id,
                field_name: binding.owner_field_name.clone(),
            },
        );
    }

    evidence.into_values().collect()
}

#[cfg(test)]
mod tests {
    use uuid::uuid;

    use crate::ir::{
        SerializeCodegenField, SerializeCodegenItem, SerializeCodegenItemKind,
        SerializeCodegenRttiBase, SerializeCodegenVariant,
    };
    use crate::role::ReflectedTypeRole;
    use crate::types::ResolvedType;

    use super::*;

    #[test]
    fn derives_facet_owner_evidence_from_component_slot_layout() {
        let faceted_component_id = uuid!("11111111-1111-1111-1111-111111111111");
        let client_facet_marker_id = uuid!("22222222-2222-2222-2222-222222222222");
        let component_id = uuid!("33333333-3333-3333-3333-333333333333");
        let client_facet_id = uuid!("44444444-4444-4444-4444-444444444444");
        let unit = SerializeCodegenUnit {
            items: vec![
                item(
                    faceted_component_id,
                    "FacetedComponent",
                    ReflectedTypeRole::FacetedComponent,
                    vec![pointer_field(
                        "m_clientFacetPtr",
                        client_facet_marker_id,
                        "ClientFacet",
                    )],
                ),
                item(
                    client_facet_marker_id,
                    "ClientFacet",
                    ReflectedTypeRole::ClientFacet,
                    Vec::new(),
                ),
                item(
                    component_id,
                    "GroupsComponent",
                    ReflectedTypeRole::FacetedComponent,
                    vec![base_field(faceted_component_id, "FacetedComponent")],
                ),
                item(
                    client_facet_id,
                    "GroupsComponentClientFacet",
                    ReflectedTypeRole::ClientFacet,
                    vec![base_field(client_facet_marker_id, "ClientFacet")],
                ),
            ],
        };

        let evidence = facet_owner_evidence_from_layout(&unit);

        assert_eq!(evidence.len(), 1);
        assert_eq!(evidence[0].facet_name, "GroupsComponentClientFacet");
        assert_eq!(evidence[0].facet_type_id, client_facet_id);
        assert_eq!(evidence[0].owner_name, "GroupsComponent");
        assert_eq!(evidence[0].owner_type_id, component_id);
        assert_eq!(evidence[0].field_name, "m_clientFacetPtr");
    }

    fn item(
        source_type_id: uuid::Uuid,
        source_name: &str,
        role: ReflectedTypeRole,
        fields: Vec<SerializeCodegenField>,
    ) -> SerializeCodegenItem {
        SerializeCodegenItem {
            source_type_id,
            source_name: source_name.to_owned(),
            role,
            is_reflection_marker: false,
            is_abstract: Some(false),
            factory: None,
            rtti_base_chain: Vec::<SerializeCodegenRttiBase>::new(),
            kind: SerializeCodegenItemKind::Struct,
            enum_underlying_type: None,
            fields,
            variants: Vec::<SerializeCodegenVariant>::new(),
        }
    }

    fn base_field(type_id: uuid::Uuid, source_name: &str) -> SerializeCodegenField {
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

    fn pointer_field(name: &str, type_id: uuid::Uuid, source_name: &str) -> SerializeCodegenField {
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
}
