use std::collections::{BTreeMap, BTreeSet};

use crate::layout::{LayoutPathSet, layout_path_starts_with};
use crate::naming::rust_module_ident;
use crate::rust::item_plan::{RustCodegenUnit, RustItemPlan};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RustStandaloneLayoutReport {
    pub modules: Vec<RustStandaloneLayoutModuleReport>,
    pub files: Vec<RustStandaloneLayoutFileReport>,
}

impl RustStandaloneLayoutReport {
    #[must_use]
    pub fn from_codegen_unit(unit: &RustCodegenUnit) -> Self {
        standalone_type_layout_report(unit)
    }

    #[must_use]
    pub fn to_text(&self) -> String {
        let mut out = String::new();
        for module in &self.modules {
            out.push_str("module ");
            out.push_str(&module.path);
            out.push('\n');
            for child in &module.child_dirs {
                out.push_str("  child ");
                out.push_str(child);
                out.push('\n');
            }
            for leaf in &module.leaf_modules {
                out.push_str("  leaf ");
                out.push_str(leaf);
                out.push('\n');
            }
            for item in &module.items {
                append_layout_item(&mut out, item);
            }
        }
        for file in &self.files {
            out.push_str("file ");
            out.push_str(&file.path);
            out.push('\n');
            for item in &file.items {
                append_layout_item(&mut out, item);
            }
        }
        out
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RustStandaloneLayoutModuleReport {
    pub path: String,
    pub child_dirs: Vec<String>,
    pub leaf_modules: Vec<String>,
    pub items: Vec<RustStandaloneLayoutItemReport>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RustStandaloneLayoutFileReport {
    pub path: String,
    pub scope_path: Vec<String>,
    pub file_stem: String,
    pub items: Vec<RustStandaloneLayoutItemReport>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RustStandaloneLayoutItemReport {
    pub type_id: uuid::Uuid,
    pub rust_name: String,
    pub source_name: String,
    pub identity_kind: String,
}

#[derive(Debug, Default)]
pub(super) struct RustStandaloneTypeLayout<'a> {
    pub(super) module_groups: BTreeMap<Vec<String>, Vec<&'a RustItemPlan>>,
    pub(super) file_groups: BTreeMap<(Vec<String>, String), Vec<&'a RustItemPlan>>,
}

#[derive(Debug, Default)]
pub(super) struct RustStandaloneModuleNode {
    pub(super) child_dirs: BTreeSet<String>,
    pub(super) leaf_modules: BTreeSet<String>,
}

pub(super) fn standalone_type_layout(unit: &RustCodegenUnit) -> RustStandaloneTypeLayout<'_> {
    let mut layout = RustStandaloneTypeLayout::default();
    for item in &unit.items {
        if item.is_reflected_base || item.is_slot_owner || item.has_layout_family_descendants {
            let module_path = rust_family_scope_path(item);
            layout
                .module_groups
                .entry(module_path)
                .or_default()
                .push(item);
        } else {
            let mut scope_path = rust_scope_path(item);
            let mut file_stem = item
                .file_stem_override
                .as_deref()
                .map(sanitize_rust_module_name)
                .unwrap_or_else(|| generated_type_module_name(item));
            collapse_duplicate_file_scope(&mut scope_path, &file_stem);
            if layout
                .file_groups
                .contains_key(&(scope_path.clone(), file_stem.clone()))
            {
                file_stem.push('_');
                file_stem.push_str(&type_id_suffix(item.source_type_id));
            }
            layout
                .file_groups
                .entry((scope_path, file_stem))
                .or_default()
                .push(item);
        }
    }
    promote_file_groups_with_child_modules(&mut layout);
    demote_leaf_module_groups_without_child_paths(&mut layout);
    layout
}

fn collapse_duplicate_file_scope(scope_path: &mut Vec<String>, file_stem: &str) {
    if scope_path
        .last()
        .is_some_and(|segment| segment == file_stem)
    {
        scope_path.pop();
    }
}

fn promote_file_groups_with_child_modules(layout: &mut RustStandaloneTypeLayout<'_>) {
    let directory_paths = directory_paths_for_layout(layout);
    let file_groups = std::mem::take(&mut layout.file_groups);
    for ((scope_path, file_stem), items) in file_groups {
        let mut module_path = scope_path.clone();
        module_path.push(file_stem.clone());
        if directory_paths.contains(&module_path) {
            if promoted_items_collide_with_module_items(layout, &module_path, &items) {
                let mut nested_file_stem = nested_collision_file_stem(&module_path, &file_stem);
                if layout
                    .file_groups
                    .contains_key(&(module_path.clone(), nested_file_stem.clone()))
                {
                    nested_file_stem.push('_');
                    nested_file_stem.push_str(&type_id_suffix(items[0].source_type_id));
                }
                layout
                    .file_groups
                    .entry((module_path, nested_file_stem))
                    .or_default()
                    .extend(items);
            } else {
                layout
                    .module_groups
                    .entry(module_path)
                    .or_default()
                    .extend(items);
            }
        } else {
            layout.file_groups.insert((scope_path, file_stem), items);
        }
    }
}

pub(super) fn nested_collision_file_stem(module_path: &[String], file_stem: &str) -> String {
    if module_path
        .last()
        .is_some_and(|segment| segment == file_stem)
    {
        format!("{file_stem}_type")
    } else {
        file_stem.to_owned()
    }
}

fn promoted_items_collide_with_module_items(
    layout: &RustStandaloneTypeLayout<'_>,
    module_path: &[String],
    promoted_items: &[&RustItemPlan],
) -> bool {
    let Some(module_items) = layout.module_groups.get(module_path) else {
        return false;
    };
    module_items.iter().any(|module_item| {
        promoted_items
            .iter()
            .any(|promoted_item| module_item.rust_name == promoted_item.rust_name)
    })
}

fn directory_paths_for_layout(layout: &RustStandaloneTypeLayout<'_>) -> LayoutPathSet {
    LayoutPathSet::from_directory_prefixes(
        layout.module_groups.keys().cloned().chain(
            layout
                .file_groups
                .keys()
                .map(|(scope_path, _)| scope_path.clone()),
        ),
    )
}

fn demote_leaf_module_groups_without_child_paths(layout: &mut RustStandaloneTypeLayout<'_>) {
    let module_paths = layout
        .module_groups
        .keys()
        .cloned()
        .collect::<BTreeSet<_>>();
    let file_scope_paths = layout
        .file_groups
        .keys()
        .map(|(scope_path, _)| scope_path.clone())
        .collect::<BTreeSet<_>>();

    let module_groups = std::mem::take(&mut layout.module_groups);
    for (module_path, items) in module_groups {
        if can_demote_leaf_module_group(&module_path, &items, &module_paths, &file_scope_paths) {
            let (scope_path, mut file_stem) = demoted_module_file_target(&module_path, items[0]);
            if layout
                .file_groups
                .contains_key(&(scope_path.clone(), file_stem.clone()))
            {
                file_stem.push('_');
                file_stem.push_str(&type_id_suffix(items[0].source_type_id));
            }
            layout
                .file_groups
                .entry((scope_path, file_stem))
                .or_default()
                .extend(items);
        } else {
            layout.module_groups.insert(module_path, items);
        }
    }
}

fn can_demote_leaf_module_group(
    module_path: &[String],
    items: &[&RustItemPlan],
    module_paths: &BTreeSet<Vec<String>>,
    file_scope_paths: &BTreeSet<Vec<String>>,
) -> bool {
    !module_path.is_empty()
        && items.len() == 1
        && !items[0].has_layout_family_descendants
        && !module_paths.iter().any(|path| {
            path.len() > module_path.len() && layout_path_starts_with(path, module_path)
        })
        && !file_scope_paths
            .iter()
            .any(|path| layout_path_starts_with(path, module_path))
}

fn demoted_module_file_target(
    module_path: &[String],
    item: &RustItemPlan,
) -> (Vec<String>, String) {
    let file_stem = generated_type_module_name(item);
    let scope_path = if module_path
        .last()
        .is_some_and(|segment| is_family_module_segment(segment, &file_stem))
    {
        module_path[..module_path.len() - 1].to_vec()
    } else {
        module_path.to_vec()
    };
    (scope_path, file_stem)
}

fn is_family_module_segment(segment: &str, file_stem: &str) -> bool {
    segment == file_stem
        || segment == format!("{file_stem}s")
        || file_stem
            .strip_suffix('y')
            .is_some_and(|prefix| segment == format!("{prefix}ies"))
}

pub(super) fn standalone_type_module_tree(
    layout: &RustStandaloneTypeLayout<'_>,
) -> BTreeMap<Vec<String>, RustStandaloneModuleNode> {
    let mut tree = BTreeMap::<Vec<String>, RustStandaloneModuleNode>::new();
    tree.entry(Vec::new()).or_default();

    for module_path in layout.module_groups.keys() {
        for depth in 0..module_path.len() {
            let parent = module_path[..depth].to_vec();
            let child = module_path[depth].clone();
            tree.entry(parent).or_default().child_dirs.insert(child);
            tree.entry(module_path[..=depth].to_vec()).or_default();
        }
    }

    for (scope_path, file_stem) in layout.file_groups.keys() {
        for depth in 0..scope_path.len() {
            let parent = scope_path[..depth].to_vec();
            let child = scope_path[depth].clone();
            tree.entry(parent).or_default().child_dirs.insert(child);
            tree.entry(scope_path[..=depth].to_vec()).or_default();
        }
        tree.entry(scope_path.clone())
            .or_default()
            .leaf_modules
            .insert(file_stem.clone());
    }

    tree
}

pub(super) fn standalone_module_file_path(module_path: &[String]) -> String {
    if module_path.is_empty() {
        "src/types/mod.rs".to_owned()
    } else {
        format!("src/types/{}/mod.rs", module_path.join("/"))
    }
}

pub(super) fn standalone_item_file_path(scope_path: &[String], file_stem: &str) -> String {
    if scope_path.is_empty() {
        format!("src/types/{file_stem}.rs")
    } else {
        format!("src/types/{}/{}.rs", scope_path.join("/"), file_stem)
    }
}

fn standalone_type_layout_report(unit: &RustCodegenUnit) -> RustStandaloneLayoutReport {
    let layout = standalone_type_layout(unit);
    let module_tree = standalone_type_module_tree(&layout);
    let modules = module_tree
        .iter()
        .map(|(module_path, node)| RustStandaloneLayoutModuleReport {
            path: standalone_module_file_path(module_path),
            child_dirs: node.child_dirs.iter().cloned().collect(),
            leaf_modules: node.leaf_modules.iter().cloned().collect(),
            items: layout
                .module_groups
                .get(module_path)
                .map(|items| items.iter().map(|item| layout_item_report(item)).collect())
                .unwrap_or_default(),
        })
        .collect();
    let files = layout
        .file_groups
        .iter()
        .map(
            |((scope_path, file_stem), items)| RustStandaloneLayoutFileReport {
                path: standalone_item_file_path(scope_path, file_stem),
                scope_path: scope_path.clone(),
                file_stem: file_stem.clone(),
                items: items.iter().map(|item| layout_item_report(item)).collect(),
            },
        )
        .collect();
    RustStandaloneLayoutReport { modules, files }
}

fn layout_item_report(item: &RustItemPlan) -> RustStandaloneLayoutItemReport {
    RustStandaloneLayoutItemReport {
        type_id: item.source_type_id,
        rust_name: item.rust_name.clone(),
        source_name: item.source_name.clone(),
        identity_kind: format!("{:?}", item.identity.kind),
    }
}

fn append_layout_item(out: &mut String, item: &RustStandaloneLayoutItemReport) {
    out.push_str("  item ");
    out.push_str(&item.rust_name);
    out.push(' ');
    out.push_str(&item.type_id.hyphenated().to_string());
    out.push(' ');
    out.push_str(&item.source_name);
    out.push('\n');
}

fn rust_scope_path(item: &RustItemPlan) -> Vec<String> {
    item.scope_path
        .iter()
        .map(|segment| sanitize_rust_module_name(segment))
        .collect::<Vec<_>>()
}

fn rust_family_scope_path(item: &RustItemPlan) -> Vec<String> {
    let mut path = item
        .family_scope_path
        .iter()
        .map(|segment| sanitize_rust_module_name(segment))
        .collect::<Vec<_>>();
    if path.is_empty() {
        path.push(generated_type_module_name(item));
    }
    path
}

fn generated_type_module_name(item: &RustItemPlan) -> String {
    sanitize_rust_module_name(&item.rust_name)
}

fn type_id_suffix(type_id: uuid::Uuid) -> String {
    type_id.as_simple().to_string().chars().take(8).collect()
}

fn sanitize_rust_module_name(name: &str) -> String {
    rust_module_ident(name)
}

#[cfg(test)]
mod tests {
    use uuid::uuid;

    use crate::rust::identity::{RustTypeIdentityKind, RustTypeIdentityPlan};
    use crate::rust::item_plan::{RustCodegenUnit, RustItemKind, RustItemPlan};

    use super::RustStandaloneLayoutReport;

    #[test]
    fn promotes_leaf_file_to_module_when_owned_children_need_same_stem_directory() {
        let facet = item(
            "AudioSetTriggerOverrideComponentClientFacet",
            "AudioSetTriggerOverrideComponentClientFacet",
            vec![
                "components".to_owned(),
                "faceted_components".to_owned(),
                "audio_set_trigger_override_component".to_owned(),
            ],
            Some("client_facet".to_owned()),
        );
        let support = item(
            "TriggerOverridePair",
            "TriggerOverridePair",
            vec![
                "components".to_owned(),
                "faceted_components".to_owned(),
                "audio_set_trigger_override_component".to_owned(),
                "client_facet".to_owned(),
            ],
            None,
        );
        let report = RustStandaloneLayoutReport::from_codegen_unit(&RustCodegenUnit {
            items: vec![facet, support],
        });

        assert!(report.files.iter().all(|file| file.path
            != "src/types/components/faceted_components/audio_set_trigger_override_component/client_facet.rs"));
        assert!(report.modules.iter().any(|module| {
            module.path
                == "src/types/components/faceted_components/audio_set_trigger_override_component/client_facet/mod.rs"
                && module.items.iter().any(|item| item.rust_name
                    == "AudioSetTriggerOverrideComponentClientFacet")
        }));
        assert!(report.files.iter().any(|file| file.path
            == "src/types/components/faceted_components/audio_set_trigger_override_component/client_facet/trigger_override_pair.rs"));
    }

    #[test]
    fn keeps_promoted_leaf_in_child_file_when_family_module_has_same_type_name() {
        let item_base = family_item(
            "Item",
            "Item",
            vec!["item".to_owned()],
            uuid!("b9f3747d-192b-5eda-606d-737d339a9679"),
        );
        let item_record = item_with_id(
            "Item",
            "Item",
            Vec::new(),
            Some("item".to_owned()),
            uuid!("a6d8db05-cc68-4fbe-8002-55c0c7b1fd08"),
        );

        let report = RustStandaloneLayoutReport::from_codegen_unit(&RustCodegenUnit {
            items: vec![item_base, item_record],
        });

        assert!(report.modules.iter().any(|module| {
            module.path == "src/types/item/mod.rs"
                && module
                    .items
                    .iter()
                    .any(|item| item.type_id == uuid!("b9f3747d-192b-5eda-606d-737d339a9679"))
        }));
        assert!(report.files.iter().any(|file| {
            file.path == "src/types/item/item_type.rs"
                && file
                    .items
                    .iter()
                    .any(|item| item.type_id == uuid!("a6d8db05-cc68-4fbe-8002-55c0c7b1fd08"))
        }));
    }

    #[test]
    fn collapses_leaf_file_when_scope_already_names_the_type() {
        let edit_enum = item(
            "EditEnumItemClasses",
            "EditEnum<EnumType><Javelin::SBItemClass::ItemClasses >",
            vec![
                "javelin".to_owned(),
                "sb_item_class".to_owned(),
                "edit_enum_item_classes".to_owned(),
            ],
            None,
        );
        let report = RustStandaloneLayoutReport::from_codegen_unit(&RustCodegenUnit {
            items: vec![edit_enum],
        });

        assert!(report.modules.iter().all(|module| {
            module.path != "src/types/javelin/sb_item_class/edit_enum_item_classes/mod.rs"
        }));
        assert!(report.files.iter().any(|file| {
            file.path == "src/types/javelin/sb_item_class/edit_enum_item_classes.rs"
        }));
    }

    #[test]
    fn root_leaf_type_does_not_create_stuttering_module_directory() {
        let aabb = item(
            "Aabb",
            "Aabb",
            vec!["aabb".to_owned()],
            Some("aabb".to_owned()),
        );

        let report =
            RustStandaloneLayoutReport::from_codegen_unit(&RustCodegenUnit { items: vec![aabb] });

        assert!(
            report
                .files
                .iter()
                .any(|file| file.path == "src/types/aabb.rs")
        );
        assert!(
            report
                .files
                .iter()
                .all(|file| file.path != "src/types/aabb/aabb.rs")
        );
    }

    #[test]
    fn sanitizes_keyword_file_stem_overrides() {
        let item = item(
            "Override",
            "Example::Override",
            vec!["example".to_owned()],
            Some("override".to_owned()),
        );

        let report =
            RustStandaloneLayoutReport::from_codegen_unit(&RustCodegenUnit { items: vec![item] });

        assert!(report.files.iter().any(|file| {
            file.path == "src/types/example/override_.rs" && file.file_stem == "override_"
        }));
    }

    #[test]
    fn emits_reflected_base_without_selected_children_as_leaf_file() {
        let mut net_bindable = item(
            "NetBindable",
            "NetBindable",
            vec!["net_bindables".to_owned()],
            None,
        );
        net_bindable.is_reflected_base = true;
        net_bindable.scope_path = Vec::new();

        let report = RustStandaloneLayoutReport::from_codegen_unit(&RustCodegenUnit {
            items: vec![net_bindable],
        });

        assert!(
            report
                .modules
                .iter()
                .all(|module| module.path != "src/types/net_bindables/mod.rs")
        );
        assert!(
            report
                .files
                .iter()
                .any(|file| file.path == "src/types/net_bindable.rs")
        );
    }

    #[test]
    fn keeps_reflected_base_module_when_selected_children_need_family_directory() {
        let mut base = item(
            "BaseThing",
            "BaseThing",
            vec!["base_things".to_owned()],
            None,
        );
        base.is_reflected_base = true;
        base.scope_path = Vec::new();
        let child = item(
            "DerivedThing",
            "DerivedThing",
            vec!["base_things".to_owned()],
            None,
        );

        let report = RustStandaloneLayoutReport::from_codegen_unit(&RustCodegenUnit {
            items: vec![base, child],
        });

        assert!(report.modules.iter().any(|module| {
            module.path == "src/types/base_things/mod.rs"
                && module
                    .items
                    .iter()
                    .any(|item| item.rust_name == "BaseThing")
        }));
        assert!(
            report
                .files
                .iter()
                .any(|file| file.path == "src/types/base_things/derived_thing.rs")
        );
    }

    fn item(
        rust_name: &str,
        source_name: &str,
        scope_path: Vec<String>,
        file_stem_override: Option<String>,
    ) -> RustItemPlan {
        item_with_id(
            rust_name,
            source_name,
            scope_path,
            file_stem_override,
            uuid!("aaaaaaaa-aaaa-aaaa-aaaa-aaaaaaaaaaaa"),
        )
    }

    fn item_with_id(
        rust_name: &str,
        source_name: &str,
        scope_path: Vec<String>,
        file_stem_override: Option<String>,
        source_type_id: uuid::Uuid,
    ) -> RustItemPlan {
        RustItemPlan {
            source_type_id,
            source_name: source_name.to_owned(),
            is_reflected_base: false,
            is_slot_owner: false,
            has_layout_family_descendants: false,
            is_bevy_component: false,
            file_stem_override,
            family_scope_path: scope_path.clone(),
            scope_path,
            rust_name: rust_name.to_owned(),
            kind: RustItemKind::Struct,
            identity: RustTypeIdentityPlan {
                kind: RustTypeIdentityKind::AzRtti,
                type_id: source_type_id,
                name: Some(source_name.to_owned()),
            },
            repr: None,
            raw_conversion: None,
            derives: Vec::new(),
            rtti_bases: Vec::new(),
            fields: Vec::new(),
            variants: Vec::new(),
        }
    }

    fn family_item(
        rust_name: &str,
        source_name: &str,
        family_scope_path: Vec<String>,
        source_type_id: uuid::Uuid,
    ) -> RustItemPlan {
        let mut item = item_with_id(rust_name, source_name, Vec::new(), None, source_type_id);
        item.is_reflected_base = true;
        item.has_layout_family_descendants = true;
        item.family_scope_path = family_scope_path;
        item
    }
}
