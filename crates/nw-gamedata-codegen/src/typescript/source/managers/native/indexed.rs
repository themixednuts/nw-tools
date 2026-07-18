use super::*;

pub(super) fn augmentation(
    unit: &GameDataCompileUnit,
    manager: &DirectManagerSurface,
    shape: &NativeManagerShape,
) -> TsNativeManagerAugmentation {
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
        NativeManagerShape::OneTableEmote(_) => emote(unit, manager),
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
        NativeManagerShape::OneTableWorldEventRule(_) => world_event_rule(unit, manager),
        NativeManagerShape::QuickCourseData(_) => quick_course(unit, manager),
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
        NativeManagerShape::EntitlementData(_) => entitlement(unit, manager),
        NativeManagerShape::EquipmentSetData(_) => equipment_set(unit, manager),
        NativeManagerShape::OneTablePvpBalance(shape) => pvp_balance(manager, shape),
        NativeManagerShape::OneTableDyeColor(_) => dye_color(unit, manager),
        NativeManagerShape::RewardTrackData(_) => reward_track(unit, manager),
        NativeManagerShape::PostSkillCapProgression(_) => crc_schema_contract(
            unit,
            manager,
            "TradeSkillPostCapData",
            "TradeSkillType",
            &["PostSkillCapProgressionDataFromID"],
            &["PostSkillCapProgressionData"],
            None,
        ),
        NativeManagerShape::WhisperData(_) => whisper(unit, manager),
        NativeManagerShape::OneTableCostumeChange(shape) => costume_change(unit, manager, shape),
        NativeManagerShape::OneTableCrestPart(_) => numeric_contract(
            unit,
            manager,
            "CrestPartData",
            "Index",
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
            None,
        ),
        NativeManagerShape::OneTableLevelDisparity(_) => level_disparity(unit, manager),
        NativeManagerShape::OneTableEncumbrance(_) => crc_schema_contract(
            unit,
            manager,
            "EncumbranceData",
            "ContainerTypeID",
            &["EncumbranceDataFromID"],
            &["EncumbranceData", "EncumbranceDataByKey"],
            None,
        ),
        NativeManagerShape::OneTableDifficultyScaling(_) => crc_schema_contract(
            unit,
            manager,
            "DifficultyScalingData",
            "WorldEncounterID",
            &["DifficultyScalingDataFromID"],
            &["DifficultyScalingData", "DifficultyScalingDataByKey"],
            None,
        ),
        NativeManagerShape::OneTableDarkness(_) => crc_schema_contract(
            unit,
            manager,
            "DarknessData",
            "DarknessId",
            &["DarknessDataByCRC32"],
            &["DarknessData"],
            None,
        ),
        NativeManagerShape::OneTableParticleData(_) => crc_schema_contract(
            unit,
            manager,
            "ParticleData",
            "Effect Name",
            &["ParticleDataFromID"],
            &["ParticleData", "ParticleDataByKey"],
            None,
        ),
        NativeManagerShape::CharacterAttributeData(_) => character_attribute(unit, manager),
        NativeManagerShape::GovernanceData(_) => governance(unit, manager),
        NativeManagerShape::LootBucketData(_) => loot_bucket(unit, manager),
        NativeManagerShape::TerritoryDefinitionsData(_) => territory(unit, manager),
        NativeManagerShape::StatModifierData(_) => stat_modifier(unit, manager),
        _ => panic!(
            "manager {} reached indexed TypeScript native dispatch with {shape:?}",
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
) -> TsNativeManagerAugmentation {
    let mut value = crc_schema_contract(
        unit,
        manager,
        row_type,
        key_column,
        id_methods,
        key_methods,
        None,
    );
    if let Some(method) = rows_method {
        value
            .methods
            .push_str(&named_rows_method(unit, manager, row_type, method));
    }
    value
}

fn world_event_rule(
    unit: &GameDataCompileUnit,
    manager: &DirectManagerSurface,
) -> TsNativeManagerAugmentation {
    let row = required_row(unit, manager, "WorldEventRuleData");
    let field = ts_direct_row_field_name("WorldEventRuleData");
    let id = string_expression(required_field(&row, "RuleID"), "source.row");
    let category = optional_string_expression(&row, &["Category", "EventCategory"], "source.row");
    let hub = optional_string_expression(&row, &["Hub", "HubIDs", "HubId", "Hubs"], "source.row");
    let zone =
        optional_string_expression(&row, &["Zone", "ZoneIDs", "ZoneId", "Zones"], "source.row");
    let tags = optional_string_expression(&row, &["Tags", "EventTags"], "source.row");
    let max_events = optional_schema_number_expression(&row, &["MaxEvents"], "source.row");
    let min_distance = optional_schema_number_expression(&row, &["MinDistance"], "source.row");
    let disabled = optional_boolean_expression(&row, &["Disabled"], "source.row", "false");
    TsNativeManagerAugmentation {
        declarations: r#"export type WorldEventCrcFilter =
  | { readonly wildcard: true; readonly values: readonly [] }
  | { readonly wildcard: false; readonly values: readonly Crc32[] };
export type WorldEventZoneFilter =
  | { readonly wildcard: true; readonly values: readonly [] }
  | { readonly wildcard: false; readonly values: readonly number[] };
export interface WorldEventRuleData {
  readonly sourceRow: number;
  readonly key: string;
  readonly id: Crc32;
  readonly maxEvents: number;
  readonly minDistance: number;
  readonly category: WorldEventCrcFilter;
  readonly hub: WorldEventCrcFilter;
  readonly zone: WorldEventZoneFilter;
  readonly tags: readonly Crc32[];
  readonly enabled: boolean;
}
function worldEventCrcFilter(value: string): WorldEventCrcFilter {
  const normalized = value.trim();
  return normalized === "*"
    ? Object.freeze({ wildcard: true, values: Object.freeze([] as const) })
    : Object.freeze({ wildcard: false, values: splitDesignerList(normalized).map(Crc32.fromStringLower).filter((id) => id !== Crc32.ZERO) });
}
function worldEventZoneFilter(value: string, key: string): WorldEventZoneFilter {
  const normalized = value.trim();
  if (normalized === "*") return Object.freeze({ wildcard: true, values: Object.freeze([] as const) });
  const values = splitDesignerList(normalized).map((token) => {
    const zone = Number(token);
    if (!Number.isInteger(zone) || zone < 0 || zone > 0xffff) throw new Error(`WorldEventRuleData ${key} has invalid Zone ${token}`);
    return zone;
  });
  return Object.freeze({ wildcard: false, values: Object.freeze(values) });
}

"#.to_owned(),
        fields: "  private readonly worldEventRuleEntries: WorldEventRuleData[] = [];\n  private readonly worldEventRulesById = new Map<Crc32, WorldEventRuleData>();\n".to_owned(),
        initializers: format!(r#"    for (const source of this.{field}) {{
      const key = {id}.trim();
      const id = Crc32.fromStringLower(key);
      if (key.length === 0 || id === Crc32.ZERO || this.worldEventRulesById.has(id)) continue;
      const maxEvents = normalizeUint32({max_events} ?? 0);
      const minDistance = {min_distance} ?? 0;
      if (!Number.isFinite(minDistance)) throw new Error(`WorldEventRuleData ${{key}} has invalid MinDistance`);
      const data: WorldEventRuleData = Object.freeze({{ sourceRow: source.slot.rowIndex, key, id, maxEvents, minDistance, category: worldEventCrcFilter({category}), hub: worldEventCrcFilter({hub}), zone: worldEventZoneFilter({zone}, key), tags: Object.freeze(splitDesignerList({tags}).map(Crc32.fromStringLower).filter((value) => value !== Crc32.ZERO)), enabled: !({disabled}) }});
      this.worldEventRuleEntries.push(data);
      this.worldEventRulesById.set(id, data);
    }}
"#),
        methods: "  worldEventRuleByCrc32(id: Crc32): WorldEventRuleData | undefined { return this.worldEventRulesById.get(id); }\n  worldEventRule(key: string): WorldEventRuleData | undefined { return this.worldEventRuleByCrc32(Crc32.fromStringLower(key.trim())); }\n  worldEventRules(): IterableIterator<WorldEventRuleData> { return this.worldEventRuleEntries.values(); }\n\n".to_owned(),
        rows_interface: Some(" implements Rows<WorldEventRuleData>".to_owned()),
        row_methods: Some("  rows(): IterableIterator<WorldEventRuleData> { return this.worldEventRuleEntries.values(); }\n  [Symbol.iterator](): Iterator<WorldEventRuleData> { return this.rows(); }\n\n".to_owned()),
    }
}

