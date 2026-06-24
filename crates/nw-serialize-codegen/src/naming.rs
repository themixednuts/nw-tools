use std::collections::BTreeMap;

use heck::{ToSnakeCase, ToUpperCamelCase};
use uuid::Uuid;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SourceNameKind {
    Plain,
    EditEnum {
        target: String,
    },
    TemplateWrapper {
        wrapper: String,
        target: String,
    },
    LocalComponentRef {
        target: String,
        get_type_name: Option<CppGetTypeNameFunction>,
    },
    GetTypeNameFunction(CppGetTypeNameFunction),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CppGetTypeNameFunction {
    pub declaration: String,
    pub target_type: String,
    pub calling_convention: Option<CppCallingConvention>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CppCallingConvention {
    Cdecl,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParsedSourceName {
    pub original: String,
    pub kind: SourceNameKind,
}

impl ParsedSourceName {
    #[must_use]
    pub fn parse(source: &str) -> Self {
        let get_type_name = CppGetTypeNameFunction::parse(source);
        let kind = if let Some(target) = edit_enum_target(source) {
            SourceNameKind::EditEnum { target }
        } else if source.starts_with("LocalComponentRef<") {
            if let Some(function) = get_type_name {
                SourceNameKind::LocalComponentRef {
                    target: function.target_type.clone(),
                    get_type_name: Some(function),
                }
            } else if let Some(target) = local_component_ref_direct_target(source) {
                SourceNameKind::LocalComponentRef {
                    target,
                    get_type_name: None,
                }
            } else {
                SourceNameKind::Plain
            }
        } else if let Some((wrapper, target)) = template_wrapper(source) {
            SourceNameKind::TemplateWrapper { wrapper, target }
        } else {
            get_type_name
                .map(SourceNameKind::GetTypeNameFunction)
                .unwrap_or(SourceNameKind::Plain)
        };
        Self {
            original: source.to_owned(),
            kind,
        }
    }

    #[must_use]
    pub fn rust_type_ident(&self) -> String {
        rust_type_ident_from_source_name(self)
    }

    #[must_use]
    pub fn rust_type_name(&self) -> String {
        self.rust_type_ident()
    }
}

#[must_use]
pub fn rust_type_ident(source_name: &str) -> String {
    ParsedSourceName::parse(source_name).rust_type_ident()
}

#[must_use]
pub fn rust_type_name(source_name: &str) -> String {
    ParsedSourceName::parse(source_name).rust_type_name()
}

#[must_use]
pub fn rust_variant_ident(source_name: &str) -> String {
    let mut ident = source_name.to_upper_camel_case();
    if ident.is_empty() {
        ident = "Variant".to_owned();
    }
    if ident
        .as_bytes()
        .first()
        .is_some_and(|byte| byte.is_ascii_digit())
    {
        ident.insert_str(0, "Variant");
    }
    if is_rust_keyword(&ident) {
        ident.push('_');
    }
    ident
}

#[must_use]
pub fn rust_type_names_by_id<'a, I>(types: I) -> BTreeMap<Uuid, String>
where
    I: IntoIterator<Item = (Uuid, &'a str)>,
{
    let mut candidate_ids = BTreeMap::<String, Vec<(Uuid, String)>>::new();
    for (type_id, source_name) in types {
        if type_id.is_nil() {
            continue;
        }
        candidate_ids
            .entry(rust_type_ident(source_name))
            .or_default()
            .push((type_id, source_name.to_owned()));
    }

    let mut names_by_id = BTreeMap::new();
    for (candidate, mut entries) in candidate_ids {
        entries.sort_by(|(left_id, left_name), (right_id, right_name)| {
            left_id
                .cmp(right_id)
                .then_with(|| left_name.cmp(right_name))
        });
        if entries.len() == 1 {
            names_by_id.insert(entries[0].0, candidate);
            continue;
        }
        if let Some((type_id, _)) = source_preferred_entry(&entries) {
            names_by_id.insert(type_id, candidate.clone());
            entries.retain(|(entry_id, _)| *entry_id != type_id);
        }
        for (type_id, _) in entries {
            names_by_id.insert(type_id, format!("{candidate}{}", short_type_id(type_id)));
        }
    }
    names_by_id
}

fn source_preferred_entry<T>(entries: &[T]) -> Option<T>
where
    T: Clone + SourceNamedEntry,
{
    entries
        .iter()
        .find(|entry| entry.source_name() == "AZ::Component")
        .cloned()
}

trait SourceNamedEntry {
    fn source_name(&self) -> &str;
}

impl SourceNamedEntry for (Uuid, String) {
    fn source_name(&self) -> &str {
        &self.1
    }
}

#[must_use]
pub fn scoped_rust_type_names_by_id<'a, I>(types: I) -> BTreeMap<Uuid, String>
where
    I: IntoIterator<Item = (Uuid, Vec<String>, &'a str)>,
{
    let mut candidate_ids = BTreeMap::<(Vec<String>, String), Vec<(Uuid, String)>>::new();
    for (type_id, scope, source_name) in types {
        if type_id.is_nil() {
            continue;
        }
        candidate_ids
            .entry((scope, rust_type_ident(source_name)))
            .or_default()
            .push((type_id, source_name.to_owned()));
    }

    let mut names_by_id = BTreeMap::new();
    for ((_, candidate), mut entries) in candidate_ids {
        entries.sort_by(|(left_id, left_name), (right_id, right_name)| {
            left_id
                .cmp(right_id)
                .then_with(|| left_name.cmp(right_name))
        });
        if entries.len() == 1 {
            names_by_id.insert(entries[0].0, candidate);
            continue;
        }
        for (type_id, _) in entries {
            names_by_id.insert(type_id, format!("{candidate}{}", short_type_id(type_id)));
        }
    }
    names_by_id
}

#[must_use]
pub fn rust_reflected_type_name(source_name: &str, type_id: Uuid) -> String {
    if source_name.starts_with("legacy:") {
        let short_id = type_id
            .simple()
            .to_string()
            .chars()
            .take(8)
            .collect::<String>();
        return format!("Unknown{}Component", short_id.to_ascii_uppercase());
    }

    let leaf = source_name
        .rsplit([':', '/', '\\'])
        .find(|part| !part.is_empty())
        .unwrap_or(source_name);
    let mut name = reflected_pascal_case(leaf);
    if name.is_empty() {
        name = "UnknownComponent".to_owned();
    }
    if name.chars().next().is_some_and(|ch| ch.is_ascii_digit()) {
        name.insert_str(0, "Component");
    }
    name
}

#[must_use]
pub fn missing_reflected_type_name(type_id: Uuid) -> String {
    format!("Type{}", short_type_id(type_id))
}

#[must_use]
pub fn rust_field_ident(source_name: &str) -> String {
    let mut ident = split_identifier_words(strip_member_prefix(source_name))
        .into_iter()
        .map(str::to_ascii_lowercase)
        .collect::<Vec<_>>()
        .join("_");
    if ident.is_empty() {
        ident = "field".to_owned();
    }
    if ident
        .as_bytes()
        .first()
        .is_some_and(|byte| byte.is_ascii_digit())
    {
        ident.insert_str(0, "field_");
    }
    if is_rust_keyword(&ident) {
        ident.push('_');
    }
    ident
}

#[must_use]
pub fn rust_module_ident(source_name: &str) -> String {
    let mut module = source_name
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
    while module.contains("__") {
        module = module.replace("__", "_");
    }
    let module = module.trim_matches('_');
    let mut module = if module.is_empty() {
        "types".to_owned()
    } else {
        module.to_owned()
    };
    if module.chars().next().is_some_and(|ch| ch.is_ascii_digit()) {
        module.insert_str(0, "m_");
    }
    if is_rust_keyword(&module) {
        module.push('_');
    }
    module
}

fn short_type_id(type_id: Uuid) -> String {
    type_id
        .simple()
        .to_string()
        .chars()
        .take(8)
        .collect::<String>()
        .to_ascii_uppercase()
}

fn rust_type_ident_from_source_name(name: &ParsedSourceName) -> String {
    match &name.kind {
        SourceNameKind::Plain => rust_ident_from_leaf(cpp_type_leaf(&name.original)),
        SourceNameKind::EditEnum { target } => {
            format!("EditEnum{}", rust_ident_from_leaf(cpp_type_leaf(target)))
        }
        SourceNameKind::TemplateWrapper { wrapper, target } => {
            format!(
                "{}{}",
                rust_ident_from_leaf(cpp_type_leaf(wrapper)),
                rust_ident_from_leaf(cpp_type_leaf(target))
            )
        }
        SourceNameKind::LocalComponentRef { target, .. } => {
            format!(
                "LocalComponentRef{}",
                rust_ident_from_leaf(cpp_type_leaf(target))
            )
        }
        SourceNameKind::GetTypeNameFunction(function) => {
            rust_ident_from_leaf(cpp_type_leaf(&function.target_type))
        }
    }
}

impl CppGetTypeNameFunction {
    #[must_use]
    pub fn parse(source: &str) -> Option<Self> {
        if !source.contains("GetTypeName<") {
            return None;
        }
        let declaration = get_type_name_declaration(source).unwrap_or(source);
        let syntax = parse_cpp_declaration(declaration).or_else(|| {
            Some(CppDeclarationSyntax {
                target_type: get_type_name_template_argument(source),
                calling_convention: source
                    .split(|ch: char| !ch.is_ascii_alphanumeric() && ch != '_')
                    .any(|token| token == "__cdecl")
                    .then_some(CppCallingConvention::Cdecl),
            })
        })?;
        Some(Self {
            declaration: declaration.to_owned(),
            target_type: syntax.target_type?,
            calling_convention: syntax.calling_convention,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct CppDeclarationSyntax {
    target_type: Option<String>,
    calling_convention: Option<CppCallingConvention>,
}

fn parse_cpp_declaration(declaration: &str) -> Option<CppDeclarationSyntax> {
    let calling_convention = declaration
        .split(|ch: char| !ch.is_ascii_alphanumeric() && ch != '_')
        .any(|token| token == "__cdecl")
        .then_some(CppCallingConvention::Cdecl);
    let source = format!("{};", declaration.replace("__cdecl", ""));
    let mut parser = treesitter_types_cpp::tree_sitter::Parser::new();
    parser
        .set_language(&treesitter_types_cpp::tree_sitter_cpp::LANGUAGE.into())
        .ok()?;
    let tree = parser.parse(source.as_bytes(), None)?;
    let root = tree.root_node();
    if root.has_error() {
        return None;
    }

    let mut nodes = Vec::new();
    collect_cpp_nodes(root, source.as_bytes(), &mut nodes);
    let target_type = nodes
        .iter()
        .find_map(|node| get_type_name_template_argument(&node.text))
        .or_else(|| {
            nodes
                .iter()
                .filter(|node| {
                    matches!(
                        node.kind.as_str(),
                        "qualified_identifier"
                            | "type_identifier"
                            | "template_type"
                            | "dependent_type"
                            | "template_function"
                            | "template_method"
                    )
                })
                .filter_map(|node| normalized_cpp_target_type(&node.text))
                .max_by_key(|candidate| candidate.len())
        });

    Some(CppDeclarationSyntax {
        target_type,
        calling_convention,
    })
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct CppSyntaxNode {
    kind: String,
    text: String,
}

fn collect_cpp_nodes(
    node: treesitter_types_cpp::tree_sitter::Node<'_>,
    source: &[u8],
    nodes: &mut Vec<CppSyntaxNode>,
) {
    if node.is_named()
        && let Ok(text) = node.utf8_text(source)
    {
        nodes.push(CppSyntaxNode {
            kind: node.kind().to_owned(),
            text: text.to_owned(),
        });
    }

    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        collect_cpp_nodes(child, source, nodes);
    }
}

fn get_type_name_template_argument(text: &str) -> Option<String> {
    let start = text.find("GetTypeName<")? + "GetTypeName<".len();
    let mut depth = 0usize;
    let mut end = None;
    for (offset, ch) in text[start..].char_indices() {
        match ch {
            '<' => depth += 1,
            '>' if depth == 0 => {
                end = Some(start + offset);
                break;
            }
            '>' => depth -= 1,
            _ => {}
        }
    }
    let candidate = if let Some(end) = end {
        text[start..end].trim()
    } else {
        let tail = text[start..].trim();
        let end = tail.find(['(', ')', ';']).unwrap_or(tail.len());
        tail[..end].trim().trim_end_matches('>').trim()
    };
    normalized_cpp_target_type(candidate)
}

fn normalized_cpp_target_type(text: &str) -> Option<String> {
    let text = text.trim();
    let text = text
        .strip_prefix("class ")
        .or_else(|| text.strip_prefix("struct "))
        .unwrap_or(text)
        .trim();
    if text.contains("GetTypeName") || text.contains("__cdecl") || text.contains(';') {
        return None;
    }
    if text.split("::").all(is_cpp_identifier) {
        return Some(text.to_owned());
    }
    None
}

fn is_cpp_identifier(text: &str) -> bool {
    let mut chars = text.chars();
    let Some(first) = chars.next() else {
        return false;
    };
    (first.is_ascii_alphabetic() || first == '_')
        && chars.all(|ch| ch.is_ascii_alphanumeric() || ch == '_')
}

fn get_type_name_declaration(source: &str) -> Option<&str> {
    let start = source.find("GetTypeName<")?;
    let declaration_start = source[..start]
        .rfind("const char")
        .or_else(|| source[..start].rfind("char"))?;
    let declaration_end = source[declaration_start..]
        .find("(void)")
        .map(|offset| declaration_start + offset + "(void)".len())?;
    Some(source[declaration_start..declaration_end].trim())
}

fn rust_ident_from_leaf(name: &str) -> String {
    let mut ident = name
        .split(|ch: char| !ch.is_ascii_alphanumeric())
        .filter(|part| !part.is_empty())
        .map(title_case)
        .collect::<String>();
    if ident.is_empty() {
        ident = "Unnamed".to_owned();
    } else if ident
        .as_bytes()
        .first()
        .is_some_and(|byte| byte.is_ascii_digit())
    {
        ident.insert_str(0, "Type");
    }
    if is_rust_keyword(&ident) {
        ident.push('_');
    }
    ident
}

fn split_identifier_words(value: &str) -> Vec<&str> {
    let mut words = Vec::new();
    let mut start = None;
    let chars = value.char_indices().collect::<Vec<_>>();
    for (index, (byte_index, ch)) in chars.iter().copied().enumerate() {
        if !ch.is_ascii_alphanumeric() {
            if let Some(word_start) = start.take()
                && word_start < byte_index
            {
                words.push(&value[word_start..byte_index]);
            }
            continue;
        }
        if start.is_none() {
            start = Some(byte_index);
            continue;
        }

        let previous = chars[index - 1].1;
        let next = chars.get(index + 1).map(|(_, next)| *next);
        let starts_new_word = (previous.is_ascii_lowercase() && ch.is_ascii_uppercase())
            || (previous.is_ascii_alphabetic() && ch.is_ascii_digit())
            || (previous.is_ascii_digit() && ch.is_ascii_alphabetic())
            || (previous.is_ascii_uppercase()
                && ch.is_ascii_uppercase()
                && next.is_some_and(|next| next.is_ascii_lowercase()));
        if starts_new_word && let Some(word_start) = start.replace(byte_index) {
            words.push(&value[word_start..byte_index]);
        }
    }
    if let Some(word_start) = start {
        words.push(&value[word_start..]);
    }
    words
}

fn edit_enum_target(source: &str) -> Option<String> {
    let prefix = "EditEnum<EnumType><";
    let target = source.strip_prefix(prefix)?;
    let target = target.strip_suffix('>')?.trim();
    (!target.is_empty()).then(|| target.to_owned())
}

fn template_wrapper(source: &str) -> Option<(String, String)> {
    let open = source.find('<')?;
    let wrapper = source[..open].trim();
    if wrapper.is_empty() {
        return None;
    }

    let mut cursor = open;
    let mut arguments = Vec::new();
    while cursor < source.len() {
        let Some(start_offset) = source[cursor..].find(|ch: char| !ch.is_whitespace()) else {
            break;
        };
        cursor += start_offset;
        if !source[cursor..].starts_with('<') {
            return None;
        }

        let mut depth = 0usize;
        let mut close = None;
        for (offset, ch) in source[cursor..].char_indices() {
            match ch {
                '<' => depth += 1,
                '>' => {
                    depth = depth.checked_sub(1)?;
                    if depth == 0 {
                        close = Some(cursor + offset);
                        break;
                    }
                }
                _ => {}
            }
        }

        let close = close?;
        let target = source[cursor + 1..close].trim();
        if target.is_empty() {
            return None;
        }
        arguments.push(target.to_owned());
        cursor = close + 1;
    }

    arguments.pop().map(|target| (wrapper.to_owned(), target))
}

fn local_component_ref_direct_target(source: &str) -> Option<String> {
    let prefix = "LocalComponentRef<";
    let mut depth = 1usize;
    let mut close = None;
    for (offset, ch) in source.strip_prefix(prefix)?.char_indices() {
        match ch {
            '<' => depth += 1,
            '>' => {
                depth = depth.checked_sub(1)?;
                if depth == 0 {
                    close = Some(prefix.len() + offset);
                    break;
                }
            }
            _ => {}
        }
    }

    let close = close?;
    let target = source[prefix.len()..close].trim();
    let suffix = source[close + 1..].trim();
    if suffix != "::GetTypeName" {
        return None;
    }
    normalized_cpp_target_type(target)
}

fn cpp_type_leaf(name: &str) -> &str {
    let mut depth = 0usize;
    let mut leaf_start = 0usize;
    let mut chars = name.char_indices().peekable();
    while let Some((index, ch)) = chars.next() {
        match ch {
            '<' => depth += 1,
            '>' => depth = depth.saturating_sub(1),
            ':' if depth == 0 && chars.peek().is_some_and(|(_, next)| *next == ':') => {
                chars.next();
                leaf_start = index + 2;
            }
            _ => {}
        }
    }
    name[leaf_start..].trim()
}

fn strip_member_prefix(name: &str) -> &str {
    name.strip_prefix("m_").unwrap_or(name)
}

fn title_case(part: &str) -> String {
    let mut chars = part.chars();
    let Some(first) = chars.next() else {
        return String::new();
    };
    let mut ident = String::new();
    ident.push(first.to_ascii_uppercase());
    ident.extend(chars);
    ident
}

fn reflected_pascal_case(value: &str) -> String {
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

fn is_rust_keyword(value: &str) -> bool {
    matches!(
        value,
        "abstract"
            | "as"
            | "async"
            | "await"
            | "become"
            | "box"
            | "break"
            | "const"
            | "continue"
            | "crate"
            | "do"
            | "dyn"
            | "else"
            | "enum"
            | "extern"
            | "false"
            | "final"
            | "fn"
            | "for"
            | "if"
            | "impl"
            | "in"
            | "let"
            | "loop"
            | "macro"
            | "match"
            | "mod"
            | "move"
            | "mut"
            | "override"
            | "pub"
            | "priv"
            | "ref"
            | "return"
            | "self"
            | "Self"
            | "static"
            | "struct"
            | "super"
            | "trait"
            | "true"
            | "try"
            | "type"
            | "typeof"
            | "unsafe"
            | "unsized"
            | "use"
            | "virtual"
            | "where"
            | "while"
            | "yield"
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_get_type_name_function_target() {
        let name = "const char *__cdecl MB::GetTypeName<class Javelin::BeamAttackComponent>(void)";
        let parsed = ParsedSourceName::parse(name);

        assert_eq!(
            parsed.kind,
            SourceNameKind::GetTypeNameFunction(CppGetTypeNameFunction {
                declaration:
                    "const char *__cdecl MB::GetTypeName<class Javelin::BeamAttackComponent>(void)"
                        .to_owned(),
                target_type: "Javelin::BeamAttackComponent".to_owned(),
                calling_convention: Some(CppCallingConvention::Cdecl),
            })
        );
        assert_eq!(parsed.rust_type_ident(), "BeamAttackComponent");
    }

    #[test]
    fn parses_local_component_ref_target() {
        let name = "LocalComponentRef<InterfaceType><const char *__cdecl MB::GetTypeName<class Javelin::ActionListComponent>(void)>";
        let parsed = ParsedSourceName::parse(name);

        assert_eq!(
            parsed.kind,
            SourceNameKind::LocalComponentRef {
                target: "Javelin::ActionListComponent".to_owned(),
                get_type_name: Some(CppGetTypeNameFunction {
                    declaration:
                        "const char *__cdecl MB::GetTypeName<class Javelin::ActionListComponent>(void)"
                            .to_owned(),
                    target_type: "Javelin::ActionListComponent".to_owned(),
                    calling_convention: Some(CppCallingConvention::Cdecl),
                }),
            }
        );
        assert_eq!(
            parsed.rust_type_ident(),
            "LocalComponentRefActionListComponent"
        );
    }

    #[test]
    fn parses_truncated_local_component_ref_function_signature() {
        let name = "LocalComponentRef<InterfaceType><const char *__cdecl MB::GetTypeName<class Javelin::SpellTargetIndicatorManagerComponent>(void";
        let parsed = ParsedSourceName::parse(name);

        assert!(matches!(
            parsed.kind,
            SourceNameKind::LocalComponentRef { ref target, .. }
                if target == "Javelin::SpellTargetIndicatorManagerComponent"
        ));
        assert_eq!(
            parsed.rust_type_ident(),
            "LocalComponentRefSpellTargetIndicatorManagerComponent"
        );
    }

    #[test]
    fn parses_truncated_local_component_ref_template_target() {
        let name = "LocalComponentRef<InterfaceType><const char *__cdecl MB::GetTypeName<class Javelin::VitalsComponent";
        let parsed = ParsedSourceName::parse(name);

        assert!(matches!(
            parsed.kind,
            SourceNameKind::LocalComponentRef { ref target, .. }
                if target == "Javelin::VitalsComponent"
        ));
        assert_eq!(parsed.rust_type_ident(), "LocalComponentRefVitalsComponent");
    }

    #[test]
    fn parses_unqualified_local_component_ref_target() {
        let name = "LocalComponentRef<InterfaceType><const char *__cdecl MB::GetTypeName<class TestInterface>(void)>";
        let parsed = ParsedSourceName::parse(name);

        assert!(matches!(
            parsed.kind,
            SourceNameKind::LocalComponentRef { ref target, .. } if target == "TestInterface"
        ));
        assert_eq!(parsed.rust_type_ident(), "LocalComponentRefTestInterface");
    }

    #[test]
    fn parses_direct_local_component_ref_get_type_name_target() {
        let parsed = ParsedSourceName::parse("LocalComponentRef<TransformComponent>::GetTypeName");

        assert_eq!(
            parsed.kind,
            SourceNameKind::LocalComponentRef {
                target: "TransformComponent".to_owned(),
                get_type_name: None,
            }
        );
        assert_eq!(
            parsed.rust_type_ident(),
            "LocalComponentRefTransformComponent"
        );
    }

    #[test]
    fn preserves_template_wrapper_when_template_argument_is_qualified() {
        let name = "EditEnum<EnumType><Javelin::SBItemClass::ItemClasses >";
        let parsed = ParsedSourceName::parse(name);

        assert_eq!(
            parsed.kind,
            SourceNameKind::EditEnum {
                target: "Javelin::SBItemClass::ItemClasses".to_owned(),
            }
        );
        assert_eq!(parsed.rust_type_ident(), "EditEnumItemClasses");
    }

    #[test]
    fn parses_single_template_wrapper_without_collapsing_to_target() {
        let name = "RemoteServerFacetRef<PlayerComponentServerFacet >";
        let parsed = ParsedSourceName::parse(name);

        assert_eq!(
            parsed.kind,
            SourceNameKind::TemplateWrapper {
                wrapper: "RemoteServerFacetRef".to_owned(),
                target: "PlayerComponentServerFacet".to_owned(),
            }
        );
        assert_eq!(
            parsed.rust_type_ident(),
            "RemoteServerFacetRefPlayerComponentServerFacet"
        );
    }

    #[test]
    fn parses_two_stage_template_wrapper_by_payload_argument() {
        let name = "SimpleAssetReference<AssetType><BinkAsset >";
        let parsed = ParsedSourceName::parse(name);

        assert_eq!(
            parsed.kind,
            SourceNameKind::TemplateWrapper {
                wrapper: "SimpleAssetReference".to_owned(),
                target: "BinkAsset".to_owned(),
            }
        );
        assert_eq!(parsed.rust_type_ident(), "SimpleAssetReferenceBinkAsset");
    }

    #[test]
    fn parses_namespaced_template_wrapper_without_collapsing_to_target_namespace() {
        let name = "AzFramework::SimpleAssetReference<MB::DataSheetAsset>";
        let parsed = ParsedSourceName::parse(name);

        assert_eq!(
            parsed.kind,
            SourceNameKind::TemplateWrapper {
                wrapper: "AzFramework::SimpleAssetReference".to_owned(),
                target: "MB::DataSheetAsset".to_owned(),
            }
        );
        assert_eq!(
            parsed.rust_type_ident(),
            "SimpleAssetReferenceDataSheetAsset"
        );
    }

    #[test]
    fn suffixes_reflected_type_name_collisions_by_type_id() {
        let first = Uuid::parse_str("11111111-1111-1111-1111-111111111111").unwrap();
        let second = Uuid::parse_str("22222222-2222-2222-2222-222222222222").unwrap();

        let names = rust_type_names_by_id([
            (first, "Example::RangedAttackData"),
            (second, "Other::RangedAttackData"),
        ]);

        assert_eq!(
            names.get(&first).map(String::as_str),
            Some("RangedAttackData11111111")
        );
        assert_eq!(
            names.get(&second).map(String::as_str),
            Some("RangedAttackData22222222")
        );
    }

    #[test]
    fn global_names_suffix_all_leaf_collisions() {
        let unqualified = Uuid::parse_str("ef435e77-5f70-4b2d-a959-41aba75890ea").unwrap();
        let namespaced = Uuid::parse_str("edFCb2cf-f75d-43be-b26b-f35821b29247").unwrap();

        let names =
            rust_type_names_by_id([(unqualified, "Component"), (namespaced, "AZ::Component")]);

        assert_eq!(
            names.get(&namespaced).map(String::as_str),
            Some("Component")
        );
        assert_eq!(
            names.get(&unqualified).map(String::as_str),
            Some("ComponentEF435E77")
        );
    }

    #[test]
    fn scoped_names_allow_duplicate_leaf_names_in_different_output_scopes() {
        let first = Uuid::parse_str("11111111-1111-1111-1111-111111111111").unwrap();
        let second = Uuid::parse_str("22222222-2222-2222-2222-222222222222").unwrap();

        let names = scoped_rust_type_names_by_id([
            (first, vec!["inventory".to_owned()], "Item"),
            (second, vec!["catalog".to_owned()], "Item"),
        ]);

        assert_eq!(names.get(&first).map(String::as_str), Some("Item"));
        assert_eq!(names.get(&second).map(String::as_str), Some("Item"));
    }

    #[test]
    fn scoped_names_suffix_duplicate_leaf_names_in_the_same_output_scope() {
        let first = Uuid::parse_str("11111111-1111-1111-1111-111111111111").unwrap();
        let second = Uuid::parse_str("22222222-2222-2222-2222-222222222222").unwrap();

        let names = scoped_rust_type_names_by_id([
            (first, vec!["items".to_owned()], "Item"),
            (second, vec!["items".to_owned()], "Item"),
        ]);

        assert_eq!(names.get(&first).map(String::as_str), Some("Item11111111"));
        assert_eq!(names.get(&second).map(String::as_str), Some("Item22222222"));
    }

    #[test]
    fn rust_module_ident_matches_rust_file_module_rules() {
        assert_eq!(rust_module_ident("AZ::Component"), "az_component");
        assert_eq!(rust_module_ident("override"), "override_");
        assert_eq!(rust_module_ident("3pSpawner"), "m_3p_spawner");
    }

    #[test]
    fn normalizes_member_field_names() {
        assert_eq!(rust_field_ident("m_targetEntity"), "target_entity");
        assert_eq!(rust_field_ident("m_GDERef"), "gde_ref");
        assert_eq!(rust_field_ident("type"), "type_");
    }

    #[test]
    fn normalizes_enum_variant_labels_without_cpp_type_parsing() {
        assert_eq!(rust_variant_ident("On Enter"), "OnEnter");
        assert_eq!(rust_variant_ident("Client & Server"), "ClientServer");
        assert_eq!(rust_variant_ident("2-Handed"), "Variant2Handed");
    }

    #[test]
    fn reflected_type_names_preserve_space_separated_words() {
        assert_eq!(rust_type_ident("Item Type"), "ItemType");
    }

    #[test]
    fn reflected_type_name_preserves_component_scaffold_fallbacks() {
        let type_id = Uuid::parse_str("11111111-2222-3333-4444-555555555555").unwrap();

        assert_eq!(
            rust_reflected_type_name("Javelin::ActionListComponent", type_id),
            "ActionListComponent"
        );
        assert_eq!(
            rust_reflected_type_name("legacy:missing", type_id),
            "Unknown11111111Component"
        );
        assert_eq!(
            rust_reflected_type_name("3pSpawnerComponent", type_id),
            "Component3pSpawnerComponent"
        );
    }
}
