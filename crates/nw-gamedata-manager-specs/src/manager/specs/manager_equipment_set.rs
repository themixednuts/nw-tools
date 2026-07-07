use super::*;

pub(super) fn equipment_set_data_manager_spec() -> NativeManagerSpec {
    manager_spec_with_class_evidence(
        "Javelin::EquipmentSetDataManager",
        "crate::EquipmentSetDataManager",
        Vec::new(),
    )
    .with_shape(NativeManagerShape::equipment_set_data(
        NativeEquipmentSetDataManager::new(ident("equipment_set_data")),
    ))
}
