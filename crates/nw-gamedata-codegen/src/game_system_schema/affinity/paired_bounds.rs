use std::collections::HashMap;

use super::TypeAffinityState;
use crate::game_system_schema::{
    GameSystemColumnTypeRepair, GameSystemColumnTypeRepairKind, GameSystemColumnValueShape,
    GameSystemDataTables, GameSystemNumberShape, evidence::column_shape_evidence,
    number::combine_number_shapes, semantic::ColumnSemanticProfile,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum BoundRole {
    Min,
    Max,
}

#[derive(Debug, Default)]
pub(super) struct PairedBoundColumns {
    min: Vec<usize>,
    max: Vec<usize>,
}

pub(super) fn apply_paired_bound_shape_affinity(
    data_tables: &GameSystemDataTables,
    states: &mut [TypeAffinityState],
) {
    let mut pairs: HashMap<(usize, Vec<String>), PairedBoundColumns> = HashMap::new();

    for (state_index, state) in states.iter().enumerate() {
        if state.effective_row_key || state_number_shape(state).is_none() {
            continue;
        }

        let Some((key, role)) = paired_bound_key(&state.column_name) else {
            continue;
        };

        let pair = pairs.entry((state.table_index, key)).or_default();
        match role {
            BoundRole::Min => pair.min.push(state_index),
            BoundRole::Max => pair.max.push(state_index),
        }
    }

    for pair in pairs.into_values() {
        let ([min_index], [max_index]) = (pair.min.as_slice(), pair.max.as_slice()) else {
            continue;
        };

        let Some(min_shape) = state_number_shape(&states[*min_index]) else {
            continue;
        };
        let Some(max_shape) = state_number_shape(&states[*max_index]) else {
            continue;
        };

        let paired_shape = combine_number_shapes(min_shape, max_shape);
        let max_column = states[*max_index].column_name.clone();
        apply_paired_bound_shape_repair(
            data_tables,
            &mut states[*min_index],
            paired_shape,
            &max_column,
        );

        let min_column = states[*min_index].column_name.clone();
        apply_paired_bound_shape_repair(
            data_tables,
            &mut states[*max_index],
            paired_shape,
            &min_column,
        );
    }
}

pub(super) fn apply_paired_bound_shape_repair(
    data_tables: &GameSystemDataTables,
    state: &mut TypeAffinityState,
    number_shape: GameSystemNumberShape,
    paired_column: &str,
) {
    let to = GameSystemColumnValueShape::Number { number_shape };
    if state.effective_shape == to {
        return;
    }

    let evidence = column_shape_evidence(data_tables, state.table_index, state.column_index, &to);
    if !evidence.has_no_values() && !evidence.is_complete() {
        return;
    }

    let confidence = 0.90_f64.min(evidence.confidence());
    let from = state.effective_shape.clone();
    state.repairs.push(GameSystemColumnTypeRepair {
        kind: GameSystemColumnTypeRepairKind::SemanticName,
        from,
        to: to.clone(),
        confidence,
        reason: format!(
            "numeric bound column `{}` shares paired min/max affinity with `{paired_column}`",
            state.column_name
        ),
        row_index: None,
        value: None,
        adjacent_column: None,
        adjacent_direction: None,
    });
    state.effective_shape = to;
    state.confidence = state.confidence.min(confidence);
}

pub(super) fn state_number_shape(state: &TypeAffinityState) -> Option<GameSystemNumberShape> {
    let GameSystemColumnValueShape::Number { number_shape } = &state.effective_shape else {
        return None;
    };
    Some(*number_shape)
}

pub(super) fn paired_bound_key(column_name: &str) -> Option<(Vec<String>, BoundRole)> {
    let profile = ColumnSemanticProfile::new("", column_name);
    let words = profile.words();
    if words.is_empty() {
        return None;
    }

    if words[0] == "min" {
        return Some((words[1..].to_vec(), BoundRole::Min));
    }
    if words[0] == "max" {
        return Some((words[1..].to_vec(), BoundRole::Max));
    }

    match words.last().map(String::as_str) {
        Some("min") => Some((words[..words.len() - 1].to_vec(), BoundRole::Min)),
        Some("max") => Some((words[..words.len() - 1].to_vec(), BoundRole::Max)),
        _ => None,
    }
}
