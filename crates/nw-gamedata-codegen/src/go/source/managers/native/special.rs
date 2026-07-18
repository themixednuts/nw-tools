use super::*;

pub(super) fn augmentation(
    unit: &GameDataCompileUnit,
    manager: &DirectManagerSurface,
    shape: &NativeManagerShape,
) -> GoNativeManagerAugmentation {
    match shape {
        NativeManagerShape::ElementalMutationStaticData(_) => elemental_mutations(unit, manager),
        NativeManagerShape::PromotionMutationStaticData(_) => promotion_mutations(unit, manager),
        NativeManagerShape::MusicalRewardsData(_) => musical_rewards(unit, manager),
        NativeManagerShape::CombatProfilesData(_) => combat_profiles(unit, manager),
        NativeManagerShape::GatherableData(_) => gatherable(unit, manager),
        NativeManagerShape::SocialData(_) => social(),
        NativeManagerShape::PlayerData(_) => player(),
        NativeManagerShape::RecipeData(_) => recipe(unit, manager),
        _ => panic!(
            "manager {} reached special Go native dispatch with {shape:?}",
            manager.manager_name
        ),
    }
}

fn elemental_mutations(
    unit: &GameDataCompileUnit,
    manager: &DirectManagerSurface,
) -> GoNativeManagerAugmentation {
    let row = indexed::required_row(unit, manager, "ElementalMutationStaticData");
    let id = indexed::required_field(&row, "ElementalMutationID");
    let row_field = go_direct_row_field_name("ElementalMutationStaticData");
    let table_type = go_direct_table_type_name(manager, "ElementalMutationStaticData", true);
    let row_name = row.type_name.clone();
    let manager_type = go_method_name(&manager.manager_class_name);
    let id_expression = string_expression(id, "source.Row");
    let priority_expression = "statusEffect.UIPriority";
    let mut bucket_fields = row
        .fields
        .iter()
        .filter(|field| field.source_name.eq_ignore_ascii_case("Dungeon"))
        .collect::<Vec<_>>();
    for column in ["Dungeon2", "Dungeon3"] {
        if let Some(field) = optional_field(&row, column) {
            if !bucket_fields
                .iter()
                .any(|candidate| candidate.field_name == field.field_name)
            {
                bucket_fields.push(field);
            }
        }
    }
    let mut bucket_specs = ["Dungeon-", "Dungeon", "Dungeon+"]
        .into_iter()
        .zip(bucket_fields)
        .collect::<Vec<_>>();
    for (creature, column) in [
        ("DungeonMiniBoss", "DungeonMiniBoss"),
        ("DungeonBoss", "DungeonBoss"),
    ] {
        if let Some(field) = optional_field(&row, column) {
            bucket_specs.push((creature, field));
        }
    }
    let bucket_initializers = bucket_specs
        .into_iter()
        .enumerate()
        .map(|(index, (creature, field))| {
            let expression = string_expression(field, "source.Row");
            format!(
                r#"		bucket{index}Key := strings.TrimSpace({expression})
		bucket{index}ID := gametypes.CRC32(crc32Lowercase(bucket{index}Key))
		if bucket{index}ID != 0 {{
			data.Buckets = append(data.Buckets, ElementalMutationBucket{{CreatureTypeID: gametypes.CRC32(crc32Lowercase({creature:?})), BucketKey: bucket{index}Key, BucketID: bucket{index}ID}})
			for entry := range _buffBucketData.VisitAllBuffsFromID(bucket{index}ID) {{
				if entry.Kind != BuffBucketEntryStatusEffect {{ continue }}
				statusEffect := _statusEffectData.StatusEffectDataFromID(entry.BuffID)
				if statusEffect == nil {{ continue }}
				insertElementalStatusSelection(selected, entry.BuffKey, entry.BuffID, int32({priority_expression}))
			}}
		}}
"#
            )
        })
        .collect::<String>();
    GoNativeManagerAugmentation {
        declarations: format!(
            r#"
type ElementalMutationBucket struct {{ CreatureTypeID gametypes.CRC32; BucketKey string; BucketID gametypes.CRC32 }}
type ElementalMutationStaticData struct {{
	Source RowRef[{table_type}, {row_name}]
	Key string
	ID gametypes.CRC32
	Buckets []ElementalMutationBucket
	PossibleStatusEffectIDs []gametypes.CRC32
}}
type elementalStatusSelection struct {{ ID gametypes.CRC32; Tier uint32; UIPriority int32 }}

func insertElementalStatusSelection(selected map[string]elementalStatusSelection, key string, id gametypes.CRC32, priority int32) {{
	group := strings.Map(func(character rune) rune {{ if character >= '0' && character <= '9' {{ return -1 }}; return character }}, key)
	digits := strings.Map(func(character rune) rune {{ if character >= '0' && character <= '9' {{ return character }}; return -1 }}, key)
	tier64, _ := strconv.ParseUint(digits, 10, 32)
	candidate := elementalStatusSelection{{ID: id, Tier: uint32(tier64), UIPriority: priority}}
	if current, exists := selected[group]; exists && current.Tier >= candidate.Tier {{ return }}
	selected[group] = candidate
}}
"#
        ),
        fields: "\telementalMutations []ElementalMutationStaticData\n\telementalMutationsByID map[gametypes.CRC32]int\n".to_owned(),
        field_values: "\t\telementalMutationsByID: make(map[gametypes.CRC32]int),\n".to_owned(),
        initializers: format!(
            r#"	for index := range manager.{row_field}.entries {{
		source := &manager.{row_field}.entries[index]
		key := strings.TrimSpace({id_expression})
		id := gametypes.CRC32(crc32Lowercase(key))
		if key == "" || id == 0 {{ continue }}
		if _, exists := manager.elementalMutationsByID[id]; exists {{ continue }}
		data := ElementalMutationStaticData{{Source: source.Ref, Key: key, ID: id}}
		selected := make(map[string]elementalStatusSelection)
{bucket_initializers}		ordered := make([]elementalStatusSelection, 0, len(selected))
		for _, candidate := range selected {{ ordered = append(ordered, candidate) }}
		sort.Slice(ordered, func(left int, right int) bool {{ if ordered[left].UIPriority != ordered[right].UIPriority {{ return ordered[left].UIPriority > ordered[right].UIPriority }}; return ordered[left].ID < ordered[right].ID }})
		for _, candidate := range ordered {{ data.PossibleStatusEffectIDs = append(data.PossibleStatusEffectIDs, candidate.ID) }}
		manager.elementalMutationsByID[id] = len(manager.elementalMutations)
		manager.elementalMutations = append(manager.elementalMutations, data)
	}}
"#
        ),
        methods: format!(
            r#"func (manager *{manager_type}) ElementalMutationStaticDataFromID(id gametypes.CRC32) *ElementalMutationStaticData {{
	index, ok := manager.elementalMutationsByID[id]
	if !ok {{ return nil }}
	return rowCopy(manager.elementalMutations[index])
}}

