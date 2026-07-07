use super::super::NativeDevCellValue;
use super::number::{
    native_dev_number_cell_value_for_shape, native_dev_string_number_cell_value_or_zero,
};
use super::range::{native_dev_string_range_cell_value, validate_u32_range_shape};
use super::text::{enum_discriminant_for_source_token, native_dev_string_bool_cell_value};
use crate::game_system_schema::{
    GameSystemColumnSchema, GameSystemEnumRepresentation, GameSystemEnumShape,
    GameSystemListAtomShape, GameSystemListElementShape, GameSystemListShape,
    GameSystemNumberShape, GameSystemRangeBounds, is_hex_color_text, range_f32_from_text,
    range_i32_from_text, range_inclusive_f32_from_text, range_inclusive_i32_from_text,
    range_inclusive_u32_from_text, range_u32_from_text,
};
use anyhow::{Context, Result, bail};
use std::borrow::Cow;
pub(super) fn native_dev_string_list_cell_value(
    column: &GameSystemColumnSchema,
    list: &GameSystemListShape,
    value: &str,
) -> Result<NativeDevCellValue> {
    let entries = split_native_dev_list(value, list);
    let values = match list.element_shape.as_ref() {
        Some(GameSystemListElementShape::Boolean) => entries
            .iter()
            .map(|entry| {
                native_dev_string_bool_cell_value(column, entry)
                    .map(Option::unwrap_or_default)
                    .map(NativeDevCellValue::boolean)
            })
            .collect::<Result<Vec<_>>>()?,
        Some(GameSystemListElementShape::Color { .. }) => {
            validate_hex_color_entries(column, &entries)?;
            native_dev_string_list_items(entries)
        }
        Some(GameSystemListElementShape::Number { number_shape }) => match *number_shape {
            GameSystemNumberShape::Float => entries
                .iter()
                .map(|entry| {
                    native_dev_string_number_cell_value_or_zero(column, entry)
                        .map(NativeDevCellValue::f32)
                })
                .collect::<Result<Vec<_>>>()?,
            _ => entries
                .iter()
                .map(|entry| {
                    native_dev_string_number_cell_value_or_zero(column, entry).and_then(|value| {
                        native_dev_number_cell_value_for_shape(column, *number_shape, value)
                    })
                })
                .collect::<Result<Vec<_>>>()?,
        },
        Some(GameSystemListElementShape::Crc32) => native_dev_string_list_items(entries),
        Some(GameSystemListElementShape::Range {
            bounds,
            number_shape,
        }) => native_dev_string_range_list_value(column, *bounds, *number_shape, &entries)?,
        Some(GameSystemListElementShape::Enum { enum_shape }) => {
            native_dev_string_enum_list_value(column, enum_shape, &entries)?
        }
        Some(GameSystemListElementShape::Pair {
            separator,
            first,
            second,
            default_second_source_token,
        }) => native_dev_string_pair_list_value(
            column,
            list,
            *separator,
            first,
            second,
            default_second_source_token.as_deref(),
            &entries,
        )?,
        Some(GameSystemListElementShape::String) | None => native_dev_string_list_items(entries),
    };
    Ok(NativeDevCellValue::list(values))
}

pub(super) fn native_dev_number_list_cell_value(
    column: &GameSystemColumnSchema,
    list: &GameSystemListShape,
    value: f32,
) -> Result<Vec<NativeDevCellValue>> {
    match list.element_shape.as_ref() {
        Some(GameSystemListElementShape::Number { number_shape }) => {
            Ok(vec![native_dev_number_cell_value_for_shape(
                column,
                *number_shape,
                value,
            )?])
        }
        Some(GameSystemListElementShape::Pair { .. }) if value == 0.0 => Ok(Vec::new()),
        Some(GameSystemListElementShape::Boolean)
        | Some(GameSystemListElementShape::Color { .. })
        | Some(GameSystemListElementShape::Crc32)
        | Some(GameSystemListElementShape::Range { .. })
        | Some(GameSystemListElementShape::Enum { .. })
        | Some(GameSystemListElementShape::Pair { .. })
        | Some(GameSystemListElementShape::String)
        | None => bail!("column {} has number cell outside list schema", column.name),
    }
}

fn native_dev_string_list_items(entries: Vec<String>) -> Vec<NativeDevCellValue> {
    entries
        .into_iter()
        .map(NativeDevCellValue::string)
        .collect()
}

