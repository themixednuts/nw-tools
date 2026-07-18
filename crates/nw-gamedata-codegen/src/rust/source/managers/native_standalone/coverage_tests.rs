use std::collections::{BTreeMap, BTreeSet};

use nw_datasheet::{ColumnType, game_system::Crc32};

use super::super::rust_effective_native_manager_surface;
use super::*;
use crate::game_system_schema::{
    GameSystemColumnSchema, GameSystemColumnValueShape, GameSystemDataTablesSchemaReport,
    GameSystemTableSchema,
};
use crate::manager::validated_native_manager_specs;
use crate::manager_records::{ManagerSurface, manager_surfaces_from_managers};

#[test]
fn every_validated_native_surface_has_a_complete_standalone_rust_contract() {
    let surfaces = validated_surfaces();
    let schema_report = native_contract_schema(&surfaces);
    let mut covered = 0usize;

    for surface in &surfaces {
        let ManagerSurface::Native { manager, shape, .. } = surface else {
            continue;
        };
        let effective = rust_effective_native_manager_surface(manager, shape);
        let augmentation = augment_native_manager(&effective, shape, &schema_report)
            .unwrap_or_else(|error| {
                panic!(
                    "{} ({shape:?}) rejected its shipping-like schema: {error:#}",
                    manager.manager_name
                )
            });
        let product_only = effective.tables.is_empty() && !effective.products.is_empty();

        assert!(
            product_only || !augmentation.fields.is_empty() || !augmentation.methods.is_empty(),
            "{} ({shape:?}) emitted no fields or methods",
            manager.manager_name
        );

        match (
            augmentation.rows_type.as_str(),
            augmentation.rows_method.as_str(),
        ) {
            ("", "") => {}
            (row_type, rows_method) if !row_type.is_empty() && !rows_method.is_empty() => {
                assert!(
                    augmentation.methods.contains(&format!("fn {rows_method}(")),
                    "{} ({shape:?}) declares Rows<Row = {row_type}> through missing method `{rows_method}`",
                    manager.manager_name
                );
            }
            (row_type, rows_method) => panic!(
                "{} ({shape:?}) emitted an incomplete Rows contract: type={row_type:?}, method={rows_method:?}",
                manager.manager_name
            ),
        }

        let emitted = format!(
            "{}{}{}{}{}",
            augmentation.declarations,
            augmentation.fields,
            augmentation.field_values,
            augmentation.initializers,
            augmentation.methods
        );
        assert_clean_vocabulary(&manager.manager_name, shape, &emitted);
        covered += 1;
    }

    assert!(
        covered >= 69,
        "expected at least 69 validated native surfaces, covered {covered}"
    );
}

#[test]
fn shipping_like_contract_fixture_is_unique_and_strongly_typed() {
    let columns = native_contract_columns();
    let mut names = BTreeSet::new();

    for column in &columns {
        assert!(
            names.insert(column.name.to_ascii_lowercase()),
            "duplicate fixture column {}",
            column.name
        );
    }

    for name in ["IntID", "Level", "TerritoryID", "XPToLevel"] {
        let column = columns
            .iter()
            .find(|column| column.name.eq_ignore_ascii_case(name))
            .unwrap_or_else(|| panic!("missing numeric fixture column {name}"));
        assert_eq!(column.declared_type, ColumnType::Number, "{name}");
    }
    for name in ["Disabled", "IsEnabled", "KeepPerks", "RequiresPremium"] {
        let column = columns
            .iter()
            .find(|column| column.name.eq_ignore_ascii_case(name))
            .unwrap_or_else(|| panic!("missing boolean fixture column {name}"));
        assert_eq!(column.declared_type, ColumnType::Boolean, "{name}");
    }
}

fn validated_surfaces() -> Vec<ManagerSurface> {
    manager_surfaces_from_managers(&validated_native_manager_specs()).expect("manager surfaces")
}

fn native_contract_schema(surfaces: &[ManagerSurface]) -> GameSystemDataTablesSchemaReport {
    let mut tables = BTreeMap::new();
    for surface in surfaces {
        let ManagerSurface::Native { manager, shape, .. } = surface else {
            continue;
        };
        let effective = rust_effective_native_manager_surface(manager, shape);
        for table in effective.tables {
            tables
                .entry((table.table_name.clone(), table.row_type_name.clone()))
                .or_insert_with(|| {
                    schema_table(
                        &table.table_name,
                        &table.row_type_name,
                        native_contract_columns(),
                    )
                });
        }
    }
    GameSystemDataTablesSchemaReport {
        tables: tables.into_values().collect(),
        diagnostics: Vec::new(),
        type_affinities: Vec::new(),
    }
}

fn assert_clean_vocabulary(manager: &str, shape: &NativeManagerShape, emitted: &str) {
    for forbidden in [
        "native",
        "Native",
        "project",
        "Project",
        "bevy",
        "Bevy",
        "cooked",
        "Cooked",
        "generated_table",
        "generated-table",
        "generated table",
        "newworld_gamedata_tables",
    ] {
        assert!(
            !emitted.contains(forbidden),
            "{manager} ({shape:?}) emitted forbidden term {forbidden:?}"
        );
    }
}

