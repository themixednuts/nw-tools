use super::*;

pub(super) fn territory_definitions_data_manager_spec() -> NativeManagerSpec {
    manager_spec_with_inputs(
        "Javelin::TerritoryDefinitionsDataManager",
        "crate::TerritoryDefinitionsDataManager",
        Vec::new(),
        vec![
            "Javelin::TerritoryDefinitionsDataManager::TerritoryDefinitionsDataManager",
            "Javelin::TerritoryDefinitionsDataManager::CacheTerritoryDefinitions",
        ],
    )
    .with_shape(NativeManagerShape::territory_definitions_data(
        NativeTerritoryDefinitionsDataManager::new(ident("territory_definitions_data")),
    ))
}
