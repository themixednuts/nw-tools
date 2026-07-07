use super::*;

pub(super) fn governance_data_manager_spec() -> NativeManagerSpec {
    manager_spec(
        "Javelin::GovernanceDataManager",
        "crate::GovernanceDataManager",
        "TerritoryUpkeep",
        "TerritoryUpkeepDefinition",
        vec![
            "Javelin::GovernanceDataManager::GovernanceDataManager",
            "Javelin::GovernanceDataManager::CacheAllDataTables",
        ],
    )
    .with_shape(NativeManagerShape::governance_data(
        NativeGovernanceDataManager::new(ident("governance_data")),
    ))
}
