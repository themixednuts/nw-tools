use super::*;

pub(super) fn augmentation(
    unit: &GameDataCompileUnit,
    manager: &DirectManagerSurface,
    shape: &NativeManagerShape,
) -> GoNativeManagerAugmentation {
    match shape {
        NativeManagerShape::SeasonsRewardsData(_) => rewards(unit, manager),
        NativeManagerShape::SeasonsTrackedStatData(_) => crc_schema_contract(
            unit,
            manager,
            "SeasonsRewardsStats",
            "TrackedStatID",
            &["TrackedStatFromID"],
            &["TrackedStat", "TrackedStatByKey"],
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
            "manager {} reached seasons Go native dispatch with {shape:?}",
            manager.manager_name
        ),
    }
}

fn rewards(
    unit: &GameDataCompileUnit,
    manager: &DirectManagerSurface,
) -> GoNativeManagerAugmentation {
    let mut augmentation = crc_schema_contract(
        unit,
        manager,
        "SeasonsRewardData",
        "RewardID",
        &["ByID"],
        &["ByKey"],
    );
    let row = indexed::required_row(unit, manager, "SeasonsRewardData");
    let reward_index = indexed::required_field(&row, "RewardIndex");
    let reward_type = indexed::required_field(&row, "RewardType");
    let row_field = go_direct_row_field_name("SeasonsRewardData");
    let manager_type = go_method_name(&manager.manager_class_name);
    let row_name = row.type_name.clone();
    augmentation
        .fields
        .push_str("\trewardsByIndex map[uint32]int\n\trewardsByType map[string][]int\n");
    augmentation.field_values.push_str(
        "\t\trewardsByIndex: make(map[uint32]int),\n\t\trewardsByType: make(map[string][]int),\n",
    );
    augmentation.initializers.push_str(&format!(
        r#"	for index := range manager.{row_field}.entries {{
		row := rowCopy(manager.{row_field}.entries[index].Row)
		if row.{reward_index_field} != nil {{
			if value, ok := exactUint32(*row.{reward_index_field}); ok {{ manager.rewardsByIndex[value] = index }}
		}}
		if row.{reward_type_field} != nil {{
			key := strings.ToLower(strings.TrimSpace(*row.{reward_type_field}))
			if key != "" {{ manager.rewardsByType[key] = append(manager.rewardsByType[key], index) }}
		}}
	}}
"#,
        reward_index_field = reward_index.field_name,
        reward_type_field = reward_type.field_name,
    ));
    augmentation.methods.push_str(&format!(
        r#"func (manager *{manager_type}) ByIndex(index uint32) *{row_name} {{
	row, ok := manager.rewardsByIndex[index]
	if !ok {{ return nil }}
	return rowCopy(manager.{row_field}.entries[row].Row)
}}

func (manager *{manager_type}) RewardsByType(rewardType string) iter.Seq[{row_name}] {{
	return func(yield func({row_name}) bool) {{
		for _, index := range manager.rewardsByType[strings.ToLower(strings.TrimSpace(rewardType))] {{
			if !yield(manager.{row_field}.entries[index].Row) {{ return }}
		}}
	}}
}}

func (manager *{manager_type}) Rewards() iter.Seq[{row_name}] {{
	return func(yield func({row_name}) bool) {{
		for index := range manager.{row_field}.entries {{
			if !yield(manager.{row_field}.entries[index].Row) {{ return }}
		}}
	}}
}}

"#
    ));
    augmentation
}

