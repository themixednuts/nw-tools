use super::*;

pub(super) fn seasons_rewards_data_manager_spec() -> NativeManagerSpec {
    manager_spec_with_inputs(
        "Javelin::SeasonsRewardsDataManager",
        "crate::SeasonsRewardsDataManager",
        Vec::new(),
        vec![
            "Javelin::SeasonsRewardsDataManager::SeasonsRewardsDataManager",
            "Javelin::SeasonsRewardsDataManager::CacheAllDataTables",
            "Javelin::SeasonsRewardsDataManager::CacheDataTable",
            "Javelin::SeasonsRewardsDataManager::CacheTableRows",
            "Javelin::SeasonsRewardsDataManager::PopulateDataTableEntryKeys",
            "Javelin::SeasonsRewardsDataManager::DecodeRow",
        ],
    )
    .with_shape(NativeManagerShape::seasons_rewards_data(
        NativeSeasonsRewardsDataManager::new(ident("seasons_rewards_data")),
    ))
}

pub(super) fn seasons_tracked_stat_data_manager_spec() -> NativeManagerSpec {
    manager_spec_with_inputs(
        "Javelin::SeasonsTrackedStatDataManager",
        "crate::SeasonsTrackedStatDataManager",
        Vec::new(),
        vec![
            "Javelin::SeasonsTrackedStatDataManager::SeasonsTrackedStatDataManager",
            "Javelin::SeasonsTrackedStatDataManager::CacheAllDataTables",
            "Javelin::SeasonsTrackedStatDataManager::CacheDataTable",
            "Javelin::SeasonsTrackedStatDataManager::CacheTableRows",
            "Javelin::SeasonsTrackedStatDataManager::DecodeRow",
            "Javelin::SeasonsTrackedStatDataManager::GetGameSystemData",
            "Javelin::SeasonsTrackedStatDataManager::FindGameSystemDataByKey",
        ],
    )
    .with_shape(NativeManagerShape::seasons_tracked_stat_data(
        NativeSeasonsTrackedStatDataManager::new(ident("seasons_tracked_stat_data")),
    ))
}
