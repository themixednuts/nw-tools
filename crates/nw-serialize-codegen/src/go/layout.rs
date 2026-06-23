use std::collections::{BTreeMap, BTreeSet};

use heck::ToSnakeCase;

use crate::dependency_graph::sorted_strongly_connected_components;
use crate::ir::{SerializeCodegenItem, SerializeCodegenUnit};
use crate::layout::{
    LayoutIndex, LayoutPathSet, dependency_ordered_codegen_items, reflected_base_type_ids,
};
use crate::naming::rust_type_ident;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GoStandaloneLayoutReport {
    pub files: Vec<GoStandaloneLayoutFileReport>,
}

impl GoStandaloneLayoutReport {
    #[must_use]
    pub fn from_codegen_unit(unit: &SerializeCodegenUnit) -> Self {
        Self::from_codegen_unit_with_context(unit, unit)
    }

    #[must_use]
    pub fn from_codegen_unit_with_context(
        emitted_unit: &SerializeCodegenUnit,
        context_unit: &SerializeCodegenUnit,
    ) -> Self {
        let context_items_by_type_id = items_by_type_id(context_unit);
        let base_type_ids = reflected_base_type_ids(context_unit);
        let mut files = Vec::new();
        for ((package, file_stem), items) in go_package_groups_with_context(
            emitted_unit,
            context_unit,
            &context_items_by_type_id,
            &base_type_ids,
        ) {
            let mut report_items = Vec::with_capacity(items.len());
            for item in items {
                report_items.push(layout_item_report(item));
            }
            files.push(GoStandaloneLayoutFileReport {
                path: go_type_file_path(&package, &file_stem),
                package_dir: package.dir,
                package_name: package.name,
                file_stem,
                items: report_items,
            });
        }
        Self { files }
    }