func (manager *{manager_type}) ElementalMutationStaticData(key string) *ElementalMutationStaticData {{ return manager.ElementalMutationStaticDataFromID(gametypes.CRC32(crc32Lowercase(key))) }}

func (manager *{manager_type}) PossibleElementalStatusEffects(id gametypes.CRC32) iter.Seq[gametypes.CRC32] {{
	data := manager.ElementalMutationStaticDataFromID(id)
	if data == nil {{ return slices.Values([]gametypes.CRC32(nil)) }}
	return slices.Values(data.PossibleStatusEffectIDs)
}}

func (manager *{manager_type}) PossibleElementalStatusEffectsByKey(key string) iter.Seq[gametypes.CRC32] {{ return manager.PossibleElementalStatusEffects(gametypes.CRC32(crc32Lowercase(key))) }}
func (manager *{manager_type}) ElementalMutations() iter.Seq[ElementalMutationStaticData] {{ return rowValues(manager.elementalMutations) }}
func (manager *{manager_type}) Rows() iter.Seq[ElementalMutationStaticData] {{ return manager.ElementalMutations() }}

"#
        ),
    }
}

fn promotion_mutations(
    unit: &GameDataCompileUnit,
    manager: &DirectManagerSurface,
) -> GoNativeManagerAugmentation {
    let row = indexed::required_row(unit, manager, "PromotionMutationStaticData");
    let id = indexed::required_field(&row, "PromotionMutationID");
    let row_field = go_direct_row_field_name("PromotionMutationStaticData");
    let table_type = go_direct_table_type_name(manager, "PromotionMutationStaticData", true);
    let row_name = row.type_name.clone();
    let manager_type = go_method_name(&manager.manager_class_name);
    let id_expression = string_expression(id, "source.Row");
    let priority_expression = "statusEffect.UIPriority";
    let promotion_initializers = (1..=3u8)
        .filter_map(|slot| {
            let field = optional_field(&row, &format!("Promotion{slot}"))?;
            let expression = string_expression(field, "source.Row");
            Some(format!(
                r#"		promotion{slot}Key := strings.TrimSpace({expression})
		promotion{slot}ID := gametypes.CRC32(crc32Lowercase(promotion{slot}Key))
		if promotion{slot}ID != 0 {{ data.Promotions = append(data.Promotions, PromotionStatusEffect{{Slot: {slot}, SlotTagID: gametypes.CRC32(crc32Lowercase("Promotion{slot}")), StatusEffectKey: promotion{slot}Key, StatusEffectID: promotion{slot}ID}}) }}
"#
            ))
        })
        .collect::<String>();
    GoNativeManagerAugmentation {
        declarations: format!(
            r#"
type PromotionStatusEffect struct {{ Slot uint8; SlotTagID gametypes.CRC32; StatusEffectKey string; StatusEffectID gametypes.CRC32 }}
type PromotionMutationStaticData struct {{
	Source RowRef[{table_type}, {row_name}]
	Key string
	ID gametypes.CRC32
	Promotions []PromotionStatusEffect
	PossibleStatusEffectIDsByElement map[gametypes.CRC32][]gametypes.CRC32
}}
"#
        ),
        fields: "\tpromotionMutations []PromotionMutationStaticData\n\tpromotionMutationsByID map[gametypes.CRC32]int\n".to_owned(),
        field_values: "\t\tpromotionMutationsByID: make(map[gametypes.CRC32]int),\n".to_owned(),
        initializers: format!(
            r#"	for index := range manager.{row_field}.entries {{
		source := &manager.{row_field}.entries[index]
		key := strings.TrimSpace({id_expression})
		id := gametypes.CRC32(crc32Lowercase(key))
		if key == "" || id == 0 {{ continue }}
		if _, exists := manager.promotionMutationsByID[id]; exists {{ continue }}
		data := PromotionMutationStaticData{{Source: source.Ref, Key: key, ID: id, PossibleStatusEffectIDsByElement: make(map[gametypes.CRC32][]gametypes.CRC32)}}
{promotion_initializers}		promotionsBySlot := make(map[gametypes.CRC32]PromotionStatusEffect)
		for _, promotion := range data.Promotions {{ promotionsBySlot[promotion.SlotTagID] = promotion }}
		for elemental := range _elementalMutationStaticData.ElementalMutations() {{
			selected := make(map[string]elementalStatusSelection)
			for _, bucket := range elemental.Buckets {{
				for entry := range _buffBucketData.VisitAllBuffsFromID(bucket.BucketID) {{
					if entry.Kind != BuffBucketEntryPromotion {{ continue }}
					promotion, exists := promotionsBySlot[entry.BuffID]
					if !exists {{ continue }}
					statusEffect := _statusEffectData.StatusEffectDataFromID(promotion.StatusEffectID)
					if statusEffect == nil {{ continue }}
					insertElementalStatusSelection(selected, promotion.StatusEffectKey, promotion.StatusEffectID, int32({priority_expression}))
				}}
			}}
			ordered := make([]elementalStatusSelection, 0, len(selected))
			for _, candidate := range selected {{ ordered = append(ordered, candidate) }}
			sort.Slice(ordered, func(left int, right int) bool {{ if ordered[left].UIPriority != ordered[right].UIPriority {{ return ordered[left].UIPriority > ordered[right].UIPriority }}; return ordered[left].ID < ordered[right].ID }})
			for _, candidate := range ordered {{ data.PossibleStatusEffectIDsByElement[elemental.ID] = append(data.PossibleStatusEffectIDsByElement[elemental.ID], candidate.ID) }}
		}}
		manager.promotionMutationsByID[id] = len(manager.promotionMutations)
		manager.promotionMutations = append(manager.promotionMutations, data)
	}}
"#
        ),
        methods: format!(
            r#"func (manager *{manager_type}) PromotionMutationStaticDataFromID(id gametypes.CRC32) *PromotionMutationStaticData {{
	index, ok := manager.promotionMutationsByID[id]
	if !ok {{ return nil }}
	return rowCopy(manager.promotionMutations[index])
}}

