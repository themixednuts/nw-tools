use super::super::{
    GameSystemColumnValueShape, GameSystemEnumShape, GameSystemListAtomShape,
    GameSystemListElementShape, GameSystemListShape, GameSystemNumberShape, GameSystemRangeBounds,
    OwnedCellValue,
    color::is_hex_color_text,
    number::{number_matches_shape, string_token_as_number},
    range::{
        parse_range_inclusive_f32_text, range_f32_from_cell_value,
        range_inclusive_f32_from_cell_value,
    },
    syntax::trim_authored_schema_value,
};

pub(in crate::game_system_schema) fn cell_value_matches_shape(
    value: &OwnedCellValue,
    shape: &GameSystemColumnValueShape,
) -> bool {
    match shape {
        GameSystemColumnValueShape::Boolean => cell_value_as_bool(value).is_some(),
        GameSystemColumnValueShape::Crc32 => match value {
            OwnedCellValue::String(value) => !value.trim().is_empty(),
            OwnedCellValue::Number(_) | OwnedCellValue::Boolean(_) => false,
        },
        GameSystemColumnValueShape::Color { .. } => match value {
            OwnedCellValue::String(value) => is_hex_color_text(value),
            OwnedCellValue::Number(_) | OwnedCellValue::Boolean(_) => false,
        },
        GameSystemColumnValueShape::Number { number_shape } => cell_value_as_number(value)
            .is_some_and(|value| number_matches_shape(value, *number_shape)),
        GameSystemColumnValueShape::Enum { enum_shape } => match value {
            OwnedCellValue::String(value) => enum_shape_matches_source_token(enum_shape, value),
            OwnedCellValue::Number(_) | OwnedCellValue::Boolean(_) => false,
        },
        GameSystemColumnValueShape::Range {
            bounds,
            number_shape,
        } => range_cell_matches_shape(value, *bounds, *number_shape),
        GameSystemColumnValueShape::String { list, .. } => match value {
            OwnedCellValue::String(value) => list
                .as_ref()
                .is_none_or(|list| string_list_value_matches_shape(value, list)),
            OwnedCellValue::Number(_) | OwnedCellValue::Boolean(_) => false,
        },
    }
}

pub(in crate::game_system_schema) fn range_cell_matches_shape(
    value: &OwnedCellValue,
    bounds: GameSystemRangeBounds,
    number_shape: GameSystemNumberShape,
) -> bool {
    match bounds {
        GameSystemRangeBounds::Exclusive => range_f32_from_cell_value(value).is_some_and(|range| {
            number_matches_shape(range.start, number_shape)
                && number_matches_shape(range.end, number_shape)
        }),
        GameSystemRangeBounds::Inclusive => {
            range_inclusive_f32_from_cell_value(value).is_some_and(|range| {
                number_matches_shape(range.start, number_shape)
                    && number_matches_shape(range.last, number_shape)
            })
        }
    }
}

pub(in crate::game_system_schema) fn string_list_value_matches_shape(
    value: &str,
    list: &GameSystemListShape,
) -> bool {
    let entries = string_list_entries_for_shape(value, list);
    if entries.is_empty() {
        return true;
    }
    entries
        .iter()
        .all(|entry| string_list_entry_matches_shape(entry, list.element_shape.as_ref()))
}

pub(in crate::game_system_schema) fn string_list_entry_matches_shape(
    value: &str,
    element_shape: Option<&GameSystemListElementShape>,
) -> bool {
    match element_shape {
        None | Some(GameSystemListElementShape::String) => true,
        Some(GameSystemListElementShape::Boolean) => parse_schema_bool(value).is_some(),
        Some(GameSystemListElementShape::Color { .. }) => is_hex_color_text(value),
        Some(GameSystemListElementShape::Number { number_shape }) => string_token_as_number(value)
            .is_some_and(|value| number_matches_shape(value, *number_shape)),
        Some(GameSystemListElementShape::Crc32) => !value.trim().is_empty(),
        Some(GameSystemListElementShape::Range {
            bounds,
            number_shape,
        }) => range_text_matches_shape(value, *bounds, *number_shape),
        Some(GameSystemListElementShape::Enum { enum_shape }) => {
            enum_shape_matches_source_token(enum_shape, value)
        }
        Some(GameSystemListElementShape::Pair {
            separator,
            first,
            second,
            default_second_source_token,
        }) => pair_text_matches_shape(
            value,
            *separator,
            first,
            second,
            default_second_source_token.as_deref(),
        ),
    }
}

