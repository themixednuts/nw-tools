use super::super::NativeDevCellValue;
use crate::game_system_schema::{
    GameSystemColumnSchema, GameSystemNumberShape, GameSystemRangeBounds,
    range_f32_from_cell_value, range_f32_from_text, range_i32_from_cell_value, range_i32_from_text,
    range_inclusive_f32_from_cell_value, range_inclusive_f32_from_text,
    range_inclusive_i32_from_cell_value, range_inclusive_i32_from_text,
    range_inclusive_u32_from_cell_value, range_inclusive_u32_from_text, range_u32_from_cell_value,
    range_u32_from_text,
};
use anyhow::{Context, Result, bail};
use nw_datasheet::game_system::OwnedCellValue;
pub(super) fn native_dev_number_range_cell_value(
    bounds: GameSystemRangeBounds,
    number_shape: GameSystemNumberShape,
    value: f32,
) -> Result<NativeDevCellValue> {
    let value = OwnedCellValue::Number(value);
    native_dev_range_cell_value(bounds, number_shape, &value)
}

pub(super) fn native_dev_string_range_cell_value(
    column: &GameSystemColumnSchema,
    bounds: GameSystemRangeBounds,
    number_shape: GameSystemNumberShape,
    value: &str,
) -> Result<NativeDevCellValue> {
    match (bounds, number_shape) {
        (GameSystemRangeBounds::Exclusive, GameSystemNumberShape::Float) => {
            Ok(NativeDevCellValue::range_f32(range_f32_from_text(value)))
        }
        (GameSystemRangeBounds::Inclusive, GameSystemNumberShape::Float) => Ok(
            NativeDevCellValue::range_inclusive_f32(range_inclusive_f32_from_text(value)),
        ),
        (GameSystemRangeBounds::Exclusive, GameSystemNumberShape::Integer) => {
            range_i32_from_text(value)
                .map(NativeDevCellValue::range_i32)
                .with_context(|| format!("expected exclusive i32 range value, found {value}"))
        }
        (GameSystemRangeBounds::Inclusive, GameSystemNumberShape::Integer) => {
            range_inclusive_i32_from_text(value)
                .map(NativeDevCellValue::range_inclusive_i32)
                .with_context(|| format!("expected inclusive i32 range value, found {value}"))
        }
        (
            GameSystemRangeBounds::Exclusive,
            GameSystemNumberShape::NonNegativeInteger
            | GameSystemNumberShape::PositiveInteger
            | GameSystemNumberShape::U8
            | GameSystemNumberShape::NonZeroU8
            | GameSystemNumberShape::U16
            | GameSystemNumberShape::NonZeroU16,
        ) => {
            let range = range_u32_from_text(value)
                .with_context(|| format!("expected exclusive u32 range value, found {value}"))?;
            validate_u32_range_shape(&column.name, number_shape, range.start, range.end)?;
            Ok(NativeDevCellValue::range_u32(range))
        }
        (
            GameSystemRangeBounds::Inclusive,
            GameSystemNumberShape::NonNegativeInteger
            | GameSystemNumberShape::PositiveInteger
            | GameSystemNumberShape::U8
            | GameSystemNumberShape::NonZeroU8
            | GameSystemNumberShape::U16
            | GameSystemNumberShape::NonZeroU16,
        ) => {
            let range = range_inclusive_u32_from_text(value)
                .with_context(|| format!("expected inclusive u32 range value, found {value}"))?;
            validate_u32_range_shape(&column.name, number_shape, range.start, range.last)?;
            Ok(NativeDevCellValue::range_inclusive_u32(range))
        }
    }
}

fn native_dev_range_cell_value(
    bounds: GameSystemRangeBounds,
    number_shape: GameSystemNumberShape,
    value: &OwnedCellValue,
) -> Result<NativeDevCellValue> {
    match (bounds, number_shape) {
        (GameSystemRangeBounds::Exclusive, GameSystemNumberShape::Float) => {
            range_f32_from_cell_value(value)
                .map(NativeDevCellValue::range_f32)
                .with_context(|| format!("expected exclusive f32 range value, found {value}"))
        }
        (GameSystemRangeBounds::Inclusive, GameSystemNumberShape::Float) => {
            range_inclusive_f32_from_cell_value(value)
                .map(NativeDevCellValue::range_inclusive_f32)
                .with_context(|| format!("expected inclusive f32 range value, found {value}"))
        }
        (GameSystemRangeBounds::Exclusive, GameSystemNumberShape::Integer) => {
            range_i32_from_cell_value(value)
                .map(NativeDevCellValue::range_i32)
                .with_context(|| format!("expected exclusive i32 range value, found {value}"))
        }
        (GameSystemRangeBounds::Inclusive, GameSystemNumberShape::Integer) => {
            range_inclusive_i32_from_cell_value(value)
                .map(NativeDevCellValue::range_inclusive_i32)
                .with_context(|| format!("expected inclusive i32 range value, found {value}"))
        }
        (
            GameSystemRangeBounds::Exclusive,
            GameSystemNumberShape::NonNegativeInteger
            | GameSystemNumberShape::PositiveInteger
            | GameSystemNumberShape::U8
            | GameSystemNumberShape::NonZeroU8
            | GameSystemNumberShape::U16
            | GameSystemNumberShape::NonZeroU16,
        ) => {
            let range = range_u32_from_cell_value(value)
                .with_context(|| format!("expected exclusive u32 range value, found {value}"))?;
            validate_u32_range_shape("range", number_shape, range.start, range.end)?;
            Ok(NativeDevCellValue::range_u32(range))
        }
        (
            GameSystemRangeBounds::Inclusive,
            GameSystemNumberShape::NonNegativeInteger
            | GameSystemNumberShape::PositiveInteger
            | GameSystemNumberShape::U8
            | GameSystemNumberShape::NonZeroU8
            | GameSystemNumberShape::U16
            | GameSystemNumberShape::NonZeroU16,
        ) => {
            let range = range_inclusive_u32_from_cell_value(value)
                .with_context(|| format!("expected inclusive u32 range value, found {value}"))?;
            validate_u32_range_shape("range", number_shape, range.start, range.last)?;
            Ok(NativeDevCellValue::range_inclusive_u32(range))
        }
    }
}

pub(super) fn validate_u32_range_shape(
    column: &str,
    number_shape: GameSystemNumberShape,
    start: u32,
    last: u32,
) -> Result<()> {
    if number_shape == GameSystemNumberShape::PositiveInteger && (start == 0 || last == 0) {
        bail!("column {column} expected positive u32 range bounds, found {start}-{last}");
    }
    if number_shape == GameSystemNumberShape::NonZeroU8 && (start == 0 || last == 0) {
        bail!("column {column} expected non-zero u8 range bounds, found {start}-{last}");
    }
    if number_shape == GameSystemNumberShape::NonZeroU16 && (start == 0 || last == 0) {
        bail!("column {column} expected non-zero u16 range bounds, found {start}-{last}");
    }
    if matches!(
        number_shape,
        GameSystemNumberShape::U8 | GameSystemNumberShape::NonZeroU8
    ) && (start > u32::from(u8::MAX) || last > u32::from(u8::MAX))
    {
        bail!("column {column} expected u8 range bounds, found {start}-{last}");
    }
    if matches!(
        number_shape,
        GameSystemNumberShape::U16 | GameSystemNumberShape::NonZeroU16
    ) && (start > u32::from(u16::MAX) || last > u32::from(u16::MAX))
    {
        bail!("column {column} expected u16 range bounds, found {start}-{last}");
    }
    Ok(())
}
