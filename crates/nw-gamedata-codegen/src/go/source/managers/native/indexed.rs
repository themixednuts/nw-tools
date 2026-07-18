use super::*;
use crate::manager::{NativeCrcIndexLookupMethod, NativeCrcIndexLookupParameterKind};

pub(super) fn augmentation(
    unit: &GameDataCompileUnit,
    manager: &DirectManagerSurface,
    shape: &NativeManagerShape,
) -> GoNativeManagerAugmentation {
    match shape {
        NativeManagerShape::OneTableCampSkin(_) => simple_crc(
            unit,
            manager,
            "CampSkinData",
            "CampSkinID",
            &["CampSkinDataFromID"],
            &["CampSkinData", "CampSkinDataByKey"],
            Some("CampSkins"),
        ),
        NativeManagerShape::OneTableEmote(_) => emote_contract(unit, manager),
        NativeManagerShape::OneTableStoreCategory(_) => simple_crc(
            unit,
            manager,
            "StoreCategoryProperties",
            "StoreCategory",
            &["StoreCategoryPropertiesFromID"],
            &["StoreCategoryProperties", "StoreCategoryPropertiesByName"],
            Some("Categories"),
        ),
        NativeManagerShape::OneTableStoreProduct(_) => simple_crc(
            unit,
            manager,
            "StoreProductData",
            "UniqueTagID",
            &["StoreProductDataFromID"],
            &["StoreProductData", "StoreProductDataByTag"],
            Some("Products"),
        ),
        NativeManagerShape::OneTableRewardTrackItem(_) => simple_crc(
            unit,
            manager,
            "RewardTrackItemData",
            "RewardID",
            &["RewardTrackItemFromID"],
            &["RewardTrackItem", "RewardTrackItemByKey"],
            Some("RewardTrackItems"),
        ),
        NativeManagerShape::OneTableWorldEventRule(_) => simple_crc(
            unit,
            manager,
            "WorldEventRuleData",
            "RuleID",
            &["WorldEventRuleByCRC32"],
            &["WorldEventRule"],
            Some("WorldEventRules"),
        ),
        NativeManagerShape::RotationalQueueData(_) => simple_crc(
            unit,
            manager,
            "RotationalQueueData",
            "RotationalQueueID",
            &["RotationalQueueFromID"],
            &["RotationalQueue"],
            Some("RotationalQueues"),
        ),
        NativeManagerShape::DynamicDifficultyData(_) => dynamic_difficulty(unit, manager),
        NativeManagerShape::ProgressionPointData(_) => simple_crc(
            unit,
            manager,
            "ProgressionPointData",
            "ProgressionPointID",
            &["ProgressionPointFromID"],
            &["ProgressionPoint"],
            Some("ProgressionPoints"),
        ),
        NativeManagerShape::EntitlementData(_) => entitlement_contract(unit, manager),
        NativeManagerShape::EquipmentSetData(_) => equipment_set_contract(unit, manager),
        NativeManagerShape::OneTablePvpBalance(shape) => crc_contract_from_shape(
            unit,
            manager,
            shape.row_type_name().as_str(),
            shape.target_column().as_str(),
            shape.methods(),
            shape.balances_method().map(|value| value.as_str()),
        ),
        NativeManagerShape::OneTableDyeColor(_) => dye_color_contract(unit, manager),
        NativeManagerShape::RewardTrackData(_) => reward_track_contract(unit, manager),
        NativeManagerShape::PostSkillCapProgression(_) => crc_schema_contract(
            unit,
            manager,
            "TradeSkillPostCapData",
            "TradeSkillType",
            &["PostSkillCapProgressionDataFromID"],
            &["PostSkillCapProgressionData"],
        ),
        NativeManagerShape::WhisperData(_) => whisper_contract(unit, manager),
        NativeManagerShape::OneTableCostumeChange(_) => crc_schema_contract(
            unit,
            manager,
            "CostumeChangeData",
            "CostumeChangeId",
            &["CostumeChangeDataFromID"],
            &["CostumeChangeData", "CostumeChangeDataByKey"],
        ),
        NativeManagerShape::OneTableCrestPart(_) => numeric_contract(
            unit,
            manager,
            "CrestPartData",
            "Index",
            "uint32",
            &["CrestPartDataFromID", "CrestPartDataFromIndex"],
            Some("CrestParts"),
        ),
        NativeManagerShape::OneTableDungeonTile(_) => crc_schema_contract(
            unit,
            manager,
            "DungeonTileStaticData",
            "DungeonTileId",
            &["DungeonTileStaticData"],
            &["DungeonTileStaticDataByKey"],
        ),
        NativeManagerShape::OneTableLevelDisparity(_) => level_disparity_contract(unit, manager),
        NativeManagerShape::OneTableEncumbrance(_) => crc_schema_contract(
            unit,
            manager,
            "EncumbranceData",
            "ContainerTypeID",
            &["EncumbranceDataFromID"],
            &["EncumbranceData", "EncumbranceDataByKey"],
        ),
        NativeManagerShape::OneTableDifficultyScaling(_) => crc_schema_contract(
            unit,
            manager,
            "DifficultyScalingData",
            "WorldEncounterID",
            &["DifficultyScalingDataFromID"],
            &["DifficultyScalingData", "DifficultyScalingDataByKey"],
        ),
        NativeManagerShape::OneTableDarkness(_) => crc_schema_contract(
            unit,
            manager,
            "DarknessData",
            "DarknessId",
            &["DarknessDataByCRC32"],
            &["DarknessData"],
        ),
        NativeManagerShape::OneTableParticleData(_) => crc_schema_contract(
            unit,
            manager,
            "ParticleData",
            "Effect Name",
            &["ParticleDataFromID"],
            &["ParticleData", "ParticleDataByKey"],
        ),
        NativeManagerShape::CharacterAttributeData(_) => {
            character_attribute_contract(unit, manager)
        }
        NativeManagerShape::GovernanceData(_) => governance_contract(unit, manager),
        NativeManagerShape::LootBucketData(_) => loot_bucket_contract(unit, manager),
        NativeManagerShape::TerritoryDefinitionsData(_) => territory_contract(unit, manager),
        NativeManagerShape::StatModifierData(_) => stat_modifier_contract(unit, manager),
        _ => panic!(
            "manager {} reached indexed Go native dispatch with {shape:?}",
            manager.manager_name
        ),
    }
}

fn simple_crc(
    unit: &GameDataCompileUnit,
    manager: &DirectManagerSurface,
    row_type: &str,
    key_column: &str,
    id_methods: &[&str],
    key_methods: &[&str],
    rows_method: Option<&str>,
) -> GoNativeManagerAugmentation {
    let mut augmentation =
        crc_schema_contract(unit, manager, row_type, key_column, id_methods, key_methods);
    if let Some(rows_method) = rows_method {
        augmentation
            .methods
            .push_str(&named_rows_method(unit, manager, row_type, rows_method));
    }
    augmentation
}

fn emote_contract(
    unit: &GameDataCompileUnit,
    manager: &DirectManagerSurface,
) -> GoNativeManagerAugmentation {
    let mut augmentation = simple_crc(
        unit,
        manager,
        "EmoteData",
        "UniqueTagID",
        &["EmoteDataFromID"],
        &["EmoteData", "EmoteDataByKey"],
        Some("Emotes"),
    );
    let row = required_row(unit, manager, "EmoteData");
    if let Some(status) = optional_field(&row, "StatusEffectTimer") {
        let id = required_field(&row, "UniqueTagID");
        let row_field = go_direct_row_field_name("EmoteData");
        let status_expression = string_expression(status, "source.Row");
        let id_expression = string_expression(id, "source.Row");
        augmentation
            .fields
            .push_str("\temoteIDByStatusEffect map[gametypes.CRC32]gametypes.CRC32\n");
        augmentation
            .field_values
            .push_str("\t\temoteIDByStatusEffect: make(map[gametypes.CRC32]gametypes.CRC32),\n");
        augmentation.initializers.push_str(&format!(
            r#"	for index := range manager.{row_field}.entries {{
		source := &manager.{row_field}.entries[index]
		statusID := gametypes.CRC32(crc32Lowercase(strings.TrimSpace({status_expression})))
		emoteID := gametypes.CRC32(crc32Lowercase(strings.TrimSpace({id_expression})))
		if statusID != 0 && emoteID != 0 {{ if _, exists := manager.emoteIDByStatusEffect[statusID]; !exists {{ manager.emoteIDByStatusEffect[statusID] = emoteID }} }}
	}}
"#
        ));
    }
    augmentation.methods.push_str(
        r#"func (manager *EmoteDataManager) EmoteIDByStatusEffect(statusEffectID gametypes.CRC32) (gametypes.CRC32, bool) { id, ok := manager.emoteIDByStatusEffect[statusEffectID]; return id, ok }
func (manager *EmoteDataManager) EmoteIDForStatusEffect(statusEffectKey string) (gametypes.CRC32, bool) { return manager.EmoteIDByStatusEffect(gametypes.CRC32(crc32Lowercase(statusEffectKey))) }

"#,
    );
    augmentation
}

