use super::*;

pub(super) fn reusable_scoreboard_data_manager_spec() -> NativeManagerSpec {
    let pug_activity_table_name = "PUGActivityInfo";
    let pug_activity_row_type_name = "PUGActivityInfo";
    let scoreboard_table_name = "ReusableScoreboard";
    let scoreboard_row_type_name = "ReusableScoreboardTabData";
    let shape = NativeReusableScoreboardDataManager::new(
        ident("reusable_scoreboard_data"),
        game_table(pug_activity_table_name),
        game_row_type(pug_activity_row_type_name),
        game_table(scoreboard_table_name),
        game_row_type(scoreboard_row_type_name),
    );

    NativeManagerSpec::new(
        GhidraClassPath::new("Javelin::ReusableScoreboardDataManager")
            .expect("validated Ghidra class"),
        rust_type("crate::ReusableScoreboardDataManager"),
        vec![
            table_input(pug_activity_table_name, pug_activity_row_type_name),
            table_input(scoreboard_table_name, scoreboard_row_type_name),
        ],
        vec![
            GhidraFunctionPath::new(
                "Javelin::ReusableScoreboardDataManager::ReusableScoreboardDataManager",
            )
            .expect("validated Ghidra function"),
            GhidraFunctionPath::new("Javelin::ReusableScoreboardDataManager::CacheAllDataTables")
                .expect("validated Ghidra function"),
        ],
    )
    .expect("validated native manager spec")
    .with_shape(NativeManagerShape::reusable_scoreboard_data(shape))
}