    #[must_use]
    pub fn to_text(&self) -> String {
        let mut out = String::new();
        for file in &self.files {
            out.push_str("file ");
            out.push_str(&file.path);
            out.push_str(" package ");
            out.push_str(&file.package_name);
            out.push('\n');
            for item in &file.items {
                append_layout_item(&mut out, item);
            }
        }
        out
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GoStandaloneLayoutFileReport {
    pub path: String,
    pub package_dir: String,
    pub package_name: String,
    pub file_stem: String,
    pub items: Vec<GoStandaloneLayoutItemReport>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GoStandaloneLayoutItemReport {
    pub type_id: uuid::Uuid,
    pub go_name: String,
    pub source_name: String,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub(super) struct GoTypePackage {
    pub(super) dir: String,
    pub(super) name: String,
}

pub(super) fn items_by_type_id(
    unit: &SerializeCodegenUnit,
) -> BTreeMap<uuid::Uuid, &SerializeCodegenItem> {
    unit.index().into_items_by_type_id()
}

pub(super) fn go_type_packages_by_id(
    unit: &SerializeCodegenUnit,
    items_by_type_id: &BTreeMap<uuid::Uuid, &SerializeCodegenItem>,
    base_type_ids: &BTreeSet<uuid::Uuid>,
) -> BTreeMap<uuid::Uuid, GoTypePackage> {
    let layout_index = LayoutIndex::from_codegen_unit(unit);
    let mut packages = BTreeMap::new();
    for item in dependency_ordered_codegen_items(unit)
        .into_iter()
        .filter(|item| !item.is_reflection_marker)
    {
        packages.insert(
            item.source_type_id,
            go_type_package(item, items_by_type_id, base_type_ids, &layout_index),
        );
    }
    promote_parent_files_to_child_packages(unit, &mut packages, &layout_index);
    split_root_data_packages_from_artificial_cycles(unit, &mut packages, &layout_index);
    split_same_name_package_collisions(unit, &mut packages, &layout_index);
    collapse_go_package_cycles(unit, packages)
}

pub(super) fn go_type_packages_by_id_with_context(
    emitted_unit: &SerializeCodegenUnit,
    context_unit: &SerializeCodegenUnit,
    context_items_by_type_id: &BTreeMap<uuid::Uuid, &SerializeCodegenItem>,
    context_base_type_ids: &BTreeSet<uuid::Uuid>,
) -> BTreeMap<uuid::Uuid, GoTypePackage> {
    let emitted_type_ids = emitted_unit
        .items
        .iter()
        .map(|item| item.source_type_id)
        .collect::<BTreeSet<_>>();
    go_type_packages_by_id(
        context_unit,
        context_items_by_type_id,
        context_base_type_ids,
    )
    .into_iter()
    .filter(|(type_id, _)| emitted_type_ids.contains(type_id))
    .collect()
}

fn promote_parent_files_to_child_packages(
    unit: &SerializeCodegenUnit,
    packages_by_type_id: &mut BTreeMap<uuid::Uuid, GoTypePackage>,
    layout_index: &LayoutIndex,
) {
    let item_index = items_by_type_id(unit);
    let package_paths = LayoutPathSet::from_paths(
        packages_by_type_id
            .values()
            .map(|package| slash_path_segments(&package.dir)),
    );

    for item in dependency_ordered_codegen_items(unit)
        .into_iter()
        .filter(|item| !item.is_reflection_marker)
    {
        let Some(package) = packages_by_type_id.get(&item.source_type_id).cloned() else {
            continue;
        };
        let file_stem = sanitize_go_file_stem(&layout_index.type_path(item, &item_index).file_stem);
        let mut child_path = slash_path_segments(&package.dir);
        child_path.push(file_stem.clone());
        if package_paths.contains_self_or_descendant(&child_path) {
            packages_by_type_id.insert(
                item.source_type_id,
                GoTypePackage {
                    dir: child_path.join("/"),
                    name: sanitize_go_package_segment(&file_stem),
                },
            );
        }
    }
}

fn split_root_data_packages_from_artificial_cycles(
    unit: &SerializeCodegenUnit,
    packages_by_type_id: &mut BTreeMap<uuid::Uuid, GoTypePackage>,
    layout_index: &LayoutIndex,
) {
    let item_index = items_by_type_id(unit);
    let max_iterations = unit.items.len().saturating_add(1);
    for _ in 0..max_iterations {
        let graph = go_package_dependency_graph(unit, packages_by_type_id);
        let root_is_cyclic = sorted_strongly_connected_components(&graph)
            .into_iter()
            .any(|component| {
                component.len() > 1 && component.iter().any(|package| package.dir == "types")
            });
        if !root_is_cyclic {
            return;
        }

        let mut changed = false;
        for item in dependency_ordered_codegen_items(unit)
            .into_iter()
            .filter(|item| !item.is_reflection_marker)
        {
            let Some(package) = packages_by_type_id.get(&item.source_type_id) else {
                continue;
            };
            if package.dir != "types" {
                continue;
            }

            let file_stem =
                sanitize_go_file_stem(&layout_index.type_path(item, &item_index).file_stem);
            let isolated = GoTypePackage {
                dir: format!("types/{file_stem}"),
                name: sanitize_go_package_segment(&file_stem),
            };
            if package != &isolated {
                packages_by_type_id.insert(item.source_type_id, isolated);
                changed = true;
            }
        }
        if !changed {
            return;
        }
    }
}

fn split_same_name_package_collisions(
    unit: &SerializeCodegenUnit,
    packages_by_type_id: &mut BTreeMap<uuid::Uuid, GoTypePackage>,
    layout_index: &LayoutIndex,
) {
    let item_index = items_by_type_id(unit);
    let type_components_by_id = type_components_by_id(unit);
    let max_iterations = unit.items.len().saturating_add(1);
    for _ in 0..max_iterations {
        let mut groups = BTreeMap::<(GoTypePackage, String), Vec<&SerializeCodegenItem>>::new();
        for item in dependency_ordered_codegen_items(unit)
            .into_iter()
            .filter(|item| !item.is_reflection_marker)
        {
            let Some(package) = packages_by_type_id.get(&item.source_type_id) else {
                continue;
            };
            groups
                .entry((package.clone(), rust_type_ident(&item.source_name)))
                .or_default()
                .push(item);
        }

        let mut changed = false;
        for ((package, _), mut items) in groups {
            if items.len() <= 1 {
                continue;
            }
            items.sort_by_key(|item| {
                (
                    !layout_index.has_layout_family_descendants(item),
                    item.source_type_id,
                )
            });
            let retained_type_id = items[0].source_type_id;
            for item in items.into_iter().skip(1) {
                let component_type_ids = type_components_by_id
                    .get(&item.source_type_id)
                    .cloned()
                    .unwrap_or_else(|| vec![item.source_type_id]);
                if component_type_ids.contains(&retained_type_id) {
                    continue;
                }
                let file_stem =
                    sanitize_go_file_stem(&layout_index.type_path(item, &item_index).file_stem);
                let isolated = GoTypePackage {
                    dir: format!("{}/{file_stem}", package.dir),
                    name: sanitize_go_package_segment(&file_stem),
                };
                for type_id in component_type_ids {
                    if packages_by_type_id.contains_key(&type_id)
                        && packages_by_type_id.get(&type_id) != Some(&isolated)
                    {
                        packages_by_type_id.insert(type_id, isolated.clone());
                        changed = true;
                    }
                }
            }
        }
        if !changed {
            return;
        }
    }
}

fn type_components_by_id(unit: &SerializeCodegenUnit) -> BTreeMap<uuid::Uuid, Vec<uuid::Uuid>> {
    let graph = type_dependency_graph(unit);
    let mut components_by_id = BTreeMap::new();
    for component in sorted_strongly_connected_components(&graph) {
        for type_id in &component {
            components_by_id.insert(*type_id, component.clone());
        }
    }
    components_by_id
}

fn type_dependency_graph(
    unit: &SerializeCodegenUnit,
) -> BTreeMap<uuid::Uuid, BTreeSet<uuid::Uuid>> {
    let index = unit.index();
    let mut graph = BTreeMap::<uuid::Uuid, BTreeSet<uuid::Uuid>>::new();
    for item in unit.items.iter().filter(|item| !item.is_reflection_marker) {
        graph.entry(item.source_type_id).or_default();
        for referenced_type_id in index.known_direct_dependency_type_ids(item) {
            if index
                .item_by_type_id(referenced_type_id)
                .is_some_and(|item| !item.is_reflection_marker)
            {
                graph
                    .entry(item.source_type_id)
                    .or_default()
                    .insert(referenced_type_id);
            }
        }
    }
    graph
}

pub(super) fn go_package_groups<'a>(
    unit: &'a SerializeCodegenUnit,
    items_by_type_id: &BTreeMap<uuid::Uuid, &'a SerializeCodegenItem>,
    base_type_ids: &BTreeSet<uuid::Uuid>,
) -> BTreeMap<(GoTypePackage, String), Vec<&'a SerializeCodegenItem>> {
    let layout_index = LayoutIndex::from_codegen_unit(unit);
    let packages_by_type_id = go_type_packages_by_id(unit, items_by_type_id, base_type_ids);
    let mut groups = BTreeMap::<(GoTypePackage, String), Vec<&SerializeCodegenItem>>::new();
    for item in dependency_ordered_codegen_items(unit)
        .into_iter()
        .filter(|item| !item.is_reflection_marker)
    {
        let package = packages_by_type_id
            .get(&item.source_type_id)
            .cloned()
            .unwrap_or_else(|| {
                go_type_package(item, items_by_type_id, base_type_ids, &layout_index)
            });
        let mut file_stem =
            sanitize_go_file_stem(&layout_index.type_path(item, items_by_type_id).file_stem);
        if groups.contains_key(&(package.clone(), file_stem.clone())) {
            file_stem.push('_');
            file_stem.push_str(&type_id_suffix(item.source_type_id));
        }
        groups.entry((package, file_stem)).or_default().push(item);
    }
    groups
}

pub(super) fn go_package_groups_with_context<'a>(
    emitted_unit: &'a SerializeCodegenUnit,
    context_unit: &'a SerializeCodegenUnit,
    context_items_by_type_id: &BTreeMap<uuid::Uuid, &'a SerializeCodegenItem>,
    context_base_type_ids: &BTreeSet<uuid::Uuid>,
) -> BTreeMap<(GoTypePackage, String), Vec<&'a SerializeCodegenItem>> {
    debug_assert!(
        emitted_unit
            .items
            .iter()
            .all(|item| context_items_by_type_id.contains_key(&item.source_type_id))
    );
    let context_targets = go_package_groups(
        context_unit,
        context_items_by_type_id,
        context_base_type_ids,
    )
    .into_iter()
    .flat_map(|((package, file_stem), items)| {
        items
            .into_iter()
            .map(move |item| (item.source_type_id, (package.clone(), file_stem.clone())))
    })
    .collect::<BTreeMap<_, _>>();
    let mut groups = BTreeMap::<(GoTypePackage, String), Vec<&SerializeCodegenItem>>::new();
    for item in dependency_ordered_codegen_items(emitted_unit)
        .into_iter()
        .filter(|item| !item.is_reflection_marker)
    {
        let (package, file_stem) = context_targets
            .get(&item.source_type_id)
            .cloned()
            .expect("emitted Go item must exist in the full context layout");
        groups.entry((package, file_stem)).or_default().push(item);
    }
    groups
}