fn dye_color_contract(
    unit: &GameDataCompileUnit,
    manager: &DirectManagerSurface,
) -> GoNativeManagerAugmentation {
    let row = required_row(unit, manager, "DyeColorData");
    let row_field = go_direct_row_field_name("DyeColorData");
    let manager_type = go_method_name(&manager.manager_class_name);
    let default = go_direct_default_row_spec(unit, manager)
        .map(|row| row.source_row_type)
        .as_deref()
        == Some("DyeColorData");
    let table_type = go_direct_table_type_name(manager, "DyeColorData", default);
    let index = number_expression(required_field(&row, "Index"), "source.Row");
    let name = string_expression(required_field(&row, "Name"), "source.Row");
    let color = string_expression(required_field(&row, "Color"), "source.Row");
    let category = string_expression(required_field(&row, "Category"), "source.Row");
    let entitlement = bool_expression(required_field(&row, "IsEntitlement"), "source.Row");
    let color_amount = number_expression(required_field(&row, "ColorAmount"), "source.Row");
    let color_override = number_expression(required_field(&row, "ColorOverride"), "source.Row");
    let spec_color = string_expression(required_field(&row, "SpecColor"), "source.Row");
    let spec_amount = number_expression(required_field(&row, "SpecAmount"), "source.Row");
    let mask_gloss_shift = number_expression(required_field(&row, "MaskGlossShift"), "source.Row");

    GoNativeManagerAugmentation {
        declarations: format!(
            r##"
type DyeColorIndex uint8

type DyeColorData struct {{
	Index DyeColorIndex
	Name string
	Color ColorRGBA
	Category string
	IsEntitlement bool
	ColorAmount float32
	ColorOverride float32
	SpecColor ColorRGBA
	SpecAmount float32
	MaskGlossShift float32
	Source RowRef[{table_type}, {row_type}]
}}

func parseDyeColor(value string) (ColorRGBA, error) {{
	value = strings.TrimSpace(value)
	value = strings.TrimPrefix(value, "#")
	if len(value) == 6 {{ value += "ff" }}
	if len(value) != 8 {{ return ColorRGBA{{}}, fmt.Errorf("dye color %q must contain 6 or 8 hexadecimal digits", value) }}
	raw, err := strconv.ParseUint(value, 16, 32)
	if err != nil {{ return ColorRGBA{{}}, fmt.Errorf("decode dye color %q: %w", value, err) }}
	channel := func(shift uint) float32 {{ return float32((raw >> shift) & 0xff) / 255 }}
	return ColorRGBA{{R: channel(24), G: channel(16), B: channel(8), A: channel(0)}}, nil
}}
"##,
            row_type = row.type_name,
        ),
        fields: "\tdyeColors []DyeColorData\n\tdyeColorsByIndex map[DyeColorIndex]int\n\tdyeEntitlementIndexes []DyeColorIndex\n".to_owned(),
        field_values: "\t\tdyeColorsByIndex: make(map[DyeColorIndex]int),\n".to_owned(),
        initializers: format!(
            r#"	for sourceIndex := range manager.{row_field}.entries {{
		source := &manager.{row_field}.entries[sourceIndex]
		colorText := strings.TrimSpace({color})
		if colorText == "" {{ continue }}
		rawIndex, ok := exactUint32({index})
		if !ok || rawIndex == 0 || rawIndex > 255 {{ return nil, fmt.Errorf("DyeColorData Index %v is outside the non-zero byte key range", {index}) }}
		dyeIndex := DyeColorIndex(rawIndex)
		if _, exists := manager.dyeColorsByIndex[dyeIndex]; exists {{ continue }}
		baseColor, err := parseDyeColor(colorText)
		if err != nil {{ return nil, fmt.Errorf("DyeColorData Index %d: %w", rawIndex, err) }}
		specText := strings.TrimSpace({spec_color})
		specColor := baseColor
		if specText != "" {{ specColor, err = parseDyeColor(specText); if err != nil {{ return nil, fmt.Errorf("DyeColorData Index %d SpecColor: %w", rawIndex, err) }} }}
		data := DyeColorData{{Index: dyeIndex, Name: strings.TrimSpace({name}), Color: baseColor, Category: strings.TrimSpace({category}), IsEntitlement: {entitlement}, ColorAmount: {color_amount}, ColorOverride: {color_override}, SpecColor: specColor, SpecAmount: {spec_amount}, MaskGlossShift: {mask_gloss_shift}, Source: source.Ref}}
		manager.dyeColorsByIndex[dyeIndex] = len(manager.dyeColors)
		manager.dyeColors = append(manager.dyeColors, data)
		if data.IsEntitlement {{ manager.dyeEntitlementIndexes = append(manager.dyeEntitlementIndexes, dyeIndex) }}
	}}
	slices.Sort(manager.dyeEntitlementIndexes)
"#
        ),
        methods: format!(
            r#"func (manager *{manager_type}) DyeColorData(index DyeColorIndex) *DyeColorData {{ position, ok := manager.dyeColorsByIndex[index]; if !ok {{ return nil }}; return rowCopy(manager.dyeColors[position]) }}
func (manager *{manager_type}) DyeColorDataFromIndex(index uint8) *DyeColorData {{ if index == 0 {{ return nil }}; return manager.DyeColorData(DyeColorIndex(index)) }}
func (manager *{manager_type}) DyeColorDataByKey(index DyeColorIndex) *DyeColorData {{ return manager.DyeColorData(index) }}
func (manager *{manager_type}) DyeColors() iter.Seq[DyeColorData] {{ return rowValues(manager.dyeColors) }}
func (manager *{manager_type}) Rows() iter.Seq[DyeColorData] {{ return manager.DyeColors() }}
func (manager *{manager_type}) EntitlementIndexes() iter.Seq[DyeColorIndex] {{ return slices.Values(manager.dyeEntitlementIndexes) }}
func (manager *{manager_type}) Len() int {{ return len(manager.dyeColors) }}
func (manager *{manager_type}) IsEmpty() bool {{ return len(manager.dyeColors) == 0 }}

"#
        ),
    }
}

