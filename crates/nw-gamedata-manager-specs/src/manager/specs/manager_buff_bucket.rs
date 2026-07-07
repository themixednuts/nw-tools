use super::*;

pub(super) fn buff_bucket_data_manager_spec() -> NativeManagerSpec {
    let table_name = "BuffBuckets";
    let row_type_name = "BuffBucketData";
    let shape = NativeBuffBucketDataManager::new(
        ident("buff_bucket_data"),
        game_table(table_name),
        game_row_type(row_type_name),
    );

    NativeManagerSpec::new(
        GhidraClassPath::new("Javelin::BuffBucketDataManager").expect("validated Ghidra class"),
        rust_type("crate::BuffBucketDataManager"),
        vec![table_input(table_name, row_type_name)],
        vec![
            GhidraFunctionPath::new("Javelin::BuffBucketDataManager::CacheBuffBucketDataTables")
                .expect("validated Ghidra function"),
            GhidraFunctionPath::new("Javelin::BuffBucketDataManager::IterateOverAllBuffs")
                .expect("validated Ghidra function"),
        ],
    )
    .expect("validated native manager spec")
    .with_shape(NativeManagerShape::buff_bucket_data(shape))
}
