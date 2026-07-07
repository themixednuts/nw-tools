use std::collections::HashMap;

use super::super::{
    GameSystemAdjacentColumnDirection, GameSystemCell, GameSystemColumnTypeRepair,
    GameSystemColumnTypeRepairKind, GameSystemColumnValueShape, GameSystemDataTables,
    GameSystemTableSchema, OwnedCellValue, TYPE_AFFINITY_CONFIDENCE_THRESHOLD,
    affinity::TypeAffinityState,
    rules::column_has_boolean_affinity,
    syntax::{canonical_key, is_empty_cell},
};
use super::{shape::column_shape_evidence, value_match::cell_value_matches_shape};

pub(in crate::game_system_schema) fn detect_adjacent_column_shifts(
    data_tables: &GameSystemDataTables,
    tables: &[GameSystemTableSchema],
    states: &mut [TypeAffinityState],
) {
    let state_by_column = states
        .iter()
        .enumerate()
        .map(|(state_index, state)| ((state.table_index, state.column_index), state_index))
        .collect::<HashMap<_, _>>();

    for state_index in 0..states.len() {
        let table_index = states[state_index].table_index;
        let column_index = states[state_index].column_index;
        let Some(table) = data_tables.tables().get(table_index) else {
            continue;
        };
        let Some(table_schema) = tables.get(table_index) else {
            continue;
        };
        let expected_shapes = adjacent_shift_expected_shapes(data_tables, &states[state_index]);

        let mut repairs = Vec::new();
        for row in table.row_refs() {
            let row_cells = row.cells();
            let Some(cell) = row_cells.get(column_index) else {
                continue;
            };
            if is_empty_cell(cell.value()) {
                continue;
            }

            for expected_shape in &expected_shapes {
                if cell_value_matches_shape(cell.value(), expected_shape) {
                    continue;
                }

                if let Some(repair) = adjacent_column_shift_repair(
                    data_tables,
                    &states[state_index],
                    row.index(),
                    cell.value(),
                    expected_shape,
                    table_schema,
                    &state_by_column,
                    states,
                    table_index,
                    column_index,
                    row_cells,
                    GameSystemAdjacentColumnDirection::Left,
                ) {
                    repairs.push(repair);
                }
                if let Some(repair) = adjacent_column_shift_repair(
                    data_tables,
                    &states[state_index],
                    row.index(),
                    cell.value(),
                    expected_shape,
                    table_schema,
                    &state_by_column,
                    states,
                    table_index,
                    column_index,
                    row_cells,
                    GameSystemAdjacentColumnDirection::Right,
                ) {
                    repairs.push(repair);
                }
            }
        }

        states[state_index].repairs.extend(repairs);
    }
}

pub(in crate::game_system_schema) fn adjacent_shift_expected_shapes(
    data_tables: &GameSystemDataTables,
    state: &TypeAffinityState,
) -> Vec<GameSystemColumnValueShape> {
    let mut shapes = vec![state.effective_shape.clone()];
    let boolean_shape = GameSystemColumnValueShape::Boolean;
    if state.effective_shape != boolean_shape
        && column_has_boolean_affinity(&state.row_type_name, &state.column_name)
        && column_shape_evidence(
            data_tables,
            state.table_index,
            state.column_index,
            &boolean_shape,
        )
        .confidence()
            >= TYPE_AFFINITY_CONFIDENCE_THRESHOLD
    {
        shapes.push(boolean_shape);
    }
    shapes
}

