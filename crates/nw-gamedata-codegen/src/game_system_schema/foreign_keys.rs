use std::collections::{HashMap, HashSet, hash_map::Entry};

use super::{
    FOREIGN_KEY_CONFIDENCE_THRESHOLD, FOREIGN_KEY_FAMILY_CONFIDENCE_THRESHOLD,
    FOREIGN_KEY_FAMILY_MIN_CHECKED_VALUES, FOREIGN_KEY_MIN_CHECKED_VALUES, GameSystemColumn,
    GameSystemColumnTypeAffinity, GameSystemColumnTypeRepair, GameSystemColumnTypeRepairKind,
    GameSystemColumnValueShape, GameSystemDataTables, GameSystemForeignKeyCandidate,
    GameSystemListElementShape, GameSystemListShape, GameSystemTable, GameSystemTableSchema,
    GameSystemValidationDiagnostic, GameSystemValidationDiagnosticKind,
    number::{combine_list_element_shapes, usize_ratio},
    rules::{SemanticListRowKey, row_type_specific_list_affinity},
    syntax::{
        canonical_key, columns_semantically_compatible, is_blank_schema_string,
        is_probable_reference_token, key_column_score, reference_key_candidates, row_label,
        schema_tokens, string_column_values,
    },
};
use nw_datasheet::ColumnType;

#[derive(Debug, Clone)]
pub(super) struct KeyIndex {
    sources: Vec<(usize, usize)>,
    table_name: String,
    column_name: String,
    values: HashSet<String>,
    key_score: u8,
}

pub(super) fn build_key_lookup(key_indexes: &[KeyIndex]) -> HashMap<String, Vec<usize>> {
    let mut lookup: HashMap<String, Vec<usize>> = HashMap::new();
    for (index_id, index) in key_indexes.iter().enumerate() {
        for value in &index.values {
            lookup.entry(value.clone()).or_default().push(index_id);
        }
    }
    lookup
}

pub(super) fn collect_key_indexes(data_tables: &GameSystemDataTables) -> Vec<KeyIndex> {
    let mut grouped: HashMap<(String, String), KeyIndex> = HashMap::new();
    for (table_index, table) in data_tables.tables().iter().enumerate() {
        for (column_index, column) in table.columns().iter().enumerate() {
            if column.column_type() != ColumnType::String {
                continue;
            }
            let row_key = column_index == 0;
            if row_type_specific_list_affinity(table.type_name(), column.name())
                .is_some_and(|affinity| affinity.row_key == SemanticListRowKey::Demote)
            {
                continue;
            }

            let values = string_column_values(table, column_index)
                .filter(|value| !is_blank_schema_string(value))
                .map(canonical_key)
                .collect::<HashSet<_>>();
            if values.is_empty() {
                continue;
            }

            let unique = values.len() == table.len();
            let key_score = key_column_score(column.name(), row_key, unique);
            if !is_reference_target_column(row_key, unique, key_score) {
                continue;
            }

            let group_key = (table.type_name().to_owned(), column.name().to_owned());
            match grouped.entry(group_key) {
                Entry::Occupied(mut entry) => {
                    let index = entry.get_mut();
                    index.sources.push((table_index, column_index));
                    index.values.extend(values);
                    index.key_score = index.key_score.max(key_score);
                }
                Entry::Vacant(entry) => {
                    entry.insert(KeyIndex {
                        sources: vec![(table_index, column_index)],
                        table_name: table.type_name().to_owned(),
                        column_name: column.name().to_owned(),
                        values,
                        key_score,
                    });
                }
            }
        }
    }
    let mut out = grouped.into_values().collect::<Vec<_>>();
    out.sort_by(|a, b| {
        a.table_name
            .cmp(&b.table_name)
            .then(a.column_name.cmp(&b.column_name))
    });
    out
}

fn is_reference_target_column(row_key: bool, unique: bool, key_score: u8) -> bool {
    row_key && key_score > 0 || unique && key_score >= 4
}

