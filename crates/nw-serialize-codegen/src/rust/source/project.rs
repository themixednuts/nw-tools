use std::collections::{BTreeMap, BTreeSet};

use quote::quote;
use syn::{GenericArgument, Ident, PathArguments, Type};

use crate::CodegenContext;
use crate::rust::item_plan::{RustCodegenUnit, RustItemPlan};
use crate::rust::layout::{
    standalone_item_file_path, standalone_module_file_path, standalone_type_layout,
    standalone_type_module_tree,
};
use crate::symbol_surface::{SymbolSurfaceExport, SymbolSurfaceInput, SymbolSurfaceModule};

use super::{
    RustSourceEmitError, RustSourceMode, RustSourceOptions, render_item, rustfmt_source, support,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RustStandaloneProject {
    pub files: Vec<RustStandaloneProjectFile>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RustStandaloneProjectFile {
    pub path: String,
    pub source: String,
}

pub(super) fn emit_standalone_project(
    unit: &RustCodegenUnit,
    context: &CodegenContext,
) -> Result<RustStandaloneProject, RustSourceEmitError> {
    let mut files = standalone_support_files()?;
    files.extend(emit_standalone_type_files(unit, context)?);
    Ok(RustStandaloneProject { files })
}

fn standalone_support_files() -> Result<Vec<RustStandaloneProjectFile>, RustSourceEmitError> {
    Ok(vec![
        RustStandaloneProjectFile {
            path: "src/lib.rs".to_owned(),
            source: rustfmt_source(
                "#![allow(clippy::struct_excessive_bools, clippy::zero_sized_map_values)]\n\npub mod az;\npub mod types;\n",
            )?,
        },
        RustStandaloneProjectFile {
            path: "src/az/mod.rs".to_owned(),
            source: rustfmt_source("pub mod asset;\npub mod crc;\npub mod rtti;\npub mod uuid;\n")?,
        },
        RustStandaloneProjectFile {
            path: "src/az/rtti.rs".to_owned(),
            source: rustfmt_source(support::rtti_module_source())?,
        },
        RustStandaloneProjectFile {
            path: "src/az/uuid.rs".to_owned(),
            source: rustfmt_source(support::uuid_module_source())?,
        },
        RustStandaloneProjectFile {
            path: "src/az/crc.rs".to_owned(),
            source: rustfmt_source(support::crc_module_source())?,
        },
        RustStandaloneProjectFile {
            path: "src/az/asset.rs".to_owned(),
            source: rustfmt_source(support::asset_module_source())?,
        },
    ])
}

pub(super) fn emit_integrated_project(
    unit: &RustCodegenUnit,
    context: &CodegenContext,
) -> Result<RustStandaloneProject, RustSourceEmitError> {
    Ok(RustStandaloneProject {
        files: emit_integrated_type_files(unit, context)?,
    })
}

fn emit_standalone_type_files(
    unit: &RustCodegenUnit,
    context: &CodegenContext,
) -> Result<Vec<RustStandaloneProjectFile>, RustSourceEmitError> {
    let layout = standalone_type_layout(unit);
    let module_tree = standalone_type_module_tree(&layout);
    let symbol_surface = standalone_type_symbol_surface(&layout);
    let exports_by_module = standalone_type_exports_by_module(&symbol_surface);
    let known_type_names = standalone_known_type_names(unit, &exports_by_module);
    let tasks = standalone_type_tasks(&layout, &module_tree);

    let emitted = context
        .runner()
        .try_map_until_cancelled(&tasks, context.cancel(), |task| {
            let emit_context = TypeModuleEmitContext {
                module_path: &task.module_path,
                symbol_surface: &symbol_surface,
                known_type_names: &known_type_names,
                context,
            };
            emit_standalone_type_module(
                &emit_context,
                task.child_dirs.iter().map(String::as_str),
                task.leaf_modules.iter().map(String::as_str),
                Vec::new(),
                &task.items,
            )
            .map(|source| RustStandaloneProjectFile {
                path: task.path.clone(),
                source,
            })
        })?;
    if emitted.was_cancelled() {
        return Err(RustSourceEmitError::Cancelled);
    }
    Ok(emitted.into_completed())
}

fn standalone_type_tasks<'a>(
    layout: &'a crate::rust::layout::RustStandaloneTypeLayout<'a>,
    module_tree: &BTreeMap<Vec<String>, crate::rust::layout::RustStandaloneModuleNode>,
) -> Vec<TypeModuleTask<'a>> {
    let root_module_path = Vec::<String>::new();
    let root_child_modules = module_tree
        .get(&root_module_path)
        .map(|node| node.child_dirs.iter().cloned().collect::<Vec<_>>())
        .unwrap_or_default();
    let root_leaf_modules = module_tree
        .get(&root_module_path)
        .map(|node| node.leaf_modules.iter().cloned().collect::<Vec<_>>())
        .unwrap_or_default();
    let mut tasks = vec![TypeModuleTask {
        path: "src/types/mod.rs".to_owned(),
        module_path: root_module_path,
        child_dirs: root_child_modules,
        leaf_modules: root_leaf_modules,
        register_child_modules: Vec::new(),
        items: Vec::new(),
    }];

    for (module_path, node) in module_tree.iter() {
        if module_path.is_empty() {
            continue;
        }
        tasks.push(TypeModuleTask {
            path: standalone_module_file_path(module_path),
            module_path: module_path.clone(),
            child_dirs: node.child_dirs.iter().cloned().collect(),
            leaf_modules: node.leaf_modules.iter().cloned().collect(),
            register_child_modules: Vec::new(),
            items: layout
                .module_groups
                .get(module_path)
                .cloned()
                .unwrap_or_default(),
        });
    }

    for ((scope_path, file_stem), items) in &layout.file_groups {
        tasks.push(TypeModuleTask {
            path: standalone_item_file_path(scope_path, file_stem),
            module_path: scope_path.clone(),
            child_dirs: Vec::new(),
            leaf_modules: Vec::new(),
            register_child_modules: Vec::new(),
            items: (*items).clone(),
        });
    }

    tasks
}

