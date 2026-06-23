use std::path::Path;

use heck::ToUpperCamelCase;
use serde_json::Value;

pub const DEFAULT_MODULE_NAME: &str = "NewWorld";

#[must_use]
pub fn module_descriptors_root(modules: Vec<Value>) -> Value {
    let mut root = serde_json::Map::new();
    root.insert("modules".to_owned(), Value::Array(modules));
    Value::Object(root)
}

#[must_use]
pub fn module_descriptors_root_from_capture(module_name: String, root: Value) -> Value {
    if root.get("modules").is_some() {
        return root;
    }
    module_descriptors_root(vec![module_descriptor_capture(module_name, root)])
}

#[must_use]
pub fn module_descriptor_capture(module_name: String, mut root: Value) -> Value {
    if let Value::Object(map) = &mut root {
        map.entry("moduleName".to_owned())
            .or_insert(Value::String(module_name));
    }
    root
}

#[must_use]
pub fn module_name_from_path(path: &Path) -> String {
    path.file_stem()
        .and_then(|stem| stem.to_str())
        .map(module_name_from_capture_stem)
        .unwrap_or_else(|| DEFAULT_MODULE_NAME.to_owned())
}

#[must_use]
pub fn module_name_from_resource_name(name: &str) -> String {
    Path::new(name)
        .file_stem()
        .and_then(|stem| stem.to_str())
        .map(module_name_from_capture_stem)
        .unwrap_or_else(|| DEFAULT_MODULE_NAME.to_owned())
}

#[must_use]
pub fn module_name_from_capture_stem(stem: &str) -> String {
    let stem = stem
        .strip_suffix("-module")
        .or_else(|| stem.strip_suffix("-gem"))
        .unwrap_or(stem)
        .trim_matches(['-', '_', ' ']);
    if stem.is_empty()
        || matches!(
            stem.to_ascii_lowercase().as_str(),
            "module" | "newworld" | "new-world" | "new_world" | "nw"
        )
    {
        return DEFAULT_MODULE_NAME.to_owned();
    }

    stem.split(['-', '_', ' ', '.'])
        .filter(|segment| !segment.is_empty())
        .map(canonical_module_segment)
        .collect()
}

#[must_use]
pub fn is_module_descriptor_json_name(name: &str) -> bool {
    let lower = name.to_ascii_lowercase();
    lower.ends_with(".json") && !lower.ends_with(".debug.json") && !lower.ends_with(".renames.json")
}

fn canonical_module_segment(segment: &str) -> String {
    match segment.to_ascii_lowercase().as_str() {
        "ai" => "AI".to_owned(),
        "api" => "API".to_owned(),
        "aws" => "AWS".to_owned(),
        "ebus" => "EBus".to_owned(),
        "gui" => "GUI".to_owned(),
        "id" => "ID".to_owned(),
        "ids" => "IDs".to_owned(),
        "lmbr" => "Lmbr".to_owned(),
        "ly" => "Ly".to_owned(),
        "rhi" => "RHI".to_owned(),
        "rtti" => "RTTI".to_owned(),
        "sdk" => "SDK".to_owned(),
        "ui" => "UI".to_owned(),
        "uuid" => "Uuid".to_owned(),
        _ => segment.to_upper_camel_case(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn module_capture_stems_become_lumberyard_style_module_names() {
        assert_eq!(module_name_from_capture_stem("module"), "NewWorld");
        assert_eq!(module_name_from_capture_stem("nw-module"), "NewWorld");
        assert_eq!(
            module_name_from_capture_stem("lmbr-central-module"),
            "LmbrCentral"
        );
        assert_eq!(module_name_from_capture_stem("rain-gem"), "Rain");
        assert_eq!(
            module_name_from_capture_stem("amazon-games-sdk-module"),
            "AmazonGamesSDK"
        );
        assert_eq!(
            module_name_from_capture_stem("javelin-components-ai-module"),
            "JavelinComponentsAI"
        );
    }
}