fn native_dev_string_range_list_value(
    column: &GameSystemColumnSchema,
    bounds: GameSystemRangeBounds,
    number_shape: GameSystemNumberShape,
    entries: &[String],
) -> Result<Vec<NativeDevCellValue>> {
    match (bounds, number_shape) {
        (GameSystemRangeBounds::Exclusive, GameSystemNumberShape::Float) => Ok(entries
            .iter()
            .map(|entry| NativeDevCellValue::range_f32(range_f32_from_text(entry)))
            .collect()),
        (GameSystemRangeBounds::Inclusive, GameSystemNumberShape::Float) => Ok(entries
            .iter()
            .map(|entry| {
                NativeDevCellValue::range_inclusive_f32(range_inclusive_f32_from_text(entry))
            })
            .collect()),
        (GameSystemRangeBounds::Exclusive, GameSystemNumberShape::Integer) => entries
            .iter()
            .map(|entry| {
                range_i32_from_text(entry)
                    .map(NativeDevCellValue::range_i32)
                    .with_context(|| {
                        format!(
                            "column {} expected exclusive i32 range value, found {entry}",
                            column.name
                        )
                    })
            })
            .collect(),
        (GameSystemRangeBounds::Inclusive, GameSystemNumberShape::Integer) => entries
            .iter()
            .map(|entry| {
                range_inclusive_i32_from_text(entry)
                    .map(NativeDevCellValue::range_inclusive_i32)
                    .with_context(|| {
                        format!(
                            "column {} expected inclusive i32 range value, found {entry}",
                            column.name
                        )
                    })
            })
            .collect(),
        (
            GameSystemRangeBounds::Exclusive,
            GameSystemNumberShape::NonNegativeInteger
            | GameSystemNumberShape::PositiveInteger
            | GameSystemNumberShape::U8
            | GameSystemNumberShape::NonZeroU8
            | GameSystemNumberShape::U16
            | GameSystemNumberShape::NonZeroU16,
        ) => entries
            .iter()
            .map(|entry| {
                let range = range_u32_from_text(entry).with_context(|| {
                    format!(
                        "column {} expected exclusive u32 range value, found {entry}",
                        column.name
                    )
                })?;
                validate_u32_range_shape(&column.name, number_shape, range.start, range.end)?;
                Ok(NativeDevCellValue::range_u32(range))
            })
            .collect(),
        (
            GameSystemRangeBounds::Inclusive,
            GameSystemNumberShape::NonNegativeInteger
            | GameSystemNumberShape::PositiveInteger
            | GameSystemNumberShape::U8
            | GameSystemNumberShape::NonZeroU8
            | GameSystemNumberShape::U16
            | GameSystemNumberShape::NonZeroU16,
        ) => entries
            .iter()
            .map(|entry| {
                let range = range_inclusive_u32_from_text(entry).with_context(|| {
                    format!(
                        "column {} expected inclusive u32 range value, found {entry}",
                        column.name
                    )
                })?;
                validate_u32_range_shape(&column.name, number_shape, range.start, range.last)?;
                Ok(NativeDevCellValue::range_inclusive_u32(range))
            })
            .collect(),
    }
}

fn native_dev_string_pair_list_value(
    column: &GameSystemColumnSchema,
    list: &GameSystemListShape,
    separator: char,
    first: &GameSystemListAtomShape,
    second: &GameSystemListAtomShape,
    default_second_source_token: Option<&str>,
    entries: &[String],
) -> Result<Vec<NativeDevCellValue>> {
    let entries = repaired_pair_list_entries(list, separator, entries);
    entries
        .iter()
        .map(|entry| {
            let (first_value, second_value) = if let Some((first_value, second_value)) =
                entry.split_once(separator)
            {
                (first_value.trim(), second_value.trim())
            } else {
                let Some(second_value) = default_second_source_token else {
                    bail!(
                        "column {} expected pair value separated by `{separator}`, found {entry}",
                        column.name
                    );
                };
                (entry.trim(), second_value)
            };
            if second_value.contains(separator) {
                bail!(
                    "column {} expected exactly one pair separator `{separator}`, found {entry}",
                    column.name
                );
            }
            Ok(NativeDevCellValue::pair(
                native_dev_string_atom_value(column, first, first_value)?,
                native_dev_string_atom_value(column, second, second_value)?,
            ))
        })
        .collect::<Result<Vec<_>>>()
}

fn repaired_pair_list_entries<'a>(
    list: &GameSystemListShape,
    pair_separator: char,
    entries: &'a [String],
) -> Cow<'a, [String]> {
    let [entry] = entries else {
        return Cow::Borrowed(entries);
    };
    if list
        .separators
        .iter()
        .any(|separator| separator.as_str() == ",")
        || !entry.contains(',')
    {
        return Cow::Borrowed(entries);
    }

    let repaired = entry
        .split(',')
        .map(str::trim)
        .filter(|part| !part.is_empty())
        .map(str::to_owned)
        .collect::<Vec<_>>();
    if repaired.len() <= 1
        || repaired
            .iter()
            .any(|part| part.matches(pair_separator).count() != 1)
    {
        return Cow::Borrowed(entries);
    }

    Cow::Owned(repaired)
}

