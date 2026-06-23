use std::{collections::BTreeSet, fs, path::Path};

use crate::{ReflectedTypeCatalog, RustSourceTypeIndex};

use super::api::{ComponentScaffoldError, ModuleScaffoldAction};
use super::edit::{
    add_derive_token_to_attr_block, ensure_module_use, existing_reexports_by_module,
    find_attr_block_start, find_component_struct_start, find_matching_brace,
};
use super::module_plan::ModulePlan;
use super::naming::is_facet_type_name;
use super::source_render::{
    bundle_names, plan_has_nested_structs, plan_maps_entities, registration_types,
    render_module_append, render_new_module,
};
use super::{read_to_string, write_string};

pub(super) fn apply_module_plans(
    plans: &[ModulePlan],
    catalog: &ReflectedTypeCatalog,
    source_types: &RustSourceTypeIndex,
) -> Result<(), ComponentScaffoldError> {
    for plan in plans {
        let code = match plan.action {
            ModuleScaffoldAction::CreateFile => render_new_module(plan, catalog, source_types),
            ModuleScaffoldAction::AppendToExistingFile => {
                render_module_append(plan, catalog, source_types)
            }
        };

        match plan.action {
            ModuleScaffoldAction::CreateFile => {
                if let Some(parent) = plan.file_path.parent() {
                    fs::create_dir_all(parent).map_err(|source| {
                        ComponentScaffoldError::CreateDir {
                            path: parent.to_path_buf(),
                            source,
                        }
                    })?;
                }
                write_string(&plan.file_path, &code)?;
            }
            ModuleScaffoldAction::AppendToExistingFile => {
                let mut existing = read_to_string(&plan.file_path)?;
                ensure_module_use(&mut existing, "use az_derive::AzRtti;");
                if plan_has_nested_structs(plan, catalog, source_types) {
                    ensure_module_use(&mut existing, "use az_derive::AzTypeInfo;");
                }
                ensure_module_use(&mut existing, "use bevy::prelude::*;");
                if plan_maps_entities(plan, catalog, source_types) {
                    ensure_module_use(&mut existing, "use bevy::ecs::entity::MapEntities;");
                }
                if plan
                    .components
                    .iter()
                    .any(|component| is_facet_type_name(&component.component_name))
                {
                    ensure_module_use(&mut existing, "use newworld_derive::Facet;");
                }
                if !existing.ends_with('\n') {
                    existing.push('\n');
                }
                existing.push('\n');
                existing.push_str(&code);
                if plan.patch_existing_plugin {
                    patch_existing_module_plugin_registers(
                        &mut existing,
                        plan,
                        catalog,
                        source_types,
                    );
                }
                write_string(&plan.file_path, &existing)?;
            }
        }
    }
    Ok(())
}

pub(super) fn ensure_map_entities_derives(text: &mut String, component_names: &BTreeSet<String>) {
    for component_name in component_names {
        ensure_map_entities_derive(text, component_name);
    }
}

fn ensure_map_entities_derive(text: &mut String, component_name: &str) {
    let Some(struct_start) = find_component_struct_start(text, component_name) else {
        return;
    };
    let attr_block_start = find_attr_block_start(text, struct_start);
    if add_derive_token_to_attr_block(text, attr_block_start, struct_start, "MapEntities") {
        return;
    }
    text.insert_str(struct_start, "#[derive(MapEntities)]\n");
}

fn patch_existing_module_plugin_registers(
    text: &mut String,
    plan: &ModulePlan,
    catalog: &ReflectedTypeCatalog,
    source_types: &RustSourceTypeIndex,
) {
    let impl_header = format!("impl Plugin for {}", plan.plugin_name);
    let Some(impl_start) = text.find(&impl_header) else {
        return;
    };
    let Some(build_start) = text[impl_start..]
        .find("fn build")
        .map(|offset| impl_start + offset)
    else {
        return;
    };
    let Some(open) = text[build_start..]
        .find('{')
        .map(|offset| build_start + offset)
    else {
        return;
    };
    let Some(close) = find_matching_brace(text, open) else {
        return;
    };

    let mut insertion = String::new();
    for ty in registration_types(plan, catalog, source_types) {
        let type_path = ty.name.clone();
        let registration = format!("app.register_type::<{type_path}>()");
        let local_registration = format!("app.register_type::<{}>()", ty.name);
        if !text[open..close].contains(&registration)
            && !text[open..close].contains(&local_registration)
        {
            insertion.push_str("        app.register_type::<");
            insertion.push_str(&type_path);
            insertion.push_str(">();\n");
        }

        let type_info =
            format!("app.register_type_data::<{type_path}, ::az_core::ReflectAzTypeInfo>()");
        let local_type_info = format!(
            "app.register_type_data::<{}, ::az_core::ReflectAzTypeInfo>()",
            ty.name
        );
        if !text[open..close].contains(&type_info) && !text[open..close].contains(&local_type_info)
        {
            insertion.push_str("        app.register_type_data::<");
            insertion.push_str(&type_path);
            insertion.push_str(", ::az_core::ReflectAzTypeInfo>();\n");
        }

        if ty.rtti {
            let rtti = format!("app.register_type_data::<{type_path}, ::az_core::ReflectAzRtti>()");
            let local_rtti = format!(
                "app.register_type_data::<{}, ::az_core::ReflectAzRtti>()",
                ty.name
            );
            if !text[open..close].contains(&rtti) && !text[open..close].contains(&local_rtti) {
                insertion.push_str("        app.register_type_data::<");
                insertion.push_str(&type_path);
                insertion.push_str(", ::az_core::ReflectAzRtti>();\n");
            }
        }
    }

    if !insertion.is_empty() {
        text.insert_str(close, &insertion);
    }
}

