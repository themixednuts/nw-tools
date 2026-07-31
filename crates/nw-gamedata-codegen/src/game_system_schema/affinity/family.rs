use std::collections::HashMap;

use super::TypeAffinityState;
use crate::game_system_schema::{
    GameSystemColumnTypeRepair, GameSystemColumnTypeRepairKind, GameSystemColumnValueShape,
    GameSystemDataTables, GameSystemNumberShape, NATIVE_NUMERIC_TEXT_CONFIDENCE_THRESHOLD,
    TYPE_AFFINITY_CONFIDENCE_THRESHOLD,
    evidence::*,
    number::combine_number_shapes,
    rules::{
        column_has_boolean_affinity, numeric_column_has_float_affinity,
        string_column_blocks_numeric_affinity,
    },
};

#[derive(Debug, Default)]
pub(super) struct NumberFamilyAffinity {
    shape: Option<GameSystemNumberShape>,
    has_observed_float: bool,
}

#[derive(Debug, Default)]
pub(super) struct FamilyShapeAffinity {
    number: NumberFamilyAffinity,
    number_columns: usize,
    number_like_string_columns: usize,
    native_numeric_text_columns: usize,
    empty_string_columns: usize,
    string_columns: usize,
    scalar_string_columns: usize,
    identifier_string_columns: usize,
    boolean_columns: usize,
    semantic_boolean: bool,
    semantic_float: bool,
}