fn collapse_go_package_cycles(
    unit: &SerializeCodegenUnit,
    packages_by_type_id: BTreeMap<uuid::Uuid, GoTypePackage>,
) -> BTreeMap<uuid::Uuid, GoTypePackage> {
    let package_graph = go_package_dependency_graph(unit, &packages_by_type_id);
    let mut collapsed_by_package = BTreeMap::new();
    for component in sorted_strongly_connected_components(&package_graph) {
        if component.len() <= 1 {
            continue;
        }
        let package = common_go_package(&component);
        for member in component {
            collapsed_by_package.insert(member, package.clone());
        }
    }

    packages_by_type_id
        .into_iter()
        .map(|(type_id, package)| {
            let package = collapsed_by_package
                .get(&package)
                .cloned()
                .unwrap_or(package);
            (type_id, package)
        })
        .collect()
}

fn go_package_dependency_graph(
    unit: &SerializeCodegenUnit,
    packages_by_type_id: &BTreeMap<uuid::Uuid, GoTypePackage>,
) -> BTreeMap<GoTypePackage, BTreeSet<GoTypePackage>> {
    let index = unit.index();
    let mut graph = BTreeMap::<GoTypePackage, BTreeSet<GoTypePackage>>::new();
    for item in unit.items.iter().filter(|item| !item.is_reflection_marker) {
        let Some(source_package) = packages_by_type_id.get(&item.source_type_id) else {
            continue;
        };
        graph.entry(source_package.clone()).or_default();
        for referenced_type_id in index.known_direct_dependency_type_ids(item) {
            if let Some(target_package) = packages_by_type_id.get(&referenced_type_id)
                && target_package != source_package
            {
                graph
                    .entry(source_package.clone())
                    .or_default()
                    .insert(target_package.clone());
            }
        }
    }
    graph
}

