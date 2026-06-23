use std::{
    collections::{BTreeMap, BTreeSet},
    path::PathBuf,
};

use super::api::ComponentScaffoldError;
use super::edit::{
    add_derive_token_to_attr_block, ensure_module_use, find_attr_block_start,
    find_component_struct_start, find_matching_brace, remove_derive_token_from_attr_block,
};
use super::module_render::ensure_map_entities_derives;
use super::source_render::{render_field_line, render_nested_struct};
use super::type_model::{
    NestedStruct, RustField, derive_capabilities_for_fields, rust_fields_map_entities,
};
use super::{read_to_string, write_string};
use crate::CodegenContext;

#[derive(Debug, Clone)]
pub(super) struct ExistingComponentPatch {
    pub(super) component_name: String,
    pub(super) shape: ExistingComponentPatchShape,
    pub(super) fields: Vec<RustField>,
    pub(super) desired_field_order: Vec<String>,
}

#[derive(Debug, Clone, Copy)]
pub(super) enum ExistingComponentPatchShape {
    Braced { insert_offset: usize },
    Unit { semicolon_offset: usize },
}

impl ExistingComponentPatch {
    fn offset(&self) -> usize {
        match self.shape {
            ExistingComponentPatchShape::Braced { insert_offset } => insert_offset,
            ExistingComponentPatchShape::Unit { semicolon_offset } => semicolon_offset,
        }
    }

    fn maps_entities(&self) -> bool {
        rust_fields_map_entities(&self.fields)
            || self.fields.iter().any(|field| {
                field
                    .nested_structs
                    .iter()
                    .any(|nested| nested.maps_entities)
            })
    }
}

pub(super) fn apply_existing_patches(
    patches: BTreeMap<PathBuf, Vec<ExistingComponentPatch>>,
    context: &CodegenContext,
) -> Result<(), ComponentScaffoldError> {
    let mut file_patches = patches.into_iter().collect::<Vec<_>>();
    file_patches.sort_by(|left, right| left.0.cmp(&right.0));
    context
        .runner()
        .try_map(&file_patches, |(file_path, file_patches)| {
            let mut file_patches = file_patches.clone();
            let file_path = file_path.clone();
            file_patches.sort_by_key(|patch| std::cmp::Reverse(patch.offset()));
            let mut text = read_to_string(&file_path)?;
            for patch in file_patches.iter() {
                let mut insertion = String::new();
                for field in &patch.fields {
                    render_field_line(&mut insertion, field);
                }
                match patch.shape {
                    ExistingComponentPatchShape::Braced { insert_offset } => {
                        text.insert_str(insert_offset, &insertion);
                    }
                    ExistingComponentPatchShape::Unit { semicolon_offset } => {
                        let replacement = format!(" {{\n{insertion}}}");
                        text.replace_range(semicolon_offset..semicolon_offset + 1, &replacement);
                    }
                }
            }

            for patch in file_patches.iter() {
                patch_manual_default_impl(&mut text, &patch.component_name, &patch.fields);
                reconcile_existing_struct_value_derives(
                    &mut text,
                    &patch.component_name,
                    patch.shape,
                    &patch.fields,
                );
                reorder_struct_fields(&mut text, &patch.component_name, &patch.desired_field_order);
            }

            let mut nested_structs = BTreeMap::<String, NestedStruct>::new();
            for patch in file_patches.iter() {
                for field in &patch.fields {
                    for nested in &field.nested_structs {
                        nested_structs
                            .entry(nested.name.clone())
                            .or_insert_with(|| nested.clone());
                    }
                }
            }
            for nested in nested_structs.values() {
                let declaration = format!("pub struct {}", nested.name);
                if text.contains(&declaration) {
                    continue;
                }
                ensure_module_use(&mut text, "use az_derive::AzTypeInfo;");
                if !text.ends_with('\n') {
                    text.push('\n');
                }
                text.push('\n');
                text.push_str(&render_nested_struct(nested));
            }
            let map_entity_components = file_patches
                .iter()
                .filter(|patch| patch.maps_entities())
                .map(|patch| patch.component_name.clone())
                .collect::<BTreeSet<_>>();
            if !map_entity_components.is_empty() {
                ensure_module_use(&mut text, "use bevy::ecs::entity::MapEntities;");
                ensure_map_entities_derives(&mut text, &map_entity_components);
            }
            write_string(&file_path, &text)
        })?;
    Ok(())
}

