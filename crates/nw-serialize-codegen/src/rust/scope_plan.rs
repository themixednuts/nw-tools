use std::collections::{BTreeMap, BTreeSet};

use uuid::Uuid;

use crate::ir::SerializeCodegenItem;
use crate::layout::LayoutIndex;
use crate::naming::rust_type_ident;
use crate::rust::layout::nested_collision_file_stem;

pub(super) fn rust_symbol_scope_key(
    item: &SerializeCodegenItem,
    items_by_type_id: &BTreeMap<Uuid, &SerializeCodegenItem>,
    reflected_base_type_ids: &BTreeSet<Uuid>,
    layout_index: &LayoutIndex,
    family_symbol_module_paths: &BTreeSet<(Vec<String>, String)>,
) -> Vec<String> {
    if layout_index.has_concrete_slot_children(item) {
        return layout_index.concrete_slot_owner_scope_segments(item, items_by_type_id);
    }
    if reflected_base_type_ids.contains(&item.source_type_id)
        || layout_index.has_layout_family_descendants(item)
    {
        let mut scope = layout_index.inheritance_family_scope_segments(item, items_by_type_id);
        if scope.is_empty() {
            scope.push(rust_type_ident(&item.source_name));
        }
        return scope;
    }

    let type_path = layout_index.type_path(item, items_by_type_id);
    let mut scope = type_path.scope_segments;
    scope.push(type_path.file_stem.clone());
    if family_symbol_module_paths.contains(&(scope.clone(), rust_type_ident(&item.source_name))) {
        scope.push(nested_collision_file_stem(&scope, &type_path.file_stem));
    }
    scope
}

pub(super) fn standalone_family_symbol_module_paths_by_candidate(
    emitted_items: &[&SerializeCodegenItem],
    items_by_type_id: &BTreeMap<Uuid, &SerializeCodegenItem>,
    reflected_base_type_ids: &BTreeSet<Uuid>,
    layout_index: &LayoutIndex,
) -> BTreeSet<(Vec<String>, String)> {
    emitted_items
        .iter()
        .filter(|item| {
            layout_index.has_concrete_slot_children(item)
                || reflected_base_type_ids.contains(&item.source_type_id)
                || layout_index.has_layout_family_descendants(item)
        })
        .map(|item| {
            (
                rust_family_symbol_scope_key(
                    item,
                    items_by_type_id,
                    reflected_base_type_ids,
                    layout_index,
                ),
                rust_type_ident(&item.source_name),
            )
        })
        .collect()
}

pub(super) fn rust_family_symbol_scope_key(
    item: &SerializeCodegenItem,
    items_by_type_id: &BTreeMap<Uuid, &SerializeCodegenItem>,
    reflected_base_type_ids: &BTreeSet<Uuid>,
    layout_index: &LayoutIndex,
) -> Vec<String> {
    if layout_index.has_concrete_slot_children(item) {
        return layout_index.concrete_slot_owner_scope_segments(item, items_by_type_id);
    }
    if reflected_base_type_ids.contains(&item.source_type_id)
        || layout_index.has_layout_family_descendants(item)
    {
        let mut scope = layout_index.inheritance_family_scope_segments(item, items_by_type_id);
        if scope.is_empty() {
            scope.push(rust_type_ident(&item.source_name));
        }
        return scope;
    }
    Vec::new()
}
