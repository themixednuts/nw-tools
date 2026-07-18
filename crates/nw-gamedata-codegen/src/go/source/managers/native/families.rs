use super::*;

pub(super) fn augmentation(
    unit: &GameDataCompileUnit,
    manager: &DirectManagerSurface,
    shape: &NativeManagerShape,
) -> GoNativeManagerAugmentation {
    match shape {
        NativeManagerShape::ObjectivesData(_) => objectives(unit, manager),
        NativeManagerShape::ContributionData(_) => contribution(unit, manager),
        NativeManagerShape::BuffBucketData(_) => buff_bucket(unit, manager),
        NativeManagerShape::StructureData(_) => structure(unit, manager),
        NativeManagerShape::ReusableScoreboardData(_) => reusable_scoreboard(unit, manager),
        NativeManagerShape::MountHitVolumeData(_) => mount_hit_volume(unit, manager),
        _ => panic!(
            "manager {} reached family Go native dispatch with {shape:?}",
            manager.manager_name
        ),
    }
}

fn objectives(
    unit: &GameDataCompileUnit,
    manager: &DirectManagerSurface,
) -> GoNativeManagerAugmentation {
    merge_augmentations([
        indexed::crc_secondary_contract(
            unit,
            manager,
            "Objectives",
            "ObjectiveID",
            "ObjectiveDataFromID",
            "ObjectiveData",
            "objectivesByID",
        ),
        indexed::crc_secondary_contract(
            unit,
            manager,
            "ObjectiveTasks",
            "TaskID",
            "ObjectiveTaskDataFromID",
            "ObjectiveTaskData",
            "objectiveTasksByID",
        ),
        named_rows(unit, manager, "Objectives", "Objectives"),
        named_rows(unit, manager, "ObjectiveTasks", "ObjectiveTasks"),
    ])
}

fn contribution(
    unit: &GameDataCompileUnit,
    manager: &DirectManagerSurface,
) -> GoNativeManagerAugmentation {
    let row = indexed::required_row(unit, manager, "ContributionData");
    let id = indexed::required_field(&row, "ContributionID");
    let category = indexed::required_field(&row, "Category");
    let category_expression = indexed::string_expression(category, "source.Row");
    let row_field = go_direct_row_field_name("ContributionData");
    let table_type = go_direct_table_type_name(manager, "ContributionData", true);
    let manager_type = go_method_name(&manager.manager_class_name);
    let row_name = row.type_name.clone();
    let key_type = format!("{manager_type}Key");
    GoNativeManagerAugmentation {
        declarations: format!(
            "\ntype {key_type} struct {{ Table {table_type}; ContributionID gametypes.CRC32; Category string }}\n"
        ),
        fields: format!("\tcontributions map[{key_type}]int\n"),
        field_values: format!("\t\tcontributions: make(map[{key_type}]int),\n"),
        initializers: format!(
            r#"	for index := range manager.{row_field}.entries {{
		source := &manager.{row_field}.entries[index]
		idText := strings.TrimSpace(source.Row.{id_field})
        category := strings.ToLower(strings.TrimSpace({category_expression}))
		if idText == "" {{ continue }}
		key := {key_type}{{Table: source.Ref.Table(), ContributionID: gametypes.CRC32(crc32Lowercase(idText)), Category: category}}
		manager.contributions[key] = index
	}}
"#,
            id_field = id.field_name,
        ),
        methods: format!(
            r#"func (manager *{manager_type}) ContributionData(table {table_type}, contributionID gametypes.CRC32, category string) *{row_name} {{
	key := {key_type}{{Table: table, ContributionID: contributionID, Category: strings.ToLower(strings.TrimSpace(category))}}
	index, ok := manager.contributions[key]
	if !ok {{ return nil }}
	return rowCopy(manager.{row_field}.entries[index].Row)
}}

func (manager *{manager_type}) ContributionDataByKey(table {table_type}, contributionID string, category string) *{row_name} {{
	return manager.ContributionData(table, gametypes.CRC32(crc32Lowercase(contributionID)), category)
}}