fn common_go_package(packages: &[GoTypePackage]) -> GoTypePackage {
    let common_segments = common_package_segments(packages);
    let dir = common_segments.join("/");
    let name = common_segments
        .last()
        .cloned()
        .unwrap_or_else(|| "types".to_owned());
    GoTypePackage { dir, name }
}

fn common_package_segments(packages: &[GoTypePackage]) -> Vec<String> {
    let mut iter = packages.iter().map(|package| {
        package
            .dir
            .split('/')
            .map(str::to_owned)
            .collect::<Vec<_>>()
    });
    let Some(mut common) = iter.next() else {
        return vec!["types".to_owned()];
    };
    for segments in iter {
        let keep = common
            .iter()
            .zip(&segments)
            .take_while(|(left, right)| left == right)
            .count();
        common.truncate(keep);
    }
    if common.is_empty() {
        vec!["types".to_owned()]
    } else {
        common
    }
}

fn slash_path_segments(path: &str) -> Vec<String> {
    path.split('/').map(str::to_owned).collect()
}

pub(super) fn go_type_file_path(package: &GoTypePackage, file_stem: &str) -> String {
    format!("{}/{file_stem}.go", package.dir)
}

fn go_type_package(
    item: &SerializeCodegenItem,
    items_by_type_id: &BTreeMap<uuid::Uuid, &SerializeCodegenItem>,
    base_type_ids: &BTreeSet<uuid::Uuid>,
    layout_index: &LayoutIndex,
) -> GoTypePackage {
    let _ = base_type_ids;
    let type_path = layout_index.type_path(item, items_by_type_id);
    let mut segments = type_path
        .scope_segments
        .into_iter()
        .map(|segment| sanitize_go_package_segment(&segment))
        .collect::<Vec<_>>();
    let file_stem = sanitize_go_file_stem(&type_path.file_stem);
    collapse_duplicate_package_scope(&mut segments, &file_stem);
    let name = segments
        .last()
        .cloned()
        .unwrap_or_else(|| "types".to_owned());
    let dir = if segments.is_empty() {
        "types".to_owned()
    } else {
        format!("types/{}", segments.join("/"))
    };
    GoTypePackage { dir, name }
}

