use std::collections::{BTreeMap, BTreeSet};

use heck::ToSnakeCase;

use crate::ir::{SerializeCodegenItem, SerializeCodegenUnit};
use crate::layout::{
    LayoutIndex, LayoutPathSet, dependency_ordered_codegen_items, reflected_base_type_ids,
};
use crate::naming::rust_type_ident;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TypeScriptStandaloneLayoutReport {
    pub type_files: Vec<TypeScriptStandaloneTypeFileReport>,
    pub index_files: Vec<TypeScriptStandaloneIndexFileReport>,
}

impl TypeScriptStandaloneLayoutReport {
    #[must_use]
    pub fn from_codegen_unit(unit: &SerializeCodegenUnit) -> Self {
        Self::from_codegen_unit_with_context(unit, unit)
    }

    #[must_use]
    pub fn from_codegen_unit_with_context(
        emitted_unit: &SerializeCodegenUnit,
        context_unit: &SerializeCodegenUnit,
    ) -> Self {
        let layout = standalone_type_layout_with_context(emitted_unit, context_unit);
        let mut type_files = Vec::new();
        for ((type_file, _bucket), items) in layout.groups {
            let mut report_items = Vec::with_capacity(items.len());
            for item in items {
                report_items.push(layout_item_report(item));
            }
            type_files.push(TypeScriptStandaloneTypeFileReport {
                path: typescript_type_source_path(&type_file),
                dir: type_file.dir,
                file_stem: type_file.file_stem,
                items: report_items,
            });
        }
        let mut index_files = Vec::new();
        for (dir, exports) in layout.exports_by_dir {
            index_files.push(TypeScriptStandaloneIndexFileReport {
                path: format!("src/{dir}/index.ts"),
                dir,
                exports: exports.into_iter().collect(),
            });
        }
        Self {
            type_files,
            index_files,
        }
    }

