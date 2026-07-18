use super::*;

pub(super) fn augmentation(
    unit: &GameDataCompileUnit,
    manager: &DirectManagerSurface,
    shape: &NativeManagerShape,
) -> TsNativeManagerAugmentation {
    match shape {
        NativeManagerShape::ObjectivesData(_) => objectives(unit, manager),
        NativeManagerShape::ContributionData(_) => contribution(unit, manager),
        NativeManagerShape::BuffBucketData(_) => buff_bucket(unit, manager),
        NativeManagerShape::StructureData(_) => structure(unit, manager),
        NativeManagerShape::ReusableScoreboardData(_) => reusable_scoreboard(unit, manager),
        NativeManagerShape::MountHitVolumeData(_) => mount_hit_volume(unit, manager),
        _ => panic!(
            "manager {} reached family TypeScript native dispatch with {shape:?}",
            manager.manager_name
        ),
    }
}

fn objectives(
    unit: &GameDataCompileUnit,
    manager: &DirectManagerSurface,
) -> TsNativeManagerAugmentation {
    merge_augmentations([
        crc_secondary_contract(
            unit,
            manager,
            "Objectives",
            "ObjectiveID",
            "ObjectiveDataFromID",
            "ObjectiveData",
            "objectivesById",
        ),
        crc_secondary_contract(
            unit,
            manager,
            "ObjectiveTasks",
            "TaskID",
            "ObjectiveTaskDataFromID",
            "ObjectiveTaskData",
            "objectiveTasksById",
        ),
        named_rows(unit, manager, "Objectives", "Objectives"),
        named_rows(unit, manager, "ObjectiveTasks", "ObjectiveTasks"),
    ])
}

fn contribution(
    unit: &GameDataCompileUnit,
    manager: &DirectManagerSurface,
) -> TsNativeManagerAugmentation {
    let row = required_row(unit, manager, "ContributionData");
    let id = string_expression(required_field(&row, "ContributionID"), "source.row");
    let category = string_expression(required_field(&row, "Category"), "source.row");
    let row_field = ts_direct_row_field_name("ContributionData");
    let table = ts_direct_table_type_name(manager, "ContributionData");
    let schema = row.type_name.clone();
    TsNativeManagerAugmentation {
        fields: format!(
            "  private readonly contributions = new Map<string, RowEntry<{table}, {schema}>>();\n"
        ),
        initializers: format!(
            r#"    for (const source of this.{row_field}) {{
      const id = Crc32.fromStringLower({id}.trim());
      const category = normalizeLookupText({category});
      this.contributions.set(tableCrcTextLookupKey(source.ref.table, id, category), source);
    }}
"#
        ),
        methods: format!(
            "  contributionData(table: {table}, contributionId: Crc32, category: string): {schema} | undefined {{ return this.contributions.get(tableCrcTextLookupKey(table, contributionId, normalizeLookupText(category)))?.row; }}\n\n  contributionDataByKey(table: {table}, contributionId: string, category: string): {schema} | undefined {{ return this.contributionData(table, Crc32.fromStringLower(contributionId), category); }}\n\n"
        ),
        ..TsNativeManagerAugmentation::default()
    }
}