fn dynamic_difficulty(
    unit: &GameDataCompileUnit,
    manager: &DirectManagerSurface,
) -> GoNativeManagerAugmentation {
    let row = required_row(unit, manager, "DynamicDifficultyStaticData");
    let id = required_field(&row, "DynamicDifficultyID");
    let row_field = go_direct_row_field_name("DynamicDifficultyStaticData");
    let table_type = go_direct_table_type_name(manager, "DynamicDifficultyStaticData", true);
    let row_name = row.type_name.clone();
    let manager_type = go_method_name(&manager.manager_class_name);
    let id_expression = string_expression(id, "source.Row");
    let game_modes = optional_field(&row, "GameModeIds")
        .map(|field| string_expression(field, "source.Row"))
        .unwrap_or_else(|| "\"\"".to_owned());
    let tier = optional_field(&row, "DifficultyTier")
        .map(|field| number_expression(field, "source.Row"))
        .unwrap_or_else(|| "0".to_owned());
    let mut status_initializers = String::new();
    let mut potency_initializers = String::new();
    for slot in 1..=5u8 {
        let status_column = format!("StatusEffect_{slot}");
        let Some(status) = optional_field(&row, &status_column)
            .or_else(|| optional_field(&row, &format!("StatusEffect{slot}")))
        else {
            continue;
        };
        let status_expression = string_expression(status, "source.Row");
        status_initializers.push_str(&format!(
            r#"		effect{slot}Key := strings.TrimSpace({status_expression})
		effect{slot}ID := gametypes.CRC32(crc32Lowercase(effect{slot}Key))
		if effect{slot}ID != 0 {{ data.StatusEffects = append(data.StatusEffects, DynamicDifficultyStatusEffect{{Slot: {slot}, Key: effect{slot}Key, ID: effect{slot}ID}}) }}
"#
        ));
        for creature in [
            "Catacombs",
            "Catacombs-",
            "Catacombs+",
            "CatacombsMiniBoss",
            "CatacombsBoss",
        ] {
            let candidates = [
                format!("StatusEffect_{slot}_Potency_{creature}"),
                format!("StatusEffect{slot}Potency{creature}"),
            ];
            let Some(field) = candidates
                .iter()
                .find_map(|candidate| optional_field(&row, candidate))
            else {
                continue;
            };
            let potency = number_expression(field, "source.Row");
            potency_initializers.push_str(&format!(
                r#"		if effect{slot}ID != 0 {{
			creatureID := gametypes.CRC32(crc32Lowercase({creature:?}))
			if _, exists := _vitalsData.creatureTypeIDSet[creatureID]; exists {{
				data.Potencies = append(data.Potencies, DynamicDifficultyStatusEffectPotency{{Slot: {slot}, CreatureTypeID: creatureID, StatusEffectID: effect{slot}ID, Potency: {potency}}})
			}}
		}}
"#
            ));
        }
    }
    GoNativeManagerAugmentation {
        declarations: format!(
            r#"
type DynamicDifficultyStatusEffect struct {{ Slot uint8; Key string; ID gametypes.CRC32 }}
type DynamicDifficultyStatusEffectPotency struct {{ Slot uint8; CreatureTypeID gametypes.CRC32; StatusEffectID gametypes.CRC32; Potency float32 }}
type DynamicDifficultyData struct {{
	Source RowRef[{table_type}, {row_name}]
	Key string
	ID gametypes.CRC32
	GameModeIDs []gametypes.CRC32
	DifficultyTier uint8
	StatusEffects []DynamicDifficultyStatusEffect
	Potencies []DynamicDifficultyStatusEffectPotency
}}
"#
        ),
        fields: "\tdynamicDifficulties []DynamicDifficultyData\n\tdynamicDifficultiesByID map[gametypes.CRC32]int\n\tdynamicDifficultiesBySource map[RowSlot<INVALID>]int\n"
            .replace("RowSlot<INVALID>", &format!("RowSlot[{table_type}, {row_name}]")),
        field_values: "\t\tdynamicDifficultiesByID: make(map[gametypes.CRC32]int),\n\t\tdynamicDifficultiesBySource: make(map[RowSlot<INVALID>]int),\n"
            .replace("RowSlot<INVALID>", &format!("RowSlot[{table_type}, {row_name}]")),
        initializers: format!(
            r#"	for index := range manager.{row_field}.entries {{
		source := &manager.{row_field}.entries[index]
		key := strings.TrimSpace({id_expression})
		id := gametypes.CRC32(crc32Lowercase(key))
		if key == "" || id == 0 {{ continue }}
		if _, exists := manager.dynamicDifficultiesByID[id]; exists {{ continue }}
		tier, ok := exactUint32({tier})
		if !ok || tier > 255 {{ continue }}
		data := DynamicDifficultyData{{Source: source.Ref, Key: key, ID: id, DifficultyTier: uint8(tier)}}
		for _, gameMode := range splitDesignerList({game_modes}) {{
			gameModeID := gametypes.CRC32(crc32Lowercase(gameMode))
			if gameModeID != 0 {{ data.GameModeIDs = append(data.GameModeIDs, gameModeID) }}
		}}
{status_initializers}{potency_initializers}		manager.dynamicDifficultiesByID[id] = len(manager.dynamicDifficulties)
		manager.dynamicDifficultiesBySource[source.Slot] = len(manager.dynamicDifficulties)
		manager.dynamicDifficulties = append(manager.dynamicDifficulties, data)
	}}
"#
        ),
        methods: format!(
            r#"func (manager *{manager_type}) DynamicDifficultyDataFromID(id gametypes.CRC32) *DynamicDifficultyData {{
	index, ok := manager.dynamicDifficultiesByID[id]
	if !ok {{ return nil }}
	return rowCopy(manager.dynamicDifficulties[index])
}}

func (manager *{manager_type}) DynamicDifficultyData(key string) *DynamicDifficultyData {{ return manager.DynamicDifficultyDataFromID(gametypes.CRC32(crc32Lowercase(key))) }}
func (manager *{manager_type}) DynamicDifficultyDataByKey(key string) *DynamicDifficultyData {{ return manager.DynamicDifficultyData(key) }}

func (manager *{manager_type}) DynamicDifficultyForSource(source RowSlot[{table_type}, {row_name}]) *DynamicDifficultyData {{
	index, ok := manager.dynamicDifficultiesBySource[source]
	if !ok {{ return nil }}
	return rowCopy(manager.dynamicDifficulties[index])
}}

func (manager *{manager_type}) DynamicDifficulties() iter.Seq[DynamicDifficultyData] {{ return rowValues(manager.dynamicDifficulties) }}
func (manager *{manager_type}) Rows() iter.Seq[DynamicDifficultyData] {{ return manager.DynamicDifficulties() }}

"#
        ),
    }
}

fn entitlement_contract(
    unit: &GameDataCompileUnit,
    manager: &DirectManagerSurface,
) -> GoNativeManagerAugmentation {
    let mut augmentation = simple_crc(
        unit,
        manager,
        "EntitlementData",
        "UniqueTagID",
        &["ByID"],
        &["ByKey"],
        Some("Entitlements"),
    );
    let row = required_row(unit, manager, "EntitlementData");
    let index_field = required_field(&row, "EntitlementIndex");
    let rewards_field = required_field(&row, "Reward(s)");
    let row_field = go_direct_row_field_name("EntitlementData");
    let manager_type = go_method_name(&manager.manager_class_name);
    let row_name = row.type_name.clone();
    augmentation.fields.push_str(
        "\tentitlementsByIndex map[uint32]int\n\tentitlementsByReward map[gametypes.CRC32][]int\n",
    );
    augmentation.field_values.push_str(
        "\t\tentitlementsByIndex: make(map[uint32]int),\n\t\tentitlementsByReward: make(map[gametypes.CRC32][]int),\n",
    );
    augmentation.initializers.push_str(&format!(
        r#"	for index := range manager.{row_field}.entries {{
		row := rowCopy(manager.{row_field}.entries[index].Row)
		if row.{index_field} != nil {{ if value, ok := exactUint32(*row.{index_field}); ok {{ manager.entitlementsByIndex[value] = index }} }}
		if row.{rewards_field} != nil {{
			for _, reward := range splitDesignerList(*row.{rewards_field}) {{
				id := gametypes.CRC32(crc32Lowercase(reward))
				if id != 0 {{ manager.entitlementsByReward[id] = append(manager.entitlementsByReward[id], index) }}
			}}
		}}
	}}
"#,
        index_field = index_field.field_name,
        rewards_field = rewards_field.field_name,
    ));
    augmentation.methods.push_str(&format!(
        r#"func (manager *{manager_type}) ByIndex(index uint32) *{row_name} {{
	row, ok := manager.entitlementsByIndex[index]
	if !ok {{ return nil }}
	return rowCopy(manager.{row_field}.entries[row].Row)
}}

func (manager *{manager_type}) EntitlementsForReward(reward gametypes.CRC32) iter.Seq[{row_name}] {{
	return func(yield func({row_name}) bool) {{
		for _, index := range manager.entitlementsByReward[reward] {{ if !yield(manager.{row_field}.entries[index].Row) {{ return }} }}
	}}
}}

"#
    ));
    augmentation
}