    #[must_use]
    pub fn to_text(&self) -> String {
        let mut out = String::new();
        for file in &self.type_files {
            out.push_str("file ");
            out.push_str(&file.path);
            out.push('\n');
            for item in &file.items {
                append_layout_item(&mut out, item);
            }
        }
        for file in &self.index_files {
            out.push_str("index ");
            out.push_str(&file.path);
            out.push('\n');
            for export in &file.exports {
                out.push_str("  export ");
                out.push_str(export);
                out.push('\n');
            }
        }
        out
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TypeScriptStandaloneTypeFileReport {
    pub path: String,
    pub dir: String,
    pub file_stem: String,
    pub items: Vec<TypeScriptStandaloneLayoutItemReport>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TypeScriptStandaloneIndexFileReport {
    pub path: String,
    pub dir: String,
    pub exports: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TypeScriptStandaloneLayoutItemReport {
    pub type_id: uuid::Uuid,
    pub type_name: String,
    pub source_name: String,
}

#[derive(Debug)]
pub(super) struct TypeScriptStandaloneTypeLayout<'a> {
    pub(super) files_by_type_id: BTreeMap<uuid::Uuid, TypeScriptTypeFile>,
    pub(super) groups: BTreeMap<(TypeScriptTypeFile, String), Vec<&'a SerializeCodegenItem>>,
    pub(super) exports_by_dir: BTreeMap<String, BTreeSet<String>>,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub(super) struct TypeScriptTypeFile {
    pub(super) dir: String,
    pub(super) file_stem: String,
}

pub(super) fn standalone_type_layout_with_context<'a>(
    emitted_unit: &'a SerializeCodegenUnit,
    context_unit: &'a SerializeCodegenUnit,
) -> TypeScriptStandaloneTypeLayout<'a> {
    let items_by_type_id = items_by_type_id(context_unit);
    debug_assert!(
        emitted_unit
            .items
            .iter()
            .all(|item| items_by_type_id.contains_key(&item.source_type_id))
    );
    let layout_index = LayoutIndex::from_codegen_unit(context_unit);
    let base_type_ids = reflected_base_type_ids(context_unit);
    let files_by_type_id = typescript_type_files_by_id_with_context(
        emitted_unit,
        context_unit,
        &items_by_type_id,
        &base_type_ids,
        &layout_index,
    );
    let groups = typescript_type_file_groups(emitted_unit, &files_by_type_id);
    let mut exports_by_dir = BTreeMap::<String, BTreeSet<String>>::new();

    for (type_file, _bucket) in groups.keys() {
        exports_by_dir
            .entry(type_file.dir.clone())
            .or_default()
            .insert(format!("./{}.js", type_file.file_stem));
        register_typescript_index_exports(&type_file.dir, &mut exports_by_dir);
    }
    exports_by_dir.entry("types".to_owned()).or_default();

    TypeScriptStandaloneTypeLayout {
        files_by_type_id,
        groups,
        exports_by_dir,
    }
}

pub(super) fn items_by_type_id(
    unit: &SerializeCodegenUnit,
) -> BTreeMap<uuid::Uuid, &SerializeCodegenItem> {
    unit.index().into_items_by_type_id()
}

pub(super) fn typescript_type_source_path(type_file: &TypeScriptTypeFile) -> String {
    format!("src/{}/{}.ts", type_file.dir, type_file.file_stem)
}

fn typescript_type_files_by_id(
    unit: &SerializeCodegenUnit,
    items_by_type_id: &BTreeMap<uuid::Uuid, &SerializeCodegenItem>,
    base_type_ids: &BTreeSet<uuid::Uuid>,
    layout_index: &LayoutIndex,
) -> BTreeMap<uuid::Uuid, TypeScriptTypeFile> {
    let mut files_by_type_id = BTreeMap::new();
    for item in dependency_ordered_codegen_items(unit)
        .into_iter()
        .filter(|item| !item.is_reflection_marker)
    {
        let file = typescript_type_file(item, items_by_type_id, base_type_ids, layout_index);
        files_by_type_id.insert(item.source_type_id, file);
    }
    promote_parent_files_to_child_dirs(unit, &mut files_by_type_id);
    uniquify_typescript_type_files(unit, files_by_type_id)
}

fn typescript_type_files_by_id_with_context(
    emitted_unit: &SerializeCodegenUnit,
    context_unit: &SerializeCodegenUnit,
    context_items_by_type_id: &BTreeMap<uuid::Uuid, &SerializeCodegenItem>,
    context_base_type_ids: &BTreeSet<uuid::Uuid>,
    context_layout_index: &LayoutIndex,
) -> BTreeMap<uuid::Uuid, TypeScriptTypeFile> {
    let emitted_type_ids = emitted_unit
        .items
        .iter()
        .map(|item| item.source_type_id)
        .collect::<BTreeSet<_>>();
    typescript_type_files_by_id(
        context_unit,
        context_items_by_type_id,
        context_base_type_ids,
        context_layout_index,
    )
    .into_iter()
    .filter(|(type_id, _)| emitted_type_ids.contains(type_id))
    .collect()
}

fn promote_parent_files_to_child_dirs(
    unit: &SerializeCodegenUnit,
    files_by_type_id: &mut BTreeMap<uuid::Uuid, TypeScriptTypeFile>,
) {
    let item_index = items_by_type_id(unit);
    let layout_index = LayoutIndex::from_codegen_unit(unit);
    let file_paths = LayoutPathSet::from_paths(
        files_by_type_id
            .values()
            .map(|file| slash_path_segments(&file.dir)),
    );

    for item in dependency_ordered_codegen_items(unit)
        .into_iter()
        .filter(|item| !item.is_reflection_marker)
    {
        let Some(file) = files_by_type_id.get(&item.source_type_id).cloned() else {
            continue;
        };
        let file_stem =
            sanitize_typescript_path_segment(&layout_index.type_path(item, &item_index).file_stem);
        let mut child_path = slash_path_segments(&file.dir);
        child_path.push(file_stem.clone());
        if file_paths.contains_self_or_descendant(&child_path) {
            files_by_type_id.insert(
                item.source_type_id,
                TypeScriptTypeFile {
                    dir: child_path.join("/"),
                    file_stem,
                },
            );
        }
    }
}

fn uniquify_typescript_type_files(
    unit: &SerializeCodegenUnit,
    mut raw_files_by_type_id: BTreeMap<uuid::Uuid, TypeScriptTypeFile>,
) -> BTreeMap<uuid::Uuid, TypeScriptTypeFile> {
    let mut files_by_type_id = BTreeMap::new();
    let mut occupied = BTreeSet::<TypeScriptTypeFile>::new();
    for item in dependency_ordered_codegen_items(unit)
        .into_iter()
        .filter(|item| !item.is_reflection_marker)
    {
        let Some(mut file) = raw_files_by_type_id.remove(&item.source_type_id) else {
            continue;
        };
        while occupied.contains(&file) {
            file.file_stem.push('_');
            file.file_stem
                .push_str(&type_id_suffix(item.source_type_id));
        }
        occupied.insert(file.clone());
        files_by_type_id.insert(item.source_type_id, file);
    }
    files_by_type_id
}

fn typescript_type_file_groups<'a>(
    unit: &'a SerializeCodegenUnit,
    files_by_type_id: &BTreeMap<uuid::Uuid, TypeScriptTypeFile>,
) -> BTreeMap<(TypeScriptTypeFile, String), Vec<&'a SerializeCodegenItem>> {
    let mut groups = BTreeMap::<(TypeScriptTypeFile, String), Vec<&SerializeCodegenItem>>::new();
    for item in dependency_ordered_codegen_items(unit)
        .into_iter()
        .filter(|item| !item.is_reflection_marker)
    {
        let file = files_by_type_id
            .get(&item.source_type_id)
            .cloned()
            .expect("emitted TypeScript item must exist in the full context layout");
        groups
            .entry((file.clone(), file.file_stem.clone()))
            .or_default()
            .push(item);
    }
    groups
}

