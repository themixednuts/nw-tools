use super::*;

pub(super) fn augmentation(
    unit: &GameDataCompileUnit,
    manager: &DirectManagerSurface,
    shape: &NativeManagerShape,
) -> TsNativeManagerAugmentation {
    match shape {
        NativeManagerShape::SeasonsRewardsData(_) => rewards(unit, manager),
        NativeManagerShape::SeasonsTrackedStatData(_) => crc_schema_contract(
            unit,
            manager,
            "SeasonsRewardsStats",
            "TrackedStatID",
            &["TrackedStatFromID"],
            &["TrackedStat", "TrackedStatByKey"],
            None,
        ),
        NativeManagerShape::SeasonsRewardsActivitiesTasksData(_) => typed_table_crc(
            unit,
            manager,
            "SeasonsRewardsActivitiesTasksData",
            "ActivitiesTaskID",
            "ActivityTask",
            "ActivityTaskByKey",
            "ActivityTasks",
        ),
        NativeManagerShape::SeasonsRewardsBattlePassData(_) => battle_pass(unit, manager),
        NativeManagerShape::SeasonsRewardsCardTemplateData(_) => typed_table_crc(
            unit,
            manager,
            "SeasonsRewardsCardTemplates",
            "CardAndRowID",
            "CardTemplate",
            "CardTemplateByKey",
            "CardTemplates",
        ),
        NativeManagerShape::SeasonsRewardsChapterData(_) => chapters(unit, manager),
        NativeManagerShape::SeasonsRewardsJourneyData(_) => journeys(unit, manager),
        NativeManagerShape::SongBookSheetData(_) => song_sheets(unit, manager),
        NativeManagerShape::SongBookData(_) => songs(unit, manager),
        _ => panic!(
            "manager {} reached seasons TypeScript native dispatch with {shape:?}",
            manager.manager_name
        ),
    }
}

fn rewards(
    unit: &GameDataCompileUnit,
    manager: &DirectManagerSurface,
) -> TsNativeManagerAugmentation {
    let mut value = crc_schema_contract(
        unit,
        manager,
        "SeasonsRewardData",
        "RewardID",
        &["ByID"],
        &["ByKey"],
        None,
    );
    let row = required_row(unit, manager, "SeasonsRewardData");
    let reward_index = number_expression(required_field(&row, "RewardIndex"), "source.row");
    let reward_type = string_expression(required_field(&row, "RewardType"), "source.row");
    let field = ts_direct_row_field_name("SeasonsRewardData");
    let table = ts_direct_table_type_name(manager, "SeasonsRewardData");
    let schema = row.type_name.clone();
    value.fields.push_str(&format!("  private readonly rewardsByIndex = new Map<number, RowEntry<{table}, {schema}>>();\n  private readonly rewardsByTypeIndex = new Map<string, RowEntry<{table}, {schema}>[]>();\n"));
    value.initializers.push_str(&format!(r#"    for (const source of this.{field}) {{
      this.rewardsByIndex.set(normalizeUnsignedInteger({reward_index}), source);
      const rewardType = normalizeLookupText({reward_type}); if (rewardType.length !== 0) appendMapValue(this.rewardsByTypeIndex, rewardType, source);
    }}
"#));
    value.methods.push_str(&format!("  byIndex(index: number): {schema} | undefined {{ return this.rewardsByIndex.get(normalizeUnsignedInteger(index))?.row; }}\n  *rewardsByType(rewardType: string): IterableIterator<{schema}> {{ for (const source of this.rewardsByTypeIndex.get(normalizeLookupText(rewardType)) ?? []) yield source.row; }}\n  *rewards(): IterableIterator<{schema}> {{ for (const source of this.{field}) yield source.row; }}\n\n"));
    value
}

