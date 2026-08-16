use super::{
    TsNativeManagerAugmentation, TsSchemaField, TsSchemaRow, damage_manager_augmentation,
    experience_manager_augmentation, tradeskill_rank_manager_augmentation,
    ts_direct_default_row_spec, ts_direct_row_field_name, ts_direct_row_specs,
    ts_direct_table_type_name, vitals_manager_augmentation,
};
use crate::GameDataCompileUnit;
use crate::manager::{
    NativeCrcIndexLookupParameterKind, NativeDuplicateKeyPolicy, NativeManagerShape,
    NativeMultiTableCrcKeyProjectionManager, NativeOneTableCostumeChangeManager,
    NativeOneTableCrcKeyProjectionManager, NativeOneTablePvpBalanceManager,
};
use crate::manager_records::{DirectManagerSurface, ts_field_name, ts_method_name};
use nw_datasheet::ColumnType;

mod families;
mod indexed;
mod seasons;
mod special;

pub(super) fn augmentation(
    unit: &GameDataCompileUnit,
    manager: &DirectManagerSurface,
    shape: &NativeManagerShape,
) -> TsNativeManagerAugmentation {
    match shape {
        NativeManagerShape::OneTableExperience(_) => experience_manager_augmentation(),
        NativeManagerShape::DamageData(_) => damage_manager_augmentation(unit, manager),
        NativeManagerShape::VitalsData(_) => vitals_manager_augmentation(),
        NativeManagerShape::TradeskillRankData(_) => {
            tradeskill_rank_manager_augmentation(unit, manager)
        }
        NativeManagerShape::AbilityData(_) => ability(unit, manager),
        NativeManagerShape::StatusEffectData(_) => crc_schema_contract(
            unit,
            manager,
            "StatusEffectData",
            "StatusID",
            &["StatusEffectDataFromID"],
            &["StatusEffectDataByName"],
            Some(("StatusEffectDataInTable", "StatusEffectDataByKeyInTable")),
        ),
        NativeManagerShape::ItemConversionData(_) => crc_schema_contract(
            unit,
            manager,
            "ItemCurrencyConversionData",
            "ConversionID",
            &["ByID"],
            &["ByKey"],
            None,
        ),
        NativeManagerShape::MultiTableCrcKeyProjection(shape) => {
            multi_table_crc_projection(unit, manager, shape)
        }

        NativeManagerShape::OneTableCampSkin(_)
        | NativeManagerShape::OneTableEmote(_)
        | NativeManagerShape::OneTableStoreCategory(_)
        | NativeManagerShape::OneTableStoreProduct(_)
        | NativeManagerShape::OneTableRewardTrackItem(_)
        | NativeManagerShape::OneTableWorldEventRule(_)
        | NativeManagerShape::QuickCourseData(_)
        | NativeManagerShape::RotationalQueueData(_)
        | NativeManagerShape::DynamicDifficultyData(_)
        | NativeManagerShape::ProgressionPointData(_)
        | NativeManagerShape::EntitlementData(_)
        | NativeManagerShape::EquipmentSetData(_)
        | NativeManagerShape::OneTablePvpBalance(_)
        | NativeManagerShape::OneTableDyeColor(_)
        | NativeManagerShape::RewardTrackData(_)
        | NativeManagerShape::PostSkillCapProgression(_)
        | NativeManagerShape::WhisperData(_)
        | NativeManagerShape::OneTableCostumeChange(_)
        | NativeManagerShape::OneTableCrestPart(_)
        | NativeManagerShape::OneTableDungeonTile(_)
        | NativeManagerShape::OneTableLevelDisparity(_)
        | NativeManagerShape::OneTableEncumbrance(_)
        | NativeManagerShape::OneTableDifficultyScaling(_)
        | NativeManagerShape::OneTableDarkness(_)
        | NativeManagerShape::OneTableParticleData(_)
        | NativeManagerShape::CharacterAttributeData(_)
        | NativeManagerShape::GovernanceData(_)
        | NativeManagerShape::LootBucketData(_)
        | NativeManagerShape::TerritoryDefinitionsData(_)
        | NativeManagerShape::StatModifierData(_) => indexed::augmentation(unit, manager, shape),

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
        | NativeManagerShape::ItemTransformData(_)
        | NativeManagerShape::GatherableData(_)
        | NativeManagerShape::SocialData(_)
        | NativeManagerShape::PlayerData(_)
        | NativeManagerShape::RecipeData(_) => special::augmentation(unit, manager, shape),

        NativeManagerShape::RequirementsOnly
        | NativeManagerShape::OneTableCrcIndex(_)
        | NativeManagerShape::TableFamilyCrcIndex(_)
        | NativeManagerShape::OneTableOwnedStringCrcIndex(_)
        | NativeManagerShape::TableFamilyOwnedStringCrcIndex(_)
        | NativeManagerShape::OneTableCrcKeyProjection(_)
        | NativeManagerShape::StaticBackstoryData(_)
        | NativeManagerShape::TableFamilyCrcKeyProjection(_)
        | NativeManagerShape::TableFamilyFallbackCrcKeyProjection(_)
        | NativeManagerShape::TableFamilyPartitionedCrcKeyProjection(_)
        | NativeManagerShape::OneTableNumericKeyProjection(_)
        | NativeManagerShape::TableFamilyNumericKeyProjection(_)
        | NativeManagerShape::OneTableEnumKeyProjection(_)
        | NativeManagerShape::OneTableStringKeyProjection(_)
        | NativeManagerShape::OneTableRowProjection(_)
        | NativeManagerShape::ItemData(_)
        | NativeManagerShape::CurrencyExchangeMapping(_)
        | NativeManagerShape::StaticTradeskillRankDataMapping(_)
        | NativeManagerShape::VitalsModifierMapping(_)
        | NativeManagerShape::ReplicationData(_)
        | NativeManagerShape::ProductAssetResource(_)
        | NativeManagerShape::ComposedResource(_) => panic!(
            "manager {} reached TypeScript native dispatch with pre-lowered shape {shape:?}",
            manager.manager_name
        ),
    }
}

