use std::collections::BTreeSet;

use crate::game_system_schema::{
    GameSystemColumnSchema, GameSystemColumnValueShape,
    GameSystemDataTablesSchemaReport as GameSystemCatalogSchemaReport, GameSystemNumberShape,
    GameSystemTableSchema, infer_data_tables_schema as infer_catalog_schema,
};
use nw_datasheet::ColumnType;
use nw_datasheet::game_system::{
    GameSystemColumn, GameSystemDataTables as GameSystemCatalog, GameSystemTable, OwnedCellValue,
};

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub enum GameDataCompileMode {
    #[default]
    Strict,
    SourceFormat,
}

#[must_use]
pub fn schema_report_for_mode(
    catalog: &GameSystemCatalog,
    mode: GameDataCompileMode,
) -> GameSystemCatalogSchemaReport {
    match mode {
        GameDataCompileMode::Strict => infer_catalog_schema(catalog),
        GameDataCompileMode::SourceFormat => source_format_schema_report(catalog),
    }
}

fn source_format_schema_report(catalog: &GameSystemCatalog) -> GameSystemCatalogSchemaReport {
    let tables = catalog
        .tables()
        .iter()
        .map(source_format_table_schema)
        .collect();
    GameSystemCatalogSchemaReport {
        tables,
        diagnostics: Vec::new(),
        type_affinities: Vec::new(),
    }
}

fn source_format_table_schema(table: &GameSystemTable) -> GameSystemTableSchema {
    GameSystemTableSchema {
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
        columns: table
            .columns()
            .iter()
            .enumerate()
            .map(|(column_index, column)| source_format_column_schema(table, column_index, column))
            .collect(),
    }
}

fn source_format_column_schema(
    table: &GameSystemTable,
    column_index: usize,
    column: &GameSystemColumn,
) -> GameSystemColumnSchema {
    let mut present_rows = 0usize;
    let mut non_empty_rows = 0usize;
    let mut distinct_values = BTreeSet::new();

    for row in table.row_refs() {
        let Some(cell) = row.cells().get(column_index) else {
            continue;
        };
        present_rows += 1;
        if !source_format_cell_is_empty(cell.value()) {
            non_empty_rows += 1;
        }
        distinct_values.insert(cell.value().to_string());
    }

    GameSystemColumnSchema {
        name: column.name().to_owned(),
        crc: column.crc(),
        declared_type: column.column_type(),
        row_key: column_index == 0,
        required: present_rows == table.len(),
        non_empty_rows,
        empty_rows: table.len().saturating_sub(non_empty_rows),
        distinct_values: distinct_values.len(),
        value_shape: source_format_value_shape(table, column_index, column.column_type()),
    }
}

fn source_format_value_shape(
    table: &GameSystemTable,
    column_index: usize,
    column_type: ColumnType,
) -> GameSystemColumnValueShape {
    match column_type {
        ColumnType::String => GameSystemColumnValueShape::String {
            identifier_like: false,
            localized_key_like: false,
            asset_path_like: false,
            expression_like: false,
            qualified_reference_like: false,
            list: None,
            foreign_keys: Vec::new(),
        },
        ColumnType::Number => GameSystemColumnValueShape::Number {
            number_shape: if column_index == 0 {
                source_format_row_key_number_shape(table, column_index)
            } else {
                GameSystemNumberShape::Float
            },
        },
        ColumnType::Boolean => GameSystemColumnValueShape::Boolean,
    }
}

fn source_format_row_key_number_shape(
    table: &GameSystemTable,
    column_index: usize,
) -> GameSystemNumberShape {
    let has_negative = table.row_refs().any(|row| {
        row.cells()
            .get(column_index)
            .and_then(|cell| match cell.value() {
                OwnedCellValue::Number(value) if value.is_finite() && value.fract() == 0.0 => {
                    Some(*value < 0.0)
                }
                _ => None,
            })
            .unwrap_or_default()
    });
    if has_negative {
        GameSystemNumberShape::Integer
    } else {
        GameSystemNumberShape::NonNegativeInteger
    }
}

fn source_format_cell_is_empty(value: &OwnedCellValue) -> bool {
    match value {
        OwnedCellValue::String(value) => value.is_empty(),
        OwnedCellValue::Number(_) | OwnedCellValue::Boolean(_) => false,
    }
}
