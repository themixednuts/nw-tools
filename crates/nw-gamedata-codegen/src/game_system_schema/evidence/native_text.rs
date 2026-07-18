use super::super::{
    GameSystemColumnTypeRepair, GameSystemColumnTypeRepairKind, GameSystemColumnValueShape,
    GameSystemDataTables, GameSystemNumberShape, OwnedCellValue,
    affinity::TypeAffinityState,
    number::{NumberStats, number_matches_shape, usize_ratio},
    syntax::{is_empty_cell, is_empty_schema_string},
};
use super::shape::ShapeEvidence;

#[derive(Debug, Clone, Copy)]
pub(in crate::game_system_schema) struct NativeNumericTextEvidence {
    pub(in crate::game_system_schema) shape: GameSystemNumberShape,
    pub(in crate::game_system_schema) non_empty: usize,
    pub(in crate::game_system_schema) parsed: usize,
    pub(in crate::game_system_schema) compatible: usize,
    pub(in crate::game_system_schema) zero_fallbacks: usize,
}

impl NativeNumericTextEvidence {
    pub(in crate::game_system_schema) fn is_complete(self) -> bool {
        self.non_empty > 0 && self.zero_fallbacks > 0 && self.compatible == self.non_empty
    }

    pub(in crate::game_system_schema) fn values_parse(self) -> bool {
        self.non_empty > 0 && self.parsed == self.non_empty && self.compatible == self.non_empty
    }

    pub(in crate::game_system_schema) fn confidence(self) -> f64 {
        if self.non_empty == 0 {
            return 1.0;
        }
        usize_ratio(self.parsed, self.non_empty)
    }
}

pub(in crate::game_system_schema) fn column_literal_boolean_text_evidence(
    data_tables: &GameSystemDataTables,
    table_index: usize,
    column_index: usize,
) -> ShapeEvidence {
    let Some(table) = data_tables.tables().get(table_index) else {
        return ShapeEvidence::default();
    };

    let mut evidence = ShapeEvidence::default();
    for row in table.row_refs() {
        let Some(cell) = row.cells().get(column_index) else {
            continue;
        };
        let OwnedCellValue::String(value) = cell.value() else {
            continue;
        };
        if is_empty_schema_string(value) {
            continue;
        }
        evidence.non_empty += 1;
        if is_literal_boolean_text(value) {
            evidence.compatible += 1;
        }
    }
    evidence
}

pub(in crate::game_system_schema) fn column_native_numeric_text_shape(
    data_tables: &GameSystemDataTables,
    table_index: usize,
    column_index: usize,
) -> Option<NativeNumericTextEvidence> {
    collect_native_numeric_text_evidence(data_tables, table_index, column_index, None)
}

pub(in crate::game_system_schema) fn column_native_numeric_text_evidence(
    data_tables: &GameSystemDataTables,
    table_index: usize,
    column_index: usize,
    shape: &GameSystemColumnValueShape,
) -> Option<NativeNumericTextEvidence> {
    let GameSystemColumnValueShape::Number { number_shape } = shape else {
        return None;
    };
    collect_native_numeric_text_evidence(
        data_tables,
        table_index,
        column_index,
        Some(*number_shape),
    )
}

pub(in crate::game_system_schema) fn collect_native_numeric_text_evidence(
    data_tables: &GameSystemDataTables,
    table_index: usize,
    column_index: usize,
    expected_shape: Option<GameSystemNumberShape>,
) -> Option<NativeNumericTextEvidence> {
    let table = data_tables.tables().get(table_index)?;

    let mut stats = NumberStats::default();
    let mut non_empty = 0usize;
    let mut parsed = 0usize;
    let mut compatible = 0usize;
    let mut zero_fallbacks = 0usize;
    for row in table.row_refs() {
        let Some(cell) = row.cells().get(column_index) else {
            continue;
        };
        if is_empty_cell(cell.value()) {
            continue;
        }
        let OwnedCellValue::String(value) = cell.value() else {
            return None;
        };
        non_empty += 1;
        let numeric = match parse_native_numeric_text(value) {
            NativeNumericTextValue::Parsed(value) => {
                parsed += 1;
                value
            }
            NativeNumericTextValue::ZeroFallback => {
                zero_fallbacks += 1;
                0.0
            }
        };
        stats.observe(numeric);
        if expected_shape.is_none_or(|shape| number_matches_shape(numeric, shape)) {
            compatible += 1;
        }
    }
    if non_empty == 0 {
        return None;
    }

    Some(NativeNumericTextEvidence {
        shape: stats.finish_observed(),
        non_empty,
        parsed,
        compatible,
        zero_fallbacks,
    })
}

