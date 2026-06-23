use std::collections::{BTreeMap, BTreeSet};

use uuid::Uuid;

use crate::model::SerializeContextModel;
use crate::types::scalar_type;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReflectedTypeRole {
    FacetedComponent,
    AzComponent,
    ClientFacet,
    ServerFacet,
    AzEntity,
    SupportType,
}

impl ReflectedTypeRole {
    #[must_use]
    pub const fn is_component(self) -> bool {
        matches!(self, Self::FacetedComponent | Self::AzComponent)
    }

    #[must_use]
    pub const fn is_az_component_like(self) -> bool {
        matches!(
            self,
            Self::FacetedComponent | Self::AzComponent | Self::ClientFacet | Self::ServerFacet
        )
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RoleRootPolicy {
    pub az_component_names: Vec<String>,
    pub component_names: Vec<String>,
    pub faceted_component_names: Vec<String>,
    pub facet_marker_names: Vec<String>,
    pub client_facet_names: Vec<String>,
    pub server_facet_names: Vec<String>,
    pub az_entity_names: Vec<String>,
}

impl Default for RoleRootPolicy {
    fn default() -> Self {
        Self {
            az_component_names: vec!["AZ::Component".to_owned()],
            component_names: vec!["Component".to_owned()],
            faceted_component_names: vec!["FacetedComponent".to_owned()],
            facet_marker_names: vec!["Facet".to_owned()],
            client_facet_names: vec!["ClientFacet".to_owned()],
            server_facet_names: vec!["ServerFacet".to_owned()],
            az_entity_names: vec!["AZ::Entity".to_owned()],
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SerializeRoleClassifier {
    roots: ReflectedRoleRoots,
    base_type_ids_by_id: BTreeMap<Uuid, Vec<Uuid>>,
}

impl SerializeRoleClassifier {
    #[must_use]
    pub fn from_model(model: &SerializeContextModel) -> Self {
        let names_by_id = model
            .classes
            .values()
            .map(|class| (class.type_id, class.name.clone()))
            .collect::<BTreeMap<_, _>>();
        let base_type_ids_by_id = model
            .classes
            .values()
            .map(|class| {
                let mut base_type_ids = class
                    .az_rtti
                    .as_ref()
                    .filter(|rtti| rtti.type_id == Some(class.type_id))
                    .map(|rtti| {
                        rtti.hierarchy
                            .iter()
                            .enumerate()
                            .filter(|(index, entry)| {
                                !(*index == 0 && entry.type_id == class.type_id)
                            })
                            .map(|(_, entry)| entry.type_id)
                            .collect::<Vec<_>>()
                    })
                    .unwrap_or_default();
                base_type_ids.extend(
                    class
                        .members
                        .iter()
                        .filter(|member| member.is_base_class)
                        .map(|member| member.type_id),
                );
                base_type_ids.sort_unstable();
                base_type_ids.dedup();
                (class.type_id, base_type_ids)
            })
            .collect::<BTreeMap<_, _>>();
        Self::from_names_and_bases(
            &names_by_id,
            base_type_ids_by_id,
            &RoleRootPolicy::default(),
        )
    }

    #[must_use]
    pub fn from_names_and_bases(
        names_by_id: &BTreeMap<Uuid, String>,
        base_type_ids_by_id: BTreeMap<Uuid, Vec<Uuid>>,
        policy: &RoleRootPolicy,
    ) -> Self {
        Self {
            roots: ReflectedRoleRoots::from_names(names_by_id, policy),
            base_type_ids_by_id,
        }
    }

    #[must_use]
    pub fn classify(&self, type_id: Uuid) -> ReflectedTypeRole {
        if self.inherits_or_is(type_id, self.roots.client_facet) {
            ReflectedTypeRole::ClientFacet
        } else if self.inherits_or_is(type_id, self.roots.server_facet) {
            ReflectedTypeRole::ServerFacet
        } else if self.inherits_or_is(type_id, self.roots.faceted_component) {
            ReflectedTypeRole::FacetedComponent
        } else if self.inherits_or_is(type_id, self.roots.component)
            || self.inherits_or_is(type_id, self.roots.az_component)
        {
            ReflectedTypeRole::AzComponent
        } else if self.inherits_or_is(type_id, self.roots.az_entity) {
            ReflectedTypeRole::AzEntity
        } else {
            ReflectedTypeRole::SupportType
        }
    }

    #[must_use]
    pub fn is_reflection_marker(&self, type_id: Uuid) -> bool {
        self.roots.reflection_marker_type_ids.contains(&type_id)
    }

    #[must_use]
    pub fn reflection_marker_type_ids(&self) -> &BTreeSet<Uuid> {
        &self.roots.reflection_marker_type_ids
    }

    fn inherits_or_is(&self, type_id: Uuid, root: Option<Uuid>) -> bool {
        root.is_some_and(|root| type_id == root || self.inherits_from(type_id, root))
    }

    fn inherits_from(&self, type_id: Uuid, root_type_id: Uuid) -> bool {
        let mut stack = self
            .base_type_ids_by_id
            .get(&type_id)
            .cloned()
            .unwrap_or_default();
        let mut visited = BTreeSet::new();
        while let Some(base_type_id) = stack.pop() {
            if base_type_id == root_type_id {
                return true;
            }
            if !visited.insert(base_type_id) {
                continue;
            }
            if let Some(base_base_type_ids) = self.base_type_ids_by_id.get(&base_type_id) {
                stack.extend(base_base_type_ids.iter().copied());
            }
        }
        false
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
struct ReflectedRoleRoots {
    az_component: Option<Uuid>,
    component: Option<Uuid>,
    faceted_component: Option<Uuid>,
    facet_marker: Option<Uuid>,
    client_facet: Option<Uuid>,
    server_facet: Option<Uuid>,
    az_entity: Option<Uuid>,
    reflection_marker_type_ids: BTreeSet<Uuid>,
}

impl ReflectedRoleRoots {
    fn from_names(names_by_id: &BTreeMap<Uuid, String>, policy: &RoleRootPolicy) -> Self {
        let mut roots = Self::default();
        for (type_id, name) in names_by_id {
            if scalar_type(*type_id).is_some() {
                roots.reflection_marker_type_ids.insert(*type_id);
            } else if contains_name(&policy.az_component_names, name) {
                roots.az_component = Some(*type_id);
            } else if contains_name(&policy.component_names, name) {
                roots.component = Some(*type_id);
            } else if contains_name(&policy.faceted_component_names, name) {
                roots.faceted_component = Some(*type_id);
            } else if contains_name(&policy.facet_marker_names, name) {
                roots.facet_marker = Some(*type_id);
            } else if contains_name(&policy.client_facet_names, name) {
                roots.client_facet = Some(*type_id);
            } else if contains_name(&policy.server_facet_names, name) {
                roots.server_facet = Some(*type_id);
            } else if contains_name(&policy.az_entity_names, name) {
                roots.az_entity = Some(*type_id);
            }
        }
        roots
    }
}

fn contains_name(names: &[String], name: &str) -> bool {
    names.iter().any(|candidate| candidate == name)
}

#[cfg(test)]
mod tests {
    use serde_json::json;
    use uuid::uuid;

    use crate::model::SerializeContextModel;

    use super::*;

    #[test]
    fn classifies_roles_from_base_class_edges() {
        let component = uuid!("AAAAAAAA-AAAA-AAAA-AAAA-AAAAAAAAAAAA");
        let health = uuid!("BBBBBBBB-BBBB-BBBB-BBBB-BBBBBBBBBBBB");
        let support = uuid!("CCCCCCCC-CCCC-CCCC-CCCC-CCCCCCCCCCCC");
        let names = BTreeMap::from([
            (component, "Component".to_owned()),
            (health, "Example::HealthComponent".to_owned()),
            (support, "Example::Support".to_owned()),
        ]);
        let bases = BTreeMap::from([(health, vec![component]), (support, Vec::new())]);

        let classifier = SerializeRoleClassifier::from_names_and_bases(
            &names,
            bases,
            &RoleRootPolicy::default(),
        );

        assert_eq!(
            classifier.classify(component),
            ReflectedTypeRole::AzComponent
        );
        assert_eq!(classifier.classify(health), ReflectedTypeRole::AzComponent);
        assert_eq!(classifier.classify(support), ReflectedTypeRole::SupportType);
        assert!(!classifier.is_reflection_marker(component));
    }

    #[test]
    fn classifies_roles_from_az_rtti_hierarchy_when_base_fields_are_absent() {
        let model = SerializeContextModel::from_root(&json!({
            "$id": 1,
            "uuidMap": {
                "AAAAAAAA-AAAA-AAAA-AAAA-AAAAAAAAAAAA": {
                    "$id": 10,
                    "name": "AZ::Component",
                    "typeId": "AAAAAAAA-AAAA-AAAA-AAAA-AAAAAAAAAAAA",
                    "azRtti": {
                        "typeId": "AAAAAAAA-AAAA-AAAA-AAAA-AAAAAAAAAAAA",
                        "typeName": "AZ::Component",
                        "hierarchy": [{
                            "typeId": "AAAAAAAA-AAAA-AAAA-AAAA-AAAAAAAAAAAA",
                            "typeName": "AZ::Component"
                        }],
                        "isAbstract": true
                    },
                    "elements": [],
                    "attributes": []
                },
                "BBBBBBBB-BBBB-BBBB-BBBB-BBBBBBBBBBBB": {
                    "$id": 20,
                    "name": "Example::RuntimeOnlyComponent",
                    "typeId": "BBBBBBBB-BBBB-BBBB-BBBB-BBBBBBBBBBBB",
                    "azRtti": {
                        "typeId": "BBBBBBBB-BBBB-BBBB-BBBB-BBBBBBBBBBBB",
                        "typeName": "Example::RuntimeOnlyComponent",
                        "hierarchy": [{
                            "typeId": "BBBBBBBB-BBBB-BBBB-BBBB-BBBBBBBBBBBB",
                            "typeName": "Example::RuntimeOnlyComponent"
                        }, {
                            "typeId": "AAAAAAAA-AAAA-AAAA-AAAA-AAAAAAAAAAAA",
                            "typeName": "AZ::Component"
                        }],
                        "isAbstract": false
                    },
                    "elements": [],
                    "attributes": []
                }
            },
            "classNameToUuid": [],
            "uuidGenericMap": [],
            "uuidAnyCreationMap": {},
            "editContext": {"$id": 2, "classData": [], "enumData": []},
            "enumTypeIdToUnderlyingTypeIdMap": {}
        }));

        let classifier = SerializeRoleClassifier::from_model(&model);

        assert_eq!(
            classifier.classify(uuid!("BBBBBBBB-BBBB-BBBB-BBBB-BBBBBBBBBBBB")),
            ReflectedTypeRole::AzComponent
        );
    }

    #[test]
    fn ignores_generic_instance_rtti_hierarchy_for_role_classification() {
        let model = SerializeContextModel::from_root(&json!({
            "$id": 1,
            "uuidMap": {
                "AAAAAAAA-AAAA-AAAA-AAAA-AAAAAAAAAAAA": {
                    "$id": 10,
                    "name": "ClientFacet",
                    "typeId": "AAAAAAAA-AAAA-AAAA-AAAA-AAAAAAAAAAAA",
                    "azRtti": {
                        "typeId": "AAAAAAAA-AAAA-AAAA-AAAA-AAAAAAAAAAAA",
                        "typeName": "ClientFacet",
                        "hierarchy": [{
                            "typeId": "AAAAAAAA-AAAA-AAAA-AAAA-AAAAAAAAAAAA",
                            "typeName": "ClientFacet"
                        }],
                        "isAbstract": true
                    },
                    "elements": [],
                    "attributes": []
                },
                "BBBBBBBB-BBBB-BBBB-BBBB-BBBBBBBBBBBB": {
                    "$id": 20,
                    "name": "UnstuckComponentClientFacet",
                    "typeId": "BBBBBBBB-BBBB-BBBB-BBBB-BBBBBBBBBBBB",
                    "azRtti": {
                        "typeId": "BBBBBBBB-BBBB-BBBB-BBBB-BBBBBBBBBBBB",
                        "typeName": "UnstuckComponentClientFacet",
                        "hierarchy": [{
                            "typeId": "BBBBBBBB-BBBB-BBBB-BBBB-BBBBBBBBBBBB",
                            "typeName": "UnstuckComponentClientFacet"
                        }, {
                            "typeId": "AAAAAAAA-AAAA-AAAA-AAAA-AAAAAAAAAAAA",
                            "typeName": "ClientFacet"
                        }],
                        "isAbstract": false
                    },
                    "elements": [{
                        "$id": 21,
                        "name": "BaseClass1",
                        "typeId": "AAAAAAAA-AAAA-AAAA-AAAA-AAAAAAAAAAAA",
                        "is_base_class": true
                    }],
                    "attributes": []
                },
                "CCCCCCCC-CCCC-CCCC-CCCC-CCCCCCCCCCCC": {
                    "$id": 30,
                    "name": "LocalComponentRefBase",
                    "typeId": "CCCCCCCC-CCCC-CCCC-CCCC-CCCCCCCCCCCC",
                    "elements": [],
                    "attributes": []
                },
                "DDDDDDDD-DDDD-DDDD-DDDD-DDDDDDDDDDDD": {
                    "$id": 40,
                    "name": "LocalComponentRef<InterfaceType><const char *__cdecl MB::GetTypeName<class Javelin::MagicComponent>(void)>",
                    "typeId": "DDDDDDDD-DDDD-DDDD-DDDD-DDDDDDDDDDDD",
                    "azRtti": {
                        "typeId": "EEEEEEEE-EEEE-EEEE-EEEE-EEEEEEEEEEEE",
                        "typeName": null,
                        "hierarchy": [{
                            "typeId": "EEEEEEEE-EEEE-EEEE-EEEE-EEEEEEEEEEEE",
                            "typeName": null
                        }, {
                            "typeId": "CCCCCCCC-CCCC-CCCC-CCCC-CCCCCCCCCCCC",
                            "typeName": "LocalComponentRefBase"
                        }, {
                            "typeId": "BBBBBBBB-BBBB-BBBB-BBBB-BBBBBBBBBBBB",
                            "typeName": "UnstuckComponentClientFacet"
                        }],
                        "isAbstract": false
                    },
                    "elements": [{
                        "$id": 41,
                        "name": "BaseClass1",
                        "typeId": "CCCCCCCC-CCCC-CCCC-CCCC-CCCCCCCCCCCC",
                        "is_base_class": true
                    }],
                    "attributes": []
                }
            },
            "classNameToUuid": [],
            "uuidGenericMap": [],
            "uuidAnyCreationMap": {},
            "editContext": {"$id": 2, "classData": [], "enumData": []},
            "enumTypeIdToUnderlyingTypeIdMap": {}
        }));

        let classifier = SerializeRoleClassifier::from_model(&model);

        assert_eq!(
            classifier.classify(uuid!("DDDDDDDD-DDDD-DDDD-DDDD-DDDDDDDDDDDD")),
            ReflectedTypeRole::SupportType
        );
    }
}