fn quick_course(
    unit: &GameDataCompileUnit,
    manager: &DirectManagerSurface,
) -> TsNativeManagerAugmentation {
    let course = required_row(unit, manager, "QuickCourseData");
    let node = required_row(unit, manager, "QuickCourseNodeTypeData");
    let course_field = ts_direct_row_field_name("QuickCourseData");
    let node_field = ts_direct_row_field_name("QuickCourseNodeTypeData");
    let course_id = string_expression(required_field(&course, "QuickCourseID"), "source.row");
    let node_id = string_expression(required_field(&node, "TimedRaceNodeTypeId"), "source.row");
    let path = optional_string_expression(&course, &["PathReferenceQuickCourseID"], "source.row");
    let timed = optional_boolean_expression(&course, &["IsTimed"], "source.row", "false");
    let starting =
        optional_schema_number_expression(&course, &["StartingTimerSeconds"], "source.row");
    let accumulate =
        optional_boolean_expression(&course, &["AccumulateTime"], "source.row", "false");
    let multiplier =
        optional_schema_number_expression(&course, &["NodeTimeOverrideMultiplier"], "source.row");
    let audio = optional_string_expression(&course, &["AudioGroup"], "source.row");
    let radius = optional_schema_number_expression(&node, &["DetectionRadius"], "source.row");
    let use_override =
        optional_boolean_expression(&node, &["UseTimeOverride"], "source.row", "false");
    let add_time = optional_schema_number_expression(&node, &["AddTimeSeconds"], "source.row");
    let visual = optional_string_expression(&node, &["VisualSlicePath"], "source.row");
    let sfx = optional_string_expression(&node, &["SFX"], "source.row");
    TsNativeManagerAugmentation {
        declarations: r#"export interface QuickCourseData { readonly id: string; readonly idCrc: Crc32; readonly pathReferenceId: Crc32; readonly isTimed: boolean; readonly startingTimerSeconds: number; readonly accumulateTime: boolean; readonly nodeTimeOverrideMultiplier: number; readonly audioGroup: string; }
export interface QuickCourseNodeTypeData { readonly id: string; readonly idCrc: Crc32; readonly detectionRadius: number; readonly useTimeOverride: boolean; readonly addTimeSeconds: number; readonly visualSlicePath: string; readonly sfx: string; }

"#.to_owned(),
        fields: "  private readonly quickCourseEntries: QuickCourseData[] = [];\n  private readonly quickCoursesById = new Map<Crc32, QuickCourseData>();\n  private readonly quickCourseIds: string[] = [];\n  private readonly quickCourseNodeTypeEntries: QuickCourseNodeTypeData[] = [];\n  private readonly quickCourseNodeTypesById = new Map<Crc32, QuickCourseNodeTypeData>();\n  private readonly quickCourseNodeTypeIds: string[] = [];\n".to_owned(),
        initializers: format!(r#"    for (const source of this.{course_field}) {{
      const id = {course_id}.trim(); const idCrc = Crc32.fromStringLower(id); if (id.length === 0 || idCrc === Crc32.ZERO) continue;
      const startingTimerSeconds = normalizeUint32({starting} ?? 0);
      let nodeTimeOverrideMultiplier = {multiplier} ?? 1; if (!Number.isFinite(nodeTimeOverrideMultiplier)) throw new Error(`QuickCourseData ${{id}} has invalid NodeTimeOverrideMultiplier`); if (nodeTimeOverrideMultiplier === 0) nodeTimeOverrideMultiplier = 1;
      const data = Object.freeze({{ id, idCrc, pathReferenceId: Crc32.fromStringLower({path}.trim()), isTimed: {timed}, startingTimerSeconds, accumulateTime: {accumulate}, nodeTimeOverrideMultiplier, audioGroup: {audio}.trim() }});
      this.quickCourseIds.push(id); const previous = this.quickCoursesById.get(idCrc); if (previous === undefined) this.quickCourseEntries.push(data); else this.quickCourseEntries[this.quickCourseEntries.indexOf(previous)] = data; this.quickCoursesById.set(idCrc, data);
    }}
    for (const source of this.{node_field}) {{
      const id = {node_id}.trim(); const idCrc = Crc32.fromStringLower(id); if (id.length === 0 || idCrc === Crc32.ZERO) continue;
      const detectionRadius = {radius} ?? 0; const addTimeSeconds = {add_time} ?? 0; if (!Number.isFinite(detectionRadius) || !Number.isFinite(addTimeSeconds)) throw new Error(`QuickCourseNodeTypeData ${{id}} has invalid numeric data`);
      const data = Object.freeze({{ id, idCrc, detectionRadius, useTimeOverride: {use_override}, addTimeSeconds, visualSlicePath: {visual}.trim().toLowerCase(), sfx: {sfx}.trim() }});
      this.quickCourseNodeTypeIds.push(id); const previous = this.quickCourseNodeTypesById.get(idCrc); if (previous === undefined) this.quickCourseNodeTypeEntries.push(data); else this.quickCourseNodeTypeEntries[this.quickCourseNodeTypeEntries.indexOf(previous)] = data; this.quickCourseNodeTypesById.set(idCrc, data);
    }}
"#),
        methods: "  quickCourseByCrc32(id: Crc32): QuickCourseData | undefined { return this.quickCoursesById.get(id); }\n  quickCourse(key: string): QuickCourseData | undefined { return this.quickCourseByCrc32(Crc32.fromStringLower(key.trim())); }\n  nodeTypeByCrc32(id: Crc32): QuickCourseNodeTypeData | undefined { return this.quickCourseNodeTypesById.get(id); }\n  nodeType(key: string): QuickCourseNodeTypeData | undefined { return this.nodeTypeByCrc32(Crc32.fromStringLower(key.trim())); }\n  quickCourses(): IterableIterator<QuickCourseData> { return this.quickCourseEntries.values(); }\n  nodeTypes(): IterableIterator<QuickCourseNodeTypeData> { return this.quickCourseNodeTypeEntries.values(); }\n  firstQuickCourseId(): string | undefined { return this.quickCourseIds[0]; }\n  firstNodeTypeId(): string | undefined { return this.quickCourseNodeTypeIds[0]; }\n\n".to_owned(),
        rows_interface: Some(" implements Rows<QuickCourseData>".to_owned()),
        row_methods: Some("  rows(): IterableIterator<QuickCourseData> { return this.quickCourseEntries.values(); }\n  [Symbol.iterator](): Iterator<QuickCourseData> { return this.rows(); }\n\n".to_owned()),
    }
}

