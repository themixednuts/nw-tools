use super::{
    GameSystemDataError, GameSystemListElementShape, GameSystemListShape, GameSystemTable,
    OwnedCellValue,
    evidence::parse_schema_bool,
    number::{NumberStats, string_token_as_number},
    rules::{
        SemanticListSeparators, numeric_column_has_number_affinity,
        string_column_has_scalar_text_affinity,
    },
    semantic::SemanticWords,
};
use std::collections::HashSet;

#[derive(Debug, Default)]
pub(super) struct StringStats {
    pub(super) saw_value: bool,
    pub(super) identifier_like: bool,
    pub(super) localized_key_like: bool,
    pub(super) asset_path_like: bool,
    pub(super) expression_like: bool,
    pub(super) rows_with_lists: usize,
    pub(super) total_entries: usize,
    pub(super) separators: HashSet<String>,
    pub(super) list_elements: ListElementStats,
}

impl StringStats {
    pub(super) fn observe(&mut self, column_name: &str, value: &str, preserve_empty_entries: bool) {
        let trimmed = value.trim();
        if trimmed.is_empty() {
            return;
        }

        self.identifier_like =
            !self.saw_value || self.identifier_like && is_identifier_like(trimmed);
        self.localized_key_like |= is_localized_key_like(trimmed);
        self.asset_path_like |= is_asset_path_like(trimmed);
        self.expression_like |= is_expression_like(column_name, trimmed);
        let list_entries = schema_list_entries(column_name, value, preserve_empty_entries);
        if list_entries.len() > 1 {
            self.rows_with_lists += 1;
        }
        self.total_entries += list_entries.len();
        self.list_elements.observe(&list_entries);
        for separator in detected_separators(trimmed) {
            self.separators.insert(separator.to_owned());
        }
        self.saw_value = true;
    }

    pub(super) fn observe_semantic_list(
        &mut self,
        column_name: &str,
        value: &str,
        default_separator: &str,
        separators: SemanticListSeparators,
        preserve_empty_entries: bool,
    ) {
        let trimmed = value.trim();
        if trimmed.is_empty() {
            return;
        }

        self.identifier_like =
            !self.saw_value || self.identifier_like && is_identifier_like(trimmed);
        self.localized_key_like |= is_localized_key_like(trimmed);
        self.asset_path_like |= is_asset_path_like(trimmed);
        self.expression_like |= is_expression_like(column_name, trimmed);
        let list_entries = match separators {
            SemanticListSeparators::Detected => {
                schema_list_entries(column_name, value, preserve_empty_entries)
            }
            SemanticListSeparators::Exact => schema_list_entries_with_separator(
                column_name,
                value,
                default_separator,
                preserve_empty_entries,
            ),
        };
        if list_entries.len() > 1 {
            self.rows_with_lists += 1;
        }
        self.total_entries += list_entries.len();
        self.list_elements.observe(&list_entries);
        match separators {
            SemanticListSeparators::Detected => {
                for separator in detected_separators(trimmed) {
                    self.separators.insert(separator.to_owned());
                }
            }
            SemanticListSeparators::Exact => {
                self.separators.insert(default_separator.to_owned());
            }
        }
        self.saw_value = true;
    }

    pub(super) fn finish_list(
        self,
        row_type_name: &str,
        column_name: &str,
    ) -> Option<GameSystemListShape> {
        if self.rows_with_lists == 0
            || string_column_has_scalar_text_affinity(row_type_name, column_name)
        {
            return None;
        }

        let mut separators = self.separators.into_iter().collect::<Vec<_>>();
        separators.sort();
        Some(GameSystemListShape {
            separators,
            rows_with_lists: self.rows_with_lists,
            total_entries: self.total_entries,
            preserve_empty_entries: false,
            element_shape: self.list_elements.finish(row_type_name, column_name),
        })
    }

    pub(super) fn finish_semantic_list(
        self,
        row_type_name: &str,
        column_name: &str,
        default_separator: &str,
        separators: SemanticListSeparators,
        element_shape: Option<GameSystemListElementShape>,
        preserve_empty_entries: bool,
    ) -> GameSystemListShape {
        let mut separators = match separators {
            SemanticListSeparators::Detected => {
                let detected = self.separators.into_iter().collect::<Vec<_>>();
                if detected.is_empty() {
                    vec![default_separator.to_owned()]
                } else {
                    detected
                }
            }
            SemanticListSeparators::Exact => vec![default_separator.to_owned()],
        };
        separators.sort();
        GameSystemListShape {
            separators,
            rows_with_lists: self.rows_with_lists,
            total_entries: self.total_entries,
            preserve_empty_entries,
            element_shape: element_shape
                .or_else(|| self.list_elements.finish(row_type_name, column_name)),
        }
    }
}

