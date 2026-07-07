use super::*;

#[test]
fn loot_bucket_item_columns_stay_text_and_emit_native_column_projection() {
    let item_schema = GameSystemTableSchema {
        table_name: "ItemTooltipLayout".to_owned(),
        table_name_crc: 20,
        row_type_name: "ItemTooltipLayout".to_owned(),
        row_type_crc: 21,
        row_count: 1,
        sources: vec!["itemtooltiplayout/itemtooltiplayout.ron".to_owned()],
        columns: vec![GameSystemColumnSchema {
            name: "ItemID".to_owned(),
            crc: 22,
            declared_type: ColumnType::String,
            row_key: true,
            required: true,
            non_empty_rows: 1,
            empty_rows: 0,
            distinct_values: 1,
            value_shape: GameSystemColumnValueShape::String {
                identifier_like: true,
                localized_key_like: false,
                asset_path_like: false,
                expression_like: false,
                list: None,
                foreign_keys: Vec::new(),
            },
        }],
    };
    let schema = GameSystemTableSchema {
        table_name: "LootBuckets".to_owned(),
        table_name_crc: 10,
        row_type_name: "LootBucketData".to_owned(),
        row_type_crc: 11,
        row_count: 2,
        sources: vec!["lootbucketdata/lootbuckets.ron".to_owned()],
        columns: vec![
            GameSystemColumnSchema {
                name: "LootBucket1".to_owned(),
                crc: 12,
                declared_type: ColumnType::String,
                row_key: false,
                required: false,
                non_empty_rows: 1,
                empty_rows: 1,
                distinct_values: 1,
                value_shape: GameSystemColumnValueShape::String {
                    identifier_like: true,
                    localized_key_like: false,
                    asset_path_like: false,
                    expression_like: false,
                    list: None,
                    foreign_keys: Vec::new(),
                },
            },
            GameSystemColumnSchema {
                name: "FilterLootedItems1".to_owned(),
                crc: 13,
                declared_type: ColumnType::Boolean,
                row_key: false,
                required: true,
                non_empty_rows: 2,
                empty_rows: 0,
                distinct_values: 1,
                value_shape: GameSystemColumnValueShape::Boolean,
            },
            GameSystemColumnSchema {
                name: "LootBiasingDisabled1".to_owned(),
                crc: 14,
                declared_type: ColumnType::Boolean,
                row_key: false,
                required: true,
                non_empty_rows: 2,
                empty_rows: 0,
                distinct_values: 1,
                value_shape: GameSystemColumnValueShape::Boolean,
            },
            GameSystemColumnSchema {
                name: "Tags1".to_owned(),
                crc: 15,
                declared_type: ColumnType::String,
                row_key: false,
                required: false,
                non_empty_rows: 2,
                empty_rows: 0,
                distinct_values: 2,
                value_shape: GameSystemColumnValueShape::String {
                    identifier_like: false,
                    localized_key_like: false,
                    asset_path_like: false,
                    expression_like: false,
                    list: None,
                    foreign_keys: Vec::new(),
                },
            },
            GameSystemColumnSchema {
                name: "MatchOne1".to_owned(),
                crc: 16,
                declared_type: ColumnType::String,
                row_key: false,
                required: false,
                non_empty_rows: 2,
                empty_rows: 0,
                distinct_values: 1,
                value_shape: GameSystemColumnValueShape::String {
                    identifier_like: false,
                    localized_key_like: false,
                    asset_path_like: false,
                    expression_like: false,
                    list: None,
                    foreign_keys: Vec::new(),
                },
            },
            GameSystemColumnSchema {
                name: "Item1".to_owned(),
                crc: 17,
                declared_type: ColumnType::String,
                row_key: false,
                required: false,
                non_empty_rows: 2,
                empty_rows: 0,
                distinct_values: 2,
                value_shape: GameSystemColumnValueShape::String {
                    identifier_like: true,
                    localized_key_like: false,
                    asset_path_like: false,
                    expression_like: false,
                    list: None,
                    foreign_keys: vec![GameSystemForeignKeyCandidate {
                        target_table: "ItemTooltipLayout".to_owned(),
                        target_column: "ItemID".to_owned(),
                        checked_values: 2,
                        matched_values: 2,
                        missing_values: 0,
                        confidence: 1.0,
                    }],
                },
            },
            GameSystemColumnSchema {
                name: "Quantity1".to_owned(),
                crc: 18,
                declared_type: ColumnType::Number,
                row_key: false,
                required: true,
                non_empty_rows: 2,
                empty_rows: 0,
                distinct_values: 1,
                value_shape: GameSystemColumnValueShape::Number {
                    number_shape: GameSystemNumberShape::Float,
                },
            },
            GameSystemColumnSchema {
                name: "Odds1".to_owned(),
                crc: 19,
                declared_type: ColumnType::String,
                row_key: false,
                required: false,
                non_empty_rows: 1,
                empty_rows: 1,
                distinct_values: 1,
                value_shape: GameSystemColumnValueShape::Number {
                    number_shape: GameSystemNumberShape::Float,
                },
            },
        ],
    };

    let rendered = render_test_table_with_schemas(&schema, &[item_schema]);

    assert!(rust_source_contains(
        &rendered,
        "pub struct LootBucketColumn<'a>"
    ));
    assert!(rust_source_contains(
        &rendered,
        "pub struct LootBucketColumnTag<'a>"
    ));
    assert!(rust_source_contains(
        &rendered,
        "pub fn loot_bucket_columns(&self,)"
    ));
    assert!(rust_source_contains(
        &rendered,
        "pub quantity: core::ops::RangeInclusive<u16>,"
    ));
    assert!(rust_source_contains(
        &rendered,
        "loot_bucket_range_from_f32(row.quantity1()?)?"
    ));
    assert!(rust_source_contains(
        &rendered,
        "let header_row_index = loot_bucket_row_index(0)?;"
    ));
    assert!(rust_source_contains(&rendered, "pub struct Item1Column"));
    assert!(rust_source_contains(
        &rendered,
        "type Cell<'cell> = &'cell str;"
    ));
    assert!(!rendered.contains("Item1Column>()];"));
    assert!(!rendered.contains("gamedata::ForeignKey<'cell"));
}

