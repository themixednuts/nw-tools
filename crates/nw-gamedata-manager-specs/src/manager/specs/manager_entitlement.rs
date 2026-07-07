use super::*;

pub(super) fn entitlement_data_manager_spec() -> NativeManagerSpec {
    manager_spec_with_inputs(
        "Javelin::EntitlementDataManager",
        "crate::EntitlementDataManager",
        Vec::new(),
        vec![
            "Javelin::EntitlementDataManager::EntitlementDataManager",
            "Javelin::EntitlementDataManager::CacheAllEntitlementDataTables",
            "Javelin::EntitlementDataManager::GetEntitlementDataById",
            "Javelin::EntitlementDataManager::GetEntitlementsForReward",
            "Javelin::EntitlementDataManager::GetEntitlementsForExpansion",
        ],
    )
    .with_shape(NativeManagerShape::entitlement_data(
        NativeEntitlementDataManager::new(ident("entitlement_data")),
    ))
}