#[derive(Debug, Default)]
pub(super) struct ListElementStats {
    total: usize,
    numeric: usize,
    boolean: usize,
    number_shape: NumberStats,
}

impl ListElementStats {
    pub(super) fn observe(&mut self, tokens: &[&str]) {
        for token in tokens {
            self.total += 1;
            if let Some(number) = string_token_as_number(token) {
                self.numeric += 1;
                self.number_shape.observe(number);
            }
            if parse_schema_bool(token).is_some() {
                self.boolean += 1;
            }
        }
    }

    pub(super) fn finish(
        self,
        row_type_name: &str,
        column_name: &str,
    ) -> Option<GameSystemListElementShape> {
        if self.total == 0 {
            return Some(GameSystemListElementShape::String);
        }
        if self.numeric == self.total {
            let number_shape = numeric_column_has_number_affinity(row_type_name, column_name)
                .unwrap_or_else(|| self.number_shape.finish_observed());
            return Some(GameSystemListElementShape::Number { number_shape });
        }
        if self.boolean == self.total {
            return Some(GameSystemListElementShape::Boolean);
        }
        Some(GameSystemListElementShape::String)
    }
}

pub(super) fn string_column_values(
    table: &GameSystemTable,
    column_index: usize,
) -> impl Iterator<Item = &str> + '_ {
    table
        .row_refs()
        .filter_map(move |row| row.cells().get(column_index)?.value().as_str())
}

pub(super) fn schema_list_entries<'a>(
    column_name: &str,
    value: &'a str,
    preserve_empty_entries: bool,
) -> Vec<&'a str> {
    let trimmed = trim_authored_schema_value(value);
    if trimmed.is_empty() {
        return Vec::new();
    }
    if is_expression_like(column_name, trimmed) {
        return vec![trimmed];
    }
    let entries = if preserve_empty_entries {
        schema_positional_list_split_entries(trimmed)
    } else {
        schema_list_split_entries(trimmed)
    };
    entries
        .into_iter()
        .map(str::trim)
        .filter(|entry| preserve_empty_entries || !entry.is_empty())
        .collect()
}

pub(super) fn schema_list_entries_with_separator<'a>(
    column_name: &str,
    value: &'a str,
    separator: &str,
    preserve_empty_entries: bool,
) -> Vec<&'a str> {
    let trimmed = trim_authored_schema_value(value);
    if trimmed.is_empty() {
        return Vec::new();
    }
    if is_expression_like(column_name, trimmed) {
        return vec![trimmed];
    }
    let mut separator_chars = separator.chars();
    let separator = separator_chars
        .next()
        .expect("semantic list separator is non-empty");
    debug_assert!(separator_chars.next().is_none());
    trimmed
        .split(separator)
        .map(str::trim)
        .filter(|entry| preserve_empty_entries || !entry.is_empty())
        .collect()
}

pub(super) fn schema_tokens(column_name: &str, value: &str) -> Vec<String> {
    let authored = trim_authored_schema_value(value);
    if is_expression_like(column_name, authored) {
        return reference_token_variants(authored);
    }

    let trimmed = normalize_reference_token(value);
    if trimmed.is_empty() {
        return Vec::new();
    }

    schema_reference_split_entries(trimmed)
        .into_iter()
        .flat_map(reference_token_variants)
        .filter(|token| !token.is_empty())
        .collect()
}

pub(super) fn schema_list_split_entries(value: &str) -> Vec<&str> {
    if value.contains(',') {
        value.split(',').collect::<Vec<_>>()
    } else if value.contains('|') || value.contains('&') || value.contains('(') {
        value.split(['|', '&', '(', ')']).collect::<Vec<_>>()
    } else if value.contains('+') && !value.starts_with('+') {
        value.split('+').collect::<Vec<_>>()
    } else {
        vec![value]
    }
}

pub(super) fn schema_positional_list_split_entries(value: &str) -> Vec<&str> {
    value.split([',', '|']).collect::<Vec<_>>()
}

