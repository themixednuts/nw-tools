use super::{
    enums::{
        darkness_threshold_enum_shape, game_event_type_enum_shape,
        game_mode_participant_result_enum_shape, seasons_tracked_stat_reason_enum_shape,
        stat_multiplier_type_enum_shape, tradeskill_type_enum_shape, warboard_stat_type_enum_shape,
    },
    number::row_type_is_stat_modifier_source,
};
use crate::game_system_schema::{
    GameSystemEnumShape, GameSystemListAtomShape, GameSystemListElementShape,
    GameSystemNumberShape, GameSystemRangeBounds,
};

#[derive(Debug, Clone)]
pub(in crate::game_system_schema) struct SemanticListAffinity {
    pub(in crate::game_system_schema) separator: &'static str,
    pub(in crate::game_system_schema) separators: SemanticListSeparators,
    pub(in crate::game_system_schema) element_shape: Option<GameSystemListElementShape>,
    pub(in crate::game_system_schema) confidence: f64,
    pub(in crate::game_system_schema) row_key: SemanticListRowKey,
    pub(in crate::game_system_schema) preserve_empty_entries: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in crate::game_system_schema) enum SemanticListSeparators {
    Detected,
    Exact,
}

impl SemanticListSeparators {
    pub(in crate::game_system_schema) const fn exact(self) -> bool {
        matches!(self, Self::Exact)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in crate::game_system_schema) enum SemanticListRowKey {
    Keep,
    Demote,
}

pub(in crate::game_system_schema) fn column_has_list_affinity(
    row_type_name: &str,
    column_name: &str,
) -> Option<SemanticListAffinity> {
    row_type_specific_list_affinity(row_type_name, column_name)
}

pub(in crate::game_system_schema) fn row_type_specific_list_affinity(
    row_type_name: &str,
    column_name: &str,
) -> Option<SemanticListAffinity> {
    if row_type_is_stat_modifier_source(row_type_name) {
        if column_name == "AttributePlacingMods" {
            return Some(stat_modifier_float_list_affinity(","));
        }
        if column_name == "ItemClassWeightMods" {
            return Some(stat_modifier_pair_float_list_affinity(
                GameSystemListAtomShape::String,
            ));
        }
        if column_name == "StatMultipliers" {
            return Some(stat_modifier_pair_float_list_affinity(
                GameSystemListAtomShape::Enum {
                    enum_shape: stat_multiplier_type_enum_shape(),
                },
            ));
        }
        if matches!(
            column_name,
            "DMGVitalsCategory"
                | "ABSVitalsCategory"
                | "XPIncreases"
                | "StatBonuses"
                | "EffectDurationMods"
                | "EffectPotencyMods"
                | "StaminaCostReductions"
        ) {
            return Some(stat_modifier_pair_float_list_affinity(
                GameSystemListAtomShape::Crc32,
            ));
        }
    }
    if row_type_name == "MutationDifficultyStaticData"
        && matches!(
            column_name,
            "ReqItemsToEnter"
                | "InjectedLootTags"
                | "InjectedCreatureLoot"
                | "InjectedContainerLoot"
        )
    {
        return Some(SemanticListAffinity {
            separator: ",",
            separators: SemanticListSeparators::Detected,
            element_shape: Some(GameSystemListElementShape::Crc32),
            confidence: 0.95,
            row_key: SemanticListRowKey::Keep,
            preserve_empty_entries: false,
        });
    }
    if row_type_name == "DungeonTileStaticData" && column_name == "SupportedRoomTypes" {
        return Some(SemanticListAffinity {
            separator: ",",
            separators: SemanticListSeparators::Detected,
            element_shape: Some(GameSystemListElementShape::Crc32),
            confidence: 0.95,
            row_key: SemanticListRowKey::Keep,
            preserve_empty_entries: false,
        });
    }
    if row_type_name == "DungeonGrammarStaticData"
        && matches!(column_name, "ThemeTags" | "GrammarReplacements")
    {
        return Some(SemanticListAffinity {
            separator: ",",
            separators: SemanticListSeparators::Exact,
            element_shape: Some(GameSystemListElementShape::Crc32),
            confidence: 0.95,
            row_key: SemanticListRowKey::Keep,
            preserve_empty_entries: false,
        });
    }
    if row_type_name == "DungeonRoomStaticData"
        && dungeon_room_alias_tag_column_has_affinity(column_name)
    {
        return Some(SemanticListAffinity {
            separator: ",",
            separators: SemanticListSeparators::Exact,
            element_shape: Some(GameSystemListElementShape::Crc32),
            confidence: 0.95,
            row_key: SemanticListRowKey::Keep,
            preserve_empty_entries: false,
        });
    }
    if row_type_name == "SeasonsRewardsStats" {
        if matches!(
            column_name,
            "TargetID"
                | "LootTags"
                | "WeaponMasteryIds"
                | "CategoricalProgressionIds"
                | "CreatureTypes"
                | "GatherableIds"
                | "CraftingGroups"
                | "ResultItemID"
                | "GameEventIDs"
                | "IngredientItemID"
                | "RequiresStatusEffect"
        ) {
            return Some(SemanticListAffinity {
                separator: ",",
                separators: SemanticListSeparators::Detected,
                element_shape: Some(GameSystemListElementShape::Crc32),
                confidence: 0.95,
                row_key: SemanticListRowKey::Keep,
                preserve_empty_entries: false,
            });
        }
        if matches!(column_name, "ItemClass" | "ExcludeItemClass") {
            return Some(SemanticListAffinity {
                separator: "+",
                separators: SemanticListSeparators::Exact,
                element_shape: Some(GameSystemListElementShape::String),
                confidence: 0.95,
                row_key: SemanticListRowKey::Keep,
                preserve_empty_entries: false,
            });
        }
        return match column_name {
            "GameEventTypes" => Some(seasons_enum_list_affinity(game_event_type_enum_shape())),
            "Reasons" => Some(seasons_enum_list_affinity(
                seasons_tracked_stat_reason_enum_shape(),
            )),
            "Tradeskills" => Some(seasons_enum_list_affinity(tradeskill_type_enum_shape())),
            "GameModeResult" => Some(seasons_enum_list_affinity(
                game_mode_participant_result_enum_shape(),
            )),
            _ => None,
        };
    }
    if row_type_name == "SeasonsRewardData" {
        if column_name == "RequiresWorldTags" {
            return Some(SemanticListAffinity {
                separator: ",",
                separators: SemanticListSeparators::Detected,
                element_shape: Some(GameSystemListElementShape::Crc32),
                confidence: 0.95,
                row_key: SemanticListRowKey::Keep,
                preserve_empty_entries: false,
            });
        }
        if column_name == "EntitlementIds" {
            return Some(SemanticListAffinity {
                separator: ",",
                separators: SemanticListSeparators::Exact,
                element_shape: Some(GameSystemListElementShape::Pair {
                    separator: ':',
                    first: GameSystemListAtomShape::Crc32,
                    second: GameSystemListAtomShape::Number {
                        number_shape: GameSystemNumberShape::PositiveInteger,
                    },
                    default_second_source_token: Some("1".to_owned()),
                }),
                confidence: 0.95,
                row_key: SemanticListRowKey::Keep,
                preserve_empty_entries: false,
            });
        }
    }
    if row_type_name == "DynamicDifficultyStaticData"
        && column_name.eq_ignore_ascii_case("GameModeIds")
    {
        return Some(SemanticListAffinity {
            separator: ",",
            separators: SemanticListSeparators::Detected,
            element_shape: None,
            confidence: 0.85,
            row_key: SemanticListRowKey::Keep,
            preserve_empty_entries: false,
        });
    }
    if matches!(
        row_type_name,
        "ElementalMutationStaticData" | "PromotionMutationStaticData"
    ) && matches!(column_name, "TextColor" | "BackgroundColor")
    {
        return Some(float_list_affinity(","));
    }
    if row_type_name == "FishingCatchablesData" && column_name.eq_ignore_ascii_case("FishBehaviors")
    {
        return Some(SemanticListAffinity {
            separator: ",",
            separators: SemanticListSeparators::Detected,
            element_shape: Some(GameSystemListElementShape::Crc32),
            confidence: 0.95,
            row_key: SemanticListRowKey::Keep,
            preserve_empty_entries: false,
        });
    }
    if row_type_name == "PUGRewardData" && column_name.eq_ignore_ascii_case("ActivityTypes") {
        return Some(SemanticListAffinity {
            separator: ",",
            separators: SemanticListSeparators::Detected,
            element_shape: Some(GameSystemListElementShape::Crc32),
            confidence: 0.95,
            row_key: SemanticListRowKey::Keep,
            preserve_empty_entries: false,
        });
    }
    if row_type_name == "LeaderboardData"
        && matches!(
            column_name,
            "Rewards" | "ItemRewards" | "EntitlementRewards"
        )
    {
        return Some(SemanticListAffinity {
            separator: ",",
            separators: SemanticListSeparators::Exact,
            element_shape: Some(GameSystemListElementShape::Pair {
                separator: ':',
                first: GameSystemListAtomShape::Number {
                    number_shape: GameSystemNumberShape::NonNegativeInteger,
                },
                second: GameSystemListAtomShape::Crc32,
                default_second_source_token: None,
            }),
            confidence: 0.95,
            row_key: SemanticListRowKey::Keep,
            preserve_empty_entries: false,
        });
    }
    if row_type_name == "DarknessData" && column_name.eq_ignore_ascii_case("DarknessActivationSpec")
    {
        return Some(SemanticListAffinity {
            separator: ",",
            separators: SemanticListSeparators::Detected,
            element_shape: Some(GameSystemListElementShape::Range {
                bounds: GameSystemRangeBounds::Inclusive,
                number_shape: GameSystemNumberShape::NonNegativeInteger,
            }),
            confidence: 0.95,
            row_key: SemanticListRowKey::Keep,
            preserve_empty_entries: false,
        });
    }
    if row_type_name == "DarknessData" && column_name.eq_ignore_ascii_case("DarknessLevels") {
        return Some(SemanticListAffinity {
            separator: ",",
            separators: SemanticListSeparators::Detected,
            element_shape: Some(GameSystemListElementShape::Pair {
                separator: '-',
                first: GameSystemListAtomShape::Enum {
                    enum_shape: darkness_threshold_enum_shape(),
                },
                second: GameSystemListAtomShape::Number {
                    number_shape: GameSystemNumberShape::NonNegativeInteger,
                },
                default_second_source_token: None,
            }),
            confidence: 0.95,
            row_key: SemanticListRowKey::Keep,
            preserve_empty_entries: false,
        });
    }
    if row_type_name == "DarknessData" && column_name.eq_ignore_ascii_case("DarknessGroupSpec") {
        return Some(SemanticListAffinity {
            separator: ",",
            separators: SemanticListSeparators::Detected,
            element_shape: Some(GameSystemListElementShape::Pair {
                separator: '-',
                first: GameSystemListAtomShape::Number {
                    number_shape: GameSystemNumberShape::NonNegativeInteger,
                },
                second: GameSystemListAtomShape::Number {
                    number_shape: GameSystemNumberShape::NonNegativeInteger,
                },
                default_second_source_token: None,
            }),
            confidence: 0.95,
            row_key: SemanticListRowKey::Keep,
            preserve_empty_entries: false,
        });
    }
    if row_type_name == "StoryProgressData" && column_name.eq_ignore_ascii_case("AchievementIds") {
        return Some(SemanticListAffinity {
            separator: ",",
            separators: SemanticListSeparators::Detected,
            element_shape: Some(GameSystemListElementShape::Crc32),
            confidence: 0.95,
            row_key: SemanticListRowKey::Demote,
            preserve_empty_entries: false,
        });
    }
    if row_type_name == "MetaAchievementData"
        && matches!(
            column_name,
            "Predecessor MetaAchievementIds" | "AchievementsID"
        )
    {
        return Some(SemanticListAffinity {
            separator: ",",
            separators: SemanticListSeparators::Detected,
            element_shape: Some(GameSystemListElementShape::String),
            confidence: 0.95,
            row_key: SemanticListRowKey::Keep,
            preserve_empty_entries: false,
        });
    }
    if row_type_name == "PlayerTitleData" && column_name.eq_ignore_ascii_case("AchievementId") {
        return Some(SemanticListAffinity {
            separator: ",",
            separators: SemanticListSeparators::Detected,
            element_shape: Some(GameSystemListElementShape::String),
            confidence: 0.95,
            row_key: SemanticListRowKey::Keep,
            preserve_empty_entries: false,
        });
    }
    if row_type_name == "NotificationData" && column_name.eq_ignore_ascii_case("NumberFields") {
        return Some(SemanticListAffinity {
            separator: ",",
            separators: SemanticListSeparators::Detected,
            element_shape: Some(GameSystemListElementShape::Crc32),
            confidence: 0.95,
            row_key: SemanticListRowKey::Keep,
            preserve_empty_entries: false,
        });
    }
    if row_type_name == "ReusableScoreboardTabData" && column_name.eq_ignore_ascii_case("Columns") {
        return Some(SemanticListAffinity {
            separator: "|",
            separators: SemanticListSeparators::Exact,
            element_shape: Some(GameSystemListElementShape::String),
            confidence: 0.95,
            row_key: SemanticListRowKey::Keep,
            preserve_empty_entries: false,
        });
    }
    if row_type_name == "ReusableScoreboardTabData" && column_name.eq_ignore_ascii_case("Rows") {
        return Some(SemanticListAffinity {
            separator: "|",
            separators: SemanticListSeparators::Exact,
            element_shape: Some(GameSystemListElementShape::String),
            confidence: 0.95,
            row_key: SemanticListRowKey::Keep,
            preserve_empty_entries: false,
        });
    }
    if row_type_name == "ReusableScoreboardTabData"
        && column_name.eq_ignore_ascii_case("StatsToShowPlusSign")
    {
        return Some(SemanticListAffinity {
            separator: ",",
            separators: SemanticListSeparators::Detected,
            element_shape: Some(GameSystemListElementShape::Enum {
                enum_shape: warboard_stat_type_enum_shape(),
            }),
            confidence: 0.95,
            row_key: SemanticListRowKey::Keep,
            preserve_empty_entries: false,
        });
    }
    if row_type_name == "ReusableScoreboardTabData"
        && column_name.eq_ignore_ascii_case("StatsToShowAsBlank")
    {
        return Some(SemanticListAffinity {
            separator: ",",
            separators: SemanticListSeparators::Detected,
            element_shape: Some(GameSystemListElementShape::Enum {
                enum_shape: warboard_stat_type_enum_shape(),
            }),
            confidence: 0.95,
            row_key: SemanticListRowKey::Keep,
            preserve_empty_entries: false,
        });
    }

    None
}

fn dungeon_room_alias_tag_column_has_affinity(column_name: &str) -> bool {
    let Some(slot) = column_name.strip_prefix("AliasTag") else {
        return false;
    };
    matches!(slot, "1" | "2" | "3" | "4")
}

pub(in crate::game_system_schema) fn stat_modifier_float_list_affinity(
    separator: &'static str,
) -> SemanticListAffinity {
    let mut affinity = float_list_affinity(separator);
    affinity.separators = SemanticListSeparators::Exact;
    affinity
}

pub(in crate::game_system_schema) fn float_list_affinity(
    separator: &'static str,
) -> SemanticListAffinity {
    SemanticListAffinity {
        separator,
        separators: SemanticListSeparators::Detected,
        element_shape: Some(GameSystemListElementShape::Number {
            number_shape: GameSystemNumberShape::Float,
        }),
        confidence: 0.95,
        row_key: SemanticListRowKey::Keep,
        preserve_empty_entries: false,
    }
}

pub(in crate::game_system_schema) fn stat_modifier_pair_float_list_affinity(
    first: GameSystemListAtomShape,
) -> SemanticListAffinity {
    SemanticListAffinity {
        separator: "+",
        separators: SemanticListSeparators::Detected,
        element_shape: Some(GameSystemListElementShape::Pair {
            separator: '=',
            first,
            second: GameSystemListAtomShape::Number {
                number_shape: GameSystemNumberShape::Float,
            },
            default_second_source_token: None,
        }),
        confidence: 0.95,
        row_key: SemanticListRowKey::Keep,
        preserve_empty_entries: false,
    }
}

pub(in crate::game_system_schema) fn seasons_enum_list_affinity(
    enum_shape: GameSystemEnumShape,
) -> SemanticListAffinity {
    SemanticListAffinity {
        separator: ",",
        separators: SemanticListSeparators::Detected,
        element_shape: Some(GameSystemListElementShape::Enum { enum_shape }),
        confidence: 0.95,
        row_key: SemanticListRowKey::Keep,
        preserve_empty_entries: false,
    }
}
