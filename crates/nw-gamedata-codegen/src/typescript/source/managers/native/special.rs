use super::*;

pub(super) fn augmentation(
    unit: &GameDataCompileUnit,
    manager: &DirectManagerSurface,
    shape: &NativeManagerShape,
) -> TsNativeManagerAugmentation {
    match shape {
        NativeManagerShape::ElementalMutationStaticData(_) => elemental_mutations(unit, manager),
        NativeManagerShape::PromotionMutationStaticData(_) => promotion_mutations(unit, manager),
        NativeManagerShape::MusicalRewardsData(_) => musical_rewards(unit, manager),
        NativeManagerShape::CombatProfilesData(_) => combat_profiles(unit, manager),
        NativeManagerShape::ItemTransformData(_) => item_transform(unit, manager),
        NativeManagerShape::GatherableData(_) => gatherable(unit, manager),
        NativeManagerShape::SocialData(_) => social(),
        NativeManagerShape::PlayerData(_) => player(),
        NativeManagerShape::RecipeData(_) => recipe(unit, manager),
        _ => panic!(
            "manager {} reached special TypeScript native dispatch with {shape:?}",
            manager.manager_name
        ),
    }
}

fn elemental_mutations(
    unit: &GameDataCompileUnit,
    manager: &DirectManagerSurface,
) -> TsNativeManagerAugmentation {
    let row = required_row(unit, manager, "ElementalMutationStaticData");
    let id = string_expression(required_field(&row, "ElementalMutationID"), "source.row");
    let field = ts_direct_row_field_name("ElementalMutationStaticData");
    let table = ts_direct_table_type_name(manager, "ElementalMutationStaticData");
    let schema = row.type_name.clone();
    let mut bucket_fields = row
        .fields
        .iter()
        .filter(|column| column.source_name.eq_ignore_ascii_case("Dungeon"))
        .collect::<Vec<_>>();
    for name in ["Dungeon2", "Dungeon3"] {
        if let Some(column) = optional_field(&row, name) {
            if !bucket_fields
                .iter()
                .any(|value| value.field_name == column.field_name)
            {
                bucket_fields.push(column);
            }
        }
    }
    let mut buckets = ["Dungeon-", "Dungeon", "Dungeon+"]
        .into_iter()
        .zip(bucket_fields)
        .collect::<Vec<_>>();
    for (creature, column) in [
        ("DungeonMiniBoss", "DungeonMiniBoss"),
        ("DungeonBoss", "DungeonBoss"),
    ] {
        if let Some(field) = optional_field(&row, column) {
            buckets.push((creature, field));
        }
    }
    let bucket_init = buckets.into_iter().map(|(creature, column)| {
        let expression = string_expression(column, "source.row");
        format!(r#"      {{
        const bucketKey = {expression}.trim(); const bucketId = Crc32.fromStringLower(bucketKey);
        if (bucketId !== Crc32.ZERO) {{ data.buckets.push(Object.freeze({{ creatureTypeId: Crc32.fromStringLower({creature:?}), bucketKey, bucketId }})); for (const entry of _buffBucketData.visitAllBuffsFromId(bucketId)) {{ if (entry.kind !== "StatusEffect") continue; const status = _statusEffectData.statusEffectDataFromId(entry.buffId); if (status !== undefined) selectHighestTierStatus(selected, entry.buffKey, entry.buffId, Math.trunc(optionalSchemaNumber(status.uiPriority) ?? 0)); }} }}
      }}
"#)
    }).collect::<String>();
    TsNativeManagerAugmentation {
        declarations: format!(r#"export interface ElementalMutationBucket {{ readonly creatureTypeId: Crc32; readonly bucketKey: string; readonly bucketId: Crc32; }}
export interface ElementalMutationStaticData {{ readonly source: RowRef<{table}, {schema}>; readonly key: string; readonly id: Crc32; readonly buckets: ElementalMutationBucket[]; readonly possibleStatusEffectIds: Crc32[]; }}
interface ElementalStatusSelection {{ readonly id: Crc32; readonly tier: number; readonly uiPriority: number; }}
function selectHighestTierStatus(selected: Map<string, ElementalStatusSelection>, key: string, id: Crc32, uiPriority: number): void {{ const group = key.replace(/[0-9]/g, ""); const tier = Number.parseInt(key.replace(/[^0-9]/g, ""), 10) || 0; const current = selected.get(group); if (current === undefined || current.tier < tier) selected.set(group, Object.freeze({{ id, tier, uiPriority }})); }}

"#),
        fields: "  private readonly elementalMutationEntries: ElementalMutationStaticData[] = [];\n  private readonly elementalMutationsById = new Map<Crc32, ElementalMutationStaticData>();\n".to_owned(),
        initializers: format!(r#"    for (const source of this.{field}) {{
      const key = {id}.trim(); const id = Crc32.fromStringLower(key); if (key.length === 0 || id === Crc32.ZERO || this.elementalMutationsById.has(id)) continue;
      const data: ElementalMutationStaticData = {{ source: source.ref, key, id, buckets: [], possibleStatusEffectIds: [] }}; const selected = new Map<string, ElementalStatusSelection>();
{bucket_init}      data.possibleStatusEffectIds.push(...[...selected.values()].sort((left, right) => right.uiPriority - left.uiPriority || left.id - right.id).map((value) => value.id)); this.elementalMutationEntries.push(data); this.elementalMutationsById.set(id, data);
    }}
"#),
        methods: "  elementalMutationStaticDataFromId(id: Crc32): ElementalMutationStaticData | undefined { return this.elementalMutationsById.get(id); }\n  elementalMutationStaticData(key: string): ElementalMutationStaticData | undefined { return this.elementalMutationStaticDataFromId(Crc32.fromStringLower(key)); }\n  possibleElementalStatusEffects(id: Crc32): IterableIterator<Crc32> { return (this.elementalMutationStaticDataFromId(id)?.possibleStatusEffectIds ?? []).values(); }\n  possibleElementalStatusEffectsByKey(key: string): IterableIterator<Crc32> { return this.possibleElementalStatusEffects(Crc32.fromStringLower(key)); }\n  elementalMutations(): IterableIterator<ElementalMutationStaticData> { return this.elementalMutationEntries.values(); }\n\n".to_owned(),
        rows_interface: Some(" implements Rows<ElementalMutationStaticData>".to_owned()),
        row_methods: Some("  rows(): IterableIterator<ElementalMutationStaticData> { return this.elementalMutationEntries.values(); }\n  [Symbol.iterator](): Iterator<ElementalMutationStaticData> { return this.rows(); }\n\n".to_owned()),
    }
}

