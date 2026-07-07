use std::borrow::Cow;

use super::super::{
    GameSystemDataTables, GameSystemListElementShape, GameSystemListShape, OwnedCellValue,
    rules::SemanticListSeparators,
    syntax::{StringStats, is_empty_cell},
};
use super::{shape::ShapeEvidence, value_match::string_list_value_matches_shape};

pub(in crate::game_system_schema) fn column_semantic_list_shape(
    data_tables: &GameSystemDataTables,
    table_index: usize,
    column_index: usize,
    row_type_name: &str,
    column_name: &str,
    default_separator: &str,
    separators: SemanticListSeparators,
    element_shape: Option<GameSystemListElementShape>,
    preserve_empty_entries: bool,
) -> Option<GameSystemListShape> {
    let table = data_tables.tables().get(table_index)?;

    let mut stats = StringStats::default();
    for row in table.row_refs() {
        let Some(cell) = row.cells().get(column_index) else {
            continue;
        };
        let value = semantic_list_cell_text(cell.value(), element_shape.as_ref())?;
        if value.trim().is_empty() {
            continue;
        }
        stats.observe_semantic_list(
            column_name,
            value.as_ref(),
            default_separator,
            separators,
            preserve_empty_entries,
        );
    }
    Some(stats.finish_semantic_list(
        row_type_name,
        column_name,
        default_separator,
        separators,
        element_shape,
        preserve_empty_entries,
    ))
}

pub(in crate::game_system_schema) fn column_semantic_list_evidence(
    data_tables: &GameSystemDataTables,
    table_index: usize,
    column_index: usize,
    list: &GameSystemListShape,
) -> ShapeEvidence {
    let Some(table) = data_tables.tables().get(table_index) else {
        return ShapeEvidence::default();
    };

    let mut evidence = ShapeEvidence::default();
    for row in table.row_refs() {
        let Some(cell) = row.cells().get(column_index) else {
            continue;
        };
        let Some(value) = semantic_list_cell_text(cell.value(), list.element_shape.as_ref()) else {
            if !is_empty_cell(cell.value()) {
                evidence.non_empty += 1;
            }
            continue;
        };
        if value.trim().is_empty() {
            continue;
        }
        evidence.non_empty += 1;
        if string_list_value_matches_shape(value.as_ref(), list) {
            evidence.compatible += 1;
        }
    }
    evidence
}

pub(in crate::game_system_schema) fn semantic_list_cell_text<'a>(
    value: &'a OwnedCellValue,
    element_shape: Option<&GameSystemListElementShape>,
) -> Option<Cow<'a, str>> {
    match value {
        OwnedCellValue::String(value) => Some(Cow::Borrowed(value)),
        OwnedCellValue::Number(value) if semantic_list_number_is_empty(*value, element_shape) => {
            Some(Cow::Borrowed(""))
        }
        OwnedCellValue::Number(value) if value.is_finite() => Some(Cow::Owned(value.to_string())),
        OwnedCellValue::Number(_) | OwnedCellValue::Boolean(_) => None,
    }
}

pub(in crate::game_system_schema) fn semantic_list_number_is_empty(
    value: f32,
    element_shape: Option<&GameSystemListElementShape>,
) -> bool {
    value == 0.0 && matches!(element_shape, Some(GameSystemListElementShape::Pair { .. }))
}