fn typed_table_crc(
    unit: &GameDataCompileUnit,
    manager: &DirectManagerSurface,
    row_type: &str,
    key_column: &str,
    id_method: &str,
    key_method: &str,
    rows_method: &str,
) -> GoNativeManagerAugmentation {
    let row = indexed::required_row(unit, manager, row_type);
    let key = indexed::required_field(&row, key_column);
    let row_field = go_direct_row_field_name(row_type);
    let table_type = go_direct_table_type_name(manager, row_type, true);
    let manager_type = go_method_name(&manager.manager_class_name);
    let row_name = row.type_name.clone();
    let key_type = format!("{manager_type}Key");
    let id_method = go_method_name(id_method);
    let key_method = go_method_name(key_method);
    let rows_method = go_method_name(rows_method);
    GoNativeManagerAugmentation {
        declarations: format!(
            "\ntype {key_type} struct {{ Table {table_type}; ID gametypes.CRC32 }}\n"
        ),
        fields: format!(
            "\trowsByTableAndID map[{key_type}]int\n\trowsByTable map[{table_type}][]int\n"
        ),
        field_values: format!(
            "\t\trowsByTableAndID: make(map[{key_type}]int),\n\t\trowsByTable: make(map[{table_type}][]int),\n"
        ),
        initializers: format!(
            r#"	for index := range manager.{row_field}.entries {{
		source := &manager.{row_field}.entries[index]
		text := strings.TrimSpace(source.Row.{key_field})
		id := gametypes.CRC32(crc32Lowercase(text))
		if text == "" || id == 0 {{ continue }}
		key := {key_type}{{Table: source.Ref.Table(), ID: id}}
		if _, exists := manager.rowsByTableAndID[key]; !exists {{ manager.rowsByTableAndID[key] = index }}
		manager.rowsByTable[source.Ref.Table()] = append(manager.rowsByTable[source.Ref.Table()], index)
	}}
"#,
            key_field = key.field_name,
        ),
        methods: format!(
            r#"func (manager *{manager_type}) {id_method}(table {table_type}, id gametypes.CRC32) *{row_name} {{
	index, ok := manager.rowsByTableAndID[{key_type}{{Table: table, ID: id}}]
	if !ok {{ return nil }}
	return rowCopy(manager.{row_field}.entries[index].Row)
}}

func (manager *{manager_type}) {key_method}(table {table_type}, key string) *{row_name} {{
	return manager.{id_method}(table, gametypes.CRC32(crc32Lowercase(key)))
}}

func (manager *{manager_type}) {rows_method}(table {table_type}) iter.Seq[{row_name}] {{
	return func(yield func({row_name}) bool) {{
		for _, index := range manager.rowsByTable[table] {{
			if !yield(manager.{row_field}.entries[index].Row) {{ return }}
		}}
	}}
}}

"#
        ),
    }
}

fn battle_pass(
    unit: &GameDataCompileUnit,
    manager: &DirectManagerSurface,
) -> GoNativeManagerAugmentation {
    let row = indexed::required_row(unit, manager, "SeasonPassRankData");
    let level = indexed::required_field(&row, "Level");
    let maximum = indexed::required_field(&row, "MaximumInfluence");
    let row_field = go_direct_row_field_name("SeasonPassRankData");
    let table_type = go_direct_table_type_name(manager, "SeasonPassRankData", true);
    let manager_type = go_method_name(&manager.manager_class_name);
    let row_name = row.type_name.clone();
    let key_type = format!("{manager_type}RankKey");
    GoNativeManagerAugmentation {
        declarations: format!(
            r#"
type {key_type} struct {{ SeasonID gametypes.CRC32; Level uint32 }}

func {manager_type}SeasonID(table {table_type}) gametypes.CRC32 {{
	name := string(table)
	separator := strings.LastIndexByte(name, '_')
	if separator < 0 || separator+1 == len(name) {{ return 0 }}
	return gametypes.CRC32(crc32Lowercase(name[separator+1:]))
}}
"#
        ),
        fields: format!(
            "\tranks map[{key_type}]int\n\tranksBySeason map[gametypes.CRC32][]int\n\tmaxRank map[gametypes.CRC32]uint32\n\tmaximumInfluence map[gametypes.CRC32]uint32\n"
        ),
        field_values: format!(
            "\t\tranks: make(map[{key_type}]int),\n\t\tranksBySeason: make(map[gametypes.CRC32][]int),\n\t\tmaxRank: make(map[gametypes.CRC32]uint32),\n\t\tmaximumInfluence: make(map[gametypes.CRC32]uint32),\n"
        ),
        initializers: format!(
            r#"	for index := range manager.{row_field}.entries {{
		source := &manager.{row_field}.entries[index]
		level, ok := exactUint32(source.Row.{level_field})
		if !ok {{ continue }}
		seasonID := {manager_type}SeasonID(source.Ref.Table())
		if seasonID == 0 {{ continue }}
		manager.ranks[{key_type}{{SeasonID: seasonID, Level: level}}] = index
		manager.ranksBySeason[seasonID] = append(manager.ranksBySeason[seasonID], index)
		if level > manager.maxRank[seasonID] {{ manager.maxRank[seasonID] = level }}
		if source.Row.{maximum_field} != nil {{
			if value, ok := exactUint32(*source.Row.{maximum_field}); ok && value > manager.maximumInfluence[seasonID] {{ manager.maximumInfluence[seasonID] = value }}
		}}
	}}
"#,
            level_field = level.field_name,
            maximum_field = maximum.field_name,
        ),
        methods: format!(
            r#"func (manager *{manager_type}) Rank(seasonID gametypes.CRC32, level uint32) *{row_name} {{
	index, ok := manager.ranks[{key_type}{{SeasonID: seasonID, Level: level}}]
	if !ok {{ return nil }}
	return rowCopy(manager.{row_field}.entries[index].Row)
}}