pub(super) fn apply_family_shape_affinity(
    data_tables: &GameSystemDataTables,
    states: &mut [TypeAffinityState],
) {
    let mut shape_by_family: HashMap<(String, String), FamilyShapeAffinity> = HashMap::new();
    for state in states.iter() {
        if state.effective_shape.requires_authored_string() {
            continue;
        }
        let family = (state.row_type_name.clone(), state.column_name.clone());
        let affinity = shape_by_family.entry(family).or_default();
        affinity.semantic_boolean |=
            column_has_boolean_affinity(&state.row_type_name, &state.column_name);
        if !string_column_blocks_numeric_affinity(&state.row_type_name, &state.column_name) {
            affinity.semantic_float |=
                numeric_column_has_float_affinity(&state.row_type_name, &state.column_name);
        }
        match &state.effective_shape {
            GameSystemColumnValueShape::Boolean => affinity.boolean_columns += 1,
            GameSystemColumnValueShape::String {
                identifier_like,
                list,
                ..
            } => {
                affinity.string_columns += 1;
                if list.is_none() {
                    affinity.scalar_string_columns += 1;
                }
                if list.is_none() && *identifier_like {
                    affinity.identifier_string_columns += 1;
                }
                if let Some(number_evidence) = column_parseable_number_shape(
                    data_tables,
                    state.table_index,
                    state.column_index,
                ) && number_evidence.evidence.confidence() >= TYPE_AFFINITY_CONFIDENCE_THRESHOLD
                {
                    affinity.number_like_string_columns += 1;
                    affinity.number.shape = Some(match affinity.number.shape {
                        Some(shape) => combine_number_shapes(shape, number_evidence.shape),
                        None => number_evidence.shape,
                    });
                    affinity.number.has_observed_float |=
                        number_evidence.shape == GameSystemNumberShape::Float;
                } else if list.is_none()
                    && !string_column_blocks_numeric_affinity(
                        &state.row_type_name,
                        &state.column_name,
                    )
                    && let Some(number_evidence) = column_native_numeric_text_shape(
                        data_tables,
                        state.table_index,
                        state.column_index,
                    )
                    && number_evidence.confidence() >= NATIVE_NUMERIC_TEXT_CONFIDENCE_THRESHOLD
                {
                    affinity.native_numeric_text_columns += 1;
                    affinity.number.shape = Some(match affinity.number.shape {
                        Some(shape) => combine_number_shapes(shape, number_evidence.shape),
                        None => number_evidence.shape,
                    });
                    affinity.number.has_observed_float |=
                        number_evidence.shape == GameSystemNumberShape::Float;
                } else if column_shape_evidence(
                    data_tables,
                    state.table_index,
                    state.column_index,
                    &state.effective_shape,
                )
                .has_no_values()
                {
                    affinity.empty_string_columns += 1;
                }
            }
            GameSystemColumnValueShape::Number { number_shape } => {
                affinity.number_columns += 1;
                affinity.number.shape = Some(match affinity.number.shape {
                    Some(shape) => combine_number_shapes(shape, *number_shape),
                    None => *number_shape,
                });
                affinity.number.has_observed_float |= matches!(
                    &state.observed_shape,
                    GameSystemColumnValueShape::Number {
                        number_shape: GameSystemNumberShape::Float
                    }
                );
            }
            GameSystemColumnValueShape::Color { .. }
            | GameSystemColumnValueShape::Crc32
            | GameSystemColumnValueShape::Enum { .. }
            | GameSystemColumnValueShape::Range { .. } => {}
        }
    }

    for state in states {
        if state.effective_row_key || state.effective_shape.requires_authored_string() {
            continue;
        };
        if matches!(
            state.effective_shape,
            GameSystemColumnValueShape::String { list: Some(_), .. }
        ) {
            continue;
        }
        let family = (state.row_type_name.clone(), state.column_name.clone());
        let Some(family_affinity) = shape_by_family.get(&family) else {
            continue;
        };

        let only_empty_scalar_string_siblings = family_affinity.string_columns > 0
            && family_affinity.string_columns == family_affinity.scalar_string_columns
            && family_affinity.scalar_string_columns == family_affinity.empty_string_columns;
        if family_affinity.boolean_columns > 0
            && (family_affinity.semantic_boolean || only_empty_scalar_string_siblings)
        {
            apply_family_shape_repair(
                data_tables,
                state,
                GameSystemColumnValueShape::Boolean,
                if family_affinity.semantic_boolean {
                    0.85
                } else {
                    0.90
                },
                "uses family-wide boolean affinity",
            );
            continue;
        }

        if !family_affinity.semantic_boolean
            && family_affinity.scalar_string_columns >= family_affinity.boolean_columns
            && matches!(state.effective_shape, GameSystemColumnValueShape::Boolean)
        {
            apply_family_text_shape_repair(data_tables, state, family_affinity);
            continue;
        }

        let Some(family_shape) = family_affinity.number.shape else {
            continue;
        };
        if family_affinity.number_columns == 0 && !family_affinity.semantic_float {
            continue;
        }
        let non_numeric_string_columns = family_affinity
            .string_columns
            .saturating_sub(family_affinity.empty_string_columns)
            .saturating_sub(family_affinity.number_like_string_columns)
            .saturating_sub(family_affinity.native_numeric_text_columns);
        if family_affinity.number_columns
            + family_affinity.number_like_string_columns
            + family_affinity.native_numeric_text_columns
            <= non_numeric_string_columns
            && !family_affinity.semantic_float
        {
            continue;
        }

        let confidence = match &state.effective_shape {
            GameSystemColumnValueShape::Number { .. }
                if family_affinity.number.has_observed_float =>
            {
                0.95
            }
            GameSystemColumnValueShape::Number { .. } => 0.90,
            GameSystemColumnValueShape::String { .. } if family_affinity.semantic_float => 0.80,
            GameSystemColumnValueShape::String { .. } => 0.75,
            GameSystemColumnValueShape::Boolean
            | GameSystemColumnValueShape::Color { .. }
            | GameSystemColumnValueShape::Crc32
            | GameSystemColumnValueShape::Enum { .. }
            | GameSystemColumnValueShape::Range { .. } => continue,
        };
        let target_shape = GameSystemColumnValueShape::Number {
            number_shape: family_shape,
        };
        if matches!(
            state.effective_shape,
            GameSystemColumnValueShape::String { .. }
        ) && string_column_blocks_numeric_affinity(&state.row_type_name, &state.column_name)
            && !column_shape_evidence(
                data_tables,
                state.table_index,
                state.column_index,
                &target_shape,
            )
            .has_no_values()
        {
            continue;
        }
        apply_family_shape_repair(
            data_tables,
            state,
            target_shape,
            confidence,
            "uses family-wide numeric affinity",
        );
    }
}

