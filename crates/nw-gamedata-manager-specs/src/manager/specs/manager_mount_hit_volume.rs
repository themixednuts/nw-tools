use super::*;

pub(super) fn mount_hit_volume_data_manager_spec() -> NativeManagerSpec {
    let table_name = "MountTypes";
    let row_type_name = "MountTypeData";
    let shape = NativeMountHitVolumeDataManager::new(
        ident("mount_hit_volume_data"),
        game_table(table_name),
        game_row_type(row_type_name),
        asset_path("slices/MountHitVolumes/MountHitVolumes_Master.dynamicslice"),
    );

    NativeManagerSpec::new(
        GhidraClassPath::new("Javelin::MountHitVolumeDataManager").expect("validated Ghidra class"),
        rust_type("crate::MountHitVolumeDataManager"),
        vec![table_input(table_name, row_type_name)],
        vec![
            GhidraFunctionPath::new(
                "Javelin::MountHitVolumeDataManager::MountHitVolumeDataManager",
            )
            .expect("validated Ghidra function"),
        ],
    )
    .expect("validated native manager spec")
    .with_shape(NativeManagerShape::mount_hit_volume_data(shape))
}
