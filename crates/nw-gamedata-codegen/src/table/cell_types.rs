use crate::game_system_schema::{
    GameSystemColumnSchema, GameSystemColumnValueShape, GameSystemEnumRepresentation,
    GameSystemEnumShape, GameSystemListAtomShape, GameSystemListElementShape,
    GameSystemNumberShape, GameSystemRangeBounds,
};
use anyhow::{Result, bail};

use super::enum_render::table_code_enum_type_name;
use super::model::{RustField, RustForeignKeyColumn};

pub(crate) fn table_key_type_for_column(
    column: &GameSystemColumnSchema,
    field: &RustField,
    lifetime: &str,
) -> Result<String> {
    let cell_type = table_code_cell_type(column, field);
    match cell_type {
        gamedata::CellType::Scalar(gamedata::ScalarType::RowKey)
        | gamedata::CellType::Scalar(gamedata::ScalarType::String) => {
            Ok(format!("&{lifetime} str"))
        }
        gamedata::CellType::Scalar(gamedata::ScalarType::Crc32) => {
            Ok("az_core::crc::Crc32".to_owned())
        }
        gamedata::CellType::Scalar(
            scalar_type @ (gamedata::ScalarType::I8
            | gamedata::ScalarType::I16
            | gamedata::ScalarType::I32
            | gamedata::ScalarType::U8
            | gamedata::ScalarType::U16
            | gamedata::ScalarType::U32
            | gamedata::ScalarType::NonZeroI8
            | gamedata::ScalarType::NonZeroI16
            | gamedata::ScalarType::NonZeroI32
            | gamedata::ScalarType::NonZeroU8
            | gamedata::ScalarType::NonZeroU16
            | gamedata::ScalarType::NonZeroU32),
        ) => Ok(borrowed_scalar_cell_type(scalar_type)
            .expect("supported key scalar has Rust type")
            .to_owned()),
        gamedata::CellType::Scalar(_)
        | gamedata::CellType::Range(_)
        | gamedata::CellType::List(_) => {
            bail!(
                "row-key column {} ({}) cannot be emitted as a typed table key from {:?}",
                column.name,
                field.rust_column_marker,
                cell_type
            )
        }
    }
}

pub(super) fn foreign_key_column_type(root: &str, column: &RustForeignKeyColumn) -> String {
    format!("{root}::{}", column.rust_marker)
}

pub(crate) fn table_code_cell_type(
    column: &GameSystemColumnSchema,
    field: &RustField,
) -> gamedata::CellType {
    if column.row_key {
        return table_code_row_key_cell_type(column);
    }

    match &column.value_shape {
        GameSystemColumnValueShape::Boolean => {
            gamedata::CellType::Scalar(gamedata::ScalarType::Bool)
        }
        GameSystemColumnValueShape::Crc32 => {
            gamedata::CellType::Scalar(gamedata::ScalarType::Crc32)
        }
        GameSystemColumnValueShape::Color { .. } => {
            gamedata::CellType::Scalar(gamedata::ScalarType::LinearRgba)
        }
        GameSystemColumnValueShape::Number { number_shape } => {
            gamedata::CellType::Scalar(table_code_number_scalar_type(*number_shape))
        }
        GameSystemColumnValueShape::Range {
            bounds,
            number_shape,
        } => gamedata::CellType::Range(table_code_range_type(*bounds, *number_shape)),
        GameSystemColumnValueShape::Enum { enum_shape } => {
            gamedata::CellType::Scalar(table_code_enum_scalar_type(enum_shape.representation))
        }
        GameSystemColumnValueShape::String { list, .. } if list.is_some() => {
            gamedata::CellType::List(table_code_list_element_type(column, field))
        }
        GameSystemColumnValueShape::String { .. } if field.foreign_key_column.is_some() => {
            gamedata::CellType::Scalar(gamedata::ScalarType::ForeignKey)
        }
        GameSystemColumnValueShape::String { .. } => {
            gamedata::CellType::Scalar(gamedata::ScalarType::String)
        }
    }
}

