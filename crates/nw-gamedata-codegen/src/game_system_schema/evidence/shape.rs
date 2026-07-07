use super::super::{
    GameSystemColumnValueShape, GameSystemDataTables, GameSystemNumberShape, OwnedCellValue,
    number::{NumberStats, usize_ratio},
    range::{RangeInclusiveF32TextValue, parse_range_inclusive_f32_text},
    syntax::is_empty_cell,
};
use super::value_match::{cell_value_as_number, cell_value_matches_shape};

#[derive(Debug, Clone, Copy, Default)]
pub(in crate::game_system_schema) struct ShapeEvidence {
    pub(in crate::game_system_schema) non_empty: usize,
    pub(in crate::game_system_schema) compatible: usize,
}

impl ShapeEvidence {
    pub(in crate::game_system_schema) fn has_no_values(self) -> bool {
        self.non_empty == 0
    }

    pub(in crate::game_system_schema) fn is_complete(self) -> bool {
        self.non_empty > 0 && self.compatible == self.non_empty
    }

    pub(in crate::game_system_schema) fn confidence(self) -> f64 {
        if self.non_empty == 0 {
            return 1.0;
        }
        usize_ratio(self.compatible, self.non_empty)
    }
}

#[derive(Debug, Clone, Copy)]
pub(in crate::game_system_schema) struct NumberShapeEvidence {
    pub(in crate::game_system_schema) shape: GameSystemNumberShape,
    pub(in crate::game_system_schema) evidence: ShapeEvidence,
}

#[derive(Debug, Clone, Copy, Default)]
pub(in crate::game_system_schema) struct RangeTextEvidence {
    pub(in crate::game_system_schema) non_empty: usize,
    pub(in crate::game_system_schema) parsed: usize,
    pub(in crate::game_system_schema) compatible: usize,
}

impl RangeTextEvidence {
    pub(in crate::game_system_schema) fn is_complete(self) -> bool {
        self.non_empty > 0 && self.compatible == self.non_empty
    }

    pub(in crate::game_system_schema) fn confidence(self) -> f64 {
        if self.non_empty == 0 {
            return 1.0;
        }
        usize_ratio(self.parsed, self.non_empty)
    }
}

pub(in crate::game_system_schema) fn column_range_text_evidence(
    data_tables: &GameSystemDataTables,
    table_index: usize,
    column_index: usize,
) -> Option<RangeTextEvidence> {
    let table = data_tables.tables().get(table_index)?;

    let mut evidence = RangeTextEvidence::default();
    for row in table.row_refs() {
        let Some(cell) = row.cells().get(column_index) else {
            continue;
        };
        if is_empty_cell(cell.value()) {
            continue;
        }
        evidence.non_empty += 1;
        match cell.value() {
            OwnedCellValue::Number(value) if value.is_finite() => {
                evidence.compatible += 1;
                evidence.parsed += 1;
            }
            OwnedCellValue::String(value) => {
                evidence.compatible += 1;
                if matches!(
                    parse_range_inclusive_f32_text(value),
                    RangeInclusiveF32TextValue::Parsed(_)
                ) {
                    evidence.parsed += 1;
                }
            }
            OwnedCellValue::Number(_) | OwnedCellValue::Boolean(_) => {}
        }
    }
    (evidence.non_empty > 0).then_some(evidence)
}

pub(in crate::game_system_schema) fn column_parseable_number_shape(
    data_tables: &GameSystemDataTables,
    table_index: usize,
    column_index: usize,
) -> Option<NumberShapeEvidence> {
    let table = data_tables.tables().get(table_index)?;

    let mut stats = NumberStats::default();
    let mut parsed_values = 0usize;
    for row in table.row_refs() {
        let Some(cell) = row.cells().get(column_index) else {
            continue;
        };
        if is_empty_cell(cell.value()) {
            continue;
        }
        let Some(value) = cell_value_as_number(cell.value()) else {
            continue;
        };
        stats.observe(value);
        parsed_values += 1;
    }
    if parsed_values == 0 {
        return None;
    }

    let shape = stats.finish_observed();
    let evidence = column_shape_evidence(
        data_tables,
        table_index,
        column_index,
        &GameSystemColumnValueShape::Number {
            number_shape: shape,
        },
    );
    Some(NumberShapeEvidence { shape, evidence })
}

pub(in crate::game_system_schema) fn column_shape_evidence(
    data_tables: &GameSystemDataTables,
    table_index: usize,
    column_index: usize,
    shape: &GameSystemColumnValueShape,
) -> ShapeEvidence {
    let Some(table) = data_tables.tables().get(table_index) else {
        return ShapeEvidence::default();
    };

    let mut evidence = ShapeEvidence::default();
    for row in table.row_refs() {
        let Some(cell) = row.cells().get(column_index) else {
            continue;
        };
        let matches_shape = cell_value_matches_shape(cell.value(), shape);
        if is_empty_cell(cell.value()) && !empty_cell_counts_for_shape(cell.value(), shape) {
            continue;
        }
        evidence.non_empty += 1;
        if matches_shape {
            evidence.compatible += 1;
        }
    }
    evidence
}

fn empty_cell_counts_for_shape(value: &OwnedCellValue, shape: &GameSystemColumnValueShape) -> bool {
    matches!(shape, GameSystemColumnValueShape::Enum { .. })
        && cell_value_matches_shape(value, shape)
}