fn schema_table(
    table_name: &str,
    row_type_name: &str,
    columns: Vec<GameSystemColumnSchema>,
) -> GameSystemTableSchema {
    GameSystemTableSchema {
        table_name: table_name.to_owned(),
        table_name_crc: Crc32::from_str_lower(table_name).value(),
        row_type_name: row_type_name.to_owned(),
        row_type_crc: Crc32::from_str_lower(row_type_name).value(),
        row_count: 1,
        sources: vec![format!("{table_name}.datasheet")],
        columns,
    }
}

fn native_contract_columns() -> Vec<GameSystemColumnSchema> {
    const STRING_COLUMNS: &[&str] = &[
        "UIPriority",
        "OutputQty",
        "AbilityID",
        "ActivitiesTaskID",
        "AffixID",
        "AfflictionID",
        "Attribute",
        "AudioGroup",
        "BalanceCategory",
        "BalanceTarget",
        "BaseVitalsID",
        "Buff1",
        "Buff2",
        "Buff3",
        "Buff4",
        "Buff5",
        "Buff6",
        "BuffBucketID",
        "BuffType1",
        "BuffType2",
        "BuffType3",
        "BuffType4",
        "BuffType5",
        "BuffType6",
        "Bucket1",
        "BucketID",
        "CampSkinID",
        "CardAndRowID",
        "CategoricalProgressionId",
        "Category",
        "CategoryText",
        "Chapter",
        "ChapterID",
        "ChapterRewardID",
        "ChapterType",
        "ChildCategoryList",
        "Connections",
        "ContainerTypeID",
        "ContributionID",
        "ConversionID",
        "CostumeChangeID",
        "CostumeChangeMesh",
        "CraftingCategory",
        "DamageID",
        "DarknessActivationSpec",
        "DarknessGroupSpec",
        "DarknessID",
        "DarknessLevels",
        "Description",
        "DifficultyScalingGroup",
        "DifficultyScalingTable",
        "DisplayName",
        "Dungeon",
        "Dungeon2",
        "Dungeon3",
        "DungeonBoss",
        "DungeonMiniBoss",
        "DungeonTileID",
        "DynamicDifficultyID",
        "Effect Name",
        "EffectCategories",
        "ElementalMutationID",
        "Entitlement",
        "EquipmentSetID",
        "EventTags",
        "FeatureID",
        "FootprintID",
        "FromItemID",
        "GameEvent",
        "GameEventIDRankAmazing",
        "GameEventIDRankBad",
        "GameEventIDRankGreat",
        "GameEventIDRankOkay",
        "GameModeIds",
        "GatherableID",
        "GatheringAction",
        "GatheringType",
        "HEAD_SLOT_Left",
        "HEAD_SLOT_Right",
        "CHEST_SLOT_Left",
        "CHEST_SLOT_Right",
        "HANDS_SLOT_Left",
        "HANDS_SLOT_Right",
        "LEGS_SLOT_Left",
        "LEGS_SLOT_Right",
        "FEET_SLOT_Left",
        "FEET_SLOT_Right",
        "Group",
        "Hub",
        "IconPath",
        "Ingredient1",
        "Ingredient2",
        "Ingredient3",
        "Ingredient4",
        "Ingredient5",
        "Ingredient6",
        "Ingredient7",
        "Instrument",
        "Item",
        "ItemID",
        "ItemIds",
        "JourneyTaskID",
        "LandscapeImage",
        "LootBucketID",
        "MountID",
        "Name",
        "Notes",
        "ObjectiveID",
        "Pages",
        "ParticleID",
        "PathReferenceQuickCourseID",
        "PointPoolID",
        "PoolCategory",
        "PortraitImage",
        "PrefabPath",
        "ProfileName",
        "ProfileType",
        "ProgressionPointID",
        "Promotion1",
        "Promotion2",
        "Promotion3",
        "PromotionMutationID",
        "QueueEndTime",
        "QueueGameModes",
        "QueueStartTime",
        "QuickCourseID",
        "RecipeID",
        "RequiredAchievementID",
        "RequiredCategoricalProgressionID",
        "RequiredProgressionPointID",
        "ReusableScoreboardTabId",
        "RewardID",
        "RewardID1",
        "Reward(s)",
        "RewardType",
        "RotationalQueueID",
        "RuleID",
        "SheetID",
        "Slot01",
        "Slot02",
        "Slot03",
        "Slot04",
        "Slot05",
        "SongID",
        "SquareImage",
        "StatusEffect_1",
        "StatusEffect_2",
        "StatusEffect_3",
        "StatusEffect_4",
        "StatusEffect_5",
        "StatusID",
        "StoreCategory",
        "StoreProductType",
        "StoreProductTypeList",
        "StructureFootprintID",
        "StructurePieceID",
        "SupportedRoomTypes",
        "TableType",
        "Tags",
        "TaskID",
        "TerritoryBonusCategory",
        "TerritoryName",
        "ThumbnailImage",
        "TimedRaceNodeTypeId",
        "ToItemID",
        "TrackedStatID",
        "TradeSkillType",
        "TreeID",
        "TreeRowPosition",
        "TypeDescription",
        "TypeID",
        "UniqueTagID",
        "UpgradeCardCategory",
        "UpgradeCardDescription",
        "UpgradeCardIcon",
        "UpgradeCardSprite",
        "UpgradeCardStat",
        "VariationAssetPaths",
        "VitalsID",
        "WeaponCategory",
        "WeaponName",
        "WhisperID",
        "WhisperVfxID",
        "WorldEncounterID",
        "Zone",
    ];
    const NUMBER_COLUMNS: &[&str] = &[
        "AbilityBaseDamageAdjustment",
        "AddTimeSeconds",
        "AffixStatAdjustment",
        "BuffPotency1",
        "BuffPotency2",
        "BuffPotency3",
        "BuffPotency4",
        "BuffPotency5",
        "BuffPotency6",
        "BuyCategoricalProgressionCost",
        "CategoryOrder",
        "ChapterIndex",
        "Constants",
        "ConsumableHealAdjustment",
        "CooldownAdjustment",
        "DetectionRadius",
        "DifficultyTier",
        "DurationAdjustment",
        "EntitlementIndex",
        "ExpectedParticipantCount",
        "FunctionCoefficient",
        "GameModeTimeSpan",
        "IncomingHealAdjustment",
        "Index",
        "InfluenceCost",
        "IntID",
        "Level",
        "Level Number",
        "LevelDisparity",
        "MaxEquippableGearScore",
        "MaxEvents",
        "MaximumInfluence",
        "MaxLevel",
        "Max Number",
        "MaxRoll",
        "MeshRenderZPosOffset",
        "MinDistance",
        "NodeTimeOverrideMultiplier",
        "PotencyAdjustment",
        "Priority",
        "Qty1",
        "Qty2",
        "Qty3",
        "Qty4",
        "Qty5",
        "Qty6",
        "Qty7",
        "Quantity",
        "QueueStartIndex",
        "RequiredCategoricalProgressionLevel",
        "RequiredCharacterLevel",
        "RequiredProgressionPointLevel",
        "RewardIndex",
        "Rotations",
        "ScalingFactorMax",
        "ScalingFactorMin",
        "SelfHealAdjustment",
        "SeasonsXP",
        "SortOrder",
        "StartingTimerSeconds",
        "SubRewardPerc1",
        "SubRewardPerc2",
        "Tempo",
        "TerritoryID",
        "TerritoryType",
        "TileSize",
        "TradeSkillRewardXP",
        "WeaponBaseDamageAdjustment",
        "Weight",
        "XPToLevel",
    ];
    const BOOLEAN_COLUMNS: &[&str] = &[
        "AccumulateTime",
        "Bought",
        "Disabled",
        "DoNotSpendPoint",
        "Enabled",
        "InContracts",
        "IsAbility",
        "IsEnabled",
        "IsEntitlement",
        "IsTimed",
        "KeepPerks",
        "MatchesPlayerSkeleton",
        "RequiresPremium",
        "RollOnPresent",
        "Sold",
        "UpdateEnabled",
        "UseLevelGS",
        "UseTimeOverride",
    ];

    let mut columns = BTreeMap::<String, GameSystemColumnSchema>::new();
    for (names, declared_type) in [
        (STRING_COLUMNS, ColumnType::String),
        (NUMBER_COLUMNS, ColumnType::Number),
        (BOOLEAN_COLUMNS, ColumnType::Boolean),
    ] {
        for name in names {
            let key = name.to_ascii_lowercase();
            assert!(
                columns
                    .insert(key, schema_column(name, declared_type))
                    .is_none(),
                "fixture column {name} was assigned more than one type"
            );
        }
    }
    columns.into_values().collect()
}

fn schema_column(name: &str, declared_type: ColumnType) -> GameSystemColumnSchema {
    let value_shape = match declared_type {
        ColumnType::Boolean => GameSystemColumnValueShape::Boolean,
        ColumnType::Number => GameSystemColumnValueShape::Number {
            number_shape: crate::game_system_schema::GameSystemNumberShape::Float,
        },
        ColumnType::String => GameSystemColumnValueShape::String {
            identifier_like: true,
            localized_key_like: false,
            asset_path_like: false,
            expression_like: false,
            list: None,
            foreign_keys: Vec::new(),
        },
    };
    GameSystemColumnSchema {
        name: name.to_owned(),
        crc: Crc32::from_str_lower(name).value(),
        declared_type,
        row_key: true,
        required: true,
        non_empty_rows: 1,
        empty_rows: 0,
        distinct_values: 1,
        value_shape,
    }
}