fn emote(
    unit: &GameDataCompileUnit,
    manager: &DirectManagerSurface,
) -> TsNativeManagerAugmentation {
    let mut value = simple_crc(
        unit,
        manager,
        "EmoteData",
        "UniqueTagID",
        &["EmoteDataFromID"],
        &["EmoteData", "EmoteDataByKey"],
        Some("Emotes"),
    );
    let row = required_row(unit, manager, "EmoteData");
    let Some(status) = optional_field(&row, "StatusEffectTimer") else {
        return value;
    };
    let id = required_field(&row, "UniqueTagID");
    let field = ts_direct_row_field_name("EmoteData");
    let status = string_expression(status, "source.row");
    let id = string_expression(id, "source.row");
    value
        .fields
        .push_str("  private readonly emoteIdByStatusEffectIndex = new Map<Crc32, Crc32>();\n");
    value.initializers.push_str(&format!(r#"    for (const source of this.{field}) {{
      const statusId = Crc32.fromStringLower({status}.trim());
      const emoteId = Crc32.fromStringLower({id}.trim());
      if (statusId !== Crc32.ZERO && emoteId !== Crc32.ZERO && !this.emoteIdByStatusEffectIndex.has(statusId)) this.emoteIdByStatusEffectIndex.set(statusId, emoteId);
    }}
"#));
    value.methods.push_str("  emoteIdByStatusEffect(statusEffectId: Crc32): Crc32 | undefined { return this.emoteIdByStatusEffectIndex.get(statusEffectId); }\n\n  emoteIdForStatusEffect(key: string): Crc32 | undefined { return this.emoteIdByStatusEffect(Crc32.fromStringLower(key)); }\n\n");
    value
}

fn dye_color(
    unit: &GameDataCompileUnit,
    manager: &DirectManagerSurface,
) -> TsNativeManagerAugmentation {
    let row = required_row(unit, manager, "DyeColorData");
    let field = ts_direct_row_field_name("DyeColorData");
    let index = number_expression(required_field(&row, "Index"), "source.row");
    let name = optional_string_expression(&row, &["Name"], "source.row");
    let color = optional_string_expression(&row, &["Color"], "source.row");
    let category = optional_string_expression(&row, &["Category"], "source.row");
    let entitlement = optional_boolean_expression(&row, &["IsEntitlement"], "source.row", "false");
    let color_amount = optional_schema_number_expression(&row, &["ColorAmount"], "source.row");
    let color_override = optional_schema_number_expression(&row, &["ColorOverride"], "source.row");
    let spec_color = optional_string_expression(&row, &["SpecColor"], "source.row");
    let spec_amount = optional_schema_number_expression(&row, &["SpecAmount"], "source.row");
    let mask_gloss = optional_schema_number_expression(&row, &["MaskGlossShift"], "source.row");
    TsNativeManagerAugmentation {
        declarations: r#"export type DyeColorIndex = number;
export interface DyeColorRgba { readonly red: number; readonly green: number; readonly blue: number; readonly alpha: number; }
export interface DyeColorData { readonly index: DyeColorIndex; readonly name: string; readonly color: DyeColorRgba; readonly category: string; readonly isEntitlement: boolean; readonly colorAmount: number; readonly colorOverride: number; readonly specColor: DyeColorRgba; readonly specAmount: number; readonly maskGlossShift: number; }
function dyeColorIndex(value: number): DyeColorIndex { const index = normalizeUint8(value); if (index === 0) throw new RangeError("DyeColorData Index must be non-zero"); return index; }
function dyeColorRgba(value: string, index: DyeColorIndex, field: string): DyeColorRgba {
  const source = value.trim().replace(/^#/, "");
  if (source.length === 0 || source.length > 8 || !/^[0-9a-f]+$/i.test(source)) throw new Error(`DyeColorData index ${index} has invalid ${field} ${value}`);
  const raw = Number.parseInt(source.padEnd(8, "F"), 16);
  const channel = (shift: number): number => ((raw >>> shift) & 0xff) / 255;
  return Object.freeze({ red: channel(24), green: channel(16), blue: channel(8), alpha: channel(0) });
}
"#.to_owned(),
        fields: "  private readonly dyeColorEntries: DyeColorData[] = [];\n  private readonly dyeColorsByIndex = new Map<DyeColorIndex, DyeColorData>();\n  private readonly dyeEntitlementIndexes: DyeColorIndex[] = [];\n".to_owned(),
        initializers: format!(r#"    for (const source of this.{field}) {{
      const index = dyeColorIndex({index}); if (this.dyeColorsByIndex.has(index)) continue;
      const colorText = {color}.trim(); if (colorText.length === 0) continue;
      const color = dyeColorRgba(colorText, index, "Color"); const specText = {spec_color}.trim();
      const data: DyeColorData = Object.freeze({{ index, name: {name}.trim(), color, category: {category}.trim(), isEntitlement: {entitlement}, colorAmount: {color_amount} ?? 0, colorOverride: {color_override} ?? 0, specColor: specText.length === 0 ? color : dyeColorRgba(specText, index, "SpecColor"), specAmount: {spec_amount} ?? 0, maskGlossShift: {mask_gloss} ?? 0 }});
      this.dyeColorEntries.push(data); this.dyeColorsByIndex.set(index, data); if (data.isEntitlement) this.dyeEntitlementIndexes.push(index);
    }}
    this.dyeEntitlementIndexes.sort((left, right) => left - right);
"#),
        methods: "  dyeColorData(index: DyeColorIndex): DyeColorData | undefined { return this.dyeColorsByIndex.get(dyeColorIndex(index)); }\n  dyeColorDataFromIndex(index: number): DyeColorData | undefined { return this.dyeColorData(dyeColorIndex(index)); }\n  dyeColorDataByKey(index: DyeColorIndex): DyeColorData | undefined { return this.dyeColorData(index); }\n  entitlementIndexes(): IterableIterator<DyeColorIndex> { return this.dyeEntitlementIndexes.values(); }\n  dyeColors(): IterableIterator<DyeColorData> { return this.dyeColorEntries.values(); }\n\n".to_owned(),
        rows_interface: Some(" implements Rows<DyeColorData>".to_owned()),
        row_methods: Some("  rows(): IterableIterator<DyeColorData> { return this.dyeColorEntries.values(); }\n  [Symbol.iterator](): Iterator<DyeColorData> { return this.rows(); }\n\n".to_owned()),
    }
}

