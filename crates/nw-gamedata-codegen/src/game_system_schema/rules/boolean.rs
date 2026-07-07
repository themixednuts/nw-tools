use crate::game_system_schema::semantic::{ColumnSemanticProfile, semantic_words_match};

const BOOLEAN_NUMERIC_WORDS: &[&str] = &[
    "amount",
    "cap",
    "count",
    "limit",
    "max",
    "min",
    "num",
    "number",
    "quantity",
    "chance",
    "odds",
    "pct",
    "percent",
    "percentage",
    "probability",
    "rate",
];

const BOOLEAN_HINT_WORDS: &[&str] = &[
    "can",
    "cancel",
    "disable",
    "disabled",
    "enable",
    "enabled",
    "force",
    "has",
    "ignore",
    "ignored",
    "interrupt",
    "is",
    "no",
    "only",
    "unblockable",
    "use",
];

pub(in crate::game_system_schema) fn column_has_boolean_affinity(
    row_type_name: &str,
    column_name: &str,
) -> bool {
    let profile = ColumnSemanticProfile::new(row_type_name, column_name);
    let words = profile.words();
    if has_row_type_specific_boolean_affinity(row_type_name, words) {
        return true;
    }
    if words
        .iter()
        .any(|word| BOOLEAN_NUMERIC_WORDS.contains(&word.as_str()))
    {
        return false;
    }
    if words == ["damage", "guild", "and", "group"] {
        return true;
    }
    if has_native_item_boolean_affinity(words) {
        return true;
    }
    if words.iter().any(|word| word == "effect")
        && words.iter().any(|word| word == "only" || word == "when")
    {
        return true;
    }
    words
        .iter()
        .any(|word| BOOLEAN_HINT_WORDS.contains(&word.as_str()))
}

pub(in crate::game_system_schema) fn has_row_type_specific_boolean_affinity(
    row_type_name: &str,
    words: &[String],
) -> bool {
    row_type_name == "NotificationData" && semantic_words_match(words, &["track", "count"])
        || row_type_name == "ItemCurrencyConversionData"
            && (semantic_words_match(words, &["bought"]) || semantic_words_match(words, &["sold"]))
        || row_type_name == "GatherableData"
            && [
                &["require", "loot", "items"][..],
                &["restrict", "suspected", "bots"],
            ]
            .iter()
            .any(|expected| semantic_words_match(words, expected))
}

pub(in crate::game_system_schema) fn has_native_item_boolean_affinity(words: &[String]) -> bool {
    [
        &["accept", "snow"][..],
        &["allow", "gathering", "in", "game", "modes"],
        &["bind", "on", "equip"],
        &["bind", "on", "pickup"],
        &["confirm", "before", "use"],
        &["confirm", "destroy"],
        &["consume", "on", "use"],
        &["destroy", "on", "break"],
        &["exclude", "from", "game"],
        &["grant", "hwm", "bump"],
        &["grants", "hwm", "bump"],
        &["hide", "from", "reward", "open", "popup"],
        &["hide", "in", "loot", "ticker"],
        &["nonremovable"],
        &["not", "droppable"],
        &["override", "attribute", "scaling"],
        &["require", "los"],
        &["salvage", "resources"],
        &["should", "have", "static", "collision"],
        &["show", "on", "compass"],
        &["show", "on", "map"],
    ]
    .iter()
    .any(|expected| semantic_words_match(words, expected))
}
