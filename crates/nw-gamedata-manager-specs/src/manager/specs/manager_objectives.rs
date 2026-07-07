use super::*;

pub(super) fn objectives_data_manager_spec() -> NativeManagerSpec {
    let objective_tables =
        objective_family_table_specs(inputs::OBJECTIVES_DATA_MANAGER_TABLES, "Objectives");
    let objective_task_tables = objective_family_table_specs(
        inputs::OBJECTIVE_TASKS_DATA_MANAGER_TABLES,
        "ObjectiveTasks",
    );
    let input_tables = objective_tables
        .iter()
        .chain(objective_task_tables.iter())
        .map(|table| table_input(table.table_name, table.row_type_name))
        .collect::<Vec<_>>();
    let shape = NativeObjectivesDataManager::new(
        ident("objectives_data"),
        native_family_tables(objective_tables),
        native_family_tables(objective_task_tables),
    )
    .expect("validated ObjectivesData manager shape");

    NativeManagerSpec::new(
        GhidraClassPath::new("Javelin::ObjectivesDataManager").expect("validated Ghidra class"),
        rust_type("crate::ObjectivesDataManager"),
        input_tables,
        vec![
            GhidraFunctionPath::new("Javelin::ObjectivesDataManager::ObjectivesDataManager")
                .expect("validated Ghidra function"),
            GhidraFunctionPath::new("Javelin::ObjectivesDataManager::CacheAllDataTables")
                .expect("validated Ghidra function"),
        ],
    )
    .expect("validated native manager spec")
    .with_shape(NativeManagerShape::objectives_data(shape))
}

fn objective_family_table_specs(
    table_names: &'static [&'static str],
    row_type_name: &'static str,
) -> Vec<TableFamilyTableSpec> {
    table_names
        .iter()
        .copied()
        .map(|table_name| TableFamilyTableSpec {
            variant: to_upper_camel_ident(table_name, "Table"),
            table_name,
            row_type_name,
        })
        .collect()
}

fn native_family_tables(tables: Vec<TableFamilyTableSpec>) -> Vec<NativeTableFamilyTable> {
    tables
        .into_iter()
        .map(|table| {
            NativeTableFamilyTable::new(
                ident(table.variant),
                game_table(table.table_name),
                game_row_type(table.row_type_name),
            )
        })
        .collect()
}