fn typescript_type_file(
    item: &SerializeCodegenItem,
    items_by_type_id: &BTreeMap<uuid::Uuid, &SerializeCodegenItem>,
    base_type_ids: &BTreeSet<uuid::Uuid>,
    layout_index: &LayoutIndex,
) -> TypeScriptTypeFile {
    let _ = base_type_ids;
    let type_path = layout_index.type_path(item, items_by_type_id);
    let mut segments = type_path
        .scope_segments
        .into_iter()
        .map(|segment| sanitize_typescript_path_segment(&segment))
        .collect::<Vec<_>>();
    let file_stem = sanitize_typescript_path_segment(&type_path.file_stem);
    collapse_duplicate_file_scope(&mut segments, &file_stem);
    let dir = if segments.is_empty() {
        "types".to_owned()
    } else {
        format!("types/{}", segments.join("/"))
    };
    TypeScriptTypeFile { dir, file_stem }
}

fn collapse_duplicate_file_scope(segments: &mut Vec<String>, file_stem: &str) {
    if segments.last().is_some_and(|segment| segment == file_stem) {
        segments.pop();
    }
}

fn layout_item_report(item: &SerializeCodegenItem) -> TypeScriptStandaloneLayoutItemReport {
    TypeScriptStandaloneLayoutItemReport {
        type_id: item.source_type_id,
        type_name: rust_type_ident(&item.source_name),
        source_name: item.source_name.clone(),
    }
}

fn append_layout_item(out: &mut String, item: &TypeScriptStandaloneLayoutItemReport) {
    out.push_str("  item ");
    out.push_str(&item.type_name);
    out.push(' ');
    out.push_str(&item.type_id.hyphenated().to_string());
    out.push(' ');
    out.push_str(&item.source_name);
    out.push('\n');
}

fn register_typescript_index_exports(
    dir: &str,
    exports_by_dir: &mut BTreeMap<String, BTreeSet<String>>,
) {
    let mut parts = dir.split('/').collect::<Vec<_>>();
    while parts.len() > 1 {
        let child = parts.pop().expect("checked length");
        let parent = parts.join("/");
        exports_by_dir
            .entry(parent)
            .or_default()
            .insert(format!("./{child}/index.js"));
    }
    exports_by_dir.entry("types".to_owned()).or_default();
}

fn slash_path_segments(path: &str) -> Vec<String> {
    path.split('/').map(str::to_owned).collect()
}

fn sanitize_typescript_path_segment(segment: &str) -> String {
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
    if name.is_empty() {
        "types".to_owned()
    } else {
        name.to_owned()
    }
}

fn type_id_suffix(type_id: uuid::Uuid) -> String {
    type_id.as_simple().to_string().chars().take(8).collect()
}

#[cfg(test)]
mod tests {
    use uuid::uuid;

    use crate::ir::{
        SerializeCodegenField, SerializeCodegenItem, SerializeCodegenItemKind,
        SerializeCodegenUnit, SerializeCodegenVariant,
    };
    use crate::role::ReflectedTypeRole;
    use crate::types::ResolvedType;

    use super::TypeScriptStandaloneLayoutReport;

    #[test]
    fn promotes_parent_file_into_child_dir_when_selected_children_use_that_family() {
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

        let report = TypeScriptStandaloneLayoutReport::from_codegen_unit(&SerializeCodegenUnit {
            items: vec![owner, child],
        });

        assert!(
            report
                .type_files
                .iter()
                .any(|file| { file.path == "src/types/navigation_profile/navigation_profile.ts" })
        );
        assert!(
            report
                .type_files
                .iter()
                .any(|file| file.path == "src/types/navigation_profile/nav_configuration.ts")
        );
        assert!(
            report
                .type_files
                .iter()
                .all(|file| file.path != "src/types/navigation_profile.ts")
        );
    }

    #[test]
    fn collapses_leaf_file_when_scope_already_names_the_type() {
        let facet = item(
            uuid!("aaaaaaaa-aaaa-aaaa-aaaa-aaaaaaaaaaaa"),
            "Facet",
            Vec::new(),
        );

        let report = TypeScriptStandaloneLayoutReport::from_codegen_unit(&SerializeCodegenUnit {
            items: vec![facet],
        });

        assert!(
            report
                .type_files
                .iter()
                .any(|file| { file.path == "src/types/facet.ts" && file.dir == "types" })
        );
        assert!(
            report
                .type_files
                .iter()
                .all(|file| file.path != "src/types/facet/facet.ts")
        );
    }

    #[test]
    fn root_leaf_type_does_not_create_stuttering_directory() {
        let aabb = item(
            uuid!("aaaaaaaa-aaaa-aaaa-aaaa-aaaaaaaaaaaa"),
            "Aabb",
            Vec::new(),
        );

        let report = TypeScriptStandaloneLayoutReport::from_codegen_unit(&SerializeCodegenUnit {
            items: vec![aabb],
        });

        assert!(
            report
                .type_files
                .iter()
                .any(|file| file.path == "src/types/aabb.ts" && file.dir == "types")
        );
        assert!(
            report
                .type_files
                .iter()
                .all(|file| file.path != "src/types/aabb/aabb.ts")
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