fn table_code_row_key_cell_type(column: &GameSystemColumnSchema) -> gamedata::CellType {
    match &column.value_shape {
        GameSystemColumnValueShape::Boolean => {
            gamedata::CellType::Scalar(gamedata::ScalarType::Bool)
        }
        GameSystemColumnValueShape::Crc32 => {
            gamedata::CellType::Scalar(gamedata::ScalarType::Crc32)
        }
        GameSystemColumnValueShape::Color { .. } => {
            gamedata::CellType::Scalar(gamedata::ScalarType::LinearRgba)
        }
        GameSystemColumnValueShape::Number { number_shape } => {
            gamedata::CellType::Scalar(table_code_number_scalar_type(*number_shape))
        }
        GameSystemColumnValueShape::Range {
            bounds,
            number_shape,
        } => gamedata::CellType::Range(table_code_range_type(*bounds, *number_shape)),
        GameSystemColumnValueShape::Enum { enum_shape } => {
            gamedata::CellType::Scalar(table_code_enum_scalar_type(enum_shape.representation))
        }
        GameSystemColumnValueShape::String { .. } => {
            gamedata::CellType::Scalar(gamedata::ScalarType::RowKey)
        }
    }
}

fn table_code_range_type(
    bounds: GameSystemRangeBounds,
    number_shape: GameSystemNumberShape,
) -> gamedata::RangeType {
    gamedata::RangeType {
        bounds: match bounds {
            GameSystemRangeBounds::Exclusive => gamedata::RangeBounds::Exclusive,
            GameSystemRangeBounds::Inclusive => gamedata::RangeBounds::Inclusive,
        },
        endpoint: match number_shape {
            GameSystemNumberShape::Float => gamedata::RangeEndpointType::F32,
            GameSystemNumberShape::Integer => gamedata::RangeEndpointType::I32,
            GameSystemNumberShape::NonNegativeInteger
            | GameSystemNumberShape::PositiveInteger
            | GameSystemNumberShape::U8
            | GameSystemNumberShape::NonZeroU8
            | GameSystemNumberShape::U16
            | GameSystemNumberShape::NonZeroU16 => gamedata::RangeEndpointType::U32,
        },
    }
}

fn table_code_number_scalar_type(number_shape: GameSystemNumberShape) -> gamedata::ScalarType {
    match number_shape {
        GameSystemNumberShape::Float => gamedata::ScalarType::F32,
        GameSystemNumberShape::Integer => gamedata::ScalarType::I32,
        GameSystemNumberShape::NonNegativeInteger => gamedata::ScalarType::U32,
        GameSystemNumberShape::PositiveInteger => gamedata::ScalarType::NonZeroU32,
        GameSystemNumberShape::U8 => gamedata::ScalarType::U8,
        GameSystemNumberShape::NonZeroU8 => gamedata::ScalarType::NonZeroU8,
        GameSystemNumberShape::U16 => gamedata::ScalarType::U16,
        GameSystemNumberShape::NonZeroU16 => gamedata::ScalarType::NonZeroU16,
    }
}

fn table_code_list_element_type(
    column: &GameSystemColumnSchema,
    field: &RustField,
) -> gamedata::ListElementType {
    let GameSystemColumnValueShape::String {
        list: Some(list), ..
    } = &column.value_shape
    else {
        return gamedata::ListElementType::Scalar(gamedata::ScalarType::String);
    };
    match list.element_shape.as_ref() {
        Some(GameSystemListElementShape::Boolean) => {
            gamedata::ListElementType::Scalar(gamedata::ScalarType::Bool)
        }
        Some(GameSystemListElementShape::Color { .. }) => {
            gamedata::ListElementType::Scalar(gamedata::ScalarType::LinearRgba)
        }
        Some(GameSystemListElementShape::Number { number_shape }) => {
            gamedata::ListElementType::Scalar(table_code_number_scalar_type(*number_shape))
        }
        Some(GameSystemListElementShape::Range {
            bounds,
            number_shape,
        }) => gamedata::ListElementType::Range(table_code_range_type(*bounds, *number_shape)),
        Some(GameSystemListElementShape::Enum { enum_shape }) => gamedata::ListElementType::Scalar(
            table_code_enum_scalar_type(enum_shape.representation),
        ),
        Some(GameSystemListElementShape::Crc32) => {
            gamedata::ListElementType::Scalar(gamedata::ScalarType::Crc32)
        }
        Some(GameSystemListElementShape::Pair { first, second, .. }) => {
            gamedata::ListElementType::Pair(gamedata::PairType::new(
                table_code_list_atom_type(first),
                table_code_list_atom_type(second),
            ))
        }
        Some(GameSystemListElementShape::String) | None if field.foreign_key_column.is_some() => {
            gamedata::ListElementType::Scalar(gamedata::ScalarType::ForeignKey)
        }
        Some(GameSystemListElementShape::String) | None => {
            gamedata::ListElementType::Scalar(gamedata::ScalarType::String)
        }
    }
}

