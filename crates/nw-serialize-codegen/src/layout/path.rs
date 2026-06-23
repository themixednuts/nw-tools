use std::borrow::Cow;

use heck::ToSnakeCase;

use crate::naming::{ParsedSourceName, SourceNameKind, rust_type_ident};

#[must_use]
pub fn source_namespace_segments(source_name: &str) -> Vec<String> {
    let namespace_source = semantic_namespace_source(source_name);
    namespace_source
        .rsplit_once("::")
        .map(|(namespace, _)| {
            namespace
                .split("::")
                .filter(|segment| !segment.is_empty())
                .map(sanitize_path_segment)
                .collect()
        })
        .unwrap_or_default()
}

pub(crate) fn source_scope_segments(source_name: &str) -> Vec<String> {
    let parsed = ParsedSourceName::parse(source_name);
    if let SourceNameKind::TemplateWrapper { wrapper, .. } = parsed.kind {
        let mut segments = source_namespace_segments(&wrapper);
        segments.push(inheritance_scope_segment(&wrapper));
        return segments;
    }
    source_namespace_segments(source_name)
}

fn semantic_namespace_source(source_name: &str) -> Cow<'_, str> {
    let parsed = ParsedSourceName::parse(source_name);
    match parsed.kind {
        SourceNameKind::EditEnum { target } | SourceNameKind::LocalComponentRef { target, .. } => {
            Cow::Owned(target)
        }
        SourceNameKind::TemplateWrapper { wrapper, .. } => Cow::Owned(wrapper),
        SourceNameKind::GetTypeNameFunction(function) => Cow::Owned(function.target_type),
        SourceNameKind::Plain => Cow::Borrowed(source_name),
    }
}

#[must_use]
pub fn sanitize_path_segment(segment: &str) -> String {
    let mut value = segment
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
    while value.contains("__") {
        value = value.replace("__", "_");
    }
    let value = value.trim_matches('_');
    let mut value = if value.is_empty() {
        "types".to_owned()
    } else {
        value.to_owned()
    };
    if value.chars().next().is_some_and(|ch| ch.is_ascii_digit()) {
        value.insert_str(0, "type_");
    }
    value
}

pub fn inheritance_scope_segment(base_name: &str) -> String {
    let segment = sanitize_path_segment(&rust_type_ident(base_name));
    if is_interface_or_base_scope(&segment) {
        return segment;
    }
    pluralize_scope_segment(&segment)
}

pub(crate) fn concrete_type_scope_segment(source_name: &str) -> String {
    sanitize_path_segment(&rust_type_ident(source_name))
}

fn is_interface_or_base_scope(segment: &str) -> bool {
    segment == "base"
        || segment.ends_with("_base")
        || segment.ends_with("_interface")
        || segment.starts_with("i_")
        || segment.starts_with("interface_")
}

fn pluralize_scope_segment(segment: &str) -> String {
    if let Some(prefix) = segment.strip_suffix("child") {
        format!("{prefix}children")
    } else if segment.ends_with('s') {
        segment.to_owned()
    } else if let Some(prefix) = segment.strip_suffix('y') {
        format!("{prefix}ies")
    } else {
        format!("{segment}s")
    }
}