fn costume_change(
    unit: &GameDataCompileUnit,
    manager: &DirectManagerSurface,
    shape: &NativeOneTableCostumeChangeManager,
) -> TsNativeManagerAugmentation {
    let row = required_row(unit, manager, shape.row_type_name().as_str());
    let field = ts_direct_row_field_name(shape.row_type_name().as_str());
    let key = string_expression(
        required_field(&row, shape.key_column().as_str()),
        "source.row",
    );
    let mesh = string_expression(
        required_field(&row, shape.mesh_column().as_str()),
        "source.row",
    );
    let skeleton = required_field(&row, shape.matches_skeleton_column().as_str());
    let skeleton = format!("schemaBoolean(source.row.{}, false)", skeleton.field_name);
    let offset = number_expression(
        required_field(&row, shape.z_offset_column().as_str()),
        "source.row",
    );
    let slot_type = shape
        .slots()
        .iter()
        .map(|slot| format!("{:?}", slot.display().as_str()))
        .collect::<Vec<_>>()
        .join(" | ");
    let slots = shape.slots().iter().map(|slot| {
        let left = optional_string_expression(&row, &[slot.left_column().as_str()], "source.row");
        let right = optional_string_expression(&row, &[slot.right_column().as_str()], "source.row");
        format!("      audioOverrides.set({:?}, Object.freeze({{ left: Crc32.fromStringLower({left}.trim()), right: Crc32.fromStringLower({right}.trim()) }}));\n", slot.display().as_str())
    }).collect::<String>();
    let from_id = ts_method_name(shape.lookup_from_id_method().as_str());
    let lookup = ts_method_name(shape.lookup_method().as_str());
    let by_key = ts_method_name(shape.lookup_by_key_method().as_str());
    let audio_from_id = ts_method_name(shape.audio_override_from_id_method().as_str());
    let audio = ts_method_name(shape.audio_override_method().as_str());
    let len = ts_method_name(shape.len_method().as_str());
    let is_empty = ts_method_name(shape.is_empty_method().as_str());
    TsNativeManagerAugmentation {
        declarations: format!("export type CostumeAudioSlot = {slot_type};\nexport interface CostumeAudioOverride {{ readonly left: Crc32; readonly right: Crc32; }}\nexport interface CostumeChangeData {{ readonly sourceRow: number; readonly id: Crc32; readonly key: string; readonly mesh: string; readonly matchesPlayerSkeleton: boolean; readonly meshRenderZPosOffset: number; readonly audioOverrides: ReadonlyMap<CostumeAudioSlot, CostumeAudioOverride>; }}\n\n"),
        fields: "  private readonly costumeChangeEntries: CostumeChangeData[] = [];\n  private readonly costumeChangesById = new Map<Crc32, CostumeChangeData>();\n  private readonly costumeChangesBySourceRow = new Map<number, CostumeChangeData>();\n".to_owned(),
        initializers: format!(r#"    for (const source of this.{field}) {{
      const key = {key}.trim(); const id = Crc32.fromStringLower(key); if (key.length === 0 || id === Crc32.ZERO || this.costumeChangesById.has(id)) continue;
      const audioOverrides = new Map<CostumeAudioSlot, CostumeAudioOverride>();
{slots}      const meshRenderZPosOffset = {offset}; if (!Number.isFinite(meshRenderZPosOffset)) throw new Error(`CostumeChangeData ${{key}} has invalid MeshRenderZPosOffset`);
      const data = Object.freeze({{ sourceRow: source.slot.rowIndex, id, key, mesh: {mesh}.trim(), matchesPlayerSkeleton: {skeleton}, meshRenderZPosOffset, audioOverrides }});
      this.costumeChangeEntries.push(data); this.costumeChangesById.set(id, data); this.costumeChangesBySourceRow.set(source.slot.rowIndex, data);
    }}
"#),
        methods: format!("  {from_id}(id: Crc32): CostumeChangeData | undefined {{ return this.costumeChangesById.get(id); }}\n  {lookup}(key: string): CostumeChangeData | undefined {{ return this.{from_id}(Crc32.fromStringLower(key.trim())); }}\n  {by_key}(key: string): CostumeChangeData | undefined {{ return this.{lookup}(key); }}\n  {audio_from_id}(id: Crc32, slot: CostumeAudioSlot): CostumeAudioOverride | undefined {{ return this.{from_id}(id)?.audioOverrides.get(slot); }}\n  {audio}(key: string, slot: CostumeAudioSlot): CostumeAudioOverride | undefined {{ return this.{audio_from_id}(Crc32.fromStringLower(key.trim()), slot); }}\n  costumeChangeForSourceRow(rowIndex: number): CostumeChangeData | undefined {{ return this.costumeChangesBySourceRow.get(normalizeUnsignedInteger(rowIndex)); }}\n  {len}(): number {{ return this.costumeChangeEntries.length; }}\n  {is_empty}(): boolean {{ return this.costumeChangeEntries.length === 0; }}\n\n"),
        rows_interface: Some(" implements Rows<CostumeChangeData>".to_owned()),
        row_methods: Some("  rows(): IterableIterator<CostumeChangeData> { return this.costumeChangeEntries.values(); }\n  [Symbol.iterator](): Iterator<CostumeChangeData> { return this.rows(); }\n\n".to_owned()),
    }
}

fn whisper(
    unit: &GameDataCompileUnit,
    manager: &DirectManagerSurface,
) -> TsNativeManagerAugmentation {
    merge_augmentations([
        crc_schema_contract(
            unit,
            manager,
            "WhisperData",
            "WhisperId",
            &["WhisperDataFromID"],
            &["WhisperData", "WhisperDataByKey"],
            None,
        ),
        crc_secondary_contract(
            unit,
            manager,
            "WhisperVfxData",
            "WhisperVfxId",
            "WhisperVfxFromID",
            "WhisperVfx",
            "whisperVfxById",
        ),
    ])
}

fn entitlement(
    unit: &GameDataCompileUnit,
    manager: &DirectManagerSurface,
) -> TsNativeManagerAugmentation {
    let mut value = simple_crc(
        unit,
        manager,
        "EntitlementData",
        "UniqueTagID",
        &["ByID"],
        &["ByKey"],
        Some("Entitlements"),
    );
    let row = required_row(unit, manager, "EntitlementData");
    let index = required_field(&row, "EntitlementIndex");
    let rewards = required_field(&row, "Reward(s)");
    let field = ts_direct_row_field_name("EntitlementData");
    let table = ts_direct_table_type_name(manager, "EntitlementData");
    let index = number_expression(index, "source.row");
    let rewards = string_expression(rewards, "source.row");
    let schema = row.type_name.clone();
    value.fields.push_str(&format!("  private readonly entitlementsByIndex = new Map<number, RowEntry<{table}, {schema}>>();\n  private readonly entitlementsByReward = new Map<Crc32, RowEntry<{table}, {schema}>[]>();\n"));
    value.initializers.push_str(&format!(r#"    for (const source of this.{field}) {{
      this.entitlementsByIndex.set(normalizeUnsignedInteger({index}), source);
      for (const reward of splitDesignerList({rewards})) appendMapValue(this.entitlementsByReward, Crc32.fromStringLower(reward), source);
    }}
"#));
    value.methods.push_str(&format!("  byIndex(index: number): {schema} | undefined {{ return this.entitlementsByIndex.get(normalizeUnsignedInteger(index))?.row; }}\n\n  *entitlementsForReward(reward: Crc32): IterableIterator<{schema}> {{ for (const source of this.entitlementsByReward.get(reward) ?? []) yield source.row; }}\n\n"));
    value
}