fn emit_integrated_type_files(
    unit: &RustCodegenUnit,
    context: &CodegenContext,
) -> Result<Vec<RustStandaloneProjectFile>, RustSourceEmitError> {
    let layout = standalone_type_layout(unit);
    let module_tree = standalone_type_module_tree(&layout);
    let symbol_surface = standalone_type_symbol_surface(&layout);
    let exports_by_module = standalone_type_exports_by_module(&symbol_surface);
    let known_type_names = standalone_known_type_names(unit, &exports_by_module);
    let register_children_by_module = integrated_register_children_by_module(&layout, &module_tree);
    let tasks = integrated_type_tasks(&layout, &module_tree, &register_children_by_module);

    let emitted = context
        .runner()
        .try_map_until_cancelled(&tasks, context.cancel(), |task| {
            let emit_context = TypeModuleEmitContext {
                module_path: &task.module_path,
                symbol_surface: &symbol_surface,
                known_type_names: &known_type_names,
                context,
            };
            emit_integrated_type_module(
                &emit_context,
                task.child_dirs.iter().map(String::as_str),
                task.leaf_modules.iter().map(String::as_str),
                &task.register_child_modules,
                &task.items,
            )
            .map(|source| RustStandaloneProjectFile {
                path: task.path.clone(),
                source,
            })
        })?;
    if emitted.was_cancelled() {
        return Err(RustSourceEmitError::Cancelled);
    }
    Ok(emitted.into_completed())
}

fn integrated_type_tasks<'a>(
    layout: &'a crate::rust::layout::RustStandaloneTypeLayout<'a>,
    module_tree: &BTreeMap<Vec<String>, crate::rust::layout::RustStandaloneModuleNode>,
    register_children_by_module: &BTreeMap<Vec<String>, Vec<String>>,
) -> Vec<TypeModuleTask<'a>> {
    let root_module_path = Vec::<String>::new();
    let root_child_modules = module_tree
        .get(&root_module_path)
        .map(|node| node.child_dirs.iter().cloned().collect::<Vec<_>>())
        .unwrap_or_default();
    let root_leaf_modules = module_tree
        .get(&root_module_path)
        .map(|node| node.leaf_modules.iter().cloned().collect::<Vec<_>>())
        .unwrap_or_default();
    let mut tasks = vec![TypeModuleTask {
        path: "mod.rs".to_owned(),
        module_path: root_module_path.clone(),
        child_dirs: root_child_modules,
        leaf_modules: root_leaf_modules,
        register_child_modules: register_children_by_module
            .get(&root_module_path)
            .cloned()
            .unwrap_or_default(),
        items: Vec::new(),
    }];

    for (module_path, node) in module_tree.iter() {
        if module_path.is_empty() {
            continue;
        }
        tasks.push(TypeModuleTask {
            path: integrated_module_file_path(module_path),
            module_path: module_path.clone(),
            child_dirs: node.child_dirs.iter().cloned().collect(),
            leaf_modules: node.leaf_modules.iter().cloned().collect(),
            register_child_modules: register_children_by_module
                .get(module_path)
                .cloned()
                .unwrap_or_default(),
            items: layout
                .module_groups
                .get(module_path)
                .cloned()
                .unwrap_or_default(),
        });
    }

    for ((scope_path, file_stem), items) in &layout.file_groups {
        let mut module_path = scope_path.clone();
        module_path.push(file_stem.clone());
        tasks.push(TypeModuleTask {
            path: integrated_item_file_path(scope_path, file_stem),
            module_path,
            child_dirs: Vec::new(),
            leaf_modules: Vec::new(),
            register_child_modules: Vec::new(),
            items: (*items).clone(),
        });
    }

    tasks
}

