use super::*;

mod character;
mod commerce;
mod core;
mod families;
mod indexed;
mod seasons;
mod special;
mod world;

pub(super) fn bool_expression(field: &GoSchemaField, receiver: &str) -> String {
    match (field.column_type, field.required) {
        (ColumnType::Boolean, true) => format!("{receiver}.{}", field.field_name),
        (ColumnType::Boolean, false) => {
            format!("{receiver}.{0} != nil && *{receiver}.{0}", field.field_name)
        }
        (ColumnType::Number, true) => format!("{receiver}.{} != 0", field.field_name),
        (ColumnType::Number, false) => {
            format!(
                "{receiver}.{0} != nil && *{receiver}.{0} != 0",
                field.field_name
            )
        }
        (ColumnType::String, _) => format!(
            "boolFromText({})",
            indexed::string_expression(field, receiver)
        ),
    }
}

pub(super) fn optional_bool_pointer_expression(field: &GoSchemaField, receiver: &str) -> String {
    match (field.column_type, field.required) {
        (ColumnType::Boolean, true) => {
            format!("boolPointer({receiver}.{})", field.field_name)
        }
        (ColumnType::Boolean, false) => format!("{receiver}.{}", field.field_name),
        (ColumnType::Number, true) => {
            format!("boolPointer({receiver}.{} != 0)", field.field_name)
        }
        (ColumnType::Number, false) => {
            format!("optionalNumberBool({receiver}.{})", field.field_name)
        }
        (ColumnType::String, _) => format!(
            "optionalBoolFromText({})",
            indexed::string_expression(field, receiver)
        ),
    }
}