fn promotion_mutations(
    unit: &GameDataCompileUnit,
    manager: &DirectManagerSurface,
) -> TsNativeManagerAugmentation {
    let row = required_row(unit, manager, "PromotionMutationStaticData");
    let id = string_expression(required_field(&row, "PromotionMutationID"), "source.row");
    let field = ts_direct_row_field_name("PromotionMutationStaticData");
    let table = ts_direct_table_type_name(manager, "PromotionMutationStaticData");
    let schema = row.type_name.clone();
    let promotions = (1..=3u8).filter_map(|slot| optional_field(&row, &format!("Promotion{slot}")).map(|column| (slot, string_expression(column, "source.row")))).map(|(slot, expression)| format!("      {{ const statusEffectKey = {expression}.trim(); const statusEffectId = Crc32.fromStringLower(statusEffectKey); if (statusEffectId !== Crc32.ZERO) data.promotions.push(Object.freeze({{ slot: {slot}, slotTagId: Crc32.fromStringLower(\"Promotion{slot}\"), statusEffectKey, statusEffectId }})); }}\n")).collect::<String>();
    TsNativeManagerAugmentation {
        declarations: format!("export interface PromotionStatusEffect {{ readonly slot: number; readonly slotTagId: Crc32; readonly statusEffectKey: string; readonly statusEffectId: Crc32; }}\nexport interface PromotionMutationStaticData {{ readonly source: RowRef<{table}, {schema}>; readonly key: string; readonly id: Crc32; readonly promotions: PromotionStatusEffect[]; readonly possibleStatusEffectIdsByElement: Map<Crc32, Crc32[]>; }}\n\n"),
        fields: "  private readonly promotionMutationEntries: PromotionMutationStaticData[] = [];\n  private readonly promotionMutationsById = new Map<Crc32, PromotionMutationStaticData>();\n".to_owned(),
        initializers: format!(r#"    for (const source of this.{field}) {{
      const key = {id}.trim(); const id = Crc32.fromStringLower(key); if (key.length === 0 || id === Crc32.ZERO || this.promotionMutationsById.has(id)) continue;
      const data: PromotionMutationStaticData = {{ source: source.ref, key, id, promotions: [], possibleStatusEffectIdsByElement: new Map<Crc32, Crc32[]>() }};
{promotions}      const promotionsBySlot = new Map(data.promotions.map((value) => [value.slotTagId, value]));
      for (const elemental of _elementalMutationStaticData.elementalMutations()) {{ const selected = new Map<string, ElementalStatusSelection>(); for (const bucket of elemental.buckets) for (const entry of _buffBucketData.visitAllBuffsFromId(bucket.bucketId)) {{ if (entry.kind !== "Promotion") continue; const promotion = promotionsBySlot.get(entry.buffId); if (promotion === undefined) continue; const status = _statusEffectData.statusEffectDataFromId(promotion.statusEffectId); if (status !== undefined) selectHighestTierStatus(selected, promotion.statusEffectKey, promotion.statusEffectId, Math.trunc(optionalSchemaNumber(status.uiPriority) ?? 0)); }} data.possibleStatusEffectIdsByElement.set(elemental.id, [...selected.values()].sort((left, right) => right.uiPriority - left.uiPriority || left.id - right.id).map((value) => value.id)); }}
      this.promotionMutationEntries.push(data); this.promotionMutationsById.set(id, data);
    }}
"#),
        methods: "  promotionMutationStaticDataFromId(id: Crc32): PromotionMutationStaticData | undefined { return this.promotionMutationsById.get(id); }\n  promotionMutationStaticData(key: string): PromotionMutationStaticData | undefined { return this.promotionMutationStaticDataFromId(Crc32.fromStringLower(key)); }\n  possiblePromotionalStatusEffectsForElement(promotionId: Crc32, elementalId: Crc32): IterableIterator<Crc32> { return (this.promotionMutationStaticDataFromId(promotionId)?.possibleStatusEffectIdsByElement.get(elementalId) ?? []).values(); }\n  possiblePromotionalStatusEffectsForElementByKey(promotionKey: string, elementalKey: string): IterableIterator<Crc32> { return this.possiblePromotionalStatusEffectsForElement(Crc32.fromStringLower(promotionKey), Crc32.fromStringLower(elementalKey)); }\n  promotionMutations(): IterableIterator<PromotionMutationStaticData> { return this.promotionMutationEntries.values(); }\n\n".to_owned(),
        rows_interface: Some(" implements Rows<PromotionMutationStaticData>".to_owned()),
        row_methods: Some("  rows(): IterableIterator<PromotionMutationStaticData> { return this.promotionMutationEntries.values(); }\n  [Symbol.iterator](): Iterator<PromotionMutationStaticData> { return this.rows(); }\n\n".to_owned()),
    }
}