fn ability(
    unit: &GameDataCompileUnit,
    manager: &DirectManagerSurface,
) -> TsNativeManagerAugmentation {
    let row = required_row(unit, manager, "AbilityData");
    let row_field = ts_direct_row_field_name("AbilityData");
    let table_type = ts_direct_table_type_name(manager, "AbilityData");
    let row_type = &row.type_name;
    let ability_id = string_expression(required_field(&row, "AbilityID"), "source.row");
    let tree_id = schema_number_expression(required_field(&row, "TreeID"), "source.row");
    let tree_row = schema_number_expression(required_field(&row, "TreeRowPosition"), "source.row");
    TsNativeManagerAugmentation {
        declarations: format!(
            r#"export interface AbilityDataPosition {{ readonly table: {table_type}; readonly position: number; }}

export interface AbilityData {{
  readonly source: RowRef<{table_type}, {row_type}>;
  readonly tableOrdinal: number;
  readonly tablePosition: number;
  readonly id: Crc32;
  readonly key: string;
  readonly treeId: number;
  readonly treeRowPosition: number;
}}

"#
        ),
        fields: format!(
            r#"  private readonly abilityEntries: AbilityData[] = [];
  private readonly abilitiesById = new Map<Crc32, AbilityData>();
  private readonly abilitiesByTable = new Map<string, AbilityData>();
  private readonly abilitiesByPosition = new Map<string, AbilityData>();
  private readonly abilityTables: {table_type}[] = [];
  private readonly abilityMaxTreeRow = new Map<number, number>();
"#
        ),
        initializers: format!(
            r#"    const abilityTableOrdinals = new Map<{table_type}, number>();
    const abilityTablePositions = new Map<{table_type}, number>();
    for (const source of this.{row_field}) {{
      const key = {ability_id}.trim();
      const id = Crc32.fromStringLower(key);
      if (key.length === 0 || id === Crc32.ZERO) continue;
      const treeId = abilityCoordinate({tree_id});
      const treeRowPosition = abilityCoordinate({tree_row});
      if (treeId === null || treeRowPosition === null) continue;
      const table = source.ref.table;
      let tableOrdinal = abilityTableOrdinals.get(table);
      if (tableOrdinal === undefined) {{
        tableOrdinal = this.abilityTables.length;
        if (tableOrdinal > 0xff) throw new RangeError(`AbilityData has more than 256 tables`);
        abilityTableOrdinals.set(table, tableOrdinal);
        this.abilityTables.push(table);
      }}
      const tablePosition = abilityTablePositions.get(table) ?? 0;
      if (tablePosition > 0x3ff) throw new RangeError(`AbilityData table ${{table}} has more than 1024 rows`);
      abilityTablePositions.set(table, tablePosition + 1);
      const data: AbilityData = Object.freeze({{ source: source.ref, tableOrdinal, tablePosition, id, key, treeId, treeRowPosition }});
      this.abilityEntries.push(data);
      if (!this.abilitiesById.has(id)) this.abilitiesById.set(id, data);
      const tableKey = tableCrcLookupKey(table, id);
      if (!this.abilitiesByTable.has(tableKey)) this.abilitiesByTable.set(tableKey, data);
      this.abilitiesByPosition.set(abilityPositionLookupKey(table, tablePosition), data);
      this.abilityMaxTreeRow.set(treeId, Math.max(this.abilityMaxTreeRow.get(treeId) ?? 0, treeRowPosition));
    }}
"#
        ),
        methods: format!(
            r#"  abilityDataFromId(id: Crc32): AbilityData | undefined {{ return this.abilitiesById.get(id); }}

  abilityData(key: string): AbilityData | undefined {{ return this.abilityDataFromId(Crc32.fromStringLower(key.trim())); }}

  abilityDataForTable(table: {table_type}, id: Crc32): AbilityData | undefined {{ return this.abilitiesByTable.get(tableCrcLookupKey(table, id)); }}

  abilityDataByKeyForTable(table: {table_type}, key: string): AbilityData | undefined {{ return this.abilityDataForTable(table, Crc32.fromStringLower(key.trim())); }}

  abilityDataAtPosition(position: AbilityDataPosition): AbilityData | undefined {{ return this.abilitiesByPosition.get(abilityPositionLookupKey(position.table, position.position)); }}

  abilityDataForTableSlot(tableOrdinal: number, position: number): AbilityData | undefined {{
    const table = this.abilityTables[normalizeUint8(tableOrdinal)];
    return table === undefined ? undefined : this.abilityDataAtPosition({{ table, position: normalizeUint16(position) }});
  }}

  maxTreeRowPosition(treeId: number): number | undefined {{ return this.abilityMaxTreeRow.get(normalizeUint8(treeId)); }}

  abilityIds(): IterableIterator<Crc32> {{ return this.abilityEntries.map((ability) => ability.id).values(); }}

  abilities(): IterableIterator<AbilityData> {{ return this.abilityEntries.values(); }}

"#
        ),
        rows_interface: Some(" implements Rows<AbilityData>".to_owned()),
        row_methods: Some("  rows(): IterableIterator<AbilityData> { return this.abilityEntries.values(); }\n  [Symbol.iterator](): Iterator<AbilityData> { return this.rows(); }\n\n".to_owned()),
    }
}

