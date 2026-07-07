use super::*;
use crate::manager::GeneratedManagerSurface;
use crate::symbols::{RustIdentifier, RustPath, RustTypePath};

fn find_manager<'a>(managers: &'a [NativeManagerSpec], rust_type: &str) -> &'a NativeManagerSpec {
    managers
        .iter()
        .find(|manager| manager.rust_type().as_str() == rust_type)
        .unwrap_or_else(|| panic!("missing manager spec `{rust_type}`"))
}

fn assert_table_manager(manager: &NativeManagerSpec, table_name: &str) {
    assert!(matches!(
        manager.shape(),
        Some(NativeManagerShape::RequirementsOnly)
    ));
    let [NativeManagerInput::Table(table)] = manager.inputs() else {
        panic!(
            "expected one table input for `{}`",
            manager.rust_type().as_str()
        );
    };
    assert_eq!(table.table_name().as_str(), table_name);
}

fn assert_generated_table_manager(manager: &NativeManagerSpec, table_name: &str) {
    assert!(matches!(
        manager.shape(),
        Some(shape) if shape.exposes_native_api()
    ));
    let [NativeManagerInput::Table(table)] = manager.inputs() else {
        panic!(
            "expected one table input for `{}`",
            manager.rust_type().as_str()
        );
    };
    assert_eq!(table.table_name().as_str(), table_name);
}

fn assert_one_table_crc_projection<'a>(
    manager: &'a NativeManagerSpec,
    table_name: &str,
    key_column: &str,
    entries_field: &str,
    ids_method: &str,
    rows_method: &str,
) -> &'a NativeOneTableCrcKeyProjectionManager {
    assert_manager_has_table(manager, table_name);
    assert_native_api_manager_surface(manager);
    let Some(NativeManagerShape::OneTableCrcKeyProjection(shape)) = manager.shape() else {
        panic!(
            "expected one-table CRC-key projection shape for `{}`",
            manager.rust_type().as_str()
        );
    };
    assert_eq!(shape.table_name().as_str(), table_name);
    assert_eq!(shape.key_column().as_str(), key_column);
    assert_eq!(shape.entries_field().as_str(), entries_field);
    assert!(shape.skip_empty_key());
    assert!(!shape.reject_zero_crc());
    assert_eq!(
        shape.duplicate_key_policy(),
        NativeDuplicateKeyPolicy::FirstWins
    );
    assert_eq!(
        shape.source_row_field().map(RustIdentifier::as_str),
        Some("source_row")
    );
    assert_eq!(
        shape.source_row_method().map(RustIdentifier::as_str),
        Some("source_row")
    );
    assert_eq!(
        shape.ids_method().map(RustIdentifier::as_str),
        Some(ids_method)
    );
    assert_eq!(
        shape.rows_method().map(RustIdentifier::as_str),
        Some(rows_method)
    );
    shape
}

fn assert_table_family_crc_projection<'a>(
    manager: &'a NativeManagerSpec,
    sample_table_name: &str,
    key_column: &str,
    entries_field: &str,
    ids_method: &str,
    rows_method: &str,
) -> &'a NativeTableFamilyCrcKeyProjectionManager {
    assert_manager_has_table(manager, sample_table_name);
    assert_native_api_manager_surface(manager);
    let Some(NativeManagerShape::TableFamilyCrcKeyProjection(shape)) = manager.shape() else {
        panic!(
            "expected table-family CRC-key projection shape for `{}`",
            manager.rust_type().as_str()
        );
    };
    assert!(
        shape
            .tables()
            .iter()
            .any(|table| table.table_name().as_str() == sample_table_name)
    );
    assert_eq!(shape.key_column().as_str(), key_column);
    assert_eq!(shape.entries_field().as_str(), entries_field);
    assert!(shape.skip_empty_key());
    assert!(!shape.reject_zero_crc());
    assert_eq!(
        shape.duplicate_key_policy(),
        NativeDuplicateKeyPolicy::FirstWins
    );
    assert_eq!(
        shape.source_row_field().map(RustIdentifier::as_str),
        Some("source_row")
    );
    assert_eq!(
        shape.source_row_method().map(RustIdentifier::as_str),
        Some("source_row")
    );
    assert_eq!(
        shape.ids_method().map(RustIdentifier::as_str),
        Some(ids_method)
    );
    assert_eq!(
        shape.rows_method().map(RustIdentifier::as_str),
        Some(rows_method)
    );
    shape
}

fn assert_manager_has_table(manager: &NativeManagerSpec, table_name: &str) {
    assert!(
            manager.inputs().iter().any(|input| {
                matches!(input, NativeManagerInput::Table(table) if table.table_name().as_str() == table_name)
            }),
            "expected `{}` to include table input `{table_name}`",
            manager.rust_type().as_str()
        );
}

fn assert_product_inputs_with_format(
    manager: &NativeManagerSpec,
    format: crate::manager::NativeManagerProductFormat,
    products: &[(&str, &str)],
) {
    assert_eq!(manager.inputs().len(), products.len());
    for (input, (path, rust_type)) in manager.inputs().iter().zip(products.iter().copied()) {
        let NativeManagerInput::Product(product) = input else {
            panic!(
                "expected product input for `{}`",
                manager.rust_type().as_str()
            );
        };
        assert_eq!(product.format(), format);
        assert_eq!(product.asset_path().as_str(), path);
        assert_eq!(product.rust_type().as_str(), rust_type);
    }
}

fn assert_product_asset_resource_manager(
    manager: &NativeManagerSpec,
    products: &[(&str, &str, &str, &str, &str, &str)],
) {
    assert_product_asset_resource_manager_with_format(
        manager,
        crate::manager::NativeManagerProductFormat::ObjectStream,
        products,
    );
}

fn assert_product_asset_resource_manager_with_format(
    manager: &NativeManagerSpec,
    format: crate::manager::NativeManagerProductFormat,
    products: &[(&str, &str, &str, &str, &str, &str)],
) {
    assert_product_inputs_with_format(
        manager,
        format,
        &products
            .iter()
            .map(|(path, rust_type, _, _, _, _)| (*path, *rust_type))
            .collect::<Vec<_>>(),
    );
    let Some(NativeManagerShape::ProductAssetResource(shape)) = manager.shape() else {
        panic!(
            "expected product asset resource shape for `{}`",
            manager.rust_type().as_str()
        );
    };
    assert_product_asset_shape(shape, products);
}

fn assert_product_asset_shape(
    shape: &NativeProductAssetResourceManager,
    products: &[(&str, &str, &str, &str, &str, &str)],
) {
    assert_eq!(shape.constructor().as_str(), "new");
    assert_eq!(shape.products().len(), products.len());
    for (resource, (_, rust_type, value_type, handle_getter, asset_getter, manager_getter)) in
        shape.products().iter().zip(products.iter().copied())
    {
        assert_eq!(resource.product_type().as_str(), rust_type);
        assert_eq!(resource.value_type().as_str(), value_type);
        assert_eq!(resource.handle_getter().as_str(), handle_getter);
        assert_eq!(resource.asset_getter().as_str(), asset_getter);
        assert_eq!(resource.manager_getter().as_str(), manager_getter);
    }
}

fn assert_product_asset_resource(
    resource: &NativeProductAssetResource,
    product_type: &str,
    value_type: &str,
    handle_getter: &str,
    asset_getter: &str,
    manager_getter: &str,
) {
    assert_eq!(resource.product_type().as_str(), product_type);
    assert_eq!(resource.value_type().as_str(), value_type);
    assert_eq!(resource.handle_getter().as_str(), handle_getter);
    assert_eq!(resource.asset_getter().as_str(), asset_getter);
    assert_eq!(resource.manager_getter().as_str(), manager_getter);
}

