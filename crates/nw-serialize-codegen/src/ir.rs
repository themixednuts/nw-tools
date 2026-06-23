use std::collections::{BTreeMap, BTreeSet};

use uuid::Uuid;

use crate::layout::LayoutIndex;
use crate::model::{ReflectedClass, ReflectedEnum, ReflectedMember, SerializeContextModel};
use crate::role::{ReflectedTypeRole, SerializeRoleClassifier};
use crate::types::{ResolvedType, TypeResolver};

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SerializeCodegenUnit {
    pub items: Vec<SerializeCodegenItem>,
}

impl SerializeCodegenUnit {
    #[must_use]
    pub fn item_by_source_type_id(&self, type_id: Uuid) -> Option<&SerializeCodegenItem> {
        self.items
            .iter()
            .find(|item| item.source_type_id == type_id)
    }

    #[must_use]
    pub fn index(&self) -> SerializeCodegenIndex<'_> {
        SerializeCodegenIndex::from_unit(self)
    }

    #[must_use]
    pub fn select(&self, selection: SerializeCodegenSelection) -> Self {
        match selection {
            SerializeCodegenSelection::All => self.clone(),
            SerializeCodegenSelection::Components => self.select_component_roots(),
            SerializeCodegenSelection::ComponentFamilies => self.select_component_families(),
            SerializeCodegenSelection::RuntimeRoots => self.select_runtime_roots(),
        }
    }

    #[must_use]
    pub(crate) fn select_exact_type_ids(&self, selected_type_ids: &BTreeSet<Uuid>) -> Self {
        Self {
            items: self
                .items
                .iter()
                .filter(|item| selected_type_ids.contains(&item.source_type_id))
                .cloned()
                .collect(),
        }
    }

    fn select_component_roots(&self) -> Self {
        let index = self.index();
        let selected_type_ids = index.component_root_type_ids();
        self.select_exact_type_ids(&selected_type_ids)
    }

    fn select_component_families(&self) -> Self {
        let index = self.index();
        let layout = LayoutIndex::from_codegen_unit(self);
        let selected_type_ids = index.component_family_type_ids(&layout);
        self.select_exact_type_ids(&selected_type_ids)
    }

    fn select_runtime_roots(&self) -> Self {
        let index = self.index();
        let selected_type_ids = index.runtime_root_type_ids();
        self.select_exact_type_ids(&selected_type_ids)
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum SerializeCodegenSelection {
    #[default]
    All,
    Components,
    ComponentFamilies,
    RuntimeRoots,
}

#[derive(Debug, Clone)]
pub struct SerializeCodegenIndex<'a> {
    items_by_type_id: BTreeMap<Uuid, &'a SerializeCodegenItem>,
}

impl<'a> SerializeCodegenIndex<'a> {
    #[must_use]
    pub fn from_unit(unit: &'a SerializeCodegenUnit) -> Self {
        Self {
            items_by_type_id: unit
                .items
                .iter()
                .map(|item| (item.source_type_id, item))
                .collect(),
        }
    }