fn buff_bucket(
    unit: &GameDataCompileUnit,
    manager: &DirectManagerSurface,
) -> TsNativeManagerAugmentation {
    let row = required_row(unit, manager, "BuffBucketData");
    let bucket_id = string_expression(required_field(&row, "BuffBucketID"), "source.row");
    let table_kind = string_expression(required_field(&row, "TableType"), "source.row");
    let max_roll = number_expression(required_field(&row, "MaxRoll"), "probability.row");
    let row_field = ts_direct_row_field_name("BuffBucketData");
    let table = ts_direct_table_type_name(manager, "BuffBucketData");
    let schema = row.type_name.clone();
    let mut slots = String::new();
    for slot in 1..=6u8 {
        let Some(buff) = optional_field(&row, &format!("Buff{slot}")) else {
            continue;
        };
        let Some(kind) = optional_field(&row, &format!("BuffType{slot}")) else {
            continue;
        };
        let Some(potency) = optional_field(&row, &format!("BuffPotency{slot}")) else {
            continue;
        };
        let threshold = string_expression(buff, "probability.row");
        let buff = string_expression(buff, "source.row");
        let kind = string_expression(kind, "source.row");
        let potency = number_expression(potency, "source.row");
        slots.push_str(&format!(r#"      {{
        const buffKey = {buff}.trim();
        if (buffKey.length !== 0) {{
          const kind = {kind}.trim() as BuffBucketEntryKind;
          const rollThreshold = Number.parseInt({threshold}.trim(), 10);
          if (!isBuffBucketEntryKind(kind) || !Number.isSafeInteger(rollThreshold) || rollThreshold < 0) {{ malformed = true; }}
          else data.entries.push(Object.freeze({{ slot: {slot}, rollThreshold, buffKey, buffId: Crc32.fromStringLower(buffKey), kind, potency: {potency} }}));
        }}
      }}
"#));
    }
    TsNativeManagerAugmentation {
        declarations: format!(r#"export type BuffBucketTableType = "AND" | "OR";
export type BuffBucketEntryKind = "StatusEffect" | "Ability" | "BuffBucket" | "Promotion";
function isBuffBucketEntryKind(value: string): value is BuffBucketEntryKind {{ return value === "StatusEffect" || value === "Ability" || value === "BuffBucket" || value === "Promotion"; }}
export interface BuffBucketEntry {{ readonly slot: number; readonly rollThreshold: number; readonly buffKey: string; readonly buffId: Crc32; readonly kind: BuffBucketEntryKind; readonly potency: number; }}
export interface BuffBucketData {{ readonly source: RowRef<{table}, {schema}>; readonly probabilitySource: RowRef<{table}, {schema}>; readonly bucketKey: string; readonly bucketId: Crc32; readonly tableType: BuffBucketTableType; readonly maxRoll: number; readonly entries: BuffBucketEntry[]; }}

"#),
        fields: "  private readonly buffBucketEntries: BuffBucketData[] = [];\n  private readonly buffBucketsById = new Map<Crc32, BuffBucketData>();\n  private readonly buffBucketSourceRows = new Map<string, RowEntry<BuffBucketDataTable, BuffBucketDataSchemaRow>>();\n".to_owned(),
        initializers: format!(r#"    for (const source of this.{row_field}) {{ const key = {bucket_id}.trim(); if (key.length !== 0 && !this.buffBucketSourceRows.has(key)) this.buffBucketSourceRows.set(key, source); }}
    for (const source of this.{row_field}) {{
      const key = {bucket_id}.trim(); if (key.length === 0 || key.endsWith("_Probs")) continue;
      const id = Crc32.fromStringLower(key); if (id === Crc32.ZERO || this.buffBucketsById.has(id)) continue;
      const tableType = {table_kind}.trim(); if (tableType !== "AND" && tableType !== "OR") continue;
      const probability = this.buffBucketSourceRows.get(`${{key}}_Probs`); if (probability === undefined) continue;
      const data: BuffBucketData = {{ source: source.ref, probabilitySource: probability.ref, bucketKey: key, bucketId: id, tableType, maxRoll: normalizeUnsignedInteger({max_roll}), entries: [] }};
      let malformed = false;
{slots}      if (malformed) continue;
      data.entries.sort((left, right) => left.rollThreshold - right.rollThreshold);
      this.buffBucketEntries.push(data); this.buffBucketsById.set(id, data);
    }}
"#),
        methods: r#"  buffBucketDataFromId(id: Crc32): BuffBucketData | undefined { return this.buffBucketsById.get(id); }
  buffBucketData(key: string): BuffBucketData | undefined { return this.buffBucketDataFromId(Crc32.fromStringLower(key)); }
  *visitAllBuffsFromId(id: Crc32): IterableIterator<BuffBucketEntry> { yield* this.visitAllBuffsInner(id, new Set<Crc32>()); }
  *visitAllBuffs(key: string): IterableIterator<BuffBucketEntry> { yield* this.visitAllBuffsFromId(Crc32.fromStringLower(key)); }
  private *visitAllBuffsInner(id: Crc32, active: Set<Crc32>): IterableIterator<BuffBucketEntry> { if (active.has(id)) return; const bucket = this.buffBucketDataFromId(id); if (bucket === undefined) return; active.add(id); try { for (const entry of bucket.entries) { if (entry.kind === "BuffBucket") yield* this.visitAllBuffsInner(entry.buffId, active); else yield entry; } } finally { active.delete(id); } }
  buffBuckets(): IterableIterator<BuffBucketData> { return this.buffBucketEntries.values(); }

"#.to_owned(),
        rows_interface: Some(" implements Rows<BuffBucketData>".to_owned()),
        row_methods: Some("  rows(): IterableIterator<BuffBucketData> { return this.buffBucketEntries.values(); }\n  [Symbol.iterator](): Iterator<BuffBucketData> { return this.rows(); }\n\n".to_owned()),
    }
}

fn structure(
    unit: &GameDataCompileUnit,
    manager: &DirectManagerSurface,
) -> TsNativeManagerAugmentation {
    merge_augmentations([
        crc_secondary_contract(
            unit,
            manager,
            "StructureFootprintData",
            "FootprintID",
            "StructureFootprintDataFromID",
            "StructureFootprintData",
            "footprintsById",
        ),
        crc_secondary_contract(
            unit,
            manager,
            "StructurePieceData",
            "StructurePieceID",
            "StructurePieceDataFromID",
            "StructurePieceData",
            "piecesById",
        ),
        named_rows(unit, manager, "StructureFootprintData", "Footprints"),
        named_rows(unit, manager, "StructurePieceData", "Pieces"),
    ])
}

fn reusable_scoreboard(
    unit: &GameDataCompileUnit,
    manager: &DirectManagerSurface,
) -> TsNativeManagerAugmentation {
    merge_augmentations([
        crc_secondary_contract(
            unit,
            manager,
            "ReusableScoreboardTabData",
            "ReusableScoreboardTabId",
            "ReusableScoreboardDataFromID",
            "ReusableScoreboardData",
            "scoreboardsById",
        ),
        named_rows(unit, manager, "ReusableScoreboardTabData", "Scoreboards"),
    ])
}

fn mount_hit_volume(
    unit: &GameDataCompileUnit,
    manager: &DirectManagerSurface,
) -> TsNativeManagerAugmentation {
    let mut value = crc_secondary_contract(
        unit,
        manager,
        "MountTypeData",
        "MountID",
        "MountHitVolumeFromMountTypeID",
        "MountHitVolume",
        "mountsById",
    );
    let row = required_row(unit, manager, "MountTypeData");
    let Some(prefab) = [
        "MountHitVolumePrefab",
        "PrefabPath",
        "Prefab",
        "MountPrefabPath",
    ]
    .into_iter()
    .find_map(|name| optional_field(&row, name)) else {
        return value;
    };
    let row_field = ts_direct_row_field_name("MountTypeData");
    let table = ts_direct_table_type_name(manager, "MountTypeData");
    let prefab = string_expression(prefab, "source.row");
    let schema = row.type_name.clone();
    value.fields.push_str(&format!(
        "  private readonly mountsByPrefab = new Map<Crc32, RowEntry<{table}, {schema}>[]>();\n"
    ));
    value.initializers.push_str(&format!(r#"    for (const source of this.{row_field}) {{ const id = Crc32.fromStringLower({prefab}.trim()); if (id !== Crc32.ZERO) appendMapValue(this.mountsByPrefab, id, source); }}
"#));
    value.methods.push_str(&format!("  *mountHitVolumesForPrefabFromId(id: Crc32): IterableIterator<{schema}> {{ for (const source of this.mountsByPrefab.get(id) ?? []) yield source.row; }}\n  *mountHitVolumesForPrefab(prefab: string): IterableIterator<{schema}> {{ yield* this.mountHitVolumesForPrefabFromId(Crc32.fromStringLower(prefab)); }}\n\n"));
    value
}

fn named_rows(
    unit: &GameDataCompileUnit,
    manager: &DirectManagerSurface,
    row_type: &str,
    method: &str,
) -> TsNativeManagerAugmentation {
    TsNativeManagerAugmentation {
        methods: named_rows_method(unit, manager, row_type, method),
        ..TsNativeManagerAugmentation::default()
    }
}