fn table_code_list_atom_type(atom: &GameSystemListAtomShape) -> gamedata::AtomType {
    match atom {
        GameSystemListAtomShape::Boolean => gamedata::AtomType::Scalar(gamedata::ScalarType::Bool),
        GameSystemListAtomShape::Color { .. } => {
            gamedata::AtomType::Scalar(gamedata::ScalarType::LinearRgba)
        }
        GameSystemListAtomShape::Number { number_shape } => {
            gamedata::AtomType::Scalar(table_code_number_scalar_type(*number_shape))
        }
        GameSystemListAtomShape::Range {
            bounds,
            number_shape,
        } => gamedata::AtomType::Range(table_code_range_type(*bounds, *number_shape)),
        GameSystemListAtomShape::Enum { enum_shape } => {
            gamedata::AtomType::Scalar(table_code_enum_scalar_type(enum_shape.representation))
        }
        GameSystemListAtomShape::Crc32 => gamedata::AtomType::Scalar(gamedata::ScalarType::Crc32),
        GameSystemListAtomShape::String => gamedata::AtomType::Scalar(gamedata::ScalarType::String),
    }
}

fn table_code_enum_scalar_type(
    representation: GameSystemEnumRepresentation,
) -> gamedata::ScalarType {
    match representation {
        GameSystemEnumRepresentation::U8 => gamedata::ScalarType::U8,
        GameSystemEnumRepresentation::I32 => gamedata::ScalarType::I32,
        GameSystemEnumRepresentation::U32 => gamedata::ScalarType::U32,
        GameSystemEnumRepresentation::Crc32 => gamedata::ScalarType::Crc32,
    }
}

#[cfg(test)]
pub(super) fn borrowed_cell_type_for_column(
    column: &GameSystemColumnSchema,
    field: &RustField,
    lifetime: &str,
) -> String {
    borrowed_cell_type_for_column_in_context(
        column,
        field,
        lifetime,
        &field.rust_column_marker,
        "super::super",
        None,
    )
}

pub(crate) fn borrowed_cell_type_for_column_in_context(
    column: &GameSystemColumnSchema,
    field: &RustField,
    lifetime: &str,
    column_marker_path: &str,
    foreign_key_root: &str,
    enum_root: Option<&str>,
) -> String {
    if !column.row_key
        && let GameSystemColumnValueShape::Enum { enum_shape } = &column.value_shape
    {
        return table_code_enum_type_path(enum_shape, column_marker_path, enum_root);
    }

    match table_code_cell_type(column, field) {
        gamedata::CellType::Scalar(gamedata::ScalarType::String) => format!("&{lifetime} str"),
        gamedata::CellType::Scalar(gamedata::ScalarType::RowKey) => {
            format!("gamedata::RowKey<{lifetime}, {column_marker_path}>")
        }
        gamedata::CellType::Scalar(gamedata::ScalarType::ForeignKey) => {
            let column = field
                .foreign_key_column
                .as_ref()
                .expect("foreign-key column has resolved target column");
            format!(
                "gamedata::ForeignKey<{lifetime}, {}>",
                foreign_key_column_type(foreign_key_root, column)
            )
        }
        gamedata::CellType::Scalar(scalar_type) => borrowed_scalar_cell_type(scalar_type)
            .expect("scalar column has Rust scalar type")
            .to_owned(),
        gamedata::CellType::Range(range_type) => borrowed_range_cell_type(range_type).to_owned(),
        gamedata::CellType::List(element_type) => {
            let element_cell = borrowed_list_element_cell_type(
                column,
                field,
                element_type,
                lifetime,
                column_marker_path,
                foreign_key_root,
                enum_root,
            );
            format!("gamedata::List<{lifetime}, {column_marker_path}, {element_cell}>")
        }
    }
}

