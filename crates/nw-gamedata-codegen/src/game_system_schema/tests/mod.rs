use std::collections::HashMap;

use super::rules::enums::{darkness_threshold_enum_shape, stat_multiplier_type_enum_shape};
use super::*;
use super::{affinity::*, foreign_keys::*};

mod enum_affinity;
mod family_affinity;
mod foreign_keys;
mod list_affinity;
mod number_affinity;
mod semantic_repairs;
mod stat_modifier;

fn table_schema(
    table_name: &str,
    row_type_name: &str,
    column_name: &str,
    number_shape: GameSystemNumberShape,
) -> GameSystemTableSchema {
    GameSystemTableSchema {
        table_name: table_name.to_owned(),
        table_name_crc: 0,
        row_type_name: row_type_name.to_owned(),
        row_type_crc: 0,
        row_count: 1,
        sources: Vec::new(),
        columns: vec![GameSystemColumnSchema {
            name: column_name.to_owned(),
            crc: 0,
            declared_type: ColumnType::Number,
            row_key: false,
            required: true,
            non_empty_rows: 1,
            empty_rows: 0,
            distinct_values: 1,
            value_shape: GameSystemColumnValueShape::Number { number_shape },
        }],
    }
}

fn apply_test_type_affinity(
    tables: &mut [GameSystemTableSchema],
) -> Vec<GameSystemColumnTypeAffinity> {
    let data_tables = data_tables_for_schemas(tables);
    apply_type_affinity(&data_tables, tables)
}

fn data_tables_for_schemas(tables: &[GameSystemTableSchema]) -> GameSystemDataTables {
    let mut data_tables = GameSystemDataTables::default();
    for (table_index, table) in tables.iter().enumerate() {
        let columns = table
            .columns
            .iter()
            .enumerate()
            .map(|(column_index, column)| {
                GameSystemColumn::new(
                    u32::try_from(column_index + 1).expect("column index fits in u32"),
                    column.name.clone(),
                    column.declared_type,
                )
            })
            .collect::<Vec<_>>();
        let cells = table
            .columns
            .iter()
            .enumerate()
            .map(|(column_index, column)| {
                GameSystemCell::new(
                    u32::try_from(column_index + 1).expect("column index fits in u32"),
                    cell_value_for_shape(&column.value_shape),
                )
            })
            .collect::<Vec<_>>();
        let table_id = u32::try_from(table_index + 1).expect("table index fits in u32");
        data_tables
            .insert(GameSystemTable::from_native_columns(
                table.table_name.clone(),
                table_id,
                table.row_type_name.clone(),
                table_id,
                columns,
                vec![(0, cells)],
            ))
            .expect("test table insert");
    }
    data_tables
}

fn cell_value_for_shape(shape: &GameSystemColumnValueShape) -> OwnedCellValue {
    match shape {
        GameSystemColumnValueShape::Boolean => OwnedCellValue::Boolean(false),
        GameSystemColumnValueShape::Color { .. } => OwnedCellValue::String("#000000".to_owned()),
        GameSystemColumnValueShape::Number { number_shape } => {
            OwnedCellValue::Number(match number_shape {
                GameSystemNumberShape::Float => 0.5,
                GameSystemNumberShape::Integer => -1.0,
                GameSystemNumberShape::NonNegativeInteger => 0.0,
                GameSystemNumberShape::PositiveInteger => 1.0,
                GameSystemNumberShape::U8 => 0.0,
                GameSystemNumberShape::NonZeroU8 => 1.0,
                GameSystemNumberShape::U16 => 0.0,
                GameSystemNumberShape::NonZeroU16 => 1.0,
            })
        }
        GameSystemColumnValueShape::Crc32 => OwnedCellValue::String("value".to_owned()),
        GameSystemColumnValueShape::Enum { enum_shape } => OwnedCellValue::String(
            enum_shape
                .variants
                .first()
                .map_or_else(|| "None".to_owned(), |variant| variant.name.clone()),
        ),
        GameSystemColumnValueShape::Range { .. } => OwnedCellValue::String("0-1".to_owned()),
        GameSystemColumnValueShape::String { .. } => OwnedCellValue::String("value".to_owned()),
    }
}

fn test_data_tables(
    table_name: &str,
    row_type_name: &str,
    columns: Vec<(&str, ColumnType)>,
    rows: Vec<Vec<OwnedCellValue>>,
) -> GameSystemDataTables {
    let table = test_table(table_name, 1, row_type_name, columns, rows);
    let mut data_tables = GameSystemDataTables::default();
    data_tables.insert(table).expect("test table insert");
    data_tables
}

