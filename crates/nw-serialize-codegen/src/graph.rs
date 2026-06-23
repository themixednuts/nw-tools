use std::collections::{BTreeMap, BTreeSet};

use uuid::Uuid;

use crate::model::{ReflectedClass, ReflectedGenericClass, ReflectedMember, SerializeContextModel};
use crate::naming::{ParsedSourceName, SourceNameKind, rust_type_ident};
use crate::role::{ReflectedTypeRole, SerializeRoleClassifier};

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SchemaGraph {
    pub nodes: BTreeMap<Uuid, SchemaNode>,
    pub edges: Vec<SchemaEdge>,
    pub diagnostics: Vec<SchemaGraphDiagnostic>,
}

impl SchemaGraph {
    #[must_use]
    pub fn from_model(model: &SerializeContextModel) -> Self {
        GraphBuilder::new(model).build()
    }

    #[must_use]
    pub fn node(&self, type_id: Uuid) -> Option<&SchemaNode> {
        self.nodes.get(&type_id)
    }

    pub fn edges_from(&self, type_id: Uuid) -> impl Iterator<Item = &SchemaEdge> {
        self.edges.iter().filter(move |edge| edge.from == type_id)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SchemaNode {
    pub type_id: Uuid,
    pub source_name: String,
    pub kind: SchemaNodeKind,
    pub role: ReflectedTypeRole,
    pub is_abstract: Option<bool>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SchemaNodeKind {
    Class,
    Enum,
    GenericClass,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SchemaEdge {
    pub from: Uuid,
    pub to: Option<Uuid>,
    pub target_name: Option<String>,
    pub kind: SchemaEdgeKind,
    pub provenance: SchemaEdgeProvenance,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SchemaEdgeKind {
    Inherits {
        member_name: String,
    },
    Field {
        field_name: String,
        is_pointer: bool,
        is_dynamic_field: bool,
    },
    GenericArgument {
        index: usize,
    },
    GenericField {
        field_name: String,
    },
    WrapperTarget {
        wrapper: String,
        target: String,
    },
    FacetSlot {
        side: FacetSide,
        field_name: String,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FacetSide {
    Client,
    Server,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SchemaEdgeProvenance {
    SerializeContextMember,
    SerializeContextGeneric,
    SourceNameTemplate,
    SerializeContextFacetSlot,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SchemaGraphDiagnostic {
    pub code: SchemaGraphDiagnosticCode,
    pub source_type_id: Uuid,
    pub source_name: String,
    pub message: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SchemaGraphDiagnosticCode {
    MissingWrapperTarget,
    AmbiguousWrapperTarget,
}

struct GraphBuilder<'a> {
    model: &'a SerializeContextModel,
    role_classifier: SerializeRoleClassifier,
    graph: SchemaGraph,
    visited_generics: BTreeSet<Uuid>,
}

impl<'a> GraphBuilder<'a> {
    fn new(model: &'a SerializeContextModel) -> Self {
        Self {
            model,
            role_classifier: SerializeRoleClassifier::from_model(model),
            graph: SchemaGraph::default(),
            visited_generics: BTreeSet::new(),
        }
    }

    fn build(mut self) -> SchemaGraph {
        self.add_nodes();
        self.add_class_edges();
        self.add_generic_edges();
        self.graph
    }

    fn add_nodes(&mut self) {
        for class in self.model.classes.values() {
            self.graph.nodes.insert(
                class.type_id,
                SchemaNode {
                    type_id: class.type_id,
                    source_name: class.name.clone(),
                    kind: SchemaNodeKind::Class,
                    role: self.role_classifier.classify(class.type_id),
                    is_abstract: class.az_rtti.as_ref().and_then(|rtti| rtti.is_abstract),
                },
            );
        }

        for enumeration in self.model.enums.values() {
            self.graph
                .nodes
                .entry(enumeration.type_id)
                .or_insert_with(|| SchemaNode {
                    type_id: enumeration.type_id,
                    source_name: enumeration.name.clone(),
                    kind: SchemaNodeKind::Enum,
                    role: ReflectedTypeRole::SupportType,
                    is_abstract: Some(false),
                });
        }

        for generic in self.model.generic_classes.values() {
            let Some(type_id) = primary_generic_type_id(generic) else {
                continue;
            };
            self.graph
                .nodes
                .entry(type_id)
                .or_insert_with(|| SchemaNode {
                    type_id,
                    source_name: generic
                        .class_name
                        .clone()
                        .unwrap_or_else(|| "ReflectedGenericClass".to_owned()),
                    kind: SchemaNodeKind::GenericClass,
                    role: ReflectedTypeRole::SupportType,
                    is_abstract: Some(false),
                });
        }
    }

    fn add_class_edges(&mut self) {
        for class in self.model.classes.values() {
            self.add_wrapper_edge(class);
            for member in &class.members {
                self.add_member_edge(class, member);
                if let Some(generic) = member.generic_class.as_deref() {
                    self.add_generic_edges_from(generic);
                }
            }
        }
    }

    fn add_wrapper_edge(&mut self, class: &ReflectedClass) {
        let parsed = ParsedSourceName::parse(&class.name);
        let (wrapper, target) = match parsed.kind {
            SourceNameKind::TemplateWrapper { wrapper, target } => (wrapper, target),
            SourceNameKind::LocalComponentRef { target, .. } => {
                ("LocalComponentRef".to_owned(), target)
            }
            SourceNameKind::Plain
            | SourceNameKind::EditEnum { .. }
            | SourceNameKind::GetTypeNameFunction(_) => return,
        };
        let target_type_ids = self.type_ids_by_source_name(&target);
        let to = match target_type_ids.as_slice() {
            [type_id] => Some(*type_id),
            [] => {
                self.graph.diagnostics.push(SchemaGraphDiagnostic {
                    code: SchemaGraphDiagnosticCode::MissingWrapperTarget,
                    source_type_id: class.type_id,
                    source_name: class.name.clone(),
                    message: format!(
                        "template wrapper `{}` targets `{target}`, but no reflected type has that source name",
                        class.name
                    ),
                });
                None
            }
            _ => {
                self.graph.diagnostics.push(SchemaGraphDiagnostic {
                    code: SchemaGraphDiagnosticCode::AmbiguousWrapperTarget,
                    source_type_id: class.type_id,
                    source_name: class.name.clone(),
                    message: format!(
                        "template wrapper `{}` targets `{target}`, but the source name resolves to multiple reflected types",
                        class.name
                    ),
                });
                None
            }
        };
        self.graph.edges.push(SchemaEdge {
            from: class.type_id,
            to,
            target_name: Some(target.clone()),
            kind: SchemaEdgeKind::WrapperTarget { wrapper, target },
            provenance: SchemaEdgeProvenance::SourceNameTemplate,
        });
    }

    fn add_member_edge(&mut self, class: &ReflectedClass, member: &ReflectedMember) {
        let target_name = member_type_name(member)
            .or_else(|| self.model.type_name(member.type_id))
            .map(str::to_owned);
        if member.is_base_class {
            self.graph.edges.push(SchemaEdge {
                from: class.type_id,
                to: Some(member.type_id),
                target_name,
                kind: SchemaEdgeKind::Inherits {
                    member_name: member.name.clone(),
                },
                provenance: SchemaEdgeProvenance::SerializeContextMember,
            });
            return;
        }

        self.graph.edges.push(SchemaEdge {
            from: class.type_id,
            to: Some(member.type_id),
            target_name: target_name.clone(),
            kind: SchemaEdgeKind::Field {
                field_name: member.name.clone(),
                is_pointer: member.is_pointer,
                is_dynamic_field: member.is_dynamic_field,
            },
            provenance: SchemaEdgeProvenance::SerializeContextMember,
        });
        self.add_facet_slot_edge(class, member, target_name);
    }

    fn add_facet_slot_edge(
        &mut self,
        class: &ReflectedClass,
        member: &ReflectedMember,
        target_name: Option<String>,
    ) {
        if self.role_classifier.classify(class.type_id) != ReflectedTypeRole::FacetedComponent
            || !member.is_pointer
        {
            return;
        }
        let side = match self.role_classifier.classify(member.type_id) {
            ReflectedTypeRole::ClientFacet => FacetSide::Client,
            ReflectedTypeRole::ServerFacet => FacetSide::Server,
            _ => return,
        };
        self.graph.edges.push(SchemaEdge {
            from: class.type_id,
            to: Some(member.type_id),
            target_name,
            kind: SchemaEdgeKind::FacetSlot {
                side,
                field_name: member.name.clone(),
            },
            provenance: SchemaEdgeProvenance::SerializeContextFacetSlot,
        });
    }

    fn add_generic_edges(&mut self) {
        for generic in self.model.generic_classes.values() {
            self.add_generic_edges_from(generic);
        }
    }

    fn add_generic_edges_from(&mut self, generic: &ReflectedGenericClass) {
        let Some(from) = primary_generic_type_id(generic) else {
            return;
        };
        if !self.visited_generics.insert(from) {
            return;
        }
        for (index, type_id) in generic.argument_type_ids().into_iter().enumerate() {
            self.graph.edges.push(SchemaEdge {
                from,
                to: Some(type_id),
                target_name: self.model.type_name(type_id).map(str::to_owned),
                kind: SchemaEdgeKind::GenericArgument { index },
                provenance: SchemaEdgeProvenance::SerializeContextGeneric,
            });
        }
        for member in &generic.members {
            self.graph.edges.push(SchemaEdge {
                from,
                to: Some(member.type_id),
                target_name: member_type_name(member)
                    .or_else(|| self.model.type_name(member.type_id))
                    .map(str::to_owned),
                kind: SchemaEdgeKind::GenericField {
                    field_name: member.name.clone(),
                },
                provenance: SchemaEdgeProvenance::SerializeContextGeneric,
            });
            if let Some(child) = member.generic_class.as_deref() {
                self.add_generic_edges_from(child);
            }
        }
    }

    fn type_ids_by_source_name(&self, source_name: &str) -> Vec<Uuid> {
        let mut type_ids = self.exact_type_ids_by_source_name(source_name);
        if type_ids.is_empty() {
            let target_ident = rust_type_ident(source_name);
            type_ids = self.type_ids_by_rust_ident(&target_ident);
        }
        type_ids.sort_unstable();
        type_ids.dedup();
        type_ids
    }

    fn exact_type_ids_by_source_name(&self, source_name: &str) -> Vec<Uuid> {
        self.model
            .classes
            .values()
            .filter(|class| class.name == source_name)
            .map(|class| class.type_id)
            .chain(
                self.model
                    .enums
                    .values()
                    .filter(|enumeration| enumeration.name == source_name)
                    .map(|enumeration| enumeration.type_id),
            )
            .chain(
                self.model
                    .generic_classes
                    .values()
                    .filter(|generic| generic.class_name.as_deref() == Some(source_name))
                    .filter_map(primary_generic_type_id),
            )
            .collect::<Vec<_>>()
    }

    fn type_ids_by_rust_ident(&self, target_ident: &str) -> Vec<Uuid> {
        self.model
            .classes
            .values()
            .filter(|class| rust_type_ident(&class.name) == target_ident)
            .map(|class| class.type_id)
            .chain(
                self.model
                    .enums
                    .values()
                    .filter(|enumeration| rust_type_ident(&enumeration.name) == target_ident)
                    .map(|enumeration| enumeration.type_id),
            )
            .chain(
                self.model
                    .generic_classes
                    .values()
                    .filter(|generic| {
                        generic
                            .class_name
                            .as_deref()
                            .is_some_and(|name| rust_type_ident(name) == target_ident)
                    })
                    .filter_map(primary_generic_type_id),
            )
            .collect()
    }
}

fn primary_generic_type_id(generic: &ReflectedGenericClass) -> Option<Uuid> {
    generic
        .specialized_type_id
        .or(generic.type_id)
        .or(generic.map_key_type_id)
        .or_else(|| generic.registered_type_ids.first().copied())
}

fn member_type_name(member: &ReflectedMember) -> Option<&str> {
    member
        .az_rtti
        .as_ref()
        .and_then(|rtti| rtti.type_name.as_deref())
        .filter(|name| !name.is_empty())
}

#[cfg(test)]
mod tests {
    use serde_json::json;
    use uuid::uuid;

    use crate::model::SerializeContextModel;

    use super::*;

    #[test]
    fn graph_keeps_template_wrapper_identity_and_target_edge() {
        let target_id = uuid!("E6EA5644-D713-473E-9121-7C96D7C07022");
        let wrapper_id = uuid!("9F5AA863-376C-5397-AD50-8CC61443FFBF");
        let model = SerializeContextModel::from_root(&json!({
            "$id": 1,
            "uuidMap": {
                "E6EA5644-D713-473E-9121-7C96D7C07022": {
                    "$id": 10,
                    "name": "PlayerComponentServerFacet",
                    "typeId": "E6EA5644-D713-473E-9121-7C96D7C07022",
                    "elements": [],
                    "attributes": []
                },
                "9F5AA863-376C-5397-AD50-8CC61443FFBF": {
                    "$id": 20,
                    "name": "RemoteServerFacetRef<PlayerComponentServerFacet >",
                    "typeId": "9F5AA863-376C-5397-AD50-8CC61443FFBF",
                    "elements": [{
                        "$id": 21,
                        "name": "TypelessRef",
                        "typeId": "6328304A-B754-4AD3-BF78-87236958B55B",
                        "is_base_class": false
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

        let graph = SchemaGraph::from_model(&model);
        let edge = graph
            .edges_from(wrapper_id)
            .find(|edge| matches!(edge.kind, SchemaEdgeKind::WrapperTarget { .. }))
            .expect("wrapper target edge");

        assert_eq!(edge.to, Some(target_id));
        assert_eq!(
            edge.kind,
            SchemaEdgeKind::WrapperTarget {
                wrapper: "RemoteServerFacetRef".to_owned(),
                target: "PlayerComponentServerFacet".to_owned(),
            }
        );
        assert!(graph.diagnostics.is_empty());
    }

    #[test]
    fn graph_resolves_local_component_ref_get_type_name_target_by_canonical_name() {
        let target_id = uuid!("AAAAAAAA-AAAA-AAAA-AAAA-AAAAAAAAAAAA");
        let wrapper_id = uuid!("BBBBBBBB-BBBB-BBBB-BBBB-BBBBBBBBBBBB");
        let model = SerializeContextModel::from_root(&json!({
            "$id": 1,
            "uuidMap": {
                "AAAAAAAA-AAAA-AAAA-AAAA-AAAAAAAAAAAA": {
                    "$id": 10,
                    "name": "PlayerComponent",
                    "typeId": "AAAAAAAA-AAAA-AAAA-AAAA-AAAAAAAAAAAA",
                    "elements": [],
                    "attributes": []
                },
                "BBBBBBBB-BBBB-BBBB-BBBB-BBBBBBBBBBBB": {
                    "$id": 20,
                    "name": "LocalComponentRef<InterfaceType><const char *__cdecl MB::GetTypeName<class Javelin::PlayerComponent>(void)>",
                    "typeId": "BBBBBBBB-BBBB-BBBB-BBBB-BBBBBBBBBBBB",
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

        let graph = SchemaGraph::from_model(&model);
        let edge = graph
            .edges_from(wrapper_id)
            .find(|edge| matches!(edge.kind, SchemaEdgeKind::WrapperTarget { .. }))
            .expect("local component ref target edge");

        assert_eq!(edge.to, Some(target_id));
        assert_eq!(
            edge.kind,
            SchemaEdgeKind::WrapperTarget {
                wrapper: "LocalComponentRef".to_owned(),
                target: "Javelin::PlayerComponent".to_owned(),
            }
        );
        assert!(graph.diagnostics.is_empty());
    }

    #[test]
    fn graph_keeps_facet_slots_separate_from_concrete_facet_ownership() {
        let faceted_id = uuid!("65CD8F3E-73AA-43E9-8D9A-B5AE43F624F9");
        let client_facet_id = uuid!("0643CDC7-B1C9-4721-92CE-7AC02E6175C9");
        let server_facet_id = uuid!("0392E589-5B61-47CC-835B-C3C254E76493");
        let concrete_server_facet_id = uuid!("E6EA5644-D713-473E-9121-7C96D7C07022");
        let model = SerializeContextModel::from_root(&json!({
            "$id": 1,
            "uuidMap": {
                "65CD8F3E-73AA-43E9-8D9A-B5AE43F624F9": {
                    "$id": 10,
                    "name": "FacetedComponent",
                    "typeId": "65CD8F3E-73AA-43E9-8D9A-B5AE43F624F9",
                    "elements": [
                        {
                            "$id": 11,
                            "name": "m_clientFacetPtr",
                            "typeId": "0643CDC7-B1C9-4721-92CE-7AC02E6175C9",
                            "is_pointer": true,
                            "is_base_class": false
                        },
                        {
                            "$id": 12,
                            "name": "m_serverFacetPtr",
                            "typeId": "0392E589-5B61-47CC-835B-C3C254E76493",
                            "is_pointer": true,
                            "is_base_class": false
                        }
                    ],
                    "attributes": []
                },
                "0643CDC7-B1C9-4721-92CE-7AC02E6175C9": {
                    "$id": 20,
                    "name": "ClientFacet",
                    "typeId": "0643CDC7-B1C9-4721-92CE-7AC02E6175C9",
                    "elements": [],
                    "attributes": []
                },
                "0392E589-5B61-47CC-835B-C3C254E76493": {
                    "$id": 30,
                    "name": "ServerFacet",
                    "typeId": "0392E589-5B61-47CC-835B-C3C254E76493",
                    "elements": [],
                    "attributes": []
                },
                "E6EA5644-D713-473E-9121-7C96D7C07022": {
                    "$id": 40,
                    "name": "PlayerComponentServerFacet",
                    "typeId": "E6EA5644-D713-473E-9121-7C96D7C07022",
                    "elements": [{
                        "$id": 41,
                        "name": "BaseClass1",
                        "typeId": "0392E589-5B61-47CC-835B-C3C254E76493",
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

        let graph = SchemaGraph::from_model(&model);
        let facet_slots = graph
            .edges_from(faceted_id)
            .filter_map(|edge| match &edge.kind {
                SchemaEdgeKind::FacetSlot { side, .. } => Some((*side, edge.to)),
                _ => None,
            })
            .collect::<Vec<_>>();
        let concrete_edges_to_faceted = graph
            .edges_from(concrete_server_facet_id)
            .filter(|edge| edge.to == Some(faceted_id))
            .count();

        assert_eq!(
            facet_slots,
            vec![
                (FacetSide::Client, Some(client_facet_id)),
                (FacetSide::Server, Some(server_facet_id)),
            ]
        );
        assert_eq!(concrete_edges_to_faceted, 0);
    }

    #[test]
    fn graph_records_generic_member_edges_for_nested_value_types() {
        let pair_id = uuid!("AAAAAAAA-AAAA-AAAA-AAAA-AAAAAAAAAAAA");
        let key_id = uuid!("BBBBBBBB-BBBB-BBBB-BBBB-BBBBBBBBBBBB");
        let value_id = uuid!("CCCCCCCC-CCCC-CCCC-CCCC-CCCCCCCCCCCC");
        let model = SerializeContextModel::from_root(&json!({
            "$id": 1,
            "uuidMap": {
                "BBBBBBBB-BBBB-BBBB-BBBB-BBBBBBBBBBBB": {
                    "$id": 10,
                    "name": "KeyType",
                    "typeId": "BBBBBBBB-BBBB-BBBB-BBBB-BBBBBBBBBBBB",
                    "elements": [],
                    "attributes": []
                },
                "CCCCCCCC-CCCC-CCCC-CCCC-CCCCCCCCCCCC": {
                    "$id": 20,
                    "name": "ValueType",
                    "typeId": "CCCCCCCC-CCCC-CCCC-CCCC-CCCCCCCCCCCC",
                    "elements": [],
                    "attributes": []
                }
            },
            "classNameToUuid": [],
            "uuidGenericMap": [[
                "AAAAAAAA-AAAA-AAAA-AAAA-AAAAAAAAAAAA",
                {
                    "$id": 30,
                    "typeId": "AAAAAAAA-AAAA-AAAA-AAAA-AAAAAAAAAAAA",
                    "registeredTypeIds": ["AAAAAAAA-AAAA-AAAA-AAAA-AAAAAAAAAAAA"],
                    "templatedArgumentCount": 2,
                    "templatedTypeIds": [
                        "BBBBBBBB-BBBB-BBBB-BBBB-BBBBBBBBBBBB",
                        "CCCCCCCC-CCCC-CCCC-CCCC-CCCCCCCCCCCC"
                    ],
                    "typeIdFoldTypeIds": null,
                    "specializedTypeId": "AAAAAAAA-AAAA-AAAA-AAAA-AAAAAAAAAAAA",
                    "genericTypeId": "DDDDDDDD-DDDD-DDDD-DDDD-DDDDDDDDDDDD",
                    "legacySpecializedTypeId": null,
                    "nonTypeTemplateArguments": null,
                    "classData": {
                        "$id": 31,
                        "name": "AZStd::pair",
                        "typeId": "AAAAAAAA-AAAA-AAAA-AAAA-AAAAAAAAAAAA",
                        "elements": [],
                        "attributes": []
                    },
                    "elements": [
                        {
                            "$id": 32,
                            "name": "value1",
                            "typeId": "BBBBBBBB-BBBB-BBBB-BBBB-BBBBBBBBBBBB",
                            "is_base_class": false
                        },
                        {
                            "$id": 33,
                            "name": "value2",
                            "typeId": "CCCCCCCC-CCCC-CCCC-CCCC-CCCCCCCCCCCC",
                            "is_base_class": false
                        }
                    ]
                }
            ]],
            "uuidAnyCreationMap": {},
            "editContext": {"$id": 2, "classData": [], "enumData": []},
            "enumTypeIdToUnderlyingTypeIdMap": {}
        }));

        let graph = SchemaGraph::from_model(&model);
        let generic_fields = graph
            .edges_from(pair_id)
            .filter_map(|edge| match &edge.kind {
                SchemaEdgeKind::GenericField { field_name } => Some((field_name.as_str(), edge.to)),
                _ => None,
            })
            .collect::<Vec<_>>();

        assert_eq!(
            generic_fields,
            vec![("value1", Some(key_id)), ("value2", Some(value_id))]
        );
    }
}