fn typed_table_crc(
    unit: &GameDataCompileUnit,
    manager: &DirectManagerSurface,
    row_type: &str,
    key_column: &str,
    id_method: &str,
    key_method: &str,
    rows_method: &str,
) -> TsNativeManagerAugmentation {
    let row = required_row(unit, manager, row_type);
    let key = string_expression(required_field(&row, key_column), "source.row");
    let field = ts_direct_row_field_name(row_type);
    let table = ts_direct_table_type_name(manager, row_type);
    let schema = row.type_name.clone();
    let id_method = ts_method_name(id_method);
    let key_method = ts_method_name(key_method);
    let rows_method = ts_method_name(rows_method);
    TsNativeManagerAugmentation {
        fields: format!(
            "  private readonly rowsByTableAndId = new Map<string, RowEntry<{table}, {schema}>>();\n  private readonly rowsByTable = new Map<{table}, RowEntry<{table}, {schema}>[]>();\n"
        ),
        initializers: format!(
            r#"    for (const source of this.{field}) {{
      const text = {key}.trim(); const id = Crc32.fromStringLower(text); if (text.length === 0 || id === Crc32.ZERO) continue;
      const lookup = tableCrcLookupKey(source.ref.table, id); if (!this.rowsByTableAndId.has(lookup)) this.rowsByTableAndId.set(lookup, source);
      appendMapValue(this.rowsByTable, source.ref.table, source);
    }}
"#
        ),
        methods: format!(
            "  {id_method}(table: {table}, id: Crc32): {schema} | undefined {{ return this.rowsByTableAndId.get(tableCrcLookupKey(table, id))?.row; }}\n  {key_method}(table: {table}, key: string): {schema} | undefined {{ return this.{id_method}(table, Crc32.fromStringLower(key)); }}\n  *{rows_method}(table: {table}): IterableIterator<{schema}> {{ for (const source of this.rowsByTable.get(table) ?? []) yield source.row; }}\n\n"
        ),
        ..TsNativeManagerAugmentation::default()
    }
}

fn battle_pass(
    unit: &GameDataCompileUnit,
    manager: &DirectManagerSurface,
) -> TsNativeManagerAugmentation {
    let row = required_row(unit, manager, "SeasonPassRankData");
    let level = number_expression(required_field(&row, "Level"), "source.row");
    let maximum = number_expression(required_field(&row, "MaximumInfluence"), "source.row");
    let field = ts_direct_row_field_name("SeasonPassRankData");
    let table = ts_direct_table_type_name(manager, "SeasonPassRankData");
    let schema = row.type_name.clone();
    TsNativeManagerAugmentation {
        fields: format!(
            "  private readonly ranksBySeasonAndLevel = new Map<string, RowEntry<{table}, {schema}>>();\n  private readonly ranksBySeason = new Map<Crc32, RowEntry<{table}, {schema}>[]>();\n  private readonly maxRank = new Map<Crc32, number>();\n  private readonly maximumInfluenceBySeason = new Map<Crc32, number>();\n"
        ),
        initializers: format!(
            r#"    for (const source of this.{field}) {{
      const level = normalizeUnsignedInteger({level}); const seasonId = seasonIdFromTable(source.ref.table); if (seasonId === Crc32.ZERO) continue;
      this.ranksBySeasonAndLevel.set(crcNumberLookupKey(seasonId, level), source); appendMapValue(this.ranksBySeason, seasonId, source);
      this.maxRank.set(seasonId, Math.max(this.maxRank.get(seasonId) ?? 0, level));
      this.maximumInfluenceBySeason.set(seasonId, Math.max(this.maximumInfluenceBySeason.get(seasonId) ?? 0, normalizeUnsignedInteger({maximum})));
    }}
"#
        ),
        methods: format!(
            "  rank(seasonId: Crc32, level: number): {schema} | undefined {{ return this.ranksBySeasonAndLevel.get(crcNumberLookupKey(seasonId, normalizeUnsignedInteger(level)))?.row; }}\n  rankBySeasonKey(seasonKey: string, level: number): {schema} | undefined {{ return this.rank(Crc32.fromStringLower(seasonKey), level); }}\n  *ranks(seasonId: Crc32): IterableIterator<{schema}> {{ for (const source of this.ranksBySeason.get(seasonId) ?? []) yield source.row; }}\n  maxRankLevel(seasonId: Crc32): number {{ return this.maxRank.get(seasonId) ?? 0; }}\n  maximumInfluence(seasonId: Crc32): number {{ return this.maximumInfluenceBySeason.get(seasonId) ?? 0; }}\n\n"
        ),
        ..TsNativeManagerAugmentation::default()
    }
}

