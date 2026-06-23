use heck::ToSnakeCase;

use crate::{ReflectedField, rust_reflected_type_name as rust_type_name};

pub(super) fn module_name_for_component(component_name: &str) -> String {
    let base = component_name
        .strip_suffix("ComponentServerFacet")
        .or_else(|| component_name.strip_suffix("ComponentClientFacet"))
        .or_else(|| component_name.strip_suffix("Component"))
        .or_else(|| component_name.strip_suffix("ServerFacet"))
        .or_else(|| component_name.strip_suffix("ClientFacet"))
        .unwrap_or(component_name);
    base.to_snake_case()
}

pub(super) fn inferred_facet_owner(facet_name: &str) -> Option<String> {
    facet_name
        .strip_suffix("ComponentServerFacet")
        .or_else(|| facet_name.strip_suffix("ComponentClientFacet"))
        .map(|prefix| format!("{prefix}Component"))
        .or_else(|| {
            facet_name
                .strip_suffix("ServerFacet")
                .or_else(|| facet_name.strip_suffix("ClientFacet"))
                .map(|prefix| format!("{prefix}Component"))
        })
}

pub(super) fn is_facet_type_name(name: &str) -> bool {
    name.ends_with("ServerFacet") || name.ends_with("ClientFacet")
}

pub(super) fn is_facet_pointer_field(field_name: &str) -> bool {
    matches!(field_name, "m_clientFacetPtr" | "m_serverFacetPtr")
}

pub(super) fn facet_bundle_field_name(facet_name: &str) -> String {
    if facet_name.ends_with("ClientFacet") {
        "client_facet".to_owned()
    } else if facet_name.ends_with("ServerFacet") {
        "server_facet".to_owned()
    } else {
        facet_name.to_snake_case()
    }
}

pub(super) fn snake_field_name(name: &str) -> String {
    let trimmed = trim_member_prefix(name);
    let normalized = normalize_legacy_acronyms(trimmed);
    let field = normalized.to_snake_case();
    if field.is_empty() {
        "field".to_owned()
    } else if is_rust_keyword(&field) {
        format!("{field}_field")
    } else {
        field
    }
}

pub(super) fn base_class_field_name(field: &ReflectedField) -> String {
    let field_name = field
        .type_name
        .as_deref()
        .map(|type_name| module_name_for_component(&rust_type_name(type_name, field.type_id)))
        .unwrap_or_else(|| snake_field_name(&field.name));
    if field_name.is_empty() {
        "base".to_owned()
    } else if is_rust_keyword(&field_name) {
        format!("{field_name}_field")
    } else {
        field_name
    }
}

pub(super) fn pascal_case(value: &str) -> String {
    let mut out = String::new();
    let mut capitalize = true;
    for ch in value.chars() {
        if !ch.is_ascii_alphanumeric() {
            capitalize = true;
            continue;
        }
        if capitalize {
            out.extend(ch.to_uppercase());
            capitalize = false;
        } else {
            out.push(ch);
        }
    }
    out
}

fn trim_member_prefix(name: &str) -> &str {
    if let Some(value) = name.strip_prefix("m_") {
        return value;
    }

    if let Some(value) = name.strip_prefix('m')
        && value
            .chars()
            .next()
            .is_some_and(|ch| ch.is_ascii_uppercase())
    {
        return value;
    }

    name
}

fn normalize_legacy_acronyms(value: &str) -> String {
    value.replace("GDEID", "GDEId")
}

fn is_rust_keyword(value: &str) -> bool {
    matches!(
        value,
        "as" | "break"
            | "const"
            | "continue"
            | "crate"
            | "else"
            | "enum"
            | "extern"
            | "false"
            | "fn"
            | "for"
            | "if"
            | "impl"
            | "in"
            | "let"
            | "loop"
            | "match"
            | "mod"
            | "move"
            | "mut"
            | "pub"
            | "ref"
            | "return"
            | "self"
            | "Self"
            | "static"
            | "struct"
            | "super"
            | "trait"
            | "true"
            | "type"
            | "unsafe"
            | "use"
            | "where"
            | "while"
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn snake_field_name_preserves_legacy_acronym_boundaries() {
        assert_eq!(
            snake_field_name("m_prefabPersistenceGDEID"),
            "prefab_persistence_gde_id"
        );
        assert_eq!(
            snake_field_name("m_remoteServerGDERef"),
            "remote_server_gde_ref"
        );
        assert_eq!(snake_field_name("m_UIDValue"), "uid_value");
    }
}