#[test]
fn pvp_store_emits_reward_track_slot_projection() {
    let reward_items_schema = GameSystemTableSchema {
        table_name: "RewardTrackItems".to_owned(),
        table_name_crc: 20,
        row_type_name: "RewardTrackItemData".to_owned(),
        row_type_crc: 21,
        row_count: 1,
        sources: vec!["rewardtrackitems/rewardtrackitems.ron".to_owned()],
        columns: vec![GameSystemColumnSchema {
            name: "RewardId".to_owned(),
            crc: 22,
            declared_type: ColumnType::String,
            row_key: true,
            required: true,
            non_empty_rows: 1,
            empty_rows: 0,
            distinct_values: 1,
            value_shape: GameSystemColumnValueShape::String {
                identifier_like: true,
                localized_key_like: false,
                asset_path_like: false,
                expression_like: false,
                list: None,
                foreign_keys: Vec::new(),
            },
        }],
    };
    let string_shape = || GameSystemColumnValueShape::String {
        identifier_like: true,
        localized_key_like: false,
        asset_path_like: false,
        expression_like: false,
        list: None,
        foreign_keys: Vec::new(),
    };
    let schema = GameSystemTableSchema {
        table_name: "PvPStore".to_owned(),
        table_name_crc: 10,
        row_type_name: "PvPStoreData".to_owned(),
        row_type_crc: 11,
        row_count: 2,
        sources: vec!["pvpstore/pvpstore.ron".to_owned()],
        columns: vec![
            GameSystemColumnSchema {
                name: "RowPlaceholders".to_owned(),
                crc: 12,
                declared_type: ColumnType::String,
                row_key: true,
                required: true,
                non_empty_rows: 2,
                empty_rows: 0,
                distinct_values: 2,
                value_shape: string_shape(),
            },
            GameSystemColumnSchema {
                name: "Bucket1".to_owned(),
                crc: 13,
                declared_type: ColumnType::String,
                row_key: false,
                required: false,
                non_empty_rows: 1,
                empty_rows: 1,
                distinct_values: 1,
                value_shape: string_shape(),
            },
            GameSystemColumnSchema {
                name: "Tag1".to_owned(),
                crc: 14,
                declared_type: ColumnType::String,
                row_key: false,
                required: false,
                non_empty_rows: 1,
                empty_rows: 1,
                distinct_values: 1,
                value_shape: string_shape(),
            },
            GameSystemColumnSchema {
                name: "MatchOne1".to_owned(),
                crc: 15,
                declared_type: ColumnType::String,
                row_key: false,
                required: false,
                non_empty_rows: 1,
                empty_rows: 1,
                distinct_values: 1,
                value_shape: string_shape(),
            },
            GameSystemColumnSchema {
                name: "RewardId1".to_owned(),
                crc: 16,
                declared_type: ColumnType::String,
                row_key: false,
                required: false,
                non_empty_rows: 2,
                empty_rows: 0,
                distinct_values: 1,
                value_shape: GameSystemColumnValueShape::String {
                    identifier_like: true,
                    localized_key_like: false,
                    asset_path_like: false,
                    expression_like: false,
                    list: None,
                    foreign_keys: vec![GameSystemForeignKeyCandidate {
                        target_table: "RewardTrackItems".to_owned(),
                        target_column: "RewardId".to_owned(),
                        checked_values: 2,
                        matched_values: 2,
                        missing_values: 0,
                        confidence: 1.0,
                    }],
                },
            },
            GameSystemColumnSchema {
                name: "RandomWeights1".to_owned(),
                crc: 17,
                declared_type: ColumnType::Number,
                row_key: false,
                required: true,
                non_empty_rows: 2,
                empty_rows: 0,
                distinct_values: 1,
                value_shape: GameSystemColumnValueShape::Number {
                    number_shape: GameSystemNumberShape::NonNegativeInteger,
                },
            },
            GameSystemColumnSchema {
                name: "BudgetContribution1".to_owned(),
                crc: 18,
                declared_type: ColumnType::Number,
                row_key: false,
                required: true,
                non_empty_rows: 2,
                empty_rows: 0,
                distinct_values: 1,
                value_shape: GameSystemColumnValueShape::Number {
                    number_shape: GameSystemNumberShape::NonNegativeInteger,
                },
            },
            GameSystemColumnSchema {
                name: "Type1".to_owned(),
                crc: 19,
                declared_type: ColumnType::String,
                row_key: false,
                required: false,
                non_empty_rows: 1,
                empty_rows: 1,
                distinct_values: 1,
                value_shape: string_shape(),
            },
            GameSystemColumnSchema {
                name: "ExcludeTypeStage1".to_owned(),
                crc: 23,
                declared_type: ColumnType::String,
                row_key: false,
                required: false,
                non_empty_rows: 1,
                empty_rows: 1,
                distinct_values: 1,
                value_shape: string_shape(),
            },
            GameSystemColumnSchema {
                name: "ExcludeTypeShop1".to_owned(),
                crc: 24,
                declared_type: ColumnType::String,
                row_key: false,
                required: false,
                non_empty_rows: 1,
                empty_rows: 1,
                distinct_values: 1,
                value_shape: string_shape(),
            },
        ],
    };

    let rendered = render_test_table_with_schemas(&schema, &[reward_items_schema]);

    assert!(rust_source_contains(
        &rendered,
        "pub struct PvPStoreRewardTrackSlot<'a>"
    ));
    assert!(rust_source_contains(
        &rendered,
        "pub struct PvPStoreRewardTrackTagConstraint"
    ));
    assert!(rust_source_contains(
        &rendered,
        "pub fn reward_track_slots(&self,)"
    ));
    assert!(rust_source_contains(
        &rendered,
        "pub range: core::ops::RangeInclusive<u16>,"
    ));
    assert!(rust_source_contains(
        &rendered,
        "type Cell<'cell> = &'cell str;"
    ));
    assert!(rust_source_contains(
        &rendered,
        "pub reward_id: gamedata::ForeignKey<"
    ));
    assert!(rust_source_contains(
        &rendered,
        "tag_constraints: reward_track_store_tag_constraints(row.tag1()"
    ));
    assert!(rust_source_contains(
        &rendered,
        "reward_type: reward_track_store_crc_from_text(row.type1()"
    ));
}