func (manager *{manager_type}) PromotionMutationStaticData(key string) *PromotionMutationStaticData {{ return manager.PromotionMutationStaticDataFromID(gametypes.CRC32(crc32Lowercase(key))) }}

func (manager *{manager_type}) PossiblePromotionalStatusEffectsForElement(promotionID gametypes.CRC32, elementalID gametypes.CRC32) iter.Seq[gametypes.CRC32] {{
	data := manager.PromotionMutationStaticDataFromID(promotionID)
	if data == nil {{ return slices.Values([]gametypes.CRC32(nil)) }}
	return slices.Values(data.PossibleStatusEffectIDsByElement[elementalID])
}}

func (manager *{manager_type}) PossiblePromotionalStatusEffectsForElementByKey(promotionKey string, elementalKey string) iter.Seq[gametypes.CRC32] {{
	return manager.PossiblePromotionalStatusEffectsForElement(gametypes.CRC32(crc32Lowercase(promotionKey)), gametypes.CRC32(crc32Lowercase(elementalKey)))
}}

func (manager *{manager_type}) PromotionMutations() iter.Seq[PromotionMutationStaticData] {{ return rowValues(manager.promotionMutations) }}
func (manager *{manager_type}) Rows() iter.Seq[PromotionMutationStaticData] {{ return manager.PromotionMutations() }}

"#
        ),
    }
}

