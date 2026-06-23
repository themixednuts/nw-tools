use super::super::typescript_string_literal;

pub(super) fn viteplus_package_json(package_name: &str) -> String {
    TYPESCRIPT_VITEPLUS_PACKAGE_JSON.replace("{{PACKAGE_NAME}}", package_name)
}

pub(super) fn viteplus_config(pack_entries: &[String]) -> String {
    let pack_entry = if pack_entries.is_empty() {
        String::new()
    } else {
        format!(
            "\t\tentry: [{}],\n",
            pack_entries
                .iter()
                .map(|entry| typescript_string_literal(entry))
                .collect::<Vec<_>>()
                .join(", ")
        )
    };
    TYPESCRIPT_VITEPLUS_CONFIG.replace("{{PACK_ENTRY}}", &pack_entry)
}

const TYPESCRIPT_VITEPLUS_PACKAGE_JSON: &str =
    include_str!("../../../../resources/typescript/viteplus/package.json");
pub(super) const TYPESCRIPT_VITEPLUS_TSCONFIG: &str =
    include_str!("../../../../resources/typescript/viteplus/tsconfig.json");
const TYPESCRIPT_VITEPLUS_CONFIG: &str =
    include_str!("../../../../resources/typescript/viteplus/vite.config.ts");