func (manager *{manager_type}) RankBySeasonKey(seasonKey string, level uint32) *{row_name} {{
	return manager.Rank(gametypes.CRC32(crc32Lowercase(seasonKey)), level)
}}

func (manager *{manager_type}) Ranks(seasonID gametypes.CRC32) iter.Seq[{row_name}] {{
	return func(yield func({row_name}) bool) {{
		for _, index := range manager.ranksBySeason[seasonID] {{ if !yield(manager.{row_field}.entries[index].Row) {{ return }} }}
	}}
}}

func (manager *{manager_type}) MaxRankLevel(seasonID gametypes.CRC32) uint32 {{ return manager.maxRank[seasonID] }}
func (manager *{manager_type}) MaximumInfluence(seasonID gametypes.CRC32) uint32 {{ return manager.maximumInfluence[seasonID] }}

"#
        ),
    }
}

fn chapters(
    unit: &GameDataCompileUnit,
    manager: &DirectManagerSurface,
) -> GoNativeManagerAugmentation {
    let row = indexed::required_row(unit, manager, "SeasonsRewardsChapterData");
    let id = indexed::required_field(&row, "ChapterID");
    let kind = indexed::required_field(&row, "ChapterType");
    let chapter_index = indexed::required_field(&row, "ChapterIndex");
    let reward = indexed::required_field(&row, "ChapterRewardID");
    let row_field = go_direct_row_field_name("SeasonsRewardsChapterData");
    let table_type = go_direct_table_type_name(manager, "SeasonsRewardsChapterData", true);
    let manager_type = go_method_name(&manager.manager_class_name);
    let row_name = row.type_name.clone();
    let key_type = format!("{manager_type}ChapterKey");
    let kind_key_type = format!("{manager_type}ChapterKindKey");
    let season_function = format!("{manager_type}SeasonID");
    let id_expression = string_field_expression(id, "source.Row");
    let kind_expression = string_field_expression(kind, "source.Row");
    let reward_expression = string_field_expression(reward, "source.Row");
    let chapter_index_expression = number_field_expression(chapter_index, "source.Row");
    GoNativeManagerAugmentation {
        declarations: format!(
            r#"
type SeasonsChapterKind string
type {key_type} struct {{ SeasonID gametypes.CRC32; ID gametypes.CRC32 }}
type {kind_key_type} struct {{ SeasonID gametypes.CRC32; Kind SeasonsChapterKind; Index uint32 }}

func {season_function}(table {table_type}) gametypes.CRC32 {{
	name := string(table)
	separator := strings.LastIndexByte(name, '_')
	if separator < 0 || separator+1 == len(name) {{ return 0 }}
	return gametypes.CRC32(crc32Lowercase(name[separator+1:]))
}}
"#
        ),
        fields: format!(
            "\tchaptersByID map[{key_type}]int\n\tchaptersByReward map[{key_type}]int\n\tchaptersByKindIndex map[{kind_key_type}]int\n\tchaptersBySeason map[gametypes.CRC32][]int\n"
        ),
        field_values: format!(
            "\t\tchaptersByID: make(map[{key_type}]int),\n\t\tchaptersByReward: make(map[{key_type}]int),\n\t\tchaptersByKindIndex: make(map[{kind_key_type}]int),\n\t\tchaptersBySeason: make(map[gametypes.CRC32][]int),\n"
        ),
        initializers: format!(
            r#"	for index := range manager.{row_field}.entries {{
		source := &manager.{row_field}.entries[index]
		seasonID := {season_function}(source.Ref.Table())
		idText := strings.TrimSpace({id_expression})
		id := gametypes.CRC32(crc32Lowercase(idText))
		if seasonID == 0 || idText == "" || id == 0 {{ continue }}
		key := {key_type}{{SeasonID: seasonID, ID: id}}
		if _, exists := manager.chaptersByID[key]; exists {{ continue }}
		manager.chaptersByID[key] = index
		manager.chaptersBySeason[seasonID] = append(manager.chaptersBySeason[seasonID], index)
		rewardText := strings.TrimSpace({reward_expression})
		rewardID := gametypes.CRC32(crc32Lowercase(rewardText))
		if rewardID != 0 {{
			rewardKey := {key_type}{{SeasonID: seasonID, ID: rewardID}}
			if _, exists := manager.chaptersByReward[rewardKey]; !exists {{ manager.chaptersByReward[rewardKey] = index }}
		}}
		kind := SeasonsChapterKind(strings.TrimSpace({kind_expression}))
		if chapterIndex, ok := exactUint32({chapter_index_expression}); ok && chapterIndex != 0 && kind != "" {{
			kindKey := {kind_key_type}{{SeasonID: seasonID, Kind: kind, Index: chapterIndex}}
			if _, exists := manager.chaptersByKindIndex[kindKey]; !exists {{ manager.chaptersByKindIndex[kindKey] = index }}
		}}
	}}
"#
        ),
        methods: format!(
            r#"func (manager *{manager_type}) Chapter(seasonID gametypes.CRC32, chapterID gametypes.CRC32) *{row_name} {{
	index, ok := manager.chaptersByID[{key_type}{{SeasonID: seasonID, ID: chapterID}}]
	if !ok {{ return nil }}
	return rowCopy(manager.{row_field}.entries[index].Row)
}}