fn musical_rewards(
    unit: &GameDataCompileUnit,
    manager: &DirectManagerSurface,
) -> TsNativeManagerAugmentation {
    let mut value = crc_secondary_contract(
        unit,
        manager,
        "MusicalPerformanceRewards",
        "RewardID",
        "RewardFromID",
        "Reward",
        "musicalRewardsById",
    );
    let row = required_row(unit, manager, "MusicalPerformanceRewards");
    let field = ts_direct_row_field_name("MusicalPerformanceRewards");
    let table = ts_direct_table_type_name(manager, "MusicalPerformanceRewards");
    let schema = row.type_name.clone();
    let event_fields = ["GameEventId_Rank_Amazing", "GameEventId_Rank_Great", "GameEventId_Rank_Okay", "GameEventId_Rank_Bad"].into_iter().filter_map(|name| optional_field(&row, name)).map(|column| string_expression(column, "source.row")).map(|expression| format!("      {{ const id = Crc32.fromStringLower({expression}.trim()); if (id !== Crc32.ZERO) this.musicalRewardsByGameEvent.set(id, source); }}\n")).collect::<String>();
    value.fields.push_str(&format!("  private readonly musicalRewardsByGameEvent = new Map<Crc32, RowEntry<{table}, {schema}>>();\n"));
    value.initializers.push_str(&format!(
        "    for (const source of this.{field}) {{\n{event_fields}    }}\n"
    ));
    value.methods.push_str(&format!("  rewardForGameEvent(id: Crc32): {schema} | undefined {{ return this.musicalRewardsByGameEvent.get(id)?.row; }}\n  rewardForGameEventKey(key: string): {schema} | undefined {{ return this.rewardForGameEvent(Crc32.fromStringLower(key)); }}\n\n"));
    value
}