fn musical_rewards(
    unit: &GameDataCompileUnit,
    manager: &DirectManagerSurface,
) -> GoNativeManagerAugmentation {
    let mut augmentation = indexed::crc_secondary_contract(
        unit,
        manager,
        "MusicalPerformanceRewards",
        "RewardID",
        "RewardFromID",
        "Reward",
        "musicalRewardsByID",
    );
    let row = indexed::required_row(unit, manager, "MusicalPerformanceRewards");
    let row_field = go_direct_row_field_name("MusicalPerformanceRewards");
    let row_name = row.type_name.clone();
    let manager_type = go_method_name(&manager.manager_class_name);
    let event_fields = [
        "GameEventId_Rank_Amazing",
        "GameEventId_Rank_Great",
        "GameEventId_Rank_Okay",
        "GameEventId_Rank_Bad",
    ]
    .into_iter()
    .filter_map(|column| {
        row.fields
            .iter()
            .find(|field| field.source_name.eq_ignore_ascii_case(column))
            .map(|field| field.field_name.clone())
    })
    .collect::<Vec<_>>();
    let event_initializers = event_fields
        .iter()
        .map(|field| format!(
            "\t\tif source.Row.{field} != nil {{ value := strings.TrimSpace(*source.Row.{field}); if value != \"\" {{ manager.musicalRewardsByGameEvent[gametypes.CRC32(crc32Lowercase(value))] = index }} }}\n"
        ))
        .collect::<String>();
    augmentation
        .fields
        .push_str("\tmusicalRewardsByGameEvent map[gametypes.CRC32]int\n");
    augmentation
        .field_values
        .push_str("\t\tmusicalRewardsByGameEvent: make(map[gametypes.CRC32]int),\n");
    augmentation.initializers.push_str(&format!(
        "\tfor index := range manager.{row_field}.entries {{\n\t\tsource := &manager.{row_field}.entries[index]\n{event_initializers}\t}}\n"
    ));
    augmentation.methods.push_str(&format!(
        r#"func (manager *{manager_type}) RewardForGameEvent(id gametypes.CRC32) *{row_name} {{
	index, ok := manager.musicalRewardsByGameEvent[id]
	if !ok {{ return nil }}
	return rowCopy(manager.{row_field}.entries[index].Row)
}}

func (manager *{manager_type}) RewardForGameEventKey(key string) *{row_name} {{
	return manager.RewardForGameEvent(gametypes.CRC32(crc32Lowercase(key)))
}}

"#
    ));
    augmentation
}