pub(super) fn crc_schema_contract(
    unit: &GameDataCompileUnit,
    manager: &DirectManagerSurface,
    source_row_type: &str,
    key_column: &str,
    id_methods: &[&str],
    key_methods: &[&str],
    table_methods: Option<(&str, &str)>,
) -> TsNativeManagerAugmentation {
    crc_schema_contract_with_policy(
        unit,
        manager,
        source_row_type,
        key_column,
        id_methods,
        key_methods,
        table_methods,
        NativeDuplicateKeyPolicy::FirstWins,
    )
}

// This renderer needs the complete key and table method contract.
#[allow(clippy::too_many_arguments)]
fn crc_schema_contract_with_policy(
    unit: &GameDataCompileUnit,
    manager: &DirectManagerSurface,
    source_row_type: &str,
    key_column: &str,
    id_methods: &[&str],
    key_methods: &[&str],
    table_methods: Option<(&str, &str)>,
    duplicate_policy: NativeDuplicateKeyPolicy,
) -> TsNativeManagerAugmentation {
    let row = required_row(unit, manager, source_row_type);
    let key = required_field(&row, key_column);
    let row_field = ts_direct_row_field_name(source_row_type);
    let table_type = ts_direct_table_type_name(manager, source_row_type);
    let row_type = &row.type_name;
    let map_name = ts_field_name(&format!(
        "{} by id",
        row.type_name.trim_end_matches("SchemaRow")
    ));
    let table_map_name = format!("{map_name}AndTable");
    let key_expression = string_expression(key, "source.row");
    let mut methods = String::new();
    for method in id_methods {
        let method = ts_method_name(method);
        methods.push_str(&format!(
            "  {method}(id: Crc32): {row_type} | undefined {{ return this.{map_name}.get(id)?.row; }}\n\n"
        ));
    }
    let canonical_id = ts_method_name(
        id_methods
            .first()
            .expect("CRC native contracts require an id method"),
    );
    for method in key_methods {
        let method = ts_method_name(method);
        methods.push_str(&format!(
            "  {method}(key: string): {row_type} | undefined {{ return this.{canonical_id}(Crc32.fromStringLower(key)); }}\n\n"
        ));
    }
    if let Some((id_method, key_method)) = table_methods {
        let id_method = ts_method_name(id_method);
        let key_method = ts_method_name(key_method);
        methods.push_str(&format!(
            "  {id_method}(table: {table_type}, id: Crc32): {row_type} | undefined {{ return this.{table_map_name}.get(tableCrcLookupKey(table, id))?.row; }}\n\n  {key_method}(table: {table_type}, key: string): {row_type} | undefined {{ return this.{id_method}(table, Crc32.fromStringLower(key)); }}\n\n"
        ));
    }
    if table_methods.is_some() {
        methods.push_str(&format!(
            "  rowInTable(table: {table_type}, id: string | Crc32): {row_type} | undefined {{ return this.{table_map_name}.get(tableCrcLookupKey(table, crc32LookupKey(id)))?.row; }}\n\n"
        ));
    }
    let inserts = match duplicate_policy {
        NativeDuplicateKeyPolicy::FirstWins => format!(
            "      if (!this.{table_map_name}.has(tableKey)) this.{table_map_name}.set(tableKey, source);\n      if (!this.{map_name}.has(id)) this.{map_name}.set(id, source);"
        ),
        NativeDuplicateKeyPolicy::Overwrite => format!(
            "      this.{table_map_name}.set(tableKey, source);\n      this.{map_name}.set(id, source);"
        ),
        NativeDuplicateKeyPolicy::Error => format!(
            "      if (this.{table_map_name}.has(tableKey) || this.{map_name}.has(id)) throw new Error(`duplicate {source_row_type} key ${{key}}`);\n      this.{table_map_name}.set(tableKey, source);\n      this.{map_name}.set(id, source);"
        ),
    };

    TsNativeManagerAugmentation {
        fields: format!(
            "  private readonly {map_name} = new Map<Crc32, RowEntry<{table_type}, {row_type}>>();\n  private readonly {table_map_name} = new Map<string, RowEntry<{table_type}, {row_type}>>();\n"
        ),
        initializers: format!(
            r#"    for (const source of this.{row_field}) {{
      const key = {key_expression}.trim();
      const id = Crc32.fromStringLower(key);
      if (key.length === 0 || id === Crc32.ZERO) continue;
      const tableKey = tableCrcLookupKey(source.ref.table, id);
{inserts}
    }}
"#
        ),
        methods,
        ..TsNativeManagerAugmentation::default()
    }
}