#[derive(Debug, Clone)]
struct TypeModuleTask<'a> {
    path: String,
    module_path: Vec<String>,
    child_dirs: Vec<String>,
    leaf_modules: Vec<String>,
    register_child_modules: Vec<String>,
    items: Vec<&'a RustItemPlan>,
}

struct TypeModuleEmitContext<'a> {
    module_path: &'a [String],
    symbol_surface: &'a BTreeMap<Vec<String>, SymbolSurfaceModule<()>>,
    known_type_names: &'a BTreeSet<String>,
    context: &'a CodegenContext,
}

fn emit_standalone_type_module<'a>(
    emit_context: &TypeModuleEmitContext<'_>,
    child_dirs: impl Iterator<Item = &'a str>,
    leaf_modules: impl Iterator<Item = &'a str>,
    extra_module_reexports: Vec<StandaloneModuleReexport>,
    items: &[&RustItemPlan],
) -> Result<String, RustSourceEmitError> {
    let child_modules = child_dirs
        .map(parse_module_ident)
        .collect::<Result<Vec<_>, _>>()?;
    let leaf_modules = leaf_modules
        .map(parse_module_ident)
        .collect::<Result<Vec<_>, _>>()?;
    let options = RustSourceOptions {
        mode: RustSourceMode::Standalone,
    };
    let item_source = render_module_item_source(items, options, emit_context.context)?;

    let mut reexports = if child_modules.is_empty() && leaf_modules.is_empty() {
        Vec::new()
    } else {
        standalone_module_reexports(emit_context.module_path, emit_context.symbol_surface)
    };
    reexports.extend(extra_module_reexports);
    let reexported_type_names = reexports
        .iter()
        .flat_map(|reexport| reexport.items.iter().cloned())
        .collect::<BTreeSet<_>>();

    let mut source = String::new();
    append_standalone_type_imports(
        &mut source,
        items,
        emit_context.known_type_names,
        &reexported_type_names,
    );

    let mut modules = child_modules;
    modules.extend(leaf_modules);
    modules.sort_by_key(ToString::to_string);

    if !modules.is_empty() {
        for module in &modules {
            source.push_str("pub mod ");
            source.push_str(&module.to_string());
            source.push_str(";\n");
        }
        source.push('\n');

        reexports.sort_by(|left, right| left.module.cmp(&right.module));
        for reexport in reexports {
            if reexport.items.is_empty() {
                continue;
            }
            append_module_reexport(&mut source, &reexport);
        }
        source.push('\n');
    }

    if !item_source.trim().is_empty() {
        source.push_str(item_source.trim_start());
    }

    rustfmt_source(&source)
}

fn emit_integrated_type_module<'a>(
    emit_context: &TypeModuleEmitContext<'_>,
    child_dirs: impl Iterator<Item = &'a str>,
    leaf_modules: impl Iterator<Item = &'a str>,
    register_child_modules: &[String],
    items: &[&RustItemPlan],
) -> Result<String, RustSourceEmitError> {
    let child_modules = child_dirs
        .map(parse_module_ident)
        .collect::<Result<Vec<_>, _>>()?;
    let leaf_modules = leaf_modules
        .map(parse_module_ident)
        .collect::<Result<Vec<_>, _>>()?;
    let options = RustSourceOptions {
        mode: RustSourceMode::Integrated,
    };
    let item_source = render_module_item_source(items, options, emit_context.context)?;

    let mut reexports = if child_modules.is_empty() && leaf_modules.is_empty() {
        Vec::new()
    } else {
        standalone_module_reexports(emit_context.module_path, emit_context.symbol_surface)
    };
    let reexported_type_names = reexports
        .iter()
        .flat_map(|reexport| reexport.items.iter().cloned())
        .collect::<BTreeSet<_>>();
    let has_register = !emit_context.module_path.is_empty()
        && (!register_child_modules.is_empty() || items.iter().any(|item| can_register_type(item)));

    let mut source = String::new();
    append_integrated_type_imports(
        &mut source,
        items,
        emit_context.known_type_names,
        &reexported_type_names,
        has_register,
    );

    let mut modules = child_modules;
    modules.extend(leaf_modules);
    modules.sort_by_key(ToString::to_string);

    if !modules.is_empty() {
        for module in &modules {
            source.push_str("pub mod ");
            source.push_str(&module.to_string());
            source.push_str(";\n");
        }
        source.push('\n');

        reexports.sort_by(|left, right| left.module.cmp(&right.module));
        for reexport in reexports {
            if reexport.items.is_empty() {
                continue;
            }
            append_module_reexport(&mut source, &reexport);
        }
        source.push('\n');
    }

    if !item_source.trim().is_empty() {
        source.push_str(item_source.trim_start());
        source.push('\n');
    }

    if has_register {
        source.push_str(&render_integrated_register(items, register_child_modules)?);
    }

    rustfmt_source(&source)
}