#[test]
fn native_pvp_store_does_not_emit_strict_reward_track_projection() {
    let string_shape = || GameSystemColumnValueShape::String {
        identifier_like: false,
        localized_key_like: false,
        asset_path_like: false,
        expression_like: false,
        list: None,
        foreign_keys: Vec::new(),
    };
    let schema = GameSystemTableSchema {
        table_name: "PvPStore".to_owned(),
        table_name_crc: 10,
        row_type_name: "PvPStoreData".to_owned(),
        row_type_crc: 11,
        row_count: 2,
        sources: vec!["pvpstore/pvpstore.datasheet".to_owned()],
        columns: vec![
            GameSystemColumnSchema {
                name: "RowPlaceholders".to_owned(),
                crc: 12,
                declared_type: ColumnType::String,
                row_key: true,
                required: true,
                non_empty_rows: 2,
                empty_rows: 0,
                distinct_values: 2,
                value_shape: string_shape(),
            },
            GameSystemColumnSchema {
                name: "Bucket1".to_owned(),
                crc: 13,
                declared_type: ColumnType::String,
                row_key: false,
                required: true,
                non_empty_rows: 2,
                empty_rows: 0,
                distinct_values: 1,
                value_shape: string_shape(),
            },
            GameSystemColumnSchema {
                name: "RewardId1".to_owned(),
                crc: 16,
                declared_type: ColumnType::String,
                row_key: false,
                required: true,
                non_empty_rows: 2,
                empty_rows: 0,
                distinct_values: 2,
                value_shape: string_shape(),
            },
            GameSystemColumnSchema {
                name: "RandomWeights1".to_owned(),
                crc: 17,
                declared_type: ColumnType::Number,
                row_key: false,
                required: true,
                non_empty_rows: 2,
                empty_rows: 0,
                distinct_values: 1,
                value_shape: GameSystemColumnValueShape::Number {
                    number_shape: GameSystemNumberShape::Float,
                },
            },
            GameSystemColumnSchema {
                name: "BudgetContribution1".to_owned(),
                crc: 18,
                declared_type: ColumnType::Number,
                row_key: false,
                required: true,
                non_empty_rows: 2,
                empty_rows: 0,
                distinct_values: 1,
                value_shape: GameSystemColumnValueShape::Number {
                    number_shape: GameSystemNumberShape::Float,
                },
            },
        ],
    };

    let rendered = render_test_table_with_schemas(&schema, &[]);

    assert!(!rendered.contains("PvPStoreRewardTrackSlot"));
    assert!(!rendered.contains("reward_track_slots"));
}