fn chapters(
    unit: &GameDataCompileUnit,
    manager: &DirectManagerSurface,
) -> TsNativeManagerAugmentation {
    let row = required_row(unit, manager, "SeasonsRewardsChapterData");
    let id = string_expression(required_field(&row, "ChapterID"), "source.row");
    let kind = string_expression(required_field(&row, "ChapterType"), "source.row");
    let chapter_index = number_expression(required_field(&row, "ChapterIndex"), "source.row");
    let reward = string_expression(required_field(&row, "ChapterRewardID"), "source.row");
    let field = ts_direct_row_field_name("SeasonsRewardsChapterData");
    let table = ts_direct_table_type_name(manager, "SeasonsRewardsChapterData");
    let schema = row.type_name.clone();
    TsNativeManagerAugmentation {
        declarations: "export type SeasonsChapterKind = string;\n\n".to_owned(),
        fields: format!(
            "  private readonly chaptersById = new Map<string, RowEntry<{table}, {schema}>>();\n  private readonly chaptersByReward = new Map<string, RowEntry<{table}, {schema}>>();\n  private readonly chaptersByKindIndex = new Map<string, RowEntry<{table}, {schema}>>();\n  private readonly chaptersBySeason = new Map<Crc32, RowEntry<{table}, {schema}>[]>();\n"
        ),
        initializers: format!(
            r#"    for (const source of this.{field}) {{
      const seasonId = seasonIdFromTable(source.ref.table); const id = Crc32.fromStringLower({id}.trim()); if (seasonId === Crc32.ZERO || id === Crc32.ZERO) continue;
      const key = crcPairLookupKey(seasonId, id); if (this.chaptersById.has(key)) continue; this.chaptersById.set(key, source); appendMapValue(this.chaptersBySeason, seasonId, source);
      const rewardId = Crc32.fromStringLower({reward}.trim()); if (rewardId !== Crc32.ZERO && !this.chaptersByReward.has(crcPairLookupKey(seasonId, rewardId))) this.chaptersByReward.set(crcPairLookupKey(seasonId, rewardId), source);
      const kind = {kind}.trim(); const chapterIndex = normalizeUnsignedInteger({chapter_index}); if (kind.length !== 0 && chapterIndex !== 0) {{ const kindKey = crcTextNumberLookupKey(seasonId, kind, chapterIndex); if (!this.chaptersByKindIndex.has(kindKey)) this.chaptersByKindIndex.set(kindKey, source); }}
    }}
"#
        ),
        methods: format!(
            "  chapter(seasonId: Crc32, chapterId: Crc32): {schema} | undefined {{ return this.chaptersById.get(crcPairLookupKey(seasonId, chapterId))?.row; }}\n  chapterByKey(seasonKey: string, chapterKey: string): {schema} | undefined {{ return this.chapter(Crc32.fromStringLower(seasonKey), Crc32.fromStringLower(chapterKey)); }}\n  chapterByReward(seasonId: Crc32, rewardId: Crc32): {schema} | undefined {{ return this.chaptersByReward.get(crcPairLookupKey(seasonId, rewardId))?.row; }}\n  chapterByKindIndex(seasonId: Crc32, kind: SeasonsChapterKind, index: number): {schema} | undefined {{ return this.chaptersByKindIndex.get(crcTextNumberLookupKey(seasonId, kind, normalizeUnsignedInteger(index)))?.row; }}\n  *chapters(seasonId: Crc32): IterableIterator<{schema}> {{ for (const source of this.chaptersBySeason.get(seasonId) ?? []) yield source.row; }}\n\n"
        ),
        ..TsNativeManagerAugmentation::default()
    }
}

