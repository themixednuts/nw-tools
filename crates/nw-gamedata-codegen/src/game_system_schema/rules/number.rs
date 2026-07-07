use crate::game_system_schema::{GameSystemNumberShape, semantic::ColumnSemanticProfile};

pub(in crate::game_system_schema) fn numeric_column_has_float_affinity(
    row_type_name: &str,
    column_name: &str,
) -> bool {
    matches!(
        numeric_column_has_number_affinity(row_type_name, column_name),
        Some(GameSystemNumberShape::Float)
    )
}

pub(in crate::game_system_schema) fn numeric_column_has_number_affinity(
    row_type_name: &str,
    column_name: &str,
) -> Option<GameSystemNumberShape> {
    let profile = ColumnSemanticProfile::new(row_type_name, column_name);
    if profile.has_word("hex") {
        return None;
    }
    if let Some(number_shape) = row_type_specific_number_affinity(&profile) {
        return Some(number_shape);
    }
    profile
        .has_any_word_matching(FLOAT_NUMBER_AFFINITY_WORDS)
        .then_some(GameSystemNumberShape::Float)
}

pub(in crate::game_system_schema) fn numeric_column_allows_authored_suffix(
    row_type_name: &str,
    column_name: &str,
) -> bool {
    let profile = ColumnSemanticProfile::new(row_type_name, column_name);
    matches!(row_type_name, "MasterItemDefinitions")
        && profile.words_match(&["max", "stack", "size"])
}

const FLOAT_NUMBER_AFFINITY_WORDS: &[&str] = &[
    "add",
    "absorption",
    "chance",
    "charge",
    "coef",
    "coefficient",
    "cooldown",
    "decay",
    "delay",
    "distance",
    "duration",
    "frac",
    "fraction",
    "mod",
    "mult",
    "multiplier",
    "modifier",
    "odds",
    "penetration",
    "perc",
    "potency",
    "pct",
    "percent",
    "percentage",
    "radius",
    "rate",
    "ratio",
    "reduction",
    "scale",
    "scaling",
    "scalar",
    "stun",
    "threat",
    "time",
];

pub(in crate::game_system_schema) fn row_type_is_stat_modifier_source(row_type_name: &str) -> bool {
    matches!(
        row_type_name,
        "ConsumableItemDefinitions"
            | "BlueprintItemDefinitions"
            | "AffixStatData"
            | "ArmorItemDefinitions"
            | "WeaponItemDefinitions"
            | "StatusEffectData"
            | "VitalsBaseData"
            | "FishingPolesData"
    )
}

pub(in crate::game_system_schema) fn is_stat_modifier_range_column(column_name: &str) -> bool {
    let profile = ColumnSemanticProfile::new("", column_name);
    column_name
        .strip_prefix("MOD")
        .is_some_and(|suffix| !suffix.is_empty())
        || profile.first_word_matches("mod")
}

pub(in crate::game_system_schema) fn stat_modifier_grouped_prefix_has_float_affinity(
    column_name: &str,
    words: &[String],
) -> bool {
    const GROUP_PREFIXES: &[&str] = &["DMG", "DEF", "ABS", "WKN", "BLA", "ABA", "RES", "AFA"];

    GROUP_PREFIXES.iter().any(|prefix| {
        column_name
            .strip_prefix(prefix)
            .is_some_and(|suffix| !suffix.is_empty())
    }) || words
        .first()
        .is_some_and(|word| matches!(word.as_str(), "physical" | "elemental"))
}