pub(super) fn residual_native_manager_augmentation(
    unit: &GameDataCompileUnit,
    manager: &DirectManagerSurface,
    shape: &NativeManagerShape,
) -> GoNativeManagerAugmentation {
    match shape {
        NativeManagerShape::OneTableCampSkin(_)
        | NativeManagerShape::OneTableStoreCategory(_)
        | NativeManagerShape::OneTableStoreProduct(_)
        | NativeManagerShape::OneTableRewardTrackItem(_) => {
            commerce::augmentation(unit, manager, shape)
        }
        NativeManagerShape::OneTableWorldEventRule(_)
        | NativeManagerShape::QuickCourseData(_)
        | NativeManagerShape::RotationalQueueData(_)
        | NativeManagerShape::ProgressionPointData(_)
        | NativeManagerShape::OneTablePvpBalance(_)
        | NativeManagerShape::OneTableParticleData(_) => world::augmentation(unit, manager, shape),
        NativeManagerShape::PostSkillCapProgression(_)
        | NativeManagerShape::OneTableCostumeChange(_)
        | NativeManagerShape::OneTableDungeonTile(_)
        | NativeManagerShape::OneTableEncumbrance(_)
        | NativeManagerShape::OneTableDifficultyScaling(_)
        | NativeManagerShape::OneTableDarkness(_) => character::augmentation(unit, manager, shape),
        NativeManagerShape::OneTableEmote(_)
        | NativeManagerShape::DynamicDifficultyData(_)
        | NativeManagerShape::EntitlementData(_)
        | NativeManagerShape::EquipmentSetData(_) => indexed::augmentation(unit, manager, shape),
        NativeManagerShape::ObjectivesData(_)
        | NativeManagerShape::ContributionData(_)
        | NativeManagerShape::BuffBucketData(_)
        | NativeManagerShape::StructureData(_)
        | NativeManagerShape::ReusableScoreboardData(_)
        | NativeManagerShape::MountHitVolumeData(_) => families::augmentation(unit, manager, shape),
        NativeManagerShape::SeasonsRewardsData(_)
        | NativeManagerShape::SeasonsTrackedStatData(_)
        | NativeManagerShape::SeasonsRewardsActivitiesTasksData(_)
        | NativeManagerShape::SeasonsRewardsBattlePassData(_)
        | NativeManagerShape::SeasonsRewardsCardTemplateData(_)
        | NativeManagerShape::SeasonsRewardsChapterData(_)
        | NativeManagerShape::SeasonsRewardsJourneyData(_)
        | NativeManagerShape::SongBookSheetData(_)
        | NativeManagerShape::SongBookData(_) => seasons::augmentation(unit, manager, shape),
        NativeManagerShape::ElementalMutationStaticData(_)
        | NativeManagerShape::PromotionMutationStaticData(_)
        | NativeManagerShape::MusicalRewardsData(_)
        | NativeManagerShape::CombatProfilesData(_)
        | NativeManagerShape::GatherableData(_)
        | NativeManagerShape::SocialData(_)
        | NativeManagerShape::PlayerData(_)
        | NativeManagerShape::RecipeData(_) => special::augmentation(unit, manager, shape),
        NativeManagerShape::OneTableDyeColor(_)
        | NativeManagerShape::RewardTrackData(_)
        | NativeManagerShape::WhisperData(_)
        | NativeManagerShape::OneTableCrestPart(_)
        | NativeManagerShape::OneTableLevelDisparity(_)
        | NativeManagerShape::CharacterAttributeData(_)
        | NativeManagerShape::GovernanceData(_)
        | NativeManagerShape::LootBucketData(_)
        | NativeManagerShape::TerritoryDefinitionsData(_)
        | NativeManagerShape::StatModifierData(_) => indexed::augmentation(unit, manager, shape),

        // These shapes are lowered before residual dispatch. Keeping them explicit makes a
        // newly misclassified surface fail loudly instead of acquiring unrelated semantics.
        NativeManagerShape::AbilityData(_)
        | NativeManagerShape::ItemConversionData(_)
        | NativeManagerShape::VitalsData(_)
        | NativeManagerShape::StatusEffectData(_)
        | NativeManagerShape::ItemTransformData(_) => core::augmentation(unit, manager, shape),

        NativeManagerShape::RequirementsOnly
        | NativeManagerShape::OneTableCrcIndex(_)
        | NativeManagerShape::TableFamilyCrcIndex(_)
        | NativeManagerShape::OneTableOwnedStringCrcIndex(_)
        | NativeManagerShape::TableFamilyOwnedStringCrcIndex(_)
        | NativeManagerShape::OneTableCrcKeyProjection(_)
        | NativeManagerShape::StaticBackstoryData(_)
        | NativeManagerShape::MultiTableCrcKeyProjection(_)
        | NativeManagerShape::TableFamilyCrcKeyProjection(_)
        | NativeManagerShape::TableFamilyFallbackCrcKeyProjection(_)
        | NativeManagerShape::TableFamilyPartitionedCrcKeyProjection(_)
        | NativeManagerShape::OneTableNumericKeyProjection(_)
        | NativeManagerShape::TableFamilyNumericKeyProjection(_)
        | NativeManagerShape::OneTableEnumKeyProjection(_)
        | NativeManagerShape::OneTableStringKeyProjection(_)
        | NativeManagerShape::OneTableRowProjection(_)
        | NativeManagerShape::OneTableExperience(_)
        | NativeManagerShape::ItemData(_)
        | NativeManagerShape::DamageData(_)
        | NativeManagerShape::CurrencyExchangeMapping(_)
        | NativeManagerShape::TradeskillRankData(_)
        | NativeManagerShape::StaticTradeskillRankDataMapping(_)
        | NativeManagerShape::VitalsModifierMapping(_)
        | NativeManagerShape::ReplicationData(_)
        | NativeManagerShape::ProductAssetResource(_)
        | NativeManagerShape::ComposedResource(_) => panic!(
            "manager {} reached residual Go native dispatch with pre-lowered shape {shape:?}",
            manager.manager_name
        ),
    }
}

