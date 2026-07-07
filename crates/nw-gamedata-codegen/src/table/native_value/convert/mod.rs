use super::NativeDevCellValue;
use crate::game_system_schema::{
    GameSystemColumnSchema, GameSystemColumnValueShape, GameSystemTableSchema, is_hex_color_text,
};
use anyhow::{Result, bail};
use list::{
    native_dev_empty_list_value, native_dev_number_list_cell_value,
    native_dev_string_list_cell_value,
};
use nw_datasheet::game_system::OwnedCellValue;
use number::{
    native_dev_number_bool_cell_value, native_dev_number_cell_value,
    native_dev_string_number_cell_value,
};
use range::{native_dev_number_range_cell_value, native_dev_string_range_cell_value};
use text::{
    enum_discriminant_for_source_token, is_empty_authored_string, native_dev_string_bool_cell_value,
};
mod list;
mod number;
mod range;
mod text;
pub(in crate::table) fn native_dev_cell_value(
    _schema: &GameSystemTableSchema,
    column: &GameSystemColumnSchema,
    value: &OwnedCellValue,
) -> Result<Option<NativeDevCellValue>> {
    match value {
        OwnedCellValue::String(value) => native_dev_string_cell_value(column, value),
        OwnedCellValue::Number(value) => match &column.value_shape {
            GameSystemColumnValueShape::Boolean => {
                native_dev_number_bool_cell_value(column, *value).map(NativeDevCellValue::boolean)
            }
            GameSystemColumnValueShape::Number { .. } => {
                native_dev_number_cell_value(column, *value)
            }
            GameSystemColumnValueShape::Range {
                bounds,
                number_shape,
            } => native_dev_number_range_cell_value(*bounds, *number_shape, *value),
            GameSystemColumnValueShape::Enum { .. } => {
                bail!("column {} has number cell outside enum schema", column.name)
            }
            GameSystemColumnValueShape::Crc32 => {
                bail!(
                    "column {} has number cell outside crc32 schema",
                    column.name
                )
            }
            GameSystemColumnValueShape::Color { .. } => {
                bail!(
                    "column {} has number cell outside color schema",
                    column.name
                )
            }
            GameSystemColumnValueShape::String {
                list: Some(list), ..
            } => native_dev_number_list_cell_value(column, list, *value)
                .map(NativeDevCellValue::list),
            GameSystemColumnValueShape::String { list: None, .. } => {
                bail!(
                    "column {} has number cell outside string schema",
                    column.name
                )
            }
        }
        .map(Some),
        OwnedCellValue::Boolean(value) => match &column.value_shape {
            GameSystemColumnValueShape::Boolean => Ok(Some(NativeDevCellValue::boolean(*value))),
            GameSystemColumnValueShape::String { list: None, .. } => {
                Ok(Some(NativeDevCellValue::string(value.to_string())))
            }
            GameSystemColumnValueShape::Enum { .. }
            | GameSystemColumnValueShape::Color { .. }
            | GameSystemColumnValueShape::Crc32
            | GameSystemColumnValueShape::Number { .. }
            | GameSystemColumnValueShape::Range { .. }
            | GameSystemColumnValueShape::String { list: Some(_), .. } => {
                bail!(
                    "column {} has boolean cell outside compatible schema",
                    column.name
                )
            }
        },
    }
}
pub(in crate::table) fn native_dev_string_cell_value(
    column: &GameSystemColumnSchema,
    value: &str,
) -> Result<Option<NativeDevCellValue>> {
    if column.row_key
        && matches!(
            column.value_shape,
            GameSystemColumnValueShape::String { .. }
        )
    {
        return Ok(Some(NativeDevCellValue::string(value.to_owned())));
    }

    match &column.value_shape {
        GameSystemColumnValueShape::Enum { enum_shape } => {
            if !column.required && value.trim().is_empty() {
                return Ok(None);
            }
            enum_discriminant_for_source_token(column, enum_shape, value)?;
            Ok(Some(NativeDevCellValue::string(value.trim().to_owned())))
        }
        GameSystemColumnValueShape::Crc32 => {
            if !column.required && is_empty_authored_string(value) {
                return Ok(None);
            }
            Ok(Some(NativeDevCellValue::string(value.trim().to_owned())))
        }
        GameSystemColumnValueShape::Color { .. } => {
            if !column.required && is_empty_authored_string(value) {
                return Ok(None);
            }
            if !is_hex_color_text(value) {
                bail!(
                    "column {} expected hex color text, found {value}",
                    column.name
                );
            }
            Ok(Some(NativeDevCellValue::string(value.trim().to_owned())))
        }
        GameSystemColumnValueShape::Boolean => native_dev_string_bool_cell_value(column, value)
            .map(|value| value.map(NativeDevCellValue::boolean)),
        GameSystemColumnValueShape::Number { .. } => {
            if !column.required && is_empty_authored_string(value) {
                return Ok(None);
            }
            let Some(value) = native_dev_string_number_cell_value(column, value)? else {
                return Ok(None);
            };
            native_dev_number_cell_value(column, value).map(Some)
        }
        GameSystemColumnValueShape::Range {
            bounds,
            number_shape,
        } => {
            if !column.required && is_empty_authored_string(value) {
                return Ok(None);
            }
            native_dev_string_range_cell_value(column, *bounds, *number_shape, value).map(Some)
        }
        GameSystemColumnValueShape::String { list, .. } => {
            if !column.required && is_empty_authored_string(value) {
                return Ok(None);
            }
            let Some(list) = list else {
                return Ok(Some(NativeDevCellValue::string(value.to_owned())));
            };
            if is_empty_authored_string(value) {
                return Ok(Some(NativeDevCellValue::list(native_dev_empty_list_value(
                    list,
                ))));
            }
            native_dev_string_list_cell_value(column, list, value).map(Some)
        }
    }
}