pub(super) fn apply_family_text_shape_repair(
    data_tables: &GameSystemDataTables,
    state: &mut TypeAffinityState,
    family_affinity: &FamilyShapeAffinity,
) {
    let from = state.effective_shape.clone();
    let to = GameSystemColumnValueShape::String {
        identifier_like: family_affinity.identifier_string_columns > 0,
        localized_key_like: false,
        asset_path_like: false,
        expression_like: false,
        qualified_reference_like: false,
        list: None,
        foreign_keys: Vec::new(),
    };
    let confidence: f64 = 0.75;
    state.repairs.push(GameSystemColumnTypeRepair {
        kind: GameSystemColumnTypeRepairKind::NativeText,
        from,
        to: to.clone(),
        confidence,
        reason: format!(
            "row type `{}` column `{}` uses family-wide text affinity; native text helpers stringify boolean cells",
            state.row_type_name, state.column_name
        ),
        row_index: None,
        value: None,
        adjacent_column: None,
        adjacent_direction: None,
    });
    state.repairs.extend(native_text_cell_repairs(
        data_tables,
        state,
        &to,
        confidence,
    ));
    state.effective_shape = to;
    state.confidence = state.confidence.min(confidence);
}

pub(super) fn apply_family_shape_repair(
    data_tables: &GameSystemDataTables,
    state: &mut TypeAffinityState,
    to: GameSystemColumnValueShape,
    confidence: f64,
    reason: &'static str,
) {
    if state.effective_shape == to {
        return;
    }
    let evidence = column_shape_evidence(data_tables, state.table_index, state.column_index, &to);
    let mut kind = GameSystemColumnTypeRepairKind::Family;
    let mut repair_reason = format!(
        "row type `{}` column `{}` {reason}",
        state.row_type_name, state.column_name
    );
    let mut cell_repairs = Vec::new();
    let evidence_confidence = if !evidence.has_no_values()
        && evidence.confidence() < TYPE_AFFINITY_CONFIDENCE_THRESHOLD
        && matches!(
            &state.effective_shape,
            GameSystemColumnValueShape::String { .. }
        )
        && matches!(&to, GameSystemColumnValueShape::Number { .. })
        && !matches!(
            &state.effective_shape,
            GameSystemColumnValueShape::String { list: Some(_), .. }
        ) {
        let Some(native_evidence) = column_native_numeric_text_evidence(
            data_tables,
            state.table_index,
            state.column_index,
            &to,
        ) else {
            return;
        };
        if !native_evidence.is_complete()
            || native_evidence.confidence() < NATIVE_NUMERIC_TEXT_CONFIDENCE_THRESHOLD
        {
            return;
        }
        kind = GameSystemColumnTypeRepairKind::NativeNumericText;
        repair_reason =
            format!("{repair_reason}; native numeric helpers coerce nonnumeric text to zero");
        cell_repairs = native_numeric_text_zero_fallback_repairs(
            data_tables,
            state,
            &to,
            native_evidence.confidence(),
        );
        native_evidence.confidence()
    } else {
        evidence.confidence()
    };
    if matches!(&to, GameSystemColumnValueShape::Boolean)
        && !evidence.has_no_values()
        && !evidence.is_complete()
    {
        return;
    }
    if evidence_confidence < TYPE_AFFINITY_CONFIDENCE_THRESHOLD
        && kind != GameSystemColumnTypeRepairKind::NativeNumericText
    {
        return;
    }
    let confidence = confidence.min(evidence_confidence);

    let from = state.effective_shape.clone();
    state.repairs.push(GameSystemColumnTypeRepair {
        kind,
        from,
        to: to.clone(),
        confidence,
        reason: repair_reason,
        row_index: None,
        value: None,
        adjacent_column: None,
        adjacent_direction: None,
    });
    state.repairs.extend(cell_repairs);
    state.effective_shape = to;
    state.confidence = state.confidence.min(confidence);
}