fn borrowed_list_element_cell_type(
    column: &GameSystemColumnSchema,
    field: &RustField,
    element_type: gamedata::ListElementType,
    lifetime: &str,
    column_marker_path: &str,
    foreign_key_root: &str,
    enum_root: Option<&str>,
) -> String {
    if let GameSystemColumnValueShape::String {
        list: Some(list), ..
    } = &column.value_shape
    {
        match list.element_shape.as_ref() {
            Some(GameSystemListElementShape::Enum { enum_shape }) => {
                return table_code_enum_type_path(enum_shape, column_marker_path, enum_root);
            }
            Some(GameSystemListElementShape::Pair { first, second, .. }) => {
                return format!(
                    "gamedata::Pair<{}, {}>",
                    borrowed_list_atom_cell_type(first, lifetime, column_marker_path, enum_root),
                    borrowed_list_atom_cell_type(second, lifetime, column_marker_path, enum_root)
                );
            }
            Some(
                GameSystemListElementShape::Boolean
                | GameSystemListElementShape::Color { .. }
                | GameSystemListElementShape::Number { .. }
                | GameSystemListElementShape::Range { .. }
                | GameSystemListElementShape::Crc32
                | GameSystemListElementShape::String,
            )
            | None => {}
        }
    }

    match element_type {
        gamedata::ListElementType::Scalar(gamedata::ScalarType::ForeignKey) => {
            let column = field
                .foreign_key_column
                .as_ref()
                .expect("foreign-key list column has resolved target column");
            format!(
                "gamedata::ForeignKey<{lifetime}, {}>",
                foreign_key_column_type(foreign_key_root, column)
            )
        }
        gamedata::ListElementType::Scalar(gamedata::ScalarType::String) => {
            format!("&{lifetime} str")
        }
        gamedata::ListElementType::Scalar(scalar_type) => borrowed_scalar_cell_type(scalar_type)
            .expect("list element scalar type")
            .to_owned(),
        gamedata::ListElementType::Range(range_type) => {
            borrowed_range_cell_type(range_type).to_owned()
        }
        gamedata::ListElementType::Pair(pair_type) => format!(
            "gamedata::Pair<{}, {}>",
            borrowed_atom_type_cell_type(pair_type.first, lifetime),
            borrowed_atom_type_cell_type(pair_type.second, lifetime)
        ),
    }
}

fn borrowed_list_atom_cell_type(
    atom: &GameSystemListAtomShape,
    lifetime: &str,
    column_marker_path: &str,
    enum_root: Option<&str>,
) -> String {
    match atom {
        GameSystemListAtomShape::String => format!("&{lifetime} str"),
        GameSystemListAtomShape::Boolean => "bool".to_owned(),
        GameSystemListAtomShape::Color { .. } => "bevy_color::LinearRgba".to_owned(),
        GameSystemListAtomShape::Number { number_shape } => {
            borrowed_scalar_cell_type(table_code_number_scalar_type(*number_shape))
                .expect("number atom has scalar type")
                .to_owned()
        }
        GameSystemListAtomShape::Range {
            bounds,
            number_shape,
        } => borrowed_range_cell_type(table_code_range_type(*bounds, *number_shape)).to_owned(),
        GameSystemListAtomShape::Enum { enum_shape } => {
            table_code_enum_type_path(enum_shape, column_marker_path, enum_root)
        }
        GameSystemListAtomShape::Crc32 => "az_core::crc::Crc32".to_owned(),
    }
}