#[test]
fn perk_bucket_perk_columns_stay_text_and_emit_companion_row_projection() {
    let perk_schema = GameSystemTableSchema {
        table_name: "ItemPerks".to_owned(),
        table_name_crc: 20,
        row_type_name: "ItemPerks".to_owned(),
        row_type_crc: 21,
        row_count: 1,
        sources: vec!["itemperks/itemperks.ron".to_owned()],
        columns: vec![GameSystemColumnSchema {
            name: "PerkID".to_owned(),
            crc: 22,
            declared_type: ColumnType::String,
            row_key: true,
            required: true,
            non_empty_rows: 1,
            empty_rows: 0,
            distinct_values: 1,
            value_shape: GameSystemColumnValueShape::String {
                identifier_like: true,
                localized_key_like: false,
                asset_path_like: false,
                expression_like: false,
                list: None,
                foreign_keys: Vec::new(),
            },
        }],
    };
    let schema = GameSystemTableSchema {
        table_name: "PerkBuckets".to_owned(),
        table_name_crc: 10,
        row_type_name: "PerkBucketData".to_owned(),
        row_type_crc: 11,
        row_count: 2,
        sources: vec!["perkbucketdata/perkbuckets.ron".to_owned()],
        columns: vec![
            GameSystemColumnSchema {
                name: "PerkBucketID".to_owned(),
                crc: 12,
                declared_type: ColumnType::String,
                row_key: true,
                required: true,
                non_empty_rows: 2,
                empty_rows: 0,
                distinct_values: 2,
                value_shape: GameSystemColumnValueShape::String {
                    identifier_like: true,
                    localized_key_like: false,
                    asset_path_like: false,
                    expression_like: false,
                    list: None,
                    foreign_keys: Vec::new(),
                },
            },
            GameSystemColumnSchema {
                name: "IgnoreExclusiveLabelWeights".to_owned(),
                crc: 13,
                declared_type: ColumnType::Boolean,
                row_key: false,
                required: false,
                non_empty_rows: 1,
                empty_rows: 1,
                distinct_values: 1,
                value_shape: GameSystemColumnValueShape::Boolean,
            },
            GameSystemColumnSchema {
                name: "DisablePerkBiasing".to_owned(),
                crc: 14,
                declared_type: ColumnType::Boolean,
                row_key: false,
                required: false,
                non_empty_rows: 1,
                empty_rows: 1,
                distinct_values: 1,
                value_shape: GameSystemColumnValueShape::Boolean,
            },
            GameSystemColumnSchema {
                name: "PerkType".to_owned(),
                crc: 15,
                declared_type: ColumnType::String,
                row_key: false,
                required: false,
                non_empty_rows: 1,
                empty_rows: 1,
                distinct_values: 1,
                value_shape: GameSystemColumnValueShape::String {
                    identifier_like: true,
                    localized_key_like: false,
                    asset_path_like: false,
                    expression_like: false,
                    list: None,
                    foreign_keys: Vec::new(),
                },
            },
            GameSystemColumnSchema {
                name: "PerkChance".to_owned(),
                crc: 16,
                declared_type: ColumnType::Number,
                row_key: false,
                required: false,
                non_empty_rows: 1,
                empty_rows: 1,
                distinct_values: 1,
                value_shape: GameSystemColumnValueShape::Number {
                    number_shape: GameSystemNumberShape::Float,
                },
            },
            GameSystemColumnSchema {
                name: "Perk1".to_owned(),
                crc: 17,
                declared_type: ColumnType::String,
                row_key: false,
                required: true,
                non_empty_rows: 2,
                empty_rows: 0,
                distinct_values: 2,
                value_shape: GameSystemColumnValueShape::String {
                    identifier_like: true,
                    localized_key_like: false,
                    asset_path_like: false,
                    expression_like: false,
                    list: None,
                    foreign_keys: vec![GameSystemForeignKeyCandidate {
                        target_table: "ItemPerks".to_owned(),
                        target_column: "PerkID".to_owned(),
                        checked_values: 1,
                        matched_values: 1,
                        missing_values: 0,
                        confidence: 1.0,
                    }],
                },
            },
            GameSystemColumnSchema {
                name: "Perk2".to_owned(),
                crc: 18,
                declared_type: ColumnType::String,
                row_key: false,
                required: false,
                non_empty_rows: 1,
                empty_rows: 1,
                distinct_values: 1,
                value_shape: GameSystemColumnValueShape::String {
                    identifier_like: true,
                    localized_key_like: false,
                    asset_path_like: false,
                    expression_like: false,
                    list: None,
                    foreign_keys: Vec::new(),
                },
            },
        ],
    };

    let rendered = render_test_table_with_schemas(&schema, &[perk_schema]);

    assert!(rendered.contains("pub struct PerkBucketProjection<'a>"));
    assert!(rendered.contains("pub struct PerkBucketProjectionEntry<'a>"));
    assert!(rendered.contains("pub enum PerkBucketProjectionEntryTarget<'a>"));
    assert!(rendered.contains("PerkBucket {"));
    assert!(rendered.contains("pub weight: f32"));
    assert!(rendered.contains(".parse::<f32>()"));
    assert!(!rendered.contains("pub weight: i32"));
    assert!(!rendered.contains(".parse::<i32>()"));
    assert!(rust_source_contains(
        &rendered,
        "pub fn perk_bucket_projections(&self,)"
    ));
    assert!(rendered.contains("weight_row_id.push_str(\"_Weights\");"));
    assert!(rendered.contains("value.strip_prefix(BUCKET_REFERENCE_PREFIX)"));
    assert!(rust_source_contains(
        &rendered,
        "let key: PerkBucketsKey = PerkBuckets::key(perk_bucket_id);"
    ));
    assert!(rendered.contains("perk_bucket_entry_weight"));
    assert!(rendered.contains("pub struct Perk1Column"));
    assert!(rendered.contains("type Cell<'cell> = &'cell str;"));
    assert!(!rendered.contains("Perk1Column>()];"));
    assert!(!rendered.contains("gamedata::ForeignKey<'cell"));
}