"#
        ),
    }
}

fn buff_bucket(
    unit: &GameDataCompileUnit,
    manager: &DirectManagerSurface,
) -> GoNativeManagerAugmentation {
    let row = indexed::required_row(unit, manager, "BuffBucketData");
    let bucket_id = indexed::required_field(&row, "BuffBucketID");
    let table_type_field = indexed::required_field(&row, "TableType");
    let max_roll = indexed::required_field(&row, "MaxRoll");
    let row_field = go_direct_row_field_name("BuffBucketData");
    let table_type = go_direct_table_type_name(manager, "BuffBucketData", true);
    let row_name = row.type_name.clone();
    let manager_type = go_method_name(&manager.manager_class_name);
    let bucket_id_expression = string_expression(bucket_id, "source.Row");
    let table_type_expression = string_expression(table_type_field, "source.Row");
    let max_roll_expression = number_expression(max_roll, "probability.Row");
    let mut slot_initializers = String::new();
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
        let buff_expression = string_expression(buff, "source.Row");
        let probability_expression = string_expression(buff, "probability.Row");
        let kind_expression = string_expression(kind, "source.Row");
        let potency_expression = number_expression(potency, "source.Row");
        slot_initializers.push_str(&format!(
            r#"		buff{slot}Key := strings.TrimSpace({buff_expression})
		if buff{slot}Key != "" {{
			kind{slot} := BuffBucketEntryKind(strings.TrimSpace({kind_expression}))
			if !kind{slot}.Valid() {{ malformed = true }} else {{
				threshold, err := strconv.ParseUint(strings.TrimSpace({probability_expression}), 10, 32)
				if err != nil {{ malformed = true }} else {{
					data.Entries = append(data.Entries, BuffBucketEntry{{Slot: {slot}, RollThreshold: uint32(threshold), BuffKey: buff{slot}Key, BuffID: gametypes.CRC32(crc32Lowercase(buff{slot}Key)), Kind: kind{slot}, Potency: {potency_expression}}})
				}}
			}}
		}}
"#
        ));
    }
    GoNativeManagerAugmentation {
        declarations: format!(
            r#"
type BuffBucketTableType string
const (
	BuffBucketTableTypeAnd BuffBucketTableType = "AND"
	BuffBucketTableTypeOr BuffBucketTableType = "OR"
)

type BuffBucketEntryKind string
const (
	BuffBucketEntryStatusEffect BuffBucketEntryKind = "StatusEffect"
	BuffBucketEntryAbility BuffBucketEntryKind = "Ability"
	BuffBucketEntryBuffBucket BuffBucketEntryKind = "BuffBucket"
	BuffBucketEntryPromotion BuffBucketEntryKind = "Promotion"
)
func (kind BuffBucketEntryKind) Valid() bool {{ return kind == BuffBucketEntryStatusEffect || kind == BuffBucketEntryAbility || kind == BuffBucketEntryBuffBucket || kind == BuffBucketEntryPromotion }}
func (kind BuffBucketEntryKind) IsBucket() bool {{ return kind == BuffBucketEntryBuffBucket }}

type BuffBucketEntry struct {{ Slot uint8; RollThreshold uint32; BuffKey string; BuffID gametypes.CRC32; Kind BuffBucketEntryKind; Potency float32 }}
type BuffBucketData struct {{
	Source RowRef[{table_type}, {row_name}]
	ProbabilitySource RowRef[{table_type}, {row_name}]
	BucketKey string
	BucketID gametypes.CRC32
	TableType BuffBucketTableType
	MaxRoll uint32
	Entries []BuffBucketEntry
}}
"#
        ),
        fields: "\tbuffBuckets []BuffBucketData\n\tbuffBucketsByID map[gametypes.CRC32]int\n\tbuffBucketSourceRows map[string]int\n".to_owned(),
        field_values: "\t\tbuffBucketsByID: make(map[gametypes.CRC32]int),\n\t\tbuffBucketSourceRows: make(map[string]int),\n".to_owned(),
        initializers: format!(
            r#"	for index := range manager.{row_field}.entries {{
		source := &manager.{row_field}.entries[index]
		key := strings.TrimSpace({bucket_id_expression})
		if key != "" {{ if _, exists := manager.buffBucketSourceRows[key]; !exists {{ manager.buffBucketSourceRows[key] = index }} }}
	}}
	for index := range manager.{row_field}.entries {{
		source := &manager.{row_field}.entries[index]
		key := strings.TrimSpace({bucket_id_expression})
		if key == "" || strings.HasSuffix(key, "_Probs") {{ continue }}
		id := gametypes.CRC32(crc32Lowercase(key))
		if id == 0 {{ continue }}
		if _, exists := manager.buffBucketsByID[id]; exists {{ continue }}
		kind := BuffBucketTableType(strings.TrimSpace({table_type_expression}))
		if kind != BuffBucketTableTypeAnd && kind != BuffBucketTableTypeOr {{ continue }}
		probabilityIndex, exists := manager.buffBucketSourceRows[key+"_Probs"]
		if !exists {{ continue }}
		probability := &manager.{row_field}.entries[probabilityIndex]
		maxRoll, ok := exactUint32({max_roll_expression})
		if !ok {{ continue }}
		data := BuffBucketData{{Source: source.Ref, ProbabilitySource: probability.Ref, BucketKey: key, BucketID: id, TableType: kind, MaxRoll: maxRoll}}
		malformed := false
{slot_initializers}		if malformed {{ continue }}
		sort.SliceStable(data.Entries, func(left int, right int) bool {{ return data.Entries[left].RollThreshold < data.Entries[right].RollThreshold }})
		manager.buffBucketsByID[id] = len(manager.buffBuckets)
		manager.buffBuckets = append(manager.buffBuckets, data)
	}}
