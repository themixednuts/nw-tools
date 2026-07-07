use super::number::{is_stat_modifier_range_column, row_type_is_stat_modifier_source};
use crate::game_system_schema::{GameSystemNumberShape, GameSystemRangeBounds};

#[derive(Debug, Clone, Copy)]
pub(in crate::game_system_schema) struct RangeAffinity {
    pub(in crate::game_system_schema) bounds: GameSystemRangeBounds,
    pub(in crate::game_system_schema) number_shape: GameSystemNumberShape,
}

pub(in crate::game_system_schema) fn range_column_has_affinity(
    row_type_name: &str,
    column_name: &str,
) -> Option<RangeAffinity> {
    row_type_specific_range_affinity(row_type_name, column_name)
}

pub(in crate::game_system_schema) fn row_type_specific_range_affinity(
    row_type_name: &str,
    column_name: &str,
) -> Option<RangeAffinity> {
    if row_type_is_stat_modifier_source(row_type_name) && is_stat_modifier_range_column(column_name)
    {
        return Some(RangeAffinity {
            bounds: GameSystemRangeBounds::Inclusive,
            number_shape: GameSystemNumberShape::Float,
        });
    }

    if matches!(
        row_type_name,
        "GameModeData" | "MutationDifficultyStaticData"
    ) && column_name == "LootGSRangeOverride"
    {
        return Some(RangeAffinity {
            bounds: GameSystemRangeBounds::Inclusive,
            number_shape: GameSystemNumberShape::NonZeroU16,
        });
    }

    if row_type_name == "ExperienceData"
        && column_name.strip_prefix("GSLimitT").is_some_and(|suffix| {
            !suffix.is_empty() && suffix.chars().all(|ch| ch.is_ascii_digit())
        })
    {
        return Some(RangeAffinity {
            bounds: GameSystemRangeBounds::Inclusive,
            number_shape: GameSystemNumberShape::NonZeroU16,
        });
    }

    let inclusive = match row_type_name {
        "FishingCatchablesData" if matches!(column_name, "FishWeightRange" | "FishLengthRange") => {
            true
        }
        "FishingPolesData" if column_name == "CastDistanceRange" => true,
        "FishingHotspotsData" | "FishingWaterData"
            if matches!(
                column_name,
                "TimeToNibbleSecondsRange" | "TimeToBiteSecondsRange"
            ) =>
        {
            true
        }
        _ => false,
    };
    inclusive.then_some(RangeAffinity {
        bounds: GameSystemRangeBounds::Inclusive,
        number_shape: GameSystemNumberShape::Float,
    })
}