fn equipment_set(
    unit: &GameDataCompileUnit,
    manager: &DirectManagerSurface,
) -> TsNativeManagerAugmentation {
    let mut value = simple_crc(
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
        .collect::<Vec<_>>();
    let field = ts_direct_row_field_name("EquipmentSetData");
    let table = ts_direct_table_type_name(manager, "EquipmentSetData");
    let items = string_expression(items, "source.row");
    let perk_indexes = perks.iter().map(|perk| {
        let expression = string_expression(perk, "source.row");
        format!("      {{ const id = Crc32.fromStringLower({expression}.trim()); if (id !== Crc32.ZERO) appendMapValue(this.equipmentSetsByPerk, id, source); }}\n")
    }).collect::<String>();
    let schema = row.type_name.clone();
    value.fields.push_str(&format!("  private readonly equipmentSetsByItem = new Map<Crc32, RowEntry<{table}, {schema}>[]>();\n  private readonly equipmentSetsByPerk = new Map<Crc32, RowEntry<{table}, {schema}>[]>();\n"));
    value.initializers.push_str(&format!(r#"    for (const source of this.{field}) {{
      for (const item of splitDesignerList({items})) appendMapValue(this.equipmentSetsByItem, Crc32.fromStringLower(item), source);
{perk_indexes}    }}
"#));
    value.methods.push_str(&format!("  *setsForItem(item: Crc32): IterableIterator<{schema}> {{ for (const source of this.equipmentSetsByItem.get(item) ?? []) yield source.row; }}\n\n  *setsForPerk(perk: Crc32): IterableIterator<{schema}> {{ for (const source of this.equipmentSetsByPerk.get(perk) ?? []) yield source.row; }}\n\n"));
    value
}

fn character_attribute(
    unit: &GameDataCompileUnit,
    manager: &DirectManagerSurface,
) -> TsNativeManagerAugmentation {
    let row = required_row(unit, manager, "AttributeDefinition");
    let level = required_field(&row, "Level");
    let field = ts_direct_row_field_name("AttributeDefinition");
    let table = ts_direct_table_type_name(manager, "AttributeDefinition");
    let level = number_expression(level, "source.row");
    let schema = row.type_name.clone();
    TsNativeManagerAugmentation {
        fields: format!(
            "  private readonly attributes = new Map<string, RowEntry<{table}, {schema}>>();\n  private readonly attributeLevels = new Map<{table}, number[]>();\n"
        ),
        initializers: format!(
            r#"    for (const source of this.{field}) {{
      const level = normalizeUnsignedInteger({level});
      const key = tableNumberLookupKey(source.ref.table, level);
      if (!this.attributes.has(key)) this.attributes.set(key, source);
      appendMapValue(this.attributeLevels, source.ref.table, level);
    }}
    for (const levels of this.attributeLevels.values()) levels.sort((left, right) => left - right);
"#
        ),
        methods: format!(
            r#"  attributeData(table: {table}, level: number): {schema} | undefined {{ return this.attributes.get(tableNumberLookupKey(table, normalizeUnsignedInteger(level)))?.row; }}

  clampedLevel(table: {table}, level: number): number | undefined {{ return floorInSorted(this.attributeLevels.get(table) ?? [], normalizeUnsignedInteger(level)); }}

  clampedAttributeData(table: {table}, level: number): {schema} | undefined {{ const clamped = this.clampedLevel(table, level); return clamped === undefined ? undefined : this.attributeData(table, clamped); }}

"#
        ),
        ..TsNativeManagerAugmentation::default()
    }
}

fn level_disparity(
    unit: &GameDataCompileUnit,
    manager: &DirectManagerSurface,
) -> TsNativeManagerAugmentation {
    let mut value = numeric_contract_with_normalizer(
        unit,
        manager,
        "LevelDisparityData",
        "LevelDisparity",
        &["LevelDisparityData"],
        Some("LevelDisparityRows"),
        "normalizeInt32",
    );
    let row = required_row(unit, manager, "LevelDisparityData");
    let field = ts_direct_row_field_name("LevelDisparityData");
    let disparity = number_expression(required_field(&row, "LevelDisparity"), "source.row");
    value.fields.push_str("  private levelDisparityMin: number | undefined;\n  private levelDisparityMax: number | undefined;\n");
    value.initializers.push_str(&format!(r#"    for (const source of this.{field}) {{ const value = Math.trunc({disparity}); this.levelDisparityMin = this.levelDisparityMin === undefined ? value : Math.min(this.levelDisparityMin, value); this.levelDisparityMax = this.levelDisparityMax === undefined ? value : Math.max(this.levelDisparityMax, value); }}
"#));
    let schema = row.type_name.clone();
    value.methods.push_str(&format!(r#"  levelDisparityDataForLevels(playerLevel: number, targetLevel: number): {schema} | undefined {{ return this.levelDisparityData(Math.trunc(targetLevel) - Math.trunc(playerLevel)); }}
  clampedDisparity(disparity: number): number | undefined {{ if (this.levelDisparityMin === undefined || this.levelDisparityMax === undefined) return undefined; return Math.max(this.levelDisparityMin, Math.min(this.levelDisparityMax, Math.trunc(disparity))); }}
  clampedLevelDisparityDataForLevels(playerLevel: number, targetLevel: number): {schema} | undefined {{ const value = this.clampedDisparity(targetLevel - playerLevel); return value === undefined ? undefined : this.levelDisparityData(value); }}
  levelDisparityDataForLevelsWithPlayerLevelCap(playerLevel: number, targetLevel: number, maxPlayerLevel: number): {schema} | undefined {{ return playerLevel > maxPlayerLevel ? undefined : this.levelDisparityDataForLevels(playerLevel, targetLevel); }}
  clampedLevelDisparityDataForLevelsWithPlayerLevelCap(playerLevel: number, targetLevel: number, maxPlayerLevel: number): {schema} | undefined {{ return playerLevel > maxPlayerLevel ? undefined : this.clampedLevelDisparityDataForLevels(playerLevel, targetLevel); }}
  loadedRange(): readonly [number, number] | undefined {{ return this.levelDisparityMin === undefined || this.levelDisparityMax === undefined ? undefined : [this.levelDisparityMin, this.levelDisparityMax]; }}

"#));
    value
}

fn territory(
    unit: &GameDataCompileUnit,
    manager: &DirectManagerSurface,
) -> TsNativeManagerAugmentation {
    let mut value = numeric_contract(
        unit,
        manager,
        "TerritoryDefinition",
        "TerritoryID",
        &["ByID"],
        Some("Territories"),
    );
    let row = required_row(unit, manager, "TerritoryDefinition");
    let field = ts_direct_row_field_name("TerritoryDefinition");
    let table = ts_direct_table_type_name(manager, "TerritoryDefinition");
    let territory_id = number_expression(required_field(&row, "TerritoryID"), "source.row");
    let achievement =
        optional_field(&row, "Achievements").map(|f| string_expression(f, "source.row"));
    let tags = ["POITags", "LootTags"]
        .into_iter()
        .filter_map(|name| optional_field(&row, name))
        .map(|f| string_expression(f, "source.row"))
        .collect::<Vec<_>>();
    let achievement_init = achievement.map(|expr| format!("      for (const key of splitDesignerList({expr})) {{ const id = Crc32.fromStringLower(key); if (!this.territoriesByAchievement.has(id)) this.territoriesByAchievement.set(id, source); }}\n")).unwrap_or_default();
    let tags_init = tags.into_iter().map(|expr| format!("      for (const key of splitDesignerList({expr})) appendMapValue(this.territoriesByTag, Crc32.fromStringLower(key), source);\n")).collect::<String>();
    let schema = row.type_name.clone();
    value.fields.push_str(&format!("  private readonly territoriesByLabel = new Map<Crc32, RowEntry<{table}, {schema}>>();\n  private readonly territoriesByAchievement = new Map<Crc32, RowEntry<{table}, {schema}>>();\n  private readonly territoriesByTag = new Map<Crc32, RowEntry<{table}, {schema}>[]>();\n"));
    value.initializers.push_str(&format!(
        r#"    for (const source of this.{field}) {{
      const id = normalizeUnsignedInteger({territory_id});
      this.territoriesByLabel.set(Crc32.fromStringLower(`Territory_${{id}}`), source);
{achievement_init}{tags_init}    }}
"#
    ));
    value.methods.push_str(&format!("  byLabel(label: string): {schema} | undefined {{ return this.territoriesByLabel.get(Crc32.fromStringLower(label))?.row; }}\n  territoryForAchievement(id: Crc32): {schema} | undefined {{ return this.territoriesByAchievement.get(id)?.row; }}\n  *territoriesWithTag(id: Crc32): IterableIterator<{schema}> {{ for (const source of this.territoriesByTag.get(id) ?? []) yield source.row; }}\n\n"));
    value
}

fn governance(
    unit: &GameDataCompileUnit,
    manager: &DirectManagerSurface,
) -> TsNativeManagerAugmentation {
    let mut value = numeric_contract(
        unit,
        manager,
        "TerritoryUpkeepDefinition",
        "Level",
        &["Governance"],
        Some("GovernanceRows"),
    );
    let row = required_row(unit, manager, "TerritoryUpkeepDefinition");
    let field = ts_direct_row_field_name("TerritoryUpkeepDefinition");
    let distributions = row
        .fields
        .iter()
        .filter_map(|column| {
            let id = column
                .source_name
                .strip_prefix("EarningsDistributionTID")?
                .parse::<u32>()
                .ok()?;
            Some((id, number_expression(column, "source.row")))
        })
        .collect::<Vec<_>>();
    let initializers = distributions.iter().map(|(id, expr)| format!("      appendMapValue(this.governanceDistribution, level, Object.freeze({{ territoryId: {id}, share: {expr} }}));\n      this.maxTerritoryIdValue = Math.max(this.maxTerritoryIdValue, {id});\n")).collect::<String>();
    value.declarations.push_str("export interface TerritoryEarningsDistribution { readonly territoryId: number; readonly share: number; }\n\n");
    value.fields.push_str("  private readonly governanceDistribution = new Map<number, TerritoryEarningsDistribution[]>();\n  private maxTerritoryIdValue = 0;\n");
    value.initializers.push_str(&format!(
        r#"    for (const source of this.{field}) {{
      const level = normalizeUnsignedInteger(source.row.level);
{initializers}    }}
"#
    ));
    value.methods.push_str("  territoryEarningsDistribution(level: number): IterableIterator<TerritoryEarningsDistribution> { return (this.governanceDistribution.get(normalizeUnsignedInteger(level)) ?? []).values(); }\n  maxTerritoryId(): number { return this.maxTerritoryIdValue; }\n\n");
    value
}