fn render_module_item_source(
    items: &[&RustItemPlan],
    options: RustSourceOptions,
    context: &CodegenContext,
) -> Result<String, RustSourceEmitError> {
    let rendered = context
        .runner()
        .try_map_until_cancelled(items, context.cancel(), |item| {
            render_item_source(item, options)
        })?;
    let cancelled = rendered.was_cancelled();
    let mut source = String::new();
    for result in rendered.into_completed() {
        source.push_str(result.trim_start());
        if !source.ends_with('\n') {
            source.push('\n');
        }
    }
    if cancelled {
        return Err(RustSourceEmitError::Cancelled);
    }
    Ok(source)
}

fn render_item_source(
    item: &RustItemPlan,
    options: RustSourceOptions,
) -> Result<String, RustSourceEmitError> {
    let rendered = render_item(item, options)?;
    let file = syn::parse2::<syn::File>(quote! {
        #rendered
    })
    .map_err(RustSourceEmitError::File)?;
    Ok(prettyplease::unparse(&file))
}

fn integrated_register_children_by_module(
    layout: &crate::rust::layout::RustStandaloneTypeLayout<'_>,
    module_tree: &BTreeMap<Vec<String>, crate::rust::layout::RustStandaloneModuleNode>,
) -> BTreeMap<Vec<String>, Vec<String>> {
    let cascade_roots = integrated_register_cascade_roots(layout);
    let mut children_by_module = BTreeMap::new();
    for (module_path, node) in module_tree {
        if !cascade_roots
            .iter()
            .any(|root| module_path_starts_with(module_path, root))
        {
            continue;
        }

        let children = node
            .child_dirs
            .iter()
            .chain(node.leaf_modules.iter())
            .filter(|child| {
                let mut child_path = module_path.clone();
                child_path.push((*child).clone());
                cascade_roots
                    .iter()
                    .any(|root| module_path_starts_with(&child_path, root))
                    && module_has_register_in_subtree(layout, module_tree, &child_path)
            })
            .cloned()
            .collect::<Vec<_>>();
        if !children.is_empty() {
            children_by_module.insert(module_path.clone(), children);
        }
    }
    children_by_module
}

fn integrated_register_cascade_roots(
    layout: &crate::rust::layout::RustStandaloneTypeLayout<'_>,
) -> BTreeSet<Vec<String>> {
    let module_roots = layout
        .module_groups
        .iter()
        .filter(|(_, items)| items.iter().any(|item| can_cascade_register(item)))
        .map(|(module_path, _)| module_path.clone());
    let file_roots = layout
        .file_groups
        .iter()
        .filter(|(_, items)| items.iter().any(|item| can_cascade_register(item)))
        .map(|((scope_path, file_stem), _)| {
            let mut module_path = scope_path.clone();
            module_path.push(file_stem.clone());
            module_path
        });

    module_roots.chain(file_roots).collect()
}

fn can_cascade_register(item: &RustItemPlan) -> bool {
    can_register_type(item) && !item.is_reflected_base
}

fn module_path_starts_with(module_path: &[String], root: &[String]) -> bool {
    module_path.len() >= root.len() && module_path[..root.len()] == *root
}

fn module_has_register_in_subtree(
    layout: &crate::rust::layout::RustStandaloneTypeLayout<'_>,
    module_tree: &BTreeMap<Vec<String>, crate::rust::layout::RustStandaloneModuleNode>,
    module_path: &[String],
) -> bool {
    if module_has_local_register(layout, module_path) {
        return true;
    }

    let Some(node) = module_tree.get(module_path) else {
        return false;
    };

    node.child_dirs
        .iter()
        .chain(node.leaf_modules.iter())
        .any(|child| {
            let mut child_path = module_path.to_vec();
            child_path.push(child.clone());
            module_has_register_in_subtree(layout, module_tree, &child_path)
        })
}

