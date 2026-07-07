use super::*;

pub(super) fn reward_track_data_manager_spec() -> NativeManagerSpec {
    manager_spec_with_inputs(
        "Javelin::RewardTrackDataManager",
        "crate::RewardTrackDataManager",
        vec![
            table_input("PvPStore", "PvPStoreData"),
            table_input("RewardTrackItems", "RewardTrackItemData"),
        ],
        vec![
            "Javelin::RewardTrackDataManager::RewardTrackDataManager",
            "Javelin::RewardTrackDataManager::CacheAllDataTables",
        ],
    )
    .with_shape(NativeManagerShape::reward_track_data(
        NativeRewardTrackDataManager::new(ident("reward_track_data")),
    ))
}