pub(super) fn patch_manual_default_impl(
    text: &mut String,
    component_name: &str,
    fields: &[RustField],
) {
    if fields.is_empty() {
        return;
    }
    let Some((open, close)) = find_manual_default_struct_literal(text, component_name) else {
        return;
    };

    let body = text[open + 1..close].to_owned();
    let mut insertion = String::new();
    for field in fields {
        if initializer_contains_field(&body, &field.name) {
            continue;
        }
        insertion.push_str("            ");
        insertion.push_str(&field.name);
        insertion.push_str(": Default::default(),\n");
    }

    if !insertion.is_empty() {
        let insert_at = text[..close]
            .rfind('\n')
            .map(|offset| offset + 1)
            .unwrap_or(close);
        text.insert_str(insert_at, &insertion);
    }
}

fn find_manual_default_struct_literal(text: &str, component_name: &str) -> Option<(usize, usize)> {
    let impl_header = format!("impl Default for {component_name}");
    let impl_start = text.find(&impl_header)?;
    let impl_open = text[impl_start..].find('{')? + impl_start;
    let impl_close = find_matching_brace(text, impl_open)?;

    let default_start = text[impl_open..impl_close].find("fn default")? + impl_open;
    let fn_open = text[default_start..impl_close].find('{')? + default_start;
    let fn_close = find_matching_brace(text, fn_open)?;

    find_struct_literal_body(text, fn_open + 1, fn_close, "Self")
        .or_else(|| find_struct_literal_body(text, fn_open + 1, fn_close, component_name))
}

fn find_struct_literal_body(
    text: &str,
    start: usize,
    end: usize,
    type_name: &str,
) -> Option<(usize, usize)> {
    let mut cursor = start;
    while cursor < end {
        let relative = text[cursor..end].find(type_name)?;
        let name_start = cursor + relative;
        let name_end = name_start + type_name.len();
        let before = text[..name_start].chars().next_back();
        let after = text[name_end..end].chars().next();
        if before.is_some_and(|ch| ch == '_' || ch.is_ascii_alphanumeric())
            || after.is_some_and(|ch| ch == '_' || ch.is_ascii_alphanumeric())
        {
            cursor = name_end;
            continue;
        }

        let open = text[name_end..end]
            .char_indices()
            .find(|(_, ch)| !ch.is_whitespace())
            .and_then(|(offset, ch)| (ch == '{').then_some(name_end + offset));
        let Some(open) = open else {
            cursor = name_end;
            continue;
        };
        let close = find_matching_brace(text, open)?;
        if close <= end {
            return Some((open, close));
        }
        cursor = name_end;
    }

    None
}

fn initializer_contains_field(body: &str, field_name: &str) -> bool {
    body.lines().map(str::trim_start).any(|line| {
        line.strip_prefix(field_name)
            .is_some_and(|rest| rest.trim_start().starts_with(':'))
    })
}

pub(super) fn reconcile_existing_struct_value_derives(
    text: &mut String,
    component_name: &str,
    shape: ExistingComponentPatchShape,
    fields: &[RustField],
) {
    let capabilities = derive_capabilities_for_fields(fields);
    if !capabilities.copy {
        remove_derive_token_from_struct(text, component_name, "Copy");
    } else if matches!(shape, ExistingComponentPatchShape::Unit { .. }) {
        add_derive_token_to_struct(text, component_name, "Copy");
    }
    if !capabilities.eq {
        remove_derive_token_from_struct(text, component_name, "Eq");
    } else if matches!(shape, ExistingComponentPatchShape::Unit { .. }) {
        add_derive_token_to_struct(text, component_name, "Eq");
    }
}