pub(in crate::game_system_schema) fn adjacent_column_shift_repair(
    data_tables: &GameSystemDataTables,
    source_state: &TypeAffinityState,
    row_index: usize,
    value: &OwnedCellValue,
    expected_shape: &GameSystemColumnValueShape,
    table_schema: &GameSystemTableSchema,
    state_by_column: &HashMap<(usize, usize), usize>,
    states: &[TypeAffinityState],
    table_index: usize,
    column_index: usize,
    row_cells: &[GameSystemCell],
    direction: GameSystemAdjacentColumnDirection,
) -> Option<GameSystemColumnTypeRepair> {
    let adjacent_column_index = match direction {
        GameSystemAdjacentColumnDirection::Left => column_index.checked_sub(1)?,
        GameSystemAdjacentColumnDirection::Right => column_index + 1,
    };
    let adjacent_column = table_schema.columns.get(adjacent_column_index)?;
    let adjacent_state_index = *state_by_column.get(&(table_index, adjacent_column_index))?;
    let adjacent_state = &states[adjacent_state_index];
    if !cell_value_matches_shape(value, &adjacent_state.effective_shape) {
        return None;
    }
    let adjacent_cell_is_open = row_cells.get(adjacent_column_index).is_none_or(|cell| {
        is_empty_cell(cell.value())
            || !cell_value_matches_shape(cell.value(), &adjacent_state.effective_shape)
    });
    if !adjacent_cell_is_open {
        return None;
    }
    let evidence = adjacent_column_shift_evidence(
        data_tables,
        source_state,
        adjacent_state,
        table_index,
        adjacent_column_index,
        row_index,
        value,
    )?;

    let direction_label = match direction {
        GameSystemAdjacentColumnDirection::Left => "left",
        GameSystemAdjacentColumnDirection::Right => "right",
    };
    Some(GameSystemColumnTypeRepair {
        kind: GameSystemColumnTypeRepairKind::AdjacentColumn,
        from: expected_shape.clone(),
        to: adjacent_state.effective_shape.clone(),
        confidence: evidence.confidence,
        reason: format!(
            "row {row_index} value matches the {direction_label} adjacent column `{}` domain; source confidence {:.2}, adjacent confidence {:.2}",
            adjacent_column.name, evidence.source_confidence, evidence.adjacent_confidence
        ),
        row_index: Some(row_index),
        value: Some(value.to_string()),
        adjacent_column: Some(adjacent_column.name.clone()),
        adjacent_direction: Some(direction),
    })
}

#[derive(Debug, Clone, Copy)]
pub(in crate::game_system_schema) struct AdjacentColumnShiftEvidence {
    source_confidence: f64,
    adjacent_confidence: f64,
    confidence: f64,
}

pub(in crate::game_system_schema) fn adjacent_column_shift_evidence(
    data_tables: &GameSystemDataTables,
    source_state: &TypeAffinityState,
    adjacent_state: &TypeAffinityState,
    table_index: usize,
    adjacent_column_index: usize,
    row_index: usize,
    value: &OwnedCellValue,
) -> Option<AdjacentColumnShiftEvidence> {
    let adjacent_evidence = column_shape_evidence(
        data_tables,
        table_index,
        adjacent_column_index,
        &adjacent_state.effective_shape,
    );
    let adjacent_confidence = adjacent_evidence.confidence();
    if adjacent_evidence.has_no_values() || adjacent_confidence < TYPE_AFFINITY_CONFIDENCE_THRESHOLD
    {
        return None;
    }

    if !column_domain_contains_value(
        data_tables,
        table_index,
        adjacent_column_index,
        row_index,
        value,
    ) {
        return None;
    }

    let source_confidence = source_state.confidence;
    Some(AdjacentColumnShiftEvidence {
        source_confidence,
        adjacent_confidence,
        confidence: source_confidence.min(adjacent_confidence),
    })
}

pub(in crate::game_system_schema) fn column_domain_contains_value(
    data_tables: &GameSystemDataTables,
    table_index: usize,
    column_index: usize,
    ignored_row_index: usize,
    value: &OwnedCellValue,
) -> bool {
    let Some(table) = data_tables.tables().get(table_index) else {
        return false;
    };

    table.row_refs().any(|row| {
        if row.index() == ignored_row_index {
            return false;
        }
        let Some(cell) = row.cells().get(column_index) else {
            return false;
        };
        !is_empty_cell(cell.value()) && domain_values_match(cell.value(), value)
    })
}

pub(in crate::game_system_schema) fn domain_values_match(
    candidate: &OwnedCellValue,
    value: &OwnedCellValue,
) -> bool {
    match (candidate, value) {
        (OwnedCellValue::String(candidate), OwnedCellValue::String(value)) => {
            canonical_key(candidate) == canonical_key(value)
        }
        (OwnedCellValue::Number(candidate), OwnedCellValue::Number(value)) => candidate == value,
        (OwnedCellValue::Boolean(candidate), OwnedCellValue::Boolean(value)) => candidate == value,
        _ => false,
    }
}