fn combat_profiles(
    unit: &GameDataCompileUnit,
    manager: &DirectManagerSurface,
) -> GoNativeManagerAugmentation {
    let mut augmentation = indexed::crc_secondary_contract(
        unit,
        manager,
        "CombatProfilesData",
        "WeaponName",
        "Profile",
        "ProfileByKey",
        "combatProfilesByWeapon",
    );
    let row = indexed::required_row(unit, manager, "CombatProfilesData");
    let profile_type = indexed::required_field(&row, "ProfileType");
    let weapon = indexed::required_field(&row, "WeaponName");
    let row_field = go_direct_row_field_name("CombatProfilesData");
    let row_name = row.type_name.clone();
    let manager_type = go_method_name(&manager.manager_class_name);
    let type_expression = string_expression(profile_type, "source.Row");
    let weapon_expression = string_expression(weapon, "source.Row");
    augmentation.fields.push_str("\tcombatUnarmed int\n\thasCombatUnarmed bool\n\tcombatHeartgem int\n\thasCombatHeartgem bool\n\tcombatSiege map[string]int\n\tcombatActiveAbility map[gametypes.CRC32]int\n\tcombatAbilityAimPose map[gametypes.CRC32]int\n\tcombatItemClass map[string]int\n");
    augmentation.field_values.push_str("\t\tcombatSiege: make(map[string]int),\n\t\tcombatActiveAbility: make(map[gametypes.CRC32]int),\n\t\tcombatAbilityAimPose: make(map[gametypes.CRC32]int),\n\t\tcombatItemClass: make(map[string]int),\n");
    augmentation.initializers.push_str(&format!(r#"	for index := range manager.{row_field}.entries {{
		source := &manager.{row_field}.entries[index]
		profileType := strings.ToLower(strings.TrimSpace({type_expression}))
		weaponKey := strings.TrimSpace({weapon_expression})
		weaponID := gametypes.CRC32(crc32Lowercase(weaponKey))
		switch profileType {{
		case "weapon":
			class := strings.ToLower(weaponKey); if _, exists := manager.combatItemClass[class]; !exists {{ manager.combatItemClass[class] = index }}
			if class == "heartgem" && !manager.hasCombatHeartgem {{ manager.combatHeartgem = index; manager.hasCombatHeartgem = true }}
		case "unarmed": if !manager.hasCombatUnarmed {{ manager.combatUnarmed = index; manager.hasCombatUnarmed = true }}
		case "siegeweapon": if _, exists := manager.combatSiege[strings.ToLower(weaponKey)]; !exists {{ manager.combatSiege[strings.ToLower(weaponKey)] = index }}
		case "activeability": if _, exists := manager.combatActiveAbility[weaponID]; !exists {{ manager.combatActiveAbility[weaponID] = index }}
		case "abilityaimpose": if _, exists := manager.combatAbilityAimPose[weaponID]; !exists {{ manager.combatAbilityAimPose[weaponID] = index }}
		}}
	}}
"#));
    augmentation.methods.push_str(&indexed::named_rows_method(
        unit,
        manager,
        "CombatProfilesData",
        "Profiles",
    ));
    augmentation.methods.push_str(&format!(r#"func (manager *{manager_type}) HeartgemProfile() *{row_name} {{ if !manager.hasCombatHeartgem {{ return nil }}; return rowCopy(manager.{row_field}.entries[manager.combatHeartgem].Row) }}
func (manager *{manager_type}) UnarmedProfile() *{row_name} {{ if !manager.hasCombatUnarmed {{ return nil }}; return rowCopy(manager.{row_field}.entries[manager.combatUnarmed].Row) }}
func (manager *{manager_type}) SiegeWeaponProfile(kind string) *{row_name} {{ index, ok := manager.combatSiege[strings.ToLower(strings.TrimSpace(kind))]; if !ok {{ return nil }}; return rowCopy(manager.{row_field}.entries[index].Row) }}
func (manager *{manager_type}) ActiveAbilityProfile(id gametypes.CRC32) *{row_name} {{ index, ok := manager.combatActiveAbility[id]; if !ok {{ return nil }}; return rowCopy(manager.{row_field}.entries[index].Row) }}
func (manager *{manager_type}) ActiveAbilityProfileByKey(key string) *{row_name} {{ return manager.ActiveAbilityProfile(gametypes.CRC32(crc32Lowercase(key))) }}
func (manager *{manager_type}) AbilityAimPoseProfile(id gametypes.CRC32) *{row_name} {{ index, ok := manager.combatAbilityAimPose[id]; if !ok {{ return nil }}; return rowCopy(manager.{row_field}.entries[index].Row) }}
func (manager *{manager_type}) AbilityAimPoseProfileByKey(key string) *{row_name} {{ return manager.AbilityAimPoseProfile(gametypes.CRC32(crc32Lowercase(key))) }}
func (manager *{manager_type}) ItemClassProfile(class string) *{row_name} {{ index, ok := manager.combatItemClass[strings.ToLower(strings.TrimSpace(class))]; if !ok {{ return nil }}; return rowCopy(manager.{row_field}.entries[index].Row) }}

"#));
    augmentation
}

fn gatherable(
    unit: &GameDataCompileUnit,
    manager: &DirectManagerSurface,
) -> GoNativeManagerAugmentation {
    let row = go_direct_default_row_spec(unit, manager)
        .expect("GatherableDataManager requires gatherable schema rows");
    let id = indexed::required_field(&row, "GatherableID");
    let row_field = go_direct_row_field_name(&row.source_row_type);
    let table_type = go_direct_table_type_name(manager, &row.source_row_type, true);
    let row_name = row.type_name.clone();
    let id_expression = string_expression(id, "source.Row");
    let action_expression = optional_field(&row, "GatheringAction")
        .map(|field| string_expression(field, "source.Row"))
        .unwrap_or_else(|| "\"\"".to_owned());
    let type_expression = optional_field(&row, "GatheringType")
        .map(|field| string_expression(field, "source.Row"))
        .unwrap_or_else(|| "\"\"".to_owned());
    let table = GoNativeManagerAugmentation {
        declarations: format!(
            r#"
type GatherableData struct {{ Source RowRef[{table_type}, {row_name}]; Key string; ID gametypes.CRC32; GatheringActionKey string; GatheringActionID gametypes.CRC32; GatheringTypeKey string; GatheringTypeID gametypes.CRC32 }}
"#
        ),
        fields: "\tgatherables []GatherableData\n\tgatherablesByID map[gametypes.CRC32]int\n\tgatherablesByTable map[".to_owned()
            + &table_type
            + "][]int\n",
        field_values: "\t\tgatherablesByID: make(map[gametypes.CRC32]int),\n\t\tgatherablesByTable: make(map[".to_owned()
            + &table_type
            + "][]int),\n",
        initializers: format!(
            r#"	for index := range manager.{row_field}.entries {{
		source := &manager.{row_field}.entries[index]
		key := strings.TrimSpace({id_expression})
		id := gametypes.CRC32(crc32Lowercase(key))
		if key == "" || id == 0 {{ continue }}
		if _, exists := manager.gatherablesByID[id]; exists {{ continue }}
		actionKey := strings.TrimSpace({action_expression})
		typeKey := strings.TrimSpace({type_expression})
		data := GatherableData{{Source: source.Ref, Key: key, ID: id, GatheringActionKey: actionKey, GatheringActionID: gametypes.CRC32(crc32Lowercase(actionKey)), GatheringTypeKey: typeKey, GatheringTypeID: gametypes.CRC32(crc32Lowercase(typeKey))}}
		manager.gatherablesByID[id] = len(manager.gatherables)
		manager.gatherablesByTable[source.Ref.Table()] = append(manager.gatherablesByTable[source.Ref.Table()], len(manager.gatherables))
		manager.gatherables = append(manager.gatherables, data)
	}}
"#
        ),
        methods: format!(
            r#"func (manager *GatherableDataManager) GatherableData(id gametypes.CRC32) *GatherableData {{ index, ok := manager.gatherablesByID[id]; if !ok {{ return nil }}; return rowCopy(manager.gatherables[index]) }}
func (manager *GatherableDataManager) GatherableDataFromID(key string) *GatherableData {{ return manager.GatherableData(gametypes.CRC32(crc32Lowercase(key))) }}
func (manager *GatherableDataManager) Gatherables(table {table_type}) iter.Seq[GatherableData] {{ return func(yield func(GatherableData) bool) {{ for _, index := range manager.gatherablesByTable[table] {{ if !yield(manager.gatherables[index]) {{ return }} }} }} }}
func (manager *GatherableDataManager) GatherableRows() iter.Seq[GatherableData] {{ return rowValues(manager.gatherables) }}
func (manager *GatherableDataManager) Rows() iter.Seq[GatherableData] {{ return manager.GatherableRows() }}

"#
        ),
    };
    merge_augmentations([table, gathering_products()])
}