"#
        ),
        methods: format!(
            r#"func (manager *{manager_type}) BuffBucketDataFromID(id gametypes.CRC32) *BuffBucketData {{
	index, ok := manager.buffBucketsByID[id]
	if !ok {{ return nil }}
	return rowCopy(manager.buffBuckets[index])
}}

func (manager *{manager_type}) BuffBucketData(key string) *BuffBucketData {{ return manager.BuffBucketDataFromID(gametypes.CRC32(crc32Lowercase(key))) }}

func (manager *{manager_type}) VisitAllBuffsFromID(id gametypes.CRC32) iter.Seq[BuffBucketEntry] {{
	return func(yield func(BuffBucketEntry) bool) {{ manager.visitAllBuffs(id, make(map[gametypes.CRC32]struct{{}}), yield) }}
}}

func (manager *{manager_type}) VisitAllBuffs(key string) iter.Seq[BuffBucketEntry] {{ return manager.VisitAllBuffsFromID(gametypes.CRC32(crc32Lowercase(key))) }}

func (manager *{manager_type}) visitAllBuffs(id gametypes.CRC32, active map[gametypes.CRC32]struct{{}}, yield func(BuffBucketEntry) bool) bool {{
	if _, cycle := active[id]; cycle {{ return true }}
	bucket := manager.BuffBucketDataFromID(id)
	if bucket == nil {{ return true }}
	active[id] = struct{{}}{{}}
	defer delete(active, id)
	for index := range bucket.Entries {{
		entry := bucket.Entries[index]
		if entry.Kind.IsBucket() {{
			if !manager.visitAllBuffs(entry.BuffID, active, yield) {{ return false }}
		}} else if !yield(entry) {{ return false }}
	}}
	return true
}}

func (manager *{manager_type}) BuffBuckets() iter.Seq[BuffBucketData] {{ return rowValues(manager.buffBuckets) }}
func (manager *{manager_type}) Rows() iter.Seq[BuffBucketData] {{ return manager.BuffBuckets() }}

"#
        ),
    }
}