pub(super) fn crc_schema_contract(
    unit: &GameDataCompileUnit,
    manager: &DirectManagerSurface,
    source_row_type: &str,
    key_column: &str,
    id_methods: &[&str],
    key_methods: &[&str],
) -> GoNativeManagerAugmentation {
    let row_specs = go_direct_row_specs(unit, manager);
    let row = row_specs
        .iter()
        .find(|row| row.source_row_type == source_row_type)
        .unwrap_or_else(|| {
            panic!(
                "Go native contract for {} requires row type {}",
                manager.manager_name, source_row_type
            )
        });
    let key_field = row
        .fields
        .iter()
        .find(|field| field.source_name.eq_ignore_ascii_case(key_column))
        .unwrap_or_else(|| {
            panic!(
                "Go native contract for {} requires key column {} on {}",
                manager.manager_name, key_column, source_row_type
            )
        });
    let default_row = go_direct_default_row_spec(unit, manager)
        .map(|row| row.source_row_type)
        .as_deref()
        == Some(source_row_type);
    let table_type = go_direct_table_type_name(manager, source_row_type, default_row);
    let row_field = go_direct_row_field_name(source_row_type);
    let row_type = &row.type_name;
    let field = &key_field.field_name;
    let manager_type = go_method_name(&manager.manager_class_name);
    let table_key_type = format!("{manager_type}CRCKey");
    let mut methods = String::new();
    for method in id_methods {
        let method = go_method_name(method);
        methods.push_str(&format!(
            r#"func (manager *{manager_type}) {method}(id gametypes.CRC32) *{row_type} {{
	index, ok := manager.rowsByID[id]
	if !ok {{ return nil }}
	return rowCopy(manager.{row_field}.entries[index].Row)
}}

"#
        ));
    }
    let canonical_id_method = id_methods
        .first()
        .map(|method| go_method_name(method))
        .expect("CRC native contracts require an id method");
    for method in key_methods {
        let method = go_method_name(method);
        methods.push_str(&format!(
            r#"func (manager *{manager_type}) {method}(key string) *{row_type} {{
	return manager.{canonical_id_method}(gametypes.CRC32(crc32Lowercase(key)))
}}

"#
        ));
    }
    methods.push_str(&format!(
        r#"func (manager *{manager_type}) RowInTable(table {table_type}, id gametypes.CRC32) *{row_type} {{
	index, ok := manager.rowsByTableAndID[{table_key_type}{{table: table, id: id}}]
	if !ok {{ return nil }}
	return rowCopy(manager.{row_field}.entries[index].Row)
}}

"#
    ));

    GoNativeManagerAugmentation {
        declarations: format!(
            "\ntype {table_key_type} struct {{ table {table_type}; id gametypes.CRC32 }}\n"
        ),
        fields: format!(
            "\trowsByID map[gametypes.CRC32]int\n\trowsByTableAndID map[{table_key_type}]int\n"
        ),
        field_values: format!(
            "\t\trowsByID: make(map[gametypes.CRC32]int),\n\t\trowsByTableAndID: make(map[{table_key_type}]int),\n"
        ),
        initializers: format!(
            r#"	for index := range manager.{row_field}.entries {{
		source := &manager.{row_field}.entries[index]
		key := strings.TrimSpace(source.Row.{field})
		id := gametypes.CRC32(crc32Lowercase(key))
		if key == "" || id == 0 {{ continue }}
		tableKey := {table_key_type}{{table: source.Ref.Table(), id: id}}
		if _, exists := manager.rowsByTableAndID[tableKey]; !exists {{
			manager.rowsByTableAndID[tableKey] = index
		}}
		if _, exists := manager.rowsByID[id]; !exists {{
			manager.rowsByID[id] = index
		}}
	}}
"#
        ),
        methods,
    }
}

pub(super) fn merge_augmentations(
    augmentations: impl IntoIterator<Item = GoNativeManagerAugmentation>,
) -> GoNativeManagerAugmentation {
    let mut merged = GoNativeManagerAugmentation::default();
    for augmentation in augmentations {
        merged.declarations.push_str(&augmentation.declarations);
        merged.fields.push_str(&augmentation.fields);
        merged.field_values.push_str(&augmentation.field_values);
        merged.initializers.push_str(&augmentation.initializers);
        merged.methods.push_str(&augmentation.methods);
    }
    merged
}
