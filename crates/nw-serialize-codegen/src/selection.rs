use std::collections::BTreeSet;

use thiserror::Error;
use uuid::Uuid;

use crate::ir::{SerializeCodegenItem, SerializeCodegenUnit};
use crate::layout::LayoutIndex;
use crate::naming::rust_type_name;
use crate::role::ReflectedTypeRole;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum SerializeCodegenRootMode {
    All,
    #[default]
    Runtime,
    Explicit,
}

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum SerializeCodegenRootResolveError {
    #[error("unknown root UUID `{0}`")]
    UnknownUuid(Uuid),
    #[error("unknown root `{0}`")]
    UnknownRoot(String),
    #[error("ambiguous root `{root}`; matches: {matches}")]
    AmbiguousRoot { root: String, matches: String },
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SerializeCodegenRootSelection {
    mode: SerializeCodegenRootMode,
    explicit_root_type_ids: BTreeSet<Uuid>,
}

pub fn resolve_codegen_root_type_ids<'a>(
    unit: &SerializeCodegenUnit,
    root_specs: impl IntoIterator<Item = &'a str>,
) -> Result<Vec<Uuid>, SerializeCodegenRootResolveError> {
    root_specs
        .into_iter()
        .map(|root_spec| resolve_codegen_root_type_id(unit, root_spec))
        .collect()
}

pub fn resolve_codegen_root_type_id(
    unit: &SerializeCodegenUnit,
    root_spec: &str,
) -> Result<Uuid, SerializeCodegenRootResolveError> {
    if let Ok(type_id) = Uuid::parse_str(root_spec) {
        if unit.item_by_source_type_id(type_id).is_none() {
            return Err(SerializeCodegenRootResolveError::UnknownUuid(type_id));
        }
        return Ok(type_id);
    }

    let exact = unit
        .items
        .iter()
        .filter(|item| item.source_name == root_spec)
        .collect::<Vec<_>>();
    if let Some(type_id) = unique_root_match(root_spec, &exact)? {
        return Ok(type_id);
    }

    let unqualified = unit
        .items
        .iter()
        .filter(|item| item.source_name.rsplit("::").next() == Some(root_spec))
        .collect::<Vec<_>>();
    if let Some(type_id) = unique_root_match(root_spec, &unqualified)? {
        return Ok(type_id);
    }

    let rust_name = unit
        .items
        .iter()
        .filter(|item| rust_type_name(&item.source_name) == root_spec)
        .collect::<Vec<_>>();
    if let Some(type_id) = unique_root_match(root_spec, &rust_name)? {
        return Ok(type_id);
    }

    Err(SerializeCodegenRootResolveError::UnknownRoot(
        root_spec.to_owned(),
    ))
}

fn unique_root_match(
    root_spec: &str,
    matches: &[&SerializeCodegenItem],
) -> Result<Option<Uuid>, SerializeCodegenRootResolveError> {
    match matches {
        [] => Ok(None),
        [item] => Ok(Some(item.source_type_id)),
        _ => {
            let matches = matches
                .iter()
                .map(|item| format!("{} ({})", item.source_name, item.source_type_id))
                .collect::<Vec<_>>()
                .join(", ");
            Err(SerializeCodegenRootResolveError::AmbiguousRoot {
                root: root_spec.to_owned(),
                matches,
            })
        }
    }
}

impl SerializeCodegenRootSelection {
    #[must_use]
    pub const fn new(mode: SerializeCodegenRootMode) -> Self {
        Self {
            mode,
            explicit_root_type_ids: BTreeSet::new(),
        }
    }

    #[must_use]
    pub fn with_explicit_roots(mut self, root_type_ids: impl IntoIterator<Item = Uuid>) -> Self {
        self.explicit_root_type_ids.extend(root_type_ids);
        self
    }

    #[must_use]
    pub const fn mode(&self) -> SerializeCodegenRootMode {
        self.mode
    }

    #[must_use]
    pub fn explicit_root_type_ids(&self) -> &BTreeSet<Uuid> {
        &self.explicit_root_type_ids
    }

    #[must_use]
    pub fn select_unit(&self, unit: &SerializeCodegenUnit) -> SerializeCodegenUnit {
        unit.select_exact_type_ids(&self.selected_type_ids(unit))
    }

