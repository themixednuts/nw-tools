use crate::game_system_schema::GameSystemColorShape;

pub(in crate::game_system_schema) fn color_column_has_affinity(
    row_type_name: &str,
    column_name: &str,
) -> Option<GameSystemColorShape> {
    (row_type_name == "CrestPartData" && column_name == "Color")
        .then_some(GameSystemColorShape::LinearRgba)
}