fn loot_bucket(
    unit: &GameDataCompileUnit,
    manager: &DirectManagerSurface,
) -> TsNativeManagerAugmentation {
    required_row(unit, manager, "LootBucketData");
    let field = ts_direct_row_field_name("LootBucketData");
    let table = ts_direct_table_type_name(manager, "LootBucketData");
    TsNativeManagerAugmentation {
        declarations: format!(r#"export interface LootBucketSlot {{ readonly table: {table}; readonly slot: number; }}
export interface LootBucketTag {{ readonly key: string; readonly id: Crc32; readonly range: readonly [number, number] | null; }}
export interface LootBucketEntry {{ readonly rowIndex: number; readonly itemKey: string; readonly itemId: Crc32; readonly tags: readonly LootBucketTag[]; readonly matchOne: boolean; readonly quantity: readonly [number, number]; readonly odds: number; }}
export interface LootBucketData {{ readonly table: {table}; readonly slot: number; readonly key: string; readonly id: Crc32; readonly lootBiasingDisabled: boolean; readonly entries: LootBucketEntry[]; }}
function lootBucketNumber(value: string): number {{ const integer = Number.parseInt(value.trim(), 10); if (Number.isInteger(integer) && integer >= 0 && integer <= 0xffff) return integer; const float = Number.parseFloat(value.trim()); return Number.isFinite(float) && float >= 0 && float <= 0xffff ? Math.trunc(float) : 0; }}
function lootBucketRange(value: string | null, singleMaximum: number | null): readonly [number, number] {{ const normalized = value?.trim() ?? ""; if (normalized.length === 0) return Object.freeze([0, 0]); const [left, right] = normalized.split("-", 2); const start = lootBucketNumber(left); const end = right === undefined ? singleMaximum ?? start : lootBucketNumber(right); return Object.freeze([Math.min(start, end), Math.max(start, end)]); }}
function lootBucketTags(value: string | null): readonly LootBucketTag[] {{ return Object.freeze((value ?? "").split(",").map((token) => token.trim()).filter((token) => token.length !== 0).flatMap((token) => {{ const separator = token.indexOf(":"); const key = (separator < 0 ? token : token.slice(0, separator)).trim(); const id = Crc32.fromStringLower(key); return key.length === 0 || id === Crc32.ZERO ? [] : [Object.freeze({{ key, id, range: separator < 0 ? null : lootBucketRange(token.slice(separator + 1), 10_000) }})]; }})); }}
function lootBucketOdds(value: string | null): number {{ const odds = Number.parseFloat(value?.trim() ?? ""); return Number.isFinite(odds) ? odds : 1; }}

"#),
        fields: "  private readonly lootBuckets: LootBucketData[] = [];\n  private readonly lootBucketsById = new Map<Crc32, LootBucketData>();\n  private readonly lootBucketsBySlot = new Map<string, LootBucketData>();\n".to_owned(),
        initializers: format!(r#"    for (const source of this.{field}) {{
      for (const entry of source.row.entries) {{
        const slotKey = tableNumberLookupKey(source.ref.table, entry.slot);
        let bucket = this.lootBucketsBySlot.get(slotKey);
        if (source.slot.rowIndex === 0) {{
          const key = entry.lootBucket?.trim() ?? "";
          const id = Crc32.fromStringLower(key);
          if (key.length !== 0 && id !== Crc32.ZERO) {{
            const lootBiasingDisabled = source.row.lootBiasingDisabled.find((value) => value.slot === entry.slot)?.disabled ?? false;
            const data: LootBucketData = {{ table: source.ref.table, slot: entry.slot, key, id, lootBiasingDisabled, entries: [] }};
            const duplicate = this.lootBucketsById.get(id);
            if (duplicate === undefined) {{ bucket = data; this.lootBuckets.push(data); this.lootBucketsById.set(id, data); }}
            else {{ const index = this.lootBuckets.indexOf(duplicate); this.lootBucketsBySlot.delete(tableNumberLookupKey(duplicate.table, duplicate.slot)); this.lootBuckets[index] = data; this.lootBucketsById.set(id, data); bucket = data; }}
            this.lootBucketsBySlot.set(slotKey, data);
          }}
        }}
        if (bucket === undefined) continue;
        for (const itemKey of splitDesignerList(entry.item ?? "")) {{ const itemId = Crc32.fromStringLower(itemKey); if (itemId !== Crc32.ZERO) bucket.entries.push(Object.freeze({{ rowIndex: source.slot.rowIndex, itemKey, itemId, tags: lootBucketTags(entry.tags), matchOne: schemaBoolean(entry.matchOne, false), quantity: lootBucketRange(entry.quantity, null), odds: lootBucketOdds(entry.odds) }})); }}
      }}
    }}
"#),
        methods: "  byId(id: Crc32): LootBucketData | undefined {{ return this.lootBucketsById.get(id); }}\n  byKey(key: string): LootBucketData | undefined {{ return this.byId(Crc32.fromStringLower(key.trim())); }}\n  bucketForSlot(slot: LootBucketSlot): LootBucketData | undefined {{ return this.lootBucketsBySlot.get(tableNumberLookupKey(slot.table, normalizeUint16(slot.slot))); }}\n  buckets(): IterableIterator<LootBucketData> {{ return this.lootBuckets.values(); }}\n\n".to_owned(),
        rows_interface: Some(" implements Rows<LootBucketData>".to_owned()),
        row_methods: Some("  rows(): IterableIterator<LootBucketData> { return this.lootBuckets.values(); }\n  [Symbol.iterator](): Iterator<LootBucketData> { return this.rows(); }\n\n".to_owned()),
    }
}