func (manager *{manager_type}) ChapterByKey(seasonKey string, chapterKey string) *{row_name} {{
	return manager.Chapter(gametypes.CRC32(crc32Lowercase(seasonKey)), gametypes.CRC32(crc32Lowercase(chapterKey)))
}}

func (manager *{manager_type}) ChapterByReward(seasonID gametypes.CRC32, rewardID gametypes.CRC32) *{row_name} {{
	index, ok := manager.chaptersByReward[{key_type}{{SeasonID: seasonID, ID: rewardID}}]
	if !ok {{ return nil }}
	return rowCopy(manager.{row_field}.entries[index].Row)
}}

func (manager *{manager_type}) ChapterByKindIndex(seasonID gametypes.CRC32, kind SeasonsChapterKind, chapterIndex uint32) *{row_name} {{
	index, ok := manager.chaptersByKindIndex[{kind_key_type}{{SeasonID: seasonID, Kind: kind, Index: chapterIndex}}]
	if !ok {{ return nil }}
	return rowCopy(manager.{row_field}.entries[index].Row)
}}

func (manager *{manager_type}) Chapters(seasonID gametypes.CRC32) iter.Seq[{row_name}] {{
	return func(yield func({row_name}) bool) {{
		for _, index := range manager.chaptersBySeason[seasonID] {{ if !yield(manager.{row_field}.entries[index].Row) {{ return }} }}
	}}
}}

"#
        ),
    }
}