fn module_has_local_register(
    layout: &crate::rust::layout::RustStandaloneTypeLayout<'_>,
    module_path: &[String],
) -> bool {
    if layout
        .module_groups
        .get(module_path)
        .is_some_and(|items| items.iter().any(|item| can_register_type(item)))
    {
        return true;
    }

    let Some((file_stem, scope_path)) = module_path.split_last() else {
        return false;
    };
    layout
        .file_groups
        .get(&(scope_path.to_vec(), file_stem.clone()))
        .is_some_and(|items| items.iter().any(|item| can_register_type(item)))
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct StandaloneModuleReexport {
    module: String,
    items: Vec<String>,
}

fn standalone_type_symbol_surface(
    layout: &crate::rust::layout::RustStandaloneTypeLayout<'_>,
) -> BTreeMap<Vec<String>, SymbolSurfaceModule<()>> {
    let module_tree = standalone_type_module_tree(layout);
    let local_symbols_by_path = layout
        .module_groups
        .iter()
        .map(|(path, items)| {
            (
                path.clone(),
                items
                    .iter()
                    .map(|item| (item.rust_name.clone(), ()))
                    .collect(),
            )
        })
        .collect();
    let mut direct_reexports_by_path = BTreeMap::<Vec<String>, Vec<SymbolSurfaceExport<()>>>::new();
    for ((module_path, module), items) in &layout.file_groups {
        direct_reexports_by_path
            .entry(module_path.clone())
            .or_default()
            .push(SymbolSurfaceExport {
                module: module.clone(),
                symbols: items
                    .iter()
                    .map(|item| (item.rust_name.clone(), ()))
                    .collect(),
            });
    }
    let child_modules_by_path = module_tree
        .into_iter()
        .map(|(path, node)| (path, node.child_dirs))
        .collect();

    crate::symbol_surface::plan_symbol_surface(SymbolSurfaceInput {
        root_path: Vec::new(),
        local_symbols_by_path,
        direct_reexports_by_path,
        child_modules_by_path,
    })
}

fn standalone_type_exports_by_module(
    symbol_surface: &BTreeMap<Vec<String>, SymbolSurfaceModule<()>>,
) -> BTreeMap<Vec<String>, BTreeSet<String>> {
    symbol_surface
        .iter()
        .map(|(path, module)| {
            (
                path.clone(),
                module.public_symbols.keys().cloned().collect(),
            )
        })
        .collect()
}

fn standalone_module_reexports(
    module_path: &[String],
    symbol_surface: &BTreeMap<Vec<String>, SymbolSurfaceModule<()>>,
) -> Vec<StandaloneModuleReexport> {
    symbol_surface
        .get(module_path)
        .map(|module| {
            module
                .reexports
                .iter()
                .map(|reexport| StandaloneModuleReexport {
                    module: reexport.module.clone(),
                    items: reexport.symbols.keys().cloned().collect(),
                })
                .collect()
        })
        .unwrap_or_default()
}

fn standalone_known_type_names(
    _unit: &RustCodegenUnit,
    exports_by_module: &BTreeMap<Vec<String>, BTreeSet<String>>,
) -> BTreeSet<String> {
    exports_by_module
        .get(&Vec::new())
        .cloned()
        .unwrap_or_default()
}

fn integrated_module_file_path(module_path: &[String]) -> String {
    let mut path = module_path.join("/");
    path.push_str("/mod.rs");
    path
}

fn integrated_item_file_path(scope_path: &[String], file_stem: &str) -> String {
    let mut path = String::new();
    if !scope_path.is_empty() {
        path.push_str(&scope_path.join("/"));
        path.push('/');
    }
    path.push_str(file_stem);
    path.push_str(".rs");
    path
}

fn has_derive(item: &RustItemPlan, derive_name: &str) -> bool {
    item.derives.iter().any(|name| name == derive_name)
}

fn has_az_identity(item: &RustItemPlan) -> bool {
    has_derive(item, "AzTypeInfo") || has_derive(item, "AzRtti")
}

fn can_register_type(item: &RustItemPlan) -> bool {
    has_derive(item, "Reflect") && has_az_identity(item)
}

fn can_register_az_type_info(item: &RustItemPlan) -> bool {
    can_register_type(item)
}

fn can_register_az_rtti(item: &RustItemPlan) -> bool {
    has_derive(item, "Reflect") && has_derive(item, "AzRtti")
}

fn append_integrated_type_imports(
    source: &mut String,
    items: &[&RustItemPlan],
    known_type_names: &BTreeSet<String>,
    reexported_type_names: &BTreeSet<String>,
    has_register: bool,
) {
    if items.is_empty() && !has_register {
        return;
    }

    let az_derives = items
        .iter()
        .flat_map(|item| item.derives.iter())
        .filter(|derive| matches!(derive.as_str(), "AzRtti" | "AzTypeInfo"))
        .cloned()
        .collect::<BTreeSet<_>>();
    if !az_derives.is_empty() {
        source.push_str("use az_derive::");
        append_import_items(source, &az_derives);
        source.push_str(";\n");
    }

    let reflected_type_imports =
        reflected_type_imports_for_items(items, known_type_names, reexported_type_names);
    if !reflected_type_imports.is_empty() {
        source.push_str("use crate::generated::");
        append_import_items(source, &reflected_type_imports);
        source.push_str(";\n");
    }

    let needs_bevy_component = items.iter().any(|item| {
        item.derives
            .iter()
            .any(|derive_name| derive_name == "Component")
    });
    let needs_reflect = items.iter().any(|item| {
        item.derives
            .iter()
            .any(|derive_name| derive_name == "Reflect")
    });
    if needs_bevy_component && needs_reflect {
        source.push_str("use bevy::ecs::reflect::ReflectComponent;\n");
    }
    let needs_marshaler = items.iter().any(|item| {
        item.derives
            .iter()
            .any(|derive_name| derive_name == "Marshaler")
    });
    if needs_marshaler {
        source.push_str("use gridmate::Marshaler;\n");
    }
    if has_register {
        source.push_str("use bevy::prelude::App;\n");
    }

    source.push('\n');
}

fn render_integrated_register(
    items: &[&RustItemPlan],
    register_child_modules: &[String],
) -> Result<String, RustSourceEmitError> {
    let child_modules = register_child_modules
        .iter()
        .map(|module| parse_module_ident(module))
        .collect::<Result<Vec<_>, _>>()?;
    let type_names = items
        .iter()
        .filter(|item| can_register_type(item))
        .map(|item| parse_register_item_ident(item))
        .collect::<Result<Vec<_>, _>>()?;
    let az_type_info_names = items
        .iter()
        .filter(|item| can_register_az_type_info(item))
        .map(|item| parse_register_item_ident(item))
        .collect::<Result<Vec<_>, _>>()?;
    let az_rtti_names = items
        .iter()
        .filter(|item| can_register_az_rtti(item))
        .map(|item| parse_register_item_ident(item))
        .collect::<Result<Vec<_>, _>>()?;
    let file = syn::parse2::<syn::File>(quote! {
        pub fn register(app: &mut App) {
            #(#child_modules::register(app);)*
            #(app.register_type::<#type_names>();)*
            #(app.register_type_data::<#az_type_info_names, ::az_core::ReflectAzTypeInfo>();)*
            #(app.register_type_data::<#az_rtti_names, ::az_core::ReflectAzRtti>();)*
        }
    })
    .map_err(RustSourceEmitError::File)?;
    Ok(prettyplease::unparse(&file))
}

fn parse_register_item_ident(item: &RustItemPlan) -> Result<Ident, RustSourceEmitError> {
    parse_item_ident(&item.rust_name, &item.source_name).map_err(|source| {
        RustSourceEmitError::ItemIdent {
            source_name: item.source_name.clone(),
            identifier: item.rust_name.clone(),
            source,
        }
    })
}

fn parse_item_ident(identifier: &str, source_name: &str) -> Result<Ident, syn::Error> {
    syn::parse_str::<Ident>(identifier)
        .map_err(|source| syn::Error::new(source.span(), format!("{source_name}: {source}")))
}

fn append_module_reexport(source: &mut String, reexport: &StandaloneModuleReexport) {
    source.push_str("pub use self::");
    source.push_str(&reexport.module);
    source.push_str("::");
    append_reexport_items(source, &reexport.items);
    source.push_str(";\n");
}

fn append_reexport_items(source: &mut String, items: &[String]) {
    if items.len() == 1 {
        source.push_str(
            items
                .first()
                .expect("single reexport item set contains an item"),
        );
        return;
    }

    source.push('{');
    for (index, item) in items.iter().enumerate() {
        if index > 0 {
            source.push_str(", ");
        }
        source.push_str(item);
    }
    source.push('}');
}

fn append_standalone_type_imports(
    source: &mut String,
    items: &[&RustItemPlan],
    known_type_names: &BTreeSet<String>,
    reexported_type_names: &BTreeSet<String>,
) {
    if items.is_empty() {
        return;
    }

    for import in standalone_support_imports_for_items(items) {
        source.push_str(import);
        source.push('\n');
    }

    let reflected_type_imports =
        reflected_type_imports_for_items(items, known_type_names, reexported_type_names);
    if !reflected_type_imports.is_empty() {
        source.push_str("use crate::types::");
        append_import_items(source, &reflected_type_imports);
        source.push_str(";\n");
    }
    source.push('\n');

    let needs_bevy_component = items.iter().any(|item| {
        item.derives
            .iter()
            .any(|derive_name| derive_name == "Component")
    });
    if needs_bevy_component {
        source.push_str("use bevy_ecs::reflect::ReflectComponent;\n");
        source.push('\n');
    }
}

fn standalone_support_imports_for_items(items: &[&RustItemPlan]) -> Vec<&'static str> {
    if items.is_empty() {
        return Vec::new();
    }

    let mut imports = vec![
        "use crate::az::rtti::AzRtti;",
        "use crate::az::uuid::Uuid as AzUuid;",
    ];
    if items_use_rust_type_ident(items, "AzAsset") {
        imports.push("use crate::az::asset::Asset as AzAsset;");
    }
    if items_use_rust_type_ident(items, "AzAssetId") {
        imports.push("use crate::az::asset::AssetId as AzAssetId;");
    }
    if items_use_rust_type_ident(items, "AzCrc32") {
        imports.push("use crate::az::crc::Crc32 as AzCrc32;");
    }
    imports.sort_unstable();
    imports
}