fn collapse_duplicate_package_scope(segments: &mut Vec<String>, file_stem: &str) {
    if segments.last().is_some_and(|segment| segment == file_stem) {
        segments.pop();
    }
}

fn layout_item_report(item: &SerializeCodegenItem) -> GoStandaloneLayoutItemReport {
    GoStandaloneLayoutItemReport {
        type_id: item.source_type_id,
        go_name: rust_type_ident(&item.source_name),
        source_name: item.source_name.clone(),
    }
}

fn append_layout_item(out: &mut String, item: &GoStandaloneLayoutItemReport) {
    out.push_str("  item ");
    out.push_str(&item.go_name);
    out.push(' ');
    out.push_str(&item.type_id.hyphenated().to_string());
    out.push(' ');
    out.push_str(&item.source_name);
    out.push('\n');
}

fn sanitize_go_package_segment(segment: &str) -> String {
    let mut name = segment
        .to_snake_case()
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() {
                ch.to_ascii_lowercase()
            } else {
                '_'
            }
        })
        .collect::<String>();
    while name.contains("__") {
        name = name.replace("__", "_");
    }
    let name = name.trim_matches('_');
    let mut name = if name.is_empty() {
        "types".to_owned()
    } else {
        name.to_owned()
    };
    if name.chars().next().is_some_and(|ch| ch.is_ascii_digit()) {
        name.insert_str(0, "pkg_");
    }
    if !is_go_identifier(&name) || is_go_keyword(&name) {
        name.push_str("_types");
    }
    name
}

fn sanitize_go_file_stem(name: &str) -> String {
    let mut stem = name
        .to_snake_case()
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() {
                ch.to_ascii_lowercase()
            } else {
                '_'
            }
        })
        .collect::<String>();
    while stem.contains("__") {
        stem = stem.replace("__", "_");
    }
    let stem = stem.trim_matches('_');
    if stem.is_empty() {
        "types".to_owned()
    } else if stem.chars().next().is_some_and(|ch| ch.is_ascii_digit()) {
        format!("type_{stem}")
    } else {
        stem.to_owned()
    }
}