fn journeys(
    unit: &GameDataCompileUnit,
    manager: &DirectManagerSurface,
) -> TsNativeManagerAugmentation {
    let row = required_row(unit, manager, "SeasonsRewardsJourneyData");
    let id = string_expression(required_field(&row, "JourneyTaskID"), "source.row");
    let chapter = string_expression(required_field(&row, "Chapter"), "source.row");
    let reward = string_expression(required_field(&row, "RewardID"), "source.row");
    required_field(&row, "SortOrder");
    let field = ts_direct_row_field_name("SeasonsRewardsJourneyData");
    let table = ts_direct_table_type_name(manager, "SeasonsRewardsJourneyData");
    let schema = row.type_name.clone();
    TsNativeManagerAugmentation {
        fields: format!(
            "  private readonly journeysById = new Map<string, RowEntry<{table}, {schema}>>();\n  private readonly journeysByReward = new Map<string, RowEntry<{table}, {schema}>>();\n  private readonly journeysByChapter = new Map<string, RowEntry<{table}, {schema}>[]>();\n  private readonly journeysBySeason = new Map<Crc32, RowEntry<{table}, {schema}>[]>();\n"
        ),
        initializers: format!(
            r#"    for (const source of this.{field}) {{
      const seasonId = seasonIdFromTable(source.ref.table); const id = Crc32.fromStringLower({id}.trim()); const chapterId = Crc32.fromStringLower({chapter}.trim()); const rewardId = Crc32.fromStringLower({reward}.trim());
      if (seasonId === Crc32.ZERO || id === Crc32.ZERO || chapterId === Crc32.ZERO || rewardId === Crc32.ZERO) continue;
      const key = crcPairLookupKey(seasonId, id); if (this.journeysById.has(key)) continue; this.journeysById.set(key, source); appendMapValue(this.journeysBySeason, seasonId, source); appendMapValue(this.journeysByChapter, crcPairLookupKey(seasonId, chapterId), source);
      const rewardKey = crcPairLookupKey(seasonId, rewardId); if (!this.journeysByReward.has(rewardKey)) this.journeysByReward.set(rewardKey, source);
    }}
    const sortRows = (left: RowEntry<{table}, {schema}>, right: RowEntry<{table}, {schema}>): number => (left.row.sortOrder ?? 0) - (right.row.sortOrder ?? 0);
    for (const rows of this.journeysBySeason.values()) rows.sort(sortRows); for (const rows of this.journeysByChapter.values()) rows.sort(sortRows);
"#
        ),
        methods: format!(
            "  journeyTask(seasonId: Crc32, journeyTaskId: Crc32): {schema} | undefined {{ return this.journeysById.get(crcPairLookupKey(seasonId, journeyTaskId))?.row; }}\n  journeyTaskByKey(seasonKey: string, taskKey: string): {schema} | undefined {{ return this.journeyTask(Crc32.fromStringLower(seasonKey), Crc32.fromStringLower(taskKey)); }}\n  journeyTaskByReward(seasonId: Crc32, rewardId: Crc32): {schema} | undefined {{ return this.journeysByReward.get(crcPairLookupKey(seasonId, rewardId))?.row; }}\n  *journeys(seasonId: Crc32): IterableIterator<{schema}> {{ for (const source of this.journeysBySeason.get(seasonId) ?? []) yield source.row; }}\n  *journeysForChapter(seasonId: Crc32, chapterId: Crc32): IterableIterator<{schema}> {{ for (const source of this.journeysByChapter.get(crcPairLookupKey(seasonId, chapterId)) ?? []) yield source.row; }}\n\n"
        ),
        ..TsNativeManagerAugmentation::default()
    }
}