fn equipment_set_contract(
    unit: &GameDataCompileUnit,
    manager: &DirectManagerSurface,
) -> GoNativeManagerAugmentation {
    let mut augmentation = simple_crc(
        unit,
        manager,
        "EquipmentSetData",
        "EquipmentSetID",
        &["ByID"],
        &["ByKey"],
        Some("Sets"),
    );
    let row = required_row(unit, manager, "EquipmentSetData");
    let items = required_field(&row, "ItemIds");
    let perks = row
        .fields
        .iter()
        .filter(|field| {
            field.source_name.starts_with("Perk") && !field.source_name.ends_with("Threshold")
        })
        .map(|field| field.field_name.clone())
        .collect::<Vec<_>>();
    let row_field = go_direct_row_field_name("EquipmentSetData");
    let manager_type = go_method_name(&manager.manager_class_name);
    let row_name = row.type_name.clone();
    let perk_initializers = perks
        .iter()
        .map(|field| {
            format!(
                "\t\tif row.{field} != nil {{ key := gametypes.CRC32(crc32Lowercase(strings.TrimSpace(*row.{field}))); if key != 0 {{ manager.equipmentSetsByPerk[key] = append(manager.equipmentSetsByPerk[key], index) }} }}\n"
            )
        })
        .collect::<String>();
    augmentation.fields.push_str(
        "\tequipmentSetsByItem map[gametypes.CRC32][]int\n\tequipmentSetsByPerk map[gametypes.CRC32][]int\n",
    );
    augmentation.field_values.push_str(
        "\t\tequipmentSetsByItem: make(map[gametypes.CRC32][]int),\n\t\tequipmentSetsByPerk: make(map[gametypes.CRC32][]int),\n",
    );
    augmentation.initializers.push_str(&format!(
        r#"	for index := range manager.{row_field}.entries {{
		row := rowCopy(manager.{row_field}.entries[index].Row)
		if row.{items_field} != nil {{
			for _, item := range splitDesignerList(*row.{items_field}) {{ key := gametypes.CRC32(crc32Lowercase(item)); if key != 0 {{ manager.equipmentSetsByItem[key] = append(manager.equipmentSetsByItem[key], index) }} }}
		}}
{perk_initializers}	}}
"#,
        items_field = items.field_name,
    ));
    augmentation.methods.push_str(&format!(
        r#"func (manager *{manager_type}) SetsForItem(item gametypes.CRC32) iter.Seq[{row_name}] {{
	return func(yield func({row_name}) bool) {{ for _, index := range manager.equipmentSetsByItem[item] {{ if !yield(manager.{row_field}.entries[index].Row) {{ return }} }} }}
}}

func (manager *{manager_type}) SetsForPerk(perk gametypes.CRC32) iter.Seq[{row_name}] {{
	return func(yield func({row_name}) bool) {{ for _, index := range manager.equipmentSetsByPerk[perk] {{ if !yield(manager.{row_field}.entries[index].Row) {{ return }} }} }}
}}

"#
    ));
    augmentation
}

fn crc_contract_from_shape(
    unit: &GameDataCompileUnit,
    manager: &DirectManagerSurface,
    row_type: &str,
    key_column: &str,
    methods: &[NativeCrcIndexLookupMethod],
    rows_method: Option<&str>,
) -> GoNativeManagerAugmentation {
    let mut by_id = Vec::new();
    let mut by_key = Vec::new();
    for method in methods {
        match method.parameter().kind() {
            NativeCrcIndexLookupParameterKind::Crc32
            | NativeCrcIndexLookupParameterKind::IntoCrc32 => by_id.push(method.name().as_str()),
            NativeCrcIndexLookupParameterKind::StrRef
            | NativeCrcIndexLookupParameterKind::AsRefStr => by_key.push(method.name().as_str()),
        }
    }
    let mut augmentation =
        crc_schema_contract(unit, manager, row_type, key_column, &by_id, &by_key);
    if let Some(rows_method) = rows_method {
        augmentation
            .methods
            .push_str(&named_rows_method(unit, manager, row_type, rows_method));
    }
    augmentation
}

pub(super) fn named_rows_method(
    unit: &GameDataCompileUnit,
    manager: &DirectManagerSurface,
    row_type: &str,
    method: &str,
) -> String {
    let row = required_row(unit, manager, row_type);
    let field = go_direct_row_field_name(row_type);
    let manager_type = go_method_name(&manager.manager_class_name);
    let method = go_method_name(method);
    let schema_row = row.type_name;
    format!(
        r#"func (manager *{manager_type}) {method}() iter.Seq[{schema_row}] {{
	return func(yield func({schema_row}) bool) {{
		for index := range manager.{field}.entries {{
			if !yield(manager.{field}.entries[index].Row) {{ return }}
		}}
	}}
}}

"#
    )
}

pub(super) fn numeric_contract(
    unit: &GameDataCompileUnit,
    manager: &DirectManagerSurface,
    row_type: &str,
    key_column: &str,
    key_type: &str,
    methods: &[&str],
    rows_method: Option<&str>,
) -> GoNativeManagerAugmentation {
    let row = required_row(unit, manager, row_type);
    let key = required_field(&row, key_column);
    let row_field = go_direct_row_field_name(row_type);
    let row_name = row.type_name.clone();
    let manager_type = go_method_name(&manager.manager_class_name);
    let map_name = format!("{row_type} by number");
    let map_field = go_local_name(&map_name);
    let (key_guard, key_expression) = numeric_key_parts(key, key_type);
    let mut method_source = String::new();
    for method in methods {
        let method = go_method_name(method);
        method_source.push_str(&format!(
            r#"func (manager *{manager_type}) {method}(key {key_type}) *{row_name} {{
	index, ok := manager.{map_field}[key]
	if !ok {{ return nil }}
	return rowCopy(manager.{row_field}.entries[index].Row)
}}

"#
        ));
    }
    if let Some(rows_method) = rows_method {
        method_source.push_str(&named_rows_method(unit, manager, row_type, rows_method));
    }
    GoNativeManagerAugmentation {
        declarations: String::new(),
        fields: format!("\t{map_field} map[{key_type}]int\n"),
        field_values: format!("\t\t{map_field}: make(map[{key_type}]int),\n"),
        initializers: format!(
            r#"	for index := range manager.{row_field}.entries {{
		source := &manager.{row_field}.entries[index]
		{key_guard}
		key := {key_expression}
		if _, exists := manager.{map_field}[key]; !exists {{ manager.{map_field}[key] = index }}
	}}
"#
        ),
        methods: method_source,
    }
}

fn numeric_key_parts(field: &GoSchemaField, key_type: &str) -> (String, String) {
    let value = format!("source.Row.{}", field.field_name);
    if field.required {
        (String::new(), format!("{key_type}({value})"))
    } else {
        (
            format!("if {value} == nil {{ continue }}"),
            format!("{key_type}(*{value})"),
        )
    }
}

fn whisper_contract(
    unit: &GameDataCompileUnit,
    manager: &DirectManagerSurface,
) -> GoNativeManagerAugmentation {
    merge_augmentations([
        crc_schema_contract(
            unit,
            manager,
            "WhisperData",
            "WhisperId",
            &["WhisperDataFromID"],
            &["WhisperData", "WhisperDataByKey"],
        ),
        crc_secondary_contract(
            unit,
            manager,
            "WhisperVfxData",
            "WhisperVfxId",
            "WhisperVfxFromID",
            "WhisperVfx",
            "whisperVfxByID",
        ),
    ])
}

fn character_attribute_contract(
    unit: &GameDataCompileUnit,
    manager: &DirectManagerSurface,
) -> GoNativeManagerAugmentation {
    let row = required_row(unit, manager, "AttributeDefinition");
    let level = required_field(&row, "Level");
    let row_field = go_direct_row_field_name("AttributeDefinition");
    let table_type = go_direct_table_type_name(manager, "AttributeDefinition", true);
    let manager_type = go_method_name(&manager.manager_class_name);
    let row_name = row.type_name.clone();
    let key_type = format!("{manager_type}Key");
    GoNativeManagerAugmentation {
        declarations: format!("\ntype {key_type} struct {{ Table {table_type}; Level uint32 }}\n"),
        fields: format!(
            "\tattributes map[{key_type}]int\n\tattributeLevels map[{table_type}][]uint32\n"
        ),
        field_values: format!(
            "\t\tattributes: make(map[{key_type}]int),\n\t\tattributeLevels: make(map[{table_type}][]uint32),\n"
        ),
        initializers: format!(
            r#"	for index := range manager.{row_field}.entries {{
		source := &manager.{row_field}.entries[index]
		level, ok := exactUint32(source.Row.{level_field})
		if !ok {{ continue }}
		table := source.Ref.Table()
		key := {key_type}{{Table: table, Level: level}}
		if _, exists := manager.attributes[key]; !exists {{ manager.attributes[key] = index }}
		manager.attributeLevels[table] = append(manager.attributeLevels[table], level)
	}}
	for table := range manager.attributeLevels {{ slices.Sort(manager.attributeLevels[table]) }}
"#,
            level_field = level.field_name,
        ),
        methods: format!(
            r#"func (manager *{manager_type}) AttributeData(table {table_type}, level uint32) *{row_name} {{
	index, ok := manager.attributes[{key_type}{{Table: table, Level: level}}]
	if !ok {{ return nil }}
	return rowCopy(manager.{row_field}.entries[index].Row)
}}

func (manager *{manager_type}) ClampedLevel(table {table_type}, level uint32) (uint32, bool) {{
	levels := manager.attributeLevels[table]
	if len(levels) == 0 {{ return 0, false }}
	index, found := slices.BinarySearch(levels, level)
	if found {{ return level, true }}
	if index == 0 {{ return levels[0], true }}
	return levels[index-1], true
}}