fn multi_table_crc_projection(
    unit: &GameDataCompileUnit,
    manager: &DirectManagerSurface,
    shape: &NativeMultiTableCrcKeyProjectionManager,
) -> TsNativeManagerAugmentation {
    merge_augmentations(
        shape
            .projections()
            .iter()
            .map(|projection| crc_projection(unit, manager, projection)),
    )
}

fn crc_projection(
    unit: &GameDataCompileUnit,
    manager: &DirectManagerSurface,
    projection: &NativeOneTableCrcKeyProjectionManager,
) -> TsNativeManagerAugmentation {
    let mut id_methods = Vec::new();
    let mut key_methods = Vec::new();
    for method in projection.methods() {
        match method.parameter().kind() {
            NativeCrcIndexLookupParameterKind::Crc32
            | NativeCrcIndexLookupParameterKind::IntoCrc32 => {
                id_methods.push(method.name().as_str())
            }
            NativeCrcIndexLookupParameterKind::StrRef
            | NativeCrcIndexLookupParameterKind::AsRefStr => {
                key_methods.push(method.name().as_str())
            }
        }
    }
    let mut value = crc_schema_contract_with_policy(
        unit,
        manager,
        projection.row_type_name().as_str(),
        projection.key_column().as_str(),
        &id_methods,
        &key_methods,
        None,
        projection.duplicate_key_policy(),
    );
    if let Some(method) = projection.rows_method() {
        value.methods.push_str(&named_rows_method(
            unit,
            manager,
            projection.row_type_name().as_str(),
            method.as_str(),
        ));
    }
    let map_name = ts_field_name(&format!(
        "{} by id",
        required_row(unit, manager, projection.row_type_name().as_str())
            .type_name
            .trim_end_matches("SchemaRow")
    ));
    if let Some(method) = projection.ids_method() {
        value.methods.push_str(&format!(
            "  {}(): IterableIterator<Crc32> {{ return this.{map_name}.keys(); }}\n\n",
            ts_method_name(method.as_str())
        ));
    }
    if let Some(method) = projection.crc_ids_method() {
        value.methods.push_str(&format!(
            "  {}(): IterableIterator<Crc32> {{ return this.{map_name}.keys(); }}\n\n",
            ts_method_name(method.as_str())
        ));
    }
    if let Some(method) = projection.len_method() {
        value.methods.push_str(&format!(
            "  {}(): number {{ return this.{map_name}.size; }}\n\n",
            ts_method_name(method.as_str())
        ));
    }
    if let Some(method) = projection.is_empty_method() {
        value.methods.push_str(&format!(
            "  {}(): boolean {{ return this.{map_name}.size === 0; }}\n\n",
            ts_method_name(method.as_str())
        ));
    }
    value
}