fn type_id_suffix(type_id: uuid::Uuid) -> String {
    type_id.as_simple().to_string().chars().take(8).collect()
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use uuid::uuid;

    use crate::ir::{
        SerializeCodegenField, SerializeCodegenItem, SerializeCodegenItemKind,
        SerializeCodegenRttiBase, SerializeCodegenUnit, SerializeCodegenVariant,
    };
    use crate::role::ReflectedTypeRole;
    use crate::types::ResolvedType;

    use super::{
        GoStandaloneLayoutReport, GoTypePackage, go_package_dependency_graph, type_components_by_id,
    };

    #[test]
    fn promotes_parent_file_into_child_package_when_selected_children_use_that_family() {
        let child_type_id = uuid!("bbbbbbbb-bbbb-bbbb-bbbb-bbbbbbbbbbbb");
        let owner = item(
            uuid!("aaaaaaaa-aaaa-aaaa-aaaa-aaaaaaaaaaaa"),
            "NavigationProfile",
            vec![value_field(
                "m_navConfigs",
                child_type_id,
                "NavConfiguration",
            )],
        );
        let child = item(child_type_id, "NavConfiguration", Vec::new());

        let report =
            GoStandaloneLayoutReport::from_codegen_unit(&crate::ir::SerializeCodegenUnit {
                items: vec![owner, child],
            });

        assert!(report.files.iter().any(|file| {
            file.path == "types/navigation_profile/navigation_profile.go"
                && file.package_name == "navigation_profile"
        }));
        assert!(report.files.iter().any(|file| {
            file.path == "types/navigation_profile/nav_configuration.go"
                && file.package_name == "navigation_profile"
        }));
        assert!(
            report
                .files
                .iter()
                .all(|file| file.path != "types/navigation_profile.go")
        );
    }

    #[test]
    fn collapses_leaf_file_when_scope_already_names_the_type() {
        let facet = item(
            uuid!("aaaaaaaa-aaaa-aaaa-aaaa-aaaaaaaaaaaa"),
            "Facet",
            Vec::new(),
        );

        let report =
            GoStandaloneLayoutReport::from_codegen_unit(&crate::ir::SerializeCodegenUnit {
                items: vec![facet],
            });

        assert!(
            report
                .files
                .iter()
                .any(|file| { file.path == "types/facet.go" && file.package_name == "types" })
        );
        assert!(
            report
                .files
                .iter()
                .all(|file| { file.path != "types/facet/facet.go" })
        );
    }

    #[test]
    fn root_leaf_type_does_not_create_stuttering_package_directory() {
        let aabb = item(
            uuid!("aaaaaaaa-aaaa-aaaa-aaaa-aaaaaaaaaaaa"),
            "Aabb",
            Vec::new(),
        );

        let report =
            GoStandaloneLayoutReport::from_codegen_unit(&crate::ir::SerializeCodegenUnit {
                items: vec![aabb],
            });

        assert!(
            report
                .files
                .iter()
                .any(|file| { file.path == "types/aabb.go" && file.package_name == "types" })
        );
        assert!(
            report
                .files
                .iter()
                .all(|file| { file.path != "types/aabb/aabb.go" })
        );
    }

    #[test]
    fn splits_root_package_cycles_and_same_name_family_leaf_collisions() {
        let item_family_id = uuid!("b9f3747d-192b-5eda-606d-737d339a9679");
        let item_record_id = uuid!("a6d8db05-cc68-4fbe-8002-55c0c7b1fd08");
        let ammo_id = uuid!("11111111-1111-1111-1111-111111111111");
        let descriptor_id = uuid!("22222222-2222-2222-2222-222222222222");
        let slot_id = uuid!("33333333-3333-3333-3333-333333333333");
        let other_slot_id = uuid!("44444444-4444-4444-4444-444444444444");
        let version_data_id = uuid!("55555555-5555-5555-5555-555555555555");

        let report =
            GoStandaloneLayoutReport::from_codegen_unit(&crate::ir::SerializeCodegenUnit {
                items: vec![
                    item(
                        item_family_id,
                        "Item",
                        vec![value_field("m_descriptor", descriptor_id, "ItemDescriptor")],
                    ),
                    item(ammo_id, "Ammo", vec![base_field(item_family_id, "Item")]),
                    item(
                        item_record_id,
                        "Item",
                        vec![value_field(
                            "m_currentVersion",
                            version_data_id,
                            "ItemVersionData",
                        )],
                    ),
                    item(
                        version_data_id,
                        "ItemVersionData",
                        vec![value_field("m_childItem", item_record_id, "Item")],
                    ),
                    item(descriptor_id, "ItemDescriptor", Vec::new()),
                    item(
                        slot_id,
                        "ItemContainerSlot",
                        vec![value_field("m_item", item_family_id, "Item")],
                    ),
                    item(
                        other_slot_id,
                        "OtherItemContainerSlot",
                        vec![value_field("m_item", item_family_id, "Item")],
                    ),
                ],
            });

        let text = report.to_text();
        assert!(
            report.files.iter().any(|file| {
                file.path == "types/item/item.go"
                    && file.items.iter().any(|item| item.type_id == item_family_id)
            }),
            "{text}"
        );
        assert!(
            report.files.iter().any(|file| {
                file.path == "types/item/item/item.go"
                    && file.items.iter().any(|item| item.type_id == item_record_id)
            }),
            "{text}"
        );
        assert!(
            report.files.iter().any(|file| {
                file.path == "types/item/item/item_version_data.go"
                    && file
                        .items
                        .iter()
                        .any(|item| item.type_id == version_data_id)
            }),
            "{text}"
        );
        assert!(
            report.files.iter().any(|file| {
                file.path == "types/item/item_descriptor.go"
                    && file.items.iter().any(|item| item.type_id == descriptor_id)
            }),
            "{text}"
        );
        assert!(
            report.files.iter().all(|file| file.path != "types/item.go"),
            "{text}"
        );
    }

    #[test]
    fn type_components_follow_rtti_base_chain_dependencies() {
        let base_id = uuid!("aaaaaaaa-aaaa-aaaa-aaaa-aaaaaaaaaaaa");
        let derived_id = uuid!("bbbbbbbb-bbbb-bbbb-bbbb-bbbbbbbbbbbb");
        let mut derived = item(derived_id, "Example::Derived", Vec::new());
        derived.rtti_base_chain = vec![SerializeCodegenRttiBase {
            type_id: base_id,
            source_name: "Example::Base".to_owned(),
        }];
        let base = item(
            base_id,
            "Example::Base",
            vec![value_field("m_derived", derived_id, "Example::Derived")],
        );
        let unit = SerializeCodegenUnit {
            items: vec![derived, base],
        };

        let components = type_components_by_id(&unit);

        assert_eq!(components.get(&base_id), Some(&vec![base_id, derived_id]));
        assert_eq!(
            components.get(&derived_id),
            Some(&vec![base_id, derived_id])
        );
    }

    #[test]
    fn package_graph_follows_rtti_base_chain_dependencies() {
        let base_id = uuid!("aaaaaaaa-aaaa-aaaa-aaaa-aaaaaaaaaaaa");
        let derived_id = uuid!("bbbbbbbb-bbbb-bbbb-bbbb-bbbbbbbbbbbb");
        let mut derived = item(derived_id, "Example::Derived", Vec::new());
        derived.rtti_base_chain = vec![SerializeCodegenRttiBase {
            type_id: base_id,
            source_name: "Example::Base".to_owned(),
        }];
        let base = item(base_id, "Example::Base", Vec::new());
        let unit = SerializeCodegenUnit {
            items: vec![derived, base],
        };
        let derived_package = GoTypePackage {
            dir: "types/derived".to_owned(),
            name: "derived".to_owned(),
        };
        let base_package = GoTypePackage {
            dir: "types/base".to_owned(),
            name: "base".to_owned(),
        };
        let packages_by_type_id = BTreeMap::from([
            (derived_id, derived_package.clone()),
            (base_id, base_package.clone()),
        ]);

        let graph = go_package_dependency_graph(&unit, &packages_by_type_id);

        assert!(
            graph
                .get(&derived_package)
                .is_some_and(|targets| targets.contains(&base_package))
        );
    }

    fn item(
        type_id: uuid::Uuid,
        name: &str,
        fields: Vec<SerializeCodegenField>,
    ) -> SerializeCodegenItem {
        SerializeCodegenItem {
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
        }
    }

    fn base_field(source_type_id: uuid::Uuid, source_name: &str) -> SerializeCodegenField {
        SerializeCodegenField {
            source_name: "BaseClass1".to_owned(),
            source_type_id,
            resolved_type: ResolvedType::Named {
                type_id: source_type_id,
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

    fn value_field(name: &str, type_id: uuid::Uuid, source_name: &str) -> SerializeCodegenField {
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
}

fn is_go_identifier(value: &str) -> bool {
    let mut chars = value.chars();
    let Some(first) = chars.next() else {
        return false;
    };
    (first == '_' || first.is_ascii_alphabetic())
        && chars.all(|ch| ch == '_' || ch.is_ascii_alphanumeric())
}

fn is_go_keyword(value: &str) -> bool {
    matches!(
        value,
        "break"
            | "default"
            | "func"
            | "interface"
            | "select"
            | "case"
            | "defer"
            | "go"
            | "map"
            | "struct"
            | "chan"
            | "else"
            | "goto"
            | "package"
            | "switch"
            | "const"
            | "fallthrough"
            | "if"
            | "range"
            | "type"
            | "continue"
            | "for"
            | "import"
            | "return"
            | "var"
    )
}