pub(super) fn infer_foreign_keys(
    data_tables: &GameSystemDataTables,
    table_index: usize,
    column_index: usize,
    column: &GameSystemColumn,
    tokens_by_row: &[Vec<String>],
    key_indexes: &[KeyIndex],
    key_lookup: &HashMap<String, Vec<usize>>,
    diagnostics: &mut Vec<GameSystemValidationDiagnostic>,
) -> Vec<GameSystemForeignKeyCandidate> {
    if column_index == 0 {
        return Vec::new();
    }

    let mut unique_tokens = HashSet::new();
    for row_tokens in tokens_by_row {
        for token in row_tokens {
            if !is_probable_reference_token(token) {
                continue;
            }
            unique_tokens.insert(token.clone());
        }
    }

    if unique_tokens.len() < FOREIGN_KEY_MIN_CHECKED_VALUES {
        return Vec::new();
    }

    let mut matched_by_index: HashMap<usize, HashSet<String>> = HashMap::new();
    for token in &unique_tokens {
        let mut matched_indexes = HashSet::new();
        for candidate in reference_key_candidates(token) {
            let Some(indexes) = key_lookup.get(&candidate) else {
                continue;
            };
            matched_indexes.extend(indexes.iter().copied());
        }
        for index_id in matched_indexes {
            matched_by_index
                .entry(index_id)
                .or_default()
                .insert(token.clone());
        }
    }

    let mut selected = Vec::new();
    let mut remaining = unique_tokens.clone();
    let mut used_indexes = HashSet::new();
    while !remaining.is_empty() {
        let minimum_checked_values = if selected.is_empty() {
            FOREIGN_KEY_MIN_CHECKED_VALUES
        } else {
            1
        };
        if remaining.len() < minimum_checked_values {
            break;
        }

        let mut best: Option<(usize, ForeignKeyMatch<'_>)> = None;
        for (index_id, matched) in &matched_by_index {
            if used_indexes.contains(index_id) {
                continue;
            }

            let index = &key_indexes[*index_id];
            if index
                .sources
                .iter()
                .any(|source| *source == (table_index, column_index))
            {
                continue;
            }
            if !columns_semantically_compatible(column.name(), &index.column_name) {
                continue;
            }

            let matched = matched
                .intersection(&remaining)
                .cloned()
                .collect::<HashSet<_>>();
            if matched.is_empty() {
                continue;
            }

            let checked_values = remaining.len();
            let matched_count =
                u32::try_from(matched.len()).expect("schema match count fits in u32");
            let checked_count =
                u32::try_from(checked_values).expect("schema checked count fits in u32");
            let confidence = f64::from(matched_count) / f64::from(checked_count);
            let minimum_confidence = if remaining.len() < FOREIGN_KEY_MIN_CHECKED_VALUES {
                1.0
            } else {
                FOREIGN_KEY_CONFIDENCE_THRESHOLD
            };
            if confidence < minimum_confidence {
                continue;
            }
            let missing = remaining
                .difference(&matched)
                .cloned()
                .collect::<HashSet<_>>();
            let candidate = ForeignKeyMatch {
                index,
                checked_values,
                matched,
                missing,
                confidence,
            };
            if best
                .as_ref()
                .is_none_or(|(_, current)| candidate.is_better_than(current))
            {
                best = Some((*index_id, candidate));
            }
        }

        let Some((index_id, best)) = best else {
            break;
        };
        used_indexes.insert(index_id);
        for token in &best.matched {
            remaining.remove(token);
        }
        selected.push(best);
    }

    emit_missing_foreign_key_diagnostics(
        data_tables,
        table_index,
        column.name(),
        tokens_by_row,
        &selected,
        &remaining,
        diagnostics,
    );

    selected
        .into_iter()
        .map(|candidate| GameSystemForeignKeyCandidate {
            target_table: candidate.index.table_name.clone(),
            target_column: candidate.index.column_name.clone(),
            checked_values: candidate.checked_values,
            matched_values: candidate.matched.len(),
            missing_values: candidate.missing.len(),
            confidence: candidate.confidence,
        })
        .collect()
}

pub(super) fn emit_missing_foreign_key_diagnostics(
    data_tables: &GameSystemDataTables,
    table_index: usize,
    source_column: &str,
    tokens_by_row: &[Vec<String>],
    selected: &[ForeignKeyMatch<'_>],
    remaining: &HashSet<String>,
    diagnostics: &mut Vec<GameSystemValidationDiagnostic>,
) {
    if selected.is_empty() || remaining.is_empty() {
        return;
    }

    let table = &data_tables.tables()[table_index];
    let mut missing_occurrences: HashMap<String, MissingForeignKeyOccurrence> = HashMap::new();
    for (row_index, row_tokens) in tokens_by_row.iter().enumerate() {
        for token in row_tokens {
            if !remaining.contains(token) {
                continue;
            }

            let Some(row) = table.row_at_index(row_index) else {
                continue;
            };
            missing_occurrences
                .entry(token.clone())
                .and_modify(|occurrence| {
                    occurrence.occurrences = occurrence.occurrences.saturating_add(1);
                })
                .or_insert_with(|| MissingForeignKeyOccurrence {
                    source_row: row_label(table, row_index),
                    source_row_key_crc: row.key_crc(),
                    occurrences: 1,
                });
        }
    }

    let mut missing_occurrences = missing_occurrences.into_iter().collect::<Vec<_>>();
    missing_occurrences.sort_by(|(left, _), (right, _)| left.cmp(right));
    let mut target_tables = selected
        .iter()
        .map(|candidate| candidate.index.table_name.as_str())
        .collect::<HashSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    let mut target_columns = selected
        .iter()
        .map(|candidate| candidate.index.column_name.as_str())
        .collect::<HashSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    target_tables.sort();
    target_columns.sort();
    let target_table = target_tables.join("|");
    let target_column = target_columns.join("|");
    for (value, occurrence) in missing_occurrences {
        diagnostics.push(GameSystemValidationDiagnostic {
            source_table: table.name().to_owned(),
            source_column: source_column.to_owned(),
            source_row: occurrence.source_row,
            source_row_key_crc: occurrence.source_row_key_crc,
            value,
            occurrences: occurrence.occurrences,
            kind: GameSystemValidationDiagnosticKind::MissingForeignKey {
                target_table: target_table.clone(),
                target_column: target_column.clone(),
            },
        });
    }
}

#[derive(Debug)]
pub(super) struct MissingForeignKeyOccurrence {
    source_row: String,
    source_row_key_crc: u32,
    occurrences: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub(super) struct ForeignKeyTarget {
    pub(super) table_name: String,
    pub(super) column_name: String,
}

#[derive(Debug, Default)]
pub(super) struct ForeignKeyFamilyEvidence {
    pub(super) checked_values: usize,
    pub(super) matched_values: usize,
    pub(super) missing_values: usize,
    pub(super) source_columns: usize,
}

#[derive(Debug, Clone)]
pub(super) struct ForeignKeyFamilyTarget {
    target: ForeignKeyTarget,
    list_shape: Option<GameSystemListShape>,
    confidence: f64,
}

#[derive(Debug)]
pub(super) struct ColumnForeignKeyEvidence {
    candidate: GameSystemForeignKeyCandidate,
    missing: HashSet<String>,
    tokens_by_row: Vec<Vec<String>>,
}

pub(super) fn apply_foreign_key_family_affinity(
    data_tables: &GameSystemDataTables,
    key_indexes: &[KeyIndex],
    tables: &mut [GameSystemTableSchema],
    type_affinities: &mut [GameSystemColumnTypeAffinity],
    diagnostics: &mut Vec<GameSystemValidationDiagnostic>,
) {
    let family_targets = foreign_key_family_targets(tables);
    if family_targets.is_empty() {
        return;
    }

    let key_index_by_target = key_indexes
        .iter()
        .map(|index| ((index.table_name.clone(), index.column_name.clone()), index))
        .collect::<HashMap<_, _>>();

    for (table_index, table) in tables.iter_mut().enumerate() {
        for (column_index, column) in table.columns.iter_mut().enumerate() {
            if column.row_key {
                continue;
            }
            let family_key = (table.row_type_name.clone(), column.name.clone());
            let Some(family_target) = family_targets.get(&family_key) else {
                continue;
            };
            let Some(target_index) = key_index_by_target.get(&(
                family_target.target.table_name.clone(),
                family_target.target.column_name.clone(),
            )) else {
                continue;
            };
            let Some(mut evidence) =
                column_foreign_key_evidence(data_tables, table_index, column_index, target_index)
            else {
                continue;
            };
            if evidence.candidate.missing_values > 0 {
                evidence.candidate.confidence =
                    evidence
                        .candidate
                        .confidence
                        .max(foreign_key_classification_confidence(
                            &evidence.candidate,
                            family_target,
                        ));
            }

            emit_missing_foreign_key_diagnostics_for_target(
                data_tables,
                table_index,
                &column.name,
                &evidence.tokens_by_row,
                &evidence.missing,
                &family_target.target,
                diagnostics,
            );

            if !is_column_foreign_key_candidate(&evidence.candidate) {
                continue;
            }

            let before = column.value_shape.clone();
            let GameSystemColumnValueShape::String {
                list, foreign_keys, ..
            } = &mut column.value_shape
            else {
                continue;
            };

            if list.is_none() {
                *list = family_target.list_shape.clone();
            }
            if !foreign_keys.iter().any(|candidate| {
                candidate.target_table == evidence.candidate.target_table
                    && candidate.target_column == evidence.candidate.target_column
            }) {
                foreign_keys.push(evidence.candidate);
                foreign_keys.sort_by(|left, right| {
                    right
                        .confidence
                        .total_cmp(&left.confidence)
                        .then_with(|| right.matched_values.cmp(&left.matched_values))
                        .then_with(|| left.target_table.cmp(&right.target_table))
                        .then_with(|| left.target_column.cmp(&right.target_column))
                });
            }
            record_foreign_key_affinity_repairs(
                data_tables,
                type_affinities,
                table_index,
                column_index,
                &table.table_name,
                &column.name,
                before,
                column.value_shape.clone(),
                &family_target.target,
                family_target.confidence,
                &evidence.missing,
            );
        }
    }
}

pub(super) fn foreign_key_family_targets(
    tables: &[GameSystemTableSchema],
) -> HashMap<(String, String), ForeignKeyFamilyTarget> {
    let mut evidence_by_family =
        HashMap::<(String, String), HashMap<ForeignKeyTarget, ForeignKeyFamilyEvidence>>::new();
    let mut list_shape_by_family = HashMap::<(String, String), GameSystemListShapeBuilder>::new();

    for table in tables {
        for column in &table.columns {
            let GameSystemColumnValueShape::String {
                list, foreign_keys, ..
            } = &column.value_shape
            else {
                continue;
            };
            let family_key = (table.row_type_name.clone(), column.name.clone());
            if let Some(list) = list {
                list_shape_by_family
                    .entry(family_key.clone())
                    .or_default()
                    .observe(list);
            }
            for foreign_key in foreign_keys
                .iter()
                .filter(|foreign_key| is_foreign_key_family_seed(foreign_key))
            {
                let target = ForeignKeyTarget {
                    table_name: foreign_key.target_table.clone(),
                    column_name: foreign_key.target_column.clone(),
                };
                let evidence = evidence_by_family
                    .entry(family_key.clone())
                    .or_default()
                    .entry(target)
                    .or_default();
                evidence.checked_values = evidence
                    .checked_values
                    .saturating_add(foreign_key.checked_values);
                evidence.matched_values = evidence
                    .matched_values
                    .saturating_add(foreign_key.matched_values);
                evidence.missing_values = evidence
                    .missing_values
                    .saturating_add(foreign_key.missing_values);
                evidence.source_columns = evidence.source_columns.saturating_add(1);
            }
        }
    }

    let mut out = HashMap::new();
    for (family_key, by_target) in evidence_by_family {
        let Some((target, evidence)) = best_foreign_key_family_target(by_target) else {
            continue;
        };
        let list_shape = list_shape_by_family
            .get(&family_key)
            .map(GameSystemListShapeBuilder::finish);
        let confidence = foreign_key_family_confidence(&evidence);
        out.insert(
            family_key,
            ForeignKeyFamilyTarget {
                target,
                list_shape,
                confidence,
            },
        );
    }
    out
}

pub(super) fn best_foreign_key_family_target(
    by_target: HashMap<ForeignKeyTarget, ForeignKeyFamilyEvidence>,
) -> Option<(ForeignKeyTarget, ForeignKeyFamilyEvidence)> {
    let mut candidates = by_target
        .into_iter()
        .filter(|(_, evidence)| is_strong_foreign_key_family(evidence))
        .collect::<Vec<_>>();
    candidates.sort_by(|(left_target, left), (right_target, right)| {
        right
            .matched_values
            .cmp(&left.matched_values)
            .then_with(|| right.source_columns.cmp(&left.source_columns))
            .then_with(|| {
                foreign_key_family_confidence(right).total_cmp(&foreign_key_family_confidence(left))
            })
            .then_with(|| left.missing_values.cmp(&right.missing_values))
            .then_with(|| left_target.table_name.cmp(&right_target.table_name))
            .then_with(|| left_target.column_name.cmp(&right_target.column_name))
    });
    let best = candidates.first()?;
    if candidates.get(1).is_some_and(|next| {
        next.1.matched_values == best.1.matched_values
            && next.1.source_columns == best.1.source_columns
            && foreign_key_family_confidence(&next.1) == foreign_key_family_confidence(&best.1)
            && next.1.missing_values == best.1.missing_values
    }) {
        return None;
    }
    Some(candidates.remove(0))
}

pub(super) fn is_foreign_key_family_seed(foreign_key: &GameSystemForeignKeyCandidate) -> bool {
    foreign_key.checked_values >= FOREIGN_KEY_MIN_CHECKED_VALUES
        && foreign_key.confidence >= FOREIGN_KEY_CONFIDENCE_THRESHOLD
}

pub(super) fn is_strong_foreign_key_family(evidence: &ForeignKeyFamilyEvidence) -> bool {
    if evidence.checked_values < FOREIGN_KEY_MIN_CHECKED_VALUES {
        return false;
    }
    if evidence.source_columns >= 2
        && evidence.matched_values == evidence.checked_values
        && evidence.missing_values == 0
    {
        return true;
    }
    evidence.checked_values >= FOREIGN_KEY_FAMILY_MIN_CHECKED_VALUES
        && foreign_key_family_confidence(evidence) >= FOREIGN_KEY_FAMILY_CONFIDENCE_THRESHOLD
}

pub(super) fn foreign_key_family_confidence(evidence: &ForeignKeyFamilyEvidence) -> f64 {
    if evidence.checked_values == 0 {
        0.0
    } else {
        usize_ratio(evidence.matched_values, evidence.checked_values)
    }
}

pub(super) fn is_column_foreign_key_candidate(candidate: &GameSystemForeignKeyCandidate) -> bool {
    if candidate.checked_values == 0 {
        return false;
    }
    if candidate.matched_values == candidate.checked_values && candidate.missing_values == 0 {
        return true;
    }
    candidate.confidence >= FOREIGN_KEY_FAMILY_CONFIDENCE_THRESHOLD
}

pub(super) fn foreign_key_classification_confidence(
    candidate: &GameSystemForeignKeyCandidate,
    family_target: &ForeignKeyFamilyTarget,
) -> f64 {
    if candidate.matched_values == candidate.checked_values && candidate.missing_values == 0 {
        candidate.confidence
    } else {
        family_target.confidence
    }
}

pub(super) fn column_foreign_key_evidence(
    data_tables: &GameSystemDataTables,
    table_index: usize,
    column_index: usize,
    target: &KeyIndex,
) -> Option<ColumnForeignKeyEvidence> {
    let table = data_tables.tables().get(table_index)?;
    let column = table.columns().get(column_index)?;
    let tokens_by_row = column_tokens_by_row(table, column_index, column.name());
    let mut unique_tokens = HashSet::new();
    for row_tokens in &tokens_by_row {
        for token in row_tokens {
            if is_probable_reference_token(token) {
                unique_tokens.insert(token.clone());
            }
        }
    }
    if unique_tokens.is_empty() {
        return None;
    }

    let matched = unique_tokens
        .iter()
        .filter(|token| token_matches_key_index(token, target))
        .cloned()
        .collect::<HashSet<_>>();
    let missing = unique_tokens
        .difference(&matched)
        .cloned()
        .collect::<HashSet<_>>();
    let checked_values = unique_tokens.len();
    let matched_values = matched.len();
    Some(ColumnForeignKeyEvidence {
        candidate: GameSystemForeignKeyCandidate {
            target_table: target.table_name.clone(),
            target_column: target.column_name.clone(),
            checked_values,
            matched_values,
            missing_values: missing.len(),
            confidence: usize_ratio(matched_values, checked_values),
        },
        missing,
        tokens_by_row,
    })
}

pub(super) fn column_tokens_by_row(
    table: &GameSystemTable,
    column_index: usize,
    column_name: &str,
) -> Vec<Vec<String>> {
    table
        .row_refs()
        .map(|row| {
            row.cells()
                .get(column_index)
                .and_then(|cell| cell.value().as_str())
                .map(|value| schema_tokens(column_name, value))
                .unwrap_or_default()
        })
        .collect()
}

pub(super) fn token_matches_key_index(token: &str, target: &KeyIndex) -> bool {
    reference_key_candidates(token)
        .into_iter()
        .any(|candidate| target.values.contains(&candidate))
}

pub(super) fn emit_missing_foreign_key_diagnostics_for_target(
    data_tables: &GameSystemDataTables,
    table_index: usize,
    source_column: &str,
    tokens_by_row: &[Vec<String>],
    missing: &HashSet<String>,
    target: &ForeignKeyTarget,
    diagnostics: &mut Vec<GameSystemValidationDiagnostic>,
) {
    if missing.is_empty() {
        return;
    }

    let table = &data_tables.tables()[table_index];
    let mut missing_occurrences: HashMap<String, MissingForeignKeyOccurrence> = HashMap::new();
    for (row_index, row_tokens) in tokens_by_row.iter().enumerate() {
        for token in row_tokens {
            if !missing.contains(token) {
                continue;
            }
            let Some(row) = table.row_at_index(row_index) else {
                continue;
            };
            missing_occurrences
                .entry(token.clone())
                .and_modify(|occurrence| {
                    occurrence.occurrences = occurrence.occurrences.saturating_add(1);
                })
                .or_insert_with(|| MissingForeignKeyOccurrence {
                    source_row: row_label(table, row_index),
                    source_row_key_crc: row.key_crc(),
                    occurrences: 1,
                });
        }
    }

    let mut missing_occurrences = missing_occurrences.into_iter().collect::<Vec<_>>();
    missing_occurrences.sort_by(|(left, _), (right, _)| left.cmp(right));
    for (value, occurrence) in missing_occurrences {
        if diagnostics.iter().any(|diagnostic| {
            diagnostic.source_table == table.name()
                && diagnostic.source_column == source_column
                && diagnostic.source_row_key_crc == occurrence.source_row_key_crc
                && diagnostic.value == value
                && matches!(
                    &diagnostic.kind,
                    GameSystemValidationDiagnosticKind::MissingForeignKey {
                        target_table,
                        target_column,
                    } if target_table == &target.table_name && target_column == &target.column_name
                )
        }) {
            continue;
        }
        diagnostics.push(GameSystemValidationDiagnostic {
            source_table: table.name().to_owned(),
            source_column: source_column.to_owned(),
            source_row: occurrence.source_row,
            source_row_key_crc: occurrence.source_row_key_crc,
            value,
            occurrences: occurrence.occurrences,
            kind: GameSystemValidationDiagnosticKind::MissingForeignKey {
                target_table: target.table_name.clone(),
                target_column: target.column_name.clone(),
            },
        });
    }
}

pub(super) fn record_foreign_key_affinity_repairs(
    data_tables: &GameSystemDataTables,
    type_affinities: &mut [GameSystemColumnTypeAffinity],
    table_index: usize,
    column_index: usize,
    table_name: &str,
    column_name: &str,
    from: GameSystemColumnValueShape,
    to: GameSystemColumnValueShape,
    target: &ForeignKeyTarget,
    confidence: f64,
    missing: &HashSet<String>,
) {
    let Some(affinity) = type_affinities
        .iter_mut()
        .find(|affinity| affinity.table_name == table_name && affinity.column_name == column_name)
    else {
        return;
    };
    affinity.effective_shape = to.clone();
    affinity.confidence = affinity.confidence.min(confidence);
    if from != to {
        affinity.repairable = true;
        affinity.repairs.push(GameSystemColumnTypeRepair {
            kind: GameSystemColumnTypeRepairKind::ForeignKey,
            from: from.clone(),
            to: to.clone(),
            confidence,
            reason: format!(
                "row type `{}` column `{}` uses family-wide foreign-key affinity to `{}.{}`",
                affinity.row_type_name, affinity.column_name, target.table_name, target.column_name
            ),
            row_index: None,
            value: None,
            adjacent_column: None,
            adjacent_direction: None,
        });
    }

    if missing.is_empty() {
        return;
    }
    let Some(table) = data_tables.tables().get(table_index) else {
        return;
    };
    let tokens_by_row = column_tokens_by_row(table, column_index, column_name);
    for (row_index, row_tokens) in tokens_by_row.iter().enumerate() {
        for token in row_tokens {
            if !missing.contains(token) {
                continue;
            }
            affinity.repairable = true;
            affinity.repairs.push(GameSystemColumnTypeRepair {
                kind: GameSystemColumnTypeRepairKind::ForeignKey,
                from: from.clone(),
                to: to.clone(),
                confidence,
                reason: format!(
                    "row {row_index} value `{token}` does not resolve to foreign-key target `{}.{}`",
                    target.table_name, target.column_name
                ),
                row_index: Some(row_index),
                value: Some(token.clone()),
                adjacent_column: None,
                adjacent_direction: None,
            });
        }
    }
}

#[derive(Debug, Default)]
pub(super) struct GameSystemListShapeBuilder {
    separators: HashSet<String>,
    rows_with_lists: usize,
    total_entries: usize,
    preserve_empty_entries: bool,
    element_shape: Option<GameSystemListElementShape>,
}

impl GameSystemListShapeBuilder {
    pub(super) fn observe(&mut self, list: &GameSystemListShape) {
        self.separators.extend(list.separators.iter().cloned());
        self.rows_with_lists = self.rows_with_lists.saturating_add(list.rows_with_lists);
        self.total_entries = self.total_entries.saturating_add(list.total_entries);
        self.preserve_empty_entries |= list.preserve_empty_entries;
        if let Some(element_shape) = &list.element_shape {
            self.element_shape = Some(match self.element_shape.take() {
                Some(current) => combine_list_element_shapes(current, element_shape.clone()),
                None => element_shape.clone(),
            });
        }
    }

    pub(super) fn finish(&self) -> GameSystemListShape {
        let mut separators = self.separators.iter().cloned().collect::<Vec<_>>();
        separators.sort();
        GameSystemListShape {
            separators,
            rows_with_lists: self.rows_with_lists,
            total_entries: self.total_entries,
            preserve_empty_entries: self.preserve_empty_entries,
            element_shape: self.element_shape.clone(),
        }
    }
}

#[derive(Debug)]
pub(super) struct ForeignKeyMatch<'a> {
    index: &'a KeyIndex,
    checked_values: usize,
    matched: HashSet<String>,
    missing: HashSet<String>,
    confidence: f64,
}

impl ForeignKeyMatch<'_> {
    pub(super) fn is_better_than(&self, other: &Self) -> bool {
        // Total order: the trailing name comparisons break exact stat ties so
        // the greedy winner never depends on HashMap iteration order (which
        // varies per process and made regeneration non-deterministic).
        self.confidence
            .total_cmp(&other.confidence)
            .then(self.matched.len().cmp(&other.matched.len()))
            .then(self.index.key_score.cmp(&other.index.key_score))
            .then(other.index.table_name.cmp(&self.index.table_name))
            .then(other.index.column_name.cmp(&self.index.column_name))
            .is_gt()
    }
}