func (manager *{manager_type}) ClampedAttributeData(table {table_type}, level uint32) *{row_name} {{
	clamped, ok := manager.ClampedLevel(table, level)
	if !ok {{ return nil }}
	return manager.AttributeData(table, clamped)
}}

"#
        ),
    }
}

fn level_disparity_contract(
    unit: &GameDataCompileUnit,
    manager: &DirectManagerSurface,
) -> GoNativeManagerAugmentation {
    let mut augmentation = numeric_contract(
        unit,
        manager,
        "LevelDisparityData",
        "LevelDisparity",
        "int32",
        &["LevelDisparityData"],
        Some("LevelDisparityRows"),
    );
    let row = required_row(unit, manager, "LevelDisparityData");
    let disparity = required_field(&row, "LevelDisparity");
    let row_field = go_direct_row_field_name("LevelDisparityData");
    let manager_type = go_method_name(&manager.manager_class_name);
    let row_name = row.type_name.clone();
    augmentation.fields.push_str(
        "\tlevelDisparityMin int32\n\tlevelDisparityMax int32\n\thasLevelDisparityRange bool\n",
    );
    augmentation.initializers.push_str(&format!(
        r#"	for index := range manager.{row_field}.entries {{
		value := int32(manager.{row_field}.entries[index].Row.{disparity_field})
		if !manager.hasLevelDisparityRange {{ manager.levelDisparityMin = value; manager.levelDisparityMax = value; manager.hasLevelDisparityRange = true; continue }}
		if value < manager.levelDisparityMin {{ manager.levelDisparityMin = value }}
		if value > manager.levelDisparityMax {{ manager.levelDisparityMax = value }}
	}}
"#,
        disparity_field = disparity.field_name,
    ));
    augmentation.methods.push_str(&format!(
        r#"func (manager *{manager_type}) LevelDisparityDataForLevels(playerLevel int32, targetLevel int32) *{row_name} {{
	return manager.LevelDisparityData(targetLevel - playerLevel)
}}

func (manager *{manager_type}) ClampedDisparity(disparity int32) (int32, bool) {{
	if !manager.hasLevelDisparityRange {{ return 0, false }}
	if disparity < manager.levelDisparityMin {{ return manager.levelDisparityMin, true }}
	if disparity > manager.levelDisparityMax {{ return manager.levelDisparityMax, true }}
	return disparity, true
}}

func (manager *{manager_type}) ClampedLevelDisparityDataForLevels(playerLevel int32, targetLevel int32) *{row_name} {{
	disparity, ok := manager.ClampedDisparity(targetLevel - playerLevel)
	if !ok {{ return nil }}
	return manager.LevelDisparityData(disparity)
}}

func (manager *{manager_type}) LevelDisparityDataForLevelsWithPlayerLevelCap(playerLevel int32, targetLevel int32, maxPlayerLevel int32) *{row_name} {{
	if playerLevel > maxPlayerLevel {{ return nil }}
	return manager.LevelDisparityDataForLevels(playerLevel, targetLevel)
}}

func (manager *{manager_type}) ClampedLevelDisparityDataForLevelsWithPlayerLevelCap(playerLevel int32, targetLevel int32, maxPlayerLevel int32) *{row_name} {{
	if playerLevel > maxPlayerLevel {{ return nil }}
	return manager.ClampedLevelDisparityDataForLevels(playerLevel, targetLevel)
}}

func (manager *{manager_type}) LoadedRange() (int32, int32, bool) {{ return manager.levelDisparityMin, manager.levelDisparityMax, manager.hasLevelDisparityRange }}

"#
    ));
    augmentation
}

fn territory_contract(
    unit: &GameDataCompileUnit,
    manager: &DirectManagerSurface,
) -> GoNativeManagerAugmentation {
    let mut augmentation = numeric_contract(
        unit,
        manager,
        "TerritoryDefinition",
        "TerritoryID",
        "uint32",
        &["ByID"],
        Some("Territories"),
    );
    let row = required_row(unit, manager, "TerritoryDefinition");
    let territory_id = required_field(&row, "TerritoryID");
    let row_field = go_direct_row_field_name("TerritoryDefinition");
    let manager_type = go_method_name(&manager.manager_class_name);
    let row_name = row.type_name.clone();
    augmentation.fields.push_str("\tterritoriesByLabel map[gametypes.CRC32]int\n\tterritoriesByAchievement map[gametypes.CRC32]int\n\tterritoriesByTag map[gametypes.CRC32][]int\n");
    augmentation.field_values.push_str("\t\tterritoriesByLabel: make(map[gametypes.CRC32]int),\n\t\tterritoriesByAchievement: make(map[gametypes.CRC32]int),\n\t\tterritoriesByTag: make(map[gametypes.CRC32][]int),\n");
    let achievement_initializers = optional_field(&row, "Achievements")
        .map(|field| {
            let expression = string_expression(field, "manager.{row_field}.entries[index].Row")
                .replace("{row_field}", &row_field);
            format!("\t\tfor _, key := range splitDesignerList({expression}) {{ id := gametypes.CRC32(crc32Lowercase(key)); if id != 0 {{ if _, exists := manager.territoriesByAchievement[id]; !exists {{ manager.territoriesByAchievement[id] = index }} }} }}\n")
        })
        .unwrap_or_default();
    let tag_initializers = ["POITags", "LootTags"]
        .into_iter()
        .filter_map(|column| optional_field(&row, column))
        .map(|field| {
            let expression = string_expression(field, "manager.{row_field}.entries[index].Row")
                .replace("{row_field}", &row_field);
            format!("\t\tfor _, key := range splitDesignerList({expression}) {{ id := gametypes.CRC32(crc32Lowercase(key)); if id != 0 {{ manager.territoriesByTag[id] = append(manager.territoriesByTag[id], index) }} }}\n")
        })
        .collect::<String>();
    augmentation.initializers.push_str(&format!(
        r#"	for index := range manager.{row_field}.entries {{
		id, ok := exactUint32(manager.{row_field}.entries[index].Row.{territory_id_field})
		if !ok {{ continue }}
		label := fmt.Sprintf("Territory_%d", id)
		manager.territoriesByLabel[gametypes.CRC32(crc32Lowercase(label))] = index
{achievement_initializers}{tag_initializers}	}}
"#,
        territory_id_field = territory_id.field_name,
    ));
    augmentation.methods.push_str(&format!(
        r#"func (manager *{manager_type}) ByLabel(label string) *{row_name} {{
	index, ok := manager.territoriesByLabel[gametypes.CRC32(crc32Lowercase(label))]
	if !ok {{ return nil }}
	return rowCopy(manager.{row_field}.entries[index].Row)
}}

func (manager *{manager_type}) TerritoryForAchievement(achievementID gametypes.CRC32) *{row_name} {{
	index, ok := manager.territoriesByAchievement[achievementID]
	if !ok {{ return nil }}
	return rowCopy(manager.{row_field}.entries[index].Row)
}}

func (manager *{manager_type}) TerritoriesWithTag(tagID gametypes.CRC32) iter.Seq[{row_name}] {{
	return func(yield func({row_name}) bool) {{ for _, index := range manager.territoriesByTag[tagID] {{ if !yield(manager.{row_field}.entries[index].Row) {{ return }} }} }}
}}

"#
    ));
    augmentation
}

fn governance_contract(
    unit: &GameDataCompileUnit,
    manager: &DirectManagerSurface,
) -> GoNativeManagerAugmentation {
    let mut augmentation = numeric_contract(
        unit,
        manager,
        "TerritoryUpkeepDefinition",
        "Level",
        "uint32",
        &["Governance"],
        Some("GovernanceRows"),
    );
    let row = required_row(unit, manager, "TerritoryUpkeepDefinition");
    let row_field = go_direct_row_field_name("TerritoryUpkeepDefinition");
    let manager_type = go_method_name(&manager.manager_class_name);
    let row_name = row.type_name.clone();
    let distributions = row
        .fields
        .iter()
        .filter_map(|field| {
            let suffix = field.source_name.strip_prefix("EarningsDistributionTID")?;
            let territory_id = suffix.parse::<u32>().ok()?;
            Some((territory_id, number_expression(field, "source.Row")))
        })
        .collect::<Vec<_>>();
    let distribution_initializers = distributions
        .iter()
        .map(|(territory_id, expression)| {
            format!("\t	manager.governanceDistribution[index] = append(manager.governanceDistribution[index], TerritoryEarningsDistribution{{TerritoryID: {territory_id}, Share: {expression}}})\n\t	if {territory_id} > manager.maxTerritoryID {{ manager.maxTerritoryID = {territory_id} }}\n")
        })
        .collect::<String>();
    augmentation.declarations.push_str(
        "\ntype TerritoryEarningsDistribution struct { TerritoryID uint32; Share float32 }\n",
    );
    augmentation.fields.push_str(
        "\tgovernanceDistribution map[int][]TerritoryEarningsDistribution\n\tmaxTerritoryID uint32\n",
    );
    augmentation
        .field_values
        .push_str("\t\tgovernanceDistribution: make(map[int][]TerritoryEarningsDistribution),\n");
    augmentation.initializers.push_str(&format!(
        r#"	for index := range manager.{row_field}.entries {{
		source := &manager.{row_field}.entries[index]
{distribution_initializers}	}}
"#
    ));
    augmentation.methods.push_str(&format!(
        r#"func (manager *{manager_type}) TerritoryEarningsDistribution(level uint32) iter.Seq[TerritoryEarningsDistribution] {{
	index, ok := manager.territoryUpkeepDefinitionByNumber[level]
	if !ok {{ return slices.Values([]TerritoryEarningsDistribution(nil)) }}
	return slices.Values(manager.governanceDistribution[index])
}}