fn reward_track(
    unit: &GameDataCompileUnit,
    manager: &DirectManagerSurface,
) -> TsNativeManagerAugmentation {
    let row = required_row(unit, manager, "PvPStoreData");
    let field = ts_direct_row_field_name("PvPStoreData");
    let table = ts_direct_table_type_name(manager, "PvPStoreData");
    let mut slots = std::collections::BTreeMap::<u16, RewardTrackFields>::new();
    for column in &row.fields {
        if let Some(slot) = numbered_suffix(&column.source_name, "Bucket") {
            slots.entry(slot).or_default().bucket = Some(column.field_name.clone());
        }
        if let Some(slot) = numbered_suffix(&column.source_name, "RewardID") {
            slots.entry(slot).or_default().reward = Some(column.field_name.clone());
        }
        if let Some(slot) = numbered_suffix(&column.source_name, "Tag") {
            slots.entry(slot).or_default().tags = Some(column.field_name.clone());
        }
        if let Some(slot) = numbered_suffix(&column.source_name, "MatchOne") {
            slots.entry(slot).or_default().match_one = Some(column.field_name.clone());
        }
        if let Some(slot) = numbered_suffix(&column.source_name, "RandomWeights") {
            slots.entry(slot).or_default().random_weight = Some(column.field_name.clone());
        }
        if let Some(slot) = numbered_suffix(&column.source_name, "BudgetContribution") {
            slots.entry(slot).or_default().budget = Some(column.field_name.clone());
        }
        if let Some(slot) = numbered_suffix(&column.source_name, "Type") {
            slots.entry(slot).or_default().reward_type = Some(column.field_name.clone());
        }
        if let Some(slot) = numbered_suffix(&column.source_name, "ExcludeTypeStage") {
            slots.entry(slot).or_default().stage_exclusion = Some(column.field_name.clone());
        }
        if let Some(slot) = numbered_suffix(&column.source_name, "ExcludeTypeShop") {
            slots.entry(slot).or_default().shop_exclusion = Some(column.field_name.clone());
        }
    }
    let init = slots.into_iter().filter_map(|(slot, fields)| {
        Some((slot, fields.bucket.clone()?, fields.reward.clone()?, fields))
    }).map(|(slot, bucket, reward, fields)| {
        let tags = optional_ts_field(&fields.tags, "source.row", "\"\"");
        let match_one = optional_ts_field(&fields.match_one, "source.row", "\"\"");
        let random_weight = optional_ts_field(&fields.random_weight, "source.row", "0");
        let budget = optional_ts_field(&fields.budget, "source.row", "0");
        let reward_type = optional_ts_field(&fields.reward_type, "source.row", "\"\"");
        let stage = optional_ts_field(&fields.stage_exclusion, "source.row", "\"\"");
        let shop = optional_ts_field(&fields.shop_exclusion, "source.row", "\"\"");
        format!(r#"      {{
        const slotKey = tableNumberLookupKey(source.ref.table, {slot});
        let track = this.rewardTracksBySlot.get(slotKey);
        if (source.slot.rowIndex === 0 && track === undefined) {{ const key = source.row.{bucket}?.trim() ?? ""; const id = Crc32.fromStringLower(key); if (key.length !== 0 && id !== Crc32.ZERO) {{ track = {{ table: source.ref.table, slot: {slot}, key, id, entries: [] }}; this.rewardTrackEntries.push(track); if (!this.rewardTracksById.has(id)) this.rewardTracksById.set(id, track); this.rewardTracksBySlot.set(slotKey, track); }} }}
        const rewardKey = source.row.{reward}?.trim() ?? "";
        const rewardId = Crc32.fromStringLower(rewardKey);
        if (track !== undefined && rewardKey.length !== 0 && rewardId !== Crc32.ZERO) track.entries.push(Object.freeze({{ sourceSlot: {slot}, sourceRow: source.slot.rowIndex, rewardKey, rewardId, rewardType: rewardTrackCrc({reward_type}), tagConstraints: rewardTrackTags({tags}), matchOne: schemaBoolean({match_one}, false), selectOnce: true, randomWeight: normalizeUint32(optionalSchemaNumber({random_weight}) ?? 0), budgetContribution: normalizeUint32(optionalSchemaNumber({budget}) ?? 0), stageExclusion: rewardTrackCrc({stage}), shopExclusion: rewardTrackCrc({shop}) }}));
      }}
"#)
    }).collect::<String>();
    TsNativeManagerAugmentation {
        declarations: format!(r#"export interface RewardTrackSlot {{ readonly table: {table}; readonly slot: number; }}
export interface RewardTrackTagConstraint {{ readonly tag: Crc32; readonly range: readonly [number, number]; }}
export interface RewardTrackEntry {{ readonly sourceSlot: number; readonly sourceRow: number; readonly rewardKey: string; readonly rewardId: Crc32; readonly rewardType: Crc32 | null; readonly tagConstraints: readonly RewardTrackTagConstraint[]; readonly matchOne: boolean; readonly selectOnce: boolean; readonly randomWeight: number; readonly budgetContribution: number; readonly stageExclusion: Crc32 | null; readonly shopExclusion: Crc32 | null; }}
export interface RewardTrackData {{ readonly table: {table}; readonly slot: number; readonly key: string; readonly id: Crc32; readonly entries: RewardTrackEntry[]; }}
function rewardTrackCrc(value: string): Crc32 | null {{ const id = Crc32.fromStringLower(value.trim()); return id === Crc32.ZERO ? null : id; }}
function rewardTrackRangeNumber(value: string): number {{ const integer = Number.parseInt(value.trim(), 10); if (Number.isInteger(integer) && integer >= 0 && integer <= 0xffff) return integer; const float = Number.parseFloat(value.trim()); return Number.isFinite(float) && float >= 0 && float <= 0xffff ? Math.trunc(float) : 0; }}
function rewardTrackRange(value: string): readonly [number, number] {{ const [left, right] = value.trim().split("-", 2); const start = rewardTrackRangeNumber(left); const end = right === undefined ? 10_000 : rewardTrackRangeNumber(right); return Object.freeze([Math.min(start, end), Math.max(start, end)]); }}
function rewardTrackTags(value: string): readonly RewardTrackTagConstraint[] {{ return Object.freeze(value.split(",").map((token) => token.trim()).filter((token) => token.length !== 0).flatMap((token) => {{ const separator = token.indexOf(":"); const key = (separator < 0 ? token : token.slice(0, separator)).trim(); const tag = Crc32.fromStringLower(key); if (key.length === 0 || tag === Crc32.ZERO) return []; const range = separator < 0 ? Object.freeze([0, 0] as const) : rewardTrackRange(token.slice(separator + 1)); return [Object.freeze({{ tag, range }})]; }})); }}

"#),
        fields: "  private readonly rewardTrackEntries: RewardTrackData[] = [];\n  private readonly rewardTracksById = new Map<Crc32, RewardTrackData>();\n  private readonly rewardTracksBySlot = new Map<string, RewardTrackData>();\n".to_owned(),
        initializers: format!("    for (const source of this.{field}) {{\n{init}    }}\n"),
        methods: "  rewardTrackDataFromId(id: Crc32): RewardTrackData | undefined {{ return this.rewardTracksById.get(id); }}\n  rewardTrackData(key: string): RewardTrackData | undefined {{ return this.rewardTrackDataFromId(Crc32.fromStringLower(key.trim())); }}\n  rewardTrackDataByKey(key: string): RewardTrackData | undefined {{ return this.rewardTrackData(key); }}\n  rewardTrackForSlot(slot: RewardTrackSlot): RewardTrackData | undefined {{ return this.rewardTracksBySlot.get(tableNumberLookupKey(slot.table, normalizeUint16(slot.slot))); }}\n  rewardTracks(): IterableIterator<RewardTrackData> {{ return this.rewardTrackEntries.values(); }}\n\n".to_owned(),
        rows_interface: Some(" implements Rows<RewardTrackData>".to_owned()),
        row_methods: Some("  rows(): IterableIterator<RewardTrackData> { return this.rewardTrackEntries.values(); }\n  [Symbol.iterator](): Iterator<RewardTrackData> { return this.rows(); }\n\n".to_owned()),
    }
}

#[derive(Default)]
struct RewardTrackFields {
    bucket: Option<String>,
    reward: Option<String>,
    tags: Option<String>,
    match_one: Option<String>,
    random_weight: Option<String>,
    budget: Option<String>,
    reward_type: Option<String>,
    stage_exclusion: Option<String>,
    shop_exclusion: Option<String>,
}

