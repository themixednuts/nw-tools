mod affinity;
mod color;
mod evidence;
mod foreign_keys;
mod model;
mod number;
mod range;
mod rules;
mod semantic;
mod syntax;

#[cfg(test)]
mod tests;

use std::collections::HashSet;

use nw_datasheet::{
    ColumnType,
    game_system::{
        GameSystemCell, GameSystemColumn, GameSystemDataError, GameSystemDataTables,
        GameSystemTable, GameSystemValidationDiagnostic, GameSystemValidationDiagnosticKind,
        OwnedCellValue,
    },
};

use affinity::apply_type_affinity;
pub use color::is_hex_color_text;
use evidence::{enum_shape_matches_source_token, parse_schema_bool};
use foreign_keys::{
    apply_foreign_key_family_affinity, build_key_lookup, collect_key_indexes, infer_foreign_keys,
};
use number::NumberStats;
use rules::{scalar_enum_column_affinity, scalar_enum_shape_by_name};
use syntax::{StringStats, is_empty_cell_for_column, schema_tokens};

pub use model::*;
pub use range::{
    range_f32_from_cell_value, range_f32_from_text, range_i32_from_cell_value, range_i32_from_text,
    range_inclusive_f32_from_cell_value, range_inclusive_f32_from_text,
    range_inclusive_i32_from_cell_value, range_inclusive_i32_from_text,
    range_inclusive_u32_from_cell_value, range_inclusive_u32_from_text, range_u32_from_cell_value,
    range_u32_from_text,
};

/// Returns the reflected semantic enum assigned to a logical row column.
///
/// Manager emitters use this same catalog as schema inference so generated
/// semantic APIs cannot drift from the row schema they project.
#[must_use]
pub fn semantic_enum_shape_for_column(
    row_type_name: &str,
    column_name: &str,
) -> Option<GameSystemEnumShape> {
    scalar_enum_column_affinity(row_type_name, column_name)
}

/// Returns a reflected semantic enum by its canonical type identity.
///
/// This is used when a projection already carries its validated semantic type
/// and needs the source discriminants without depending on a target language's
/// generated conversion traits.
#[must_use]
pub fn semantic_enum_shape_by_name(name: &str) -> Option<GameSystemEnumShape> {
    scalar_enum_shape_by_name(name)
}

const FOREIGN_KEY_CONFIDENCE_THRESHOLD: f64 = 0.80;
const FOREIGN_KEY_MIN_CHECKED_VALUES: usize = 2;
const FOREIGN_KEY_FAMILY_CONFIDENCE_THRESHOLD: f64 = 0.95;
const FOREIGN_KEY_FAMILY_MIN_CHECKED_VALUES: usize = 20;
const TYPE_AFFINITY_CONFIDENCE_THRESHOLD: f64 = 0.80;
const NATIVE_NUMERIC_TEXT_CONFIDENCE_THRESHOLD: f64 = 0.75;

#[must_use]
pub fn native_float_prefix(value: &str) -> Option<&str> {
    evidence::native_float_prefix(value)
}

#[derive(Debug, Clone)]
struct ColumnAnalysis {
    schema: GameSystemColumnSchema,
    tokens_by_row: Vec<Vec<String>>,
}

#[must_use]
pub fn infer_data_tables_schema(
    data_tables: &GameSystemDataTables,
) -> GameSystemDataTablesSchemaReport {
    let key_indexes = collect_key_indexes(data_tables);
    let key_lookup = build_key_lookup(&key_indexes);
    let mut diagnostics = Vec::new();
    let mut tables = Vec::with_capacity(data_tables.tables().len());

    for (table_index, table) in data_tables.tables().iter().enumerate() {
        let mut columns = Vec::with_capacity(table.columns().len());
        for (column_index, column) in table.columns().iter().enumerate() {
            let mut analysis = analyze_column(table, column_index, column);
            if let GameSystemColumnValueShape::String { foreign_keys, .. } =
                &mut analysis.schema.value_shape
            {
                *foreign_keys = infer_foreign_keys(
                    data_tables,
                    table_index,
                    column_index,
                    column,
                    &analysis.tokens_by_row,
                    &key_indexes,
                    &key_lookup,
                    &mut diagnostics,
                );
            }
            columns.push(analysis.schema);
        }

        tables.push(GameSystemTableSchema {
            table_name: table.name().to_owned(),
            table_name_crc: table.name_crc(),
            row_type_name: table.type_name().to_owned(),
            row_type_crc: table.type_crc(),
            row_count: table.len(),
            sources: table
                .sources()
                .iter()
                .map(|source| source.path().display().to_string())
                .collect(),
            columns,
        });
    }

    let mut type_affinities = apply_type_affinity(data_tables, &mut tables);
    apply_foreign_key_family_affinity(
        data_tables,
        &key_indexes,
        &mut tables,
        &mut type_affinities,
        &mut diagnostics,
    );

    GameSystemDataTablesSchemaReport {
        tables,
        diagnostics,
        type_affinities,
    }
}