func (manager *{manager_type}) MaxTerritoryID() uint32 {{ return manager.maxTerritoryID }}
func (manager *{manager_type}) GovernanceForSourceRow(source RowSlot[GovernanceDataTable, {row_name}]) *{row_name} {{ return manager.RowByIndex(source) }}

"#
    ));
    augmentation
}

fn loot_bucket_contract(
    unit: &GameDataCompileUnit,
    manager: &DirectManagerSurface,
) -> GoNativeManagerAugmentation {
    let row = required_row(unit, manager, "LootBucketData");
    let row_field = go_direct_row_field_name("LootBucketData");
    let default_row = go_direct_default_row_spec(unit, manager)
        .map(|value| value.source_row_type)
        .as_deref()
        == Some("LootBucketData");
    let table_type = go_direct_table_type_name(manager, "LootBucketData", default_row);
    let manager_type = go_method_name(&manager.manager_class_name);
    let _schema_row = row.type_name;
    GoNativeManagerAugmentation {
        declarations: format!(
            r#"
type LootBucketSlot struct {{ Table {table_type}; Slot uint16 }}

type LootBucketData struct {{
	Table LootBucketDataTable
	Slot uint16
	Key string
	ID gametypes.CRC32
	FiltersLootedItems bool
	LootBiasingDisabled bool
	Entries []LootBucketEntry
}}

type LootBucketEntry struct {{
	RowIndex int
	ItemKey string
	ItemID gametypes.CRC32
	Tags []LootBucketTag
	MatchOne bool
	Quantity Uint16Range
	Odds float32
}}

type LootBucketTag struct {{ Key string; ID gametypes.CRC32; Range *Uint16Range }}
type Uint16Range struct {{ Min uint16; Max uint16 }}

func lootBucketBool(value *string) bool {{
	if value == nil {{ return false }}
	switch strings.ToLower(strings.TrimSpace(*value)) {{ case "true", "1", "yes": return true; default: return false }}
}}

func lootBucketUint16(value string) uint16 {{
	value = strings.TrimSpace(value)
	if parsed, err := strconv.ParseInt(value, 10, 64); err == nil {{ if parsed >= 0 && parsed <= 65535 {{ return uint16(parsed) }}; return 0 }}
	parsed, err := strconv.ParseFloat(value, 32)
	if err != nil || math.IsNaN(parsed) || math.IsInf(parsed, 0) || parsed < 0 || parsed > 65535 {{ return 0 }}
	return uint16(parsed)
}}

func lootBucketRange(value *string, singleMax uint16) Uint16Range {{
	if value == nil {{ return Uint16Range{{}} }}
	parts := strings.SplitN(strings.TrimSpace(*value), "-", 2)
	start := lootBucketUint16(parts[0]); end := start
	if len(parts) == 2 {{ end = lootBucketUint16(parts[1]) }} else if singleMax != 0 {{ end = singleMax }}
	if start > end {{ start, end = end, start }}
	return Uint16Range{{Min: start, Max: end}}
}}

func lootBucketOdds(value *string) float32 {{
	if value == nil {{ return 1 }}
	parsed, err := strconv.ParseFloat(strings.TrimSpace(*value), 32)
	if err != nil || math.IsNaN(parsed) || math.IsInf(parsed, 0) {{ return 1 }}
	return float32(parsed)
}}

func lootBucketTags(value *string) []LootBucketTag {{
	if value == nil {{ return nil }}
	var tags []LootBucketTag
	for _, token := range strings.Split(*value, ",") {{
		token = strings.TrimSpace(token); if token == "" {{ continue }}
		key, rangeText, hasRange := strings.Cut(token, ":"); key = strings.TrimSpace(key)
		id := gametypes.CRC32(crc32Lowercase(key)); if key == "" || id == 0 {{ continue }}
		tag := LootBucketTag{{Key: key, ID: id}}
		if hasRange {{ value := strings.TrimSpace(rangeText); tag.Range = &Uint16Range{{}}; if value != "" {{ tag.Range = new(Uint16Range); *tag.Range = lootBucketRange(&value, 10000) }} }}
		tags = append(tags, tag)
	}}
	return tags
}}
"#,
            table_type = table_type,
        )
        .replace("LootBucketDataTable", &table_type),
        fields: "\tlootBuckets []LootBucketData\n\tlootBucketsByID map[gametypes.CRC32]int\n\tlootBucketsBySlot map[LootBucketSlot]int\n".to_owned(),
        field_values: "\t\tlootBucketsByID: make(map[gametypes.CRC32]int),\n\t\tlootBucketsBySlot: make(map[LootBucketSlot]int),\n".to_owned(),
        initializers: format!(
            r#"	for sourceIndex := range manager.{row_field}.entries {{
		source := &manager.{row_field}.entries[sourceIndex]
		for _, entry := range source.Row.Entries {{
			slot := LootBucketSlot{{Table: source.Ref.Table(), Slot: entry.Slot}}
			bucketIndex, exists := manager.lootBucketsBySlot[slot]
			if entry.LootBucket != nil {{
				key := strings.TrimSpace(*entry.LootBucket)
				id := gametypes.CRC32(crc32Lowercase(key))
				if key != "" && id != 0 {{
					data := LootBucketData{{Table: source.Ref.Table(), Slot: entry.Slot, Key: key, ID: id, FiltersLootedItems: lootBucketBool(entry.FilterLootedItems), LootBiasingDisabled: lootBucketBool(entry.LootBiasingDisabled)}}
					if previous, duplicate := manager.lootBucketsByID[id]; duplicate {{
						bucketIndex = previous
						manager.lootBuckets[bucketIndex] = data
					}} else {{
						bucketIndex = len(manager.lootBuckets)
						manager.lootBuckets = append(manager.lootBuckets, data)
						manager.lootBucketsByID[id] = bucketIndex
					}}
					manager.lootBucketsBySlot[slot] = bucketIndex
					exists = true
				}}
			}}
			if !exists || entry.Item == nil {{ continue }}
			for _, itemKey := range splitDesignerList(*entry.Item) {{
				itemKey = strings.TrimSpace(itemKey)
				itemID := gametypes.CRC32(crc32Lowercase(itemKey))
				if itemKey == "" || itemID == 0 {{ continue }}
				value := LootBucketEntry{{RowIndex: source.Slot.RowIndex(), ItemKey: itemKey, ItemID: itemID, Tags: lootBucketTags(entry.Tags), MatchOne: lootBucketBool(entry.MatchOne), Quantity: lootBucketRange(entry.Quantity, 0), Odds: lootBucketOdds(entry.Odds)}}
				manager.lootBuckets[bucketIndex].Entries = append(manager.lootBuckets[bucketIndex].Entries, value)
			}}
		}}
	}}
"#
        ),
        methods: format!(
            r#"func (manager *{manager_type}) ByID(id gametypes.CRC32) *LootBucketData {{
	index, ok := manager.lootBucketsByID[id]
	if !ok {{ return nil }}
	return rowCopy(manager.lootBuckets[index])
}}

func (manager *{manager_type}) ByKey(key string) *LootBucketData {{
	return manager.ByID(gametypes.CRC32(crc32Lowercase(key)))
}}

func (manager *{manager_type}) BucketAtSlot(slot LootBucketSlot) *LootBucketData {{
	index, ok := manager.lootBucketsBySlot[slot]
	if !ok {{ return nil }}
	return rowCopy(manager.lootBuckets[index])
}}