fn add_derive_token_to_struct(text: &mut String, component_name: &str, token: &str) {
    let Some(struct_start) = find_component_struct_start(text, component_name) else {
        return;
    };
    let attr_block_start = find_attr_block_start(text, struct_start);
    if add_derive_token_to_attr_block(text, attr_block_start, struct_start, token) {
        return;
    }
    text.insert_str(struct_start, &format!("#[derive({token})]\n"));
}

fn remove_derive_token_from_struct(text: &mut String, component_name: &str, token: &str) {
    let Some(struct_start) = find_component_struct_start(text, component_name) else {
        return;
    };
    let attr_block_start = find_attr_block_start(text, struct_start);
    remove_derive_token_from_attr_block(text, attr_block_start, struct_start, token);
}

fn reorder_struct_fields(text: &mut String, component_name: &str, desired_order: &[String]) {
    if desired_order.is_empty() {
        return;
    }
    let Some((open, close)) = find_struct_body_range(text, component_name) else {
        return;
    };
    let body = text[open + 1..close].to_owned();
    let Some(reordered) = reorder_struct_body_fields(&body, desired_order) else {
        return;
    };
    if reordered != body {
        text.replace_range(open + 1..close, &reordered);
    }
}

fn find_struct_body_range(text: &str, component_name: &str) -> Option<(usize, usize)> {
    let struct_start = find_component_struct_start(text, component_name)?;
    let open = text[struct_start..].find('{')? + struct_start;
    let close = find_matching_brace(text, open)?;
    Some((open, close))
}

#[derive(Debug)]
struct FieldBlock {
    name: String,
    text: String,
}

fn reorder_struct_body_fields(body: &str, desired_order: &[String]) -> Option<String> {
    let mut fields = Vec::<FieldBlock>::new();
    let mut pending = String::new();
    let mut tail = String::new();

    for line in body.split_inclusive('\n') {
        if let Some(field_name) = public_field_name(line) {
            let mut block = String::new();
            block.push_str(&pending);
            pending.clear();
            block.push_str(line);
            fields.push(FieldBlock {
                name: field_name,
                text: block,
            });
            continue;
        }

        let trimmed = line.trim_start();
        if trimmed.is_empty()
            || trimmed.starts_with("#[")
            || trimmed.starts_with("///")
            || trimmed.starts_with("//")
        {
            pending.push_str(line);
        } else {
            tail.push_str(&pending);
            pending.clear();
            tail.push_str(line);
        }
    }
    tail.push_str(&pending);

    if fields.is_empty() {
        return None;
    }

    let mut field_indexes = BTreeMap::new();
    for (index, field) in fields.iter().enumerate() {
        if field_indexes.insert(field.name.clone(), index).is_some() {
            return None;
        }
    }

    let mut emitted = BTreeSet::new();
    let mut reordered = String::new();
    for name in desired_order {
        let Some(index) = field_indexes.get(name) else {
            continue;
        };
        if emitted.insert(name.clone()) {
            reordered.push_str(&fields[*index].text);
        }
    }
    for field in &fields {
        if emitted.insert(field.name.clone()) {
            reordered.push_str(&field.text);
        }
    }
    reordered.push_str(&tail);

    Some(reordered)
}

fn public_field_name(line: &str) -> Option<String> {
    let trimmed = line.trim_start();
    let public = trimmed.strip_prefix("pub ")?;
    let colon = public.find(':')?;
    let name = public[..colon].trim();
    if name.is_empty()
        || !name
            .chars()
            .all(|ch| ch == '_' || ch.is_ascii_alphanumeric())
    {
        return None;
    }
    Some(name.to_owned())
}