fn combat_profiles(
    unit: &GameDataCompileUnit,
    manager: &DirectManagerSurface,
) -> TsNativeManagerAugmentation {
    let mut value = crc_secondary_contract(
        unit,
        manager,
        "CombatProfilesData",
        "WeaponName",
        "Profile",
        "ProfileByKey",
        "combatProfilesByWeapon",
    );
    let row = required_row(unit, manager, "CombatProfilesData");
    let profile_type = string_expression(required_field(&row, "ProfileType"), "source.row");
    let weapon = string_expression(required_field(&row, "WeaponName"), "source.row");
    let field = ts_direct_row_field_name("CombatProfilesData");
    let table = ts_direct_table_type_name(manager, "CombatProfilesData");
    let schema = row.type_name.clone();
    value.fields.push_str(&format!("  private combatUnarmed: RowEntry<{table}, {schema}> | undefined;\n  private combatHeartgem: RowEntry<{table}, {schema}> | undefined;\n  private readonly combatSiege = new Map<string, RowEntry<{table}, {schema}>>();\n  private readonly combatActiveAbility = new Map<Crc32, RowEntry<{table}, {schema}>>();\n  private readonly combatAbilityAimPose = new Map<Crc32, RowEntry<{table}, {schema}>>();\n  private readonly combatItemClass = new Map<string, RowEntry<{table}, {schema}>>();\n"));
    value.initializers.push_str(&format!(r#"    for (const source of this.{field}) {{
      const profileType = normalizeLookupText({profile_type}); const weaponKey = {weapon}.trim(); const weaponId = Crc32.fromStringLower(weaponKey);
      switch (profileType) {{ case "weapon": {{ const itemClass = normalizeLookupText(weaponKey); if (!this.combatItemClass.has(itemClass)) this.combatItemClass.set(itemClass, source); if (itemClass === "heartgem" && this.combatHeartgem === undefined) this.combatHeartgem = source; break; }} case "unarmed": if (this.combatUnarmed === undefined) this.combatUnarmed = source; break; case "siegeweapon": if (!this.combatSiege.has(normalizeLookupText(weaponKey))) this.combatSiege.set(normalizeLookupText(weaponKey), source); break; case "activeability": if (!this.combatActiveAbility.has(weaponId)) this.combatActiveAbility.set(weaponId, source); break; case "abilityaimpose": if (!this.combatAbilityAimPose.has(weaponId)) this.combatAbilityAimPose.set(weaponId, source); break; }}
    }}
"#));
    value.methods.push_str(&named_rows_method(
        unit,
        manager,
        "CombatProfilesData",
        "Profiles",
    ));
    value.methods.push_str(&format!("  heartgemProfile(): {schema} | undefined {{ return this.combatHeartgem?.row; }}\n  unarmedProfile(): {schema} | undefined {{ return this.combatUnarmed?.row; }}\n  siegeWeaponProfile(kind: string): {schema} | undefined {{ return this.combatSiege.get(normalizeLookupText(kind))?.row; }}\n  activeAbilityProfile(id: Crc32): {schema} | undefined {{ return this.combatActiveAbility.get(id)?.row; }}\n  activeAbilityProfileByKey(key: string): {schema} | undefined {{ return this.activeAbilityProfile(Crc32.fromStringLower(key)); }}\n  abilityAimPoseProfile(id: Crc32): {schema} | undefined {{ return this.combatAbilityAimPose.get(id)?.row; }}\n  abilityAimPoseProfileByKey(key: string): {schema} | undefined {{ return this.abilityAimPoseProfile(Crc32.fromStringLower(key)); }}\n  itemClassProfile(itemClass: string): {schema} | undefined {{ return this.combatItemClass.get(normalizeLookupText(itemClass))?.row; }}\n\n"));
    value
}

