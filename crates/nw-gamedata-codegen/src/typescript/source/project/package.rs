use super::super::typescript_string_literal;
use crate::project::{
    TYPESCRIPT_NODE_TYPES, TYPESCRIPT_PACKAGE_MANAGER, TYPESCRIPT_VERSION, VITE_PLUS_CORE_OVERRIDE,
    VITE_PLUS_TEST_OVERRIDE, VITE_PLUS_VERSION,
};

pub(super) fn viteplus_package_json(package_name: &str) -> String {
    let manifest = serde_json::json!({
        "name": package_name,
        "version": "0.0.0",
        "description": "New World GameData TypeScript package.",
        "files": ["dist"],
        "type": "module",
        "exports": {
            ".": "./dist/index.mjs",
            "./package.json": "./package.json",
        },
        "scripts": {
            "build": "vp pack",
            "check": "tsc --noEmit",
        },
        "dependencies": {
            "fast-xml-parser": "5.9.3",
        },
        "devDependencies": {
            "@types/node": TYPESCRIPT_NODE_TYPES,
            "typescript": TYPESCRIPT_VERSION,
            "vite-plus": VITE_PLUS_VERSION,
        },
        "overrides": {
            "vite": VITE_PLUS_CORE_OVERRIDE,
            "vitest": VITE_PLUS_TEST_OVERRIDE,
        },
        "packageManager": TYPESCRIPT_PACKAGE_MANAGER,
    });
    let mut package_json =
        serde_json::to_string_pretty(&manifest).expect("manifest contains only JSON values");
    package_json.push('\n');
    package_json
}

pub(super) fn viteplus_config(pack_entries: &[String]) -> String {
    let pack_entry = if pack_entries.is_empty() {
        String::new()
    } else {
        format!(
            ",\n    entry: [{}]",
            pack_entries
                .iter()
                .map(|entry| typescript_string_literal(entry))
                .collect::<Vec<_>>()
                .join(", ")
        )
    };
    TYPESCRIPT_VITEPLUS_CONFIG.replace("{{PACK_ENTRY}}", &pack_entry)
}

pub(super) const TYPESCRIPT_VITEPLUS_TSCONFIG: &str =
    include_str!("../../../../resources/typescript/viteplus/tsconfig.json");
const TYPESCRIPT_VITEPLUS_CONFIG: &str =
    include_str!("../../../../resources/typescript/viteplus/vite.config.ts");