pub(super) fn crc_secondary_contract(
    unit: &GameDataCompileUnit,
    manager: &DirectManagerSurface,
    row_type: &str,
    key_column: &str,
    id_method: &str,
    key_method: &str,
    map_name: &str,
) -> TsNativeManagerAugmentation {
    let row = required_row(unit, manager, row_type);
    let key = required_field(&row, key_column);
    let row_field = ts_direct_row_field_name(row_type);
    let table_type = ts_direct_table_type_name(manager, row_type);
    let schema_row = &row.type_name;
    let key_expression = string_expression(key, "source.row");
    let id_method = ts_method_name(id_method);
    let key_method = ts_method_name(key_method);
    TsNativeManagerAugmentation {
        fields: format!(
            "  private readonly {map_name} = new Map<Crc32, RowEntry<{table_type}, {schema_row}>>();\n"
        ),
        initializers: format!(
            r#"    for (const source of this.{row_field}) {{
      const text = {key_expression}.trim();
      const id = Crc32.fromStringLower(text);
      if (text.length !== 0 && id !== Crc32.ZERO && !this.{map_name}.has(id)) this.{map_name}.set(id, source);
    }}
"#
        ),
        methods: format!(
            "  {id_method}(id: Crc32): {schema_row} | undefined {{ return this.{map_name}.get(id)?.row; }}\n\n  {key_method}(key: string): {schema_row} | undefined {{ return this.{id_method}(Crc32.fromStringLower(key)); }}\n\n"
        ),
        ..TsNativeManagerAugmentation::default()
    }
}