fn native_dev_string_enum_list_value(
    column: &GameSystemColumnSchema,
    enum_shape: &GameSystemEnumShape,
    entries: &[String],
) -> Result<Vec<NativeDevCellValue>> {
    for entry in entries {
        enum_discriminant_for_source_token(column, enum_shape, entry)?;
    }
    Ok(native_dev_string_list_items(entries.to_vec()))
}

fn native_dev_string_atom_value(
    column: &GameSystemColumnSchema,
    atom: &GameSystemListAtomShape,
    value: &str,
) -> Result<NativeDevCellValue> {
    Ok(match atom {
        GameSystemListAtomShape::String => NativeDevCellValue::string(value.to_owned()),
        GameSystemListAtomShape::Boolean => NativeDevCellValue::boolean(
            native_dev_string_bool_cell_value(column, value)?.unwrap_or_default(),
        ),
        GameSystemListAtomShape::Color { .. } => {
            if !is_hex_color_text(value) {
                bail!(
                    "column {} expected hex color text atom, found {value}",
                    column.name
                );
            }
            NativeDevCellValue::string(value.to_owned())
        }
        GameSystemListAtomShape::Number { number_shape } => {
            let value = native_dev_string_number_cell_value_or_zero(column, value)?;
            native_dev_number_cell_value_for_shape(column, *number_shape, value)?
        }
        GameSystemListAtomShape::Range {
            bounds,
            number_shape,
        } => native_dev_string_range_cell_value(column, *bounds, *number_shape, value)?,
        GameSystemListAtomShape::Enum { enum_shape } => {
            native_dev_enum_atom_value(column, enum_shape, value)?
        }
        GameSystemListAtomShape::Crc32 => NativeDevCellValue::string(value.to_owned()),
    })
}

fn native_dev_enum_atom_value(
    column: &GameSystemColumnSchema,
    enum_shape: &GameSystemEnumShape,
    value: &str,
) -> Result<NativeDevCellValue> {
    let discriminant = enum_discriminant_for_source_token(column, enum_shape, value)?;
    match enum_shape.representation {
        GameSystemEnumRepresentation::U8 => u8::try_from(discriminant)
            .map(NativeDevCellValue::u8)
            .with_context(|| {
                format!(
                    "column {} enum {} discriminant {} does not fit u8",
                    column.name, enum_shape.name, discriminant
                )
            }),
        GameSystemEnumRepresentation::I32 => i32::try_from(discriminant)
            .map(i64::from)
            .map(NativeDevCellValue::i64)
            .with_context(|| {
                format!(
                    "column {} enum {} discriminant {} does not fit i32",
                    column.name, enum_shape.name, discriminant
                )
            }),
        GameSystemEnumRepresentation::U32 | GameSystemEnumRepresentation::Crc32 => {
            let value = u32::try_from(discriminant).with_context(|| {
                format!(
                    "column {} enum {} discriminant {} does not fit u32",
                    column.name, enum_shape.name, discriminant
                )
            })?;
            Ok(match enum_shape.representation {
                GameSystemEnumRepresentation::U32 => NativeDevCellValue::u64(u64::from(value)),
                GameSystemEnumRepresentation::Crc32 => NativeDevCellValue::crc32(value),
                GameSystemEnumRepresentation::U8 => unreachable!("handled above"),
                GameSystemEnumRepresentation::I32 => unreachable!("handled above"),
            })
        }
    }
}

pub(super) fn native_dev_empty_list_value(_list: &GameSystemListShape) -> Vec<NativeDevCellValue> {
    Vec::new()
}

fn split_native_dev_list(value: &str, list: &GameSystemListShape) -> Vec<String> {
    let separators = list
        .separators
        .iter()
        .filter_map(|separator| separator.chars().next())
        .collect::<Vec<_>>();
    value
        .split(|character| separators.contains(&character))
        .map(str::trim)
        .filter(|entry| list.preserve_empty_entries || !entry.is_empty())
        .map(str::to_owned)
        .collect()
}

fn validate_hex_color_entries(column: &GameSystemColumnSchema, entries: &[String]) -> Result<()> {
    for entry in entries {
        if !is_hex_color_text(entry) {
            bail!(
                "column {} expected hex color text list entry, found {entry}",
                column.name
            );
        }
    }
    Ok(())
}
