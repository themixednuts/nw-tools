use std::num::{NonZeroU8, NonZeroU16};

use anyhow::{Context, Result, bail};

use crate::game_system_schema::{
    GameSystemColumnSchema, GameSystemColumnValueShape, GameSystemNumberShape, native_float_prefix,
};

use super::super::NativeDevCellValue;

pub(super) fn native_dev_number_cell_value(
    column: &GameSystemColumnSchema,
    value: f32,
) -> Result<NativeDevCellValue> {
    let GameSystemColumnValueShape::Number { number_shape } = &column.value_shape else {
        bail!(
            "column {} has number cell outside number schema",
            column.name
        );
    };
    native_dev_number_cell_value_for_shape(column, *number_shape, value)
}

pub(super) fn native_dev_number_cell_value_for_shape(
    column: &GameSystemColumnSchema,
    number_shape: GameSystemNumberShape,
    value: f32,
) -> Result<NativeDevCellValue> {
    Ok(match number_shape {
        GameSystemNumberShape::Float => NativeDevCellValue::f32(value),
        GameSystemNumberShape::Integer => {
            NativeDevCellValue::i64(integer_dev_number(column, value)?)
        }
        GameSystemNumberShape::NonNegativeInteger => {
            NativeDevCellValue::u64(unsigned_dev_number(column, value)?)
        }
        GameSystemNumberShape::PositiveInteger => {
            NativeDevCellValue::u64(positive_dev_number(column, value)?)
        }
        GameSystemNumberShape::U8 => NativeDevCellValue::u8(u8_dev_number(column, value)?),
        GameSystemNumberShape::NonZeroU8 => {
            NativeDevCellValue::nonzero_u8(nonzero_u8_dev_number(column, value)?)
        }
        GameSystemNumberShape::U16 => NativeDevCellValue::u16(u16_dev_number(column, value)?),
        GameSystemNumberShape::NonZeroU16 => {
            NativeDevCellValue::nonzero_u16(nonzero_u16_dev_number(column, value)?)
        }
    })
}

pub(super) fn native_dev_number_bool_cell_value(
    column: &GameSystemColumnSchema,
    value: f32,
) -> Result<bool> {
    if value == 0.0 {
        Ok(false)
    } else if value == 1.0 {
        Ok(true)
    } else {
        bail!(
            "column {} expected bool dev value, found {value}",
            column.name
        )
    }
}

pub(super) fn native_dev_string_number_cell_value(
    column: &GameSystemColumnSchema,
    value: &str,
) -> Result<Option<f32>> {
    let value = value.trim();
    if value.is_empty() {
        return Ok(if column.required { Some(0.0) } else { None });
    }
    // NewWorld+0x6596ce0 zero-initializes the float output before sscanf("%f").
    Ok(Some(
        native_float_prefix(value)
            .and_then(|value| value.parse::<f32>().ok())
            .filter(|value| value.is_finite())
            .unwrap_or(0.0),
    ))
}

pub(super) fn native_dev_string_number_cell_value_or_zero(
    column: &GameSystemColumnSchema,
    value: &str,
) -> Result<f32> {
    Ok(native_dev_string_number_cell_value(column, value)?.unwrap_or(0.0))
}

fn integer_dev_number(column: &GameSystemColumnSchema, value: f32) -> Result<i64> {
    if !value.is_finite() || value.fract() != 0.0 {
        bail!(
            "column {} expected integer dev value, found {value}",
            column.name
        );
    }
    format!("{value:.0}")
        .parse::<i64>()
        .with_context(|| format!("column {} expected integer dev value", column.name))
}

fn unsigned_dev_number(column: &GameSystemColumnSchema, value: f32) -> Result<u64> {
    let value = integer_dev_number(column, value)?;
    u64::try_from(value)
        .with_context(|| format!("column {} expected unsigned dev value", column.name))
}

fn positive_dev_number(column: &GameSystemColumnSchema, value: f32) -> Result<u64> {
    let value = unsigned_dev_number(column, value)?;
    if value == 0 {
        bail!(
            "column {} expected positive dev value, found 0",
            column.name
        );
    }
    Ok(value)
}

fn u8_dev_number(column: &GameSystemColumnSchema, value: f32) -> Result<u8> {
    let value = unsigned_dev_number(column, value)?;
    u8::try_from(value).with_context(|| format!("column {} expected u8 dev value", column.name))
}

fn nonzero_u8_dev_number(column: &GameSystemColumnSchema, value: f32) -> Result<NonZeroU8> {
    let value = u8_dev_number(column, value)?;
    NonZeroU8::new(value)
        .with_context(|| format!("column {} expected non-zero u8 dev value", column.name))
}

fn u16_dev_number(column: &GameSystemColumnSchema, value: f32) -> Result<u16> {
    let value = unsigned_dev_number(column, value)?;
    u16::try_from(value).with_context(|| format!("column {} expected u16 dev value", column.name))
}

fn nonzero_u16_dev_number(column: &GameSystemColumnSchema, value: f32) -> Result<NonZeroU16> {
    let value = u16_dev_number(column, value)?;
    NonZeroU16::new(value)
        .with_context(|| format!("column {} expected non-zero u16 dev value", column.name))
}
