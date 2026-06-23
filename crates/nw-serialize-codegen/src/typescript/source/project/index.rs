use std::collections::{BTreeMap, BTreeSet};

use crate::ir::{SerializeCodegenItem, SerializeCodegenItemKind, SerializeCodegenUnit};
use crate::symbol_surface::{SymbolSurfaceExport, SymbolSurfaceInput, SymbolSurfaceModule};
use crate::typescript::layout::TypeScriptTypeFile;

use super::super::{typescript_rtti_const_name, typescript_string_literal};
use super::type_file::typescript_type_name;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct TypeScriptIndexReexport {
    module_path: String,
    exports: BTreeMap<String, TypeScriptExportKind>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TypeScriptExportKind {
    TypeOnly,
    Value,
}

pub(super) fn typescript_index_reexports_by_dir(
    unit: &SerializeCodegenUnit,
    groups: &BTreeMap<(TypeScriptTypeFile, String), Vec<&SerializeCodegenItem>>,
    names_by_type_id: &BTreeMap<uuid::Uuid, String>,
) -> BTreeMap<String, Vec<TypeScriptIndexReexport>> {
    let direct_reexports_by_path =
        direct_typescript_index_reexports(unit, groups, names_by_type_id);
    let child_modules_by_path = typescript_index_child_dirs(&direct_reexports_by_path);
    let symbol_surface = crate::symbol_surface::plan_symbol_surface(SymbolSurfaceInput {
        root_path: vec!["types".to_owned()],
        local_symbols_by_path: BTreeMap::new(),
        direct_reexports_by_path,
        child_modules_by_path,
    });
    typescript_index_reexports_from_symbol_surface(&symbol_surface)
}

fn direct_typescript_index_reexports(
    _unit: &SerializeCodegenUnit,
    groups: &BTreeMap<(TypeScriptTypeFile, String), Vec<&SerializeCodegenItem>>,
    names_by_type_id: &BTreeMap<uuid::Uuid, String>,
) -> BTreeMap<Vec<String>, Vec<SymbolSurfaceExport<TypeScriptExportKind>>> {
    let mut direct_reexports =
        BTreeMap::<Vec<String>, Vec<SymbolSurfaceExport<TypeScriptExportKind>>>::new();
    for ((type_file, _bucket), items) in groups {
        direct_reexports
            .entry(typescript_slash_path_segments(&type_file.dir))
            .or_default()
            .push(SymbolSurfaceExport {
                module: format!("./{}.js", type_file.file_stem),
                symbols: items
                    .iter()
                    .flat_map(|item| typescript_export_symbols(item, names_by_type_id))
                    .collect(),
            });
    }
    direct_reexports
}

fn typescript_export_symbols(
    item: &SerializeCodegenItem,
    names_by_type_id: &BTreeMap<uuid::Uuid, String>,
) -> Vec<(String, TypeScriptExportKind)> {
    let type_name = typescript_type_name(item, names_by_type_id);
    let mut symbols = vec![(
        typescript_rtti_const_name(&type_name),
        TypeScriptExportKind::Value,
    )];
    match item.kind {
        SerializeCodegenItemKind::Struct => {
            symbols.push((type_name, TypeScriptExportKind::Value));
        }
        SerializeCodegenItemKind::Enum => {
            symbols.push((type_name, TypeScriptExportKind::Value));
        }
    }
    symbols
}

fn typescript_index_child_dirs(
    direct_reexports: &BTreeMap<Vec<String>, Vec<SymbolSurfaceExport<TypeScriptExportKind>>>,
) -> BTreeMap<Vec<String>, BTreeSet<String>> {
    let mut dirs = BTreeSet::from([vec!["types".to_owned()]]);
    for dir in direct_reexports.keys() {
        register_typescript_index_dir(dir, &mut dirs);
    }

    let mut child_dirs = BTreeMap::<Vec<String>, BTreeSet<String>>::new();
    for dir in dirs {
        let Some((child, parent)) = dir.split_last() else {
            continue;
        };
        if parent.is_empty() {
            child_dirs.entry(dir).or_default();
            continue;
        }
        child_dirs
            .entry(parent.to_vec())
            .or_default()
            .insert(child.clone());
        child_dirs.entry(dir).or_default();
    }
    child_dirs
}

fn register_typescript_index_dir(dir: &[String], dirs: &mut BTreeSet<Vec<String>>) {
    let mut segments = dir.to_vec();
    while !segments.is_empty() {
        dirs.insert(segments.clone());
        segments.pop();
    }
}

fn typescript_slash_path_segments(path: &str) -> Vec<String> {
    path.split('/').map(str::to_owned).collect()
}

fn typescript_index_reexports_from_symbol_surface(
    symbol_surface: &BTreeMap<Vec<String>, SymbolSurfaceModule<TypeScriptExportKind>>,
) -> BTreeMap<String, Vec<TypeScriptIndexReexport>> {
    symbol_surface
        .iter()
        .map(|(path, module)| {
            (
                path.join("/"),
                module
                    .reexports
                    .iter()
                    .map(|reexport| TypeScriptIndexReexport {
                        module_path: typescript_index_module_path(&reexport.module),
                        exports: reexport.symbols.clone(),
                    })
                    .collect(),
            )
        })
        .collect()
}

fn typescript_index_module_path(module: &str) -> String {
    if module.starts_with("./") {
        module.to_owned()
    } else {
        format!("./{module}/index.js")
    }
}

pub(super) fn append_typescript_index_reexport(
    out: &mut String,
    reexport: &TypeScriptIndexReexport,
) {
    let type_only_names = reexport
        .exports
        .iter()
        .filter_map(|(name, kind)| (*kind == TypeScriptExportKind::TypeOnly).then_some(name))
        .cloned()
        .collect::<Vec<_>>();
    let value_names = reexport
        .exports
        .iter()
        .filter_map(|(name, kind)| (*kind == TypeScriptExportKind::Value).then_some(name))
        .cloned()
        .collect::<Vec<_>>();
    append_typescript_index_reexport_group(out, "export type", &type_only_names, reexport);
    append_typescript_index_reexport_group(out, "export", &value_names, reexport);
}

fn append_typescript_index_reexport_group(
    out: &mut String,
    keyword: &str,
    names: &[String],
    reexport: &TypeScriptIndexReexport,
) {
    if names.is_empty() {
        return;
    }
    out.push_str(keyword);
    out.push_str(" { ");
    out.push_str(&names.join(", "));
    out.push_str(" } from ");
    out.push_str(&typescript_string_literal(&reexport.module_path));
    out.push_str(";\n");
}