pub(super) fn update_components_module_file(
    module_file: &Path,
    plans: &[ModulePlan],
) -> Result<(), ComponentScaffoldError> {
    if plans.is_empty() {
        return Ok(());
    }

    let mut text = read_to_string(module_file)?;
    let mut module_decls = Vec::new();
    let mut reexports = Vec::new();
    let mut existing_reexports = existing_reexports_by_module(&text);

    for plan in plans {
        let module_decl = format!("pub mod {};", plan.module_name);
        if !text.contains(&module_decl) {
            module_decls.push(module_decl);
        }

        let mut items = BTreeSet::new();
        for component in &plan.components {
            items.insert(component.component_name.clone());
        }
        for bundle_name in bundle_names(plan) {
            items.insert(bundle_name);
        }
        items.insert(plan.plugin_name.clone());

        let exported = existing_reexports
            .entry(plan.module_name.clone())
            .or_default();
        let missing = items
            .into_iter()
            .filter(|item| !exported.contains(item))
            .collect::<Vec<_>>();
        if !missing.is_empty() {
            for item in &missing {
                exported.insert(item.clone());
            }
            let reexport = format!("pub use {}::{{{}}};", plan.module_name, missing.join(", "));
            reexports.push(reexport);
        }
    }

    if !module_decls.is_empty() {
        let insertion = format!("{}\n", module_decls.join("\n"));
        if let Some(marker) = text.find("// Re-export") {
            text.insert_str(marker, &insertion);
        } else {
            if !text.ends_with('\n') {
                text.push('\n');
            }
            text.push_str(&insertion);
        }
    }

    if !reexports.is_empty() {
        let insertion = format!("{}\n", reexports.join("\n"));
        if let Some(marker) = text.find("// Re-export") {
            let line_end = text[marker..]
                .find('\n')
                .map(|offset| marker + offset + 1)
                .unwrap_or(text.len());
            text.insert_str(line_end, &insertion);
        } else {
            if !text.ends_with('\n') {
                text.push('\n');
            }
            text.push_str(&insertion);
        }
    }

    write_string(module_file, &text)
}

pub(super) fn update_newworld_plugin_file(
    plugin_file: &Path,
    module_root: Option<&str>,
    plans: &[ModulePlan],
) -> Result<(), ComponentScaffoldError> {
    if plans.is_empty() {
        return Ok(());
    }

    let mut text = read_to_string(plugin_file)?;
    let mut registrations = Vec::new();
    for plan in plans {
        let plugin_path = module_item_path(module_root, &plan.module_name, &plan.plugin_name);
        let registration = format!("        app.add_plugins({plugin_path});");
        if !text.contains(&registration) {
            registrations.push(registration);
        }
    }

    if registrations.is_empty() {
        return Ok(());
    }

    let insertion = format!("{}\n", registrations.join("\n"));
    if let Some(marker) = text.find("        app.add_message::<ActorReady>();") {
        text.insert_str(marker, &insertion);
    } else {
        if !text.ends_with('\n') {
            text.push('\n');
        }
        text.push_str(&insertion);
    }

    write_string(plugin_file, &text)
}

fn module_item_path(module_root: Option<&str>, module_name: &str, item_name: &str) -> String {
    let mut path = String::from("crate::");
    if let Some(module_root) = module_root.filter(|module_root| !module_root.is_empty()) {
        path.push_str(module_root);
        path.push_str("::");
    }
    path.push_str(module_name);
    path.push_str("::");
    path.push_str(item_name);
    path
}