pub(super) fn numeric_contract(
    unit: &GameDataCompileUnit,
    manager: &DirectManagerSurface,
    row_type: &str,
    key_column: &str,
    methods: &[&str],
    rows_method: Option<&str>,
) -> TsNativeManagerAugmentation {
    numeric_contract_with_normalizer(
        unit,
        manager,
        row_type,
        key_column,
        methods,
        rows_method,
        "normalizeUnsignedInteger",
    )
}

pub(super) fn numeric_contract_with_normalizer(
    unit: &GameDataCompileUnit,
    manager: &DirectManagerSurface,
    row_type: &str,
    key_column: &str,
    methods: &[&str],
    rows_method: Option<&str>,
    normalizer: &str,
) -> TsNativeManagerAugmentation {
    let row = required_row(unit, manager, row_type);
    let key = required_field(&row, key_column);
    let row_field = ts_direct_row_field_name(row_type);
    let table_type = ts_direct_table_type_name(manager, row_type);
    let schema_row = &row.type_name;
    let map_name = ts_field_name(&format!(
        "{} by number",
        row.type_name.trim_end_matches("SchemaRow")
    ));
    let key_expression = number_expression(key, "source.row");
    let mut method_source = methods
        .iter()
        .map(|method| {
            let method = ts_method_name(method);
            format!(
                "  {method}(key: number): {schema_row} | undefined {{ return this.{map_name}.get({normalizer}(key))?.row; }}\n\n"
            )
        })
        .collect::<String>();
    if let Some(rows_method) = rows_method {
        method_source.push_str(&named_rows_method(unit, manager, row_type, rows_method));
    }
    TsNativeManagerAugmentation {
        fields: format!(
            "  private readonly {map_name} = new Map<number, RowEntry<{table_type}, {schema_row}>>();\n"
        ),
        initializers: format!(
            r#"    for (const source of this.{row_field}) {{
      const key = {normalizer}({key_expression});
      if (!this.{map_name}.has(key)) this.{map_name}.set(key, source);
    }}
"#
        ),
        methods: method_source,
        ..TsNativeManagerAugmentation::default()
    }
}

pub(super) fn named_rows_method(
    unit: &GameDataCompileUnit,
    manager: &DirectManagerSurface,
    row_type: &str,
    method: &str,
) -> String {
    let row = required_row(unit, manager, row_type);
    let field = ts_direct_row_field_name(row_type);
    let method = ts_method_name(method);
    format!(
        "  *{method}(): IterableIterator<{}> {{ for (const source of this.{field}) yield source.row; }}\n\n",
        row.type_name
    )
}

pub(super) fn merge_augmentations(
    augmentations: impl IntoIterator<Item = TsNativeManagerAugmentation>,
) -> TsNativeManagerAugmentation {
    let mut merged = TsNativeManagerAugmentation::default();
    for augmentation in augmentations {
        merged.declarations.push_str(&augmentation.declarations);
        merged.fields.push_str(&augmentation.fields);
        merged.initializers.push_str(&augmentation.initializers);
        merged.methods.push_str(&augmentation.methods);
        if augmentation.rows_interface.is_some() {
            assert!(merged.rows_interface.is_none());
            merged.rows_interface = augmentation.rows_interface;
        }
        if augmentation.row_methods.is_some() {
            assert!(merged.row_methods.is_none());
            merged.row_methods = augmentation.row_methods;
        }
    }
    merged
}

pub(super) fn required_row(
    unit: &GameDataCompileUnit,
    manager: &DirectManagerSurface,
    row_type: &str,
) -> TsSchemaRow {
    ts_direct_row_specs(unit, manager)
        .into_iter()
        .find(|row| row.source_row_type.eq_ignore_ascii_case(row_type))
        .unwrap_or_else(|| panic!("{} requires schema row {row_type}", manager.manager_name))
}

pub(super) fn required_field<'a>(row: &'a TsSchemaRow, column: &str) -> &'a TsSchemaField {
    row.fields
        .iter()
        .find(|field| field.source_name.eq_ignore_ascii_case(column))
        .unwrap_or_else(|| panic!("{} requires column {column}", row.source_row_type))
}

pub(super) fn optional_field<'a>(row: &'a TsSchemaRow, column: &str) -> Option<&'a TsSchemaField> {
    row.fields
        .iter()
        .find(|field| field.source_name.eq_ignore_ascii_case(column))
}

