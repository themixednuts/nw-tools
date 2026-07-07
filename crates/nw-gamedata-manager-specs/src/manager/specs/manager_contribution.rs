use super::*;

pub(super) fn contribution_data_manager_spec() -> NativeManagerSpec {
    let tables = inputs::manager_table_family_specs("crate::ContributionDataManager");
    let input_tables = tables
        .iter()
        .map(|table| table_input(table.table_name, table.row_type_name))
        .collect::<Vec<_>>();
    let shape = NativeContributionDataManager::new(
        ident("contribution_data"),
        tables
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
    .expect("validated ContributionData manager shape");

    NativeManagerSpec::new(
        GhidraClassPath::new("Javelin::ContributionDataManager").expect("validated Ghidra class"),
        rust_type("crate::ContributionDataManager"),
        input_tables,
        vec![
            GhidraFunctionPath::new("Javelin::ContributionDataManager::ContributionDataManager")
                .expect("validated Ghidra function"),
            GhidraFunctionPath::new("Javelin::ContributionDataManager::CacheAllDataTables")
                .expect("validated Ghidra function"),
        ],
    )
    .expect("validated native manager spec")
    .with_shape(NativeManagerShape::contribution_data(shape))
}