fn journeys(
    unit: &GameDataCompileUnit,
    manager: &DirectManagerSurface,
) -> GoNativeManagerAugmentation {
    let row = indexed::required_row(unit, manager, "SeasonsRewardsJourneyData");
    let id = indexed::required_field(&row, "JourneyTaskID");
    let chapter = indexed::required_field(&row, "Chapter");
    let reward = indexed::required_field(&row, "RewardID");
    let sort_order = indexed::required_field(&row, "SortOrder");
    let row_field = go_direct_row_field_name("SeasonsRewardsJourneyData");
    let table_type = go_direct_table_type_name(manager, "SeasonsRewardsJourneyData", true);
    let manager_type = go_method_name(&manager.manager_class_name);
    let row_name = row.type_name.clone();
    let key_type = format!("{manager_type}JourneyKey");
    let season_function = format!("{manager_type}SeasonID");
    let id_expression = string_field_expression(id, "source.Row");
    let chapter_expression = string_field_expression(chapter, "source.Row");
    let reward_expression = string_field_expression(reward, "source.Row");
    let sort_expression =
        number_field_expression(sort_order, "manager.{row_field}.entries[left].Row")
            .replace("{row_field}", &row_field);
    let right_sort_expression =
        number_field_expression(sort_order, "manager.{row_field}.entries[right].Row")
            .replace("{row_field}", &row_field);
    GoNativeManagerAugmentation {
        declarations: format!(
            r#"
type {key_type} struct {{ SeasonID gametypes.CRC32; ID gametypes.CRC32 }}

func {season_function}(table {table_type}) gametypes.CRC32 {{
	name := string(table)
	separator := strings.LastIndexByte(name, '_')
	if separator < 0 || separator+1 == len(name) {{ return 0 }}
	return gametypes.CRC32(crc32Lowercase(name[separator+1:]))
}}
"#
        ),
        fields: format!(
            "\tjourneysByID map[{key_type}]int\n\tjourneysByReward map[{key_type}]int\n\tjourneysByChapter map[{key_type}][]int\n\tjourneysBySeason map[gametypes.CRC32][]int\n"
        ),
        field_values: format!(
            "\t\tjourneysByID: make(map[{key_type}]int),\n\t\tjourneysByReward: make(map[{key_type}]int),\n\t\tjourneysByChapter: make(map[{key_type}][]int),\n\t\tjourneysBySeason: make(map[gametypes.CRC32][]int),\n"
        ),
        initializers: format!(
            r#"	for index := range manager.{row_field}.entries {{
		source := &manager.{row_field}.entries[index]
		seasonID := {season_function}(source.Ref.Table())
		idText := strings.TrimSpace({id_expression})
		id := gametypes.CRC32(crc32Lowercase(idText))
		chapterID := gametypes.CRC32(crc32Lowercase(strings.TrimSpace({chapter_expression})))
		rewardID := gametypes.CRC32(crc32Lowercase(strings.TrimSpace({reward_expression})))
		if seasonID == 0 || idText == "" || id == 0 || chapterID == 0 || rewardID == 0 {{ continue }}
		key := {key_type}{{SeasonID: seasonID, ID: id}}
		if _, exists := manager.journeysByID[key]; exists {{ continue }}
		manager.journeysByID[key] = index
		manager.journeysBySeason[seasonID] = append(manager.journeysBySeason[seasonID], index)
		chapterKey := {key_type}{{SeasonID: seasonID, ID: chapterID}}
		manager.journeysByChapter[chapterKey] = append(manager.journeysByChapter[chapterKey], index)
		rewardKey := {key_type}{{SeasonID: seasonID, ID: rewardID}}
		if _, exists := manager.journeysByReward[rewardKey]; !exists {{ manager.journeysByReward[rewardKey] = index }}
	}}
	for seasonID := range manager.journeysBySeason {{
		indexes := manager.journeysBySeason[seasonID]
		sort.SliceStable(indexes, func(left int, right int) bool {{ return {sort_expression} < {right_sort_expression} }})
	}}
	for key := range manager.journeysByChapter {{
		indexes := manager.journeysByChapter[key]
		sort.SliceStable(indexes, func(left int, right int) bool {{ return {sort_expression} < {right_sort_expression} }})
	}}
"#
        ),
        methods: format!(
            r#"func (manager *{manager_type}) JourneyTask(seasonID gametypes.CRC32, journeyTaskID gametypes.CRC32) *{row_name} {{
	index, ok := manager.journeysByID[{key_type}{{SeasonID: seasonID, ID: journeyTaskID}}]
	if !ok {{ return nil }}
	return rowCopy(manager.{row_field}.entries[index].Row)
}}

func (manager *{manager_type}) JourneyTaskByKey(seasonKey string, journeyTaskKey string) *{row_name} {{
	return manager.JourneyTask(gametypes.CRC32(crc32Lowercase(seasonKey)), gametypes.CRC32(crc32Lowercase(journeyTaskKey)))
}}

func (manager *{manager_type}) JourneyTaskByReward(seasonID gametypes.CRC32, rewardID gametypes.CRC32) *{row_name} {{
	index, ok := manager.journeysByReward[{key_type}{{SeasonID: seasonID, ID: rewardID}}]
	if !ok {{ return nil }}
	return rowCopy(manager.{row_field}.entries[index].Row)
}}