fn gathering_products() -> GoNativeManagerAugmentation {
    GoNativeManagerAugmentation {
        fields: "\tgatheringTypesByID map[gametypes.CRC32]int\n\tgatheringActionsByID map[gametypes.CRC32]int\n".to_owned(),
        field_values: "\t\tgatheringTypesByID: make(map[gametypes.CRC32]int),\n\t\tgatheringActionsByID: make(map[gametypes.CRC32]int),\n".to_owned(),
        initializers: r#"	for index := range manager.gatheringDatabase.GatheringData.GatheringTypes {
		key := gametypes.CRC32(crc32Lowercase(manager.gatheringDatabase.GatheringData.GatheringTypes[index].GatheringType))
		if key != 0 { if _, exists := manager.gatheringTypesByID[key]; !exists { manager.gatheringTypesByID[key] = index } }
	}
	for index := range manager.gatheringActionDatabase.GatheringActions {
		key := gametypes.CRC32(crc32Lowercase(manager.gatheringActionDatabase.GatheringActions[index].Name))
		if key != 0 { if _, exists := manager.gatheringActionsByID[key]; !exists { manager.gatheringActionsByID[key] = index } }
	}
"#
        .to_owned(),
        methods: r#"func (manager *GatherableDataManager) GatheringType(id gametypes.CRC32) *GatheringTypeData {
	index, ok := manager.gatheringTypesByID[id]
	if !ok { return nil }
	return rowCopy(manager.gatheringDatabase.GatheringData.GatheringTypes[index])
}

func (manager *GatherableDataManager) GatheringTypeByKey(key string) *GatheringTypeData {
	return manager.GatheringType(gametypes.CRC32(crc32Lowercase(key)))
}

func (manager *GatherableDataManager) GatheringAction(id gametypes.CRC32) *GatheringActionData {
	index, ok := manager.gatheringActionsByID[id]
	if !ok { return nil }
	return rowCopy(manager.gatheringActionDatabase.GatheringActions[index])
}

func (manager *GatherableDataManager) GatheringActionByKey(key string) *GatheringActionData {
	return manager.GatheringAction(gametypes.CRC32(crc32Lowercase(key)))
}

"#
        .to_owned(),
        declarations: String::new(),
    }
}

fn social() -> GoNativeManagerAugmentation {
    GoNativeManagerAugmentation {
        fields: "\tsocialRanksByName map[string]int\n\tsocialRanksBySecurityLevel map[uint32]int\n".to_owned(),
        field_values: "\t\tsocialRanksByName: make(map[string]int),\n\t\tsocialRanksBySecurityLevel: make(map[uint32]int),\n".to_owned(),
        initializers: r#"	for index := range manager.socialRankDatabase.Ranks {
		rank := &manager.socialRankDatabase.Ranks[index].GuildRankData
		name := strings.ToLower(strings.TrimSpace(rank.Name))
		if name != "" { if _, exists := manager.socialRanksByName[name]; !exists { manager.socialRanksByName[name] = index } }
		if _, exists := manager.socialRanksBySecurityLevel[rank.SecurityLevel]; !exists { manager.socialRanksBySecurityLevel[rank.SecurityLevel] = index }
	}
"#
        .to_owned(),
        methods: r#"func (manager *SocialDataManager) RankByName(name string) *SocialRankData {
	index, ok := manager.socialRanksByName[strings.ToLower(strings.TrimSpace(name))]
	if !ok { return nil }
	return rowCopy(manager.socialRankDatabase.Ranks[index])
}