    #[must_use]
    pub fn selected_type_ids(&self, unit: &SerializeCodegenUnit) -> BTreeSet<Uuid> {
        let index = unit.index();
        let layout = LayoutIndex::from_codegen_unit(unit);
        let mut type_ids = match self.mode {
            SerializeCodegenRootMode::All => unit
                .items
                .iter()
                .map(|item| item.source_type_id)
                .collect::<BTreeSet<_>>(),
            SerializeCodegenRootMode::Runtime => index.runtime_root_type_ids(),
            SerializeCodegenRootMode::Explicit => BTreeSet::new(),
        };

        for root_type_id in &self.explicit_root_type_ids {
            let Some(root) = index.item_by_type_id(*root_type_id) else {
                continue;
            };
            extend_root_family_type_ids(&index, &layout, root, &mut type_ids);
        }
        extend_selected_support_family_type_ids(&index, &layout, &mut type_ids);

        type_ids
    }
}

fn extend_root_family_type_ids(
    index: &crate::ir::SerializeCodegenIndex<'_>,
    layout: &LayoutIndex,
    root: &SerializeCodegenItem,
    type_ids: &mut BTreeSet<Uuid>,
) {
    index.extend_transitive_dependency_type_ids(root, type_ids);
    for (child_type_id, binding) in &layout.concrete_slot_bindings {
        if binding.owner_type_id != root.source_type_id {
            continue;
        }
        let Some(child) = index.item_by_type_id(*child_type_id) else {
            continue;
        };
        index.extend_transitive_dependency_type_ids(child, type_ids);
    }
}

fn extend_selected_support_family_type_ids(
    index: &crate::ir::SerializeCodegenIndex<'_>,
    layout: &LayoutIndex,
    type_ids: &mut BTreeSet<Uuid>,
) {
    let mut stack = type_ids.iter().copied().collect::<Vec<_>>();
    while let Some(owner_type_id) = stack.pop() {
        let Some(owner) = index.item_by_type_id(owner_type_id) else {
            continue;
        };
        if owner.role != ReflectedTypeRole::SupportType {
            continue;
        }

        for child_type_id in support_family_child_type_ids(layout, owner_type_id) {
            let Some(child) = index.item_by_type_id(child_type_id) else {
                continue;
            };
            let before = type_ids.clone();
            index.extend_transitive_dependency_type_ids(child, type_ids);
            stack.extend(type_ids.difference(&before).copied());
        }
    }
}