func (manager *{manager_type}) Journeys(seasonID gametypes.CRC32) iter.Seq[{row_name}] {{
	return func(yield func({row_name}) bool) {{
		for _, index := range manager.journeysBySeason[seasonID] {{ if !yield(manager.{row_field}.entries[index].Row) {{ return }} }}
	}}
}}

func (manager *{manager_type}) JourneysForChapter(seasonID gametypes.CRC32, chapterID gametypes.CRC32) iter.Seq[{row_name}] {{
	return func(yield func({row_name}) bool) {{
		for _, index := range manager.journeysByChapter[{key_type}{{SeasonID: seasonID, ID: chapterID}}] {{ if !yield(manager.{row_field}.entries[index].Row) {{ return }} }}
	}}
}}

"#
        ),
    }
}

fn song_sheets(
    unit: &GameDataCompileUnit,
    manager: &DirectManagerSurface,
) -> GoNativeManagerAugmentation {
    let mut augmentation = indexed::crc_secondary_contract(
        unit,
        manager,
        "SongBookSheets",
        "SheetID",
        "SheetFromID",
        "Sheet",
        "sheetsByID",
    );
    let row = indexed::required_row(unit, manager, "SongBookSheets");
    let instrument = indexed::required_field(&row, "Instrument");
    let pages = indexed::required_field(&row, "Pages");
    let row_field = go_direct_row_field_name("SongBookSheets");
    let row_name = row.type_name.clone();
    let manager_type = go_method_name(&manager.manager_class_name);
    let instrument_expression = string_field_expression(instrument, "source.Row");
    let pages_expression = string_field_expression(pages, "source.Row");
    augmentation.fields.push_str(
        "\tsheetsByInstrument map[string][]int\n\tsheetIDsByPage map[gametypes.CRC32][]gametypes.CRC32\n\tsheetInstrumentByID map[gametypes.CRC32]string\n\tpageIDsBySheetID map[gametypes.CRC32][]gametypes.CRC32\n",
    );
    augmentation.field_values.push_str(
        "\t\tsheetsByInstrument: make(map[string][]int),\n\t\tsheetIDsByPage: make(map[gametypes.CRC32][]gametypes.CRC32),\n\t\tsheetInstrumentByID: make(map[gametypes.CRC32]string),\n\t\tpageIDsBySheetID: make(map[gametypes.CRC32][]gametypes.CRC32),\n",
    );
    augmentation.initializers.push_str(&format!(
        r#"	for index := range manager.{row_field}.entries {{
		source := &manager.{row_field}.entries[index]
		instrument := strings.ToLower(strings.TrimSpace({instrument_expression}))
		if instrument != "" {{ manager.sheetsByInstrument[instrument] = append(manager.sheetsByInstrument[instrument], index) }}
		sheetKey := strings.TrimSpace(source.Row.SheetID)
		sheetID := gametypes.CRC32(crc32Lowercase(sheetKey))
		if sheetID == 0 {{ continue }}
		manager.sheetInstrumentByID[sheetID] = instrument
		for _, pageKey := range splitDesignerList({pages_expression}) {{
			pageID := gametypes.CRC32(crc32Lowercase(pageKey))
			if pageID == 0 {{ continue }}
			if !slices.Contains(manager.pageIDsBySheetID[sheetID], pageID) {{ manager.pageIDsBySheetID[sheetID] = append(manager.pageIDsBySheetID[sheetID], pageID) }}
			ids := manager.sheetIDsByPage[pageID]
			if !slices.Contains(ids, sheetID) {{ manager.sheetIDsByPage[pageID] = append(ids, sheetID) }}
		}}
	}}
"#
    ));
    augmentation.methods.push_str(&indexed::named_rows_method(
        unit,
        manager,
        "SongBookSheets",
        "Sheets",
    ));
    augmentation.methods.push_str(&format!(
        r#"func (manager *{manager_type}) SheetsForInstrument(instrument string) iter.Seq[{row_name}] {{
	return func(yield func({row_name}) bool) {{
		for _, index := range manager.sheetsByInstrument[strings.ToLower(strings.TrimSpace(instrument))] {{
			if !yield(manager.{row_field}.entries[index].Row) {{ return }}
		}}
	}}
}}

func (manager *{manager_type}) SheetIDsForPage(pageID gametypes.CRC32) iter.Seq[gametypes.CRC32] {{
	return slices.Values(manager.sheetIDsByPage[pageID])
}}

