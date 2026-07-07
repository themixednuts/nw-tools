use super::*;

pub(super) fn loot_bucket_data_manager_spec() -> NativeManagerSpec {
    manager_spec_with_inputs(
        "Javelin::LootBucketDataManager",
        "crate::LootBucketDataManager",
        Vec::new(),
        vec![
            "Javelin::LootBucketDataManager::LootBucketDataManager",
            "Javelin::LootBucketDataManager::CacheAllDataTables",
            "Javelin::LootBucketDataManager::GetLootBucketDataFromId",
        ],
    )
    .with_shape(NativeManagerShape::loot_bucket_data(
        NativeLootBucketDataManager::new(ident("loot_bucket_data")),
    ))
}