fn assert_product_inputs(manager: &NativeManagerSpec, products: &[(&str, &str)]) {
    let product_inputs = manager
        .inputs()
        .iter()
        .filter_map(|input| match input {
            NativeManagerInput::Product(product) => Some(product),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(product_inputs.len(), products.len());
    for (input, (path, rust_type)) in product_inputs.into_iter().zip(products.iter().copied()) {
        assert_eq!(
            input.format(),
            crate::manager::NativeManagerProductFormat::ObjectStream
        );
        assert_eq!(input.asset_path().as_str(), path);
        assert_eq!(input.rust_type().as_str(), rust_type);
    }
}

fn assert_player_data_manager(
    manager: &NativeManagerSpec,
    products: &[(&str, &str, &str, &str, &str, &str)],
) {
    assert_product_inputs_with_format(
        manager,
        crate::manager::NativeManagerProductFormat::ObjectStream,
        &products
            .iter()
            .map(|(path, rust_type, _, _, _, _)| (*path, *rust_type))
            .collect::<Vec<_>>(),
    );
    let Some(NativeManagerShape::PlayerData(shape)) = manager.shape() else {
        panic!(
            "expected player data native API shape for `{}`",
            manager.rust_type().as_str()
        );
    };
    assert_eq!(shape.module().as_str(), "player_data");
    assert_product_asset_shape(shape.product_assets(), products);
}

fn assert_mixed_table_product_manager(
    manager: &NativeManagerSpec,
    tables: &[&str],
    products: &[(&str, &str)],
) {
    assert_eq!(manager.inputs().len(), tables.len() + products.len());
    let table_inputs = manager
        .inputs()
        .iter()
        .filter_map(|input| match input {
            NativeManagerInput::Table(table) => Some(table.table_name().as_str()),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(table_inputs, tables);

    let product_inputs = manager
        .inputs()
        .iter()
        .filter_map(|input| match input {
            NativeManagerInput::Product(product) => Some(product),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(product_inputs.len(), products.len());
    for (input, (path, rust_type)) in product_inputs.into_iter().zip(products.iter().copied()) {
        assert_eq!(
            input.format(),
            crate::manager::NativeManagerProductFormat::ObjectStream
        );
        assert_eq!(input.asset_path().as_str(), path);
        assert_eq!(input.rust_type().as_str(), rust_type);
    }
}

fn assert_manager_dependencies(manager: &NativeManagerSpec, dependencies: &[&str]) {
    let manager_inputs = manager
        .inputs()
        .iter()
        .filter_map(|input| match input {
            NativeManagerInput::Manager(resource) => Some(resource.as_str()),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(manager_inputs, dependencies);
}

fn assert_native_api_manager_surface(manager: &NativeManagerSpec) {
    assert_eq!(
        manager.shape().map(NativeManagerShape::resource_surface),
        Some(GeneratedManagerSurface::NativeApiManager)
    );
    assert!(
        manager
            .shape()
            .is_some_and(NativeManagerShape::exposes_native_api)
    );
}

fn assert_item_data_manager_shape(manager: &NativeManagerSpec) {
    let Some(NativeManagerShape::ItemData(shape)) = manager.shape() else {
        panic!(
            "expected item-data native API shape for `{}`",
            manager.rust_type().as_str()
        );
    };

    assert_eq!(shape.module().as_str(), "item_data");
    assert_eq!(shape.table_type().as_str(), "ItemDataTable");
    assert_eq!(shape.handle_type().as_str(), "ItemDataHandle");
    assert_eq!(shape.data_type().as_str(), "StaticItemData");
    assert!(!shape.tables().is_empty());
    assert!(
        shape
            .tables()
            .iter()
            .all(|table| table.row_type_name().as_str() == "MasterItemDefinitions")
    );
    assert!(
        shape
            .tables()
            .iter()
            .any(|table| table.table_name().as_str() == "MasterItemDefinitions_Common")
    );

    let master_item_table_count = manager
        .inputs()
        .iter()
        .filter(|input| {
            matches!(
                input,
                NativeManagerInput::Table(table)
                    if table.row_type_name().as_str() == "MasterItemDefinitions"
            )
        })
        .count();
    assert_eq!(shape.tables().len(), master_item_table_count);
}

fn assert_item_conversion_data_manager_shape(manager: &NativeManagerSpec) {
    let Some(NativeManagerShape::ItemConversionData(shape)) = manager.shape() else {
        panic!(
            "expected item-conversion native API shape for `{}`",
            manager.rust_type().as_str()
        );
    };

    assert_eq!(shape.module().as_str(), "item_conversion_data");
    assert_eq!(shape.table_name().as_str(), "ItemCurrencyConversions");
    assert_eq!(shape.row_type_name().as_str(), "ItemCurrencyConversionData");
    assert_eq!(shape.handle_type().as_str(), "ItemConversionDataHandle");
    assert_eq!(shape.data_type().as_str(), "ItemConversionData");
    assert_manager_dependencies(
        manager,
        &[
            "crate::ItemDataManager",
            "crate::CategoricalProgressionDataManager",
            "crate::AchievementDataManager",
        ],
    );
    assert!(manager.inputs().iter().any(|input| {
        matches!(
            input,
            NativeManagerInput::Table(table)
                if table.table_name().as_str() == "ItemCurrencyConversions"
                    && table.row_type_name().as_str() == "ItemCurrencyConversionData"
        )
    }));
}

fn assert_replication_data_manager_shape(manager: &NativeManagerSpec) {
    let Some(NativeManagerShape::ReplicationData(shape)) = manager.shape() else {
        panic!(
            "expected replication-data native API shape for `{}`",
            manager.rust_type().as_str()
        );
    };

    assert_eq!(shape.module().as_str(), "replication_data");
    assert_manager_dependencies(manager, &["crate::PerkDataManager"]);
    assert!(
        manager
            .inputs()
            .iter()
            .all(|input| { matches!(input, NativeManagerInput::Manager(_)) })
    );
}

fn assert_damage_data_manager_shape(manager: &NativeManagerSpec) {
    let Some(NativeManagerShape::DamageData(shape)) = manager.shape() else {
        panic!(
            "expected damage-data native API shape for `{}`",
            manager.rust_type().as_str()
        );
    };

    assert_eq!(shape.module().as_str(), "damage_data");
    assert!(!shape.damage_tables().is_empty());
    assert!(
        shape
            .damage_tables()
            .iter()
            .all(|table| table.row_type_name().as_str() == "DamageData")
    );

    let damage_table_count = manager
        .inputs()
        .iter()
        .filter(|input| {
            matches!(
                input,
                NativeManagerInput::Table(table)
                    if table.row_type_name().as_str() == "DamageData"
            )
        })
        .count();
    assert_eq!(shape.damage_tables().len(), damage_table_count);
    assert!(
        manager
            .inputs()
            .iter()
            .all(|input| { matches!(input, NativeManagerInput::Table(_)) })
    );
}

fn assert_character_attribute_data_manager_shape(manager: &NativeManagerSpec) {
    let Some(NativeManagerShape::CharacterAttributeData(shape)) = manager.shape() else {
        panic!(
            "expected character-attribute-data native API shape for `{}`",
            manager.rust_type().as_str()
        );
    };

    assert_eq!(shape.module().as_str(), "character_attribute_data");
    assert_eq!(shape.tables().len(), 5);
    for table_name in [
        "Constitution",
        "Dexterity",
        "Focus",
        "Intelligence",
        "Strength",
    ] {
        assert!(
            shape
                .tables()
                .iter()
                .any(|table| table.table_name().as_str() == table_name),
            "missing {table_name}",
        );
    }
    assert!(
        shape
            .tables()
            .iter()
            .all(|table| table.row_type_name().as_str() == "AttributeDefinition")
    );
    assert!(
        manager
            .inputs()
            .iter()
            .all(|input| { matches!(input, NativeManagerInput::Table(_)) })
    );
}

fn assert_vitals_data_manager_shape(manager: &NativeManagerSpec) {
    let Some(NativeManagerShape::VitalsData(shape)) = manager.shape() else {
        panic!(
            "expected vitals-data native API shape for `{}`",
            manager.rust_type().as_str()
        );
    };

    assert_eq!(shape.module().as_str(), "vitals_data");
    assert!(!shape.tables().is_empty());
    assert!(
        shape
            .tables()
            .iter()
            .all(|table| table.row_type_name().as_str() == "VitalsLevelVariantData")
    );
    assert!(
        shape
            .tables()
            .iter()
            .any(|table| table.table_name().as_str() == "LevelVariantVitals_Common")
    );
    assert_manager_dependencies(manager, &["crate::VitalsBaseDataManager"]);

    let vitals_level_variant_table_count = manager
        .inputs()
        .iter()
        .filter(|input| {
            matches!(
                input,
                NativeManagerInput::Table(table)
                    if table.row_type_name().as_str() == "VitalsLevelVariantData"
            )
        })
        .count();
    assert_eq!(shape.tables().len(), vitals_level_variant_table_count);
}

fn assert_status_effect_data_manager_shape(manager: &NativeManagerSpec) {
    let Some(NativeManagerShape::StatusEffectData(shape)) = manager.shape() else {
        panic!(
            "expected status-effect-data native API shape for `{}`",
            manager.rust_type().as_str()
        );
    };

    assert_eq!(shape.module().as_str(), "status_effect_data");
    assert!(!shape.tables().is_empty());
    assert!(
        shape
            .tables()
            .iter()
            .all(|table| table.row_type_name().as_str() == "StatusEffectData")
    );
    assert!(
        shape
            .tables()
            .iter()
            .any(|table| table.table_name().as_str() == "StatusEffects_Warhammer")
    );

    let status_effect_table_count = manager
        .inputs()
        .iter()
        .filter(|input| {
            matches!(
                input,
                NativeManagerInput::Table(table)
                    if table.row_type_name().as_str() == "StatusEffectData"
            )
        })
        .count();
    assert_eq!(shape.tables().len(), status_effect_table_count);
    assert!(
        manager
            .inputs()
            .iter()
            .all(|input| { matches!(input, NativeManagerInput::Table(_)) })
    );
}

fn assert_tradeskill_rank_data_manager_shape(manager: &NativeManagerSpec) {
    let Some(NativeManagerShape::TradeskillRankData(shape)) = manager.shape() else {
        panic!(
            "expected tradeskill-rank-data native API shape for `{}`",
            manager.rust_type().as_str()
        );
    };

    assert_eq!(shape.module().as_str(), "tradeskill_rank_data");
    assert_eq!(shape.xp_table_name().as_str(), "XPLevels");
    assert_eq!(shape.xp_row_type_name().as_str(), "ExperienceData");
    assert!(!shape.rank_tables().is_empty());
    assert!(
        shape
            .rank_tables()
            .iter()
            .all(|table| table.row_type_name().as_str() == "TradeskillRankData")
    );
    assert!(
        shape
            .rank_tables()
            .iter()
            .any(|table| table.table_name().as_str() == "Arcana")
    );

    let xp_table_count = manager
        .inputs()
        .iter()
        .filter(|input| {
            matches!(
                input,
                NativeManagerInput::Table(table)
                    if table.table_name().as_str() == "XPLevels"
                        && table.row_type_name().as_str() == "ExperienceData"
            )
        })
        .count();
    assert_eq!(xp_table_count, 1);
    let rank_table_count = manager
        .inputs()
        .iter()
        .filter(|input| {
            matches!(
                input,
                NativeManagerInput::Table(table)
                    if table.row_type_name().as_str() == "TradeskillRankData"
            )
        })
        .count();
    assert_eq!(shape.rank_tables().len(), rank_table_count);
    assert!(
        manager
            .inputs()
            .iter()
            .all(|input| { matches!(input, NativeManagerInput::Table(_)) })
    );
}

fn assert_static_tradeskill_rank_data_mapping_manager_shape(manager: &NativeManagerSpec) {
    let Some(NativeManagerShape::StaticTradeskillRankDataMapping(shape)) = manager.shape() else {
        panic!(
            "expected static tradeskill rank mapping native API shape for `{}`",
            manager.rust_type().as_str()
        );
    };
    assert_eq!(
        shape.module().as_str(),
        "static_tradeskill_rank_data_mapping"
    );
    assert_manager_dependencies(
        manager,
        &[
            "crate::ExperienceDataManager",
            "crate::PlayerDataManager",
            "crate::CategoricalProgressionDataManager",
            "crate::TradeskillRankDataManager",
        ],
    );
}

fn assert_currency_exchange_mapping_manager_shape(manager: &NativeManagerSpec) {
    let Some(NativeManagerShape::CurrencyExchangeMapping(shape)) = manager.shape() else {
        panic!(
            "expected currency exchange mapping native API shape for `{}`",
            manager.rust_type().as_str()
        );
    };
    assert_eq!(shape.module().as_str(), "currency_exchange_mapping");
    assert_manager_dependencies(
        manager,
        &[
            "crate::CurrencyExchangeDataManager",
            "crate::CategoricalProgressionDataManager",
        ],
    );
    assert!(
        manager
            .inputs()
            .iter()
            .all(|input| { matches!(input, NativeManagerInput::Manager(_)) })
    );
}

fn assert_dynamic_difficulty_data_manager_shape(manager: &NativeManagerSpec) {
    let Some(NativeManagerShape::DynamicDifficultyData(shape)) = manager.shape() else {
        panic!(
            "expected dynamic difficulty native API shape for `{}`",
            manager.rust_type().as_str()
        );
    };
    assert_eq!(shape.module().as_str(), "dynamic_difficulty_data");
    assert_mixed_table_manager_dependencies(
        manager,
        &["DynamicDifficulty"],
        &["crate::VitalsDataManager"],
    );
}

fn assert_elemental_mutation_static_data_manager_shape(manager: &NativeManagerSpec) {
    let Some(NativeManagerShape::ElementalMutationStaticData(shape)) = manager.shape() else {
        panic!(
            "expected elemental mutation native API shape for `{}`",
            manager.rust_type().as_str()
        );
    };
    assert_eq!(shape.module().as_str(), "elemental_mutation_static_data");
    assert_mixed_table_manager_dependencies(
        manager,
        &["ElementalMutation"],
        &[
            "crate::BuffBucketDataManager",
            "crate::StatusEffectDataManager",
        ],
    );
}

fn assert_promotion_mutation_static_data_manager_shape(manager: &NativeManagerSpec) {
    let Some(NativeManagerShape::PromotionMutationStaticData(shape)) = manager.shape() else {
        panic!(
            "expected promotion mutation native API shape for `{}`",
            manager.rust_type().as_str()
        );
    };
    assert_eq!(shape.module().as_str(), "promotion_mutation_static_data");
    assert_mixed_table_manager_dependencies(
        manager,
        &["PromotionMutation"],
        &[
            "crate::ElementalMutationStaticDataManager",
            "crate::BuffBucketDataManager",
            "crate::StatusEffectDataManager",
        ],
    );
}

fn assert_musical_rewards_data_manager_shape(manager: &NativeManagerSpec) {
    let Some(NativeManagerShape::MusicalRewardsData(shape)) = manager.shape() else {
        panic!(
            "expected musical rewards native API shape for `{}`",
            manager.rust_type().as_str()
        );
    };
    assert_eq!(shape.module().as_str(), "musical_rewards_data");
    assert_mixed_table_manager_dependencies(
        manager,
        &["MusicalPerformanceRewardsTable"],
        &[
            "crate::MusicalRankingDataManager",
            "crate::GameEventDataManager",
        ],
    );
}

fn assert_progression_point_data_manager_shape(manager: &NativeManagerSpec) {
    let Some(NativeManagerShape::ProgressionPointData(shape)) = manager.shape() else {
        panic!(
            "expected progression point native API shape for `{}`",
            manager.rust_type().as_str()
        );
    };
    assert_eq!(shape.module().as_str(), "progression_point_data");
    assert_mixed_table_manager_dependencies(
        manager,
        &["ProgressionPoints"],
        &["crate::ProgressionPoolDataManager"],
    );
}

fn assert_combat_profiles_data_manager_shape(manager: &NativeManagerSpec) {
    let Some(NativeManagerShape::CombatProfilesData(shape)) = manager.shape() else {
        panic!(
            "expected combat-profiles native API shape for `{}`",
            manager.rust_type().as_str()
        );
    };
    assert_eq!(shape.module().as_str(), "combat_profiles_data");
    assert_mixed_table_manager_dependencies(
        manager,
        &["CombatProfilesDataTable"],
        &["crate::CombatSettingsDataManager"],
    );
}

fn assert_item_transform_data_manager_shape(manager: &NativeManagerSpec) {
    let Some(NativeManagerShape::ItemTransformData(shape)) = manager.shape() else {
        panic!(
            "expected item-transform native API shape for `{}`",
            manager.rust_type().as_str()
        );
    };
    assert_eq!(shape.module().as_str(), "item_transform_data");
    assert_mixed_table_manager_dependencies(
        manager,
        &["DefaultItemTransforms", "GameModesItemTransforms"],
        &["crate::ItemDataManager"],
    );
}

fn assert_gatherable_data_manager_shape(manager: &NativeManagerSpec) {
    let Some(NativeManagerShape::GatherableData(shape)) = manager.shape() else {
        panic!(
            "expected gatherable-data native API shape for `{}`",
            manager.rust_type().as_str()
        );
    };
    assert_eq!(shape.module().as_str(), "gatherable_data");
    assert_product_asset_resource(
        shape.gathering_database(),
        "newworld_plugin::assets::gathering_database::GatheringDatabaseAsset",
        "newworld_plugin::assets::gathering_database::GatheringDatabase",
        "gathering_database",
        "database",
        "gathering_database",
    );
    assert_product_asset_resource(
        shape.gathering_action_database(),
        "newworld_plugin::assets::gathering_database::GatheringActionDatabaseAsset",
        "newworld_plugin::assets::gathering_database::GatheringActionDatabase",
        "gathering_action_database",
        "database",
        "gathering_action_database",
    );
    assert_mixed_table_product_manager(
        manager,
        &[
            "Gatherables",
            "GatherablesCatacombs",
            "GatherablesDunwood",
            "Gatherables_IsleOfNight",
            "QuestGatherables",
        ],
        &[
            (
                "sharedassets/genericassets/gathering/gatheringdatabase.gdb",
                "newworld_plugin::assets::gathering_database::GatheringDatabaseAsset",
            ),
            (
                "sharedassets/genericassets/gatheringactiondatabase.gactdb",
                "newworld_plugin::assets::gathering_database::GatheringActionDatabaseAsset",
            ),
        ],
    );
}

fn assert_social_data_manager_shape(manager: &NativeManagerSpec) {
    let Some(NativeManagerShape::SocialData(shape)) = manager.shape() else {
        panic!(
            "expected social-data native API shape for `{}`",
            manager.rust_type().as_str()
        );
    };
    assert_eq!(shape.module().as_str(), "social_data");
    assert_product_asset_resource(
        shape.rank_database(),
        "newworld_plugin::assets::rank_database::SocialRankDatabaseAsset",
        "newworld_plugin::assets::rank_database::SocialRankDatabase",
        "social_rank_database",
        "database",
        "rank_database",
    );
    assert_manager_dependencies(manager, &["crate::CrestPartDataManager"]);
    assert_product_inputs(
        manager,
        &[(
            "sharedassets/genericassets/rankdatabase.rankdb",
            "newworld_plugin::assets::rank_database::SocialRankDatabaseAsset",
        )],
    );
}

fn assert_seasons_rewards_activities_tasks_data_manager_shape(manager: &NativeManagerSpec) {
    let Some(NativeManagerShape::SeasonsRewardsActivitiesTasksData(shape)) = manager.shape() else {
        panic!(
            "expected seasons-rewards-activities-tasks native API shape for `{}`",
            manager.rust_type().as_str()
        );
    };
    assert_eq!(
        shape.module().as_str(),
        "seasons_rewards_activities_tasks_data"
    );
}

fn assert_seasons_rewards_battle_pass_data_manager_shape(manager: &NativeManagerSpec) {
    let Some(NativeManagerShape::SeasonsRewardsBattlePassData(shape)) = manager.shape() else {
        panic!(
            "expected seasons-rewards-battle-pass native API shape for `{}`",
            manager.rust_type().as_str()
        );
    };
    assert_eq!(shape.module().as_str(), "seasons_rewards_battle_pass_data");
}

fn assert_seasons_rewards_chapter_data_manager_shape(manager: &NativeManagerSpec) {
    let Some(NativeManagerShape::SeasonsRewardsChapterData(shape)) = manager.shape() else {
        panic!(
            "expected seasons-rewards-chapter native API shape for `{}`",
            manager.rust_type().as_str()
        );
    };
    assert_eq!(shape.module().as_str(), "seasons_rewards_chapter_data");
}

fn assert_seasons_rewards_journey_data_manager_shape(manager: &NativeManagerSpec) {
    let Some(NativeManagerShape::SeasonsRewardsJourneyData(shape)) = manager.shape() else {
        panic!(
            "expected seasons-rewards-journey native API shape for `{}`",
            manager.rust_type().as_str()
        );
    };
    assert_eq!(shape.module().as_str(), "seasons_rewards_journey_data");
}

fn assert_song_book_sheet_data_manager_shape(manager: &NativeManagerSpec) {
    let Some(NativeManagerShape::SongBookSheetData(shape)) = manager.shape() else {
        panic!(
            "expected song book sheet native API shape for `{}`",
            manager.rust_type().as_str()
        );
    };
    assert_eq!(shape.module().as_str(), "song_book_sheet_data");
    assert_mixed_table_manager_dependencies(
        manager,
        &["SongBookSheets"],
        &["crate::MusicalInstrumentSlotDataManager"],
    );
}

fn assert_song_book_data_manager_shape(manager: &NativeManagerSpec) {
    let Some(NativeManagerShape::SongBookData(shape)) = manager.shape() else {
        panic!(
            "expected song book native API shape for `{}`",
            manager.rust_type().as_str()
        );
    };
    assert_eq!(shape.module().as_str(), "song_book_data");
    assert_mixed_table_manager_dependencies(
        manager,
        &["SongBookData"],
        &["crate::SongBookSheetDataManager"],
    );
}

fn assert_vitals_modifier_mapping_manager_shape(manager: &NativeManagerSpec) {
    let Some(NativeManagerShape::VitalsModifierMapping(shape)) = manager.shape() else {
        panic!(
            "expected vitals-modifier-mapping native API shape for `{}`",
            manager.rust_type().as_str()
        );
    };

    assert_eq!(shape.module().as_str(), "vitals_modifier_mapping");
    assert_manager_dependencies(
        manager,
        &[
            "crate::VitalsDataManager",
            "crate::DamageDataManager",
            "crate::ItemDataManager",
        ],
    );
    assert!(
        manager
            .inputs()
            .iter()
            .all(|input| { matches!(input, NativeManagerInput::Manager(_)) })
    );
}

fn assert_mixed_table_manager_dependencies(
    manager: &NativeManagerSpec,
    tables: &[&str],
    dependencies: &[&str],
) {
    assert_eq!(manager.inputs().len(), tables.len() + dependencies.len());
    let table_inputs = manager
        .inputs()
        .iter()
        .filter_map(|input| match input {
            NativeManagerInput::Table(table) => Some(table.table_name().as_str()),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(table_inputs, tables);
    let manager_inputs = manager
        .inputs()
        .iter()
        .filter_map(|input| match input {
            NativeManagerInput::Manager(resource) => Some(resource.as_str()),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(manager_inputs, dependencies);
}

fn checked_in_runtime_table_resource_manager_names() -> BTreeSet<String> {
    let source = std::fs::read_to_string(checked_in_runtime_registry_path())
        .expect("read checked-in New World Bevy table-resource manager registration");

    source
        .match_indices("TableDataManagerPlugin::<")
        .filter_map(|(index, _)| {
            let rest = &source[index + "TableDataManagerPlugin::<".len()..];
            let end = rest.find('>')?;
            checked_in_runtime_manager_name(&rest[..end])
        })
        .collect()
}

fn checked_in_runtime_resource_manager_names() -> BTreeSet<String> {
    const PREFIX: &str = "RuntimeResourceManager::new(\"";
    let source = std::fs::read_to_string(checked_in_runtime_registry_path())
        .expect("read checked-in New World Bevy runtime-resource manager registration");

    source
        .match_indices(PREFIX)
        .filter_map(|(index, _)| {
            let rest = &source[index + PREFIX.len()..];
            let end = rest.find('"')?;
            checked_in_runtime_manager_name(&rest[..end])
        })
        .collect()
}

fn checked_in_runtime_bevy_manager_names() -> BTreeSet<String> {
    let mut names = checked_in_runtime_table_resource_manager_names();
    names.extend(checked_in_runtime_resource_manager_names());
    names
}

fn checked_in_runtime_ready_manager_names() -> BTreeSet<String> {
    const PREFIX: &str = "contains_resource::<";
    let source = std::fs::read_to_string(checked_in_runtime_registry_path())
        .expect("read checked-in New World Bevy runtime manager registration");

    source
        .match_indices(PREFIX)
        .filter_map(|(index, _)| {
            let rest = &source[index + PREFIX.len()..];
            let end = rest.find('>')?;
            checked_in_runtime_manager_name(&rest[..end])
        })
        .collect()
}

fn checked_in_runtime_manager_name(raw: &str) -> Option<String> {
    let raw = raw.trim().trim_end_matches(',').trim();
    let name = raw.strip_prefix("crate::").unwrap_or(raw);
    name.ends_with("Manager").then(|| name.to_owned())
}

fn checked_in_runtime_registry_path() -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../crates/newworld-gamedata/src/runtime_managers/registry.rs")
}

#[test]
fn validated_manager_specs_have_unique_runtime_types() {
    let mut seen = BTreeSet::new();
    let mut duplicates = Vec::new();

    for manager in validated_native_manager_specs() {
        let rust_type = manager.rust_type().as_str().to_owned();
        if !seen.insert(rust_type.clone()) {
            duplicates.push(rust_type);
        }
    }

    assert!(
        duplicates.is_empty(),
        "validated manager specs contain duplicate runtime types: {duplicates:?}"
    );
}

#[test]
fn validated_specs_cover_checked_in_runtime_bevy_managers() {
    let table_resource_managers = checked_in_runtime_table_resource_manager_names();
    let runtime_resource_managers = checked_in_runtime_resource_manager_names();
    let runtime_managers = checked_in_runtime_bevy_manager_names();
    let readiness_managers = checked_in_runtime_ready_manager_names();
    let spec_managers = validated_native_manager_specs()
        .into_iter()
        .filter_map(|manager| {
            manager
                .rust_type()
                .as_str()
                .strip_prefix("crate::")
                .map(str::to_owned)
        })
        .collect::<BTreeSet<_>>();

    assert!(
        !runtime_managers.is_empty(),
        "generated runtime manager inventory should not be empty"
    );
    let table_runtime_overlap = table_resource_managers
        .intersection(&runtime_resource_managers)
        .cloned()
        .collect::<Vec<_>>();
    assert!(
        table_runtime_overlap.is_empty(),
        "table-resource and runtime-resource manager inventories must not overlap: {table_runtime_overlap:?}"
    );
    let expected_runtime_resource_managers = runtime_managers
        .difference(&table_resource_managers)
        .cloned()
        .collect::<BTreeSet<_>>();
    assert_eq!(
        runtime_resource_managers, expected_runtime_resource_managers,
        "generated runtime-resource manager inventory must match non-table Bevy runtime managers"
    );
    assert_eq!(
        &readiness_managers, &runtime_managers,
        "generated runtime readiness checks must match generated runtime manager resources"
    );

    let missing = runtime_managers
        .difference(&spec_managers)
        .cloned()
        .collect::<Vec<_>>();
    assert!(
        missing.is_empty(),
        "missing codegen manager specs for runtime managers: {missing:?}"
    );

    let managers = validated_native_manager_specs();
    assert_mixed_table_manager_dependencies(
        find_manager(&managers, "crate::AffixDataManager"),
        &["AffixDataTable", "AffixStatDataTable"],
        &[],
    );
    let affix_data = find_manager(&managers, "crate::AffixDataManager");
    let Some(NativeManagerShape::MultiTableCrcKeyProjection(shape)) = affix_data.shape() else {
        panic!("expected multi-table CRC-key projection shape for `AffixDataManager`");
    };
    assert_eq!(shape.module().as_str(), "affix_data");
    assert_eq!(shape.projections().len(), 2);
    assert_eq!(
        shape.projections()[0].table_name().as_str(),
        "AffixDataTable"
    );
    assert_eq!(
        shape.projections()[1].table_name().as_str(),
        "AffixStatDataTable"
    );
    assert_eq!(
        shape.projections()[0].duplicate_key_policy(),
        NativeDuplicateKeyPolicy::FirstWins
    );
    assert_eq!(
        shape.projections()[1].duplicate_key_policy(),
        NativeDuplicateKeyPolicy::FirstWins
    );
    assert_manager_has_table(
        find_manager(&managers, "crate::AbilityDataManager"),
        "BowAbilityTable",
    );
    let ability_data = find_manager(&managers, "crate::AbilityDataManager");
    let Some(NativeManagerShape::AbilityData(shape)) = ability_data.shape() else {
        panic!("expected AbilityData shape for `AbilityDataManager`");
    };
    assert_eq!(shape.module().as_str(), "ability_data");
    assert_eq!(shape.tables().len(), 29);
    assert_eq!(
        shape.tables()[0].table_name().as_str(),
        "2025PerksAbilityTable"
    );
    assert_eq!(
        shape.tables()[28].table_name().as_str(),
        "WarHammerAbilityTable"
    );
    assert_manager_has_table(
        find_manager(&managers, "crate::MissionDataManager"),
        "Missions",
    );
    let mission_data = find_manager(&managers, "crate::MissionDataManager");
    let Some(NativeManagerShape::TableFamilyCrcKeyProjection(shape)) = mission_data.shape() else {
        panic!("expected table-family CRC-key projection shape for `MissionDataManager`");
    };
    assert_eq!(shape.module().as_str(), "mission_data");
    assert_eq!(shape.tables_type().as_str(), "MissionDataTables");
    assert_eq!(shape.table_type().as_str(), "MissionDataTable");
    assert_eq!(shape.handle_type().as_str(), "MissionDataHandle");
    assert_eq!(shape.row_alias().as_str(), "MissionDataRow");
    assert_eq!(shape.data_type().as_str(), "MissionData");
    assert_eq!(shape.entries_field().as_str(), "missions");
    assert_eq!(shape.index_field().as_str(), "missions_by_id");
    assert_eq!(shape.key_field().as_str(), "mission_key");
    assert_eq!(shape.crc_field().as_str(), "mission_id");
    assert_eq!(shape.key_column().as_str(), "MissionID");
    assert_eq!(shape.key_getter().as_str(), "mission_id");
    assert!(shape.skip_empty_key());
    assert!(!shape.reject_zero_crc());
    assert_eq!(
        shape.duplicate_key_policy(),
        NativeDuplicateKeyPolicy::FirstWins
    );
    assert_eq!(shape.source_row_field(), None);
    assert_eq!(shape.source_handle_field(), None);
    assert!(shape.fields().is_empty());
    assert!(shape.schema_validation_fields().is_some());
    assert_eq!(
        shape
            .tables()
            .iter()
            .map(|table| table.table_name().as_str())
            .collect::<Vec<_>>(),
        [
            "AffinityMissions",
            "Missions",
            "TerritoryProgressionMissions"
        ]
    );
    assert_eq!(
        shape
            .methods()
            .iter()
            .map(|method| method.name().as_str())
            .collect::<Vec<_>>(),
        [
            "mission_data_from_id",
            "mission_data",
            "mission_data_by_key"
        ]
    );
    assert_manager_has_table(
        find_manager(&managers, "crate::ObjectivesDataManager"),
        "ObjectivesDataManager",
    );
    assert_manager_has_table(
        find_manager(&managers, "crate::ObjectivesDataManager"),
        "ObjectiveTasksDataManager",
    );
    let objectives_data = find_manager(&managers, "crate::ObjectivesDataManager");
    let Some(NativeManagerShape::ObjectivesData(shape)) = objectives_data.shape() else {
        panic!("expected objectives-data shape for `ObjectivesDataManager`");
    };
    assert_eq!(shape.module().as_str(), "objectives_data");
    assert_eq!(shape.objective_tables().len(), 139);
    assert_eq!(shape.objective_task_tables().len(), 139);
    assert_eq!(
        shape.objective_tables()[0].table_name().as_str(),
        "ObjectivesDataManager"
    );
    assert_eq!(
        shape.objective_task_tables()[0].table_name().as_str(),
        "ObjectiveTasksDataManager"
    );
    assert_eq!(
        shape.objective_tables()[138].table_name().as_str(),
        "Quest_61001.datasheet"
    );
    assert_eq!(
        shape.objective_task_tables()[138].table_name().as_str(),
        "Quest_61001.datasheet"
    );
    assert_manager_has_table(
        find_manager(&managers, "crate::ContributionDataManager"),
        "Contribution",
    );
    let contribution_data = find_manager(&managers, "crate::ContributionDataManager");
    let Some(NativeManagerShape::ContributionData(shape)) = contribution_data.shape() else {
        panic!("expected contribution-data shape for `ContributionDataManager`");
    };
    assert_eq!(shape.module().as_str(), "contribution_data");
    assert_eq!(shape.tables().len(), 7);
    assert_eq!(shape.tables()[0].table_name().as_str(), "ArenaContribution");
    assert_eq!(
        shape.tables()[6].table_name().as_str(),
        "Season_02_Event_Contribution"
    );
    assert_manager_has_table(
        find_manager(&managers, "crate::BuffBucketDataManager"),
        "BuffBuckets",
    );
    let buff_bucket_data = find_manager(&managers, "crate::BuffBucketDataManager");
    let Some(NativeManagerShape::BuffBucketData(shape)) = buff_bucket_data.shape() else {
        panic!("expected buff-bucket-data shape for `BuffBucketDataManager`");
    };
    assert_eq!(shape.module().as_str(), "buff_bucket_data");
    assert_eq!(shape.table_name().as_str(), "BuffBuckets");
    assert_eq!(shape.row_type_name().as_str(), "BuffBucketData");
    assert_manager_has_table(
        find_manager(&managers, "crate::StructureDataManager"),
        "WallFootprint",
    );
    assert_manager_has_table(
        find_manager(&managers, "crate::StructureDataManager"),
        "T0_Wall_Pieces",
    );
    let structure_data = find_manager(&managers, "crate::StructureDataManager");
    let Some(NativeManagerShape::StructureData(shape)) = structure_data.shape() else {
        panic!("expected structure-data shape for `StructureDataManager`");
    };
    assert_eq!(shape.module().as_str(), "structure_data");
    assert_eq!(shape.footprint_table_name().as_str(), "WallFootprint");
    assert_eq!(
        shape.footprint_row_type_name().as_str(),
        "StructureFootprintData"
    );
    assert_eq!(shape.piece_table_name().as_str(), "T0_Wall_Pieces");
    assert_eq!(shape.piece_row_type_name().as_str(), "StructurePieceData");
    assert_manager_has_table(
        find_manager(&managers, "crate::ReusableScoreboardDataManager"),
        "PUGActivityInfo",
    );
    assert_manager_has_table(
        find_manager(&managers, "crate::ReusableScoreboardDataManager"),
        "ReusableScoreboard",
    );
    let reusable_scoreboard = find_manager(&managers, "crate::ReusableScoreboardDataManager");
    let Some(NativeManagerShape::ReusableScoreboardData(shape)) = reusable_scoreboard.shape()
    else {
        panic!("expected reusable-scoreboard-data shape for `ReusableScoreboardDataManager`");
    };
    assert_eq!(shape.module().as_str(), "reusable_scoreboard_data");
    assert_eq!(shape.pug_activity_table_name().as_str(), "PUGActivityInfo");
    assert_eq!(
        shape.pug_activity_row_type_name().as_str(),
        "PUGActivityInfo"
    );
    assert_eq!(shape.scoreboard_table_name().as_str(), "ReusableScoreboard");
    assert_eq!(
        shape.scoreboard_row_type_name().as_str(),
        "ReusableScoreboardTabData"
    );
    assert_manager_has_table(
        find_manager(&managers, "crate::MountHitVolumeDataManager"),
        "MountTypes",
    );
    let mount_hit_volume = find_manager(&managers, "crate::MountHitVolumeDataManager");
    let Some(NativeManagerShape::MountHitVolumeData(shape)) = mount_hit_volume.shape() else {
        panic!("expected mount-hit-volume-data shape for `MountHitVolumeDataManager`");
    };
    assert_eq!(shape.module().as_str(), "mount_hit_volume_data");
    assert_eq!(shape.table_name().as_str(), "MountTypes");
    assert_eq!(shape.row_type_name().as_str(), "MountTypeData");
    assert_eq!(
        shape.master_dynamic_slice().as_str(),
        "slices/MountHitVolumes/MountHitVolumes_Master.dynamicslice"
    );
    assert_generated_table_manager(
        find_manager(&managers, "crate::ConsumableItemDataManager"),
        "ConsumableItemDefinitions",
    );
    let consumable_item = find_manager(&managers, "crate::ConsumableItemDataManager");
    let Some(NativeManagerShape::OneTableOwnedStringCrcIndex(shape)) = consumable_item.shape()
    else {
        panic!("expected owned-string CRC index shape for `ConsumableItemDataManager`");
    };
    assert_eq!(shape.module().as_str(), "consumable_item_data");
    assert_eq!(shape.key_column().as_str(), "ConsumableID");
    assert_eq!(shape.key_getter().as_str(), "consumable_id");
    assert_eq!(
        shape.duplicate_key_policy(),
        NativeDuplicateKeyPolicy::Error
    );
    assert_manager_has_table(
        find_manager(&managers, "crate::SpellDataManager"),
        "SpellDataTable",
    );
    assert_manager_has_table(
        find_manager(&managers, "crate::NPCDataManager"),
        "NPCs_C99G",
    );
    let npc_data = find_manager(&managers, "crate::NPCDataManager");
    let Some(NativeManagerShape::TableFamilyCrcKeyProjection(shape)) = npc_data.shape() else {
        panic!("expected table-family CRC-key projection shape for `NPCDataManager`");
    };
    assert_eq!(shape.key_column().as_str(), "NpcId");
    assert!(shape.skip_empty_key());
    assert_eq!(
        shape.duplicate_key_policy(),
        NativeDuplicateKeyPolicy::FirstWins
    );
    assert_eq!(
        shape.source_row_field().map(RustIdentifier::as_str),
        Some("source_row")
    );
    assert_eq!(
        shape
            .fields()
            .iter()
            .map(|field| (field.field().as_str(), field.transform()))
            .collect::<Vec<_>>(),
        [
            ("shop_id_key", NativeProjectionTransform::OptionalString),
            (
                "shop_id_crc",
                NativeProjectionTransform::OptionalLowercaseCrcString
            ),
        ]
    );
    assert_eq!(shape.field_lookup_methods().len(), 1);
    let npc_shop = &shape.field_lookup_methods()[0];
    assert_eq!(npc_shop.name().as_str(), "shop_id");
    assert_eq!(npc_shop.key_parameter().name().as_str(), "npc_id");
    assert_eq!(
        npc_shop.key_parameter().kind(),
        NativeCrcIndexLookupParameterKind::Crc32
    );
    assert_eq!(npc_shop.field().as_str(), "shop_id_crc");
    assert_eq!(npc_shop.value_type().as_str(), "Crc32");
    assert!(npc_shop.optional_result());
    let simple_tree = find_manager(&managers, "crate::SimpleTreeCategoryDataManager");
    let Some(NativeManagerShape::TableFamilyCrcKeyProjection(shape)) = simple_tree.shape() else {
        panic!(
            "expected table-family CRC-key projection shape for `SimpleTreeCategoryDataManager`"
        );
    };
    assert_eq!(shape.key_column().as_str(), "MetaAchievementCategoryId");
    assert!(shape.skip_empty_key());
    assert_eq!(
        shape.duplicate_key_policy(),
        NativeDuplicateKeyPolicy::FirstWins
    );
    assert_eq!(shape.source_row_field(), None);
    assert_eq!(shape.source_handle_field(), None);
    assert_eq!(
        shape
            .fields()
            .iter()
            .map(|field| (field.field().as_str(), field.transform()))
            .collect::<Vec<_>>(),
        [
            (
                "parent_category_crc",
                NativeProjectionTransform::OptionalLowercaseCrcStringDefaultZero
            ),
            ("index", NativeProjectionTransform::NonZeroU32),
            ("title", NativeProjectionTransform::String),
            (
                "icon_color_background",
                NativeProjectionTransform::OptionalString
            ),
            (
                "hide_from_ui",
                NativeProjectionTransform::OptionalBoolDefaultFalse
            ),
        ]
    );
    let ammo_items = find_manager(&managers, "crate::AmmoItemDataManager");
    assert_mixed_table_manager_dependencies(
        ammo_items,
        &["AmmoItemDefinitions", "AmmoItemDefinitions_IsleOfNight"],
        &[],
    );
    assert_character_attribute_data_manager_shape(find_manager(
        &managers,
        "crate::CharacterAttributeDataManager",
    ));
    let Some(NativeManagerShape::TableFamilyCrcKeyProjection(shape)) = ammo_items.shape() else {
        panic!("expected table-family CRC-key projection shape for `AmmoItemDataManager`");
    };
    assert_eq!(shape.key_column().as_str(), "AmmoID");
    assert!(shape.skip_empty_key());
    assert_eq!(
        shape.duplicate_key_policy(),
        NativeDuplicateKeyPolicy::Error
    );
    assert_generated_table_manager(
        find_manager(&managers, "crate::CampSkinDataManager"),
        "CampSkinDataTable",
    );
    let camp_skin = find_manager(&managers, "crate::CampSkinDataManager");
    let Some(NativeManagerShape::OneTableCampSkin(shape)) = camp_skin.shape() else {
        panic!("expected one-table camp-skin shape for `CampSkinDataManager`");
    };
    assert_eq!(shape.key_column().as_str(), "CampSkinID");
    assert_eq!(shape.key_getter().as_str(), "camp_skin_id");
    assert_eq!(shape.item_id_getter().as_str(), "item_id");
    assert_eq!(
        shape.required_achievement_id_getter().as_str(),
        "required_achievement_id"
    );
    assert_eq!(shape.is_entitlement_getter().as_str(), "is_entitlement");
    assert_eq!(shape.is_enabled_getter().as_str(), "is_enabled");
    assert_eq!(shape.lookup_method().as_str(), "camp_skin_data");
    assert_eq!(
        shape.lookup_by_key_method().as_str(),
        "camp_skin_data_by_key"
    );
    assert_eq!(shape.ids_method().as_str(), "camp_skin_ids");
    assert_generated_table_manager(
        find_manager(&managers, "crate::DyeColorDataManager"),
        "DyeColorDataTable",
    );
    let dye_color = find_manager(&managers, "crate::DyeColorDataManager");
    let Some(NativeManagerShape::OneTableDyeColor(shape)) = dye_color.shape() else {
        panic!("expected one-table dye-color shape for `DyeColorDataManager`");
    };
    assert_eq!(shape.index_column().as_str(), "Index");
    assert_eq!(shape.index_getter().as_str(), "index");
    assert_eq!(shape.name_getter().as_str(), "name");
    assert_eq!(shape.color_getter().as_str(), "color");
    assert_eq!(shape.category_getter().as_str(), "category");
    assert_eq!(shape.is_entitlement_getter().as_str(), "is_entitlement");
    assert_eq!(shape.lookup_method().as_str(), "dye_color_data");
    assert_eq!(
        shape.lookup_from_index_method().as_str(),
        "dye_color_data_from_index"
    );
    assert_eq!(
        shape.lookup_by_key_method().as_str(),
        "dye_color_data_by_key"
    );
    assert_eq!(shape.rows_method().as_str(), "rows");
    assert_eq!(
        shape.entitlement_indexes_method().as_str(),
        "entitlement_indexes"
    );
    assert_generated_table_manager(
        find_manager(&managers, "crate::EmoteDataManager"),
        "EmoteDefinitions",
    );
    let emote = find_manager(&managers, "crate::EmoteDataManager");
    let Some(NativeManagerShape::OneTableEmote(shape)) = emote.shape() else {
        panic!("expected one-table emote shape for `EmoteDataManager`");
    };
    assert_eq!(shape.table_name().as_str(), "EmoteDefinitions");
    assert_eq!(shape.row_type_name().as_str(), "EmoteDefinitions");
    assert_eq!(shape.settings_type().as_str(), "EmoteDataSettings");
    assert_eq!(shape.cache_type().as_str(), "EmoteIndexes");
    assert_eq!(shape.cache_field().as_str(), "emotes");
    assert_eq!(shape.lookup_from_id_method().as_str(), "emote_data_from_id");
    assert_eq!(shape.lookup_method().as_str(), "emote_data");
    assert_eq!(shape.lookup_by_key_method().as_str(), "emote_data_by_key");
    assert_eq!(
        shape.status_effect_lookup_by_crc_method().as_str(),
        "emote_id_by_status_effect"
    );
    assert_eq!(
        shape.status_effect_lookup_method().as_str(),
        "emote_id_for_status_effect"
    );
    assert_generated_table_manager(
        find_manager(&managers, "crate::ExperienceDataManager"),
        "XPLevels",
    );
    let experience = find_manager(&managers, "crate::ExperienceDataManager");
    let Some(NativeManagerShape::OneTableExperience(shape)) = experience.shape() else {
        panic!("expected one-table experience shape for `ExperienceDataManager`");
    };
    assert_eq!(shape.table_name().as_str(), "XPLevels");
    assert_eq!(shape.row_type_name().as_str(), "XPLevels");
    assert_eq!(shape.cache_type().as_str(), "ExperienceDataIndexes");
    assert_eq!(shape.cache_field().as_str(), "experience");
    assert_eq!(
        shape.lookup_from_id_method().as_str(),
        "experience_data_from_id"
    );
    assert_eq!(shape.lookup_method().as_str(), "experience_data");
    assert_eq!(
        shape.gear_score_lookup_method().as_str(),
        "experience_data_for_max_equippable_gear_score"
    );
    assert_eq!(shape.level_for_xp_method().as_str(), "level_for_xp");
    assert_eq!(shape.max_level_method().as_str(), "max_level");
    assert_generated_table_manager(
        find_manager(&managers, "crate::InteractionAnimationDataManager"),
        "InteractionAnimations",
    );
    assert_status_effect_data_manager_shape(find_manager(
        &managers,
        "crate::StatusEffectDataManager",
    ));
    assert_generated_table_manager(
        find_manager(&managers, "crate::TwitchDropsStatDataManager"),
        "TwitchDropsStatDefinitions",
    );
    assert_generated_table_manager(
        find_manager(&managers, "crate::WeaponAccessoryDataManager"),
        "WeaponAccessoryDefinitions",
    );
    assert_mixed_table_manager_dependencies(
        find_manager(&managers, "crate::WeaponAppearanceDataManager"),
        &[
            "InstrumentsAppearanceDefinitions",
            "WeaponAppearanceDefinitions",
            "WeaponAppearanceDefinitions_MountAttachments",
        ],
        &[],
    );
    let weapon_appearances = find_manager(&managers, "crate::WeaponAppearanceDataManager");
    let Some(NativeManagerShape::TableFamilyCrcKeyProjection(shape)) = weapon_appearances.shape()
    else {
        panic!("expected table-family CRC-key projection shape for `WeaponAppearanceDataManager`");
    };
    assert_eq!(shape.key_column().as_str(), "WeaponAppearanceID");
    assert_eq!(shape.key_getter().as_str(), "weapon_appearance_id");
    assert!(shape.skip_empty_key());
    assert!(!shape.trim_key());
    assert!(!shape.reject_zero_crc());
    assert_eq!(
        shape.duplicate_key_policy(),
        NativeDuplicateKeyPolicy::Error
    );
    assert_eq!(shape.source_row_field(), None);
    assert_eq!(shape.source_row_method(), None);
    assert_eq!(shape.source_row_by_crc_method(), None);
    assert_eq!(shape.source_handle_field(), None);
    assert!(shape.source_handle_method().is_none());
    assert!(shape.fields().is_empty());
    assert_eq!(
        shape.rows_method().map(RustIdentifier::as_str),
        Some("rows")
    );
    assert_generated_table_manager(
        find_manager(&managers, "crate::ThrowableItemDataManager"),
        "ThrowableItemDefinitions",
    );
    assert_generated_table_manager(
        find_manager(&managers, "crate::WeaponTierDataManager"),
        "WeaponTiersTable",
    );
    assert_generated_table_manager(
        find_manager(&managers, "crate::SeasonsRewardsTaskDataManager"),
        "SeasonsRewardsTasks",
    );
    let task_data = find_manager(&managers, "crate::SeasonsRewardsTaskDataManager");
    let Some(NativeManagerShape::OneTableCrcKeyProjection(shape)) = task_data.shape() else {
        panic!("expected one-table CRC-key projection shape for `SeasonsRewardsTaskDataManager`");
    };
    assert_eq!(shape.module().as_str(), "seasons_rewards_task_data");
    assert_eq!(shape.key_column().as_str(), "SeasonsTaskID");
    assert_eq!(shape.key_getter().as_str(), "seasons_task_id");
    assert_eq!(
        shape.duplicate_key_policy(),
        NativeDuplicateKeyPolicy::FirstWins
    );
    assert_eq!(
        shape.source_row_field().map(RustIdentifier::as_str),
        Some("source_row")
    );
    assert_eq!(shape.row_filters().len(), 1);
    assert_eq!(
        shape.row_filters()[0].column().as_str(),
        "SeasonsTrackedStatID"
    );
    assert_eq!(
        shape.row_filters()[0].predicate(),
        NativeCrcProjectionRowFilterPredicate::LowercaseCrcStringNonZero
    );
    assert_eq!(
        shape
            .fields()
            .iter()
            .map(|field| (field.field().as_str(), field.transform()))
            .collect::<Vec<_>>(),
        [
            (
                "seasons_tracked_stat_id",
                NativeProjectionTransform::LowercaseCrcString
            ),
            (
                "seasons_tracked_stat_key",
                NativeProjectionTransform::String
            ),
            ("task_max_value", NativeProjectionTransform::NonZeroU32),
            ("name", NativeProjectionTransform::String),
            ("description", NativeProjectionTransform::String),
        ]
    );
    assert_manager_has_table(
        find_manager(&managers, "crate::SeasonsRewardsSeasonDataManager"),
        "SeasonsRewardsSeasonDataTable",
    );
    let season_data = find_manager(&managers, "crate::SeasonsRewardsSeasonDataManager");
    let Some(NativeManagerShape::OneTableCrcKeyProjection(shape)) = season_data.shape() else {
        panic!("expected one-table CRC-key projection shape for `SeasonsRewardsSeasonDataManager`");
    };
    assert_eq!(shape.key_column().as_str(), "SeasonId");
    assert_eq!(shape.key_getter().as_str(), "season_id");
    assert!(shape.skip_empty_key());
    assert!(!shape.trim_key());
    assert!(!shape.reject_zero_crc());
    assert_eq!(
        shape.duplicate_key_policy(),
        NativeDuplicateKeyPolicy::FirstWins
    );
    assert_eq!(shape.source_row_field(), None);
    assert_eq!(shape.source_row_method(), None);
    assert_eq!(shape.row_filters().len(), 1);
    assert_eq!(
        shape.row_filters()[0].column().as_str(),
        "PremiumEntitlementId"
    );
    assert_eq!(
        shape.row_filters()[0].predicate(),
        NativeCrcProjectionRowFilterPredicate::LowercaseCrcStringNonZero
    );
    assert_eq!(shape.secondary_indexes().len(), 1);
    let season_index = &shape.secondary_indexes()[0];
    assert_eq!(season_index.index_field().as_str(), "seasons_by_index");
    assert_eq!(season_index.key_field().as_str(), "season_index");
    assert_eq!(
        season_index.key_type(),
        NativeSecondaryIndexKeyType::NonZeroU32
    );
    assert_eq!(
        season_index.duplicate_key_policy(),
        NativeDuplicateKeyPolicy::FirstWins
    );
    assert_eq!(season_index.methods().len(), 1);
    assert_eq!(
        shape
            .fields()
            .iter()
            .map(|field| (field.field().as_str(), field.transform()))
            .collect::<Vec<_>>(),
        [
            ("season_index", NativeProjectionTransform::NonZeroU32),
            ("name", NativeProjectionTransform::String),
            ("display_name", NativeProjectionTransform::String),
            ("description", NativeProjectionTransform::String),
            (
                "premium_entitlement_id",
                NativeProjectionTransform::LowercaseCrcString
            ),
            ("premium_entitlement_key", NativeProjectionTransform::String),
            (
                "purchased_levels_entitlement_id",
                NativeProjectionTransform::LowercaseCrcString
            ),
            (
                "purchased_levels_entitlement_key",
                NativeProjectionTransform::String
            ),
            (
                "fresh_start_world_gen",
                NativeProjectionTransform::NonZeroU32
            ),
        ]
    );
    assert_eq!(
        shape.rows_method().map(RustIdentifier::as_str),
        Some("seasons")
    );
    assert_mixed_table_manager_dependencies(
        find_manager(
            &managers,
            "crate::SeasonsRewardsActivitiesConfigDataManager",
        ),
        &[
            "SeasonsRewardsActivitiesConfig_Season1",
            "SeasonsRewardsActivitiesConfig_Season10",
            "SeasonsRewardsActivitiesConfig_Season2",
            "SeasonsRewardsActivitiesConfig_Season3",
            "SeasonsRewardsActivitiesConfig_Season4",
            "SeasonsRewardsActivitiesConfig_Season5",
            "SeasonsRewardsActivitiesConfig_Season6",
            "SeasonsRewardsActivitiesConfig_Season7",
            "SeasonsRewardsActivitiesConfig_Season8",
            "SeasonsRewardsActivitiesConfig_Season9",
        ],
        &[],
    );
    let activities_config = find_manager(
        &managers,
        "crate::SeasonsRewardsActivitiesConfigDataManager",
    );
    let Some(NativeManagerShape::TableFamilyCrcKeyProjection(shape)) = activities_config.shape()
    else {
        panic!(
            "expected table-family CRC-key projection shape for `SeasonsRewardsActivitiesConfigDataManager`"
        );
    };
    assert_eq!(shape.key_column().as_str(), "ConfigId");
    assert_eq!(shape.key_getter().as_str(), "config_id");
    assert!(shape.skip_empty_key());
    assert!(!shape.trim_key());
    assert!(!shape.reject_zero_crc());
    assert_eq!(
        shape.duplicate_key_policy(),
        NativeDuplicateKeyPolicy::FirstWins
    );
    assert_eq!(shape.source_row_field(), None);
    assert_eq!(shape.source_row_method(), None);
    assert_eq!(shape.source_row_by_crc_method(), None);
    assert_eq!(shape.source_handle_field(), None);
    assert!(shape.source_handle_method().is_none());
    assert_eq!(
        shape
            .fields()
            .iter()
            .map(|field| (field.field().as_str(), field.transform()))
            .collect::<Vec<_>>(),
        [("config_value", NativeProjectionTransform::NonZeroU32)]
    );
    assert_mixed_table_manager_dependencies(
        find_manager(&managers, "crate::SeasonsRewardsActivitiesTasksDataManager"),
        &[
            "SeasonsRewardsActivitiesTasksData_Season10",
            "SeasonsRewardsActivitiesTasksData_Season5",
            "SeasonsRewardsActivitiesTasksData_Season6",
            "SeasonsRewardsActivitiesTasksData_Season7",
            "SeasonsRewardsActivitiesTasksData_Season8",
            "SeasonsRewardsActivitiesTasksData_Season9",
        ],
        &["crate::SeasonsRewardsTaskDataManager"],
    );
    assert_mixed_table_manager_dependencies(
        find_manager(&managers, "crate::SeasonsRewardsCardDataManager"),
        &[
            "SeasonsRewardsCardData_Season1",
            "SeasonsRewardsCardData_Season10",
            "SeasonsRewardsCardData_Season2",
            "SeasonsRewardsCardData_Season3",
            "SeasonsRewardsCardData_Season4",
            "SeasonsRewardsCardData_Season5",
            "SeasonsRewardsCardData_Season6",
            "SeasonsRewardsCardData_Season7",
            "SeasonsRewardsCardData_Season8",
            "SeasonsRewardsCardData_Season9",
        ],
        &[],
    );
    let card_data = find_manager(&managers, "crate::SeasonsRewardsCardDataManager");
    let Some(NativeManagerShape::TableFamilyPartitionedCrcKeyProjection(shape)) = card_data.shape()
    else {
        panic!(
            "expected table-family partitioned CRC-key projection shape for `SeasonsRewardsCardDataManager`"
        );
    };
    assert_eq!(shape.key_column().as_str(), "CardId");
    assert_eq!(shape.key_getter().as_str(), "card_id");
    assert!(shape.skip_empty_key());
    assert!(shape.trim_key());
    assert!(!shape.reject_zero_crc());
    let global_index = shape.global_index().expect("global card index");
    assert_eq!(global_index.index_field().as_str(), "cards_by_card_id");
    assert_eq!(
        global_index.duplicate_key_policy(),
        NativeDuplicateKeyPolicy::FirstWins
    );
    assert_eq!(global_index.methods().len(), 2);
    assert_eq!(shape.table_indexes().len(), 10);
    assert!(
        shape
            .table_indexes()
            .iter()
            .all(|index| index.duplicate_key_policy() == NativeDuplicateKeyPolicy::FirstWins)
    );
    assert_eq!(
        shape
            .fields()
            .iter()
            .map(|field| (field.field().as_str(), field.transform()))
            .collect::<Vec<_>>(),
        [
            ("stamps_to_complete", NativeProjectionTransform::NonZeroU32),
            ("line_bonus_xp", NativeProjectionTransform::NonZeroU32),
            ("pattern_bonus_xp", NativeProjectionTransform::NonZeroU32),
            ("card_bonus_xp", NativeProjectionTransform::NonZeroU32),
        ]
    );
    assert_eq!(
        shape.rows_method().map(RustIdentifier::as_str),
        Some("cards")
    );
    assert_mixed_table_manager_dependencies(
        find_manager(&managers, "crate::SeasonsRewardsCardTemplateDataManager"),
        &[
            "SeasonsRewardsCardTemplates_Season1",
            "SeasonsRewardsCardTemplates_Season10",
            "SeasonsRewardsCardTemplates_Season2",
            "SeasonsRewardsCardTemplates_Season3",
            "SeasonsRewardsCardTemplates_Season4",
            "SeasonsRewardsCardTemplates_Season5",
            "SeasonsRewardsCardTemplates_Season6",
            "SeasonsRewardsCardTemplates_Season7",
            "SeasonsRewardsCardTemplates_Season8",
            "SeasonsRewardsCardTemplates_Season9",
        ],
        &[],
    );
    let card_template = find_manager(&managers, "crate::SeasonsRewardsCardTemplateDataManager");
    let Some(NativeManagerShape::SeasonsRewardsCardTemplateData(shape)) = card_template.shape()
    else {
        panic!("expected card-template shape for `SeasonsRewardsCardTemplateDataManager`");
    };
    assert_eq!(
        shape.module().as_str(),
        "seasons_rewards_card_template_data"
    );
    assert_mixed_table_manager_dependencies(
        find_manager(&managers, "crate::SeasonsRewardsDataManager"),
        &[
            "SeasonsRewardData_Season1",
            "SeasonsRewardData_Season10",
            "SeasonsRewardData_Season2",
            "SeasonsRewardData_Season3",
            "SeasonsRewardData_Season4",
            "SeasonsRewardData_Season5",
            "SeasonsRewardData_Season6",
            "SeasonsRewardData_Season7",
            "SeasonsRewardData_Season8",
            "SeasonsRewardData_Season9",
        ],
        &[],
    );
    assert_mixed_table_manager_dependencies(
        find_manager(&managers, "crate::SeasonsRewardsBattlePassDataManager"),
        &[
            "SeasonPass_Season1",
            "SeasonPass_Season10",
            "SeasonPass_Season2",
            "SeasonPass_Season3",
            "SeasonPass_Season4",
            "SeasonPass_Season5",
            "SeasonPass_Season6",
            "SeasonPass_Season7",
            "SeasonPass_Season8",
            "SeasonPass_Season9",
        ],
        &["crate::SeasonsRewardsSeasonDataManager"],
    );
    assert_mixed_table_manager_dependencies(
        find_manager(&managers, "crate::SeasonsRewardsChapterDataManager"),
        &[
            "SeasonsRewardsChapterData_Season1",
            "SeasonsRewardsChapterData_Season10",
            "SeasonsRewardsChapterData_Season2",
            "SeasonsRewardsChapterData_Season3",
            "SeasonsRewardsChapterData_Season4",
            "SeasonsRewardsChapterData_Season5",
            "SeasonsRewardsChapterData_Season6",
            "SeasonsRewardsChapterData_Season7",
            "SeasonsRewardsChapterData_Season8",
            "SeasonsRewardsChapterData_Season9",
        ],
        &["crate::SeasonsRewardsSeasonDataManager"],
    );
    assert_mixed_table_manager_dependencies(
        find_manager(&managers, "crate::SeasonsRewardsJourneyDataManager"),
        &[
            "SeasonsRewardsJourneyData_Season1",
            "SeasonsRewardsJourneyData_Season10",
            "SeasonsRewardsJourneyData_Season2",
            "SeasonsRewardsJourneyData_Season3",
            "SeasonsRewardsJourneyData_Season4",
            "SeasonsRewardsJourneyData_Season5",
            "SeasonsRewardsJourneyData_Season6",
            "SeasonsRewardsJourneyData_Season7",
            "SeasonsRewardsJourneyData_Season8",
            "SeasonsRewardsJourneyData_Season9",
        ],
        &[
            "crate::SeasonsRewardsSeasonDataManager",
            "crate::SeasonsRewardsTaskDataManager",
            "crate::SeasonsRewardsChapterDataManager",
        ],
    );
    assert_mixed_table_manager_dependencies(
        find_manager(&managers, "crate::WhisperDataManager"),
        &["WhisperDataManager", "WhisperVFXData"],
        &[],
    );
    assert_mixed_table_manager_dependencies(
        find_manager(&managers, "crate::TimelineRegistryManager"),
        &[
            "GenericTimelineRegistryEntry",
            "TimelineRegistryEntry",
            "WhisperTimelineRegistryEntry",
        ],
        &[],
    );
    assert_mixed_table_manager_dependencies(
        find_manager(&managers, "crate::CameraShakeDataManager"),
        &["CameraShakeDataTable", "CameraShakeDataTable_IsleOfNight"],
        &[],
    );
    assert_mixed_table_manager_dependencies(
        find_manager(&managers, "crate::ShopDataManager"),
        &["ShopData"],
        &["crate::NPCDataManager"],
    );
    assert_static_tradeskill_rank_data_mapping_manager_shape(find_manager(
        &managers,
        "crate::StaticTradeskillRankDataMappingManager",
    ));
    assert_manager_has_table(
        find_manager(&managers, "crate::TerritoryDefinitionsDataManager"),
        "PointsOfInterest_06_04",
    );
}

#[test]
fn validated_native_specs_include_table_product_and_manager_inputs() {
    let managers = validated_native_manager_specs();

    let achievement = find_manager(&managers, "crate::AchievementDataManager");
    assert_generated_table_manager(achievement, "AchievementDataTable");
    let Some(NativeManagerShape::OneTableCrcKeyProjection(shape)) = achievement.shape() else {
        panic!("expected CRC-key projection shape for `AchievementDataManager`");
    };
    assert_eq!(shape.key_column().as_str(), "AchievementID");
    assert_eq!(
        shape.duplicate_key_policy(),
        NativeDuplicateKeyPolicy::Error
    );
    let achievement_index = shape
        .fields()
        .iter()
        .find(|field| field.field().as_str() == "achievement_index")
        .expect("achievement index field");
    assert_eq!(
        achievement_index.transform(),
        NativeProjectionTransform::U32ToU16BelowMax
    );
    assert_eq!(achievement_index.u16_max_exclusive(), Some(32_000));
    assert_eq!(shape.secondary_indexes().len(), 1);
    let bit_index = &shape.secondary_indexes()[0];
    assert_eq!(bit_index.index_field().as_str(), "rows_by_bit_index");
    assert_eq!(bit_index.key_field().as_str(), "achievement_index");
    assert_eq!(bit_index.key_type(), NativeSecondaryIndexKeyType::U16);
    assert_eq!(
        bit_index.duplicate_key_policy(),
        NativeDuplicateKeyPolicy::Error
    );
    assert_eq!(
        bit_index.methods()[0].name().as_str(),
        "achievement_data_by_bit_index"
    );

    let item_skin = find_manager(&managers, "crate::ItemSkinDataManager");
    assert_generated_table_manager(item_skin, "ItemSkinDataTable");
    let Some(NativeManagerShape::OneTableCrcKeyProjection(shape)) = item_skin.shape() else {
        panic!("expected CRC-key projection shape for `ItemSkinDataManager`");
    };
    assert_eq!(shape.key_column().as_str(), "ItemSkinID");
    assert_eq!(
        shape.duplicate_key_policy(),
        NativeDuplicateKeyPolicy::Error
    );
    assert_eq!(
        shape.source_row_method().map(RustIdentifier::as_str),
        Some("source_row")
    );
    assert_eq!(shape.secondary_indexes().len(), 1);
    let index_to_row = &shape.secondary_indexes()[0];
    assert_eq!(index_to_row.index_field().as_str(), "index_to_row");
    assert_eq!(index_to_row.key_field().as_str(), "index_id");
    assert_eq!(
        index_to_row.key_type(),
        NativeSecondaryIndexKeyType::NonZeroU32
    );
    assert_eq!(
        index_to_row.storage(),
        NativeSecondaryIndexStorage::SparseVec
    );
    assert_eq!(
        index_to_row.duplicate_key_policy(),
        NativeDuplicateKeyPolicy::Error
    );
    assert_eq!(
        index_to_row.methods()[0].name().as_str(),
        "item_skin_by_index"
    );
    assert_eq!(
        index_to_row.methods()[1].name().as_str(),
        "item_skin_id_at_index"
    );
    assert_eq!(
        index_to_row.methods()[1].result(),
        &crate::manager::NativeSecondaryIndexLookupResult::StringField(
            RustIdentifier::new("item_skin_id").expect("field")
        )
    );
    assert_eq!(
        shape
            .fields()
            .iter()
            .map(|field| (field.field().as_str(), field.transform()))
            .collect::<Vec<_>>(),
        [
            ("index_id", NativeProjectionTransform::NonZeroU32),
            ("is_entitlement", NativeProjectionTransform::Bool),
            ("from_item_ids", NativeProjectionTransform::OptionalString),
            ("needs_one_classes", NativeProjectionTransform::String),
            (
                "required_classes",
                NativeProjectionTransform::OptionalString
            ),
            (
                "excluded_classes",
                NativeProjectionTransform::OptionalString
            ),
            ("to_item_row", NativeProjectionTransform::ForeignKeyRow),
            ("outfit", NativeProjectionTransform::OptionalString),
            ("is_temporary_skin", NativeProjectionTransform::Bool),
        ]
    );

    let spell_data = find_manager(&managers, "crate::SpellDataManager");
    assert_manager_has_table(spell_data, "SpellDataTable");
    assert_manager_has_table(spell_data, "SpellDataTable_WarHammer");
    let Some(NativeManagerShape::TableFamilyCrcKeyProjection(shape)) = spell_data.shape() else {
        panic!("expected table-family CRC-key projection shape for `SpellDataManager`");
    };
    assert_eq!(shape.key_column().as_str(), "SpellID");
    assert!(shape.skip_empty_key());
    assert!(shape.reject_zero_crc());
    assert_eq!(
        shape.duplicate_key_policy(),
        NativeDuplicateKeyPolicy::FirstWins
    );
    assert_eq!(
        shape.source_handle_field().map(RustIdentifier::as_str),
        Some("source")
    );
    assert!(shape.source_handle_method().is_none());
    assert!(shape.source_row_field().is_none());
    assert_eq!(
        shape.ids_method().map(RustIdentifier::as_str),
        Some("spell_ids")
    );

    let timeline_registry = find_manager(&managers, "crate::TimelineRegistryManager");
    assert_manager_has_table(timeline_registry, "GenericTimelineRegistryEntry");
    assert_manager_has_table(timeline_registry, "TimelineRegistryEntry");
    assert_manager_has_table(timeline_registry, "WhisperTimelineRegistryEntry");
    let Some(NativeManagerShape::TableFamilyCrcKeyProjection(shape)) = timeline_registry.shape()
    else {
        panic!("expected table-family CRC-key projection shape for `TimelineRegistryManager`");
    };
    assert_eq!(shape.key_column().as_str(), "TimelineEntryName");
    assert!(shape.skip_empty_key());
    assert!(!shape.reject_zero_crc());
    assert!(!shape.store_key_text());
    assert_eq!(
        shape.source_handle_field().map(RustIdentifier::as_str),
        Some("source")
    );
    assert_eq!(
        shape
            .fields()
            .iter()
            .map(|field| (field.field().as_str(), field.transform()))
            .collect::<Vec<_>>(),
        [(
            "timeline_asset_path",
            NativeProjectionTransform::LowercaseCrcString
        )]
    );
    assert_eq!(shape.table_indexes().len(), 2);
    assert_eq!(
        shape.table_indexes()[0].index_field().as_str(),
        "generic_entries_by_name"
    );
    assert_eq!(
        shape.table_indexes()[0].table_variant().as_str(),
        "GenericTimelineRegistryEntry"
    );
    assert_eq!(
        shape.table_indexes()[0].duplicate_key_policy(),
        NativeDuplicateKeyPolicy::FirstWins
    );
    assert_eq!(
        shape.table_indexes()[0].methods()[0].name().as_str(),
        "generic_timeline_registry_entry"
    );
    assert_eq!(
        shape.table_indexes()[1].index_field().as_str(),
        "whisper_entries_by_name"
    );
    assert_eq!(
        shape.table_indexes()[1].table_variant().as_str(),
        "WhisperTimelineRegistryEntry"
    );
    assert_eq!(
        shape.table_indexes()[1].methods()[0].name().as_str(),
        "whisper_timeline_registry_entry"
    );

    let camera_shake = find_manager(&managers, "crate::CameraShakeDataManager");
    assert_manager_has_table(camera_shake, "CameraShakeDataTable");
    assert_manager_has_table(camera_shake, "CameraShakeDataTable_IsleOfNight");
    let Some(NativeManagerShape::TableFamilyPartitionedCrcKeyProjection(shape)) =
        camera_shake.shape()
    else {
        panic!(
            "expected table-family partitioned CRC-key projection shape for `CameraShakeDataManager`"
        );
    };
    assert_eq!(shape.key_column().as_str(), "CameraShakeID");
    assert!(shape.skip_empty_key());
    assert!(!shape.trim_key());
    assert!(!shape.reject_zero_crc());
    assert_eq!(shape.table_indexes().len(), 2);
    assert_eq!(
        shape.table_indexes()[0].index_field().as_str(),
        "base_camera_shakes_by_crc"
    );
    assert_eq!(shape.table_indexes()[0].table_variant().as_str(), "Table");
    assert_eq!(
        shape.table_indexes()[0].duplicate_key_policy(),
        NativeDuplicateKeyPolicy::Overwrite
    );
    assert_eq!(shape.table_indexes()[0].methods().len(), 3);
    assert_eq!(
        shape.table_indexes()[0].methods()[0].name().as_str(),
        "camera_shake_data_from_id"
    );
    assert_eq!(
        shape.table_indexes()[1].index_field().as_str(),
        "isle_of_night_camera_shakes_by_crc"
    );
    assert_eq!(
        shape.table_indexes()[1].table_variant().as_str(),
        "TableIsleOfNight"
    );
    assert!(shape.table_indexes()[1].methods().is_empty());
    assert_eq!(
        shape
            .vec3_fields()
            .iter()
            .map(|field| field.field().as_str())
            .collect::<Vec<_>>(),
        ["shake_shift", "shake_angle"]
    );
    assert_eq!(
        shape
            .fields()
            .iter()
            .map(|field| (field.field().as_str(), field.transform()))
            .collect::<Vec<_>>(),
        [
            ("sustain_duration", NativeProjectionTransform::F32),
            ("fade_in_duration", NativeProjectionTransform::F32),
            ("fade_out_duration", NativeProjectionTransform::F32),
            ("frequency", NativeProjectionTransform::F32),
            ("randomness", NativeProjectionTransform::F32),
            ("shake_channel", NativeProjectionTransform::U32),
            ("flip_vec", NativeProjectionTransform::NonZeroU32),
            ("update_only", NativeProjectionTransform::Bool),
            ("permanent", NativeProjectionTransform::U32),
            ("is_smooth", NativeProjectionTransform::Bool),
        ]
    );

    let shop_data = find_manager(&managers, "crate::ShopDataManager");
    assert_mixed_table_manager_dependencies(shop_data, &["ShopData"], &["crate::NPCDataManager"]);
    let Some(NativeManagerShape::OneTableCrcKeyProjection(shape)) = shop_data.shape() else {
        panic!("expected CRC-key projection shape for `ShopDataManager`");
    };
    assert_eq!(shape.key_column().as_str(), "ShopId");
    assert!(shape.skip_empty_key());
    assert!(!shape.reject_zero_crc());
    assert_eq!(
        shape.duplicate_key_policy(),
        NativeDuplicateKeyPolicy::FirstWins
    );
    assert_eq!(
        shape.source_row_method().map(RustIdentifier::as_str),
        Some("source_row")
    );
    assert_eq!(
        shape
            .fields()
            .iter()
            .map(|field| (field.field().as_str(), field.transform()))
            .collect::<Vec<_>>(),
        [
            ("progression_id", NativeProjectionTransform::String),
            ("shop_name", NativeProjectionTransform::String),
            ("display_on_marker", NativeProjectionTransform::Bool),
            ("display_on_compass", NativeProjectionTransform::Bool),
            ("display_on_map", NativeProjectionTransform::Bool),
            ("display_progress_panel", NativeProjectionTransform::Bool),
            ("wallet_display_gold", NativeProjectionTransform::Bool),
            ("wallet_display_azoth", NativeProjectionTransform::Bool),
            (
                "wallet_display_player_level",
                NativeProjectionTransform::Bool
            ),
        ]
    );
    assert_eq!(shape.dependency_lookup_methods().len(), 1);
    let shop_from_npc = &shape.dependency_lookup_methods()[0];
    assert_eq!(shop_from_npc.name().as_str(), "shop_data_from_npc_id");
    assert_eq!(
        shop_from_npc.dependency_type().as_str(),
        "crate::NPCDataManager"
    );
    assert_eq!(shop_from_npc.dependency_parameter().as_str(), "npc_data");
    assert_eq!(shop_from_npc.key_parameter().name().as_str(), "npc_id");
    assert_eq!(
        shop_from_npc.key_parameter().kind(),
        NativeCrcIndexLookupParameterKind::Crc32
    );
    assert_eq!(shop_from_npc.dependency_method().as_str(), "shop_id");
    assert_eq!(shop_from_npc.lookup_method().as_str(), "shop_data");

    let progression_pool = find_manager(&managers, "crate::ProgressionPoolDataManager");
    assert_generated_table_manager(progression_pool, "ProgressionPools");
    let Some(NativeManagerShape::OneTableCrcKeyProjection(shape)) = progression_pool.shape() else {
        panic!("expected CRC-key projection shape for `ProgressionPoolDataManager`");
    };
    assert_eq!(shape.key_column().as_str(), "ProgressionPoolId");
    assert!(shape.skip_empty_key());
    assert!(!shape.reject_zero_crc());
    assert_eq!(
        shape.duplicate_key_policy(),
        NativeDuplicateKeyPolicy::FirstWins
    );
    assert_eq!(
        shape.key_wrapper_type().map(RustIdentifier::as_str),
        Some("ProgressionPoolId")
    );
    assert_eq!(
        shape.crc_ids_method().map(RustIdentifier::as_str),
        Some("pool_ids")
    );
    assert_eq!(
        shape
            .fields()
            .iter()
            .map(|field| (
                field.field().as_str(),
                field.transform(),
                field.value_type().map(RustTypePath::as_str)
            ))
            .collect::<Vec<_>>(),
        [
            (
                "category",
                NativeProjectionTransform::EnumStringRejectDefault,
                Some("newworld_plugin::game_data::PoolCategory")
            ),
            ("point_cap", NativeProjectionTransform::NonZeroU32, None),
            (
                "initial_points",
                NativeProjectionTransform::OptionalU32DefaultZero,
                None
            ),
            ("version_number", NativeProjectionTransform::U32, None),
        ]
    );
    let pug_activity_info = find_manager(&managers, "crate::PugActivityInfoDataManager");
    assert_generated_table_manager(pug_activity_info, "PUGActivityInfo");
    let Some(NativeManagerShape::OneTableCrcKeyProjection(shape)) = pug_activity_info.shape()
    else {
        panic!("expected CRC-key projection shape for `PugActivityInfoDataManager`");
    };
    assert_eq!(shape.key_column().as_str(), "GameModeId");
    assert_eq!(
        shape.duplicate_key_policy(),
        NativeDuplicateKeyPolicy::FirstWins
    );
    assert!(shape.skip_empty_key());
    assert!(!shape.reject_zero_crc());
    assert_eq!(
        shape
            .fields()
            .iter()
            .map(|field| (field.field().as_str(), field.transform()))
            .collect::<Vec<_>>(),
        [
            ("name", NativeProjectionTransform::String),
            ("description", NativeProjectionTransform::String),
            ("icon_path", NativeProjectionTransform::String),
            ("is_pvp", NativeProjectionTransform::Bool),
            ("min_group_size", NativeProjectionTransform::U32),
            ("max_group_size", NativeProjectionTransform::U32),
            ("require_azoth_staff", NativeProjectionTransform::Bool),
            ("require_local_group", NativeProjectionTransform::Bool),
            ("require_raid_group", NativeProjectionTransform::Bool),
            ("icon_path_small", NativeProjectionTransform::OptionalString),
            ("panel_icon_path", NativeProjectionTransform::OptionalString),
            ("panel_name", NativeProjectionTransform::OptionalString),
            ("panel_sub_text", NativeProjectionTransform::OptionalString),
            (
                "panel_description",
                NativeProjectionTransform::OptionalString
            ),
        ]
    );
    let pug_rewards = find_manager(&managers, "crate::PugRewardsDataManager");
    assert_generated_table_manager(pug_rewards, "PUGRewards");
    let Some(NativeManagerShape::OneTableCrcKeyProjection(shape)) = pug_rewards.shape() else {
        panic!("expected CRC-key projection shape for `PugRewardsDataManager`");
    };
    assert_eq!(shape.key_column().as_str(), "IncentiveID");
    assert!(shape.skip_empty_key());
    assert!(!shape.reject_zero_crc());
    assert_eq!(
        shape.duplicate_key_policy(),
        NativeDuplicateKeyPolicy::FirstWins
    );
    assert_eq!(shape.row_filters().len(), 1);
    assert_eq!(shape.row_filters()[0].column().as_str(), "IsDisabled");
    assert_eq!(
        shape.row_filters()[0].predicate(),
        NativeCrcProjectionRowFilterPredicate::BoolTrueWhenPresent
    );
    assert_eq!(
        shape
            .fields()
            .iter()
            .map(|field| (field.field().as_str(), field.transform()))
            .collect::<Vec<_>>(),
        [
            ("activity_types", NativeProjectionTransform::CrcList),
            ("incentive_type", NativeProjectionTransform::String),
            ("role", NativeProjectionTransform::OptionalString),
            (
                "reward_below_max_lvl_game_event",
                NativeProjectionTransform::LowercaseCrcString
            ),
            (
                "reward_below_max_lvl_game_event_key",
                NativeProjectionTransform::String
            ),
            (
                "reward_at_max_lvl_game_event",
                NativeProjectionTransform::LowercaseCrcString
            ),
            (
                "reward_at_max_lvl_game_event_key",
                NativeProjectionTransform::String
            ),
            ("daily_limit", NativeProjectionTransform::U32),
        ]
    );
    let reward_data = find_manager(&managers, "crate::RewardDataManager");
    assert_generated_table_manager(reward_data, "Rewards");
    let Some(NativeManagerShape::OneTableNumericKeyProjection(shape)) = reward_data.shape() else {
        panic!("expected numeric-key projection shape for `RewardDataManager`");
    };
    assert_eq!(shape.key_column().as_str(), "Level");
    assert_eq!(
        shape.duplicate_key_policy(),
        NativeDuplicateKeyPolicy::FirstWins
    );
    assert_eq!(
        shape
            .fields()
            .iter()
            .map(|field| (field.field().as_str(), field.transform()))
            .collect::<Vec<_>>(),
        [
            ("kill_experience", NativeProjectionTransform::NonZeroU32),
            ("kill_currency", NativeProjectionTransform::NonZeroU32),
            ("kill_azoth", NativeProjectionTransform::U32),
            ("quest_experience", NativeProjectionTransform::NonZeroU32),
            ("quest_currency", NativeProjectionTransform::NonZeroU32),
            ("quest_azoth", NativeProjectionTransform::U32),
            ("darkness_major", NativeProjectionTransform::U32),
            ("darkness_minor", NativeProjectionTransform::U32),
            ("war_experience", NativeProjectionTransform::NonZeroU32),
            ("war_currency", NativeProjectionTransform::NonZeroU32),
            ("war_azoth", NativeProjectionTransform::NonZeroU32),
            ("war_faction_tokens", NativeProjectionTransform::NonZeroU32),
            ("war_azoth_salt", NativeProjectionTransform::NonZeroU32),
            ("war_pvp_xp", NativeProjectionTransform::NonZeroU32),
            (
                "war_faction_reputation",
                NativeProjectionTransform::NonZeroU32
            ),
            (
                "war_territory_standing",
                NativeProjectionTransform::NonZeroU32
            ),
            (
                "kill_progression_currency",
                NativeProjectionTransform::NonZeroU32
            ),
            (
                "kill_experience_dungeon_boss",
                NativeProjectionTransform::NonZeroU32
            ),
            (
                "kill_currency_dungeon_boss",
                NativeProjectionTransform::NonZeroU32
            ),
            (
                "kill_experience_dungeon_mini_boss",
                NativeProjectionTransform::NonZeroU32
            ),
            (
                "kill_currency_dungeon_mini_boss",
                NativeProjectionTransform::NonZeroU32
            ),
            ("seasons_xp", NativeProjectionTransform::NonZeroU32),
        ]
    );
    let particle_data = find_manager(&managers, "crate::ParticleDataManager");
    assert_generated_table_manager(particle_data, "ParticleDataTable");
    let Some(NativeManagerShape::OneTableParticleData(shape)) = particle_data.shape() else {
        panic!("expected particle-data shape for `ParticleDataManager`");
    };
    assert_eq!(shape.key_column().as_str(), "Effect Name");
    assert_eq!(shape.key_getter().as_str(), "effect_name");
    assert_eq!(shape.group_column().as_str(), "Group");
    assert_eq!(shape.group_getter().as_str(), "group");
    assert_eq!(shape.max_number_column().as_str(), "Max Number");
    assert_eq!(shape.max_number_getter().as_str(), "max_number");
    assert_eq!(shape.priority_column().as_str(), "Priority");
    assert_eq!(shape.priority_getter().as_str(), "priority");
    assert_eq!(shape.constants_column().as_str(), "Constants");
    assert_eq!(shape.constants_getter().as_str(), "constants");
    assert_eq!(
        shape.lookup_from_id_method().as_str(),
        "particle_data_from_id"
    );
    assert_eq!(shape.lookup_method().as_str(), "particle_data");
    assert_eq!(
        shape.lookup_by_key_method().as_str(),
        "particle_data_by_key"
    );
    assert_eq!(
        shape.local_player_factor_method().as_str(),
        "local_player_factor"
    );
    assert_eq!(
        shape.max_total_number_emitters_method().as_str(),
        "max_total_number_emitters"
    );
    assert_eq!(
        shape.max_total_group_number_emitters_method().as_str(),
        "max_total_group_number_emitters"
    );
    let particle_priority = find_manager(&managers, "crate::ParticlePriorityOverrideDataManager");
    assert_generated_table_manager(particle_priority, "ParticleContextualPriorityOverrides");
    let Some(NativeManagerShape::OneTableCrcKeyProjection(shape)) = particle_priority.shape()
    else {
        panic!("expected CRC-key projection shape for `ParticlePriorityOverrideDataManager`");
    };
    assert_eq!(shape.key_column().as_str(), "EffectName");
    assert_eq!(shape.key_getter().as_str(), "effect_name");
    assert_eq!(shape.key_field().as_str(), "effect_name");
    assert_eq!(shape.crc_field().as_str(), "effect_id");
    assert_eq!(shape.hash_policy(), NativeCrcHashPolicy::Lowercase);
    assert_eq!(
        shape.key_storage_transform(),
        NativeCrcKeyStorageTransform::RemoveSpaceAndTab
    );
    assert!(shape.skip_empty_key());
    assert_eq!(
        shape.duplicate_key_policy(),
        NativeDuplicateKeyPolicy::FirstWins
    );
    assert_eq!(
        shape
            .fields()
            .iter()
            .map(|field| (field.field().as_str(), field.transform()))
            .collect::<Vec<_>>(),
        [("priority_override", NativeProjectionTransform::U8Enum)]
    );
    assert_eq!(
        shape.fields()[0].value_type().map(RustTypePath::as_str),
        Some("ParticlePriorityOverride")
    );
    let tutorials_condition = find_manager(&managers, "crate::PlayerTutorialsConditionDataManager");
    assert_generated_table_manager(tutorials_condition, "TutorialConditionData");
    let Some(NativeManagerShape::OneTableCrcKeyProjection(shape)) = tutorials_condition.shape()
    else {
        panic!("expected CRC-key projection shape for `PlayerTutorialsConditionDataManager`");
    };
    assert_eq!(shape.key_column().as_str(), "ConditionId");
    assert_eq!(shape.key_getter().as_str(), "condition_id");
    assert_eq!(
        shape.duplicate_key_policy(),
        NativeDuplicateKeyPolicy::FirstWins
    );
    assert_eq!(
        shape.source_row_field().map(RustIdentifier::as_str),
        Some("source_row")
    );
    assert_eq!(
        shape
            .fields()
            .iter()
            .map(|field| (field.field().as_str(), field.transform()))
            .collect::<Vec<_>>(),
        [
            ("operation", NativeProjectionTransform::OptionalString),
            ("player_level", NativeProjectionTransform::U32),
            (
                "categorical_progression",
                NativeProjectionTransform::OptionalForeignKeyRow
            ),
            ("game_event", NativeProjectionTransform::OptionalString),
            (
                "achievement",
                NativeProjectionTransform::OptionalForeignKeyRow
            ),
            ("entitlement", NativeProjectionTransform::OptionalString),
            ("item", NativeProjectionTransform::StringList),
            ("ui_event", NativeProjectionTransform::OptionalString),
            ("status_effects", NativeProjectionTransform::OptionalString),
            ("notes", NativeProjectionTransform::OptionalString),
        ]
    );
    let tutorials_content = find_manager(&managers, "crate::PlayerTutorialsContentDataManager");
    assert_generated_table_manager(tutorials_content, "TutorialContentData");
    let Some(NativeManagerShape::OneTableCrcKeyProjection(shape)) = tutorials_content.shape()
    else {
        panic!("expected CRC-key projection shape for `PlayerTutorialsContentDataManager`");
    };
    assert_eq!(shape.key_column().as_str(), "ContentId");
    assert_eq!(shape.key_getter().as_str(), "content_id");
    assert_eq!(
        shape
            .fields()
            .iter()
            .map(|field| (field.field().as_str(), field.transform()))
            .collect::<Vec<_>>(),
        [
            ("subtitle_text", NativeProjectionTransform::String),
            ("body_text", NativeProjectionTransform::String),
            (
                "keyboard_button_display_override",
                NativeProjectionTransform::OptionalString
            ),
            ("image_path", NativeProjectionTransform::OptionalString),
            ("icon_path", NativeProjectionTransform::OptionalString),
        ]
    );
    let tutorials = find_manager(&managers, "crate::PlayerTutorialsDataManager");
    assert_generated_table_manager(tutorials, "TutorialData");
    let Some(NativeManagerShape::OneTableCrcKeyProjection(shape)) = tutorials.shape() else {
        panic!("expected CRC-key projection shape for `PlayerTutorialsDataManager`");
    };
    assert_eq!(shape.key_column().as_str(), "TutorialId");
    assert_eq!(shape.key_getter().as_str(), "tutorial_id");
    assert_eq!(
        shape
            .fields()
            .iter()
            .map(|field| (field.field().as_str(), field.transform()))
            .collect::<Vec<_>>(),
        [
            ("type_", NativeProjectionTransform::String),
            (
                "prompt_content_ids",
                NativeProjectionTransform::OptionalForeignKeyRow
            ),
            (
                "dialogue_content_ids",
                NativeProjectionTransform::OptionalForeignKeyRowListDefaultEmpty
            ),
            (
                "condition_ids_and",
                NativeProjectionTransform::OptionalForeignKeyRow
            ),
            (
                "condition_ids_relation",
                NativeProjectionTransform::OptionalString
            ),
            (
                "condition_ids_or",
                NativeProjectionTransform::OptionalForeignKeyRow
            ),
            ("classification", NativeProjectionTransform::String),
            ("prompt_style", NativeProjectionTransform::OptionalString),
            ("title_text", NativeProjectionTransform::OptionalString),
            ("category", NativeProjectionTransform::OptionalString),
            ("cta_enabled", NativeProjectionTransform::Bool),
            (
                "exit_action_and_description",
                NativeProjectionTransform::OptionalString
            ),
            ("exit_duration", NativeProjectionTransform::OptionalF32),
            (
                "hidden_trigger_condition_id",
                NativeProjectionTransform::OptionalString
            ),
            ("ignore_combat_suppression", NativeProjectionTransform::Bool),
            ("reset_on_ftue_start", NativeProjectionTransform::Bool),
            ("search_keywords", NativeProjectionTransform::StringList),
        ]
    );
    let player_milestone_modal = find_manager(&managers, "crate::PlayerMilestoneModalDataManager");
    assert_generated_table_manager(player_milestone_modal, "PlayerMilestoneModals");
    let Some(NativeManagerShape::OneTableCrcKeyProjection(shape)) = player_milestone_modal.shape()
    else {
        panic!("expected CRC-key projection shape for `PlayerMilestoneModalDataManager`");
    };
    assert_eq!(shape.key_column().as_str(), "PlayerMilestoneModalId");
    assert!(shape.skip_empty_key());
    assert!(!shape.reject_zero_crc());
    assert_eq!(
        shape.duplicate_key_policy(),
        NativeDuplicateKeyPolicy::FirstWins
    );
    assert_eq!(
        shape
            .fields()
            .iter()
            .map(|field| (field.field().as_str(), field.transform()))
            .collect::<Vec<_>>(),
        [
            ("modal_screen_state", NativeProjectionTransform::String),
            (
                "modal_milestone_conditional",
                NativeProjectionTransform::String
            ),
        ]
    );
    assert_generated_table_manager(
        find_manager(&managers, "crate::ArmorAppearanceDataManager"),
        "ArmorAppearances",
    );
    assert_generated_table_manager(
        find_manager(&managers, "crate::AppearanceTransformDataManager"),
        "DefaultAppearanceTransforms",
    );
    assert_generated_table_manager(
        find_manager(&managers, "crate::CinematicVideoStaticDataManager"),
        "CinematicVideo",
    );
    assert_generated_table_manager(
        find_manager(&managers, "crate::CollectibleStaticDataManager"),
        "Collectibles",
    );
    assert_generated_table_manager(
        find_manager(&managers, "crate::FactionStatusEffectDataManager"),
        "FactionStatusEffect",
    );
    let game_event_data = find_manager(&managers, "crate::GameEventDataManager");
    assert!(game_event_data.shape().is_some());
    assert_manager_has_table(game_event_data, "GameEvents_01");
    assert_manager_has_table(game_event_data, "GameEventsDunwood");
    assert_generated_table_manager(
        find_manager(&managers, "crate::ArenaPvpBalanceDataManager"),
        "ArenaPvpBalanceTable",
    );
    assert_generated_table_manager(
        find_manager(&managers, "crate::CaptureTheFlagPvpBalanceDataManager"),
        "CaptureTheFlagPvpBalanceTable",
    );
    assert_generated_table_manager(
        find_manager(&managers, "crate::DuelPvpBalanceDataManager"),
        "DuelPvpBalanceTable",
    );
    assert_generated_table_manager(
        find_manager(&managers, "crate::FFAZonePvpBalanceDataManager"),
        "FFAZonePvpBalanceTable",
    );
    assert_generated_table_manager(
        find_manager(&managers, "crate::OpenWorldPvpBalanceDataManager"),
        "OpenWorldPvpBalanceTable",
    );
    assert_generated_table_manager(
        find_manager(&managers, "crate::OprPvpBalanceDataManager"),
        "OutpostRushPvpBalanceTable",
    );
    assert_generated_table_manager(
        find_manager(&managers, "crate::OutPostRushNoPerksPvpBalanceDataManager"),
        "OutpostRush_NoPerksPvpBalanceTable",
    );
    assert_generated_table_manager(
        find_manager(&managers, "crate::WarPvpBalanceDataManager"),
        "WarPvpBalanceTable",
    );
    assert_generated_table_manager(
        find_manager(&managers, "crate::PostSkillCapProgressionDataManager"),
        "TradeSkillPostCap",
    );
    let post_skill_cap = find_manager(&managers, "crate::PostSkillCapProgressionDataManager");
    let Some(NativeManagerShape::PostSkillCapProgression(shape)) = post_skill_cap.shape() else {
        panic!("expected post-skill-cap shape for `PostSkillCapProgressionDataManager`");
    };
    assert_eq!(shape.table_name().as_str(), "TradeSkillPostCap");
    assert_eq!(shape.row_type_name().as_str(), "TradeSkillPostCapData");
    assert_eq!(
        shape.data_type().as_str(),
        "StaticPostSkillCapProgressionData"
    );
    assert_eq!(
        shape.level_rewards_type().as_str(),
        "PostSkillCapLevelRewards"
    );
    assert_eq!(
        shape.cache_type().as_str(),
        "PostSkillCapProgressionIndexes"
    );
    assert_eq!(shape.cache_field().as_str(), "progression");
    assert_eq!(
        shape.lookup_method().as_str(),
        "post_skill_cap_progression_data"
    );
    assert_eq!(
        shape.lookup_from_id_method().as_str(),
        "post_skill_cap_progression_data_from_id"
    );
    assert_eq!(shape.rows_method().as_str(), "entries");
    assert_progression_point_data_manager_shape(find_manager(
        &managers,
        "crate::ProgressionPointDataManager",
    ));
    let flexible_mission_board = find_manager(&managers, "crate::FlexibleMissionBoardDataManager");
    assert_generated_table_manager(flexible_mission_board, "FlexibleMissionBoardData");
    let Some(NativeManagerShape::OneTableCrcKeyProjection(shape)) = flexible_mission_board.shape()
    else {
        panic!("expected CRC-key projection shape for `FlexibleMissionBoardDataManager`");
    };
    assert_eq!(shape.key_column().as_str(), "FlexibleMissionBoardId");
    assert!(shape.reject_zero_crc());
    assert_eq!(
        shape.duplicate_key_policy(),
        NativeDuplicateKeyPolicy::FirstWins
    );
    assert_eq!(shape.row_filters().len(), 1);
    assert_eq!(
        shape.row_filters()[0].predicate(),
        NativeCrcProjectionRowFilterPredicate::LowercaseCrcStringNonZero
    );
    assert_eq!(
        shape
            .fields()
            .iter()
            .map(|field| (field.field().as_str(), field.transform()))
            .collect::<Vec<_>>(),
        [
            ("mission_board_name_key", NativeProjectionTransform::String),
            (
                "mission_board_name",
                NativeProjectionTransform::LowercaseCrcString
            ),
            ("reputation_key", NativeProjectionTransform::String),
            (
                "reputation_id",
                NativeProjectionTransform::LowercaseCrcString
            ),
            (
                "mission_objective_type_key",
                NativeProjectionTransform::String
            ),
            (
                "mission_objective_type",
                NativeProjectionTransform::LowercaseCrcString
            ),
            (
                "mission_weights_bucket_key",
                NativeProjectionTransform::String
            ),
            (
                "mission_weights_bucket",
                NativeProjectionTransform::LowercaseCrcString
            ),
            (
                "max_displayable_missions_count",
                NativeProjectionTransform::NonZeroU32
            ),
            (
                "daily_reward_modifier_key",
                NativeProjectionTransform::String
            ),
            (
                "daily_reward_modifier_id",
                NativeProjectionTransform::LowercaseCrcString
            ),
            ("daily_bonuses_count", NativeProjectionTransform::NonZeroU32),
            (
                "mission_refresh_interval_minutes",
                NativeProjectionTransform::NonZeroU32
            ),
            ("display_npc_shop_button", NativeProjectionTransform::Bool),
            ("rank_name_color", NativeProjectionTransform::String),
            (
                "reputation_bar_rank_icon",
                NativeProjectionTransform::OptionalString
            ),
            (
                "wallet_display_progression_row",
                NativeProjectionTransform::ForeignKeyRow
            ),
        ]
    );
    assert_generated_table_manager(
        find_manager(&managers, "crate::HunterSightDataManager"),
        "HunterSight",
    );
    let lore = find_manager(&managers, "crate::LoreDataManager");
    assert_generated_table_manager(lore, "Lore");
    let Some(NativeManagerShape::OneTableCrcKeyProjection(shape)) = lore.shape() else {
        panic!("expected CRC-key projection shape for `LoreDataManager`");
    };
    assert_eq!(shape.key_column().as_str(), "LoreID");
    assert_eq!(shape.key_getter().as_str(), "lore_id");
    assert!(shape.skip_empty_key());
    assert!(!shape.reject_zero_crc());
    assert!(shape.source_row_field().is_none());
    assert!(shape.source_row_method().is_none());
    assert_eq!(
        shape.duplicate_key_policy(),
        NativeDuplicateKeyPolicy::FirstWins
    );
    assert_eq!(
        shape
            .fields()
            .iter()
            .map(|field| (field.field().as_str(), field.transform()))
            .collect::<Vec<_>>(),
        [
            ("type_id", NativeProjectionTransform::LowercaseCrcString),
            ("type_key", NativeProjectionTransform::String),
            ("title", NativeProjectionTransform::String),
            ("subtitle", NativeProjectionTransform::OptionalString),
            ("body", NativeProjectionTransform::OptionalString),
            (
                "achievement_row",
                NativeProjectionTransform::OptionalForeignKeyRow
            ),
            (
                "parent_id",
                NativeProjectionTransform::OptionalLowercaseCrcString
            ),
            ("parent_key", NativeProjectionTransform::OptionalString),
            ("order", NativeProjectionTransform::U32),
            ("image_path", NativeProjectionTransform::OptionalString),
            ("location_name", NativeProjectionTransform::OptionalString),
            (
                "location_xy",
                NativeProjectionTransform::F32ListDefaultEmpty
            ),
            ("associated_quests", NativeProjectionTransform::StringList),
            ("writer", NativeProjectionTransform::OptionalString),
            ("loc_notes", NativeProjectionTransform::OptionalString),
            (
                "recording_status",
                NativeProjectionTransform::OptionalString
            ),
            (
                "lore_notes_locations",
                NativeProjectionTransform::StringList
            ),
        ]
    );
    let leaderboard_data = find_manager(&managers, "crate::LeaderboardDataManager");
    let shape = assert_one_table_crc_projection(
        leaderboard_data,
        "LeaderboardDataTable",
        "LeaderboardId",
        "leaderboards",
        "leaderboard_ids",
        "leaderboards",
    );
    assert_eq!(
        shape
            .fields()
            .iter()
            .map(|field| (field.field().as_str(), field.transform()))
            .collect::<Vec<_>>(),
        [
            (
                "leaderboard_definition_id",
                NativeProjectionTransform::String
            ),
            (
                "faction_leaderboard_definition_id",
                NativeProjectionTransform::OptionalString
            ),
            ("category", NativeProjectionTransform::String),
            ("display_name", NativeProjectionTransform::String),
            (
                "game_mode_crc",
                NativeProjectionTransform::OptionalLowercaseCrcString
            ),
            ("character_leaderboard", NativeProjectionTransform::Bool),
            ("group_leaderboard", NativeProjectionTransform::Bool),
            ("company_leaderboard", NativeProjectionTransform::Bool),
            ("faction_leaderboard", NativeProjectionTransform::Bool),
        ]
    );
    let leaderboard_rewards = find_manager(&managers, "crate::LeaderboardRewardsDataManager");
    assert_generated_table_manager(leaderboard_rewards, "LeaderboardRewardsDataTable");
    let Some(NativeManagerShape::OneTableCrcKeyProjection(shape)) = leaderboard_rewards.shape()
    else {
        panic!("expected CRC-key projection shape for `LeaderboardRewardsDataManager`");
    };
    assert_eq!(shape.key_column().as_str(), "LeaderboardRewardId");
    assert!(shape.skip_empty_key());
    assert!(!shape.reject_zero_crc());
    assert_eq!(
        shape.duplicate_key_policy(),
        NativeDuplicateKeyPolicy::FirstWins
    );
    assert_eq!(
        shape.source_row_method().map(RustIdentifier::as_str),
        Some("leaderboard_reward_for_source_row")
    );
    assert_eq!(
        shape
            .fields()
            .iter()
            .map(|field| (
                field.field().as_str(),
                field.transform(),
                field.value_type().map(RustTypePath::as_str)
            ))
            .collect::<Vec<_>>(),
        [
            (
                "entitlement_reward_id",
                NativeProjectionTransform::OptionalCrc32ZeroAsNone,
                None,
            ),
            (
                "rotation",
                NativeProjectionTransform::U8Enum,
                Some("newworld_plugin::game_data::LeaderboardRotations"),
            ),
            (
                "rotation_start",
                NativeProjectionTransform::NonZeroU32,
                None
            ),
            (
                "reward_id_no_rotation",
                NativeProjectionTransform::Crc32,
                None,
            ),
        ]
    );
    let expansion = find_manager(&managers, "crate::ExpansionDataManager");
    assert_generated_table_manager(expansion, "Expansions");
    let Some(NativeManagerShape::OneTableEnumKeyProjection(shape)) = expansion.shape() else {
        panic!("expected enum-key projection shape for `ExpansionDataManager`");
    };
    assert_eq!(shape.key_column().as_str(), "ExpansionId");
    assert_eq!(
        shape.key_type().as_str(),
        "newworld_plugin::game_data::ExpansionId"
    );
    assert!(shape.skip_empty_key());
    assert!(shape.trim_key());
    assert_eq!(
        shape.duplicate_key_policy(),
        NativeDuplicateKeyPolicy::FirstWins
    );
    assert_eq!(
        shape.source_row_field().map(RustIdentifier::as_str),
        Some("source_row")
    );
    assert!(shape.source_row_method().is_none());
    let secondary_crc = shape
        .secondary_crc_index()
        .expect("ExpansionDataManager CRC lookup index");
    assert_eq!(secondary_crc.index_field().as_str(), "expansions_by_crc");
    assert_eq!(secondary_crc.crc_field().as_str(), "expansion_id_crc");
    assert_eq!(
        secondary_crc
            .methods()
            .iter()
            .map(|method| {
                (
                    method.name().as_str(),
                    method.parameter().name().as_str(),
                    method.parameter().kind(),
                )
            })
            .collect::<Vec<_>>(),
        [
            (
                "expansion_data",
                "expansion_id",
                NativeCrcIndexLookupParameterKind::IntoCrc32,
            ),
            (
                "expansion_data_by_key",
                "key",
                NativeCrcIndexLookupParameterKind::AsRefStr,
            ),
        ]
    );
    assert_eq!(
        shape
            .fields()
            .iter()
            .map(|field| (
                field.field().as_str(),
                field.transform(),
                field.value_type().map(RustTypePath::as_str)
            ))
            .collect::<Vec<_>>(),
        [
            ("display_name", NativeProjectionTransform::String, None),
            ("icon", NativeProjectionTransform::String, None),
            (
                "max_display_level",
                NativeProjectionTransform::TypedCell,
                Some("std::num::NonZeroU16"),
            ),
            (
                "max_craft_gs",
                NativeProjectionTransform::TypedCell,
                Some("std::num::NonZeroU16"),
            ),
            (
                "max_equip_gs",
                NativeProjectionTransform::TypedCell,
                Some("std::num::NonZeroU16"),
            ),
            (
                "max_tradeskill_level",
                NativeProjectionTransform::TypedCell,
                Some("std::num::NonZeroU16"),
            ),
            (
                "entitlement_id",
                NativeProjectionTransform::OptionalCrc32,
                None,
            ),
        ]
    );
    let equipment_sets = find_manager(&managers, "crate::EquipmentSetDataManager");
    assert_manager_has_table(equipment_sets, "EquipmentSets");
    assert_manager_has_table(equipment_sets, "MasterItemDefinitions_AI");
    assert_manager_dependencies(equipment_sets, &[]);
    let leaderboard_stats = find_manager(&managers, "crate::LeaderboardStatDataManager");
    assert_mixed_table_manager_dependencies(
        leaderboard_stats,
        &["LeaderboardStatDataTable", "CategoricalProgression"],
        &[],
    );
    let shape = assert_one_table_crc_projection(
        leaderboard_stats,
        "LeaderboardStatDataTable",
        "LeaderboardStatId",
        "stats",
        "leaderboard_stat_ids",
        "stats",
    );
    assert_eq!(
        shape
            .fields()
            .iter()
            .map(|field| (field.field().as_str(), field.transform()))
            .collect::<Vec<_>>(),
        [
            ("service_stat_id", NativeProjectionTransform::String),
            (
                "categorical_progression_crc",
                NativeProjectionTransform::OptionalLowercaseCrcString
            ),
            (
                "game_mode_crc",
                NativeProjectionTransform::OptionalLowercaseCrcString
            ),
            ("personal_best", NativeProjectionTransform::OptionalString),
        ]
    );
    let mission_weights = find_manager(&managers, "crate::MissionWeightsDataManager");
    assert_mixed_table_manager_dependencies(
        mission_weights,
        &["FlexibleMissionWeights", "MissionWeights"],
        &[],
    );
    let Some(NativeManagerShape::TableFamilyCrcKeyProjection(shape)) = mission_weights.shape()
    else {
        panic!("expected table-family CRC-key projection shape for `MissionWeightsDataManager`");
    };
    assert_eq!(shape.key_column().as_str(), "MissionWeightId");
    assert!(shape.skip_empty_key());
    assert_eq!(
        shape.duplicate_key_policy(),
        NativeDuplicateKeyPolicy::FirstWins
    );
    assert_eq!(
        shape.ids_method().map(RustIdentifier::as_str),
        Some("mission_weight_ids")
    );
    assert_eq!(
        shape
            .fields()
            .iter()
            .map(|field| (
                field.field().as_str(),
                field.transform(),
                field.value_type().map(RustTypePath::as_str)
            ))
            .collect::<Vec<_>>(),
        [
            ("slot_number", NativeProjectionTransform::NonZeroU32, None),
            ("bucket_id", NativeProjectionTransform::String, None),
            (
                "bucket_crc",
                NativeProjectionTransform::LowercaseCrcString,
                None
            ),
            (
                "mission_goal_type",
                NativeProjectionTransform::EnumString,
                Some("newworld_plugin::game_data::MissionGoalType")
            ),
            ("weight", NativeProjectionTransform::NonZeroU32, None),
        ]
    );
    assert_mixed_table_manager_dependencies(
        find_manager(&managers, "crate::RotationalQueueDataManager"),
        &["RotationalQueue", "PUGActivityInfo"],
        &[],
    );
    assert_mixed_table_manager_dependencies(
        find_manager(&managers, "crate::ReusableScoreboardDataManager"),
        &["PUGActivityInfo", "ReusableScoreboard"],
        &[],
    );
    let schedule = find_manager(&managers, "crate::ScheduleDataManager");
    assert_generated_table_manager(schedule, "Schedules");
    let Some(NativeManagerShape::OneTableCrcKeyProjection(shape)) = schedule.shape() else {
        panic!("expected CRC-key projection shape for `ScheduleDataManager`");
    };
    assert_eq!(shape.key_column().as_str(), "ScheduleId");
    assert!(shape.reject_zero_crc());
    assert_eq!(
        shape.duplicate_key_policy(),
        NativeDuplicateKeyPolicy::FirstWins
    );
    assert_manager_has_table(
        find_manager(&managers, "crate::StatModifierDataManager"),
        "ConsumableItemDefinitions",
    );
    assert_manager_has_table(
        find_manager(&managers, "crate::StatModifierDataManager"),
        "StatusEffects_Warhammer",
    );
    assert_manager_has_table(
        find_manager(&managers, "crate::StatModifierDataManager"),
        "BaseVitals_Player",
    );
    assert_manager_has_table(
        find_manager(&managers, "crate::StatModifierDataManager"),
        "FishingPolesMastersheet",
    );
    let mutation_difficulty = find_manager(&managers, "crate::MutationDifficultyStaticDataManager");
    assert_generated_table_manager(mutation_difficulty, "MutationDifficulty");
    let Some(NativeManagerShape::OneTableNumericKeyProjection(shape)) = mutation_difficulty.shape()
    else {
        panic!("expected numeric-key projection shape for `MutationDifficultyStaticDataManager`");
    };
    assert_eq!(shape.key_type(), NativeNumericKeyType::NonZeroU8);
    assert_eq!(shape.key_column().as_str(), "MutationDifficulty");
    assert_eq!(
        shape.duplicate_key_policy(),
        NativeDuplicateKeyPolicy::FirstWins
    );
    assert_eq!(shape.fields().len(), 50);
    assert!(shape.fields().iter().any(|field| {
        field.field().as_str() == "loot_gs_range_override"
            && field.transform() == NativeProjectionTransform::U32RangeInclusive
    }));
    assert!(shape.fields().iter().any(|field| {
        field.field().as_str() == "difficulty_tier"
            && field.transform() == NativeProjectionTransform::TypedCell
    }));
    assert_eq!(shape.methods().len(), 1);
    assert_eq!(
        shape.methods()[0].parameter_kind(),
        NativeNumericLookupParameterKind::NonZeroU8
    );
    let mutation_perks = find_manager(&managers, "crate::ElementalMutationPerksStaticDataManager");
    assert_generated_table_manager(mutation_perks, "MutationPerks");
    let Some(NativeManagerShape::OneTableCrcKeyProjection(shape)) = mutation_perks.shape() else {
        panic!("expected CRC-key projection shape for `ElementalMutationPerksStaticDataManager`");
    };
    assert_eq!(shape.key_column().as_str(), "ElementalMutationTypeId");
    assert!(shape.reject_zero_crc());
    assert_eq!(
        shape
            .fields()
            .iter()
            .map(|field| (field.field().as_str(), field.transform()))
            .collect::<Vec<_>>(),
        [
            (
                "injected_perk_bucket1_key",
                NativeProjectionTransform::OptionalString
            ),
            (
                "injected_perk_bucket1",
                NativeProjectionTransform::OptionalLowercaseCrcString
            ),
            (
                "injected_perk_bucket_weight1",
                NativeProjectionTransform::OptionalF32
            ),
            (
                "injected_perk_bucket2_row",
                NativeProjectionTransform::ForeignKeyRow
            ),
            (
                "injected_perk_bucket_weight2",
                NativeProjectionTransform::OptionalF32
            ),
            (
                "injected_perk_bucket3_key",
                NativeProjectionTransform::OptionalString
            ),
            (
                "injected_perk_bucket3",
                NativeProjectionTransform::OptionalLowercaseCrcString
            ),
            (
                "injected_perk_bucket_weight3",
                NativeProjectionTransform::OptionalF32
            ),
            (
                "injected_perk_bucket4_key",
                NativeProjectionTransform::OptionalString
            ),
            (
                "injected_perk_bucket4",
                NativeProjectionTransform::OptionalLowercaseCrcString
            ),
            (
                "injected_perk_bucket_weight4",
                NativeProjectionTransform::OptionalF32
            ),
            (
                "injected_perk_bucket5_key",
                NativeProjectionTransform::OptionalString
            ),
            (
                "injected_perk_bucket5",
                NativeProjectionTransform::OptionalLowercaseCrcString
            ),
            (
                "injected_perk_bucket_weight5",
                NativeProjectionTransform::OptionalF32
            ),
            ("injected_creature_loot", NativeProjectionTransform::String),
            ("injected_container_loot", NativeProjectionTransform::String),
            ("injected_loot_tags", NativeProjectionTransform::String),
            ("name", NativeProjectionTransform::String),
            ("description", NativeProjectionTransform::String),
        ]
    );
    assert_elemental_mutation_static_data_manager_shape(find_manager(
        &managers,
        "crate::ElementalMutationStaticDataManager",
    ));
    assert_promotion_mutation_static_data_manager_shape(find_manager(
        &managers,
        "crate::PromotionMutationStaticDataManager",
    ));
    assert_mixed_table_manager_dependencies(
        find_manager(&managers, "crate::SimpleTreeCategoryDataManager"),
        &[
            "MetaAchievementCategoryDataTable",
            "PlayerTitleCategoryDataTable",
        ],
        &[],
    );
    assert_mixed_table_manager_dependencies(
        find_manager(&managers, "crate::StructureDataManager"),
        &["WallFootprint", "T0_Wall_Pieces"],
        &[],
    );
    assert_generated_table_manager(find_manager(&managers, "crate::MountDataManager"), "Mounts");
    assert_generated_table_manager(
        find_manager(&managers, "crate::MountDyeItemDataManager"),
        "MountDyeItemDefinitions",
    );
    assert_generated_table_manager(
        find_manager(&managers, "crate::MountItemAppearanceDataManager"),
        "MountItemAppearanceDefinitions",
    );
    assert_generated_table_manager(
        find_manager(&managers, "crate::MountTypeDataManager"),
        "MountTypes",
    );
    assert_generated_table_manager(
        find_manager(&managers, "crate::MusicalScoringDataManager"),
        "MusicalScoringTable",
    );
    let musical_ranking = find_manager(&managers, "crate::MusicalRankingDataManager");
    assert_generated_table_manager(musical_ranking, "MusicalRankingTable");
    let Some(NativeManagerShape::OneTableCrcKeyProjection(shape)) = musical_ranking.shape() else {
        panic!("expected CRC-key projection shape for `MusicalRankingDataManager`");
    };
    assert_eq!(shape.key_column().as_str(), "Grade");
    assert_eq!(
        shape.duplicate_key_policy(),
        NativeDuplicateKeyPolicy::FirstWins
    );
    assert_eq!(shape.descending_f32_indexes().len(), 1);
    let ranking_index = &shape.descending_f32_indexes()[0];
    assert_eq!(ranking_index.index_field().as_str(), "ranked_indices");
    assert_eq!(ranking_index.value_field().as_str(), "minimum_score");
    assert_eq!(
        ranking_index.rows_method().as_str(),
        "ranked_musical_rankings"
    );
    assert_eq!(
        ranking_index.threshold_lookup_method().as_str(),
        "musical_ranking_for_score"
    );
    let notification = find_manager(&managers, "crate::NotificationDataManager");
    assert_generated_table_manager(notification, "Notifications");
    let Some(NativeManagerShape::OneTableCrcKeyProjection(shape)) = notification.shape() else {
        panic!("expected CRC-key projection shape for `NotificationDataManager`");
    };
    assert_eq!(shape.key_column().as_str(), "NotificationId");
    assert_eq!(
        shape.duplicate_key_policy(),
        NativeDuplicateKeyPolicy::Overwrite
    );
    let number_fields = shape
        .fields()
        .iter()
        .find(|field| field.field().as_str() == "number_fields")
        .expect("number_fields projection");
    assert_eq!(
        number_fields.transform(),
        NativeProjectionTransform::OptionalCrcListDefaultEmpty
    );
    let title = find_manager(&managers, "crate::TitleDataManager");
    assert_generated_table_manager(title, "PlayerTitleDataTable");
    let Some(NativeManagerShape::OneTableCrcKeyProjection(shape)) = title.shape() else {
        panic!("expected CRC-key projection shape for `TitleDataManager`");
    };
    assert_eq!(shape.key_column().as_str(), "TitleID");
    assert_eq!(
        shape.duplicate_key_policy(),
        NativeDuplicateKeyPolicy::FirstWins
    );
    assert_eq!(
        shape.source_row_field().map(RustIdentifier::as_str),
        Some("source_row")
    );
    assert_eq!(
        shape.source_row_method().map(RustIdentifier::as_str),
        Some("source_row")
    );
    assert_eq!(
        shape.rows_method().map(RustIdentifier::as_str),
        Some("entries")
    );
    assert_eq!(
        shape
            .methods()
            .iter()
            .map(|method| method.name().as_str())
            .collect::<Vec<_>>(),
        ["title_data_by_crc32", "title_data"]
    );
    assert_eq!(
        shape
            .fields()
            .iter()
            .map(|field| (
                field.field().as_str(),
                field.transform(),
                field.value_type().map(RustTypePath::as_str)
            ))
            .collect::<Vec<_>>(),
        [
            (
                "title_type",
                NativeProjectionTransform::EnumStringRejectDefault,
                Some("newworld_plugin::game_data::TitleType")
            ),
            (
                "ui_display_category",
                NativeProjectionTransform::String,
                None
            ),
            ("title_male", NativeProjectionTransform::String, None),
            ("title_female", NativeProjectionTransform::String, None),
            ("title_neutral", NativeProjectionTransform::String, None),
            (
                "description",
                NativeProjectionTransform::OptionalString,
                None
            ),
            (
                "meta_achievement_rows",
                NativeProjectionTransform::OptionalForeignKeyRowListDefaultEmpty,
                None
            ),
            (
                "achievement_rows",
                NativeProjectionTransform::OptionalForeignKeyRowListDefaultEmpty,
                None
            ),
            (
                "categorical_progression_row",
                NativeProjectionTransform::OptionalForeignKeyRow,
                None
            ),
            (
                "required_categorical_progression_level",
                NativeProjectionTransform::U32,
                None
            ),
            (
                "required_player_level",
                NativeProjectionTransform::U32,
                None
            ),
        ]
    );
    assert_manager_has_table(
        find_manager(&managers, "crate::MountHitVolumeDataManager"),
        "MountTypes",
    );
    assert_mixed_table_manager_dependencies(
        find_manager(&managers, "crate::TradeskillRankDataManager"),
        &[
            "XPLevels",
            "Arcana",
            "Armoring",
            "AzothStaff",
            "Cooking",
            "Engineering",
            "Fishing",
            "Furnishing",
            "Harvesting",
            "Jewelcrafting",
            "Leatherworking",
            "Logging",
            "Mining",
            "Musician",
            "Riding",
            "Skinning",
            "Smelting",
            "Stonecutting",
            "Weaponsmithing",
            "Weaving",
            "Woodworking",
        ],
        &[],
    );
    assert_tradeskill_rank_data_manager_shape(find_manager(
        &managers,
        "crate::TradeskillRankDataManager",
    ));
    assert_static_tradeskill_rank_data_mapping_manager_shape(find_manager(
        &managers,
        "crate::StaticTradeskillRankDataMappingManager",
    ));
    let mount_movement = find_manager(&managers, "crate::MountMovementDataManager");
    assert_generated_table_manager(mount_movement, "MountsMovement");
    let Some(NativeManagerShape::OneTableCrcKeyProjection(shape)) = mount_movement.shape() else {
        panic!("expected CRC-key projection shape for `MountMovementDataManager`");
    };
    assert!(shape.schema_fields().is_some());
    let variation = find_manager(&managers, "crate::VariationDataManager");
    assert_manager_has_table(variation, "AI");
    assert_manager_has_table(variation, "HouseItems");
    let Some(NativeManagerShape::TableFamilyFallbackCrcKeyProjection(shape)) = variation.shape()
    else {
        panic!("expected fallback CRC-key projection shape for `VariationDataManager`");
    };
    assert_eq!(shape.module().as_str(), "variation_data");
    assert_eq!(shape.table_module().as_str(), "variation_data");
    assert_eq!(shape.tables_type().as_str(), "VariationDataTables");
    assert_eq!(shape.data_type().as_str(), "VariationData");
    assert_eq!(shape.key_kind_field().as_str(), "key_kind");
    assert_eq!(shape.key_kind_type().as_str(), "VariationDataKeyKind");
    assert_eq!(shape.primary_key_kind().as_str(), "VariantId");
    assert_eq!(shape.fallback_key_kind().as_str(), "HouseItemId");
    assert_eq!(shape.key_field().as_str(), "key");
    assert_eq!(shape.crc_field().as_str(), "key_id");
    assert_eq!(shape.primary_key_column().as_str(), "VariantID");
    assert_eq!(shape.primary_key_getter().as_str(), "variant_id");
    assert_eq!(shape.fallback_key_column().as_str(), "HouseItemID");
    assert_eq!(shape.fallback_key_getter().as_str(), "house_item_id");
    assert!(shape.skip_empty_key());
    assert_eq!(
        shape.duplicate_key_policy(),
        NativeDuplicateKeyPolicy::FirstWins
    );
    assert_eq!(
        shape
            .methods()
            .iter()
            .map(|method| (method.name().as_str(), method.parameter().name().as_str()))
            .collect::<Vec<_>>(),
        [
            ("variation_data_from_id", "id"),
            ("variation_data", "key"),
            ("variation_data_by_key", "key"),
        ]
    );
    assert_eq!(
        shape.rows_method().map(RustIdentifier::as_str),
        Some("rows")
    );
    assert_eq!(shape.len_method().map(RustIdentifier::as_str), Some("len"));
    assert_eq!(
        shape.is_empty_method().map(RustIdentifier::as_str),
        Some("is_empty")
    );
    let reward_milestone = find_manager(&managers, "crate::RewardMilestoneDataManager");
    assert_generated_table_manager(reward_milestone, "RewardMilestones");
    let Some(NativeManagerShape::OneTableCrcKeyProjection(shape)) = reward_milestone.shape() else {
        panic!("expected CRC-key projection shape for `RewardMilestoneDataManager`");
    };
    assert_eq!(shape.key_column().as_str(), "RewardID");
    assert!(!shape.skip_empty_key());
    assert!(shape.reject_zero_crc());
    assert_eq!(
        shape.duplicate_key_policy(),
        NativeDuplicateKeyPolicy::FirstWins
    );
    assert_eq!(
        shape.source_row_method().map(RustIdentifier::as_str),
        Some("reward_milestone_data_for_source_row")
    );
    assert_eq!(
        shape
            .fields()
            .iter()
            .map(|field| (
                field.field().as_str(),
                field.transform(),
                field.value_type().map(RustTypePath::as_str),
                field.default_value().map(RustPath::as_str)
            ))
            .collect::<Vec<_>>(),
        [
            (
                "milestone_type",
                NativeProjectionTransform::OptionalU8EnumDefaultValue,
                Some("RewardMilestoneType"),
                Some("RewardMilestoneType::None")
            ),
            (
                "milestone_level",
                NativeProjectionTransform::NonZeroU32,
                None,
                None
            ),
            ("name", NativeProjectionTransform::String, None, None),
            (
                "icon",
                NativeProjectionTransform::OptionalString,
                None,
                None
            ),
            (
                "image",
                NativeProjectionTransform::OptionalString,
                None,
                None
            ),
            (
                "tooltip",
                NativeProjectionTransform::OptionalString,
                None,
                None
            ),
            (
                "quest_name",
                NativeProjectionTransform::OptionalString,
                None,
                None
            ),
            (
                "expansion_id_unlock",
                NativeProjectionTransform::OptionalU8EnumDefaultValue,
                Some("newworld_plugin::game_data::ExpansionId"),
                Some("newworld_plugin::game_data::ExpansionId::None")
            ),
            (
                "notes",
                NativeProjectionTransform::OptionalString,
                None,
                None
            ),
        ]
    );
    let reward_modifier = find_manager(&managers, "crate::RewardModifierDataManager");
    assert_generated_table_manager(reward_modifier, "RewardModifiers");
    let Some(NativeManagerShape::OneTableCrcKeyProjection(shape)) = reward_modifier.shape() else {
        panic!("expected CRC-key projection shape for `RewardModifierDataManager`");
    };
    assert_eq!(shape.key_column().as_str(), "Modifiers");
    assert!(!shape.skip_empty_key());
    assert!(shape.reject_zero_crc());
    assert_eq!(
        shape.duplicate_key_policy(),
        NativeDuplicateKeyPolicy::FirstWins
    );
    assert_eq!(
        shape
            .fields()
            .iter()
            .map(|field| (field.field().as_str(), field.transform()))
            .collect::<Vec<_>>(),
        [
            ("experience", NativeProjectionTransform::F32),
            ("currency", NativeProjectionTransform::F32),
            ("currency_reward_chance", NativeProjectionTransform::F32),
            ("territory_standing", NativeProjectionTransform::F32),
            ("azoth", NativeProjectionTransform::F32),
            ("loot_drop_modifier", NativeProjectionTransform::F32),
            (
                "faction_reputation_modifier",
                NativeProjectionTransform::F32
            ),
            ("faction_token_modifier", NativeProjectionTransform::F32),
            (
                "progression_currency_amount",
                NativeProjectionTransform::OptionalF32
            ),
            ("azoth_salt_modifier", NativeProjectionTransform::F32),
            ("pvp_xp_modifier", NativeProjectionTransform::F32),
            ("seasons_xp_modifier", NativeProjectionTransform::F32),
            ("found_in", NativeProjectionTransform::StringList),
            ("display_name", NativeProjectionTransform::OptionalString),
        ]
    );
    assert_generated_table_manager(
        find_manager(&managers, "crate::StoreCategoryDataManager"),
        "StoreCategoryPropertiesTable",
    );
    let store_category = find_manager(&managers, "crate::StoreCategoryDataManager");
    let Some(NativeManagerShape::OneTableStoreCategory(shape)) = store_category.shape() else {
        panic!("expected one-table store-category shape for `StoreCategoryDataManager`");
    };
    assert_eq!(shape.table_name().as_str(), "StoreCategoryPropertiesTable");
    assert_eq!(shape.row_type_name().as_str(), "StoreCategoryProperties");
    assert_eq!(shape.tab_type().as_str(), "GameStoreTab");
    assert_eq!(
        shape.invalid_product_type().as_str(),
        "InvalidStoreProductType"
    );
    assert_eq!(shape.cache_type().as_str(), "StoreCategoryIndexes");
    assert_eq!(shape.cache_field().as_str(), "categories");
    assert_eq!(shape.num_categories_method().as_str(), "num_categories");
    assert_eq!(shape.rows_method().as_str(), "categories");
    assert_eq!(shape.lookup_method().as_str(), "store_category_properties");
    assert_eq!(
        shape.lookup_by_name_method().as_str(),
        "store_category_properties_by_name"
    );
    assert_eq!(
        shape.lookup_by_index_method().as_str(),
        "store_category_by_index"
    );
    assert_eq!(
        shape.product_type_lookup_method().as_str(),
        "category_for_product_type"
    );
    assert_generated_table_manager(
        find_manager(&managers, "crate::StoreProductDataManager"),
        "StoreProductData",
    );
    let store_product = find_manager(&managers, "crate::StoreProductDataManager");
    let Some(NativeManagerShape::OneTableStoreProduct(shape)) = store_product.shape() else {
        panic!("expected one-table store-product shape for `StoreProductDataManager`");
    };
    assert_eq!(shape.table_name().as_str(), "StoreProductData");
    assert_eq!(shape.row_type_name().as_str(), "StoreProductData");
    assert_eq!(
        shape.invalid_product_type().as_str(),
        "InvalidStoreProductDataProductType"
    );
    assert_eq!(shape.cache_type().as_str(), "StoreProductIndexes");
    assert_eq!(shape.cache_field().as_str(), "products");
    assert_eq!(shape.lookup_method().as_str(), "store_product_data");
    assert_eq!(
        shape.lookup_by_tag_method().as_str(),
        "store_product_data_by_tag"
    );
    assert_eq!(shape.rows_method().as_str(), "products");
    assert_generated_table_manager(
        find_manager(&managers, "crate::RewardTrackItemDataManager"),
        "RewardTrackItems",
    );
    let reward_track_item = find_manager(&managers, "crate::RewardTrackItemDataManager");
    let Some(NativeManagerShape::OneTableRewardTrackItem(shape)) = reward_track_item.shape() else {
        panic!("expected reward-track-item shape for `RewardTrackItemDataManager`");
    };
    assert_eq!(shape.table_name().as_str(), "RewardTrackItems");
    assert_eq!(shape.row_type_name().as_str(), "RewardTrackItemData");
    assert_eq!(shape.data_type().as_str(), "RewardTrackItemData");
    assert_eq!(shape.payload_type().as_str(), "RewardTrackItemPayload");
    assert_eq!(shape.cache_type().as_str(), "RewardTrackItemIndexes");
    assert_eq!(shape.cache_field().as_str(), "items");
    assert_eq!(
        shape.lookup_from_id_method().as_str(),
        "reward_track_item_data_from_id"
    );
    assert_eq!(shape.lookup_method().as_str(), "reward_track_item_data");
    assert_eq!(shape.rows_method().as_str(), "reward_track_items");
    let reward_track = find_manager(&managers, "crate::RewardTrackDataManager");
    let Some(NativeManagerShape::RewardTrackData(shape)) = reward_track.shape() else {
        panic!("expected reward-track shape for `RewardTrackDataManager`");
    };
    assert_eq!(shape.module().as_str(), "reward_track_data");
    assert_manager_has_table(reward_track, "PvPStore");
    assert_manager_has_table(reward_track, "RewardTrackItems");
    let quick_course = find_manager(&managers, "crate::QuickCourseDataManager");
    let Some(NativeManagerShape::QuickCourseData(shape)) = quick_course.shape() else {
        panic!("expected quick-course shape for `QuickCourseDataManager`");
    };
    assert_eq!(
        shape.quick_course_table_name().as_str(),
        "QuickCourse_Master"
    );
    assert_eq!(
        shape.quick_course_row_type_name().as_str(),
        "QuickCourseData"
    );
    assert_eq!(
        shape.node_type_table_name().as_str(),
        "QuickCourse_NodeTypes"
    );
    assert_eq!(
        shape.node_type_row_type_name().as_str(),
        "QuickCourseNodeTypeData"
    );
    assert_eq!(shape.data_type().as_str(), "QuickCourseData");
    assert_eq!(
        shape.node_type_data_type().as_str(),
        "QuickCourseNodeTypeData"
    );
    assert_eq!(shape.cache_type().as_str(), "QuickCourseIndexes");
    assert_eq!(shape.cache_field().as_str(), "indexes");
    assert_eq!(shape.quick_course_lookup_method().as_str(), "quick_course");
    assert_eq!(
        shape.quick_course_lookup_by_crc_method().as_str(),
        "quick_course_by_crc32"
    );
    assert_eq!(shape.quick_courses_method().as_str(), "quick_courses");
    assert_eq!(shape.node_type_lookup_method().as_str(), "node_type");
    assert_eq!(
        shape.node_type_lookup_by_crc_method().as_str(),
        "node_type_by_crc32"
    );
    assert_eq!(shape.node_types_method().as_str(), "node_types");
    let rotational_queue = find_manager(&managers, "crate::RotationalQueueDataManager");
    let Some(NativeManagerShape::RotationalQueueData(shape)) = rotational_queue.shape() else {
        panic!("expected rotational-queue shape for `RotationalQueueDataManager`");
    };
    assert_eq!(shape.queue_table_name().as_str(), "RotationalQueue");
    assert_eq!(shape.queue_row_type_name().as_str(), "RotationalQueueData");
    assert_eq!(shape.queue_table_field().as_str(), "queue_table");
    assert_eq!(shape.game_mode_table_name().as_str(), "PUGActivityInfo");
    assert_eq!(shape.game_mode_row_type_name().as_str(), "PUGActivityInfo");
    assert_eq!(shape.data_type().as_str(), "RotationalQueueStaticData");
    assert_eq!(shape.cache_type().as_str(), "RotationalQueueIndexes");
    assert_eq!(shape.cache_field().as_str(), "queues");
    assert_eq!(shape.lookup_method().as_str(), "rotational_queue");
    assert_eq!(
        shape.lookup_from_id_method().as_str(),
        "rotational_queue_from_id"
    );
    assert_eq!(shape.rows_method().as_str(), "rotational_queues");
    let whisper = find_manager(&managers, "crate::WhisperDataManager");
    let Some(NativeManagerShape::WhisperData(shape)) = whisper.shape() else {
        panic!("expected whisper-data shape for `WhisperDataManager`");
    };
    assert_eq!(shape.whisper_table_name().as_str(), "WhisperDataManager");
    assert_eq!(shape.whisper_row_type_name().as_str(), "WhisperData");
    assert_eq!(shape.vfx_table_name().as_str(), "WhisperVFXData");
    assert_eq!(shape.vfx_row_type_name().as_str(), "WhisperVfxData");
    assert_eq!(shape.cache_type().as_str(), "WhisperIndexes");
    assert_eq!(shape.cache_field().as_str(), "indexes");
    assert_eq!(shape.lookup_method().as_str(), "whisper_data");
    assert_eq!(shape.rows_method().as_str(), "whispers");
    assert_eq!(shape.vfx_for_method().as_str(), "whisper_vfx_for");
    let world_event_rule = find_manager(&managers, "crate::WorldEventRuleDataManager");
    let Some(NativeManagerShape::OneTableWorldEventRule(shape)) = world_event_rule.shape() else {
        panic!("expected world-event-rule shape for `WorldEventRuleDataManager`");
    };
    assert_eq!(shape.table_name().as_str(), "WorldEventRules");
    assert_eq!(shape.row_type_name().as_str(), "WorldEventRuleData");
    assert_eq!(shape.cache_type().as_str(), "WorldEventRuleIndexes");
    assert_eq!(shape.cache_field().as_str(), "rules");
    assert_eq!(shape.lookup_method().as_str(), "world_event_rule");
    assert_eq!(
        shape.lookup_by_crc_method().as_str(),
        "world_event_rule_by_crc32"
    );
    assert_eq!(shape.rows_method().as_str(), "world_event_rules");
    let story_progress = find_manager(&managers, "crate::StoryProgressDataManager");
    assert_generated_table_manager(story_progress, "StoryProgress");
    let Some(NativeManagerShape::OneTableRowProjection(shape)) = story_progress.shape() else {
        panic!("expected row-projection shape for `StoryProgressDataManager`");
    };
    assert_eq!(
        shape.source_row_field().map(RustIdentifier::as_str),
        Some("source_row")
    );
    assert_eq!(
        shape.source_row_method().map(RustIdentifier::as_str),
        Some("story_progress_for_source_row")
    );
    assert_eq!(
        shape.source_row_for_method().map(RustIdentifier::as_str),
        Some("source_row_for")
    );
    assert_eq!(
        shape.rows_method().map(RustIdentifier::as_str),
        Some("story_progress")
    );
    assert_eq!(
        shape
            .fields()
            .iter()
            .map(|field| (
                field.field().as_str(),
                field.transform(),
                field.reference_field().map(RustIdentifier::as_str)
            ))
            .collect::<Vec<_>>(),
        [
            ("achievement_ids", NativeProjectionTransform::CrcList, None),
            ("activity_task_name", NativeProjectionTransform::Crc32, None),
            (
                "has_activity_task_name",
                NativeProjectionTransform::Crc32NonZeroBool,
                Some("activity_task_name")
            ),
        ]
    );
    let pvp_rank = find_manager(&managers, "crate::PvpRankDataManager");
    assert_generated_table_manager(pvp_rank, "PvPXP");
    let Some(NativeManagerShape::OneTableNumericKeyProjection(shape)) = pvp_rank.shape() else {
        panic!("expected numeric-key projection shape for `PvpRankDataManager`");
    };
    assert_eq!(shape.key_column().as_str(), "Level");
    assert_eq!(shape.key_type(), NativeNumericKeyType::U16);
    assert_eq!(
        shape.duplicate_key_policy(),
        NativeDuplicateKeyPolicy::FirstWins
    );
    assert_eq!(
        shape.source_row_method().map(|method| method.as_str()),
        Some("pvp_rank_data_for_source_row")
    );
    assert_eq!(
        shape
            .fields()
            .iter()
            .map(|field| (field.field().as_str(), field.transform()))
            .collect::<Vec<_>>(),
        [
            (
                "display_name",
                NativeProjectionTransform::OptionalStringDefaultEmpty
            ),
            ("stage1_xp", NativeProjectionTransform::NonZeroU32),
            ("stage2_xp", NativeProjectionTransform::NonZeroU32),
            ("stage3_xp", NativeProjectionTransform::NonZeroU32),
            ("skip_stage1_salt", NativeProjectionTransform::NonZeroU32),
            ("skip_stage2_salt", NativeProjectionTransform::NonZeroU32),
            ("skip_stage3_salt", NativeProjectionTransform::NonZeroU32),
            ("azoth_salt_reward", NativeProjectionTransform::U32),
            (
                "game_event_id",
                NativeProjectionTransform::OptionalLowercaseCrcStringDefaultZero
            ),
            ("border_tier", NativeProjectionTransform::U32),
            (
                "reward_description",
                NativeProjectionTransform::OptionalStringDefaultEmpty
            ),
        ]
    );
    let stat_multiplier = find_manager(&managers, "crate::StatMultiplierDataManager");
    assert_generated_table_manager(stat_multiplier, "StatMultiplierTable");
    let Some(NativeManagerShape::OneTableEnumKeyProjection(shape)) = stat_multiplier.shape() else {
        panic!("expected enum-key projection shape for `StatMultiplierDataManager`");
    };
    assert_eq!(shape.key_column().as_str(), "ID");
    assert_eq!(
        shape.key_type().as_str(),
        "newworld_plugin::game_data::StatMultiplierType"
    );
    assert_eq!(
        shape
            .invalid_key_variants()
            .iter()
            .map(|variant| variant.as_str())
            .collect::<Vec<_>>(),
        ["Invalid", "StatMultiplierEnd"]
    );
    assert_eq!(
        shape
            .fields()
            .iter()
            .map(|field| (field.field().as_str(), field.transform()))
            .collect::<Vec<_>>(),
        [
            (
                "max",
                NativeProjectionTransform::F32UpperBound10000ZeroIsDefault,
            ),
            (
                "min",
                NativeProjectionTransform::F32LowerBound10000CappedToField,
            ),
        ]
    );
    assert_eq!(
        shape.fields()[1]
            .reference_field()
            .map(|field| field.as_str()),
        Some("max")
    );
    let loot_tag_preset = find_manager(&managers, "crate::LootTagPresetDataManager");
    assert_generated_table_manager(loot_tag_preset, "LootTagPresets");
    let Some(NativeManagerShape::OneTableCrcKeyProjection(shape)) = loot_tag_preset.shape() else {
        panic!("expected CRC-key projection shape for `LootTagPresetDataManager`");
    };
    assert_eq!(shape.key_column().as_str(), "LootTagPresetID");
    assert!(shape.skip_empty_key());
    assert!(shape.reject_zero_crc());
    assert_eq!(
        shape.duplicate_key_policy(),
        NativeDuplicateKeyPolicy::FirstWins
    );
    assert_eq!(
        shape.source_row_method().map(|method| method.as_str()),
        Some("loot_tag_preset_data_for_source_row")
    );
    assert_eq!(
        shape
            .fields()
            .iter()
            .map(|field| (field.field().as_str(), field.transform()))
            .collect::<Vec<_>>(),
        [(
            "loot_tags",
            NativeProjectionTransform::LowercaseCrcStringList,
        )]
    );
    let diminishing_returns = find_manager(&managers, "crate::DiminishingReturnsDataManager");
    assert_generated_table_manager(diminishing_returns, "DiminishingReturnsTable");
    let Some(NativeManagerShape::OneTableCrcKeyProjection(shape)) = diminishing_returns.shape()
    else {
        panic!("expected CRC-key projection shape for `DiminishingReturnsDataManager`");
    };
    assert_eq!(shape.key_column().as_str(), "DamageTypeId");
    assert!(shape.reject_zero_crc());
    assert_eq!(
        shape.duplicate_key_policy(),
        NativeDuplicateKeyPolicy::FirstWins
    );
    assert_eq!(
        shape
            .row_filters()
            .iter()
            .map(|filter| (filter.column().as_str(), filter.predicate()))
            .collect::<Vec<_>>(),
        [
            (
                "NegativeStartValue",
                NativeCrcProjectionRowFilterPredicate::F32LessThanOrEqualZero
            ),
            (
                "LowerLimit",
                NativeCrcProjectionRowFilterPredicate::I32LessThanOrEqualZero
            ),
            (
                "PositiveStartValue",
                NativeCrcProjectionRowFilterPredicate::F32GreaterThanOrEqualZero
            ),
            (
                "DiminishmentRate",
                NativeCrcProjectionRowFilterPredicate::F32GreaterThanOrEqualZero
            ),
        ]
    );
    assert_eq!(
        shape
            .fields()
            .iter()
            .map(|field| (field.field().as_str(), field.transform()))
            .collect::<Vec<_>>(),
        [
            ("negative_start_value", NativeProjectionTransform::F32),
            ("lower_limit", NativeProjectionTransform::I32),
            ("positive_start_value", NativeProjectionTransform::F32),
            ("upper_limit", NativeProjectionTransform::NonZeroU32),
            ("diminishment_rate", NativeProjectionTransform::F32),
        ]
    );
    let diverted_loot = find_manager(&managers, "crate::DivertedLootDataManager");
    assert_generated_table_manager(diverted_loot, "DivertedLootMaster");
    let Some(NativeManagerShape::OneTableCrcKeyProjection(shape)) = diverted_loot.shape() else {
        panic!("expected CRC-key projection shape for `DivertedLootDataManager`");
    };
    assert_eq!(shape.key_column().as_str(), "Id");
    assert!(shape.skip_empty_key());
    assert!(shape.reject_zero_crc());
    assert_eq!(
        shape.duplicate_key_policy(),
        NativeDuplicateKeyPolicy::FirstWins
    );
    assert_eq!(
        shape
            .fields()
            .iter()
            .map(|field| (field.field().as_str(), field.transform()))
            .collect::<Vec<_>>(),
        [
            ("count_to_next_level", NativeProjectionTransform::NonZeroU32),
            ("timeframe_seconds", NativeProjectionTransform::NonZeroU32),
            (
                "yellow_diverted_loot_modifier",
                NativeProjectionTransform::F32
            ),
            ("red_diverted_loot_modifier", NativeProjectionTransform::F32),
            ("yellow_xp_multiplier", NativeProjectionTransform::F32),
            ("red_xp_multiplier", NativeProjectionTransform::F32),
            (
                "green_notification_loc_tag",
                NativeProjectionTransform::String
            ),
            (
                "yellow_notification_loc_tag",
                NativeProjectionTransform::String
            ),
            (
                "red_notification_loc_tag",
                NativeProjectionTransform::String
            ),
        ]
    );
    let dungeon_cluster = find_manager(&managers, "crate::DungeonClusterStaticDataManager");
    assert_generated_table_manager(dungeon_cluster, "DungeonCluster");
    let Some(NativeManagerShape::OneTableCrcKeyProjection(shape)) = dungeon_cluster.shape() else {
        panic!("expected CRC-key projection shape for `DungeonClusterStaticDataManager`");
    };
    assert_eq!(shape.key_column().as_str(), "ClusterId");
    assert!(shape.skip_empty_key());
    assert_eq!(
        shape.duplicate_key_policy(),
        NativeDuplicateKeyPolicy::FirstWins
    );
    assert_eq!(
        shape.source_row_method().map(|method| method.as_str()),
        Some("dungeon_cluster_static_data_for_source_row")
    );
    assert_eq!(
        shape
            .methods()
            .iter()
            .map(|method| method.name().as_str())
            .collect::<Vec<_>>(),
        [
            "dungeon_cluster_static_data",
            "dungeon_cluster_static_data_by_key"
        ]
    );
    assert_eq!(
        shape
            .fields()
            .iter()
            .map(|field| (field.field().as_str(), field.transform()))
            .collect::<Vec<_>>(),
        [
            ("feature_id", NativeProjectionTransform::String),
            ("seed_graph", NativeProjectionTransform::String),
            ("weight", NativeProjectionTransform::OptionalString),
            ("theme", NativeProjectionTransform::OptionalString),
            ("replacement_count", NativeProjectionTransform::NonZeroU32),
            ("comment", NativeProjectionTransform::OptionalString),
        ]
    );
    let dungeon_grammar = find_manager(&managers, "crate::DungeonGrammarStaticDataManager");
    assert_generated_table_manager(dungeon_grammar, "DungeonGrammar");
    let Some(NativeManagerShape::OneTableCrcKeyProjection(shape)) = dungeon_grammar.shape() else {
        panic!("expected CRC-key projection shape for `DungeonGrammarStaticDataManager`");
    };
    assert_eq!(shape.key_column().as_str(), "GrammaReplacementId");
    assert!(shape.skip_empty_key());
    assert_eq!(
        shape.duplicate_key_policy(),
        NativeDuplicateKeyPolicy::FirstWins
    );
    assert_eq!(
        shape.source_row_method().map(|method| method.as_str()),
        Some("dungeon_grammar_static_data_for_source_row")
    );
    assert_eq!(
        shape
            .methods()
            .iter()
            .map(|method| method.name().as_str())
            .collect::<Vec<_>>(),
        [
            "dungeon_grammar_static_data",
            "dungeon_grammar_static_data_by_key"
        ]
    );
    assert_eq!(
        shape
            .fields()
            .iter()
            .map(|field| (field.field().as_str(), field.transform()))
            .collect::<Vec<_>>(),
        [
            ("feature_id", NativeProjectionTransform::Crc32),
            ("seed_graph", NativeProjectionTransform::String),
            (
                "min_depth",
                NativeProjectionTransform::OptionalU8DefaultZero
            ),
            ("max_depth", NativeProjectionTransform::OptionalU8DefaultMax),
            (
                "theme_tags",
                NativeProjectionTransform::OptionalCrcListDefaultEmpty
            ),
            ("weight", NativeProjectionTransform::U32),
            ("grammar_replacements", NativeProjectionTransform::CrcList),
            ("comments", NativeProjectionTransform::OptionalString),
        ]
    );
    let dungeon_room = find_manager(&managers, "crate::DungeonRoomStaticDataManager");
    assert_generated_table_manager(dungeon_room, "DungeonRoom");
    let Some(NativeManagerShape::OneTableCrcKeyProjection(shape)) = dungeon_room.shape() else {
        panic!("expected CRC-key projection shape for `DungeonRoomStaticDataManager`");
    };
    assert_eq!(shape.key_column().as_str(), "RoomId");
    assert!(shape.skip_empty_key());
    assert_eq!(
        shape.duplicate_key_policy(),
        NativeDuplicateKeyPolicy::FirstWins
    );
    assert_eq!(
        shape.source_row_method().map(|method| method.as_str()),
        Some("dungeon_room_static_data_for_source_row")
    );
    assert_eq!(
        shape
            .methods()
            .iter()
            .map(|method| method.name().as_str())
            .collect::<Vec<_>>(),
        [
            "dungeon_room_static_data",
            "dungeon_room_static_data_by_key"
        ]
    );
    assert_eq!(
        shape
            .fields()
            .iter()
            .map(|field| (field.field().as_str(), field.transform()))
            .collect::<Vec<_>>(),
        [
            ("feature_id", NativeProjectionTransform::Crc32),
            ("room_type", NativeProjectionTransform::Crc32),
            ("starting_state", NativeProjectionTransform::Crc32),
            ("alias_category1", NativeProjectionTransform::Crc32),
            (
                "alias_tag1",
                NativeProjectionTransform::OptionalCrcListDefaultEmpty
            ),
            ("alias_category2", NativeProjectionTransform::OptionalCrc32),
            (
                "alias_tag2",
                NativeProjectionTransform::OptionalCrcListDefaultEmpty
            ),
            ("alias_category3", NativeProjectionTransform::OptionalCrc32),
            (
                "alias_tag3",
                NativeProjectionTransform::OptionalCrcListDefaultEmpty
            ),
            ("alias_category4", NativeProjectionTransform::OptionalCrc32),
            (
                "alias_tag4",
                NativeProjectionTransform::OptionalCrcListDefaultEmpty
            ),
            ("is_room_passable", NativeProjectionTransform::Bool),
            ("room_passthrough_cost", NativeProjectionTransform::F32),
        ]
    );
    let dungeon_tile = find_manager(&managers, "crate::DungeonTileStaticDataManager");
    assert_generated_table_manager(dungeon_tile, "DungeonTile");
    let Some(NativeManagerShape::OneTableDungeonTile(shape)) = dungeon_tile.shape() else {
        panic!("expected DungeonTile cache shape for `DungeonTileStaticDataManager`");
    };
    assert_eq!(shape.key_column().as_str(), "DungeonTileId");
    assert_eq!(shape.key_getter().as_str(), "dungeon_tile_id");
    assert_eq!(shape.feature_column().as_str(), "FeatureId");
    assert_eq!(shape.feature_getter().as_str(), "feature_id");
    assert_eq!(shape.connections_column().as_str(), "Connections");
    assert_eq!(shape.rotations_column().as_str(), "Rotations");
    assert_eq!(shape.tile_size_column().as_str(), "TileSize");
    assert_eq!(shape.weight_column().as_str(), "Weight");
    assert_eq!(
        shape.variation_asset_paths_column().as_str(),
        "VariationAssetPaths"
    );
    assert_eq!(
        shape.supported_room_types_column().as_str(),
        "SupportedRoomTypes"
    );
    assert_eq!(
        shape
            .methods()
            .iter()
            .map(|method| method.name().as_str())
            .collect::<Vec<_>>(),
        [
            "dungeon_tile_static_data",
            "dungeon_tile_static_data_by_key"
        ]
    );
    assert_eq!(shape.tile_variants_method().as_str(), "tile_variants");
    assert_eq!(shape.tile_variant_row_method().as_str(), "tile_variant_row");
    let gear_score_upgrade = find_manager(&managers, "crate::StaticGearScoreUpgradeDataManager");
    assert_generated_table_manager(gear_score_upgrade, "GearScoreUpgrade");
    let Some(NativeManagerShape::OneTableNumericKeyProjection(shape)) = gear_score_upgrade.shape()
    else {
        panic!("expected numeric-key projection shape for `StaticGearScoreUpgradeDataManager`");
    };
    assert_eq!(shape.key_column().as_str(), "Level");
    assert_eq!(shape.key_type(), NativeNumericKeyType::U16FromNonZeroU32);
    assert_eq!(
        shape.duplicate_key_policy(),
        NativeDuplicateKeyPolicy::FirstWins
    );
    assert_eq!(
        shape.source_row_method().map(|method| method.as_str()),
        Some("gear_score_upgrade_data_for_source_row")
    );
    assert_eq!(
        shape
            .fields()
            .iter()
            .map(|field| (field.field().as_str(), field.transform()))
            .collect::<Vec<_>>(),
        [
            (
                "required_currency_id",
                NativeProjectionTransform::LowercaseCrcString
            ),
            (
                "required_currency_quantity",
                NativeProjectionTransform::NonZeroU32
            ),
        ]
    );
    let territory_progression = find_manager(&managers, "crate::TerritoryProgressionDataManager");
    assert_generated_table_manager(territory_progression, "TerritoryProgression");
    let Some(NativeManagerShape::OneTableCrcKeyProjection(shape)) = territory_progression.shape()
    else {
        panic!("expected CRC-key projection shape for `TerritoryProgressionDataManager`");
    };
    assert_eq!(shape.key_column().as_str(), "ProjectId");
    assert!(shape.skip_empty_key());
    assert!(shape.reject_zero_crc());
    assert_eq!(
        shape.duplicate_key_policy(),
        NativeDuplicateKeyPolicy::FirstWins
    );
    assert_eq!(
        shape.source_row_method().map(|method| method.as_str()),
        Some("territory_progression_data_for_source_row")
    );
    assert_eq!(
        shape
            .methods()
            .iter()
            .map(|method| (method.name().as_str(), method.parameter().kind()))
            .collect::<Vec<_>>(),
        [
            (
                "territory_progression_data_from_id",
                NativeCrcIndexLookupParameterKind::Crc32
            ),
            (
                "territory_progression_data",
                NativeCrcIndexLookupParameterKind::AsRefStr
            ),
            (
                "territory_progression_data_by_key",
                NativeCrcIndexLookupParameterKind::AsRefStr
            ),
        ]
    );
    let territory_fields = shape
        .fields()
        .iter()
        .map(|field| (field.field().as_str(), field.transform()))
        .collect::<Vec<_>>();
    assert_eq!(territory_fields.len(), 21);
    assert!(territory_fields.contains(&("level", NativeProjectionTransform::NonZeroU32)));
    assert!(territory_fields.contains(&(
        "prev_level_project_row",
        NativeProjectionTransform::OptionalForeignKeyRow
    )));
    assert!(territory_fields.contains(&(
        "next_level_project_row",
        NativeProjectionTransform::OptionalForeignKeyRow
    )));
    assert!(territory_fields.contains(&(
        "lifestyle_buff_effect_id",
        NativeProjectionTransform::OptionalString
    )));
    assert_manager_has_table(
        find_manager(&managers, "crate::TerritoryDefinitionsDataManager"),
        "AreaDefinitions",
    );
    assert_manager_has_table(
        find_manager(&managers, "crate::TerritoryDefinitionsDataManager"),
        "Territories",
    );
    let level_disparity = find_manager(&managers, "crate::LevelDisparityDataManager");
    assert_generated_table_manager(level_disparity, "AILevelDisparity");
    let Some(NativeManagerShape::OneTableLevelDisparity(shape)) = level_disparity.shape() else {
        panic!("expected LevelDisparity cache shape for `LevelDisparityDataManager`");
    };
    assert_eq!(shape.key_column().as_str(), "LevelDisparity");
    assert_eq!(shape.key_getter().as_str(), "level_disparity");
    assert_eq!(shape.range_field().as_str(), "range");
    assert_eq!(
        shape.max_capped_field().as_str(),
        "max_vision_distance_adjustment"
    );
    assert_eq!(
        shape.capped_value_source_field().as_str(),
        "vision_distance_adjustment"
    );
    assert_eq!(
        shape.source_row_method().as_str(),
        "level_disparity_data_for_source_row"
    );
    assert_eq!(
        [
            shape.lookup_method().as_str(),
            shape.levels_method().as_str(),
            shape.clamped_levels_method().as_str(),
            shape.capped_levels_method().as_str(),
            shape.capped_clamped_levels_method().as_str(),
            shape.loaded_range_method().as_str(),
            shape.clamped_key_method().as_str(),
            shape.max_capped_value_method().as_str(),
        ],
        [
            "level_disparity_data",
            "level_disparity_data_for_levels",
            "clamped_level_disparity_data_for_levels",
            "level_disparity_data_for_levels_with_player_level_cap",
            "clamped_level_disparity_data_for_levels_with_player_level_cap",
            "loaded_range",
            "clamped_disparity",
            "max_vision_distance_adjustment",
        ]
    );
    assert_eq!(
        shape
            .fields()
            .iter()
            .map(|field| (field.field().as_str(), field.transform()))
            .collect::<Vec<_>>(),
        [
            ("damage_modifier", NativeProjectionTransform::F32),
            (
                "physical_armor_rating_modifier",
                NativeProjectionTransform::F32
            ),
            (
                "elemental_armor_rating_modifier",
                NativeProjectionTransform::F32
            ),
            ("skip_deaths_door", NativeProjectionTransform::Bool),
            ("incoming_power_level_zero", NativeProjectionTransform::Bool),
            ("adjust_power_level", NativeProjectionTransform::Bool),
            ("required_power_level", NativeProjectionTransform::U32),
            ("adjusted_power_level", NativeProjectionTransform::U32),
            ("adjusted_hit_stun", NativeProjectionTransform::F32),
            ("vision_distance_adjustment", NativeProjectionTransform::F32),
            ("max_reward_level_delta", NativeProjectionTransform::U32),
            ("kill_exp_modifier", NativeProjectionTransform::F32),
            ("event_exp_modifier", NativeProjectionTransform::F32),
        ]
    );
    let costume_change = find_manager(&managers, "crate::CostumeChangeDataManager");
    assert_generated_table_manager(costume_change, "CostumeChanges");
    let Some(NativeManagerShape::OneTableCostumeChange(shape)) = costume_change.shape() else {
        panic!("expected CostumeChange cache shape for `CostumeChangeDataManager`");
    };
    assert_eq!(shape.key_column().as_str(), "CostumeChangeId");
    assert_eq!(shape.key_getter().as_str(), "costume_change_id");
    assert_eq!(shape.mesh_column().as_str(), "CostumeChangeMesh");
    assert_eq!(
        shape.matches_skeleton_column().as_str(),
        "MatchesPlayerSkeleton"
    );
    assert_eq!(shape.z_offset_column().as_str(), "MeshRenderZPosOffset");
    assert_eq!(
        shape.source_row_method().as_str(),
        "costume_change_data_for_source_row"
    );
    assert_eq!(
        [
            shape.lookup_from_id_method().as_str(),
            shape.lookup_method().as_str(),
            shape.lookup_by_key_method().as_str(),
            shape.audio_override_from_id_method().as_str(),
            shape.audio_override_method().as_str(),
        ],
        [
            "costume_change_data_from_id",
            "costume_change_data",
            "costume_change_data_by_key",
            "costume_audio_data_override_from_id",
            "costume_audio_data_override",
        ]
    );
    assert_eq!(
        shape
            .slots()
            .iter()
            .map(|slot| {
                (
                    slot.variant().as_str(),
                    slot.left_column().as_str(),
                    slot.right_column().as_str(),
                )
            })
            .collect::<Vec<_>>(),
        [
            ("Head", "HEAD_SLOT_Left", "HEAD_SLOT_Right"),
            ("Chest", "CHEST_SLOT_Left", "CHEST_SLOT_Right"),
            ("Hands", "HANDS_SLOT_Left", "HANDS_SLOT_Right"),
            ("Legs", "LEGS_SLOT_Left", "LEGS_SLOT_Right"),
            ("Feet", "FEET_SLOT_Left", "FEET_SLOT_Right"),
        ]
    );
    let cutscene_camera = find_manager(&managers, "crate::CutsceneCameraDataManager");
    assert_generated_table_manager(cutscene_camera, "CutsceneCameraPresets");
    let Some(NativeManagerShape::OneTableCrcKeyProjection(shape)) = cutscene_camera.shape() else {
        panic!("expected CRC-key projection shape for `CutsceneCameraDataManager`");
    };
    assert_eq!(shape.key_column().as_str(), "CutsceneCameraId");
    assert!(shape.reject_zero_crc());
    assert_eq!(
        shape.duplicate_key_policy(),
        NativeDuplicateKeyPolicy::FirstWins
    );
    assert_eq!(
        shape.source_row_method().map(|method| method.as_str()),
        Some("cutscene_camera_for_source_row")
    );
    assert_eq!(shape.fields().len(), 33);
    let cutscene_fields = shape
        .fields()
        .iter()
        .map(|field| (field.field().as_str(), field.transform()))
        .collect::<Vec<_>>();
    assert!(cutscene_fields.contains(&(
        "hide_nearby_player_avatars",
        NativeProjectionTransform::OptionalBool
    )));
    assert!(cutscene_fields.contains(&(
        "depth_of_field_override",
        NativeProjectionTransform::F32ListDefaultEmpty
    )));
    assert!(cutscene_fields.contains(&(
        "spectator_camera_origin_pitch",
        NativeProjectionTransform::I32
    )));
    let curse_mutation = find_manager(&managers, "crate::CurseMutationStaticDataManager");
    assert_generated_table_manager(curse_mutation, "CurseMutation");
    let Some(NativeManagerShape::OneTableCrcKeyProjection(shape)) = curse_mutation.shape() else {
        panic!("expected CRC-key projection shape for `CurseMutationStaticDataManager`");
    };
    assert_eq!(shape.key_column().as_str(), "CurseMutationId");
    assert!(shape.reject_zero_crc());
    assert_eq!(
        shape.duplicate_key_policy(),
        NativeDuplicateKeyPolicy::FirstWins
    );
    assert_eq!(
        shape.source_row_method().map(|method| method.as_str()),
        Some("curse_mutation_data_for_source_row")
    );
    assert_eq!(
        shape
            .fields()
            .iter()
            .map(|field| (field.field().as_str(), field.transform()))
            .collect::<Vec<_>>(),
        [
            ("curse_minor", NativeProjectionTransform::String),
            ("curse_major", NativeProjectionTransform::String),
            (
                "global_affliction_row",
                NativeProjectionTransform::OptionalForeignKeyRow,
            ),
            ("name", NativeProjectionTransform::String),
            ("description", NativeProjectionTransform::String),
            ("icon_path", NativeProjectionTransform::String),
        ]
    );
    let economy_tracker = find_manager(&managers, "crate::EconomyTrackerDataManager");
    assert_generated_table_manager(economy_tracker, "EconomyTrackers");
    let Some(NativeManagerShape::OneTableCrcKeyProjection(shape)) = economy_tracker.shape() else {
        panic!("expected CRC-key projection shape for `EconomyTrackerDataManager`");
    };
    assert_eq!(shape.key_column().as_str(), "EconomyTrackerID");
    assert!(shape.reject_zero_crc());
    assert_eq!(
        shape.duplicate_key_policy(),
        NativeDuplicateKeyPolicy::FirstWins
    );
    assert_eq!(
        shape.source_row_method().map(|method| method.as_str()),
        Some("economy_tracker_data_for_source_row")
    );
    assert_eq!(
        shape
            .fields()
            .iter()
            .map(|field| (field.field().as_str(), field.transform()))
            .collect::<Vec<_>>(),
        [
            ("alarm_type", NativeProjectionTransform::String),
            (
                "threshold_config1",
                NativeProjectionTransform::OptionalString
            ),
            ("metric_name1", NativeProjectionTransform::String),
            ("enable_metric1", NativeProjectionTransform::Bool),
            (
                "threshold_config2",
                NativeProjectionTransform::OptionalString
            ),
            ("metric_name2", NativeProjectionTransform::OptionalString),
            ("enable_metric2", NativeProjectionTransform::Bool),
        ]
    );
    let currency_exchange = find_manager(&managers, "crate::CurrencyExchangeDataManager");
    assert_generated_table_manager(currency_exchange, "CurrencyExchange");
    let Some(NativeManagerShape::OneTableCrcKeyProjection(shape)) = currency_exchange.shape()
    else {
        panic!("expected CRC-key projection shape for `CurrencyExchangeDataManager`");
    };
    assert_eq!(shape.key_column().as_str(), "ConversionID");
    assert!(!shape.reject_zero_crc());
    assert_eq!(
        shape.duplicate_key_policy(),
        NativeDuplicateKeyPolicy::FirstWins
    );
    assert_eq!(shape.row_filters().len(), 1);
    assert_eq!(shape.row_filters()[0].column().as_str(), "FromCurrencyId");
    assert_eq!(
        shape.row_filters()[0].predicate(),
        NativeCrcProjectionRowFilterPredicate::StringNotEqualToColumn
    );
    assert_eq!(
        shape.row_filters()[0]
            .compare_getter()
            .map(|getter| getter.as_str()),
        Some("to_currency_id")
    );
    assert_eq!(
        shape
            .fields()
            .iter()
            .map(|field| (field.field().as_str(), field.transform()))
            .collect::<Vec<_>>(),
        [
            ("from_currency_id", NativeProjectionTransform::String),
            (
                "from_currency_crc",
                NativeProjectionTransform::LowercaseCrcString
            ),
            (
                "from_currency_is_categorical_progression",
                NativeProjectionTransform::Bool
            ),
            (
                "from_currency_quantity",
                NativeProjectionTransform::NonZeroU32
            ),
            ("to_currency_id", NativeProjectionTransform::String),
            (
                "to_currency_crc",
                NativeProjectionTransform::LowercaseCrcString
            ),
            (
                "to_currency_is_categorical_progression",
                NativeProjectionTransform::Bool
            ),
            (
                "to_currency_quantity",
                NativeProjectionTransform::NonZeroU32
            ),
        ]
    );
    let status_effect_category_data =
        find_manager(&managers, "crate::StatusEffectCategoryDataManager");
    assert_generated_table_manager(status_effect_category_data, "StatusEffectCategories");
    let Some(NativeManagerShape::OneTableCrcKeyProjection(shape)) =
        status_effect_category_data.shape()
    else {
        panic!("expected CRC-key projection shape for `StatusEffectCategoryDataManager`");
    };
    assert_eq!(shape.key_column().as_str(), "StatusEffectCategoryID");
    assert_eq!(
        shape.duplicate_key_policy(),
        NativeDuplicateKeyPolicy::FirstWins
    );
    assert_eq!(
        shape
            .fields()
            .iter()
            .map(|field| (field.field().as_str(), field.transform()))
            .collect::<Vec<_>>(),
        [
            ("duration_diminishing_mod", NativeProjectionTransform::F32),
            ("duration_mod_min", NativeProjectionTransform::F32),
            ("duration_mod_max", NativeProjectionTransform::F32),
            ("potency_diminishing_mod", NativeProjectionTransform::F32),
            ("potency_mod_min", NativeProjectionTransform::F32),
            ("potency_mod_max", NativeProjectionTransform::F32),
            (
                "value_limits",
                NativeProjectionTransform::OptionalStringList
            ),
        ]
    );
    assert_table_family_crc_projection(
        find_manager(&managers, "crate::ConversationStateDataManager"),
        "ConversationStates_C01",
        "ConversationStateId",
        "states",
        "conversation_state_ids",
        "states",
    );
    assert_table_family_crc_projection(
        find_manager(&managers, "crate::ConversationTopicDataManager"),
        "ConversationTopics_C01",
        "ConversationTopicId",
        "topics",
        "conversation_topic_ids",
        "topics",
    );
    let vitals_base_data = find_manager(&managers, "crate::VitalsBaseDataManager");
    assert_mixed_table_manager_dependencies(
        vitals_base_data,
        &[
            "BaseVitals_Catacombs",
            "BaseVitals_Common",
            "BaseVitals_CutlassKeys",
            "BaseVitals_Dunwood",
            "BaseVitals_FirstLight",
            "BaseVitals_IsleOfNight",
            "BaseVitals_Player",
            "BaseVitals_Raid_CutlassKeys",
            "BaseVitals_WorldBoss",
        ],
        &[],
    );
    let Some(NativeManagerShape::TableFamilyCrcKeyProjection(shape)) = vitals_base_data.shape()
    else {
        panic!("expected table-family CRC-key projection shape for `VitalsBaseDataManager`");
    };
    assert_eq!(shape.key_column().as_str(), "VitalsID");
    assert_eq!(
        shape.duplicate_key_policy(),
        NativeDuplicateKeyPolicy::FirstWins
    );
    assert_eq!(
        shape.source_row_by_crc_method().map(RustIdentifier::as_str),
        Some("source_row_from_id")
    );
    assert_eq!(
        shape
            .fields()
            .iter()
            .map(|field| (field.field().as_str(), field.transform()))
            .collect::<Vec<_>>(),
        [
            (
                "creature_type",
                NativeProjectionTransform::OptionalFirstString
            ),
            (
                "creature_type_crc",
                NativeProjectionTransform::OptionalFirstLowercaseCrcStringDefaultZero
            ),
        ]
    );
    assert_mixed_table_manager_dependencies(
        find_manager(&managers, "crate::VitalsDataManager"),
        &[
            "LevelVariantVitals_Catacombs",
            "LevelVariantVitals_Common",
            "LevelVariantVitals_CutlassKeys",
            "LevelVariantVitals_Dunwood",
            "LevelVariantVitals_FirstLight",
            "LevelVariantVitals_IsleOfNight",
            "LevelVariantVitals_Player",
            "LevelVariantVitals_WorldBoss",
            "Vitals_Raid_CutlassKeys",
        ],
        &["crate::VitalsBaseDataManager"],
    );
    assert_vitals_data_manager_shape(find_manager(&managers, "crate::VitalsDataManager"));
    let vitals_level_data = find_manager(&managers, "crate::VitalsLevelDataManager");
    assert_generated_table_manager(vitals_level_data, "VitalsLevels");
    let Some(NativeManagerShape::OneTableNumericKeyProjection(shape)) = vitals_level_data.shape()
    else {
        panic!("expected numeric-key projection shape for `VitalsLevelDataManager`");
    };
    assert_eq!(shape.key_column().as_str(), "Level");
    assert_eq!(shape.key_type(), NativeNumericKeyType::NonZeroU32);
    assert_eq!(
        shape.duplicate_key_policy(),
        NativeDuplicateKeyPolicy::FirstWins
    );
    assert_eq!(
        shape
            .methods()
            .iter()
            .map(|method| (method.name().as_str(), method.parameter_kind()))
            .collect::<Vec<_>>(),
        [(
            "vitals_level_data",
            NativeNumericLookupParameterKind::NonZeroU32
        )]
    );
    assert_eq!(
        shape
            .fields()
            .iter()
            .map(|field| (field.field().as_str(), field.transform()))
            .collect::<Vec<_>>(),
        [
            ("gear_score", NativeProjectionTransform::NonZeroU32),
            ("base_damage", NativeProjectionTransform::F32),
            ("base_max_health", NativeProjectionTransform::F32),
            ("initial_health", NativeProjectionTransform::F32),
            ("physical_armor_rating", NativeProjectionTransform::F32),
            ("elemental_armor_rating", NativeProjectionTransform::F32),
            ("loot_gs_bonus", NativeProjectionTransform::NonZeroU32),
            ("loot_gs_bonus_chance", NativeProjectionTransform::F32),
            (
                "container_loot_gs_floor",
                NativeProjectionTransform::NonZeroU32
            ),
            ("ai_loot_gs_ceiling", NativeProjectionTransform::NonZeroU32),
            ("solo_damage", NativeProjectionTransform::OptionalString),
            ("solo_health", NativeProjectionTransform::OptionalString),
        ]
    );
    let vitals_modifier_data = find_manager(&managers, "crate::VitalsModifierDataManager");
    assert_generated_table_manager(vitals_modifier_data, "VitalsModifiers");
    let Some(NativeManagerShape::OneTableCrcKeyProjection(shape)) = vitals_modifier_data.shape()
    else {
        panic!("expected CRC-key projection shape for `VitalsModifierDataManager`");
    };
    assert_eq!(shape.key_column().as_str(), "CategoryId");
    assert_eq!(
        shape.duplicate_key_policy(),
        NativeDuplicateKeyPolicy::FirstWins
    );
    assert!(shape.skip_empty_key());
    assert!(!shape.trim_key());
    assert!(shape.reject_zero_crc());
    assert_eq!(
        shape
            .fields()
            .iter()
            .map(|field| (field.field().as_str(), field.transform()))
            .collect::<Vec<_>>(),
        [
            ("category_damage_mod", NativeProjectionTransform::F32),
            ("category_health_mod", NativeProjectionTransform::F32),
            ("category_stamina_mod", NativeProjectionTransform::F32),
            ("category_drop_chance_mod", NativeProjectionTransform::F32),
        ]
    );
    assert_dynamic_difficulty_data_manager_shape(find_manager(
        &managers,
        "crate::DynamicDifficultyDataManager",
    ));
    let darkness = find_manager(&managers, "crate::DarknessDataManager");
    assert_generated_table_manager(darkness, "DarknessDataTable");
    let Some(NativeManagerShape::OneTableDarkness(shape)) = darkness.shape() else {
        panic!("expected one-table darkness shape for `DarknessDataManager`");
    };
    assert_eq!(shape.key_column().as_str(), "DarknessId");
    assert_eq!(shape.key_getter().as_str(), "darkness_id");
    assert_eq!(
        [
            shape.lookup_crc_method().as_str(),
            shape.lookup_method().as_str(),
            shape.source_row_method().as_str(),
        ],
        ["darkness_data_by_crc32", "darkness_data", "source_row"]
    );
    let difficulty_scaling = find_manager(&managers, "crate::DifficultyScalingDataManager");
    assert_generated_table_manager(
        difficulty_scaling,
        "DifficultyScaling_WorldEncounter_Participants",
    );
    let Some(NativeManagerShape::OneTableDifficultyScaling(shape)) = difficulty_scaling.shape()
    else {
        panic!("expected one-table difficulty-scaling shape for `DifficultyScalingDataManager`");
    };
    assert_eq!(shape.key_column().as_str(), "WorldEncounterID");
    assert_eq!(shape.key_getter().as_str(), "world_encounter_id");
    assert_eq!(
        [
            shape.lookup_from_id_method().as_str(),
            shape.lookup_method().as_str(),
            shape.lookup_by_key_method().as_str(),
        ],
        [
            "difficulty_scaling_data_from_id",
            "difficulty_scaling_data",
            "difficulty_scaling_data_by_key",
        ]
    );
    let game_mode_data = find_manager(&managers, "crate::GameModeDataManager");
    assert_generated_table_manager(game_mode_data, "GameModes");
    let Some(NativeManagerShape::OneTableCrcIndex(shape)) = game_mode_data.shape() else {
        panic!("expected one-table CRC-index shape for `GameModeDataManager`");
    };
    assert_eq!(shape.key_column().as_str(), "GameModeId");
    assert_eq!(
        shape.row_crc_method().map(|method| method.as_str()),
        Some("game_mode_id_crc")
    );
    assert_eq!(shape.hash_policy(), NativeCrcHashPolicy::Lowercase);
    assert!(shape.reject_zero_crc());
    assert_eq!(
        shape
            .methods()
            .iter()
            .map(|method| method.name().as_str())
            .collect::<Vec<_>>(),
        ["game_mode", "game_mode_from_id"]
    );
    let game_mode_map_data = find_manager(&managers, "crate::GameModeMapDataManager");
    assert_generated_table_manager(game_mode_map_data, "GameModeMap");
    let Some(NativeManagerShape::OneTableCrcIndex(shape)) = game_mode_map_data.shape() else {
        panic!("expected one-table CRC-index shape for `GameModeMapDataManager`");
    };
    assert_eq!(shape.key_column().as_str(), "GameModeMapId");
    assert_eq!(
        shape.row_crc_method().map(|method| method.as_str()),
        Some("game_mode_map_id_crc")
    );
    assert_eq!(shape.hash_policy(), NativeCrcHashPolicy::Lowercase);
    assert!(shape.reject_zero_crc());
    assert_eq!(
        shape
            .methods()
            .iter()
            .map(|method| method.name().as_str())
            .collect::<Vec<_>>(),
        ["game_mode_map", "game_mode_map_from_id"]
    );
    let generic_invite = find_manager(&managers, "crate::GenericInviteStaticDataManager");
    assert_generated_table_manager(generic_invite, "GenericInvites");
    let Some(NativeManagerShape::OneTableCrcKeyProjection(shape)) = generic_invite.shape() else {
        panic!("expected CRC-key projection shape for `GenericInviteStaticDataManager`");
    };
    assert_eq!(shape.key_column().as_str(), "ActivityCrc");
    assert!(shape.skip_empty_key());
    assert!(shape.trim_key());
    assert!(shape.reject_zero_crc());
    assert_eq!(
        shape.duplicate_key_policy(),
        NativeDuplicateKeyPolicy::FirstWins
    );
    assert_eq!(
        shape
            .methods()
            .iter()
            .map(|method| method.name().as_str())
            .collect::<Vec<_>>(),
        [
            "generic_invite_static_data",
            "generic_invite_static_data_by_key"
        ]
    );
    assert_eq!(
        shape
            .fields()
            .iter()
            .find(|field| field.field().as_str() == "disallow_in_game_mode_rows")
            .map(NativeProjectionField::transform),
        Some(NativeProjectionTransform::ForeignKeyRowList)
    );
    let game_mode_scheduler = find_manager(&managers, "crate::GameModeSchedulerDataManager");
    assert_mixed_table_manager_dependencies(
        game_mode_scheduler,
        &["GameModeScheduler", "AchievementDataTable"],
        &[],
    );
    let Some(NativeManagerShape::OneTableCrcKeyProjection(shape)) = game_mode_scheduler.shape()
    else {
        panic!("expected CRC-key projection shape for `GameModeSchedulerDataManager`");
    };
    assert_eq!(shape.key_column().as_str(), "GameModeId");
    assert_eq!(shape.key_getter().as_str(), "game_mode_id");
    assert!(shape.skip_empty_key());
    assert!(shape.reject_zero_crc());
    assert_eq!(
        shape.duplicate_key_policy(),
        NativeDuplicateKeyPolicy::FirstWins
    );
    assert_eq!(
        shape
            .fields()
            .iter()
            .map(|field| (field.field().as_str(), field.transform()))
            .collect::<Vec<_>>(),
        [
            ("category_key", NativeProjectionTransform::String),
            ("category", NativeProjectionTransform::LowercaseCrcString),
            (
                "quest_active_achievement_id",
                NativeProjectionTransform::ForeignKeyTargetLowercaseCrc
            ),
            (
                "quest_completion_achievement_id",
                NativeProjectionTransform::ForeignKeyTargetLowercaseCrc
            ),
            (
                "scheduled_gm_entry_achievement_id",
                NativeProjectionTransform::ForeignKeyTargetLowercaseCrc
            ),
            (
                "scheduled_gm_completion_achievement_id",
                NativeProjectionTransform::ForeignKeyTargetLowercaseCrc
            ),
            (
                "scheduled_gm_completion_game_event_key",
                NativeProjectionTransform::String
            ),
            (
                "scheduled_gm_completion_game_event",
                NativeProjectionTransform::LowercaseCrcString
            ),
        ]
    );
    let world_event_category_data = find_manager(&managers, "crate::WorldEventCategoryDataManager");
    assert_generated_table_manager(world_event_category_data, "WorldEventCategories");
    let Some(NativeManagerShape::OneTableCrcIndex(shape)) = world_event_category_data.shape()
    else {
        panic!("expected one-table CRC-index shape for `WorldEventCategoryDataManager`");
    };
    assert_eq!(shape.key_column().as_str(), "CategoryId");
    assert_eq!(
        shape.row_crc_method().map(|method| method.as_str()),
        Some("category_id_crc")
    );
    assert_eq!(shape.hash_policy(), NativeCrcHashPolicy::Lowercase);
    assert!(shape.reject_zero_crc());
    assert_eq!(
        shape
            .methods()
            .iter()
            .map(|method| method.name().as_str())
            .collect::<Vec<_>>(),
        [
            "world_event_category_data_by_crc32",
            "world_event_category_data"
        ]
    );
    let crest_part_data = find_manager(&managers, "crate::CrestPartDataManager");
    assert_generated_table_manager(crest_part_data, "Crests");
    let Some(NativeManagerShape::OneTableCrestPart(shape)) = crest_part_data.shape() else {
        panic!("expected crest-part cache shape for `CrestPartDataManager`");
    };
    assert_eq!(shape.table_name().as_str(), "Crests");
    assert_eq!(shape.row_type_name().as_str(), "CrestPartData");
    assert_eq!(shape.entries_field().as_str(), "crest_parts");
    let crafting_category_data = find_manager(&managers, "crate::CraftingCategoryDataManager");
    assert_generated_table_manager(crafting_category_data, "CraftingCategories");
    let Some(NativeManagerShape::OneTableCrcKeyProjection(shape)) = crafting_category_data.shape()
    else {
        panic!("expected CRC-key projection shape for `CraftingCategoryDataManager`");
    };
    assert_eq!(shape.key_column().as_str(), "CategoryID");
    assert_eq!(
        shape.duplicate_key_policy(),
        NativeDuplicateKeyPolicy::Overwrite
    );
    assert!(shape
        .fields()
        .iter()
        .all(|field| field.transform() == NativeProjectionTransform::OptionalStringDefaultEmpty));
    let encumbrance_data = find_manager(&managers, "crate::EncumbranceDataManager");
    assert_generated_table_manager(encumbrance_data, "EncumbranceLimits");
    let Some(NativeManagerShape::OneTableEncumbrance(shape)) = encumbrance_data.shape() else {
        panic!("expected encumbrance cache shape for `EncumbranceDataManager`");
    };
    assert_eq!(shape.key_column().as_str(), "ContainerTypeID");
    assert_eq!(shape.key_getter().as_str(), "container_type_id");
    assert_eq!(
        shape.lookup_from_id_method().as_str(),
        "encumbrance_data_from_id"
    );
    assert_eq!(shape.lookup_method().as_str(), "encumbrance_data");
    assert_eq!(
        shape.lookup_by_key_method().as_str(),
        "encumbrance_data_by_key"
    );
    assert_generated_table_manager(
        find_manager(&managers, "crate::HunterSightDataManager"),
        "HunterSight",
    );

    assert_product_asset_resource_manager_with_format(
        find_manager(&managers, "crate::CameraSettingsDataManager"),
        crate::manager::NativeManagerProductFormat::Xml,
        &[(
            "libs/camera/gamecamera.xml",
            "newworld_plugin::assets::camera_settings::GameCameraSettingsAsset",
            "newworld_plugin::assets::camera_settings::GameCameraSettings",
            "game_camera_settings",
            "settings",
            "settings",
        )],
    );
    assert_product_asset_resource_manager(
        find_manager(&managers, "crate::ArmorOffsetDataManager"),
        &[(
            "sharedassets/genericassets/items/armoroffsets.aoffdb",
            "newworld_plugin::assets::armor_offset_database::ArmorOffsetDatabaseAsset",
            "newworld_plugin::assets::armor_offset_database::ArmorOffsetDatabase",
            "armor_offset_database",
            "database",
            "database",
        )],
    );
    assert_product_asset_resource_manager(
        find_manager(&managers, "crate::EquipTypesDataManager"),
        &[(
            "sharedassets/genericassets/items/equiptypesdatabase.equipdb",
            "newworld_plugin::assets::equip_types_database::EquipTypesDatabaseAsset",
            "newworld_plugin::assets::equip_types_database::EquipTypesDatabase",
            "equip_types_database",
            "database",
            "database",
        )],
    );
    assert_product_asset_resource_manager(
        find_manager(&managers, "crate::GameDebugSettingsManager"),
        &[(
            "sharedassets/genericassets/debug/gamedebugsettings.gds",
            "newworld_plugin::assets::game_debug_settings::GameDebugSettingsAsset",
            "newworld_plugin::assets::game_debug_settings::GameDebugSettings",
            "game_debug_settings",
            "settings",
            "settings",
        )],
    );
    assert_product_asset_resource_manager(
        find_manager(&managers, "crate::UiDataManager"),
        &[(
            "sharedassets/genericassets/ui/uidatabase.uidb",
            "newworld_plugin::assets::ui_database::UiDatabaseAsset",
            "newworld_plugin::assets::ui_database::UiDatabase",
            "ui_database",
            "database",
            "database",
        )],
    );
    assert_player_data_manager(
        find_manager(&managers, "crate::PlayerDataManager"),
        &[
            (
                "sharedassets/genericassets/playerbaseattributes.pbadb",
                "newworld_plugin::assets::player_base_attributes::PlayerBaseAttributesAsset",
                "newworld_plugin::assets::player_base_attributes::PlayerBaseAttributes",
                "player_base_attributes",
                "attributes",
                "player_base_attributes",
            ),
            (
                "sharedassets/genericassets/settlementprogression.sprd",
                "newworld_plugin::assets::settlement_progression_data::SettlementProgressionDataAsset",
                "newworld_plugin::assets::settlement_progression_data::SettlementProgressionData",
                "settlement_progression_data",
                "data",
                "settlement_progression_data",
            ),
        ],
    );
    assert_currency_exchange_mapping_manager_shape(find_manager(
        &managers,
        "crate::CurrencyExchangeMappingManager",
    ));
    let combat_settings = find_manager(&managers, "crate::CombatSettingsDataManager");
    assert_generated_table_manager(combat_settings, "CombatSettingsDataTable");
    let Some(NativeManagerShape::OneTableCrcKeyProjection(shape)) = combat_settings.shape() else {
        panic!("expected CRC-key projection shape for `CombatSettingsDataManager`");
    };
    assert_eq!(shape.key_column().as_str(), "ProfileName");
    assert!(shape.skip_empty_key());
    assert_eq!(
        shape.duplicate_key_policy(),
        NativeDuplicateKeyPolicy::FirstWins
    );
    assert_eq!(
        shape.source_row_method().map(|method| method.as_str()),
        Some("source_row")
    );
    assert_eq!(
        shape
            .methods()
            .iter()
            .map(|method| method.name().as_str())
            .collect::<Vec<_>>(),
        ["combat_settings_data", "combat_settings_data_by_key"]
    );
    assert_combat_profiles_data_manager_shape(find_manager(
        &managers,
        "crate::CombatProfilesDataManager",
    ));
    assert_dynamic_difficulty_data_manager_shape(find_manager(
        &managers,
        "crate::DynamicDifficultyDataManager",
    ));
    assert_elemental_mutation_static_data_manager_shape(find_manager(
        &managers,
        "crate::ElementalMutationStaticDataManager",
    ));
    assert_promotion_mutation_static_data_manager_shape(find_manager(
        &managers,
        "crate::PromotionMutationStaticDataManager",
    ));
    assert_vitals_data_manager_shape(find_manager(&managers, "crate::VitalsDataManager"));
    assert_seasons_rewards_activities_tasks_data_manager_shape(find_manager(
        &managers,
        "crate::SeasonsRewardsActivitiesTasksDataManager",
    ));
    assert_seasons_rewards_battle_pass_data_manager_shape(find_manager(
        &managers,
        "crate::SeasonsRewardsBattlePassDataManager",
    ));
    assert_seasons_rewards_chapter_data_manager_shape(find_manager(
        &managers,
        "crate::SeasonsRewardsChapterDataManager",
    ));
    assert_seasons_rewards_journey_data_manager_shape(find_manager(
        &managers,
        "crate::SeasonsRewardsJourneyDataManager",
    ));
    assert_musical_rewards_data_manager_shape(find_manager(
        &managers,
        "crate::MusicalRewardsDataManager",
    ));
    assert_item_data_manager_shape(find_manager(&managers, "crate::ItemDataManager"));
    assert_damage_data_manager_shape(find_manager(&managers, "crate::DamageDataManager"));
    assert_item_conversion_data_manager_shape(find_manager(
        &managers,
        "crate::ItemConversionDataManager",
    ));
    assert_item_transform_data_manager_shape(find_manager(
        &managers,
        "crate::ItemTransformDataManager",
    ));
    let perk_buckets = find_manager(&managers, "crate::PerkBucketDataManager");
    let shape = assert_one_table_crc_projection(
        perk_buckets,
        "PerkBuckets",
        "PerkBucketId",
        "buckets",
        "perk_bucket_ids",
        "buckets",
    );
    assert_eq!(
        shape
            .fields()
            .iter()
            .map(|field| (field.field().as_str(), field.transform()))
            .collect::<Vec<_>>(),
        [
            (
                "ignore_exclusive_label_weights",
                NativeProjectionTransform::OptionalBool
            ),
            (
                "disable_perk_biasing",
                NativeProjectionTransform::OptionalBool
            ),
            ("perk_type", NativeProjectionTransform::OptionalString),
            ("perk_chance", NativeProjectionTransform::OptionalF32),
        ]
    );
    assert_progression_point_data_manager_shape(find_manager(
        &managers,
        "crate::ProgressionPointDataManager",
    ));
    assert_gatherable_data_manager_shape(find_manager(&managers, "crate::GatherableDataManager"));
    assert_song_book_sheet_data_manager_shape(find_manager(
        &managers,
        "crate::SongBookSheetDataManager",
    ));
    assert_song_book_data_manager_shape(find_manager(&managers, "crate::SongBookDataManager"));
    assert_manager_dependencies(
        find_manager(&managers, "crate::ItemDataManager"),
        &[
            "crate::DyeItemDataManager",
            "crate::MountDyeItemDataManager",
            "crate::DyeColorDataManager",
        ],
    );
    assert_musical_rewards_data_manager_shape(find_manager(
        &managers,
        "crate::MusicalRewardsDataManager",
    ));
    assert_song_book_data_manager_shape(find_manager(&managers, "crate::SongBookDataManager"));
    assert_song_book_sheet_data_manager_shape(find_manager(
        &managers,
        "crate::SongBookSheetDataManager",
    ));
    assert_vitals_modifier_mapping_manager_shape(find_manager(
        &managers,
        "crate::VitalsModifierMappingManager",
    ));
    assert_mixed_table_manager_dependencies(
        find_manager(&managers, "crate::PerkBucketDataManager"),
        &["PerkBuckets"],
        &[
            "crate::PerkDataManager",
            "crate::PerkExclusiveLabelDataManager",
        ],
    );
    assert_replication_data_manager_shape(find_manager(&managers, "crate::ReplicationDataManager"));
    assert_mixed_table_manager_dependencies(
        find_manager(&managers, "crate::WeaponRefDataManager"),
        &[
            "RuneItemDefinitions",
            "WeaponItemDefinitions",
            "WeaponItemDefinitions_IsleOfNight",
        ],
        &[],
    );
    let weapon_refs = find_manager(&managers, "crate::WeaponRefDataManager");
    let Some(NativeManagerShape::TableFamilyCrcKeyProjection(shape)) = weapon_refs.shape() else {
        panic!("expected table-family CRC-key projection shape for `WeaponRefDataManager`");
    };
    assert_eq!(shape.key_column().as_str(), "WeaponID");
    assert!(shape.skip_empty_key());
    assert!(shape.reject_zero_crc());
    assert_eq!(
        shape.duplicate_key_policy(),
        NativeDuplicateKeyPolicy::FirstWins
    );
    assert_eq!(shape.source_handle_field(), None);
    assert_eq!(shape.row_filters().len(), 1);
    let scaling_filter = &shape.row_filters()[0];
    assert_eq!(scaling_filter.column().as_str(), "ScalingStrength");
    assert_eq!(
        scaling_filter.predicate(),
        NativeCrcProjectionRowFilterPredicate::F32AnyGreaterThanZero
    );
    assert_eq!(
        scaling_filter
            .extra_getters()
            .iter()
            .map(RustIdentifier::as_str)
            .collect::<Vec<_>>(),
        ["scaling_dexterity", "scaling_intelligence", "scaling_focus"]
    );
    assert_eq!(
        shape
            .fields()
            .iter()
            .map(|field| (field.field().as_str(), field.transform()))
            .collect::<Vec<_>>(),
        [
            ("scaling_strength", NativeProjectionTransform::F32),
            ("scaling_dexterity", NativeProjectionTransform::F32),
            ("scaling_intelligence", NativeProjectionTransform::F32),
            ("scaling_focus", NativeProjectionTransform::F32),
        ]
    );
    assert_mixed_table_manager_dependencies(
        find_manager(&managers, "crate::WeaponItemDataManager"),
        &[
            "RuneItemDefinitions",
            "WeaponItemDefinitions",
            "WeaponItemDefinitions_IsleOfNight",
        ],
        &[],
    );
    let weapon_items = find_manager(&managers, "crate::WeaponItemDataManager");
    let Some(NativeManagerShape::TableFamilyCrcKeyProjection(shape)) = weapon_items.shape() else {
        panic!("expected table-family CRC-key projection shape for `WeaponItemDataManager`");
    };
    assert_eq!(shape.module().as_str(), "weapon_item_data");
    assert_eq!(shape.table_module().as_str(), "weapon_item_definitions");
    assert_eq!(shape.key_column().as_str(), "WeaponID");
    assert!(shape.skip_empty_key());
    assert!(!shape.reject_zero_crc());
    assert_eq!(
        shape.duplicate_key_policy(),
        NativeDuplicateKeyPolicy::Error
    );
    assert_eq!(shape.source_handle_field(), None);
    assert!(shape.fields().is_empty());
    assert_mixed_table_manager_dependencies(
        find_manager(&managers, "crate::SeasonsTrackedStatDataManager"),
        &[
            "SeasonsRewardsStats",
            "SeasonsRewardsStats_Achievements",
            "SeasonsRewardsStats_ActivityCard",
            "SeasonsRewardsStats_Arena",
            "SeasonsRewardsStats_CategoricalProgression",
            "SeasonsRewardsStats_Combat",
            "SeasonsRewardsStats_CommitResource",
            "SeasonsRewardsStats_Consume",
            "SeasonsRewardsStats_Craft",
            "SeasonsRewardsStats_Duel",
            "SeasonsRewardsStats_EquipItem",
            "SeasonsRewardsStats_Expedition",
            "SeasonsRewardsStats_FactionControl",
            "SeasonsRewardsStats_Fishing",
            "SeasonsRewardsStats_GameEvent",
            "SeasonsRewardsStats_Gather",
            "SeasonsRewardsStats_JourneyTask",
            "SeasonsRewardsStats_Kill",
            "SeasonsRewardsStats_Level",
            "SeasonsRewardsStats_OutpostRush",
            "SeasonsRewardsStats_Quest",
            "SeasonsRewardsStats_Salvage",
            "SeasonsRewardsStats_Song",
            "SeasonsRewardsStats_War",
        ],
        &[],
    );
    assert_social_data_manager_shape(find_manager(&managers, "crate::SocialDataManager"));
    assert_mixed_table_product_manager(
        find_manager(&managers, "crate::RecipeDataManager"),
        &[
            "CraftingRecipesArcana",
            "CraftingRecipesArmorer",
            "CraftingRecipesCooking",
            "CraftingRecipesDungeon",
            "CraftingRecipesEngineer",
            "CraftingRecipesGypKilm",
            "CraftingRecipesJeweler",
            "CraftingRecipesMisc",
            "CraftingRecipesRaid",
            "CraftingRecipesRefining",
            "CraftingRecipesSeasonalServers",
            "CraftingRecipesSeasons",
            "CraftingRecipesSpecialtyShops",
            "CraftingRecipesWeapon",
            "CraftingRecipes",
        ],
        &[(
            "sharedassets/genericassets/craftingstations.craftstationdb",
            "newworld_plugin::assets::crafting_station_database::CraftingStationDatabaseAsset",
        )],
    );
    assert_gatherable_data_manager_shape(find_manager(&managers, "crate::GatherableDataManager"));
}

fn table_schema(
    table_name: &str,
    row_type_name: &str,
) -> crate::game_system_schema::GameSystemTableSchema {
    crate::game_system_schema::GameSystemTableSchema {
        table_name: table_name.to_owned(),
        table_name_crc: 0,
        row_type_name: row_type_name.to_owned(),
        row_type_crc: 0,
        row_count: 0,
        sources: Vec::new(),
        columns: Vec::new(),
    }
}

#[test]
fn schema_plan_suppresses_generated_cache_when_table_inputs_are_missing() {
    let report = GameSystemDataTablesSchemaReport {
        tables: Vec::new(),
        diagnostics: Vec::new(),
        type_affinities: Vec::new(),
    };
    let plan = validated_native_manager_plan_for_schema(&report);
    let managers = plan.managers();
    let achievement = find_manager(managers, "crate::AchievementDataManager");

    assert!(achievement.shape().is_none());
    assert_manager_has_table(achievement, "AchievementDataTable");
}

#[test]
fn schema_plan_preserves_generated_cache_when_table_inputs_exist() {
    let report = GameSystemDataTablesSchemaReport {
        tables: vec![
            table_schema("ArchetypeDataTable", "ArchetypeData"),
            table_schema("ArmorAppearances", "ArmorAppearanceDefinitions"),
            table_schema("DefaultAppearanceTransforms", "AppearanceTransforms"),
            table_schema("CinematicVideo", "CinematicVideoStaticData"),
            table_schema("Collectibles", "CollectibleStaticData"),
            table_schema("Crests", "CrestPartData"),
            table_schema("BaseVitals_Catacombs", "VitalsBaseData"),
            table_schema("BaseVitals_Common", "VitalsBaseData"),
            table_schema("BaseVitals_CutlassKeys", "VitalsBaseData"),
            table_schema("BaseVitals_Dunwood", "VitalsBaseData"),
            table_schema("BaseVitals_FirstLight", "VitalsBaseData"),
            table_schema("BaseVitals_IsleOfNight", "VitalsBaseData"),
            table_schema("BaseVitals_Player", "VitalsBaseData"),
            table_schema("BaseVitals_Raid_CutlassKeys", "VitalsBaseData"),
            table_schema("BaseVitals_WorldBoss", "VitalsBaseData"),
            table_schema("FactionStatusEffect", "FactionStatusEffectData"),
            table_schema("ArenaPvpBalanceTable", "ArenaBalanceData"),
            table_schema("CaptureTheFlagPvpBalanceTable", "CaptureTheFlagBalanceData"),
            table_schema("DuelPvpBalanceTable", "DuelBalanceData"),
            table_schema("FFAZonePvpBalanceTable", "FFAZoneBalanceData"),
            table_schema("OpenWorldPvpBalanceTable", "OpenWorldBalanceData"),
            table_schema("OutpostRushPvpBalanceTable", "OutpostRushBalanceData"),
            table_schema(
                "OutpostRush_NoPerksPvpBalanceTable",
                "OutpostRush_NoPerksBalanceData",
            ),
            table_schema("WarPvpBalanceTable", "WarBalanceData"),
        ],
        diagnostics: Vec::new(),
        type_affinities: Vec::new(),
    };
    let plan = validated_native_manager_plan_for_schema(&report);
    let managers = plan.managers();

    assert!(
        find_manager(managers, "crate::ArchetypeDataManager")
            .shape()
            .is_some()
    );
    assert!(
        find_manager(managers, "crate::ArmorAppearanceDataManager")
            .shape()
            .is_some()
    );
    assert!(
        find_manager(managers, "crate::AppearanceTransformDataManager")
            .shape()
            .is_some()
    );
    assert!(
        find_manager(managers, "crate::CinematicVideoStaticDataManager")
            .shape()
            .is_some()
    );
    assert!(
        find_manager(managers, "crate::CollectibleStaticDataManager")
            .shape()
            .is_some()
    );
    assert!(matches!(
        find_manager(managers, "crate::CrestPartDataManager").shape(),
        Some(NativeManagerShape::OneTableCrestPart(_))
    ));
    assert!(matches!(
        find_manager(managers, "crate::VitalsBaseDataManager").shape(),
        Some(NativeManagerShape::TableFamilyCrcKeyProjection(_))
    ));
    assert!(
        find_manager(managers, "crate::FactionStatusEffectDataManager")
            .shape()
            .is_some()
    );
    assert!(
        find_manager(managers, "crate::ArenaPvpBalanceDataManager")
            .shape()
            .is_some()
    );
    assert!(
        find_manager(managers, "crate::CaptureTheFlagPvpBalanceDataManager")
            .shape()
            .is_some()
    );
    assert!(
        find_manager(managers, "crate::DuelPvpBalanceDataManager")
            .shape()
            .is_some()
    );
    assert!(
        find_manager(managers, "crate::FFAZonePvpBalanceDataManager")
            .shape()
            .is_some()
    );
    assert!(
        find_manager(managers, "crate::OpenWorldPvpBalanceDataManager")
            .shape()
            .is_some()
    );
    assert!(
        find_manager(managers, "crate::OprPvpBalanceDataManager")
            .shape()
            .is_some()
    );
    assert!(
        find_manager(managers, "crate::OutPostRushNoPerksPvpBalanceDataManager",)
            .shape()
            .is_some()
    );
    assert!(
        find_manager(managers, "crate::WarPvpBalanceDataManager")
            .shape()
            .is_some()
    );
    assert!(
        find_manager(managers, "crate::AITargetingDataManager")
            .shape()
            .is_none()
    );
    assert!(matches!(
        find_manager(managers, "crate::ArmorOffsetDataManager").shape(),
        Some(NativeManagerShape::ProductAssetResource(_))
    ));
}

#[test]
fn explicit_runtime_resources_have_clear_output_surfaces() {
    let managers = validated_native_manager_specs();

    let manager = find_manager(&managers, "crate::VitalsDataManager");
    assert_eq!(
        manager.shape().map(NativeManagerShape::resource_surface),
        Some(GeneratedManagerSurface::NativeApiManager)
    );
    assert!(
        manager
            .shape()
            .is_some_and(NativeManagerShape::exposes_native_api)
    );

    let manager = find_manager(&managers, "crate::DamageDataManager");
    assert_eq!(
        manager.shape().map(NativeManagerShape::resource_surface),
        Some(GeneratedManagerSurface::NativeApiManager)
    );
    assert!(
        manager
            .shape()
            .is_some_and(NativeManagerShape::exposes_native_api)
    );

    let manager = find_manager(&managers, "crate::CharacterAttributeDataManager");
    assert_eq!(
        manager.shape().map(NativeManagerShape::resource_surface),
        Some(GeneratedManagerSurface::NativeApiManager)
    );
    assert!(
        manager
            .shape()
            .is_some_and(NativeManagerShape::exposes_native_api)
    );

    let manager = find_manager(&managers, "crate::StatusEffectDataManager");
    assert_eq!(
        manager.shape().map(NativeManagerShape::resource_surface),
        Some(GeneratedManagerSurface::NativeApiManager)
    );
    assert!(
        manager
            .shape()
            .is_some_and(NativeManagerShape::exposes_native_api)
    );

    let manager = find_manager(&managers, "crate::TradeskillRankDataManager");
    assert_eq!(
        manager.shape().map(NativeManagerShape::resource_surface),
        Some(GeneratedManagerSurface::NativeApiManager)
    );
    assert!(
        manager
            .shape()
            .is_some_and(NativeManagerShape::exposes_native_api)
    );

    let manager = find_manager(&managers, "crate::ItemDataManager");
    assert_eq!(
        manager.shape().map(NativeManagerShape::resource_surface),
        Some(GeneratedManagerSurface::NativeApiManager)
    );
    assert!(
        manager
            .shape()
            .is_some_and(NativeManagerShape::exposes_native_api)
    );

    let manager = find_manager(&managers, "crate::ItemConversionDataManager");
    assert_eq!(
        manager.shape().map(NativeManagerShape::resource_surface),
        Some(GeneratedManagerSurface::NativeApiManager)
    );
    assert!(
        manager
            .shape()
            .is_some_and(NativeManagerShape::exposes_native_api)
    );

    let manager = find_manager(&managers, "crate::CombatProfilesDataManager");
    assert_native_api_manager_surface(manager);

    let manager = find_manager(&managers, "crate::ItemTransformDataManager");
    assert_native_api_manager_surface(manager);

    let manager = find_manager(&managers, "crate::GatherableDataManager");
    assert_native_api_manager_surface(manager);

    let manager = find_manager(&managers, "crate::SocialDataManager");
    assert_native_api_manager_surface(manager);

    let manager = find_manager(&managers, "crate::SeasonsRewardsActivitiesTasksDataManager");
    assert_native_api_manager_surface(manager);

    let manager = find_manager(&managers, "crate::SeasonsRewardsBattlePassDataManager");
    assert_native_api_manager_surface(manager);

    let manager = find_manager(&managers, "crate::SeasonsRewardsChapterDataManager");
    assert_native_api_manager_surface(manager);

    let manager = find_manager(&managers, "crate::SeasonsRewardsJourneyDataManager");
    assert_native_api_manager_surface(manager);

    let manager = find_manager(&managers, "crate::ScheduleDataManager");
    assert_native_api_manager_surface(manager);

    let manager = find_manager(&managers, "crate::MetaAchievementDataManager");
    assert_native_api_manager_surface(manager);

    let manager = find_manager(&managers, "crate::StaticBackstoryDataManager");
    assert_native_api_manager_surface(manager);

    let manager = find_manager(&managers, "crate::VitalsModifierMappingManager");
    assert_eq!(
        manager.shape().map(NativeManagerShape::resource_surface),
        Some(GeneratedManagerSurface::NativeApiManager)
    );
    assert!(
        manager
            .shape()
            .is_some_and(NativeManagerShape::exposes_native_api)
    );

    let manager = find_manager(&managers, "crate::PlayerDataManager");
    assert_eq!(
        manager.shape().map(NativeManagerShape::resource_surface),
        Some(GeneratedManagerSurface::NativeApiManager)
    );
    assert!(
        manager
            .shape()
            .is_some_and(NativeManagerShape::exposes_native_api)
    );

    let manager = find_manager(&managers, "crate::CurrencyExchangeMappingManager");
    assert_eq!(
        manager.shape().map(NativeManagerShape::resource_surface),
        Some(GeneratedManagerSurface::NativeApiManager)
    );
    assert!(
        manager
            .shape()
            .is_some_and(NativeManagerShape::exposes_native_api)
    );

    let manager = find_manager(&managers, "crate::PerkBucketDataManager");
    assert_eq!(
        manager.shape().map(NativeManagerShape::resource_surface),
        Some(GeneratedManagerSurface::NativeApiManager)
    );
    assert!(
        manager
            .shape()
            .is_some_and(NativeManagerShape::exposes_native_api)
    );

    let manager = find_manager(&managers, "crate::ReplicationDataManager");
    assert_eq!(
        manager.shape().map(NativeManagerShape::resource_surface),
        Some(GeneratedManagerSurface::NativeApiManager)
    );
    assert!(
        manager
            .shape()
            .is_some_and(NativeManagerShape::exposes_native_api)
    );

    let manager = find_manager(&managers, "crate::CameraSettingsDataManager");
    assert_eq!(
        manager.shape().map(NativeManagerShape::resource_surface),
        Some(GeneratedManagerSurface::TypedAssetResource)
    );
    assert!(
        !manager
            .shape()
            .is_some_and(NativeManagerShape::exposes_native_api)
    );

    let manager = find_manager(&managers, "crate::RecipeDataManager");
    assert_eq!(
        manager.shape().map(NativeManagerShape::resource_surface),
        Some(GeneratedManagerSurface::NativeApiManager)
    );
    assert!(
        manager
            .shape()
            .is_some_and(NativeManagerShape::exposes_native_api)
    );

    let manager = find_manager(&managers, "crate::StaticTradeskillRankDataMappingManager");
    assert_eq!(
        manager.shape().map(NativeManagerShape::resource_surface),
        Some(GeneratedManagerSurface::NativeApiManager)
    );
    assert!(
        manager
            .shape()
            .is_some_and(NativeManagerShape::exposes_native_api)
    );

    let manager = find_manager(&managers, "crate::ShopDataManager");
    assert_eq!(
        manager.shape().map(NativeManagerShape::resource_surface),
        Some(GeneratedManagerSurface::NativeApiManager)
    );
    assert!(
        manager
            .shape()
            .is_some_and(NativeManagerShape::exposes_native_api)
    );
}