func (manager *{manager_type}) Buckets() iter.Seq[LootBucketData] {{ return rowValues(manager.lootBuckets) }}
func (manager *{manager_type}) Rows() iter.Seq[LootBucketData] {{ return manager.Buckets() }}

"#
        ),
    }
}

fn reward_track_contract(
    unit: &GameDataCompileUnit,
    manager: &DirectManagerSurface,
) -> GoNativeManagerAugmentation {
    let row = required_row(unit, manager, "PvPStoreData");
    let reward_item_row = required_row(unit, manager, "RewardTrackItemData");
    let row_field = go_direct_row_field_name("PvPStoreData");
    let reward_item_row_field = go_direct_row_field_name("RewardTrackItemData");
    let reward_item_row_type = &reward_item_row.type_name;
    let default_row = go_direct_default_row_spec(unit, manager)
        .map(|value| value.source_row_type)
        .as_deref()
        == Some("PvPStoreData");
    let table_type = go_direct_table_type_name(manager, "PvPStoreData", default_row);
    let reward_item_default = go_direct_default_row_spec(unit, manager)
        .map(|value| value.source_row_type)
        .as_deref()
        == Some("RewardTrackItemData");
    let reward_item_table_type =
        go_direct_table_type_name(manager, "RewardTrackItemData", reward_item_default);
    let reward_item_table = manager
        .tables
        .iter()
        .find(|table| table.row_type_name == "RewardTrackItemData")
        .expect("RewardTrackData requires RewardTrackItemData table");
    let reward_item_table_value = format!(
        "{reward_item_table_type}{}",
        go_method_name(&reward_item_table.table_name)
    );
    let reward_item_key =
        text_expression(required_field(&reward_item_row, "RewardID"), "rewardItem");
    let manager_type = go_method_name(&manager.manager_class_name);
    let mut slots = BTreeMap::<u16, BTreeMap<&'static str, &GoSchemaField>>::new();
    for field in &row.fields {
        for stem in [
            "Bucket",
            "Tag",
            "MatchOne",
            "RewardID",
            "RandomWeights",
            "BudgetContribution",
            "Type",
            "SelectOnceOnly",
            "ExcludeTypeStage",
            "ExcludeTypeShop",
        ] {
            if let Some(slot) = numbered_suffix(&field.source_name, stem) {
                slots.entry(slot).or_default().insert(stem, field);
            }
        }
    }
    let slot_initializers = slots
        .into_iter()
        .filter_map(|(slot, fields)| {
            let bucket = text_expression(fields.get("Bucket").copied()?, "source.Row");
            let reward = text_expression(fields.get("RewardID").copied()?, "source.Row");
            let text = |stem: &str| {
                fields
                    .get(stem)
                    .map(|field| text_expression(field, "source.Row"))
                    .unwrap_or_else(|| "\"\"".to_owned())
            };
            Some((
                slot,
                bucket,
                reward,
                text("Tag"),
                text("MatchOne"),
                text("RandomWeights"),
                text("BudgetContribution"),
                text("Type"),
                text("SelectOnceOnly"),
                text("ExcludeTypeStage"),
                text("ExcludeTypeShop"),
            ))
        })
        .map(|(slot, bucket, reward, tags, match_one, random_weight, budget, reward_type, select_once, stage_exclusion, shop_exclusion)| {
            format!(
                r#"		{{
			slot := RewardTrackSlot{{Table: source.Ref.Table(), Slot: {slot}}}
			trackIndex, exists := manager.rewardTracksBySlot[slot]
			key := strings.TrimSpace({bucket})
			if key != "" {{
				id := gametypes.CRC32(crc32Lowercase(key))
				if key != "" && id != 0 {{
					if previous, duplicate := manager.rewardTracksByID[id]; duplicate {{ trackIndex = previous; manager.rewardTracks[trackIndex] = RewardTrackData{{Table: source.Ref.Table(), Slot: {slot}, Key: key, ID: id}} }} else {{ trackIndex = len(manager.rewardTracks); manager.rewardTracks = append(manager.rewardTracks, RewardTrackData{{Table: source.Ref.Table(), Slot: {slot}, Key: key, ID: id}}); manager.rewardTracksByID[id] = trackIndex }}
					manager.rewardTracksBySlot[slot] = trackIndex
					exists = true
				}}
			}}
			if exists {{
				rewardRow, rewardRowOK := rewardTrackUint32({reward})
				if rewardRow == 0 || !rewardRowOK {{ continue }}
				rewardItem := manager.{reward_item_row_field}.RowByIndex(RowSlot[{reward_item_table_type}, {reward_item_row_type}]{{table: {reward_item_table_value}, rowIndex: int(rewardRow - 1)}})
				if rewardItem == nil {{ return nil, fmt.Errorf("PvPStore row %d slot {slot} references missing RewardTrackItems row %d", source.Slot.RowIndex()+1, rewardRow) }}
				rewardKey := strings.TrimSpace({reward_item_key}); rewardID := gametypes.CRC32(crc32Lowercase(rewardKey)); if rewardKey == "" || rewardID == 0 {{ continue }}
				matchOne, matchOneOK := rewardTrackBool({match_one}); if !matchOneOK {{ return nil, fmt.Errorf("PvPStore row %d slot {slot} has invalid MatchOne", source.Slot.RowIndex()+1) }}
				selectOnce, selectOnceOK := rewardTrackBoolDefault({select_once}, true); if !selectOnceOK {{ return nil, fmt.Errorf("PvPStore row %d slot {slot} has invalid SelectOnceOnly", source.Slot.RowIndex()+1) }}
				randomWeight, randomWeightOK := rewardTrackUint32({random_weight}); if !randomWeightOK {{ continue }}
				budgetContribution, budgetOK := rewardTrackUint32({budget}); if !budgetOK {{ continue }}
				tagConstraints, tagsOK := rewardTrackTags({tags}); if !tagsOK {{ return nil, fmt.Errorf("PvPStore row %d slot {slot} has invalid Tag constraints", source.Slot.RowIndex()+1) }}
				manager.rewardTracks[trackIndex].Entries = append(manager.rewardTracks[trackIndex].Entries, RewardTrackEntry{{SourceSlot:{slot}, RowIndex:source.Slot.RowIndex(), RewardKey:rewardKey, RewardID:rewardID, RewardType:rewardTrackCRC({reward_type}), TagConstraints:tagConstraints, MatchOne:matchOne, SelectOnce:selectOnce, RandomWeight:randomWeight, BudgetContribution:budgetContribution, StageExclusion:rewardTrackCRC({stage_exclusion}), ShopExclusion:rewardTrackCRC({shop_exclusion})}})
			}}
		}}
"#
            )
        })
        .collect::<String>();
    GoNativeManagerAugmentation {
        declarations: format!(
            r#"
type RewardTrackSlot struct {{ Table {table_type}; Slot uint16 }}
type RewardTrackData struct {{ Table {table_type}; Slot uint16; Key string; ID gametypes.CRC32; Entries []RewardTrackEntry }}
type RewardTrackEntry struct {{ SourceSlot uint16; RowIndex int; RewardKey string; RewardID gametypes.CRC32; RewardType *gametypes.CRC32; TagConstraints []RewardTrackTagConstraint; MatchOne bool; SelectOnce bool; RandomWeight uint32; BudgetContribution uint32; StageExclusion *gametypes.CRC32; ShopExclusion *gametypes.CRC32 }}
type RewardTrackTagConstraint struct {{ Tag gametypes.CRC32; Range RewardTrackUint16Range }}
type RewardTrackUint16Range struct {{ Min uint16; Max uint16 }}

func rewardTrackUint32(value string) (uint32, bool) {{ parsed, err := strconv.ParseUint(strings.TrimSpace(value), 10, 32); return uint32(parsed), err == nil }}
func rewardTrackBool(value string) (bool, bool) {{ switch strings.ToLower(strings.TrimSpace(value)) {{ case "true", "1": return true, true; case "false", "0", "": return false, true; default: return false, false }} }}
func rewardTrackBoolDefault(value string, fallback bool) (bool, bool) {{ if strings.TrimSpace(value) == "" {{ return fallback, true }}; return rewardTrackBool(value) }}
func rewardTrackCRC(value string) *gametypes.CRC32 {{ value = strings.TrimSpace(value); id := gametypes.CRC32(crc32Lowercase(value)); if value == "" || id == 0 {{ return nil }}; return &id }}
func rewardTrackRange(value string) (RewardTrackUint16Range, bool) {{ parts := strings.Split(value, "-"); if len(parts) > 2 {{ return RewardTrackUint16Range{{}}, false }}; left, err := strconv.ParseUint(strings.TrimSpace(parts[0]), 10, 16); if err != nil {{ return RewardTrackUint16Range{{}}, false }}; right := left; if len(parts) == 2 {{ right, err = strconv.ParseUint(strings.TrimSpace(parts[1]), 10, 16); if err != nil {{ return RewardTrackUint16Range{{}}, false }} }}; if left > right {{ left, right = right, left }}; return RewardTrackUint16Range{{Min:uint16(left), Max:uint16(right)}}, true }}
func rewardTrackTags(value string) ([]RewardTrackTagConstraint, bool) {{ var out []RewardTrackTagConstraint; for _, token := range strings.Split(value, ",") {{ token = strings.TrimSpace(token); if token == "" {{ continue }}; key, rangeText, hasRange := strings.Cut(token, ":"); if strings.Contains(rangeText, ":") {{ return nil, false }}; key = strings.TrimSpace(key); id := gametypes.CRC32(crc32Lowercase(key)); if key == "" || id == 0 {{ return nil, false }}; constraint := RewardTrackTagConstraint{{Tag:id}}; if hasRange {{ parsed, ok := rewardTrackRange(strings.TrimSpace(rangeText)); if !ok {{ return nil, false }}; constraint.Range = parsed }}; out = append(out, constraint) }}; return out, true }}
"#
        ),
        fields: "\trewardTracks []RewardTrackData\n\trewardTracksByID map[gametypes.CRC32]int\n\trewardTracksBySlot map[RewardTrackSlot]int\n".to_owned(),
        field_values: "\t\trewardTracksByID: make(map[gametypes.CRC32]int),\n\t\trewardTracksBySlot: make(map[RewardTrackSlot]int),\n".to_owned(),
        initializers: format!(
            "\tfor sourceIndex := range manager.{row_field}.entries {{\n\t\tsource := &manager.{row_field}.entries[sourceIndex]\n{slot_initializers}\t}}\n"
        ),
        methods: format!(
            r#"func (manager *{manager_type}) RewardTrackDataFromID(id gametypes.CRC32) *RewardTrackData {{ index, ok := manager.rewardTracksByID[id]; if !ok {{ return nil }}; return rowCopy(manager.rewardTracks[index]) }}
func (manager *{manager_type}) RewardTrackData(key string) *RewardTrackData {{ return manager.RewardTrackDataFromID(gametypes.CRC32(crc32Lowercase(key))) }}
func (manager *{manager_type}) RewardTrackDataByKey(key string) *RewardTrackData {{ return manager.RewardTrackData(key) }}
func (manager *{manager_type}) RewardTrackAtSlot(slot RewardTrackSlot) *RewardTrackData {{ index, ok := manager.rewardTracksBySlot[slot]; if !ok {{ return nil }}; return rowCopy(manager.rewardTracks[index]) }}
func (manager *{manager_type}) RewardTracks() iter.Seq[RewardTrackData] {{ return rowValues(manager.rewardTracks) }}
func (manager *{manager_type}) Rows() iter.Seq[RewardTrackData] {{ return manager.RewardTracks() }}

"#
        ),
    }
}

