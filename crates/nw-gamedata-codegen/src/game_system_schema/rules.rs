mod boolean;
mod catalog;
mod color;
pub(in crate::game_system_schema) mod enums;
mod list;
mod number;
mod range;
mod text;

use crate::game_system_schema::semantic::ColumnSemanticProfile;

use catalog::{ColumnRule, matching_rule_value};

pub(super) use boolean::column_has_boolean_affinity;
pub(super) use color::color_column_has_affinity;
pub(super) use enums::scalar_enum_column_affinity;
pub(super) use list::{
    SemanticListRowKey, SemanticListSeparators, column_has_list_affinity,
    row_type_specific_list_affinity,
};
pub(super) use number::{
    numeric_column_allows_authored_suffix, numeric_column_has_float_affinity,
    numeric_column_has_number_affinity,
};
pub(super) use range::range_column_has_affinity;
pub(super) use text::{
    string_column_blocks_numeric_affinity, string_column_has_scalar_text_affinity,
};

pub(super) fn scalar_crc32_column_has_affinity(row_type_name: &str, column_name: &str) -> bool {
    matching_rule_value(
        CRC32_SCALAR_COLUMN_RULES,
        &ColumnSemanticProfile::new(row_type_name, column_name),
    )
    .is_some()
}

const CRC32_SCALAR_COLUMN_RULES: &[ColumnRule<()>] = &[
    ColumnRule::any_of(
        "CraftingRecipeData",
        &[
            "CraftingCategory",
            "ItemID",
            "Ingredient1",
            "Ingredient2",
            "Ingredient3",
            "Ingredient4",
            "Ingredient5",
            "Ingredient6",
            "Ingredient7",
        ],
        (),
    ),
    ColumnRule::exact("ExpansionData", "EntitlementId", ()),
    ColumnRule::exact("DungeonGrammarStaticData", "FeatureId", ()),
    ColumnRule::any_of(
        "DungeonRoomStaticData",
        &["FeatureId", "RoomType", "StartingState"],
        (),
    ),
    ColumnRule::predicate(
        "DungeonRoomStaticData",
        dungeon_room_alias_category_column_has_affinity,
        (),
    ),
    ColumnRule::exact("StoryProgressData", "ActivityTaskName", ()),
    ColumnRule::any_of(
        "LeaderboardRewardsData",
        &["EntitlementRewards", "LeaderboardRewardIdNoRotation"],
        (),
    ),
    ColumnRule::any_of(
        "MutationDifficultyStaticData",
        &[
            "HealthIncreaseMod",
            "DamageIncreaseMod",
            "CompletionEvent1",
            "CompletionEvent2",
            "CompletionEvent3",
        ],
        (),
    ),
    ColumnRule::any_of(
        "SeasonsRewardsStats",
        &["TrackingType", "CraftingType", "RequiredWorldTag"],
        (),
    ),
    ColumnRule::any_of(
        "SeasonsRewardData",
        &["ItemId", "DisplayItemId", "LimitingEntitlementId"],
        (),
    ),
];

fn dungeon_room_alias_category_column_has_affinity(profile: &ColumnSemanticProfile<'_>) -> bool {
    let Some(slot) = profile.column_name.strip_prefix("AliasCategory") else {
        return false;
    };
    matches!(slot, "1" | "2" | "3" | "4")
}
