use std::{
    fs,
    path::{Path, PathBuf},
};

use super::api::ModuleScaffoldAction;
use super::evidence::ComponentEvidence;
use super::naming::{is_facet_type_name, pascal_case};

#[derive(Debug)]
pub(super) struct ModulePlanBuilder {
    pub(super) module_name: String,
    pub(super) file_path: PathBuf,
    pub(super) components: Vec<ComponentEvidence>,
}

impl ModulePlanBuilder {
    pub(super) fn new(module_name: String, components_root: &Path) -> Self {
        Self {
            file_path: components_root.join(format!("{module_name}.rs")),
            module_name,
            components: Vec::new(),
        }
    }

    pub(super) fn build(self) -> ModulePlan {
        let action = if self.file_path.exists() {
            ModuleScaffoldAction::AppendToExistingFile
        } else {
            ModuleScaffoldAction::CreateFile
        };
        let existing_plugin_name = (action == ModuleScaffoldAction::AppendToExistingFile)
            .then(|| existing_module_plugin_name(&self.file_path))
            .flatten();
        let plugin_name = existing_plugin_name
            .clone()
            .unwrap_or_else(|| module_plugin_name(&self.module_name, &self.components));
        ModulePlan {
            module_name: self.module_name,
            file_path: self.file_path,
            components: self.components,
            action,
            plugin_name,
            patch_existing_plugin: existing_plugin_name.is_some(),
        }
    }
}

#[derive(Debug)]
pub(super) struct ModulePlan {
    pub(super) module_name: String,
    pub(super) file_path: PathBuf,
    pub(super) components: Vec<ComponentEvidence>,
    pub(super) action: ModuleScaffoldAction,
    pub(super) plugin_name: String,
    pub(super) patch_existing_plugin: bool,
}

fn existing_module_plugin_name(file_path: &Path) -> Option<String> {
    let text = fs::read_to_string(file_path).ok()?;
    for line in text.lines() {
        let trimmed = line.trim();
        let Some(rest) = trimmed.strip_prefix("pub struct ") else {
            continue;
        };
        let Some(plugin_name) = rest.strip_suffix(';') else {
            continue;
        };
        if plugin_name.ends_with("Plugin") {
            return Some(plugin_name.trim().to_owned());
        }
    }
    None
}

fn module_plugin_name(module_name: &str, components: &[ComponentEvidence]) -> String {
    components
        .iter()
        .find(|component| !is_facet_type_name(&component.component_name))
        .or_else(|| components.first())
        .map(|component| format!("{}Plugin", component.component_name))
        .unwrap_or_else(|| format!("{}Plugin", pascal_case(module_name)))
}