fn borrowed_atom_type_cell_type(atom_type: gamedata::AtomType, lifetime: &str) -> String {
    match atom_type {
        gamedata::AtomType::Scalar(gamedata::ScalarType::String) => format!("&{lifetime} str"),
        gamedata::AtomType::Scalar(scalar_type) => borrowed_scalar_cell_type(scalar_type)
            .expect("atom scalar type")
            .to_owned(),
        gamedata::AtomType::Range(range_type) => borrowed_range_cell_type(range_type).to_owned(),
    }
}

fn table_code_enum_type_path(
    enum_shape: &GameSystemEnumShape,
    column_marker_path: &str,
    enum_root: Option<&str>,
) -> String {
    let enum_name = table_code_enum_type_name(enum_shape);
    if let Some(enum_root) = enum_root {
        return format!("{enum_root}::{enum_name}");
    }
    column_marker_path
        .rsplit_once("::")
        .map_or(enum_name.clone(), |(module_path, _)| {
            format!("{module_path}::{enum_name}")
        })
}

fn borrowed_scalar_cell_type(scalar_type: gamedata::ScalarType) -> Option<&'static str> {
    Some(match scalar_type {
        gamedata::ScalarType::Bool => "bool",
        gamedata::ScalarType::I8 => "i8",
        gamedata::ScalarType::I16 => "i16",
        gamedata::ScalarType::I32 => "i32",
        gamedata::ScalarType::I64 => "i64",
        gamedata::ScalarType::U8 => "u8",
        gamedata::ScalarType::U16 => "u16",
        gamedata::ScalarType::U32 => "u32",
        gamedata::ScalarType::U64 => "u64",
        gamedata::ScalarType::NonZeroI8 => "std::num::NonZeroI8",
        gamedata::ScalarType::NonZeroI16 => "std::num::NonZeroI16",
        gamedata::ScalarType::NonZeroI32 => "std::num::NonZeroI32",
        gamedata::ScalarType::NonZeroI64 => "std::num::NonZeroI64",
        gamedata::ScalarType::NonZeroU8 => "std::num::NonZeroU8",
        gamedata::ScalarType::NonZeroU16 => "std::num::NonZeroU16",
        gamedata::ScalarType::NonZeroU32 => "std::num::NonZeroU32",
        gamedata::ScalarType::NonZeroU64 => "std::num::NonZeroU64",
        gamedata::ScalarType::F32 => "f32",
        gamedata::ScalarType::F64 => "f64",
        gamedata::ScalarType::LinearRgba => "bevy_color::LinearRgba",
        gamedata::ScalarType::Crc32 => "az_core::crc::Crc32",
        gamedata::ScalarType::RowIndex => "gamedata::RowIndex",
        gamedata::ScalarType::String
        | gamedata::ScalarType::RowKey
        | gamedata::ScalarType::ForeignKey => return None,
    })
}

fn borrowed_range_cell_type(range_type: gamedata::RangeType) -> &'static str {
    match (range_type.bounds, range_type.endpoint) {
        (gamedata::RangeBounds::Exclusive, gamedata::RangeEndpointType::F32) => {
            "::core::range::Range<f32>"
        }
        (gamedata::RangeBounds::Inclusive, gamedata::RangeEndpointType::F32) => {
            "::core::range::RangeInclusive<f32>"
        }
        (gamedata::RangeBounds::Exclusive, gamedata::RangeEndpointType::I32) => {
            "::core::range::Range<i32>"
        }
        (gamedata::RangeBounds::Inclusive, gamedata::RangeEndpointType::I32) => {
            "::core::range::RangeInclusive<i32>"
        }
        (gamedata::RangeBounds::Exclusive, gamedata::RangeEndpointType::U32) => {
            "::core::range::Range<u32>"
        }
        (gamedata::RangeBounds::Inclusive, gamedata::RangeEndpointType::U32) => {
            "::core::range::RangeInclusive<u32>"
        }
    }
}