pub(super) fn schema_reference_split_entries(value: &str) -> Vec<&str> {
    if value.contains(',') {
        value.split(',').collect::<Vec<_>>()
    } else if value.contains('|') || value.contains('&') || value.contains('(') {
        value.split(['|', '&', '(', ')']).collect::<Vec<_>>()
    } else if value.contains('+') && !value.starts_with('+') {
        value.split('+').collect::<Vec<_>>()
    } else {
        let whitespace_entries = value.split_whitespace().collect::<Vec<_>>();
        if whitespace_entries.len() > 1
            && whitespace_entries
                .iter()
                .all(|entry| is_probable_reference_token(entry))
        {
            whitespace_entries
        } else {
            vec![value]
        }
    }
}

pub(super) fn reference_token_variants(value: &str) -> Vec<String> {
    let token = normalize_reference_token(value);
    if token.is_empty() {
        return Vec::new();
    }

    let mut out = vec![token.to_owned()];
    if let Some((head, tail)) = token.split_once(':')
        && is_numeric_tail(tail)
    {
        let head = normalize_reference_token(head);
        if !head.is_empty() {
            out.push(head.to_owned());
        }
    }
    out.sort();
    out.dedup();
    out
}

pub(super) fn reference_key_candidates(token: &str) -> Vec<String> {
    let token = normalize_reference_token(token);
    let mut out = vec![canonical_key(token)];

    if let Some(instance) = generated_item_instance_id(token) {
        out.push(canonical_key(instance.base_item_id));
    }
    if let Some(base_item_id) = base_item_id_from_modifier_instance_id(token) {
        out.push(canonical_key(base_item_id));
    }
    if let Some((head, tail)) = token.split_once(':')
        && is_numeric_tail(tail)
    {
        out.push(canonical_key(head));
    }

    out.sort();
    out.dedup();
    out
}

pub(super) fn detected_separators(value: &str) -> Vec<&'static str> {
    let mut out = Vec::new();
    if value.contains(',') {
        out.push(",");
    }
    if value.contains('|') {
        out.push("|");
    }
    if value.contains('&') {
        out.push("&");
    }
    if value.contains('+') {
        out.push("+");
    }
    if value.contains(':') {
        out.push(":");
    }
    out
}

pub(super) fn trim_authored_schema_value(value: &str) -> &str {
    value.trim().trim_matches('"').trim()
}

pub(super) fn normalize_reference_token(value: &str) -> &str {
    trim_authored_schema_value(value)
        .trim()
        .trim_start_matches(['!', '+'])
        .trim()
}

pub(super) fn canonical_key(value: &str) -> String {
    normalize_reference_token(value).to_ascii_lowercase()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct GeneratedItemInstanceId<'a> {
    base_item_id: &'a str,
}

fn generated_item_instance_id(item_id: &str) -> Option<GeneratedItemInstanceId<'_>> {
    for (dash_index, _) in item_id.match_indices('-') {
        let base_item_id = item_id[..dash_index].trim();
        if base_item_id.is_empty() {
            continue;
        }

        let score_and_suffix = &item_id[dash_index + 1..];
        let digit_len = score_and_suffix
            .bytes()
            .take_while(u8::is_ascii_digit)
            .count();
        if digit_len == 0 {
            continue;
        }
        match score_and_suffix.as_bytes().get(digit_len) {
            Some(b'-') | None => {}
            Some(_) => continue,
        }
        if score_and_suffix[..digit_len].parse::<u32>().ok()? == 0 {
            continue;
        }
        return Some(GeneratedItemInstanceId { base_item_id });
    }
    None
}

fn base_item_id_from_modifier_instance_id(item_id: &str) -> Option<&str> {
    let (base_item_id, _) = item_id.split_once("-PerkID_")?;
    let base_item_id = base_item_id.trim();
    (!base_item_id.is_empty()).then_some(base_item_id)
}

pub(super) fn key_column_score(column_name: &str, row_key: bool, unique: bool) -> u8 {
    let lower = column_name.to_ascii_lowercase();
    let mut score = 0u8;
    if row_key {
        score = score.saturating_add(4);
    }
    if unique {
        score = score.saturating_add(2);
    }
    if lower == "id" || lower.ends_with("id") || lower.ends_with("_id") {
        score = score.saturating_add(4);
    }
    if lower.ends_with("name") || lower.ends_with("tag") || lower.ends_with("key") {
        score = score.saturating_add(2);
    }
    score
}