fn structure(
    unit: &GameDataCompileUnit,
    manager: &DirectManagerSurface,
) -> GoNativeManagerAugmentation {
    merge_augmentations([
        indexed::crc_secondary_contract(
            unit,
            manager,
            "StructureFootprintData",
            "FootprintID",
            "StructureFootprintDataFromID",
            "StructureFootprintData",
            "footprintsByID",
        ),
        indexed::crc_secondary_contract(
            unit,
            manager,
            "StructurePieceData",
            "StructurePieceID",
            "StructurePieceDataFromID",
            "StructurePieceData",
            "piecesByID",
        ),
        named_rows(unit, manager, "StructureFootprintData", "Footprints"),
        named_rows(unit, manager, "StructurePieceData", "Pieces"),
    ])
}

fn reusable_scoreboard(
    unit: &GameDataCompileUnit,
    manager: &DirectManagerSurface,
) -> GoNativeManagerAugmentation {
    merge_augmentations([
        indexed::crc_secondary_contract(
            unit,
            manager,
            "ReusableScoreboardTabData",
            "ReusableScoreboardTabId",
            "ReusableScoreboardDataFromID",
            "ReusableScoreboardData",
            "scoreboardsByID",
        ),
        named_rows(unit, manager, "ReusableScoreboardTabData", "Scoreboards"),
    ])
}

fn mount_hit_volume(
    unit: &GameDataCompileUnit,
    manager: &DirectManagerSurface,
) -> GoNativeManagerAugmentation {
    let mut augmentation = indexed::crc_secondary_contract(
        unit,
        manager,
        "MountTypeData",
        "MountID",
        "MountHitVolumeFromMountTypeID",
        "MountHitVolume",
        "mountsByID",
    );
    let prefab_column = [
        "MountHitVolumePrefab",
        "PrefabPath",
        "Prefab",
        "MountPrefabPath",
    ]
    .into_iter()
    .find(|column| {
        indexed::required_row(unit, manager, "MountTypeData")
            .fields
            .iter()
            .any(|field| field.source_name.eq_ignore_ascii_case(column))
    });
    if let Some(prefab_column) = prefab_column {
        let row = indexed::required_row(unit, manager, "MountTypeData");
        let prefab = indexed::required_field(&row, prefab_column);
        let row_field = go_direct_row_field_name("MountTypeData");
        let row_name = row.type_name.clone();
        let manager_type = go_method_name(&manager.manager_class_name);
        let expression = string_expression(prefab, "manager.{row_field}.entries[index].Row")
            .replace("{row_field}", &row_field);
        augmentation
            .fields
            .push_str("\tmountsByPrefab map[gametypes.CRC32][]int\n");
        augmentation
            .field_values
            .push_str("\t\tmountsByPrefab: make(map[gametypes.CRC32][]int),\n");
        augmentation.initializers.push_str(&format!(
            "\tfor index := range manager.{row_field}.entries {{ key := gametypes.CRC32(crc32Lowercase(strings.TrimSpace({expression}))); if key != 0 {{ manager.mountsByPrefab[key] = append(manager.mountsByPrefab[key], index) }} }}\n"
        ));
        augmentation.methods.push_str(&format!(
            r#"func (manager *{manager_type}) MountHitVolumesForPrefabFromID(prefabID gametypes.CRC32) iter.Seq[{row_name}] {{
	return func(yield func({row_name}) bool) {{ for _, index := range manager.mountsByPrefab[prefabID] {{ if !yield(manager.{row_field}.entries[index].Row) {{ return }} }} }}
}}
func (manager *{manager_type}) MountHitVolumesForPrefab(prefab string) iter.Seq[{row_name}] {{ return manager.MountHitVolumesForPrefabFromID(gametypes.CRC32(crc32Lowercase(prefab))) }}

"#
        ));
    }
    augmentation
}

fn named_rows(
    unit: &GameDataCompileUnit,
    manager: &DirectManagerSurface,
    row_type: &str,
    method: &str,
) -> GoNativeManagerAugmentation {
    GoNativeManagerAugmentation {
        methods: indexed::named_rows_method(unit, manager, row_type, method),
        ..Default::default()
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
