use super::*;

pub(super) fn structure_data_manager_spec() -> NativeManagerSpec {
    let footprint_table_name = "WallFootprint";
    let footprint_row_type_name = "StructureFootprintData";
    let piece_table_name = "T0_Wall_Pieces";
    let piece_row_type_name = "StructurePieceData";
    let shape = NativeStructureDataManager::new(
        ident("structure_data"),
        game_table(footprint_table_name),
        game_row_type(footprint_row_type_name),
        game_table(piece_table_name),
        game_row_type(piece_row_type_name),
    );

    NativeManagerSpec::new(
        GhidraClassPath::new("Javelin::StructureDataManager").expect("validated Ghidra class"),
        rust_type("crate::StructureDataManager"),
        vec![
            table_input(footprint_table_name, footprint_row_type_name),
            table_input(piece_table_name, piece_row_type_name),
        ],
        vec![
            GhidraFunctionPath::new("Javelin::StructureDataManager::StructureDataManager")
                .expect("validated Ghidra function"),
            GhidraFunctionPath::new("Javelin::StructureDataManager::CacheAllDataTables")
                .expect("validated Ghidra function"),
            GhidraFunctionPath::new("Javelin::StructureDataManager::Get")
                .expect("validated Ghidra function"),
        ],
    )
    .expect("validated native manager spec")
    .with_shape(NativeManagerShape::structure_data(shape))
}