pub(super) fn columns_semantically_compatible(source_column: &str, target_column: &str) -> bool {
    let source_lower = source_column.to_ascii_lowercase();
    let target_lower = target_column.to_ascii_lowercase();

    if (source_lower.contains("category") || source_lower.contains("categories"))
        && !(target_lower.contains("category")
            || target_lower.contains("categories")
            || target_lower.contains("tag")
            || target_lower.contains("type")
            || target_lower.contains("family"))
    {
        return false;
    }

    let source_words = SemanticWords::parse(source_column);
    let target_words = SemanticWords::parse(target_column);
    if source_lower == "cooldownid" && target_lower == "abilityid" {
        return true;
    }
    if source_words.contains("ingredient") && target_words.contains("item") {
        return true;
    }
    if source_words.contains("ingredient") && target_words.contains("category") {
        return true;
    }
    source_words.matches_any_word(&target_words)
}

pub(super) fn is_empty_cell(value: &OwnedCellValue) -> bool {
    match value {
        OwnedCellValue::String(value) => is_empty_schema_string(value),
        OwnedCellValue::Number(_) | OwnedCellValue::Boolean(_) => false,
    }
}

pub(super) fn is_empty_cell_for_column(value: &OwnedCellValue, row_key: bool) -> bool {
    match (row_key, value) {
        (true, OwnedCellValue::String(value)) => is_blank_schema_string(value),
        _ => is_empty_cell(value),
    }
}

pub(super) fn is_blank_schema_string(value: &str) -> bool {
    value.trim().is_empty()
}

pub(super) fn is_empty_schema_string(value: &str) -> bool {
    let trimmed = value.trim();
    trimmed.is_empty()
        || trimmed.eq_ignore_ascii_case("none")
        || trimmed.eq_ignore_ascii_case("null")
}

pub(super) fn is_probable_reference_token(value: &str) -> bool {
    let token = normalize_reference_token(value);
    if token.len() < 2
        || token.len() > 160
        || token.contains(' ')
        || is_numeric_tail(token)
        || is_numeric_range_token(token)
    {
        return false;
    }
    token
        .bytes()
        .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-' | b'.' | b'/'))
}

pub(super) fn is_identifier_like(value: &str) -> bool {
    is_probable_reference_token(value) || value.split([':', ',']).all(is_probable_reference_token)
}

pub(super) fn is_localized_key_like(value: &str) -> bool {
    value.starts_with('@')
        || value.starts_with("ui_")
        || value.starts_with("igc_")
        || value.starts_with("achievement_")
}

pub(super) fn is_asset_path_like(value: &str) -> bool {
    (value.contains('/') || value.contains('\\'))
        && value
            .rsplit(['/', '\\'])
            .next()
            .is_some_and(|file| file.contains('.'))
}

pub(super) fn is_expression_like(column_name: &str, value: &str) -> bool {
    let lower = column_name.to_ascii_lowercase();
    lower.contains("formula")
        || lower.contains("expression")
        || lower.contains("condition")
        || value.contains('{')
        || value.contains('}')
        || value.contains(">=")
        || value.contains("<=")
        || value.contains("==")
        || value.contains("!=")
}

pub(super) fn is_numeric_tail(value: &str) -> bool {
    value.split(':').all(|part| {
        part.trim().trim_start_matches('+').parse::<f32>().is_ok() || part.trim().is_empty()
    })
}

pub(super) fn is_numeric_range_token(value: &str) -> bool {
    let Some((start, end)) = value.split_once('-') else {
        return false;
    };
    if end.contains('-') {
        return false;
    }
    let start = start.trim();
    let end = end.trim();
    !start.is_empty()
        && !end.is_empty()
        && start.parse::<f32>().is_ok()
        && end.parse::<f32>().is_ok()
}

pub(super) fn row_label(table: &GameSystemTable, row_index: usize) -> String {
    table
        .row_at_index(row_index)
        .and_then(|row| row.cells().first())
        .and_then(|cell| cell.value().as_str())
        .filter(|value| !value.trim().is_empty())
        .map(str::to_owned)
        .unwrap_or_else(|| format!("row#{row_index}"))
}

#[allow(dead_code)]
pub(super) fn _assert_error_send_sync(_: &GameSystemDataError) {}