fn foreign_key_family_data_tables(singleton_value: &str) -> GameSystemDataTables {
    let mut data_tables = GameSystemDataTables::default();
    data_tables
        .insert(test_table(
            "AchievementDataTable",
            1,
            "AchievementData",
            vec![("AchievementID", ColumnType::String)],
            [
                "Achievement_A",
                "Achievement_B",
                "Achievement_C",
                "Achievement_D",
            ]
            .into_iter()
            .map(|achievement_id| vec![OwnedCellValue::String(achievement_id.to_owned())])
            .collect(),
        ))
        .expect("insert achievement table");
    data_tables
        .insert(test_table(
            "Quest_Seed_A",
            2,
            "Objectives",
            vec![
                ("ObjectiveID", ColumnType::String),
                ("RequiredAchievementId", ColumnType::String),
            ],
            vec![vec![
                OwnedCellValue::String("objective_seed_a".to_owned()),
                OwnedCellValue::String("Achievement_A+Achievement_B".to_owned()),
            ]],
        ))
        .expect("insert first quest seed table");
    data_tables
        .insert(test_table(
            "Quest_Seed_B",
            3,
            "Objectives",
            vec![
                ("ObjectiveID", ColumnType::String),
                ("RequiredAchievementId", ColumnType::String),
            ],
            vec![vec![
                OwnedCellValue::String("objective_seed_b".to_owned()),
                OwnedCellValue::String("Achievement_C+Achievement_D".to_owned()),
            ]],
        ))
        .expect("insert second quest seed table");
    data_tables
        .insert(test_table(
            "Quest_Singleton",
            4,
            "Objectives",
            vec![
                ("ObjectiveID", ColumnType::String),
                ("RequiredAchievementId", ColumnType::String),
            ],
            vec![vec![
                OwnedCellValue::String("objective_singleton".to_owned()),
                OwnedCellValue::String(singleton_value.to_owned()),
            ]],
        ))
        .expect("insert singleton quest table");
    data_tables
}

fn test_table(
    table_name: &str,
    table_id: u32,
    row_type_name: &str,
    columns: Vec<(&str, ColumnType)>,
    rows: Vec<Vec<OwnedCellValue>>,
) -> GameSystemTable {
    let columns = columns
        .into_iter()
        .enumerate()
        .map(|(column_index, (name, column_type))| {
            GameSystemColumn::new(
                u32::try_from(column_index + 1).expect("column index fits in u32"),
                name,
                column_type,
            )
        })
        .collect::<Vec<_>>();
    let rows = rows
        .into_iter()
        .map(|cells| {
            let cells = cells
                .into_iter()
                .enumerate()
                .map(|(column_index, value)| {
                    GameSystemCell::new(
                        u32::try_from(column_index + 1).expect("column index fits in u32"),
                        value,
                    )
                })
                .collect::<Vec<_>>();
            (0, cells)
        })
        .collect::<Vec<_>>();
    GameSystemTable::from_native_columns(
        table_name.to_owned(),
        table_id,
        row_type_name.to_owned(),
        100,
        columns,
        rows,
    )
}

fn report_column<'a>(
    report: &'a GameSystemDataTablesSchemaReport,
    column_name: &str,
) -> &'a GameSystemColumnSchema {
    report.tables[0]
        .columns
        .iter()
        .find(|column| column.name == column_name)
        .expect("report column")
}

fn report_table_column<'a>(
    report: &'a GameSystemDataTablesSchemaReport,
    table_name: &str,
    column_name: &str,
) -> &'a GameSystemColumnSchema {
    report
        .tables
        .iter()
        .find(|table| table.table_name == table_name)
        .and_then(|table| {
            table
                .columns
                .iter()
                .find(|column| column.name == column_name)
        })
        .expect("report table column")
}

fn enum_shape_for_column<'a>(
    report: &'a GameSystemDataTablesSchemaReport,
    column_name: &str,
) -> &'a GameSystemEnumShape {
    let column = report_column(report, column_name);
    let GameSystemColumnValueShape::Enum { enum_shape } = &column.value_shape else {
        panic!("expected enum column for {column_name}")
    };
    enum_shape
}

fn report_affinity<'a>(
    report: &'a GameSystemDataTablesSchemaReport,
    column_name: &str,
) -> &'a GameSystemColumnTypeAffinity {
    report
        .type_affinities
        .iter()
        .find(|affinity| affinity.column_name == column_name)
        .expect("report affinity")
}

fn report_table_affinity<'a>(
    report: &'a GameSystemDataTablesSchemaReport,
    table_name: &str,
    column_name: &str,
) -> &'a GameSystemColumnTypeAffinity {
    report
        .type_affinities
        .iter()
        .find(|affinity| affinity.table_name == table_name && affinity.column_name == column_name)
        .expect("report table affinity")
}

fn number_shape(column: &GameSystemColumnSchema) -> GameSystemNumberShape {
    let GameSystemColumnValueShape::Number { number_shape } = column.value_shape else {
        panic!("expected number column")
    };
    number_shape
}

fn range_bounds(column: &GameSystemColumnSchema) -> GameSystemRangeBounds {
    let GameSystemColumnValueShape::Range { bounds, .. } = column.value_shape else {
        panic!("expected range column")
    };
    bounds
}

fn list_element_shape(column: &GameSystemColumnSchema) -> Option<&GameSystemListElementShape> {
    let GameSystemColumnValueShape::String { list, .. } = &column.value_shape else {
        panic!("expected string list column")
    };
    list.as_ref().and_then(|list| list.element_shape.as_ref())
}

fn scalar_enum_name(column: &GameSystemColumnSchema) -> Option<&str> {
    let GameSystemColumnValueShape::Enum { enum_shape } = &column.value_shape else {
        return None;
    };
    Some(enum_shape.name.as_str())
}

fn list_enum_name(element: &GameSystemListElementShape) -> Option<&str> {
    let GameSystemListElementShape::Enum { enum_shape } = element else {
        return None;
    };
    Some(enum_shape.name.as_str())
}

fn string_list(column: &GameSystemColumnSchema) -> Option<&GameSystemListShape> {
    let GameSystemColumnValueShape::String { list, .. } = &column.value_shape else {
        panic!("expected string list column")
    };
    list.as_ref()
}