fn item_transform(
    unit: &GameDataCompileUnit,
    manager: &DirectManagerSurface,
) -> TsNativeManagerAugmentation {
    let row = ts_direct_row_specs(unit, manager)
        .into_iter()
        .find(|row| optional_field(row, "FromItemID").is_some())
        .unwrap_or_else(|| panic!("{} requires an item transform row", manager.manager_name));
    let field = ts_direct_row_field_name(&row.source_row_type);
    let table = ts_direct_table_type_name(manager, &row.source_row_type);
    let schema = row.type_name.clone();
    let from = string_expression(required_field(&row, "FromItemID"), "source.row");
    let to = string_expression(required_field(&row, "ToItemID"), "source.row");
    let keep_perks = optional_field(&row, "KeepPerks")
        .map(|column| format!("schemaBoolean(source.row.{}, false)", column.field_name))
        .unwrap_or_else(|| "false".to_owned());
    let feature = optional_field(&row, "FeatureID")
        .map(|column| string_expression(column, "source.row"))
        .unwrap_or_else(|| "\"\"".to_owned());
    TsNativeManagerAugmentation {
        declarations: format!("export interface ItemTransformData {{ readonly source: RowRef<{table}, {schema}>; readonly fromItemKey: string; readonly fromItemId: Crc32; readonly toItemKey: string; readonly toItemId: Crc32; readonly keepPerks: boolean; readonly featureId: Crc32; }}\n\n"),
        fields: "  private readonly itemTransformEntries: ItemTransformData[] = [];\n  private readonly itemTransformsByKey = new Map<string, ItemTransformData>();\n".to_owned(),
        initializers: format!(r#"    for (const source of this.{field}) {{
      const fromItemKey = {from}.trim(); const fromItemId = Crc32.fromStringLower(fromItemKey); const toItemKey = {to}.trim(); const toItemId = Crc32.fromStringLower(toItemKey);
      if (fromItemId === Crc32.ZERO || toItemId === Crc32.ZERO) continue;
      const key = tableCrcLookupKey(source.ref.table, fromItemId); if (this.itemTransformsByKey.has(key)) continue;
      const data = Object.freeze({{ source: source.ref, fromItemKey, fromItemId, toItemKey, toItemId, keepPerks: {keep_perks}, featureId: Crc32.fromStringLower({feature}.trim()) }});
      this.itemTransformEntries.push(data); this.itemTransformsByKey.set(key, data);
    }}
"#),
        methods: format!("  transform(table: {table}, fromItemId: Crc32): ItemTransformData | undefined {{ return this.itemTransformsByKey.get(tableCrcLookupKey(table, fromItemId)); }}\n  transformByKey(table: {table}, key: string): ItemTransformData | undefined {{ return this.transform(table, Crc32.fromStringLower(key.trim())); }}\n  transforms(): IterableIterator<ItemTransformData> {{ return this.itemTransformEntries.values(); }}\n\n"),
        rows_interface: Some(" implements Rows<ItemTransformData>".to_owned()),
        row_methods: Some("  rows(): IterableIterator<ItemTransformData> { return this.itemTransformEntries.values(); }\n  [Symbol.iterator](): Iterator<ItemTransformData> { return this.rows(); }\n\n".to_owned()),
    }
}