pub(super) fn string_expression(field: &TsSchemaField, receiver: &str) -> String {
    if field.required {
        format!("{receiver}.{}", field.field_name)
    } else {
        format!("({receiver}.{} ?? \"\")", field.field_name)
    }
}

pub(super) fn number_expression(field: &TsSchemaField, receiver: &str) -> String {
    match (field.column_type, field.required) {
        (ColumnType::Number, true) => format!("{receiver}.{}", field.field_name),
        (ColumnType::Number, false) => format!("({receiver}.{} ?? 0)", field.field_name),
        (ColumnType::String, _) => format!(
            "(optionalSchemaNumber({receiver}.{}) ?? 0)",
            field.field_name
        ),
        _ => panic!("column `{}` is not numeric-like", field.source_name),
    }
}

fn schema_number_expression(field: &TsSchemaField, receiver: &str) -> String {
    if field.required {
        format!("{receiver}.{}", field.field_name)
    } else {
        format!("({receiver}.{} ?? null)", field.field_name)
    }
}

pub(super) fn default_row(
    unit: &GameDataCompileUnit,
    manager: &DirectManagerSurface,
) -> TsSchemaRow {
    ts_direct_default_row_spec(unit, manager)
        .unwrap_or_else(|| panic!("{} requires a default schema row", manager.manager_name))
}

pub(super) fn pvp_balance(
    manager: &DirectManagerSurface,
    shape: &NativeOneTablePvpBalanceManager,
) -> TsNativeManagerAugmentation {
    let data_type = manager
        .manager_class_name
        .strip_suffix("Manager")
        .unwrap_or(&manager.manager_class_name);
    let row_type = shape.row_type_name().as_str();
    let row_field = ts_direct_row_field_name(row_type);
    let target_field = ts_field_name(shape.target_column().as_str());
    let category_field = ts_field_name(shape.category_column().as_str());
    let methods = shape
        .methods()
        .iter()
        .map(|method| {
            let name = ts_method_name(method.name().as_str());
            let parameter = method.parameter();
            let parameter_name = ts_field_name(parameter.name().as_str());
            let (parameter_type, key) = match parameter.kind() {
                NativeCrcIndexLookupParameterKind::StrRef
                | NativeCrcIndexLookupParameterKind::AsRefStr => {
                    ("string", format!("Crc32.fromStringLower({parameter_name})"))
                }
                NativeCrcIndexLookupParameterKind::Crc32 => ("Crc32", parameter_name.clone()),
                NativeCrcIndexLookupParameterKind::IntoCrc32 => {
                    ("string | Crc32", format!("crc32LookupKey({parameter_name})"))
                }
            };
            format!(
                "  {name}({parameter_name}: {parameter_type}): {data_type} | undefined {{ return this.pvpBalancesById.get({key}); }}\n\n"
            )
        })
        .collect::<String>();
    let balances_method = shape
        .balances_method()
        .map(|method| {
            format!(
                "  {}(): IterableIterator<{data_type}> {{ return this.pvpBalanceEntries.values(); }}\n",
                ts_method_name(method.as_str())
            )
        })
        .unwrap_or_default();
    let len_method = shape
        .len_method()
        .map(|method| {
            format!(
                "  {}(): number {{ return this.pvpBalanceEntries.length; }}\n",
                ts_method_name(method.as_str())
            )
        })
        .unwrap_or_default();
    let is_empty_method = shape
        .is_empty_method()
        .map(|method| {
            format!(
                "  {}(): boolean {{ return this.pvpBalanceEntries.length === 0; }}\n",
                ts_method_name(method.as_str())
            )
        })
        .unwrap_or_default();

    TsNativeManagerAugmentation {
        declarations: format!(
            r#"export interface {data_type} {{
  readonly sourceRow: number;
  readonly target: string;
  readonly targetCrc: Crc32;
  readonly category: string;
  readonly abilityBaseDamage: string | null;
  readonly affixStat: string | null;
  readonly incomingHeal: string | null;
  readonly consumableHeal: string | null;
  readonly potency: number | null;
  readonly duration: number | null;
  readonly weaponBaseDamage: number;
  readonly selfHeal: number;
  readonly cooldown: number;
}}

"#
        ),
        fields: format!(
            r#"  private readonly pvpBalanceEntries: {data_type}[] = [];
  private readonly pvpBalancesById = new Map<Crc32, {data_type}>();
  private readonly pvpBalancesByCategory = new Map<Crc32, {data_type}[]>();
"#
        ),
        initializers: format!(
            r#"    for (const source of this.{row_field}) {{
      const target = source.row.{target_field}.trim();
      if (target.length === 0) continue;
      const targetCrc = Crc32.fromStringLower(target);
      if (this.pvpBalancesById.has(targetCrc)) continue;
      const data: {data_type} = Object.freeze({{
        sourceRow: source.slot.rowIndex,
        target,
        targetCrc,
        category: source.row.{category_field}?.trim() ?? "",
        abilityBaseDamage: nonEmptyString(source.row.abilityBaseDamageAdjustment),
        affixStat: nonEmptyString(source.row.affixStatAdjustment),
        incomingHeal: nonEmptyString(source.row.incomingHealAdjustment),
        consumableHeal: nonEmptyString(source.row.consumableHealAdjustment),
        potency: optionalSchemaNumber(source.row.potencyAdjustment),
        duration: optionalSchemaNumber(source.row.durationAdjustment),
        weaponBaseDamage: requiredSchemaNumber(source.row.weaponBaseDamageAdjustment, "WeaponBaseDamageAdjustment", source.ref),
        selfHeal: requiredSchemaNumber(source.row.selfHealAdjustment, "SelfHealAdjustment", source.ref),
        cooldown: requiredSchemaNumber(source.row.cooldownAdjustment, "CooldownAdjustment", source.ref),
      }});
      this.pvpBalanceEntries.push(data);
      this.pvpBalancesById.set(targetCrc, data);
      const categoryId = Crc32.fromStringLower(data.category);
      if (categoryId !== Crc32.ZERO) appendMapValue(this.pvpBalancesByCategory, categoryId, data);
    }}