pub(in crate::game_system_schema) enum NativeNumericTextValue {
    Parsed(f32),
    ZeroFallback,
}

pub(in crate::game_system_schema) fn parse_native_numeric_text(
    value: &str,
) -> NativeNumericTextValue {
    match native_float_prefix(value).and_then(|value| value.parse::<f32>().ok()) {
        Some(value) if value.is_finite() => NativeNumericTextValue::Parsed(value),
        _ => NativeNumericTextValue::ZeroFallback,
    }
}

pub fn native_float_prefix(value: &str) -> Option<&str> {
    let value = value.trim_start();
    let bytes = value.as_bytes();
    let mut index = 0;

    if bytes
        .get(index)
        .is_some_and(|byte| matches!(byte, b'+' | b'-'))
    {
        index += 1;
    }

    let whole_start = index;
    while bytes.get(index).is_some_and(u8::is_ascii_digit) {
        index += 1;
    }
    let whole_digits = index - whole_start;

    let mut fractional_digits = 0;
    if bytes.get(index) == Some(&b'.') {
        index += 1;
        let fractional_start = index;
        while bytes.get(index).is_some_and(u8::is_ascii_digit) {
            index += 1;
        }
        fractional_digits = index - fractional_start;
    }

    if whole_digits + fractional_digits == 0 {
        return None;
    }

    let mantissa_end = index;
    if bytes
        .get(index)
        .is_some_and(|byte| matches!(byte, b'e' | b'E'))
    {
        index += 1;
        if bytes
            .get(index)
            .is_some_and(|byte| matches!(byte, b'+' | b'-'))
        {
            index += 1;
        }
        let exponent_start = index;
        while bytes.get(index).is_some_and(u8::is_ascii_digit) {
            index += 1;
        }
        if index == exponent_start {
            index = mantissa_end;
        }
    }

    Some(&value[..index])
}

pub(in crate::game_system_schema) fn is_literal_boolean_text(value: &str) -> bool {
    matches!(value.trim().to_ascii_lowercase().as_str(), "true" | "false")
}

pub(in crate::game_system_schema) fn native_numeric_text_zero_fallback_repairs(
    data_tables: &GameSystemDataTables,
    state: &TypeAffinityState,
    to: &GameSystemColumnValueShape,
    confidence: f64,
) -> Vec<GameSystemColumnTypeRepair> {
    let Some(table) = data_tables.tables().get(state.table_index) else {
        return Vec::new();
    };

    let mut repairs = Vec::new();
    for row in table.row_refs() {
        let Some(cell) = row.cells().get(state.column_index) else {
            continue;
        };
        let OwnedCellValue::String(value) = cell.value() else {
            continue;
        };
        if is_empty_schema_string(value)
            || matches!(
                parse_native_numeric_text(value),
                NativeNumericTextValue::Parsed(_)
            )
        {
            continue;
        }
        repairs.push(GameSystemColumnTypeRepair {
            kind: GameSystemColumnTypeRepairKind::NativeNumericText,
            from: state.effective_shape.clone(),
            to: to.clone(),
            confidence,
            reason: format!(
                "row {} value `{value}` is repaired through native numeric text zero coercion",
                row.index()
            ),
            row_index: Some(row.index()),
            value: Some(value.clone()),
            adjacent_column: None,
            adjacent_direction: None,
        });
    }
    repairs
}

pub(in crate::game_system_schema) fn native_text_cell_repairs(
    data_tables: &GameSystemDataTables,
    state: &TypeAffinityState,
    to: &GameSystemColumnValueShape,
    confidence: f64,
) -> Vec<GameSystemColumnTypeRepair> {
    let Some(table) = data_tables.tables().get(state.table_index) else {
        return Vec::new();
    };

    let mut repairs = Vec::new();
    for row in table.row_refs() {
        let Some(cell) = row.cells().get(state.column_index) else {
            continue;
        };
        if matches!(cell.value(), OwnedCellValue::String(_)) || is_empty_cell(cell.value()) {
            continue;
        }
        repairs.push(GameSystemColumnTypeRepair {
            kind: GameSystemColumnTypeRepairKind::NativeText,
            from: state.effective_shape.clone(),
            to: to.clone(),
            confidence,
            reason: format!(
                "row {} value `{}` is repaired through native text coercion",
                row.index(),
                cell.value()
            ),
            row_index: Some(row.index()),
            value: Some(cell.value().to_string()),
            adjacent_column: None,
            adjacent_direction: None,
        });
    }
    repairs
}