    #[must_use]
    pub fn item_by_type_id(&self, type_id: Uuid) -> Option<&'a SerializeCodegenItem> {
        self.items_by_type_id.get(&type_id).copied()
    }

    #[must_use]
    pub fn contains_type_id(&self, type_id: Uuid) -> bool {
        self.items_by_type_id.contains_key(&type_id)
    }

    #[must_use]
    pub const fn items_by_type_id(&self) -> &BTreeMap<Uuid, &'a SerializeCodegenItem> {
        &self.items_by_type_id
    }

    #[must_use]
    pub fn known_direct_dependency_type_ids(&self, item: &SerializeCodegenItem) -> Vec<Uuid> {
        let mut dependencies = item
            .direct_dependency_type_ids()
            .into_iter()
            .filter(|type_id| self.contains_type_id(*type_id))
            .collect::<Vec<_>>();
        dependencies.sort_by(|left, right| {
            let left_item = self
                .item_by_type_id(*left)
                .expect("dependency was filtered through SerializeCodegenIndex");
            let right_item = self
                .item_by_type_id(*right)
                .expect("dependency was filtered through SerializeCodegenIndex");
            left_item
                .source_name
                .cmp(&right_item.source_name)
                .then_with(|| left.cmp(right))
        });
        dependencies
    }

    #[must_use]
    pub fn transitive_dependency_type_ids(&self, root: &SerializeCodegenItem) -> BTreeSet<Uuid> {
        let mut type_ids = BTreeSet::new();
        self.extend_transitive_dependency_type_ids(root, &mut type_ids);
        type_ids
    }

    pub fn extend_transitive_dependency_type_ids(
        &self,
        root: &SerializeCodegenItem,
        type_ids: &mut BTreeSet<Uuid>,
    ) {
        let mut stack = vec![root.source_type_id];
        while let Some(type_id) = stack.pop() {
            if !type_ids.insert(type_id) {
                continue;
            }
            let item = if type_id == root.source_type_id {
                Some(root)
            } else {
                self.item_by_type_id(type_id)
            };
            let Some(item) = item else {
                continue;
            };
            let mut dependencies = self.known_direct_dependency_type_ids(item);
            dependencies.reverse();
            stack.extend(dependencies);
        }
    }

    #[must_use]
    pub fn runtime_root_type_ids(&self) -> BTreeSet<Uuid> {
        let mut type_ids = BTreeSet::new();
        for item in self.items_by_type_id.values().copied().filter(|item| {
            !item.is_reflection_marker
                && (item.role.is_az_component_like() || item.role == ReflectedTypeRole::AzEntity)
        }) {
            self.extend_transitive_dependency_type_ids(item, &mut type_ids);
        }
        type_ids
    }

    #[must_use]
    pub fn component_root_type_ids(&self) -> BTreeSet<Uuid> {
        let mut type_ids = BTreeSet::new();
        for item in self
            .items_by_type_id
            .values()
            .copied()
            .filter(|item| !item.is_reflection_marker && item.role.is_component())
        {
            self.extend_transitive_dependency_type_ids(item, &mut type_ids);
        }
        type_ids
    }

    #[must_use]
    pub fn component_family_type_ids(&self, layout: &LayoutIndex) -> BTreeSet<Uuid> {
        let mut type_ids = BTreeSet::new();
        for item in self
            .items_by_type_id
            .values()
            .copied()
            .filter(|item| !item.is_reflection_marker && item.role.is_component())
        {
            self.extend_transitive_dependency_type_ids(item, &mut type_ids);
            for (child_type_id, binding) in &layout.concrete_slot_bindings {
                if binding.owner_type_id != item.source_type_id {
                    continue;
                }
                let Some(child) = self.item_by_type_id(*child_type_id) else {
                    continue;
                };
                self.extend_transitive_dependency_type_ids(child, &mut type_ids);
            }
        }
        type_ids
    }

    #[must_use]
    pub fn into_items_by_type_id(self) -> BTreeMap<Uuid, &'a SerializeCodegenItem> {
        self.items_by_type_id
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MissingReflectedType {
    pub owner_name: String,
    pub field_name: String,
    pub type_id: Uuid,
    pub reason: String,
    pub is_base_class: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SerializeCodegenItem {
    pub source_type_id: Uuid,
    pub source_name: String,
    pub role: ReflectedTypeRole,
    pub is_reflection_marker: bool,
    pub is_abstract: Option<bool>,
    pub factory: Option<String>,
    pub rtti_base_chain: Vec<SerializeCodegenRttiBase>,
    pub kind: SerializeCodegenItemKind,
    pub enum_underlying_type: Option<ResolvedType>,
    pub fields: Vec<SerializeCodegenField>,
    pub variants: Vec<SerializeCodegenVariant>,
}

impl SerializeCodegenItem {
    #[must_use]
    pub fn direct_dependency_type_ids(&self) -> BTreeSet<Uuid> {
        let mut type_ids = BTreeSet::new();
        type_ids.extend(self.rtti_base_chain.iter().map(|base| base.type_id));
        for field in &self.fields {
            collect_resolved_named_type_ids(&field.resolved_type, &mut type_ids);
        }
        if let Some(resolved) = &self.enum_underlying_type {
            collect_resolved_named_type_ids(resolved, &mut type_ids);
        }
        type_ids.remove(&self.source_type_id);
        type_ids
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SerializeCodegenRttiBase {
    pub type_id: Uuid,
    pub source_name: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SerializeCodegenItemKind {
    Struct,
    Enum,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SerializeCodegenField {
    pub source_name: String,
    pub source_type_id: Uuid,
    pub resolved_type: ResolvedType,
    pub data_size: Option<u32>,
    pub offset: Option<u32>,
    pub flags: Option<u32>,
    pub is_base_class: bool,
    pub is_pointer: bool,
    pub is_dynamic_field: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SerializeCodegenVariant {
    pub source_name: String,
    pub value_u64: Option<u64>,
    pub value_u32: Option<u32>,
    pub value_i32: Option<i32>,
}

pub fn collect_resolved_named_type_ids(resolved: &ResolvedType, out: &mut BTreeSet<Uuid>) {
    match resolved {
        ResolvedType::Named { type_id, .. } => {
            out.insert(*type_id);
        }
        ResolvedType::Sequence { element, .. }
        | ResolvedType::RangedInteger { value: element, .. }
        | ResolvedType::Pointer {
            target: element, ..
        }
        | ResolvedType::Optional { value: element }
        | ResolvedType::ReplicatedField { value: element } => {
            collect_resolved_named_type_ids(element, out);
        }
        ResolvedType::Map { key, value, .. }
        | ResolvedType::Pair {
            first: key,
            second: value,
        } => {
            collect_resolved_named_type_ids(key, out);
            collect_resolved_named_type_ids(value, out);
        }
        ResolvedType::Tuple { elements } => {
            for element in elements {
                collect_resolved_named_type_ids(element, out);
            }
        }
        ResolvedType::Scalar(_)
        | ResolvedType::Asset { .. }
        | ResolvedType::Uid { .. }
        | ResolvedType::ByteStream
        | ResolvedType::Unknown { .. } => {}
    }
}

#[derive(Debug, Default)]
pub struct SerializeCodegenPlanner;

impl SerializeCodegenPlanner {
    #[must_use]
    pub fn plan_model(model: &SerializeContextModel) -> SerializeCodegenUnit {
        Self.plan(model)
    }

    #[must_use]
    pub fn plan(&self, model: &SerializeContextModel) -> SerializeCodegenUnit {
        let role_classifier = SerializeRoleClassifier::from_model(model);
        let resolver = TypeResolver::new(model);
        let mut items =
            model
                .classes
                .values()
                .filter(|class| class.type_id.is_nil() || !model.enums.contains_key(&class.type_id))
                .map(|class| self.plan_class(class, model, &role_classifier, &resolver))
                .chain(
                    model
                        .enums
                        .values()
                        .map(|enumeration| self.plan_enum(enumeration, model, &resolver)),
                )
                .chain(model.bodyless_rtti_types().into_values().map(|bodyless| {
                    SerializeCodegenItem {
                        source_type_id: bodyless.type_id,
                        source_name: bodyless.name,
                        role: ReflectedTypeRole::SupportType,
                        is_reflection_marker: false,
                        is_abstract: bodyless.is_abstract,
                        factory: None,
                        rtti_base_chain: Vec::new(),
                        kind: SerializeCodegenItemKind::Struct,
                        enum_underlying_type: None,
                        fields: Vec::new(),
                        variants: Vec::new(),
                    }
                }))
                .collect::<Vec<_>>();
        items.sort_by(|left, right| {
            left.source_name
                .cmp(&right.source_name)
                .then_with(|| left.source_type_id.cmp(&right.source_type_id))
        });
        SerializeCodegenUnit { items }
    }

    fn plan_class(
        &self,
        class: &ReflectedClass,
        model: &SerializeContextModel,
        role_classifier: &SerializeRoleClassifier,
        resolver: &TypeResolver<'_>,
    ) -> SerializeCodegenItem {
        SerializeCodegenItem {
            source_type_id: class.type_id,
            source_name: class.name.clone(),
            role: role_classifier.classify(class.type_id),
            is_reflection_marker: role_classifier.is_reflection_marker(class.type_id),
            is_abstract: class.az_rtti.as_ref().and_then(|rtti| rtti.is_abstract),
            factory: class.factory.clone(),
            rtti_base_chain: rtti_base_chain(class, model),
            kind: SerializeCodegenItemKind::Struct,
            enum_underlying_type: None,
            fields: class
                .members
                .iter()
                .map(|member| self.plan_field(member, resolver))
                .collect(),
            variants: Vec::new(),
        }
    }

    fn plan_field(
        &self,
        member: &ReflectedMember,
        resolver: &TypeResolver<'_>,
    ) -> SerializeCodegenField {
        let resolved_type = resolver.resolve_member_type(member);
        SerializeCodegenField {
            source_name: member.name.clone(),
            source_type_id: member.type_id,
            resolved_type,
            data_size: member.data_size,
            offset: member.offset,
            flags: member.flags,
            is_base_class: member.is_base_class,
            is_pointer: member.is_pointer,
            is_dynamic_field: member.is_dynamic_field,
        }
    }

    fn plan_enum(
        &self,
        enumeration: &ReflectedEnum,
        model: &SerializeContextModel,
        resolver: &TypeResolver<'_>,
    ) -> SerializeCodegenItem {
        SerializeCodegenItem {
            source_type_id: enumeration.type_id,
            source_name: enumeration.name.clone(),
            role: ReflectedTypeRole::SupportType,
            is_reflection_marker: false,
            is_abstract: Some(false),
            factory: None,
            rtti_base_chain: Vec::new(),
            kind: SerializeCodegenItemKind::Enum,
            enum_underlying_type: model
                .enum_underlying_types
                .get(&enumeration.type_id)
                .map(|type_id| resolver.resolve(*type_id)),
            fields: Vec::new(),
            variants: enumeration
                .variants
                .iter()
                .map(|variant| SerializeCodegenVariant {
                    source_name: variant.name.clone(),
                    value_u64: variant.value_u64,
                    value_u32: variant.value_u32,
                    value_i32: variant.value_i32,
                })
                .collect(),
        }
    }
}

fn rtti_base_chain(
    class: &ReflectedClass,
    model: &SerializeContextModel,
) -> Vec<SerializeCodegenRttiBase> {
    let Some(rtti) = &class.az_rtti else {
        return Vec::new();
    };
    if rtti.type_id != Some(class.type_id) {
        return Vec::new();
    }
    let mut bases = rtti
        .hierarchy
        .iter()
        .enumerate()
        .filter_map(|(index, entry)| {
            if index == 0 && entry.type_id == class.type_id {
                return None;
            }
            let source_name = entry
                .type_name
                .clone()
                .or_else(|| model.type_name(entry.type_id).map(str::to_owned))?;
            Some(SerializeCodegenRttiBase {
                type_id: entry.type_id,
                source_name,
            })
        })
        .collect::<Vec<_>>();
    bases.reverse();
    bases
}

#[cfg(test)]
mod tests {
    use nw_objectstream::type_uuid::type_ids;
    use serde_json::json;
    use uuid::uuid;

    use crate::model::SerializeContextModel;

    use super::*;

    #[test]
    fn plans_structs_enums_roles_and_resolved_field_types() {
        let model = SerializeContextModel::from_root(&json!({
            "$id": 1,
            "uuidMap": {
                "AAAAAAAA-AAAA-AAAA-AAAA-AAAAAAAAAAAA": {
                    "$id": 10,
                    "name": "Component",
                    "typeId": "AAAAAAAA-AAAA-AAAA-AAAA-AAAAAAAAAAAA",
                    "elements": [],
                    "attributes": []
                },
                "11111111-1111-1111-1111-111111111111": {
                    "$id": 20,
                    "name": "Example::CounterComponent",
                    "typeId": "11111111-1111-1111-1111-111111111111",
                    "elements": [
                        {
                            "$id": 21,
                            "name": "BaseClass1",
                            "typeId": "AAAAAAAA-AAAA-AAAA-AAAA-AAAAAAAAAAAA",
                            "is_base_class": true
                        },
                        {
                            "$id": 22,
                            "name": "m_count",
                            "typeId": "43DA906B-7DEF-4CA8-9790-854106D3F983",
                            "offset": "4",
                            "flags": 0,
                            "is_base_class": false
                        },
                        {
                            "$id": 23,
                            "name": "m_mode",
                            "typeId": type_ids::INT.hyphenated().to_string(),
                            "offset": "8",
                            "flags": 0,
                            "is_base_class": false,
                            "attributes": [[2, {
                                "$id": 24,
                                "attributeId": 2,
                                "attributeName": "EnumType",
                                "value": {
                                    "kind": "Uuid",
                                    "value": "22222222-2222-2222-2222-222222222222"
                                }
                            }]]
                        }
                    ],
                    "attributes": []
                }
            },
            "classNameToUuid": [],
            "uuidGenericMap": [],
            "uuidAnyCreationMap": {},
            "editContext": {
                "$id": 2,
                "classData": [],
                "enumData": [[
                    "22222222-2222-2222-2222-222222222222",
                    {
                        "$id": 30,
                        "name": "Mode",
                        "attributes": [[1, {
                            "$id": 31,
                            "attributeId": 1,
                            "attributeName": "EnumValue",
                            "value": {
                                "kind": "enumConstant",
                                "valueU64": "0x7",
                                "valueU32": 7,
                                "valueI32": 7,
                                "description": "Enabled"
                            }
                        }]]
                    }
                ]]
            },
            "enumTypeIdToUnderlyingTypeIdMap": {
                "22222222-2222-2222-2222-222222222222": type_ids::U8.hyphenated().to_string()
            }
        }));

        let unit = SerializeCodegenPlanner::plan_model(&model);
        let component = unit
            .item_by_source_type_id(uuid!("11111111-1111-1111-1111-111111111111"))
            .expect("component item");
        let mode = unit
            .item_by_source_type_id(uuid!("22222222-2222-2222-2222-222222222222"))
            .expect("enum item");

        assert_eq!(component.role, ReflectedTypeRole::AzComponent);
        assert_eq!(component.fields.len(), 3);
        assert!(component.fields[0].is_base_class);
        assert_eq!(
            component.fields[1].resolved_type,
            ResolvedType::Scalar(crate::types::ScalarType::U32)
        );
        assert_eq!(
            component.fields[2].resolved_type,
            ResolvedType::Named {
                type_id: uuid!("22222222-2222-2222-2222-222222222222"),
                source_name: "Mode".to_owned(),
            }
        );
        assert_eq!(
            mode.enum_underlying_type,
            Some(ResolvedType::Scalar(crate::types::ScalarType::U8))
        );
        assert_eq!(mode.variants[0].source_name, "Enabled");
    }

    #[test]
    fn runtime_root_selection_keeps_components_entities_and_dependency_closure() {
        let model = SerializeContextModel::from_root(&json!({
            "$id": 1,
            "uuidMap": {
                "AAAAAAAA-AAAA-AAAA-AAAA-AAAAAAAAAAAA": {
                    "$id": 10,
                    "name": "Component",
                    "typeId": "AAAAAAAA-AAAA-AAAA-AAAA-AAAAAAAAAAAA",
                    "elements": [],
                    "attributes": []
                },
                "BBBBBBBB-BBBB-BBBB-BBBB-BBBBBBBBBBBB": {
                    "$id": 20,
                    "name": "Example::TargetComponent",
                    "typeId": "BBBBBBBB-BBBB-BBBB-BBBB-BBBBBBBBBBBB",
                    "elements": [
                        {
                            "$id": 21,
                            "name": "BaseClass1",
                            "typeId": "AAAAAAAA-AAAA-AAAA-AAAA-AAAAAAAAAAAA",
                            "is_base_class": true
                        },
                        {
                            "$id": 22,
                            "name": "m_support",
                            "typeId": "CCCCCCCC-CCCC-CCCC-CCCC-CCCCCCCCCCCC",
                            "is_base_class": false
                        }
                    ],
                    "attributes": []
                },
                "CCCCCCCC-CCCC-CCCC-CCCC-CCCCCCCCCCCC": {
                    "$id": 30,
                    "name": "Example::Support",
                    "typeId": "CCCCCCCC-CCCC-CCCC-CCCC-CCCCCCCCCCCC",
                    "elements": [{
                        "$id": 31,
                        "name": "m_leaf",
                        "typeId": "DDDDDDDD-DDDD-DDDD-DDDD-DDDDDDDDDDDD",
                        "is_base_class": false
                    }],
                    "attributes": []
                },
                "DDDDDDDD-DDDD-DDDD-DDDD-DDDDDDDDDDDD": {
                    "$id": 40,
                    "name": "Example::Leaf",
                    "typeId": "DDDDDDDD-DDDD-DDDD-DDDD-DDDDDDDDDDDD",
                    "elements": [],
                    "attributes": []
                },
                "EEEEEEEE-EEEE-EEEE-EEEE-EEEEEEEEEEEE": {
                    "$id": 50,
                    "name": "Example::Unrelated",
                    "typeId": "EEEEEEEE-EEEE-EEEE-EEEE-EEEEEEEEEEEE",
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
        let unit = SerializeCodegenPlanner::plan_model(&model);

        let selected = unit.select(SerializeCodegenSelection::RuntimeRoots);
        let selected_ids = selected
            .items
            .iter()
            .map(|item| item.source_type_id)
            .collect::<BTreeSet<_>>();

        assert!(selected_ids.contains(&uuid!("AAAAAAAA-AAAA-AAAA-AAAA-AAAAAAAAAAAA")));
        assert!(selected_ids.contains(&uuid!("BBBBBBBB-BBBB-BBBB-BBBB-BBBBBBBBBBBB")));
        assert!(selected_ids.contains(&uuid!("CCCCCCCC-CCCC-CCCC-CCCC-CCCCCCCCCCCC")));
        assert!(selected_ids.contains(&uuid!("DDDDDDDD-DDDD-DDDD-DDDD-DDDDDDDDDDDD")));
        assert!(!selected_ids.contains(&uuid!("EEEEEEEE-EEEE-EEEE-EEEE-EEEEEEEEEEEE")));
    }

    #[test]
    fn component_selection_roots_components_without_unreferenced_facets_or_entities() {
        let component_id = uuid!("11111111-1111-1111-1111-111111111111");
        let support_id = uuid!("22222222-2222-2222-2222-222222222222");
        let client_facet_id = uuid!("33333333-3333-3333-3333-333333333333");
        let entity_id = uuid!("44444444-4444-4444-4444-444444444444");
        let unit = SerializeCodegenUnit {
            items: vec![
                SerializeCodegenItem {
                    source_type_id: component_id,
                    source_name: "Example::RuntimeComponent".to_owned(),
                    role: ReflectedTypeRole::AzComponent,
                    is_reflection_marker: false,
                    is_abstract: Some(false),
                    factory: None,
                    rtti_base_chain: Vec::new(),
                    kind: SerializeCodegenItemKind::Struct,
                    enum_underlying_type: None,
                    fields: vec![named_field(
                        "m_support",
                        support_id,
                        "Example::ComponentSupport",
                    )],
                    variants: Vec::new(),
                },
                SerializeCodegenItem {
                    source_type_id: support_id,
                    source_name: "Example::ComponentSupport".to_owned(),
                    role: ReflectedTypeRole::SupportType,
                    is_reflection_marker: false,
                    is_abstract: Some(false),
                    factory: None,
                    rtti_base_chain: Vec::new(),
                    kind: SerializeCodegenItemKind::Struct,
                    enum_underlying_type: None,
                    fields: Vec::new(),
                    variants: Vec::new(),
                },
                SerializeCodegenItem {
                    source_type_id: client_facet_id,
                    source_name: "Example::ClientFacet".to_owned(),
                    role: ReflectedTypeRole::ClientFacet,
                    is_reflection_marker: false,
                    is_abstract: Some(false),
                    factory: None,
                    rtti_base_chain: Vec::new(),
                    kind: SerializeCodegenItemKind::Struct,
                    enum_underlying_type: None,
                    fields: Vec::new(),
                    variants: Vec::new(),
                },
                SerializeCodegenItem {
                    source_type_id: entity_id,
                    source_name: "AZ::Entity".to_owned(),
                    role: ReflectedTypeRole::AzEntity,
                    is_reflection_marker: false,
                    is_abstract: Some(false),
                    factory: None,
                    rtti_base_chain: Vec::new(),
                    kind: SerializeCodegenItemKind::Struct,
                    enum_underlying_type: None,
                    fields: Vec::new(),
                    variants: Vec::new(),
                },
            ],
        };

        let selected = unit.select(SerializeCodegenSelection::Components);
        let selected_ids = selected
            .items
            .iter()
            .map(|item| item.source_type_id)
            .collect::<BTreeSet<_>>();

        assert_eq!(selected_ids, BTreeSet::from([component_id, support_id]));
    }

    #[test]
    fn component_family_selection_keeps_concrete_facets_without_unrelated_runtime_roots() {
        let faceted_component_id = uuid!("11111111-1111-1111-1111-111111111111");
        let client_facet_marker_id = uuid!("22222222-2222-2222-2222-222222222222");
        let server_facet_marker_id = uuid!("33333333-3333-3333-3333-333333333333");
        let component_id = uuid!("44444444-4444-4444-4444-444444444444");
        let component_support_id = uuid!("55555555-5555-5555-5555-555555555555");
        let client_facet_id = uuid!("66666666-6666-6666-6666-666666666666");
        let server_facet_id = uuid!("77777777-7777-7777-7777-777777777777");
        let facet_support_id = uuid!("88888888-8888-8888-8888-888888888888");
        let unrelated_facet_id = uuid!("99999999-9999-9999-9999-999999999999");
        let entity_id = uuid!("AAAAAAAA-AAAA-AAAA-AAAA-AAAAAAAAAAAA");
        let unit = SerializeCodegenUnit {
            items: vec![
                struct_item(
                    faceted_component_id,
                    "FacetedComponent",
                    ReflectedTypeRole::FacetedComponent,
                    vec![
                        pointer_field("m_clientFacetPtr", client_facet_marker_id, "ClientFacet"),
                        pointer_field("m_serverFacetPtr", server_facet_marker_id, "ServerFacet"),
                    ],
                ),
                struct_item(
                    client_facet_marker_id,
                    "ClientFacet",
                    ReflectedTypeRole::ClientFacet,
                    Vec::new(),
                ),
                struct_item(
                    server_facet_marker_id,
                    "ServerFacet",
                    ReflectedTypeRole::ServerFacet,
                    Vec::new(),
                ),
                struct_item(
                    component_id,
                    "RuntimeComponent",
                    ReflectedTypeRole::FacetedComponent,
                    vec![
                        base_field(faceted_component_id, "FacetedComponent"),
                        named_field("m_support", component_support_id, "RuntimeComponentSupport"),
                    ],
                ),
                struct_item(
                    component_support_id,
                    "RuntimeComponentSupport",
                    ReflectedTypeRole::SupportType,
                    Vec::new(),
                ),
                struct_item(
                    client_facet_id,
                    "RuntimeComponentClientFacet",
                    ReflectedTypeRole::ClientFacet,
                    vec![
                        base_field(client_facet_marker_id, "ClientFacet"),
                        named_field("m_payload", facet_support_id, "RuntimeFacetSupport"),
                    ],
                ),
                struct_item(
                    server_facet_id,
                    "RuntimeComponentServerFacet",
                    ReflectedTypeRole::ServerFacet,
                    vec![base_field(server_facet_marker_id, "ServerFacet")],
                ),
                struct_item(
                    facet_support_id,
                    "RuntimeFacetSupport",
                    ReflectedTypeRole::SupportType,
                    Vec::new(),
                ),
                struct_item(
                    unrelated_facet_id,
                    "OtherComponentClientFacet",
                    ReflectedTypeRole::ClientFacet,
                    vec![base_field(client_facet_marker_id, "ClientFacet")],
                ),
                struct_item(
                    entity_id,
                    "AZ::Entity",
                    ReflectedTypeRole::AzEntity,
                    Vec::new(),
                ),
            ],
        };

        let selected = unit.select(SerializeCodegenSelection::ComponentFamilies);
        let selected_ids = selected
            .items
            .iter()
            .map(|item| item.source_type_id)
            .collect::<BTreeSet<_>>();

        assert_eq!(
            selected_ids,
            BTreeSet::from([
                faceted_component_id,
                client_facet_marker_id,
                server_facet_marker_id,
                component_id,
                component_support_id,
                client_facet_id,
                server_facet_id,
                facet_support_id,
            ])
        );
        assert!(!selected_ids.contains(&unrelated_facet_id));
        assert!(!selected_ids.contains(&entity_id));
    }

    #[test]
    fn runtime_root_selection_keeps_rtti_base_chain_dependencies() {
        let base_id = uuid!("AAAAAAAA-AAAA-AAAA-AAAA-AAAAAAAAAAAA");
        let component_id = uuid!("BBBBBBBB-BBBB-BBBB-BBBB-BBBBBBBBBBBB");
        let unrelated_id = uuid!("CCCCCCCC-CCCC-CCCC-CCCC-CCCCCCCCCCCC");
        let unit = SerializeCodegenUnit {
            items: vec![
                SerializeCodegenItem {
                    source_type_id: base_id,
                    source_name: "Example::AbstractRuntimeBase".to_owned(),
                    role: ReflectedTypeRole::SupportType,
                    is_reflection_marker: false,
                    is_abstract: Some(true),
                    factory: None,
                    rtti_base_chain: Vec::new(),
                    kind: SerializeCodegenItemKind::Struct,
                    enum_underlying_type: None,
                    fields: Vec::new(),
                    variants: Vec::new(),
                },
                SerializeCodegenItem {
                    source_type_id: component_id,
                    source_name: "Example::RuntimeComponent".to_owned(),
                    role: ReflectedTypeRole::AzComponent,
                    is_reflection_marker: false,
                    is_abstract: Some(false),
                    factory: None,
                    rtti_base_chain: vec![SerializeCodegenRttiBase {
                        type_id: base_id,
                        source_name: "Example::AbstractRuntimeBase".to_owned(),
                    }],
                    kind: SerializeCodegenItemKind::Struct,
                    enum_underlying_type: None,
                    fields: Vec::new(),
                    variants: Vec::new(),
                },
                SerializeCodegenItem {
                    source_type_id: unrelated_id,
                    source_name: "Example::Unrelated".to_owned(),
                    role: ReflectedTypeRole::SupportType,
                    is_reflection_marker: false,
                    is_abstract: Some(false),
                    factory: None,
                    rtti_base_chain: Vec::new(),
                    kind: SerializeCodegenItemKind::Struct,
                    enum_underlying_type: None,
                    fields: Vec::new(),
                    variants: Vec::new(),
                },
            ],
        };

        let selected = unit.select(SerializeCodegenSelection::RuntimeRoots);
        let selected_ids = selected
            .items
            .iter()
            .map(|item| item.source_type_id)
            .collect::<BTreeSet<_>>();

        assert!(selected_ids.contains(&base_id));
        assert!(selected_ids.contains(&component_id));
        assert!(!selected_ids.contains(&unrelated_id));
    }

    #[test]
    fn plans_rtti_base_chain_from_az_rtti_hierarchy() {
        let model = SerializeContextModel::from_root(&json!({
            "$id": 1,
            "uuidMap": {
                "AAAAAAAA-AAAA-AAAA-AAAA-AAAAAAAAAAAA": {
                    "$id": 10,
                    "name": "AZ::Entity",
                    "typeId": "AAAAAAAA-AAAA-AAAA-AAAA-AAAAAAAAAAAA",
                    "azRtti": {
                        "typeId": "AAAAAAAA-AAAA-AAAA-AAAA-AAAAAAAAAAAA",
                        "typeName": "AZ::Entity",
                        "hierarchy": [{
                            "typeId": "AAAAAAAA-AAAA-AAAA-AAAA-AAAAAAAAAAAA",
                            "typeName": "AZ::Entity"
                        }],
                        "isAbstract": false
                    },
                    "elements": [],
                    "attributes": []
                },
                "BBBBBBBB-BBBB-BBBB-BBBB-BBBBBBBBBBBB": {
                    "$id": 20,
                    "name": "ModuleEntity",
                    "typeId": "BBBBBBBB-BBBB-BBBB-BBBB-BBBBBBBBBBBB",
                    "azRtti": {
                        "typeId": "BBBBBBBB-BBBB-BBBB-BBBB-BBBBBBBBBBBB",
                        "typeName": "ModuleEntity",
                        "hierarchy": [{
                            "typeId": "BBBBBBBB-BBBB-BBBB-BBBB-BBBBBBBBBBBB",
                            "typeName": "ModuleEntity"
                        }, {
                            "typeId": "AAAAAAAA-AAAA-AAAA-AAAA-AAAAAAAAAAAA"
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

        let unit = SerializeCodegenPlanner::plan_model(&model);
        let module_entity = unit
            .item_by_source_type_id(uuid!("BBBBBBBB-BBBB-BBBB-BBBB-BBBBBBBBBBBB"))
            .expect("ModuleEntity item");

        assert_eq!(
            module_entity.rtti_base_chain,
            vec![SerializeCodegenRttiBase {
                type_id: uuid!("AAAAAAAA-AAAA-AAAA-AAAA-AAAAAAAAAAAA"),
                source_name: "AZ::Entity".to_owned(),
            }]
        );
    }

    #[test]
    fn plans_bodyless_abstract_rtti_references_as_support_items() {
        let interface_id = uuid!("A60F95F5-5A4A-47DB-B3BB-525BBC0BC8DB");
        let owner_id = uuid!("760D45C1-08F2-4C70-A506-BD2E69085A48");
        let vector_id = uuid!("77D6D452-28F5-5F33-AA77-54678B6C6C7E");
        let pointer_id = uuid!("11111111-1111-1111-1111-111111111111");
        let model = SerializeContextModel::from_root(&json!({
            "$id": 1,
            "uuidMap": {
                "760D45C1-08F2-4C70-A506-BD2E69085A48": {
                    "$id": 20,
                    "name": "CMovieSystem",
                    "typeId": owner_id.hyphenated().to_string(),
                    "azRtti": {
                        "typeId": owner_id.hyphenated().to_string(),
                        "typeName": "CMovieSystem",
                        "hierarchy": [{
                            "typeId": owner_id.hyphenated().to_string(),
                            "typeName": "CMovieSystem"
                        }],
                        "isAbstract": false
                    },
                    "elements": [{
                        "$id": 21,
                        "name": "Sequences",
                        "typeId": vector_id.hyphenated().to_string(),
                        "is_base_class": false,
                        "genericClassInfo": {
                            "$id": 30,
                            "typeId": vector_id.hyphenated().to_string(),
                            "registeredTypeIds": [vector_id.hyphenated().to_string()],
                            "templatedTypeIds": [pointer_id.hyphenated().to_string()],
                            "classData": {
                                "$id": 31,
                                "name": "AZStd::vector",
                                "typeId": vector_id.hyphenated().to_string(),
                                "elements": [],
                                "attributes": []
                            },
                            "elements": [{
                                "$id": 32,
                                "name": "element",
                                "typeId": pointer_id.hyphenated().to_string(),
                                "is_base_class": false,
                                "genericClassInfo": {
                                    "$id": 40,
                                    "typeId": pointer_id.hyphenated().to_string(),
                                    "registeredTypeIds": [pointer_id.hyphenated().to_string()],
                                    "templatedTypeIds": [interface_id.hyphenated().to_string()],
                                    "classData": {
                                        "$id": 41,
                                        "name": "AZStd::intrusive_ptr",
                                        "typeId": pointer_id.hyphenated().to_string(),
                                        "elements": [],
                                        "attributes": []
                                    },
                                    "elements": [{
                                        "$id": 42,
                                        "name": "element",
                                        "typeId": interface_id.hyphenated().to_string(),
                                        "is_base_class": false,
                                        "azRtti": {
                                            "typeId": interface_id.hyphenated().to_string(),
                                            "typeName": "IAnimSequence",
                                            "hierarchy": [{
                                                "typeId": interface_id.hyphenated().to_string(),
                                                "typeName": "IAnimSequence"
                                            }],
                                            "isAbstract": true
                                        },
                                        "attributes": []
                                    }],
                                    "attributes": []
                                },
                                "attributes": []
                            }],
                            "attributes": []
                        },
                        "attributes": []
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

        let unit = SerializeCodegenPlanner::plan_model(&model);
        let interface = unit
            .item_by_source_type_id(interface_id)
            .expect("bodyless IAnimSequence item");
        let owner = unit
            .item_by_source_type_id(owner_id)
            .expect("CMovieSystem item");

        assert_eq!(interface.source_name, "IAnimSequence");
        assert_eq!(interface.is_abstract, Some(true));
        assert!(interface.fields.is_empty());
        assert_eq!(
            owner.fields[0].resolved_type,
            ResolvedType::Sequence {
                kind: crate::types::SequenceKind::Vector,
                element: Box::new(ResolvedType::Pointer {
                    kind: crate::types::PointerKind::Intrusive,
                    target: Box::new(ResolvedType::Named {
                        type_id: interface_id,
                        source_name: "IAnimSequence".to_owned(),
                    }),
                }),
                capacity: None,
            }
        );
    }

    #[test]
    fn plans_bodyless_base_edges_as_support_items() {
        let base_id = uuid!("D86C82E1-E027-453F-A43B-BD801CF88391");
        let derived_id = uuid!("D7978A94-592F-4E1A-86EF-E34A819A55FB");
        let model = SerializeContextModel::from_root(&json!({
            "$id": 1,
            "uuidMap": {
                "D7978A94-592F-4E1A-86EF-E34A819A55FB": {
                    "$id": 20,
                    "name": "UiInteractableStateColor",
                    "typeId": derived_id.hyphenated().to_string(),
                    "azRtti": {
                        "typeId": derived_id.hyphenated().to_string(),
                        "typeName": "UiInteractableStateColor",
                        "hierarchy": [{
                            "typeId": derived_id.hyphenated().to_string(),
                            "typeName": "UiInteractableStateColor"
                        }, {
                            "typeId": base_id.hyphenated().to_string(),
                            "typeName": "UiInteractableStateAction"
                        }],
                        "isAbstract": false
                    },
                    "elements": [{
                        "$id": 21,
                        "name": "BaseClass1",
                        "typeId": base_id.hyphenated().to_string(),
                        "is_base_class": true,
                        "azRtti": {
                            "typeId": base_id.hyphenated().to_string(),
                            "typeName": "UiInteractableStateAction",
                            "hierarchy": [{
                                "typeId": base_id.hyphenated().to_string(),
                                "typeName": "UiInteractableStateAction"
                            }],
                            "isAbstract": true
                        },
                        "attributes": []
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

        let unit = SerializeCodegenPlanner::plan_model(&model);
        let base = unit
            .item_by_source_type_id(base_id)
            .expect("bodyless base item");
        let derived = unit
            .item_by_source_type_id(derived_id)
            .expect("derived item");

        assert_eq!(base.source_name, "UiInteractableStateAction");
        assert_eq!(base.is_abstract, Some(true));
        assert!(base.fields.is_empty());
        assert_eq!(
            derived.fields[0].resolved_type,
            ResolvedType::Named {
                type_id: base_id,
                source_name: "UiInteractableStateAction".to_owned()
            }
        );
    }

    #[test]
    fn ignores_rtti_hierarchy_when_rtti_type_id_is_not_the_serialized_class() {
        let model = SerializeContextModel::from_root(&json!({
            "$id": 1,
            "uuidMap": {
                "AAAAAAAA-AAAA-AAAA-AAAA-AAAAAAAAAAAA": {
                    "$id": 10,
                    "name": "LocalComponentRefBase",
                    "typeId": "AAAAAAAA-AAAA-AAAA-AAAA-AAAAAAAAAAAA",
                    "azRtti": {
                        "typeId": "AAAAAAAA-AAAA-AAAA-AAAA-AAAAAAAAAAAA",
                        "typeName": "LocalComponentRefBase",
                        "hierarchy": [{
                            "typeId": "AAAAAAAA-AAAA-AAAA-AAAA-AAAAAAAAAAAA",
                            "typeName": "LocalComponentRefBase"
                        }],
                        "isAbstract": false
                    },
                    "elements": [],
                    "attributes": []
                },
                "BBBBBBBB-BBBB-BBBB-BBBB-BBBBBBBBBBBB": {
                    "$id": 20,
                    "name": "UnstuckComponentClientFacet",
                    "typeId": "BBBBBBBB-BBBB-BBBB-BBBB-BBBBBBBBBBBB",
                    "elements": [],
                    "attributes": []
                },
                "CCCCCCCC-CCCC-CCCC-CCCC-CCCCCCCCCCCC": {
                    "$id": 30,
                    "name": "LocalComponentRef<InterfaceType><const char *__cdecl MB::GetTypeName<class Javelin::MagicComponent>(void)>",
                    "typeId": "CCCCCCCC-CCCC-CCCC-CCCC-CCCCCCCCCCCC",
                    "azRtti": {
                        "typeId": "DDDDDDDD-DDDD-DDDD-DDDD-DDDDDDDDDDDD",
                        "typeName": null,
                        "hierarchy": [{
                            "typeId": "DDDDDDDD-DDDD-DDDD-DDDD-DDDDDDDDDDDD",
                            "typeName": null
                        }, {
                            "typeId": "AAAAAAAA-AAAA-AAAA-AAAA-AAAAAAAAAAAA",
                            "typeName": "LocalComponentRefBase"
                        }, {
                            "typeId": "BBBBBBBB-BBBB-BBBB-BBBB-BBBBBBBBBBBB",
                            "typeName": "UnstuckComponentClientFacet"
                        }],
                        "isAbstract": false
                    },
                    "elements": [{
                        "$id": 31,
                        "name": "BaseClass1",
                        "typeId": "AAAAAAAA-AAAA-AAAA-AAAA-AAAAAAAAAAAA",
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

        let unit = SerializeCodegenPlanner::plan_model(&model);
        let item = unit
            .item_by_source_type_id(uuid!("CCCCCCCC-CCCC-CCCC-CCCC-CCCCCCCCCCCC"))
            .expect("LocalComponentRef item");

        assert!(item.rtti_base_chain.is_empty());
        assert_eq!(item.role, ReflectedTypeRole::SupportType);
    }

    #[test]
    fn skips_unnamed_unreflected_rtti_bases() {
        let model = SerializeContextModel::from_root(&json!({
            "$id": 1,
            "uuidMap": {
                "BBBBBBBB-BBBB-BBBB-BBBB-BBBBBBBBBBBB": {
                    "$id": 20,
                    "name": "BoxShapeConfig",
                    "typeId": "BBBBBBBB-BBBB-BBBB-BBBB-BBBBBBBBBBBB",
                    "azRtti": {
                        "typeId": "BBBBBBBB-BBBB-BBBB-BBBB-BBBBBBBBBBBB",
                        "typeName": "BoxShapeConfig",
                        "hierarchy": [{
                            "typeId": "BBBBBBBB-BBBB-BBBB-BBBB-BBBBBBBBBBBB",
                            "typeName": "BoxShapeConfig"
                        }, {
                            "typeId": "AAAAAAAA-AAAA-AAAA-AAAA-AAAAAAAAAAAA",
                            "typeName": null
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

        let unit = SerializeCodegenPlanner::plan_model(&model);
        let item = unit
            .item_by_source_type_id(uuid!("BBBBBBBB-BBBB-BBBB-BBBB-BBBBBBBBBBBB"))
            .expect("BoxShapeConfig item");

        assert!(item.rtti_base_chain.is_empty());
    }

    #[test]
    fn all_selection_keeps_everything() {
        let unit = SerializeCodegenUnit {
            items: vec![SerializeCodegenItem {
                source_type_id: uuid!("11111111-1111-1111-1111-111111111111"),
                source_name: "Example::Support".to_owned(),
                role: ReflectedTypeRole::SupportType,
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

        assert_eq!(unit.select(SerializeCodegenSelection::All), unit);
    }

    #[test]
    fn index_resolves_items_by_type_id_without_rewalking_callers() {
        let support_id = uuid!("11111111-1111-1111-1111-111111111111");
        let component_id = uuid!("22222222-2222-2222-2222-222222222222");
        let unit = SerializeCodegenUnit {
            items: vec![
                SerializeCodegenItem {
                    source_type_id: support_id,
                    source_name: "Example::Support".to_owned(),
                    role: ReflectedTypeRole::SupportType,
                    is_reflection_marker: false,
                    is_abstract: Some(false),
                    factory: None,
                    rtti_base_chain: Vec::new(),
                    kind: SerializeCodegenItemKind::Struct,
                    enum_underlying_type: None,
                    fields: Vec::new(),
                    variants: Vec::new(),
                },
                SerializeCodegenItem {
                    source_type_id: component_id,
                    source_name: "Example::Component".to_owned(),
                    role: ReflectedTypeRole::AzComponent,
                    is_reflection_marker: false,
                    is_abstract: Some(false),
                    factory: None,
                    rtti_base_chain: Vec::new(),
                    kind: SerializeCodegenItemKind::Struct,
                    enum_underlying_type: None,
                    fields: Vec::new(),
                    variants: Vec::new(),
                },
            ],
        };

        let index = unit.index();

        assert_eq!(
            index
                .item_by_type_id(component_id)
                .map(|item| item.source_name.as_str()),
            Some("Example::Component")
        );
        assert!(index.contains_type_id(support_id));
        assert!(!index.contains_type_id(uuid!("33333333-3333-3333-3333-333333333333")));
        assert_eq!(index.items_by_type_id().len(), 2);
        assert_eq!(index.into_items_by_type_id().len(), 2);
    }

    #[test]
    fn index_filters_sorts_and_closes_dependencies_without_recursing() {
        let component_id = uuid!("11111111-1111-1111-1111-111111111111");
        let alpha_id = uuid!("22222222-2222-2222-2222-222222222222");
        let beta_id = uuid!("33333333-3333-3333-3333-333333333333");
        let missing_id = uuid!("44444444-4444-4444-4444-444444444444");
        let unit = SerializeCodegenUnit {
            items: vec![
                SerializeCodegenItem {
                    source_type_id: component_id,
                    source_name: "Example::RuntimeComponent".to_owned(),
                    role: ReflectedTypeRole::AzComponent,
                    is_reflection_marker: false,
                    is_abstract: Some(false),
                    factory: None,
                    rtti_base_chain: Vec::new(),
                    kind: SerializeCodegenItemKind::Struct,
                    enum_underlying_type: None,
                    fields: vec![
                        named_field("m_beta", beta_id, "Example::BetaSupport"),
                        named_field("m_missing", missing_id, "Example::MissingSupport"),
                        named_field("m_alpha", alpha_id, "Example::AlphaSupport"),
                    ],
                    variants: Vec::new(),
                },
                SerializeCodegenItem {
                    source_type_id: alpha_id,
                    source_name: "Example::AlphaSupport".to_owned(),
                    role: ReflectedTypeRole::SupportType,
                    is_reflection_marker: false,
                    is_abstract: Some(false),
                    factory: None,
                    rtti_base_chain: Vec::new(),
                    kind: SerializeCodegenItemKind::Struct,
                    enum_underlying_type: None,
                    fields: vec![named_field("m_beta", beta_id, "Example::BetaSupport")],
                    variants: Vec::new(),
                },
                SerializeCodegenItem {
                    source_type_id: beta_id,
                    source_name: "Example::BetaSupport".to_owned(),
                    role: ReflectedTypeRole::SupportType,
                    is_reflection_marker: false,
                    is_abstract: Some(false),
                    factory: None,
                    rtti_base_chain: Vec::new(),
                    kind: SerializeCodegenItemKind::Struct,
                    enum_underlying_type: None,
                    fields: vec![named_field("m_alpha", alpha_id, "Example::AlphaSupport")],
                    variants: Vec::new(),
                },
            ],
        };

        let index = unit.index();
        let component = index
            .item_by_type_id(component_id)
            .expect("component should be indexed");

        assert_eq!(
            index.known_direct_dependency_type_ids(component),
            vec![alpha_id, beta_id]
        );
        assert_eq!(
            index.transitive_dependency_type_ids(component),
            BTreeSet::from([component_id, alpha_id, beta_id])
        );
        assert_eq!(
            index.runtime_root_type_ids(),
            BTreeSet::from([component_id, alpha_id, beta_id])
        );
    }

    #[test]
    fn enum_metadata_is_canonical_when_class_and_enum_share_type_id() {
        let model = SerializeContextModel::from_root(&json!({
            "$id": 1,
            "uuidMap": {
                "33333333-3333-3333-3333-333333333333": {
                    "$id": 10,
                    "name": "Example::Mode",
                    "typeId": "33333333-3333-3333-3333-333333333333",
                    "elements": [{
                        "$id": 11,
                        "name": "m_value",
                        "typeId": type_ids::U8.hyphenated().to_string(),
                        "is_base_class": false
                    }],
                    "attributes": []
                }
            },
            "classNameToUuid": [],
            "uuidGenericMap": [],
            "uuidAnyCreationMap": {},
            "editContext": {
                "$id": 2,
                "classData": [],
                "enumData": [[
                    "33333333-3333-3333-3333-333333333333",
                    {
                        "$id": 20,
                        "name": "Mode",
                        "attributes": [[1, {
                            "$id": 21,
                            "attributeId": 1,
                            "attributeName": "EnumValue",
                            "value": {
                                "kind": "enumConstant",
                                "valueU64": "0x1",
                                "valueU32": 1,
                                "valueI32": 1,
                                "description": "Enabled"
                            }
                        }]]
                    }
                ]]
            },
            "enumTypeIdToUnderlyingTypeIdMap": {
                "33333333-3333-3333-3333-333333333333": type_ids::U8.hyphenated().to_string()
            }
        }));

        let unit = SerializeCodegenPlanner::plan_model(&model);

        assert_eq!(unit.items.len(), 1);
        assert_eq!(unit.items[0].kind, SerializeCodegenItemKind::Enum);
        assert_eq!(unit.items[0].source_name, "Mode");
    }

    fn named_field(name: &str, type_id: uuid::Uuid, source_name: &str) -> SerializeCodegenField {
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

    fn base_field(type_id: uuid::Uuid, source_name: &str) -> SerializeCodegenField {
        SerializeCodegenField {
            is_base_class: true,
            ..named_field("BaseClass1", type_id, source_name)
        }
    }

    fn pointer_field(name: &str, type_id: uuid::Uuid, source_name: &str) -> SerializeCodegenField {
        SerializeCodegenField {
            is_pointer: true,
            ..named_field(name, type_id, source_name)
        }
    }

    fn struct_item(
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
            rtti_base_chain: Vec::new(),
            kind: SerializeCodegenItemKind::Struct,
            enum_underlying_type: None,
            fields,
            variants: Vec::new(),
        }
    }
}