pub(in crate::game_system_schema) fn row_type_specific_number_affinity(
    profile: &ColumnSemanticProfile<'_>,
) -> Option<GameSystemNumberShape> {
    let row_type_name = profile.row_type_name;
    let column_name = profile.column_name;
    let words = profile.words();

    if row_type_is_stat_modifier_source(row_type_name)
        && stat_modifier_scalar_has_float_affinity(column_name)
    {
        return Some(GameSystemNumberShape::Float);
    }

    if row_type_is_stat_modifier_source(row_type_name)
        && stat_modifier_scalar_has_integer_affinity(column_name)
    {
        return Some(GameSystemNumberShape::Integer);
    }

    if row_type_is_stat_modifier_source(row_type_name)
        && stat_modifier_grouped_prefix_has_float_affinity(column_name, words)
    {
        return Some(GameSystemNumberShape::Float);
    }

    match row_type_name {
        "MasterItemDefinitions" if profile.words_match(&["max", "stack", "size"]) => {
            Some(GameSystemNumberShape::NonNegativeInteger)
        }
        "EncumbranceData" if profile.words_match(&["full", "when", "encumbered"]) => {
            Some(GameSystemNumberShape::Float)
        }
        "DyeColorData"
            if matches!(
                column_name,
                "ColorAmount" | "ColorOverride" | "SpecAmount" | "MaskGlossShift"
            ) =>
        {
            Some(GameSystemNumberShape::Float)
        }
        "ProgressionPoolData" | "ProgressionPools"
            if profile.words_match(&["initial", "points"]) =>
        {
            Some(GameSystemNumberShape::NonNegativeInteger)
        }
        "ProgressionPointData" if profile.words_match(&["character", "level"]) => {
            Some(GameSystemNumberShape::NonNegativeInteger)
        }
        "ItemCurrencyConversionData" if profile.words_match(&["item", "qty"]) => {
            Some(GameSystemNumberShape::PositiveInteger)
        }
        "ItemCurrencyConversionData"
            if profile.words_match_any(&[
                &["buy", "cooldown", "seconds"],
                &["buy", "progression", "cost"],
                &["sell", "azoth", "cost"],
            ]) =>
        {
            Some(GameSystemNumberShape::NonNegativeInteger)
        }
        "VariationData" if profile.words_match(&["weight"]) => Some(GameSystemNumberShape::Float),
        "SeasonsRewardsStats" if column_name == "MinWeight" => Some(GameSystemNumberShape::Float),
        "SeasonsRewardsStats"
            if matches!(
                column_name,
                "Precision"
                    | "ItemRarity"
                    | "MaxParticipantsOnDungeonClear"
                    | "ItemClassEquippedMinCount"
            ) =>
        {
            Some(GameSystemNumberShape::U8)
        }
        "SeasonsRewardsStats" if matches!(column_name, "Level" | "ItemGS" | "ChapterIndex") => {
            Some(GameSystemNumberShape::U16)
        }
        "SeasonsRewardsStats" if matches!(column_name, "ItemTier" | "MutationLevel" | "Score") => {
            Some(GameSystemNumberShape::NonNegativeInteger)
        }
        "MutationPerksStaticData"
            if profile.words_match(&["injected", "perk", "bucket", "weight"]) =>
        {
            Some(GameSystemNumberShape::Float)
        }
        "RewardModifierData" if profile.words_match(&["progression", "currency", "amount"]) => {
            Some(GameSystemNumberShape::Float)
        }
        "ExpansionData"
            if profile.words_match_any(&[
                &["max", "level"],
                &["max", "craft", "gs"],
                &["max", "equip", "gs"],
                &["max", "tradeskill", "level"],
            ]) =>
        {
            Some(GameSystemNumberShape::NonZeroU16)
        }
        "ExperienceData"
            if column_name
                .strip_prefix("GSBonus")
                .is_some_and(|suffix| !suffix.is_empty()) =>
        {
            Some(GameSystemNumberShape::NonNegativeInteger)
        }
        "BuffBucketData" if profile.words_match(&["buff", "potency"]) => {
            Some(GameSystemNumberShape::Float)
        }
        "MutationDifficultyStaticData"
            if profile.words_match(&["base", "curse", "damage", "mod"]) =>
        {
            Some(GameSystemNumberShape::Float)
        }
        "MutationDifficultyStaticData"
            if profile.words_match_any(&[
                &["mutation", "difficulty"],
                &["difficulty", "tier"],
                &["matchmaking", "min", "tanks"],
                &["matchmaking", "max", "tanks"],
                &["matchmaking", "min", "healers"],
                &["matchmaking", "max", "healers"],
                &["matchmaking", "min", "dps"],
                &["matchmaking", "max", "dps"],
            ]) =>
        {
            Some(GameSystemNumberShape::NonZeroU8)
        }
        "MutationDifficultyStaticData"
            if profile.words_match(&["recommended", "gear", "score"]) =>
        {
            Some(GameSystemNumberShape::NonZeroU16)
        }
        "MutationDifficultyStaticData"
            if profile.words_match_any(&[
                &["injected", "creature", "loot", "roll"],
                &["injected", "container", "loot", "roll"],
            ]) =>
        {
            Some(GameSystemNumberShape::NonNegativeInteger)
        }
        "DungeonTileStaticData" if profile.words_match(&["rotations"]) => {
            Some(GameSystemNumberShape::U8)
        }
        "DungeonTileStaticData" if profile.words_match(&["tile", "size"]) => {
            Some(GameSystemNumberShape::NonZeroU8)
        }
        "DungeonTileStaticData" if profile.words_match(&["weight"]) => {
            Some(GameSystemNumberShape::NonNegativeInteger)
        }
        "DungeonGrammarStaticData" if matches!(column_name, "MinDepth" | "MaxDepth") => {
            Some(GameSystemNumberShape::U8)
        }
        "DungeonGrammarStaticData" if profile.words_match(&["weight"]) => {
            Some(GameSystemNumberShape::NonNegativeInteger)
        }
        "DungeonRoomStaticData" if profile.words_match(&["room", "passthrough", "cost"]) => {
            Some(GameSystemNumberShape::Float)
        }
        "TerritoryUpkeepDefinition"
            if profile.words_match(&["earnings", "distribution", "tid"]) =>
        {
            Some(GameSystemNumberShape::Float)
        }
        "DyeItemDefinitions" | "MountDyeItemDefinitions"
            if profile.words_match(&["color", "index"]) =>
        {
            Some(GameSystemNumberShape::PositiveInteger)
        }
        _ => None,
    }
}

pub(in crate::game_system_schema) fn stat_modifier_scalar_has_float_affinity(
    column_name: &str,
) -> bool {
    matches!(
        column_name,
        "Health"
            | "HealthMin"
            | "HealthMinPercent"
            | "HealthModifierPercent"
            | "HealthModifierBasePercent"
            | "HealthModifierDamageBased"
            | "Stamina"
            | "Food"
            | "Drink"
            | "Mana"
            | "ManaModifierDamageBased"
            | "Encumbrance"
            | "EncumbrancePerGS"
            | "EquipLoad"
    )
}

pub(in crate::game_system_schema) fn stat_modifier_scalar_has_integer_affinity(
    column_name: &str,
) -> bool {
    matches!(column_name, "CoreTempMod" | "TempMod" | "InventorySlots")
}