fn items_use_rust_type_ident(items: &[&RustItemPlan], name: &str) -> bool {
    items
        .iter()
        .flat_map(|item| item.fields.iter().map(|field| field.rust_type.as_str()))
        .any(|rust_type| rust_type_uses_ident(rust_type, name))
}

fn rust_type_uses_ident(rust_type: &str, name: &str) -> bool {
    let Ok(ty) = syn::parse_str::<Type>(rust_type) else {
        return false;
    };
    type_uses_ident(&ty, name)
}

fn type_uses_ident(ty: &Type, name: &str) -> bool {
    match ty {
        Type::Array(array) => type_uses_ident(&array.elem, name),
        Type::Group(group) => type_uses_ident(&group.elem, name),
        Type::Paren(paren) => type_uses_ident(&paren.elem, name),
        Type::Path(path) => path_uses_ident(&path.path, name),
        Type::Ptr(ptr) => type_uses_ident(&ptr.elem, name),
        Type::Reference(reference) => type_uses_ident(&reference.elem, name),
        Type::Slice(slice) => type_uses_ident(&slice.elem, name),
        Type::Tuple(tuple) => tuple
            .elems
            .iter()
            .any(|element| type_uses_ident(element, name)),
        Type::BareFn(_)
        | Type::ImplTrait(_)
        | Type::Infer(_)
        | Type::Macro(_)
        | Type::Never(_)
        | Type::TraitObject(_)
        | Type::Verbatim(_) => false,
        _ => false,
    }
}