"#
    ));
    augmentation
}

fn songs(
    unit: &GameDataCompileUnit,
    manager: &DirectManagerSurface,
) -> GoNativeManagerAugmentation {
    let mut augmentation = indexed::crc_secondary_contract(
        unit,
        manager,
        "SongBookData",
        "SongID",
        "SongFromID",
        "Song",
        "songsByID",
    );
    let row = indexed::required_row(unit, manager, "SongBookData");
    let row_field = go_direct_row_field_name("SongBookData");
    let manager_type = go_method_name(&manager.manager_class_name);
    let slot_expressions = (1..=5)
        .filter_map(|slot| {
            let column = format!("Slot{slot:02}");
            row.fields
                .iter()
                .find(|field| field.source_name.eq_ignore_ascii_case(&column))
                .map(|field| string_field_expression(field, "source.Row"))
        })
        .collect::<Vec<_>>();
    let slot_initializers = slot_expressions
        .iter()
        .map(|expression| {
            format!(
                r#"		if sheetKey := strings.TrimSpace({expression}); sheetKey != "" {{
			sheetID := gametypes.CRC32(crc32Lowercase(sheetKey))
			if sheetID != 0 && _songBookSheetData.SheetFromID(sheetID) != nil && !slices.Contains(manager.songSheetIDs, sheetID) {{
				manager.songSheetIDs = append(manager.songSheetIDs, sheetID)
			}}
		}}
"#
            )
        })
        .collect::<String>();
    augmentation.fields.push_str(
        "\tsongSheetIDs []gametypes.CRC32\n\tsongPageIDs []gametypes.CRC32\n\tsongSheetIDsByPage map[gametypes.CRC32][]gametypes.CRC32\n\tsongSheetIDsByInstrument map[string][]gametypes.CRC32\n",
    );
    augmentation.field_values.push_str(
        "\t\tsongSheetIDsByPage: make(map[gametypes.CRC32][]gametypes.CRC32),\n\t\tsongSheetIDsByInstrument: make(map[string][]gametypes.CRC32),\n",
    );
    augmentation.initializers.push_str(&format!(
        r#"	for index := range manager.{row_field}.entries {{
		source := &manager.{row_field}.entries[index]
{slot_initializers}	}}
	for _, sheetID := range manager.songSheetIDs {{
		instrument := _songBookSheetData.sheetInstrumentByID[sheetID]
		if instrument != "" {{ manager.songSheetIDsByInstrument[instrument] = append(manager.songSheetIDsByInstrument[instrument], sheetID) }}
		for _, pageID := range _songBookSheetData.pageIDsBySheetID[sheetID] {{
			if !slices.Contains(manager.songPageIDs, pageID) {{ manager.songPageIDs = append(manager.songPageIDs, pageID) }}
			ids := manager.songSheetIDsByPage[pageID]
			if !slices.Contains(ids, sheetID) {{ manager.songSheetIDsByPage[pageID] = append(ids, sheetID) }}
		}}
	}}
"#
    ));
    augmentation.methods.push_str(&indexed::named_rows_method(
        unit,
        manager,
        "SongBookData",
        "Songs",
    ));
    augmentation.methods.push_str(&format!(
        r#"func (manager *{manager_type}) SheetIDs() iter.Seq[gametypes.CRC32] {{ return slices.Values(manager.songSheetIDs) }}
func (manager *{manager_type}) PageIDs() iter.Seq[gametypes.CRC32] {{ return slices.Values(manager.songPageIDs) }}
func (manager *{manager_type}) SheetIDsForPage(pageID gametypes.CRC32) iter.Seq[gametypes.CRC32] {{ return slices.Values(manager.songSheetIDsByPage[pageID]) }}
func (manager *{manager_type}) SheetIDsForInstrument(instrument string) iter.Seq[gametypes.CRC32] {{ return slices.Values(manager.songSheetIDsByInstrument[strings.ToLower(strings.TrimSpace(instrument))]) }}

"#
    ));
    augmentation
}

fn string_field_expression(field: &GoSchemaField, receiver: &str) -> String {
    if field.required {
        format!("{receiver}.{}", field.field_name)
    } else {
        format!("stringValue({receiver}.{})", field.field_name)
    }
}

fn number_field_expression(field: &GoSchemaField, receiver: &str) -> String {
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