fn numbered_suffix(value: &str, prefix: &str) -> Option<u16> {
    if value.len() <= prefix.len() || !value[..prefix.len()].eq_ignore_ascii_case(prefix) {
        return None;
    }
    let suffix = &value[prefix.len()..];
    (!suffix.is_empty()).then(|| suffix.parse().ok()).flatten()
}

fn stat_modifier_contract(
    unit: &GameDataCompileUnit,
    manager: &DirectManagerSurface,
) -> GoNativeManagerAugmentation {
    let contracts = go_direct_row_specs(unit, manager)
        .into_iter()
        .filter_map(|row| {
            let key = [
                "StatusID",
                "ItemID",
                "WeaponID",
                "ConsumableID",
                "VitalsID",
                "Id",
            ]
            .into_iter()
            .find(|candidate| {
                row.fields
                    .iter()
                    .any(|field| field.source_name.eq_ignore_ascii_case(candidate))
            })?;
            Some(crc_secondary_contract(
                unit,
                manager,
                &row.source_row_type,
                key,
                &format!("{}FromID", go_method_name(&row.source_row_type)),
                &format!("{}ByKey", go_method_name(&row.source_row_type)),
                &go_local_name(&format!("{} by id", row.source_row_type)),
            ))
        })
        .collect::<Vec<_>>();
    merge_augmentations(contracts)
}

pub(super) fn crc_secondary_contract(
    unit: &GameDataCompileUnit,
    manager: &DirectManagerSurface,
    row_type: &str,
    key_column: &str,
    id_method: &str,
    key_method: &str,
    map_field: &str,
) -> GoNativeManagerAugmentation {
    let row = required_row(unit, manager, row_type);
    let key = required_field(&row, key_column);
    let row_field = go_direct_row_field_name(row_type);
    let manager_type = go_method_name(&manager.manager_class_name);
    let row_name = row.type_name.clone();
    let id_method = go_method_name(id_method);
    let key_method = go_method_name(key_method);
    GoNativeManagerAugmentation {
        declarations: String::new(),
        fields: format!("\t{map_field} map[gametypes.CRC32]int\n"),
        field_values: format!("\t\t{map_field}: make(map[gametypes.CRC32]int),\n"),
        initializers: format!(
            r#"	for index := range manager.{row_field}.entries {{
		value := strings.TrimSpace(manager.{row_field}.entries[index].Row.{key_field})
		key := gametypes.CRC32(crc32Lowercase(value))
		if value == "" || key == 0 {{ continue }}
		if _, exists := manager.{map_field}[key]; !exists {{ manager.{map_field}[key] = index }}
	}}
"#,
            key_field = key.field_name,
        ),
        methods: format!(
            r#"func (manager *{manager_type}) {id_method}(id gametypes.CRC32) *{row_name} {{
	index, ok := manager.{map_field}[id]
	if !ok {{ return nil }}
	return rowCopy(manager.{row_field}.entries[index].Row)
}}

func (manager *{manager_type}) {key_method}(key string) *{row_name} {{
	return manager.{id_method}(gametypes.CRC32(crc32Lowercase(key)))
}}

"#
        ),
    }
}

pub(super) fn required_row(
    unit: &GameDataCompileUnit,
    manager: &DirectManagerSurface,
    row_type: &str,
) -> GoSchemaRow {
    go_direct_row_specs(unit, manager)
        .into_iter()
        .find(|row| row.source_row_type.eq_ignore_ascii_case(row_type))
        .unwrap_or_else(|| panic!("{} requires schema row {row_type}", manager.manager_name))
}

pub(super) fn required_field<'a>(row: &'a GoSchemaRow, column: &str) -> &'a GoSchemaField {
    row.fields
        .iter()
        .find(|field| field.source_name.eq_ignore_ascii_case(column))
        .unwrap_or_else(|| panic!("{} requires column {column}", row.source_row_type))
}

pub(super) fn optional_field<'a>(row: &'a GoSchemaRow, column: &str) -> Option<&'a GoSchemaField> {
    row.fields
        .iter()
        .find(|field| field.source_name.eq_ignore_ascii_case(column))
}

pub(super) fn string_expression(field: &GoSchemaField, receiver: &str) -> String {
    if field.required {
        format!("{receiver}.{}", field.field_name)
    } else {
        format!("stringValue({receiver}.{})", field.field_name)
    }
}

pub(super) fn number_expression(field: &GoSchemaField, receiver: &str) -> String {
    match (field.column_type, field.required) {
        (ColumnType::Number, true) => format!("{receiver}.{}", field.field_name),
        (ColumnType::Number, false) => {
            format!("float32Value({receiver}.{})", field.field_name)
        }
        (ColumnType::String, true) => {
            format!("parseFloat32OrZero({receiver}.{})", field.field_name)
        }
        (ColumnType::String, false) => format!(
            "parseFloat32OrZero(stringValue({receiver}.{}))",
            field.field_name
        ),
        _ => panic!("column `{}` is not numeric-like", field.source_name),
    }
}

fn text_expression(field: &GoSchemaField, receiver: &str) -> String {
    match field.column_type {
        ColumnType::String => string_expression(field, receiver),
        ColumnType::Number => format!(
            "strconv.FormatFloat(float64({}), 'f', -1, 32)",
            number_expression(field, receiver)
        ),
        ColumnType::Boolean => format!("strconv.FormatBool({})", bool_expression(field, receiver)),
    }
}