"#
        ),
        methods: format!(
            "{methods}{balances_method}{len_method}{is_empty_method}  balancesForCategory(category: Crc32): IterableIterator<{data_type}> {{ return (this.pvpBalancesByCategory.get(category) ?? []).values(); }}\n\n"
        ),
        rows_interface: Some(format!(" implements Rows<{data_type}>")),
        row_methods: Some(format!(
            "  rows(): IterableIterator<{data_type}> {{ return this.pvpBalanceEntries.values(); }}\n  [Symbol.iterator](): Iterator<{data_type}> {{ return this.rows(); }}\n\n"
        )),
    }
}

#[cfg(test)]
mod tests {
    use crate::manager::{NativeManagerShape, validated_native_manager_specs};

    use super::*;

    #[test]
    fn pvp_balance_uses_shape_columns_and_named_indexes() {
        let shape = validated_native_manager_specs()
            .into_iter()
            .find_map(|manager| match manager.shape() {
                Some(NativeManagerShape::OneTablePvpBalance(shape)) => Some(shape.clone()),
                _ => None,
            })
            .expect("validated PvP balance manager");
        let manager = DirectManagerSurface {
            manager_name: "ArenaPvpBalanceDataManager".to_owned(),
            manager_class_name: "ArenaPvpBalanceDataManager".to_owned(),
            tables: Vec::new(),
            products: Vec::new(),
        };
        let augmentation = pvp_balance(&manager, &shape);

        assert!(augmentation.initializers.contains("balanceTarget"));
        assert!(augmentation.initializers.contains("balanceCategory"));
        assert!(augmentation.initializers.contains("pvpBalancesById.has"));
        assert!(augmentation.methods.contains("ByKey"));
        assert!(augmentation.declarations.contains("ArenaPvpBalanceData"));
        assert!(!augmentation.initializers.contains("source.ref.key"));
        assert!(!augmentation.declarations.contains("#[cfg(test)]"));
    }
}