fn gatherable(
    unit: &GameDataCompileUnit,
    manager: &DirectManagerSurface,
) -> TsNativeManagerAugmentation {
    let row = default_row(unit, manager);
    let id = string_expression(required_field(&row, "GatherableID"), "source.row");
    let action = optional_field(&row, "GatheringAction")
        .map(|column| string_expression(column, "source.row"))
        .unwrap_or_else(|| "\"\"".to_owned());
    let kind = optional_field(&row, "GatheringType")
        .map(|column| string_expression(column, "source.row"))
        .unwrap_or_else(|| "\"\"".to_owned());
    let field = ts_direct_row_field_name(&row.source_row_type);
    let table = ts_direct_table_type_name(manager, &row.source_row_type);
    let schema = row.type_name.clone();
    TsNativeManagerAugmentation {
        declarations: format!("export interface GatherableData {{ readonly source: RowRef<{table}, {schema}>; readonly key: string; readonly id: Crc32; readonly gatheringActionKey: string; readonly gatheringActionId: Crc32; readonly gatheringTypeKey: string; readonly gatheringTypeId: Crc32; }}\n\n"),
        fields: format!("  private readonly gatherableEntries: GatherableData[] = [];\n  private readonly gatherablesById = new Map<Crc32, GatherableData>();\n  private readonly gatherablesByTable = new Map<{table}, GatherableData[]>();\n  private readonly gatheringTypesById = new Map<Crc32, GatheringTypeData>();\n  private readonly gatheringActionsById = new Map<Crc32, GatheringActionData>();\n"),
        initializers: format!(r#"    for (const source of this.{field}) {{ const key = {id}.trim(); const id = Crc32.fromStringLower(key); if (key.length === 0 || id === Crc32.ZERO || this.gatherablesById.has(id)) continue; const gatheringActionKey = {action}.trim(); const gatheringTypeKey = {kind}.trim(); const data = Object.freeze({{ source: source.ref, key, id, gatheringActionKey, gatheringActionId: Crc32.fromStringLower(gatheringActionKey), gatheringTypeKey, gatheringTypeId: Crc32.fromStringLower(gatheringTypeKey) }}); this.gatherableEntries.push(data); this.gatherablesById.set(id, data); appendMapValue(this.gatherablesByTable, source.ref.table, data); }}
    for (const value of this.gatheringDatabaseValue.gatheringData.gatheringTypes) {{ const id = Crc32.fromStringLower(value.gatheringType); if (!this.gatheringTypesById.has(id)) this.gatheringTypesById.set(id, value); }}
    for (const value of this.gatheringActionDatabaseValue.gatheringActions) {{ const id = Crc32.fromStringLower(value.name); if (!this.gatheringActionsById.has(id)) this.gatheringActionsById.set(id, value); }}
"#),
        methods: format!("  gatherableData(id: Crc32): GatherableData | undefined {{ return this.gatherablesById.get(id); }}\n  gatherableDataFromId(key: string): GatherableData | undefined {{ return this.gatherableData(Crc32.fromStringLower(key)); }}\n  gatherables(table: {table}): IterableIterator<GatherableData> {{ return (this.gatherablesByTable.get(table) ?? []).values(); }}\n  gatherableRows(): IterableIterator<GatherableData> {{ return this.gatherableEntries.values(); }}\n  gatheringType(id: Crc32): GatheringTypeData | undefined {{ return this.gatheringTypesById.get(id); }}\n  gatheringTypeByKey(key: string): GatheringTypeData | undefined {{ return this.gatheringType(Crc32.fromStringLower(key)); }}\n  gatheringAction(id: Crc32): GatheringActionData | undefined {{ return this.gatheringActionsById.get(id); }}\n  gatheringActionByKey(key: string): GatheringActionData | undefined {{ return this.gatheringAction(Crc32.fromStringLower(key)); }}\n\n"),
        rows_interface: Some(" implements Rows<GatherableData>".to_owned()),
        row_methods: Some("  rows(): IterableIterator<GatherableData> { return this.gatherableEntries.values(); }\n  [Symbol.iterator](): Iterator<GatherableData> { return this.rows(); }\n\n".to_owned()),
    }
}

fn social() -> TsNativeManagerAugmentation {
    TsNativeManagerAugmentation {
        fields: "  private readonly socialRanksByName = new Map<string, SocialRankData>();\n  private readonly socialRanksBySecurityLevel = new Map<number, SocialRankData>();\n".to_owned(),
        initializers: "    for (const value of this.socialRankDatabaseValue.ranks) { const rank = value.guildRankData; const name = normalizeLookupText(rank.name); if (name.length !== 0 && !this.socialRanksByName.has(name)) this.socialRanksByName.set(name, value); if (!this.socialRanksBySecurityLevel.has(rank.securityLevel)) this.socialRanksBySecurityLevel.set(rank.securityLevel, value); }\n".to_owned(),
        methods: "  rankByName(name: string): SocialRankData | undefined { return this.socialRanksByName.get(normalizeLookupText(name)); }\n  rankBySecurityLevel(level: number): SocialRankData | undefined { return this.socialRanksBySecurityLevel.get(normalizeUnsignedInteger(level)); }\n\n".to_owned(),
        ..TsNativeManagerAugmentation::default()
    }
}

fn player() -> TsNativeManagerAugmentation {
    TsNativeManagerAugmentation { methods: "  hasPlayerBaseAttributes(): boolean { return this.playerBaseAttributesValue !== undefined; }\n  hasSettlementProgressionData(): boolean { return this.settlementProgressionDataValue !== undefined; }\n\n".to_owned(), ..TsNativeManagerAugmentation::default() }
}

fn recipe(
    unit: &GameDataCompileUnit,
    manager: &DirectManagerSurface,
) -> TsNativeManagerAugmentation {
    let row = default_row(unit, manager);
    let id = string_expression(required_field(&row, "RecipeID"), "source.row");
    let category = optional_field(&row, "CraftingCategory")
        .map(|column| string_expression(column, "source.row"))
        .unwrap_or_else(|| "\"\"".to_owned());
    let item = optional_field(&row, "ItemID")
        .map(|column| string_expression(column, "source.row"))
        .unwrap_or_else(|| "\"\"".to_owned());
    let output = optional_field(&row, "OutputQty")
        .map(|column| number_expression(column, "source.row"))
        .unwrap_or_else(|| "1".to_owned());
    let field = ts_direct_row_field_name(&row.source_row_type);
    let table = ts_direct_table_type_name(manager, &row.source_row_type);
    let schema = row.type_name.clone();
    let ingredients = (1..=7u8).filter_map(|slot| optional_field(&row, &format!("Ingredient{slot}")).map(|ingredient| (slot, ingredient))).map(|(slot, ingredient)| {
        let ingredient = string_expression(ingredient, "source.row");
        let quantity = optional_field(&row, &format!("Qty{slot}")).map(|column| number_expression(column, "source.row")).unwrap_or_else(|| "1".to_owned());
        format!("      {{ const ingredientId = Crc32.fromStringLower({ingredient}.trim()); const quantity = normalizeUnsignedInteger({quantity}); if (ingredientId !== Crc32.ZERO && quantity !== 0) data.ingredients.push(Object.freeze({{ ingredientId, quantity }})); }}\n")
    }).collect::<String>();
    TsNativeManagerAugmentation {
        declarations: format!("export interface RecipeIngredient {{ readonly ingredientId: Crc32; readonly quantity: number; }}\nexport interface RecipeData {{ readonly source: RowRef<{table}, {schema}>; readonly recipeKey: string; readonly recipeId: Crc32; readonly craftingCategory: Crc32; readonly itemId: Crc32; readonly outputQuantity: number; readonly ingredients: RecipeIngredient[]; }}\n\n"),
        fields: "  private readonly recipeEntries: RecipeData[] = [];\n  private readonly recipesById = new Map<Crc32, RecipeData>();\n  private readonly recipesByResult = new Map<Crc32, RecipeData[]>();\n  private readonly recipesByCategoryIndex = new Map<Crc32, RecipeData[]>();\n  private readonly craftingStationsByName = new Map<string, CraftingStationData>();\n  private readonly craftingStationsByType = new Map<string, CraftingStationData[]>();\n".to_owned(),
        initializers: format!(r#"    for (const source of this.{field}) {{
      const recipeKey = {id}.trim(); const recipeId = Crc32.fromStringLower(recipeKey); if (recipeKey.length === 0 || recipeId === Crc32.ZERO || this.recipesById.has(recipeId)) continue;
      const data: RecipeData = {{ source: source.ref, recipeKey, recipeId, craftingCategory: Crc32.fromStringLower({category}.trim()), itemId: Crc32.fromStringLower({item}.trim()), outputQuantity: Math.max(1, normalizeUnsignedInteger({output})), ingredients: [] }};
{ingredients}      this.recipeEntries.push(data); this.recipesById.set(recipeId, data); if (data.itemId !== Crc32.ZERO) appendMapValue(this.recipesByResult, data.itemId, data); if (data.craftingCategory !== Crc32.ZERO) appendMapValue(this.recipesByCategoryIndex, data.craftingCategory, data);
    }}
    for (const station of this.craftingStationDatabaseValue.craftingStations) {{ const name = normalizeLookupText(station.name); if (name.length !== 0 && !this.craftingStationsByName.has(name)) this.craftingStationsByName.set(name, station); for (const kind of station.craftingTypes) appendMapValue(this.craftingStationsByType, normalizeLookupText(kind), station); }}
"#),
        methods: "  craftingRecipeData(key: string): RecipeData | undefined { return this.craftingRecipeDataFromId(Crc32.fromStringLower(key)); }\n  craftingRecipeDataFromId(id: Crc32): RecipeData | undefined { return this.recipesById.get(id); }\n  craftingRecipeDataByResult(itemId: Crc32): IterableIterator<RecipeData> { return (this.recipesByResult.get(itemId) ?? []).values(); }\n  recipesByCategory(category: string): IterableIterator<RecipeData> { return this.recipesByCategoryId(Crc32.fromStringLower(category)); }\n  recipesByCategoryId(id: Crc32): IterableIterator<RecipeData> { return (this.recipesByCategoryIndex.get(id) ?? []).values(); }\n  recipes(): IterableIterator<RecipeData> { return this.recipeEntries.values(); }\n  *recipesByIngredients(available: ReadonlyMap<Crc32, number>): IterableIterator<RecipeData> { for (const recipe of this.recipeEntries) if (recipe.ingredients.every((ingredient) => (available.get(ingredient.ingredientId) ?? 0) >= ingredient.quantity)) yield recipe; }\n  craftingStation(name: string): CraftingStationData | undefined { return this.craftingStationsByName.get(normalizeLookupText(name)); }\n  craftingStationsForType(kind: string): IterableIterator<CraftingStationData> { return (this.craftingStationsByType.get(normalizeLookupText(kind)) ?? []).values(); }\n\n".to_owned(),
        rows_interface: Some(" implements Rows<RecipeData>".to_owned()),
        row_methods: Some("  rows(): IterableIterator<RecipeData> { return this.recipeEntries.values(); }\n  [Symbol.iterator](): Iterator<RecipeData> { return this.rows(); }\n\n".to_owned()),
    }
}