fn support_family_child_type_ids(layout: &LayoutIndex, owner_type_id: Uuid) -> Vec<Uuid> {
    let mut child_type_ids = layout
        .direct_derived_type_ids_by_base_type_id
        .get(&owner_type_id)
        .cloned()
        .unwrap_or_default();
    child_type_ids.extend(layout.concrete_slot_bindings.iter().filter_map(
        |(child_type_id, binding)| {
            (binding.owner_type_id == owner_type_id).then_some(*child_type_id)
        },
    ));
    child_type_ids.sort_unstable();
    child_type_ids.dedup();
    child_type_ids
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use uuid::uuid;

    use crate::{
        ReflectedTypeRole, ResolvedType, SerializeCodegenField, SerializeCodegenItem,
        SerializeCodegenItemKind, SerializeCodegenRootMode, SerializeCodegenRootSelection,
        resolve_codegen_root_type_id, resolve_codegen_root_type_ids,
    };

    #[test]
    fn explicit_selects_only_requested_roots_and_dependency_closure() {
        let player_save_data_id = uuid!("11111111-1111-1111-1111-111111111111");
        let paperdoll_id = uuid!("22222222-2222-2222-2222-222222222222");
        let item_id = uuid!("33333333-3333-3333-3333-333333333333");
        let unrelated_component_id = uuid!("44444444-4444-4444-4444-444444444444");
        let unit = unit([
            item(
                player_save_data_id,
                "PlayerSaveData",
                ReflectedTypeRole::SupportType,
                vec![field(
                    "m_paperdoll",
                    paperdoll_id,
                    "PersistentPaperdollData",
                )],
            ),
            item(
                paperdoll_id,
                "PersistentPaperdollData",
                ReflectedTypeRole::SupportType,
                vec![field("m_item", item_id, "Item")],
            ),
            item(item_id, "Item", ReflectedTypeRole::SupportType, Vec::new()),
            item(
                unrelated_component_id,
                "UnrelatedComponent",
                ReflectedTypeRole::AzComponent,
                Vec::new(),
            ),
        ]);

        let selected = SerializeCodegenRootSelection::new(SerializeCodegenRootMode::Explicit)
            .with_explicit_roots([player_save_data_id])
            .select_unit(&unit);
        let selected_ids = selected_type_ids(&selected);

        assert_eq!(
            selected_ids,
            BTreeSet::from([player_save_data_id, paperdoll_id, item_id])
        );
    }

    #[test]
    fn runtime_can_add_explicit_data_roots() {
        let component_id = uuid!("11111111-1111-1111-1111-111111111111");
        let component_support_id = uuid!("22222222-2222-2222-2222-222222222222");
        let player_save_data_id = uuid!("33333333-3333-3333-3333-333333333333");
        let player_support_id = uuid!("44444444-4444-4444-4444-444444444444");
        let unit = unit([
            item(
                component_id,
                "RuntimeComponent",
                ReflectedTypeRole::AzComponent,
                vec![field(
                    "m_support",
                    component_support_id,
                    "RuntimeComponentSupport",
                )],
            ),
            item(
                component_support_id,
                "RuntimeComponentSupport",
                ReflectedTypeRole::SupportType,
                Vec::new(),
            ),
            item(
                player_save_data_id,
                "PlayerSaveData",
                ReflectedTypeRole::SupportType,
                vec![field("m_support", player_support_id, "PlayerSupport")],
            ),
            item(
                player_support_id,
                "PlayerSupport",
                ReflectedTypeRole::SupportType,
                Vec::new(),
            ),
        ]);

        let selected = SerializeCodegenRootSelection::new(SerializeCodegenRootMode::Runtime)
            .with_explicit_roots([player_save_data_id])
            .select_unit(&unit);
        let selected_ids = selected_type_ids(&selected);

        assert_eq!(
            selected_ids,
            BTreeSet::from([
                component_id,
                component_support_id,
                player_save_data_id,
                player_support_id,
            ])
        );
    }

    #[test]
    fn runtime_dependency_on_support_base_includes_concrete_family() {
        let component_id = uuid!("11111111-1111-1111-1111-111111111111");
        let support_base_id = uuid!("22222222-2222-2222-2222-222222222222");
        let support_child_id = uuid!("33333333-3333-3333-3333-333333333333");
        let child_payload_id = uuid!("44444444-4444-4444-4444-444444444444");
        let unrelated_support_id = uuid!("55555555-5555-5555-5555-555555555555");
        let unit = unit([
            item(
                component_id,
                "RuntimeComponent",
                ReflectedTypeRole::AzComponent,
                vec![field("m_shape", support_base_id, "QueryShapeBase")],
            ),
            item(
                support_base_id,
                "QueryShapeBase",
                ReflectedTypeRole::SupportType,
                Vec::new(),
            ),
            item(
                support_child_id,
                "QueryShapeAabb",
                ReflectedTypeRole::SupportType,
                vec![
                    base_field(support_base_id, "QueryShapeBase"),
                    field("m_payload", child_payload_id, "QueryShapePayload"),
                ],
            ),
            item(
                child_payload_id,
                "QueryShapePayload",
                ReflectedTypeRole::SupportType,
                Vec::new(),
            ),
            item(
                unrelated_support_id,
                "OtherSupport",
                ReflectedTypeRole::SupportType,
                Vec::new(),
            ),
        ]);

        let selected = SerializeCodegenRootSelection::new(SerializeCodegenRootMode::Runtime)
            .select_unit(&unit);
        let selected_ids = selected_type_ids(&selected);

        assert_eq!(
            selected_ids,
            BTreeSet::from([
                component_id,
                support_base_id,
                support_child_id,
                child_payload_id,
            ])
        );
    }

    #[test]
    fn explicit_component_root_includes_concrete_facet_family() {
        let faceted_component_id = uuid!("11111111-1111-1111-1111-111111111111");
        let client_facet_id = uuid!("22222222-2222-2222-2222-222222222222");
        let server_facet_id = uuid!("33333333-3333-3333-3333-333333333333");
        let component_id = uuid!("44444444-4444-4444-4444-444444444444");
        let component_client_facet_id = uuid!("55555555-5555-5555-5555-555555555555");
        let component_server_facet_id = uuid!("66666666-6666-6666-6666-666666666666");
        let server_support_id = uuid!("77777777-7777-7777-7777-777777777777");
        let unrelated_facet_id = uuid!("88888888-8888-8888-8888-888888888888");

        let unit = unit([
            item(
                faceted_component_id,
                "FacetedComponent",
                ReflectedTypeRole::FacetedComponent,
                vec![
                    pointer_field("m_clientFacetPtr", client_facet_id, "ClientFacet"),
                    pointer_field("m_serverFacetPtr", server_facet_id, "ServerFacet"),
                ],
            ),
            item(
                client_facet_id,
                "ClientFacet",
                ReflectedTypeRole::ClientFacet,
                Vec::new(),
            ),
            item(
                server_facet_id,
                "ServerFacet",
                ReflectedTypeRole::ServerFacet,
                Vec::new(),
            ),
            item(
                component_id,
                "RuntimeComponent",
                ReflectedTypeRole::FacetedComponent,
                vec![base_field(faceted_component_id, "FacetedComponent")],
            ),
            item(
                component_client_facet_id,
                "RuntimeComponentClientFacet",
                ReflectedTypeRole::ClientFacet,
                vec![base_field(client_facet_id, "ClientFacet")],
            ),
            item(
                component_server_facet_id,
                "RuntimeComponentServerFacet",
                ReflectedTypeRole::ServerFacet,
                vec![
                    base_field(server_facet_id, "ServerFacet"),
                    field("m_support", server_support_id, "ServerSupport"),
                ],
            ),
            item(
                server_support_id,
                "ServerSupport",
                ReflectedTypeRole::SupportType,
                Vec::new(),
            ),
            item(
                unrelated_facet_id,
                "OtherComponentClientFacet",
                ReflectedTypeRole::ClientFacet,
                vec![base_field(client_facet_id, "ClientFacet")],
            ),
        ]);

        let selected = SerializeCodegenRootSelection::new(SerializeCodegenRootMode::Explicit)
            .with_explicit_roots([component_id])
            .select_unit(&unit);
        let selected_ids = selected_type_ids(&selected);

        assert_eq!(
            selected_ids,
            BTreeSet::from([
                faceted_component_id,
                client_facet_id,
                server_facet_id,
                component_id,
                component_client_facet_id,
                component_server_facet_id,
                server_support_id,
            ])
        );
    }

    #[test]
    fn resolves_roots_by_uuid_exact_name_or_unique_leaf_name() {
        let player_save_data_id = uuid!("11111111-1111-1111-1111-111111111111");
        let persistent_data_id = uuid!("22222222-2222-2222-2222-222222222222");
        let edit_enum_id = uuid!("33333333-3333-3333-3333-333333333333");
        let unit = unit([
            item(
                player_save_data_id,
                "PlayerSaveData",
                ReflectedTypeRole::SupportType,
                Vec::new(),
            ),
            item(
                persistent_data_id,
                "Javelin::PersistentData",
                ReflectedTypeRole::SupportType,
                Vec::new(),
            ),
            item(
                edit_enum_id,
                "EditEnum<EnumType><Javelin::SBItemClass::ItemClasses >",
                ReflectedTypeRole::SupportType,
                Vec::new(),
            ),
        ]);

        assert_eq!(
            resolve_codegen_root_type_id(&unit, "PlayerSaveData").unwrap(),
            player_save_data_id
        );
        assert_eq!(
            resolve_codegen_root_type_id(&unit, "Javelin::PersistentData").unwrap(),
            persistent_data_id
        );
        assert_eq!(
            resolve_codegen_root_type_id(&unit, "PersistentData").unwrap(),
            persistent_data_id
        );
        assert_eq!(
            resolve_codegen_root_type_id(&unit, "EditEnumItemClasses").unwrap(),
            edit_enum_id
        );
        let player_save_data_uuid = player_save_data_id.to_string();
        assert_eq!(
            resolve_codegen_root_type_ids(
                &unit,
                [player_save_data_uuid.as_str(), "Javelin::PersistentData"]
            )
            .unwrap(),
            vec![player_save_data_id, persistent_data_id]
        );
    }

    fn selected_type_ids(unit: &crate::SerializeCodegenUnit) -> BTreeSet<uuid::Uuid> {
        unit.items.iter().map(|item| item.source_type_id).collect()
    }

    fn unit(items: impl IntoIterator<Item = SerializeCodegenItem>) -> crate::SerializeCodegenUnit {
        crate::SerializeCodegenUnit {
            items: items.into_iter().collect(),
        }
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
            rtti_base_chain: Vec::new(),
            kind: SerializeCodegenItemKind::Struct,
            enum_underlying_type: None,
            fields,
            variants: Vec::new(),
        }
    }

    fn field(name: &str, type_id: uuid::Uuid, source_name: &str) -> SerializeCodegenField {
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
            ..field("BaseClass1", type_id, source_name)
        }
    }

    fn pointer_field(name: &str, type_id: uuid::Uuid, source_name: &str) -> SerializeCodegenField {
        SerializeCodegenField {
            is_pointer: true,
            ..field(name, type_id, source_name)
        }
    }
}