fn path_uses_ident(path: &syn::Path, name: &str) -> bool {
    for segment in &path.segments {
        if segment.ident == name {
            return true;
        }
        if let PathArguments::AngleBracketed(arguments) = &segment.arguments {
            for argument in &arguments.args {
                if let GenericArgument::Type(ty) = argument
                    && type_uses_ident(ty, name)
                {
                    return true;
                }
            }
        }
    }
    false
}

fn reflected_type_imports_for_items(
    items: &[&RustItemPlan],
    known_type_names: &BTreeSet<String>,
    reexported_type_names: &BTreeSet<String>,
) -> BTreeSet<String> {
    let mut local_names = items
        .iter()
        .map(|item| item.rust_name.as_str())
        .collect::<BTreeSet<_>>();
    local_names.extend(reexported_type_names.iter().map(String::as_str));
    let mut imports = BTreeSet::new();
    for item in items {
        for base in &item.rtti_bases {
            collect_reflected_type_imports_from_rust_type(
                &base.rust_type,
                known_type_names,
                &local_names,
                &mut imports,
            );
        }
        for field in &item.fields {
            collect_reflected_type_imports_from_rust_type(
                &field.rust_type,
                known_type_names,
                &local_names,
                &mut imports,
            );
        }
        for variant in &item.variants {
            if let Some(payload_type) = &variant.payload_type {
                collect_reflected_type_imports_from_rust_type(
                    payload_type,
                    known_type_names,
                    &local_names,
                    &mut imports,
                );
            }
        }
    }
    imports
}

fn collect_reflected_type_imports_from_rust_type(
    rust_type: &str,
    known_type_names: &BTreeSet<String>,
    local_names: &BTreeSet<&str>,
    imports: &mut BTreeSet<String>,
) {
    let Ok(ty) = syn::parse_str::<Type>(rust_type) else {
        return;
    };
    collect_reflected_type_imports(&ty, known_type_names, local_names, imports);
}

fn collect_reflected_type_imports(
    ty: &Type,
    known_type_names: &BTreeSet<String>,
    local_names: &BTreeSet<&str>,
    imports: &mut BTreeSet<String>,
) {
    match ty {
        Type::Array(array) => {
            collect_reflected_type_imports(&array.elem, known_type_names, local_names, imports);
        }
        Type::Group(group) => {
            collect_reflected_type_imports(&group.elem, known_type_names, local_names, imports);
        }
        Type::Paren(paren) => {
            collect_reflected_type_imports(&paren.elem, known_type_names, local_names, imports);
        }
        Type::Path(path) => {
            collect_reflected_type_imports_from_path(
                &path.path,
                known_type_names,
                local_names,
                imports,
            );
        }
        Type::Ptr(ptr) => {
            collect_reflected_type_imports(&ptr.elem, known_type_names, local_names, imports);
        }
        Type::Reference(reference) => {
            collect_reflected_type_imports(&reference.elem, known_type_names, local_names, imports);
        }
        Type::Slice(slice) => {
            collect_reflected_type_imports(&slice.elem, known_type_names, local_names, imports);
        }
        Type::Tuple(tuple) => {
            for element in &tuple.elems {
                collect_reflected_type_imports(element, known_type_names, local_names, imports);
            }
        }
        Type::BareFn(_)
        | Type::ImplTrait(_)
        | Type::Infer(_)
        | Type::Macro(_)
        | Type::Never(_)
        | Type::TraitObject(_)
        | Type::Verbatim(_) => {}
        _ => {}
    }
}

