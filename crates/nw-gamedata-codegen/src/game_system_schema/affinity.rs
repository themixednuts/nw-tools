use super::{
    GameSystemColumnTypeAffinity, GameSystemColumnTypeRepair, GameSystemColumnTypeRepairKind,
    GameSystemColumnValueShape, GameSystemDataTables, GameSystemListElementShape,
    GameSystemTableSchema, NATIVE_NUMERIC_TEXT_CONFIDENCE_THRESHOLD,
    TYPE_AFFINITY_CONFIDENCE_THRESHOLD, evidence::*, rules::*,
};
use nw_datasheet::ColumnType;

mod family;
mod paired_bounds;

use family::apply_family_shape_affinity;
use paired_bounds::apply_paired_bound_shape_affinity;

#[derive(Debug)]
pub(super) struct TypeAffinityState {
    pub(super) table_index: usize,
    pub(super) column_index: usize,
    pub(super) observed_row_key: bool,
    pub(super) effective_row_key: bool,
    pub(super) declared_type: ColumnType,
    pub(super) table_name: String,
    pub(super) row_type_name: String,
    pub(super) column_name: String,
    pub(super) observed_shape: GameSystemColumnValueShape,
    pub(super) effective_shape: GameSystemColumnValueShape,
    pub(super) confidence: f64,
    pub(super) repairs: Vec<GameSystemColumnTypeRepair>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum ColumnTypeAffinityPolicy {
    LiteralBooleanText,
    SemanticEnum,
    SemanticCrc32,
    SemanticColor,
    SemanticBoolean,
    SemanticRange,
    SemanticList,
    SemanticNumber,
}

impl ColumnTypeAffinityPolicy {
    pub(super) fn apply(self, data_tables: &GameSystemDataTables, state: &mut TypeAffinityState) {
        match self {
            Self::LiteralBooleanText => apply_literal_boolean_text_affinity(data_tables, state),
            Self::SemanticEnum => apply_semantic_enum_affinity(data_tables, state),
            Self::SemanticCrc32 => apply_semantic_crc32_affinity(data_tables, state),
            Self::SemanticColor => apply_semantic_color_affinity(data_tables, state),
            Self::SemanticBoolean => apply_semantic_boolean_affinity(data_tables, state),
            Self::SemanticRange => apply_semantic_range_affinity(data_tables, state),
            Self::SemanticList => apply_semantic_list_affinity(data_tables, state),
            Self::SemanticNumber => apply_semantic_number_affinity(data_tables, state),
        }
    }
}

const COLUMN_TYPE_AFFINITY_POLICIES: &[ColumnTypeAffinityPolicy] = &[
    ColumnTypeAffinityPolicy::LiteralBooleanText,
    ColumnTypeAffinityPolicy::SemanticEnum,
    ColumnTypeAffinityPolicy::SemanticCrc32,
    ColumnTypeAffinityPolicy::SemanticColor,
    ColumnTypeAffinityPolicy::SemanticBoolean,
    ColumnTypeAffinityPolicy::SemanticRange,
    ColumnTypeAffinityPolicy::SemanticList,
    ColumnTypeAffinityPolicy::SemanticNumber,
];

pub(super) fn apply_type_affinity(
    data_tables: &GameSystemDataTables,
    tables: &mut [GameSystemTableSchema],
) -> Vec<GameSystemColumnTypeAffinity> {
    let mut states = collect_type_affinity_states(tables);
    for state in &mut states {
        if state.effective_shape.is_expression_like() {
            continue;
        }
        if state.effective_shape.is_qualified_reference_like() {
            ColumnTypeAffinityPolicy::SemanticList.apply(data_tables, state);
            continue;
        }
        for policy in COLUMN_TYPE_AFFINITY_POLICIES {
            policy.apply(data_tables, state);
        }
    }
    apply_family_shape_affinity(data_tables, &mut states);
    apply_paired_bound_shape_affinity(data_tables, &mut states);
    detect_adjacent_column_shifts(data_tables, tables, &mut states);

    for state in &states {
        tables[state.table_index].columns[state.column_index].row_key = state.effective_row_key;
        tables[state.table_index].columns[state.column_index].value_shape =
            state.effective_shape.clone();
    }

    states
        .into_iter()
        .map(|state| GameSystemColumnTypeAffinity {
            table_name: state.table_name,
            row_type_name: state.row_type_name,
            column_name: state.column_name,
            declared_type: state.declared_type,
            observed_row_key: state.observed_row_key,
            effective_row_key: state.effective_row_key,
            observed_shape: state.observed_shape,
            effective_shape: state.effective_shape,
            confidence: state.confidence,
            repairable: !state.repairs.is_empty(),
            repairs: state.repairs,
        })
        .collect()
}

pub(super) fn collect_type_affinity_states(
    tables: &[GameSystemTableSchema],
) -> Vec<TypeAffinityState> {
    let mut states = Vec::new();
    for (table_index, table) in tables.iter().enumerate() {
        for (column_index, column) in table.columns.iter().enumerate() {
            let observed_shape = column.value_shape.clone();
            let state = TypeAffinityState {
                table_index,
                column_index,
                observed_row_key: column.row_key,
                effective_row_key: column.row_key,
                declared_type: column.declared_type,
                table_name: table.table_name.clone(),
                row_type_name: table.row_type_name.clone(),
                column_name: column.name.clone(),
                observed_shape: observed_shape.clone(),
                effective_shape: observed_shape,
                confidence: 1.0,
                repairs: Vec::new(),
            };
            states.push(state);
        }
    }
    states
}

pub(super) fn apply_literal_boolean_text_affinity(
    data_tables: &GameSystemDataTables,
    state: &mut TypeAffinityState,
) {
    if state.effective_row_key || state.effective_shape == GameSystemColumnValueShape::Boolean {
        return;
    }
    let GameSystemColumnValueShape::String {
        list, foreign_keys, ..
    } = &state.effective_shape
    else {
        return;
    };
    if list.is_some() || !foreign_keys.is_empty() {
        return;
    }

    let evidence =
        column_literal_boolean_text_evidence(data_tables, state.table_index, state.column_index);
    if !evidence.is_complete() {
        return;
    }

    let from = state.effective_shape.clone();
    let to = GameSystemColumnValueShape::Boolean;
    let confidence = evidence.confidence();
    state.repairs.push(GameSystemColumnTypeRepair {
        kind: GameSystemColumnTypeRepairKind::NativeBooleanText,
        from,
        to: to.clone(),
        confidence,
        reason: format!(
            "string column `{}` has complete literal boolean text evidence",
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

pub(super) fn apply_semantic_enum_affinity(
    data_tables: &GameSystemDataTables,
    state: &mut TypeAffinityState,
) {
    if state.effective_row_key {
        return;
    }
    let Some(enum_shape) = scalar_enum_column_affinity(&state.row_type_name, &state.column_name)
    else {
        return;
    };
    if matches!(
        &state.effective_shape,
        GameSystemColumnValueShape::Enum {
            enum_shape: current
        } if current == &enum_shape
    ) {
        return;
    }
    let GameSystemColumnValueShape::String { list: None, .. } = &state.effective_shape else {
        return;
    };

    let to = GameSystemColumnValueShape::Enum { enum_shape };
    let evidence = column_shape_evidence(data_tables, state.table_index, state.column_index, &to);
    if !evidence.has_no_values() && evidence.confidence() < TYPE_AFFINITY_CONFIDENCE_THRESHOLD {
        return;
    }

    let from = state.effective_shape.clone();
    let confidence: f64 = if evidence.has_no_values() { 0.75 } else { 0.95 };
    state.repairs.push(GameSystemColumnTypeRepair {
        kind: GameSystemColumnTypeRepairKind::SemanticName,
        from,
        to: to.clone(),
        confidence,
        reason: format!(
            "column `{}` has native scalar enum semantic affinity",
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

pub(super) fn apply_semantic_crc32_affinity(
    data_tables: &GameSystemDataTables,
    state: &mut TypeAffinityState,
) {
    if state.effective_row_key
        || !scalar_crc32_column_has_affinity(&state.row_type_name, &state.column_name)
    {
        return;
    }
    if state.effective_shape == GameSystemColumnValueShape::Crc32 {
        return;
    }
    let GameSystemColumnValueShape::String { list: None, .. } = &state.effective_shape else {
        return;
    };

    let to = GameSystemColumnValueShape::Crc32;
    let evidence = column_shape_evidence(data_tables, state.table_index, state.column_index, &to);
    if !evidence.has_no_values() && evidence.confidence() < TYPE_AFFINITY_CONFIDENCE_THRESHOLD {
        return;
    }

    let from = state.effective_shape.clone();
    let confidence: f64 = if evidence.has_no_values() { 0.75 } else { 0.95 };
    state.repairs.push(GameSystemColumnTypeRepair {
        kind: GameSystemColumnTypeRepairKind::SemanticName,
        from,
        to: to.clone(),
        confidence,
        reason: format!(
            "column `{}` has native CRC32 semantic affinity",
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

pub(super) fn apply_semantic_boolean_affinity(
    data_tables: &GameSystemDataTables,
    state: &mut TypeAffinityState,
) {
    if state.effective_row_key
        || !column_has_boolean_affinity(&state.row_type_name, &state.column_name)
    {
        return;
    }
    if state.effective_shape == GameSystemColumnValueShape::Boolean {
        return;
    }

    let from = state.effective_shape.clone();
    let to = GameSystemColumnValueShape::Boolean;
    let evidence = column_shape_evidence(data_tables, state.table_index, state.column_index, &to);
    if !evidence.is_complete() && !evidence.has_no_values() {
        return;
    }
    if evidence.has_no_values() && family_declares_boolean_column(data_tables, state) {
        return;
    }
    let confidence = if evidence.has_no_values() {
        0.75
    } else {
        0.80_f64.min(evidence.confidence())
    };
    state.repairs.push(GameSystemColumnTypeRepair {
        kind: GameSystemColumnTypeRepairKind::SemanticName,
        from,
        to: to.clone(),
        confidence,
        reason: format!(
            "column `{}` has boolean-like semantic name affinity",
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

fn family_declares_boolean_column(
    data_tables: &GameSystemDataTables,
    state: &TypeAffinityState,
) -> bool {
    data_tables
        .tables()
        .iter()
        .enumerate()
        .filter(|(table_index, table)| {
            *table_index != state.table_index && table.type_name() == state.row_type_name
        })
        .flat_map(|(_, table)| table.columns())
        .any(|column| {
            column.name() == state.column_name && column.column_type() == ColumnType::Boolean
        })
}

pub(super) fn apply_semantic_color_affinity(
    data_tables: &GameSystemDataTables,
    state: &mut TypeAffinityState,
) {
    if state.effective_row_key {
        return;
    }
    let Some(color_shape) = color_column_has_affinity(&state.row_type_name, &state.column_name)
    else {
        return;
    };
    if matches!(
        &state.effective_shape,
        GameSystemColumnValueShape::Color {
            color_shape: current
        } if *current == color_shape
    ) {
        return;
    }
    let GameSystemColumnValueShape::String { list: None, .. } = &state.effective_shape else {
        return;
    };

    let to = GameSystemColumnValueShape::Color { color_shape };
    let evidence = column_shape_evidence(data_tables, state.table_index, state.column_index, &to);
    if !evidence.has_no_values() && !evidence.is_complete() {
        return;
    }

    let from = state.effective_shape.clone();
    let confidence: f64 = if evidence.has_no_values() { 0.75 } else { 0.95 };
    state.repairs.push(GameSystemColumnTypeRepair {
        kind: GameSystemColumnTypeRepairKind::SemanticColor,
        from,
        to: to.clone(),
        confidence,
        reason: format!(
            "column `{}` has validated color semantic affinity; authored hex text is compiled as linear RGBA",
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

pub(super) fn apply_semantic_number_affinity(
    data_tables: &GameSystemDataTables,
    state: &mut TypeAffinityState,
) {
    if state.effective_row_key
        && !matches!(
            state.effective_shape,
            GameSystemColumnValueShape::Number { .. }
        )
    {
        return;
    }
    if string_column_blocks_numeric_affinity(&state.row_type_name, &state.column_name) {
        return;
    }
    let Some(number_shape) =
        numeric_column_has_number_affinity(&state.row_type_name, &state.column_name)
    else {
        return;
    };
    match &state.effective_shape {
        GameSystemColumnValueShape::Number {
            number_shape: current_shape,
        } if *current_shape == number_shape => return,
        GameSystemColumnValueShape::String { foreign_keys, .. } if !foreign_keys.is_empty() => {
            return;
        }
        GameSystemColumnValueShape::String { list: Some(_), .. }
            if !numeric_column_allows_authored_suffix(&state.row_type_name, &state.column_name) =>
        {
            return;
        }
        GameSystemColumnValueShape::Number { .. } | GameSystemColumnValueShape::String { .. } => {}
        GameSystemColumnValueShape::Boolean
        | GameSystemColumnValueShape::Color { .. }
        | GameSystemColumnValueShape::Crc32
        | GameSystemColumnValueShape::Enum { .. }
        | GameSystemColumnValueShape::Range { .. } => return,
    }

    let from = state.effective_shape.clone();
    let to = GameSystemColumnValueShape::Number { number_shape };
    let evidence = column_shape_evidence(data_tables, state.table_index, state.column_index, &to);
    let mut kind = GameSystemColumnTypeRepairKind::SemanticName;
    let mut reason = format!(
        "numeric column `{}` has semantic number affinity",
        state.column_name
    );
    let mut cell_repairs = Vec::new();
    let evidence_confidence = if evidence.confidence() < TYPE_AFFINITY_CONFIDENCE_THRESHOLD
        && !evidence.has_no_values()
        && matches!(
            &state.effective_shape,
            GameSystemColumnValueShape::String { .. }
        ) {
        let Some(native_evidence) = column_native_numeric_text_evidence(
            data_tables,
            state.table_index,
            state.column_index,
            &to,
        ) else {
            return;
        };
        if native_evidence.values_parse() {
            reason = format!("{reason}; native numeric parser consumes authored numeric prefixes");
            native_evidence.confidence()
        } else if native_evidence.is_complete() {
            kind = GameSystemColumnTypeRepairKind::NativeNumericText;
            reason = format!("{reason}; native numeric helpers coerce nonnumeric text to zero");
            cell_repairs = native_numeric_text_zero_fallback_repairs(
                data_tables,
                state,
                &to,
                NATIVE_NUMERIC_TEXT_CONFIDENCE_THRESHOLD,
            );
            NATIVE_NUMERIC_TEXT_CONFIDENCE_THRESHOLD
        } else {
            return;
        }
    } else if evidence.confidence() < TYPE_AFFINITY_CONFIDENCE_THRESHOLD
        && !evidence.has_no_values()
    {
        return;
    } else {
        evidence.confidence()
    };
    let semantic_confidence: f64 = if evidence.has_no_values() {
        0.75
    } else {
        match from {
            GameSystemColumnValueShape::Number { .. } => 0.85,
            GameSystemColumnValueShape::String { .. } => 0.80,
            GameSystemColumnValueShape::Boolean
            | GameSystemColumnValueShape::Color { .. }
            | GameSystemColumnValueShape::Crc32
            | GameSystemColumnValueShape::Enum { .. }
            | GameSystemColumnValueShape::Range { .. } => {
                unreachable!("non-number shape returned above")
            }
        }
    };
    let confidence = semantic_confidence.min(evidence_confidence);
    state.repairs.push(GameSystemColumnTypeRepair {
        kind,
        from,
        to: to.clone(),
        confidence,
        reason,
        row_index: None,
        value: None,
        adjacent_column: None,
        adjacent_direction: None,
    });
    state.repairs.extend(cell_repairs);
    state.effective_shape = to;
    state.confidence = state.confidence.min(confidence);
}

pub(super) fn apply_semantic_range_affinity(
    data_tables: &GameSystemDataTables,
    state: &mut TypeAffinityState,
) {
    if state.effective_row_key {
        return;
    }
    let Some(range_affinity) = range_column_has_affinity(&state.row_type_name, &state.column_name)
    else {
        return;
    };
    match &state.effective_shape {
        GameSystemColumnValueShape::Range {
            bounds: current_bounds,
            number_shape: current_number_shape,
        } if *current_bounds == range_affinity.bounds
            && *current_number_shape == range_affinity.number_shape =>
        {
            return;
        }
        GameSystemColumnValueShape::String { list: Some(_), .. } => return,
        GameSystemColumnValueShape::String { .. } | GameSystemColumnValueShape::Number { .. } => {}
        GameSystemColumnValueShape::Boolean
        | GameSystemColumnValueShape::Color { .. }
        | GameSystemColumnValueShape::Crc32
        | GameSystemColumnValueShape::Enum { .. }
        | GameSystemColumnValueShape::Range { .. } => return,
    }

    let to = GameSystemColumnValueShape::Range {
        bounds: range_affinity.bounds,
        number_shape: range_affinity.number_shape,
    };
    let evidence = column_shape_evidence(data_tables, state.table_index, state.column_index, &to);
    if !evidence.has_no_values() && evidence.confidence() < TYPE_AFFINITY_CONFIDENCE_THRESHOLD {
        return;
    }
    let range_evidence =
        column_range_text_evidence(data_tables, state.table_index, state.column_index);
    if range_evidence.is_some_and(|evidence| !evidence.is_complete()) {
        return;
    }

    let from = state.effective_shape.clone();
    let semantic_confidence: f64 = if evidence.has_no_values() {
        0.75
    } else {
        match &from {
            GameSystemColumnValueShape::Number { .. } => 0.90,
            GameSystemColumnValueShape::String { .. } => 0.85,
            GameSystemColumnValueShape::Boolean
            | GameSystemColumnValueShape::Color { .. }
            | GameSystemColumnValueShape::Crc32
            | GameSystemColumnValueShape::Enum { .. }
            | GameSystemColumnValueShape::Range { .. } => {
                unreachable!("non-range-repair shape returned above")
            }
        }
    };
    let confidence = semantic_confidence
        .min(evidence.confidence())
        .min(range_evidence.map_or(1.0, RangeTextEvidence::confidence));
    state.repairs.push(GameSystemColumnTypeRepair {
        kind: GameSystemColumnTypeRepairKind::NativeRangeText,
        from,
        to: to.clone(),
        confidence,
        reason: format!(
            "column `{}` has range semantic name affinity and complete range evidence",
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

pub(super) fn apply_semantic_list_affinity(
    data_tables: &GameSystemDataTables,
    state: &mut TypeAffinityState,
) {
    let Some(list_affinity) = column_has_list_affinity(&state.row_type_name, &state.column_name)
    else {
        return;
    };
    if state.effective_row_key && list_affinity.row_key != SemanticListRowKey::Demote {
        return;
    }
    let (
        identifier_like,
        localized_key_like,
        asset_path_like,
        expression_like,
        qualified_reference_like,
        current_list,
        foreign_keys,
    ) = match &state.effective_shape {
        GameSystemColumnValueShape::String {
            identifier_like,
            localized_key_like,
            asset_path_like,
            expression_like,
            qualified_reference_like,
            list,
            foreign_keys,
        } => (
            *identifier_like,
            *localized_key_like,
            *asset_path_like,
            *expression_like,
            *qualified_reference_like,
            list.as_ref(),
            foreign_keys.clone(),
        ),
        GameSystemColumnValueShape::Number { .. } => {
            (false, false, false, false, false, None, Vec::new())
        }
        GameSystemColumnValueShape::Boolean
        | GameSystemColumnValueShape::Color { .. }
        | GameSystemColumnValueShape::Crc32
        | GameSystemColumnValueShape::Enum { .. }
        | GameSystemColumnValueShape::Range { .. } => return,
    };

    let semantic_list = if list_affinity.preserve_empty_entries || list_affinity.separators.exact()
    {
        let Some(semantic_list) = column_semantic_list_shape(
            data_tables,
            state.table_index,
            state.column_index,
            &state.row_type_name,
            &state.column_name,
            list_affinity.separator,
            list_affinity.separators,
            list_affinity.element_shape.clone(),
            list_affinity.preserve_empty_entries,
        ) else {
            return;
        };
        if current_list == Some(&semantic_list) {
            return;
        }
        semantic_list
    } else if let Some(current_list) = current_list {
        match list_affinity.element_shape.clone() {
            Some(element_shape) => {
                let mut list = current_list.clone();
                if current_list.element_shape.as_ref() == Some(&element_shape)
                    && current_list.preserve_empty_entries == list_affinity.preserve_empty_entries
                {
                    return;
                }
                list.element_shape = Some(element_shape.clone());
                list.preserve_empty_entries = list_affinity.preserve_empty_entries;
                list
            }
            None => {
                if current_list.preserve_empty_entries == list_affinity.preserve_empty_entries {
                    return;
                }
                let mut list = current_list.clone();
                list.preserve_empty_entries = list_affinity.preserve_empty_entries;
                list
            }
        }
    } else {
        let Some(list) = column_semantic_list_shape(
            data_tables,
            state.table_index,
            state.column_index,
            &state.row_type_name,
            &state.column_name,
            list_affinity.separator,
            list_affinity.separators,
            list_affinity.element_shape.clone(),
            list_affinity.preserve_empty_entries,
        ) else {
            return;
        };
        list
    };
    let to = GameSystemColumnValueShape::String {
        identifier_like,
        localized_key_like,
        asset_path_like,
        expression_like,
        qualified_reference_like,
        list: Some(semantic_list),
        foreign_keys,
    };
    let GameSystemColumnValueShape::String {
        list: Some(semantic_list),
        ..
    } = &to
    else {
        return;
    };
    let evidence = column_semantic_list_evidence(
        data_tables,
        state.table_index,
        state.column_index,
        semantic_list,
    );
    if matches!(
        semantic_list.element_shape.as_ref(),
        Some(GameSystemListElementShape::Pair { .. })
    ) && !evidence.has_no_values()
        && !evidence.is_complete()
    {
        return;
    }
    if !list_affinity.separators.exact()
        && !evidence.has_no_values()
        && evidence.confidence() < TYPE_AFFINITY_CONFIDENCE_THRESHOLD
    {
        return;
    }

    let from = state.effective_shape.clone();
    let demotes_row_key =
        state.effective_row_key && list_affinity.row_key == SemanticListRowKey::Demote;
    let confidence = list_affinity.confidence.min(evidence.confidence());
    state.repairs.push(GameSystemColumnTypeRepair {
        kind: GameSystemColumnTypeRepairKind::SemanticName,
        from,
        to: to.clone(),
        confidence,
        reason: if demotes_row_key {
            format!(
                "row-key column `{}` has native list semantic affinity and is not a scalar lookup key",
                state.column_name
            )
        } else {
            format!("column `{}` has native list semantic affinity", state.column_name)
        },
        row_index: None,
        value: None,
        adjacent_column: None,
        adjacent_direction: None,
    });
    state.effective_shape = to;
    if demotes_row_key {
        state.effective_row_key = false;
    }
    state.confidence = state.confidence.min(confidence);
}