func (manager *SocialDataManager) RankBySecurityLevel(level uint32) *SocialRankData {
	index, ok := manager.socialRanksBySecurityLevel[level]
	if !ok { return nil }
	return rowCopy(manager.socialRankDatabase.Ranks[index])
}

"#
        .to_owned(),
        declarations: String::new(),
    }
}

fn player() -> GoNativeManagerAugmentation {
    GoNativeManagerAugmentation {
        methods: r#"func (manager *PlayerDataManager) HasPlayerBaseAttributes() bool { return manager.playerBaseAttributes != nil }
func (manager *PlayerDataManager) HasSettlementProgressionData() bool { return manager.settlementProgressionData != nil }

"#
        .to_owned(),
        ..Default::default()
    }
}

fn recipe(
    unit: &GameDataCompileUnit,
    manager: &DirectManagerSurface,
) -> GoNativeManagerAugmentation {
    let row = go_direct_default_row_spec(unit, manager)
        .expect("RecipeDataManager requires recipe family schema rows");
    let recipe_id = indexed::required_field(&row, "RecipeID");
    let row_field = go_direct_row_field_name(&row.source_row_type);
    let table_type = go_direct_table_type_name(manager, &row.source_row_type, true);
    let row_name = row.type_name.clone();
    let recipe_id_expression = string_expression(recipe_id, "source.Row");
    let category_expression = optional_field(&row, "CraftingCategory")
        .map(|field| string_expression(field, "source.Row"))
        .unwrap_or_else(|| "\"\"".to_owned());
    let item_expression = optional_field(&row, "ItemID")
        .map(|field| string_expression(field, "source.Row"))
        .unwrap_or_else(|| "\"\"".to_owned());
    let output_expression = optional_field(&row, "OutputQty")
        .map(|field| number_expression(field, "source.Row"))
        .unwrap_or_else(|| "1".to_owned());
    let ingredient_initializers = (1..=7u8)
        .filter_map(|slot| {
            let ingredient = optional_field(&row, &format!("Ingredient{slot}"))?;
            let ingredient_expression = string_expression(ingredient, "source.Row");
            let quantity_expression = optional_field(&row, &format!("Qty{slot}"))
                .map(|field| number_expression(field, "source.Row"))
                .unwrap_or_else(|| "1".to_owned());
            Some(format!(
                r#"		ingredient{slot}Key := strings.TrimSpace({ingredient_expression})
		ingredient{slot}ID := gametypes.CRC32(crc32Lowercase(ingredient{slot}Key))
		if ingredient{slot}ID != 0 {{ if quantity, ok := exactUint32({quantity_expression}); ok && quantity != 0 {{ data.Ingredients = append(data.Ingredients, RecipeIngredient{{IngredientID: ingredient{slot}ID, Quantity: quantity}}) }} }}
"#
            ))
        })
        .collect::<String>();
    GoNativeManagerAugmentation {
        declarations: format!(
            r#"
type RecipeIngredient struct {{ IngredientID gametypes.CRC32; Quantity uint32 }}
type RecipeData struct {{ Source RowRef[{table_type}, {row_name}]; RecipeKey string; RecipeID gametypes.CRC32; CraftingCategory gametypes.CRC32; ItemID gametypes.CRC32; OutputQuantity uint32; Ingredients []RecipeIngredient }}
"#
        ),
        fields: "\trecipes []RecipeData\n\trecipesByID map[gametypes.CRC32]int\n\trecipesByResult map[gametypes.CRC32][]int\n\trecipesByCategory map[gametypes.CRC32][]int\n\tcraftingStationsByName map[string]int\n\tcraftingStationsByType map[string][]int\n".to_owned(),
        field_values: "\t\trecipesByID: make(map[gametypes.CRC32]int),\n\t\trecipesByResult: make(map[gametypes.CRC32][]int),\n\t\trecipesByCategory: make(map[gametypes.CRC32][]int),\n\t\tcraftingStationsByName: make(map[string]int),\n\t\tcraftingStationsByType: make(map[string][]int),\n".to_owned(),
        initializers: format!(r#"	for index := range manager.{row_field}.entries {{
		source := &manager.{row_field}.entries[index]
		key := strings.TrimSpace({recipe_id_expression})
		id := gametypes.CRC32(crc32Lowercase(key))
		if key == "" || id == 0 {{ continue }}
		if _, exists := manager.recipesByID[id]; exists {{ continue }}
		outputQuantity, ok := exactUint32({output_expression})
		if !ok || outputQuantity == 0 {{ outputQuantity = 1 }}
		data := RecipeData{{Source: source.Ref, RecipeKey: key, RecipeID: id, CraftingCategory: gametypes.CRC32(crc32Lowercase(strings.TrimSpace({category_expression}))), ItemID: gametypes.CRC32(crc32Lowercase(strings.TrimSpace({item_expression}))), OutputQuantity: outputQuantity}}
{ingredient_initializers}		manager.recipesByID[id] = len(manager.recipes)
		if data.ItemID != 0 {{ manager.recipesByResult[data.ItemID] = append(manager.recipesByResult[data.ItemID], len(manager.recipes)) }}
		if data.CraftingCategory != 0 {{ manager.recipesByCategory[data.CraftingCategory] = append(manager.recipesByCategory[data.CraftingCategory], len(manager.recipes)) }}
		manager.recipes = append(manager.recipes, data)
	}}
	for index := range manager.craftingStationDatabase.CraftingStations {{
		station := &manager.craftingStationDatabase.CraftingStations[index]
		name := strings.ToLower(strings.TrimSpace(station.Name))
		if name != "" {{ if _, exists := manager.craftingStationsByName[name]; !exists {{ manager.craftingStationsByName[name] = index }} }}
		for _, craftingType := range station.CraftingTypes {{
			key := strings.ToLower(strings.TrimSpace(craftingType))
			if key != "" {{ manager.craftingStationsByType[key] = append(manager.craftingStationsByType[key], index) }}
		}}
	}}
"#),
        methods: r#"func (manager *RecipeDataManager) CraftingRecipeData(key string) *RecipeData { return manager.CraftingRecipeDataFromID(gametypes.CRC32(crc32Lowercase(key))) }
func (manager *RecipeDataManager) CraftingRecipeDataFromID(id gametypes.CRC32) *RecipeData { index, ok := manager.recipesByID[id]; if !ok { return nil }; return rowCopy(manager.recipes[index]) }
func (manager *RecipeDataManager) CraftingRecipeDataByResult(itemID gametypes.CRC32) iter.Seq[RecipeData] { return manager.recipeIndexes(manager.recipesByResult[itemID]) }
func (manager *RecipeDataManager) RecipesByCategory(category string) iter.Seq[RecipeData] { return manager.RecipesByCategoryID(gametypes.CRC32(crc32Lowercase(category))) }
func (manager *RecipeDataManager) RecipesByCategoryID(categoryID gametypes.CRC32) iter.Seq[RecipeData] { return manager.recipeIndexes(manager.recipesByCategory[categoryID]) }
func (manager *RecipeDataManager) Recipes() iter.Seq[RecipeData] { return rowValues(manager.recipes) }
func (manager *RecipeDataManager) Rows() iter.Seq[RecipeData] { return manager.Recipes() }
func (manager *RecipeDataManager) RecipesByIngredients(available map[gametypes.CRC32]uint32) iter.Seq[RecipeData] { return func(yield func(RecipeData) bool) { for index := range manager.recipes { recipe := manager.recipes[index]; matches := true; for _, ingredient := range recipe.Ingredients { if available[ingredient.IngredientID] < ingredient.Quantity { matches = false; break } }; if matches && !yield(recipe) { return } } } }
func (manager *RecipeDataManager) recipeIndexes(indexes []int) iter.Seq[RecipeData] { return func(yield func(RecipeData) bool) { for _, index := range indexes { if !yield(manager.recipes[index]) { return } } } }

func (manager *RecipeDataManager) CraftingStation(name string) *CraftingStationData {
	index, ok := manager.craftingStationsByName[strings.ToLower(strings.TrimSpace(name))]
	if !ok { return nil }
	return rowCopy(manager.craftingStationDatabase.CraftingStations[index])
}

func (manager *RecipeDataManager) CraftingStationsForType(craftingType string) iter.Seq[CraftingStationData] {
	return func(yield func(CraftingStationData) bool) {
		for _, index := range manager.craftingStationsByType[strings.ToLower(strings.TrimSpace(craftingType))] {
			if !yield(manager.craftingStationDatabase.CraftingStations[index]) { return }
		}
	}
}

"#
        .to_owned(),
    }
}

fn optional_field<'a>(row: &'a GoSchemaRow, column: &str) -> Option<&'a GoSchemaField> {
    row.fields
        .iter()
        .find(|field| field.source_name.eq_ignore_ascii_case(column))
}

fn string_expression(field: &GoSchemaField, receiver: &str) -> String {
    if field.required {
        format!("{receiver}.{}", field.field_name)
    } else {
        format!("stringValue({receiver}.{})", field.field_name)
    }
}

fn number_expression(field: &GoSchemaField, receiver: &str) -> String {
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