pub(in crate::game_system_schema) fn pair_text_matches_shape(
    value: &str,
    separator: char,
    first: &GameSystemListAtomShape,
    second: &GameSystemListAtomShape,
    default_second_source_token: Option<&str>,
) -> bool {
    let value = value.trim();
    let Some((first_value, second_value)) = value.split_once(separator) else {
        let Some(second_value) = default_second_source_token else {
            return false;
        };
        return string_list_atom_matches_shape(value, first)
            && string_list_atom_matches_shape(second_value, second);
    };
    !second_value.contains(separator)
        && string_list_atom_matches_shape(first_value.trim(), first)
        && string_list_atom_matches_shape(second_value.trim(), second)
}

pub(in crate::game_system_schema) fn string_list_atom_matches_shape(
    value: &str,
    atom: &GameSystemListAtomShape,
) -> bool {
    match atom {
        GameSystemListAtomShape::String => true,
        GameSystemListAtomShape::Boolean => parse_schema_bool(value).is_some(),
        GameSystemListAtomShape::Color { .. } => is_hex_color_text(value),
        GameSystemListAtomShape::Number { number_shape } => string_token_as_number(value)
            .is_some_and(|value| number_matches_shape(value, *number_shape)),
        GameSystemListAtomShape::Range {
            bounds,
            number_shape,
        } => range_text_matches_shape(value, *bounds, *number_shape),
        GameSystemListAtomShape::Enum { enum_shape } => {
            enum_shape_matches_source_token(enum_shape, value)
        }
        GameSystemListAtomShape::Crc32 => !value.trim().is_empty(),
    }
}

pub(in crate::game_system_schema) fn enum_shape_matches_source_token(
    enum_shape: &GameSystemEnumShape,
    value: &str,
) -> bool {
    let value = value.trim();
    enum_shape.variants.iter().any(|variant| {
        variant.name.eq_ignore_ascii_case(value)
            || variant
                .source_tokens
                .iter()
                .any(|token| token.eq_ignore_ascii_case(value))
    })
}

pub(in crate::game_system_schema) fn range_text_matches_shape(
    value: &str,
    bounds: GameSystemRangeBounds,
    number_shape: GameSystemNumberShape,
) -> bool {
    match bounds {
        GameSystemRangeBounds::Exclusive => parse_range_inclusive_f32_text(value)
            .parsed()
            .is_some_and(|range| {
                number_matches_shape(range.start, number_shape)
                    && number_matches_shape(range.last, number_shape)
            }),
        GameSystemRangeBounds::Inclusive => parse_range_inclusive_f32_text(value)
            .parsed()
            .is_some_and(|range| {
                number_matches_shape(range.start, number_shape)
                    && number_matches_shape(range.last, number_shape)
            }),
    }
}

pub(in crate::game_system_schema) fn string_list_entries_for_shape<'a>(
    value: &'a str,
    list: &GameSystemListShape,
) -> Vec<&'a str> {
    let trimmed = trim_authored_schema_value(value);
    if trimmed.is_empty() {
        return Vec::new();
    }
    let separators = list
        .separators
        .iter()
        .filter_map(|separator| separator.chars().next())
        .collect::<Vec<_>>();
    if separators.is_empty() {
        return vec![trimmed];
    }
    trimmed
        .split(|character| separators.contains(&character))
        .map(str::trim)
        .filter(|entry| list.preserve_empty_entries || !entry.is_empty())
        .collect()
}

pub(in crate::game_system_schema) fn cell_value_as_bool(value: &OwnedCellValue) -> Option<bool> {
    match value {
        OwnedCellValue::Boolean(value) => Some(*value),
        OwnedCellValue::Number(value) if *value == 0.0 => Some(false),
        OwnedCellValue::Number(value) if *value == 1.0 => Some(true),
        OwnedCellValue::Number(_) => None,
        OwnedCellValue::String(value) => parse_schema_bool(value),
    }
}

pub(in crate::game_system_schema) fn parse_schema_bool(value: &str) -> Option<bool> {
    match value.trim().to_ascii_lowercase().as_str() {
        "true" | "1" | "yes" => Some(true),
        "false" | "0" | "no" => Some(false),
        _ => None,
    }
}

pub(in crate::game_system_schema) fn cell_value_as_number(value: &OwnedCellValue) -> Option<f32> {
    match value {
        OwnedCellValue::Number(value) => Some(*value),
        OwnedCellValue::String(value) => string_token_as_number(value),
        OwnedCellValue::Boolean(_) => None,
    }
}