fn collect_reflected_type_imports_from_path(
    path: &syn::Path,
    known_type_names: &BTreeSet<String>,
    local_names: &BTreeSet<&str>,
    imports: &mut BTreeSet<String>,
) {
    if path
        .segments
        .first()
        .is_some_and(|segment| segment.ident == "crate")
    {
        return;
    }

    if let Some(segment) = path.segments.first()
        && path.segments.len() == 1
    {
        let ident = segment.ident.to_string();
        if known_type_names.contains(&ident) && !local_names.contains(ident.as_str()) {
            imports.insert(ident);
        }
    }

    for segment in &path.segments {
        if let PathArguments::AngleBracketed(arguments) = &segment.arguments {
            for argument in &arguments.args {
                if let GenericArgument::Type(ty) = argument {
                    collect_reflected_type_imports(ty, known_type_names, local_names, imports);
                }
            }
        }
    }
}

fn append_import_items(source: &mut String, imports: &BTreeSet<String>) {
    if imports.len() == 1 {
        source.push_str(
            imports
                .iter()
                .next()
                .expect("single import set contains an item"),
        );
        return;
    }

    source.push('{');
    for (index, import) in imports.iter().enumerate() {
        if index > 0 {
            source.push_str(", ");
        }
        source.push_str(import);
    }
    source.push('}');
}

fn parse_module_ident(module: &str) -> Result<Ident, RustSourceEmitError> {
    syn::parse_str::<Ident>(module).map_err(|source| RustSourceEmitError::ItemIdent {
        source_name: format!("module:{module}"),
        identifier: module.to_owned(),
        source,
    })
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use crate::symbol_surface::SymbolSurfaceExport;

    use super::*;

    #[test]
    fn standalone_module_reexports_keep_duplicate_child_names_qualified() {
        let local_names = BTreeSet::<String>::new();
        let reexports = standalone_reexports_from_symbol_surface(
            crate::symbol_surface::retain_public_symbol_reexports(
                vec![
                    symbol_reexport("alpha", ["Alpha", "Shared"]),
                    symbol_reexport("beta", ["Beta", "Shared"]),
                ],
                local_names.iter(),
            ),
        );

        assert_eq!(
            reexports,
            vec![
                StandaloneModuleReexport {
                    module: "alpha".to_owned(),
                    items: vec!["Alpha".to_owned()],
                },
                StandaloneModuleReexport {
                    module: "beta".to_owned(),
                    items: vec!["Beta".to_owned()],
                },
            ]
        );

        let mut source = String::new();
        for reexport in reexports {
            append_module_reexport(&mut source, &reexport);
        }
        assert_eq!(
            source,
            "pub use self::alpha::Alpha;\npub use self::beta::Beta;\n"
        );
        assert!(!source.contains("::*"));
        assert!(!source.contains("Shared"));
    }

    #[test]
    fn standalone_module_reexports_skip_child_names_that_collide_with_local_definitions() {
        let local_names = BTreeSet::from(["Shared".to_owned()]);
        let reexports = standalone_reexports_from_symbol_surface(
            crate::symbol_surface::retain_public_symbol_reexports(
                vec![symbol_reexport("child", ["Child", "Shared"])],
                local_names.iter(),
            ),
        );

        assert_eq!(
            reexports,
            vec![StandaloneModuleReexport {
                module: "child".to_owned(),
                items: vec!["Child".to_owned()],
            }]
        );
    }

    #[test]
    fn reflected_imports_ignore_external_qualified_paths() {
        let known_type_names = BTreeSet::from([
            "ReflectedThing".to_owned(),
            "Transform".to_owned(),
            "Vec3".to_owned(),
        ]);
        let local_names = BTreeSet::<&str>::new();
        let mut imports = BTreeSet::new();

        collect_reflected_type_imports_from_rust_type(
            "bevy::math::Vec3",
            &known_type_names,
            &local_names,
            &mut imports,
        );
        collect_reflected_type_imports_from_rust_type(
            "bevy::transform::components::Transform",
            &known_type_names,
            &local_names,
            &mut imports,
        );
        collect_reflected_type_imports_from_rust_type(
            "std::collections::HashMap<String, ReflectedThing>",
            &known_type_names,
            &local_names,
            &mut imports,
        );

        assert_eq!(imports, BTreeSet::from(["ReflectedThing".to_owned()]));
    }

    fn symbol_reexport<const N: usize>(module: &str, names: [&str; N]) -> SymbolSurfaceExport<()> {
        SymbolSurfaceExport {
            module: module.to_owned(),
            symbols: names
                .into_iter()
                .map(|name| (name.to_owned(), ()))
                .collect(),
        }
    }

    fn standalone_reexports_from_symbol_surface(
        reexports: Vec<SymbolSurfaceExport<()>>,
    ) -> Vec<StandaloneModuleReexport> {
        reexports
            .into_iter()
            .map(|reexport| StandaloneModuleReexport {
                module: reexport.module,
                items: reexport.symbols.keys().cloned().collect(),
            })
            .collect()
    }
}
