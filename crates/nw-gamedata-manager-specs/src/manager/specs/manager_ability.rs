use super::*;

pub(super) fn ability_data_manager_spec() -> NativeManagerSpec {
    let table_specs = inputs::ABILITY_DATA_MANAGER_TABLES
        .iter()
        .copied()
        .map(|table_name| TableFamilyTableSpec {
            variant: to_upper_camel_ident(table_name, "Table"),
            table_name,
            row_type_name: "AbilityData",
        })
        .collect::<Vec<_>>();
    let input_tables = table_specs
        .iter()
        .map(|table| table_input(table.table_name, table.row_type_name))
        .collect::<Vec<_>>();
    let shape = NativeAbilityDataManager::new(
        ident("ability_data"),
        table_specs
            .into_iter()
            .map(|table| {
                NativeTableFamilyTable::new(
                    ident(table.variant),
                    game_table(table.table_name),
                    game_row_type(table.row_type_name),
                )
            })
            .collect(),
    )
    .expect("validated AbilityData manager shape");

    NativeManagerSpec::new(
        GhidraClassPath::new("Javelin::AbilityDataManager").expect("validated Ghidra class"),
        rust_type("crate::AbilityDataManager"),
        input_tables,
        vec![
            GhidraFunctionPath::new("Javelin::AbilityDataManager::AbilityDataManager")
                .expect("validated Ghidra function"),
            GhidraFunctionPath::new("Javelin::AbilityDataManager::CacheAllAbilityDataTables")
                .expect("validated Ghidra function"),
        ],
    )
    .expect("validated native manager spec")
    .with_shape(NativeManagerShape::ability_data(shape))
}