fn analyze_column(
    table: &GameSystemTable,
    column_index: usize,
    column: &GameSystemColumn,
) -> ColumnAnalysis {
    let mut non_empty_rows = 0usize;
    let mut empty_rows = 0usize;
    let mut distinct_values = HashSet::new();
    let mut number_shape = NumberStats::default();
    let mut string_shape = StringStats::default();
    let mut tokens_by_row = Vec::with_capacity(table.len());

    for row in table.row_refs() {
        let Some(cell) = row.cells().get(column_index) else {
            empty_rows += 1;
            tokens_by_row.push(Vec::new());
            continue;
        };

        if is_empty_cell_for_column(cell.value(), column_index == 0)
            && !is_semantic_enum_cell_value(table.type_name(), column.name(), cell.value())
        {
            empty_rows += 1;
            tokens_by_row.push(Vec::new());
            continue;
        }

        non_empty_rows += 1;
        distinct_values.insert(cell.value().to_string());
        match cell.value() {
            OwnedCellValue::Number(value) => number_shape.observe(*value),
            OwnedCellValue::String(value) => {
                let tokens = schema_tokens(column.name(), value);
                string_shape.observe(column.name(), value, false);
                tokens_by_row.push(tokens);
                continue;
            }
            OwnedCellValue::Boolean(_) => {}
        }
        tokens_by_row.push(Vec::new());
    }

    let value_shape = match column.column_type() {
        ColumnType::Boolean if declared_boolean_cells_are_boolean_like(table, column_index) => {
            GameSystemColumnValueShape::Boolean
        }
        ColumnType::Boolean => string_value_shape(table.type_name(), column.name(), string_shape),
        ColumnType::Number => GameSystemColumnValueShape::Number {
            number_shape: number_shape.finish_observed(),
        },
        ColumnType::String => string_value_shape(table.type_name(), column.name(), string_shape),
    };

    ColumnAnalysis {
        schema: GameSystemColumnSchema {
            name: column.name().to_owned(),
            crc: column.crc(),
            declared_type: column.column_type(),
            row_key: column_index == 0,
            required: empty_rows == 0,
            non_empty_rows,
            empty_rows,
            distinct_values: distinct_values.len(),
            value_shape,
        },
        tokens_by_row,
    }
}

fn string_value_shape(
    row_type_name: &str,
    column_name: &str,
    string_shape: StringStats,
) -> GameSystemColumnValueShape {
    let identifier_like = string_shape.identifier_like;
    let localized_key_like = string_shape.localized_key_like;
    let asset_path_like = string_shape.asset_path_like;
    let expression_like = string_shape.expression_like;
    let list = string_shape.finish_list(row_type_name, column_name);
    GameSystemColumnValueShape::String {
        identifier_like,
        localized_key_like,
        asset_path_like,
        expression_like,
        list,
        foreign_keys: Vec::new(),
    }
}

fn declared_boolean_cells_are_boolean_like(table: &GameSystemTable, column_index: usize) -> bool {
    for row in table.row_refs() {
        let Some(cell) = row.cells().get(column_index) else {
            continue;
        };
        if is_empty_cell_for_column(cell.value(), column_index == 0) {
            continue;
        }
        let boolean_like = match cell.value() {
            OwnedCellValue::Boolean(_) => true,
            OwnedCellValue::Number(value) => *value == 0.0 || *value == 1.0,
            OwnedCellValue::String(value) => parse_schema_bool(value).is_some(),
        };
        if !boolean_like {
            return false;
        }
    }
    true
}

fn is_semantic_enum_cell_value(
    row_type_name: &str,
    column_name: &str,
    value: &OwnedCellValue,
) -> bool {
    let OwnedCellValue::String(value) = value else {
        return false;
    };
    scalar_enum_column_affinity(row_type_name, column_name)
        .is_some_and(|enum_shape| enum_shape_matches_source_token(&enum_shape, value))
}

pub(super) fn _assert_error_send_sync(_: &GameSystemDataError) {}