fn optional_ts_field(field: &Option<String>, receiver: &str, default: &str) -> String {
    field
        .as_ref()
        .map(|field| format!("({receiver}.{field} ?? {default})"))
        .unwrap_or_else(|| default.to_owned())
}

fn dynamic_difficulty(
    unit: &GameDataCompileUnit,
    manager: &DirectManagerSurface,
) -> TsNativeManagerAugmentation {
    let row = required_row(unit, manager, "DynamicDifficultyStaticData");
    let field = ts_direct_row_field_name("DynamicDifficultyStaticData");
    let table = ts_direct_table_type_name(manager, "DynamicDifficultyStaticData");
    let id = string_expression(required_field(&row, "DynamicDifficultyID"), "source.row");
    let modes = optional_field(&row, "GameModeIds")
        .map(|f| string_expression(f, "source.row"))
        .unwrap_or_else(|| "\"\"".to_owned());
    let tier = optional_field(&row, "DifficultyTier")
        .map(|f| number_expression(f, "source.row"))
        .unwrap_or_else(|| "0".to_owned());
    let mut effects = String::new();
    let mut potencies = String::new();
    for slot in 1..=5u8 {
        let Some(effect) = optional_field(&row, &format!("StatusEffect_{slot}"))
            .or_else(|| optional_field(&row, &format!("StatusEffect{slot}")))
        else {
            continue;
        };
        let effect = string_expression(effect, "source.row");
        effects.push_str(&format!("      {{ const key = {effect}.trim(); const id = Crc32.fromStringLower(key); if (id !== Crc32.ZERO) data.statusEffects.push(Object.freeze({{ slot: {slot}, key, id }})); }}\n"));
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
            let Some(potency) = candidates
                .iter()
                .find_map(|name| optional_field(&row, name))
            else {
                continue;
            };
            let potency = number_expression(potency, "source.row");
            potencies.push_str(&format!("      {{ const effect = data.statusEffects.find((value) => value.slot === {slot}); const creatureTypeId = Crc32.fromStringLower({creature:?}); if (effect !== undefined && creatureTypes.has(creatureTypeId)) data.potencies.push(Object.freeze({{ slot: {slot}, creatureTypeId, statusEffectId: effect.id, potency: {potency} }})); }}\n"));
        }
    }
    TsNativeManagerAugmentation {
        declarations: format!("export interface DynamicDifficultyStatusEffect {{ readonly slot: number; readonly key: string; readonly id: Crc32; }}\nexport interface DynamicDifficultyStatusEffectPotency {{ readonly slot: number; readonly creatureTypeId: Crc32; readonly statusEffectId: Crc32; readonly potency: number; }}\nexport interface DynamicDifficultyData {{ readonly source: RowRef<{table}, {}>; readonly key: string; readonly id: Crc32; readonly gameModeIds: Crc32[]; readonly difficultyTier: number; readonly statusEffects: DynamicDifficultyStatusEffect[]; readonly potencies: DynamicDifficultyStatusEffectPotency[]; }}\n\n", row.type_name),
        fields: "  private readonly dynamicDifficultyEntries: DynamicDifficultyData[] = [];\n  private readonly dynamicDifficultiesById = new Map<Crc32, DynamicDifficultyData>();\n  private readonly dynamicDifficultiesBySource = new Map<string, DynamicDifficultyData>();\n".to_owned(),
        initializers: format!(r#"    const creatureTypes = new Set(_vitalsData.creatureTypeIds());
    for (const source of this.{field}) {{
      const key = {id}.trim(); const id = Crc32.fromStringLower(key); if (key.length === 0 || id === Crc32.ZERO || this.dynamicDifficultiesById.has(id)) continue;
      const data: DynamicDifficultyData = {{ source: source.ref, key, id, gameModeIds: splitDesignerList({modes}).map((value) => Crc32.fromStringLower(value)), difficultyTier: normalizeUint8({tier}), statusEffects: [], potencies: [] }};
{effects}{potencies}      this.dynamicDifficultyEntries.push(data); this.dynamicDifficultiesById.set(id, data); this.dynamicDifficultiesBySource.set(tableNumberLookupKey(source.slot.table, source.slot.rowIndex), data);
    }}
"#),
        methods: "  dynamicDifficultyDataFromId(id: Crc32): DynamicDifficultyData | undefined { return this.dynamicDifficultiesById.get(id); }\n  dynamicDifficultyData(key: string): DynamicDifficultyData | undefined { return this.dynamicDifficultyDataFromId(Crc32.fromStringLower(key)); }\n  dynamicDifficultyDataByKey(key: string): DynamicDifficultyData | undefined { return this.dynamicDifficultyData(key); }\n  dynamicDifficultyForSource(source: RowSlot<DynamicDifficultyDataTable, DynamicDifficultyStaticDataSchemaRow>): DynamicDifficultyData | undefined { return this.dynamicDifficultiesBySource.get(tableNumberLookupKey(source.table, source.rowIndex)); }\n  dynamicDifficulties(): IterableIterator<DynamicDifficultyData> { return this.dynamicDifficultyEntries.values(); }\n\n".to_owned(),
        rows_interface: Some(" implements Rows<DynamicDifficultyData>".to_owned()),
        row_methods: Some("  rows(): IterableIterator<DynamicDifficultyData> { return this.dynamicDifficultyEntries.values(); }\n  [Symbol.iterator](): Iterator<DynamicDifficultyData> { return this.rows(); }\n\n".to_owned()),
    }
}

fn stat_modifier(
    unit: &GameDataCompileUnit,
    manager: &DirectManagerSurface,
) -> TsNativeManagerAugmentation {
    let contracts = ts_direct_row_specs(unit, manager)
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
            let stem = row.type_name.trim_end_matches("SchemaRow");
            Some(crc_secondary_contract(
                unit,
                manager,
                &row.source_row_type,
                key,
                &format!("{stem}FromID"),
                &format!("{stem}ByKey"),
                &ts_field_name(&format!("{stem} by id")),
            ))
        })
        .collect::<Vec<_>>();
    merge_augmentations(contracts)
}

fn numbered_suffix(value: &str, prefix: &str) -> Option<u16> {
    let suffix = value.get(prefix.len()..)?;
    value
        .get(..prefix.len())?
        .eq_ignore_ascii_case(prefix)
        .then(|| suffix.parse().ok())
        .flatten()
}

fn optional_string_expression(row: &TsSchemaRow, columns: &[&str], receiver: &str) -> String {
    columns
        .iter()
        .find_map(|column| optional_field(row, column))
        .map(|field| string_expression(field, receiver))
        .unwrap_or_else(|| "\"\"".to_owned())
}

fn optional_schema_number_expression(
    row: &TsSchemaRow,
    columns: &[&str],
    receiver: &str,
) -> String {
    columns
        .iter()
        .find_map(|column| optional_field(row, column))
        .map(|field| {
            let value = if field.required {
                format!("{receiver}.{}", field.field_name)
            } else {
                format!("({receiver}.{} ?? null)", field.field_name)
            };
            format!("optionalSchemaNumber({value})")
        })
        .unwrap_or_else(|| "null".to_owned())
}

fn optional_boolean_expression(
    row: &TsSchemaRow,
    columns: &[&str],
    receiver: &str,
    default: &str,
) -> String {
    columns
        .iter()
        .find_map(|column| optional_field(row, column))
        .map(|field| {
            let value = if field.required {
                format!("{receiver}.{}", field.field_name)
            } else {
                format!("({receiver}.{} ?? null)", field.field_name)
            };
            format!("schemaBoolean({value}, {default})")
        })
        .unwrap_or_else(|| default.to_owned())
}
