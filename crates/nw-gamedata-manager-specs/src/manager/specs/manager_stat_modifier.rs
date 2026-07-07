use super::*;

pub(super) fn stat_modifier_data_manager_spec() -> NativeManagerSpec {
    manager_spec_with_inputs(
        "Javelin::StatModifierDataManager",
        "crate::StatModifierDataManager",
        Vec::new(),
        vec![
            "Javelin::StatModifierDataManager::StatModifierDataManager",
            "Javelin::StatModifierDataManager::CacheAllDataTables",
            "Javelin::StatModifierDataManager::GetGameSystemDataFromTableType",
        ],
    )
    .with_shape(NativeManagerShape::stat_modifier_data(
        NativeStatModifierDataManager::new(ident("stat_modifier_data")),
    ))
}
