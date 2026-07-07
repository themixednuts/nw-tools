use crate::game_system_schema::semantic::ColumnSemanticProfile;

const SCALAR_TEXT_SUFFIXES: &[&str] = &["asset", "group", "path", "ref", "reference", "table"];
const NON_NUMERIC_TEXT_SUFFIXES: &[&str] = &["id", "key", "string"];

pub(in crate::game_system_schema) fn string_column_has_scalar_text_affinity(
    row_type_name: &str,
    column_name: &str,
) -> bool {
    if row_type_name == "ExperienceData"
        && column_name
            .strip_prefix("GSBonus")
            .is_some_and(|suffix| !suffix.is_empty())
    {
        return false;
    }

    let profile = ColumnSemanticProfile::new(row_type_name, column_name);
    row_type_name == "DungeonGrammarStaticData" && column_name == "SeedGraph"
        || profile.last_word_is("hex")
        || row_type_name == "NotificationData" && column_name == "SecondaryText"
        || row_type_name == "DifficultyScalingData" && column_name == "MaxHealthMod"
        || row_type_name == "RotationalQueueData"
            && matches!(column_name, "QueueStartTime" | "QueueEndTime")
        || row_type_name == "SimpleTreeCategoryData" && column_name == "Icon Color Background"
        || SCALAR_TEXT_SUFFIXES
            .iter()
            .any(|suffix| profile.lower_column_name_ends_with(suffix))
        || SCALAR_TEXT_SUFFIXES
            .iter()
            .any(|word| profile.last_word_is(word))
        || (row_type_name == "DyeColorData" && matches!(column_name, "Color" | "SpecColor"))
}

pub(in crate::game_system_schema) fn string_column_blocks_numeric_affinity(
    row_type_name: &str,
    column_name: &str,
) -> bool {
    if string_column_has_scalar_text_affinity(row_type_name, column_name) {
        return true;
    }

    NON_NUMERIC_TEXT_SUFFIXES
        .iter()
        .any(|suffix| column_name_ends_with_semantic_suffix(column_name, suffix))
}

fn column_name_ends_with_semantic_suffix(column_name: &str, suffix: &str) -> bool {
    let lower = column_name.to_ascii_lowercase();
    if lower == suffix {
        return true;
    }
    if lower
        .strip_suffix(suffix)
        .is_some_and(|prefix| prefix.ends_with(['_', '-', ' ']))
    {
        return true;
    }

    match suffix {
        "id" => column_name.ends_with("Id") || column_name.ends_with("ID"),
        "key" => column_name.ends_with("Key") || column_name.ends_with("KEY"),
        "string" => column_name.ends_with("String") || column_name.ends_with("STRING"),
        _ => false,
    }
}