fn song_sheets(
    unit: &GameDataCompileUnit,
    manager: &DirectManagerSurface,
) -> TsNativeManagerAugmentation {
    let mut value = crc_secondary_contract(
        unit,
        manager,
        "SongBookSheets",
        "SheetID",
        "SheetFromID",
        "Sheet",
        "sheetsById",
    );
    let row = required_row(unit, manager, "SongBookSheets");
    let instrument = string_expression(required_field(&row, "Instrument"), "source.row");
    let pages = string_expression(required_field(&row, "Pages"), "source.row");
    let field = ts_direct_row_field_name("SongBookSheets");
    let table = ts_direct_table_type_name(manager, "SongBookSheets");
    let schema = row.type_name.clone();
    value.fields.push_str(&format!("  private readonly sheetsByInstrument = new Map<string, RowEntry<{table}, {schema}>[]>();\n  private readonly sheetIdsByPage = new Map<Crc32, Crc32[]>();\n  private readonly sheetInstrumentById = new Map<Crc32, string>();\n  private readonly pageIdsBySheetId = new Map<Crc32, Crc32[]>();\n"));
    value.initializers.push_str(&format!(r#"    for (const source of this.{field}) {{
      const instrument = normalizeLookupText({instrument}); if (instrument.length !== 0) appendMapValue(this.sheetsByInstrument, instrument, source);
      const sheetId = Crc32.fromStringLower(source.row.sheetId.trim()); if (sheetId === Crc32.ZERO) continue; this.sheetInstrumentById.set(sheetId, instrument);
      for (const pageKey of splitDesignerList({pages})) {{ const pageId = Crc32.fromStringLower(pageKey); appendUniqueMapValue(this.pageIdsBySheetId, sheetId, pageId); appendUniqueMapValue(this.sheetIdsByPage, pageId, sheetId); }}
    }}
"#));
    value.methods.push_str(&named_rows_method(
        unit,
        manager,
        "SongBookSheets",
        "Sheets",
    ));
    value.methods.push_str(&format!("  *sheetsForInstrument(instrument: string): IterableIterator<{schema}> {{ for (const source of this.sheetsByInstrument.get(normalizeLookupText(instrument)) ?? []) yield source.row; }}\n  sheetIdsForPage(pageId: Crc32): IterableIterator<Crc32> {{ return (this.sheetIdsByPage.get(pageId) ?? []).values(); }}\n  instrumentForSheet(sheetId: Crc32): string | undefined {{ return this.sheetInstrumentById.get(sheetId); }}\n  pageIdsForSheet(sheetId: Crc32): IterableIterator<Crc32> {{ return (this.pageIdsBySheetId.get(sheetId) ?? []).values(); }}\n\n"));
    value
}

fn songs(
    unit: &GameDataCompileUnit,
    manager: &DirectManagerSurface,
) -> TsNativeManagerAugmentation {
    let mut value = crc_secondary_contract(
        unit,
        manager,
        "SongBookData",
        "SongID",
        "SongFromID",
        "Song",
        "songsById",
    );
    let row = required_row(unit, manager, "SongBookData");
    let field = ts_direct_row_field_name("SongBookData");
    let slots = (1..=5).filter_map(|slot| optional_field(&row, &format!("Slot{slot:02}"))).map(|column| string_expression(column, "source.row")).map(|expr| format!("      {{ const id = Crc32.fromStringLower({expr}.trim()); if (id !== Crc32.ZERO && _songBookSheetData.sheetFromId(id) !== undefined && !this.songSheetIds.includes(id)) this.songSheetIds.push(id); }}\n")).collect::<String>();
    value.fields.push_str("  private readonly songSheetIds: Crc32[] = [];\n  private readonly songPageIds: Crc32[] = [];\n  private readonly songSheetIdsByPage = new Map<Crc32, Crc32[]>();\n  private readonly songSheetIdsByInstrument = new Map<string, Crc32[]>();\n");
    value.initializers.push_str(&format!(r#"    for (const source of this.{field}) {{
{slots}    }}
    for (const sheetId of this.songSheetIds) {{
      const instrument = _songBookSheetData.instrumentForSheet(sheetId); if (instrument !== undefined) appendUniqueMapValue(this.songSheetIdsByInstrument, instrument, sheetId);
      for (const pageId of _songBookSheetData.pageIdsForSheet(sheetId)) {{ if (!this.songPageIds.includes(pageId)) this.songPageIds.push(pageId); appendUniqueMapValue(this.songSheetIdsByPage, pageId, sheetId); }}
    }}
"#));
    value
        .methods
        .push_str(&named_rows_method(unit, manager, "SongBookData", "Songs"));
    value.methods.push_str("  sheetIds(): IterableIterator<Crc32> { return this.songSheetIds.values(); }\n  pageIds(): IterableIterator<Crc32> { return this.songPageIds.values(); }\n  sheetIdsForPage(pageId: Crc32): IterableIterator<Crc32> { return (this.songSheetIdsByPage.get(pageId) ?? []).values(); }\n  sheetIdsForInstrument(instrument: string): IterableIterator<Crc32> { return (this.songSheetIdsByInstrument.get(normalizeLookupText(instrument)) ?? []).values(); }\n\n");
    value
}
