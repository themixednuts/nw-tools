use super::indexed::{
    number_expression, optional_field, required_field, required_row, string_expression,
};
use super::*;
use crate::manager::NativeCrcIndexLookupParameterKind;

pub(super) fn augmentation(
    unit: &GameDataCompileUnit,
    manager: &DirectManagerSurface,
    shape: &NativeManagerShape,
) -> GoNativeManagerAugmentation {
    match shape {
        NativeManagerShape::OneTableWorldEventRule(_) => world_event_rule(unit, manager),
        NativeManagerShape::QuickCourseData(_) => quick_course(unit, manager),
        NativeManagerShape::RotationalQueueData(_) => rotational_queue(unit, manager),
        NativeManagerShape::ProgressionPointData(_) => progression_point(unit, manager),
        NativeManagerShape::OneTablePvpBalance(shape) => {
            qualified_pvp_balance(unit, manager, shape)
        }
        NativeManagerShape::OneTableParticleData(_) => particle(unit, manager),
        _ => panic!(
            "manager {} reached world Go native dispatch with {shape:?}",
            manager.manager_name
        ),
    }
}

fn world_event_rule(
    unit: &GameDataCompileUnit,
    manager: &DirectManagerSurface,
) -> GoNativeManagerAugmentation {
    let row = required_row(unit, manager, "WorldEventRuleData");
    let row_field = go_direct_row_field_name("WorldEventRuleData");
    let manager_type = go_method_name(&manager.manager_class_name);
    let key = string_expression(required_field(&row, "RuleID"), "source.Row");
    let category = string_expression(required_field(&row, "Category"), "source.Row");
    let hub = string_expression(required_field(&row, "Hub"), "source.Row");
    let zone = string_expression(required_field(&row, "Zone"), "source.Row");
    let max_events = number_expression(required_field(&row, "MaxEvents"), "source.Row");
    let min_distance = number_expression(required_field(&row, "MinDistance"), "source.Row");
    let disabled = bool_expression(required_field(&row, "Disabled"), "source.Row");

    GoNativeManagerAugmentation {
        declarations: r#"
type WorldEventCRCFilter struct { Any bool; Values []gametypes.CRC32 }
type WorldEventZoneFilter struct { Any bool; Values []uint16 }
type WorldEventRuleData struct {
	RuleID string
	RuleIDCRC gametypes.CRC32
	MaxEvents uint32
	MinDistance float32
	Category WorldEventCRCFilter
	Hub WorldEventCRCFilter
	Zone WorldEventZoneFilter
	Tags []gametypes.CRC32
	Enabled bool
}
func worldEventCRCFilter(value string) WorldEventCRCFilter { value = strings.TrimSpace(value); if value == "*" { return WorldEventCRCFilter{Any: true} }; out := WorldEventCRCFilter{}; for _, key := range strings.Split(value, ",") { key = strings.TrimSpace(key); if key == "" { continue }; id := gametypes.CRC32(crc32Lowercase(key)); if id != 0 { out.Values = append(out.Values, id) } }; return out }
func worldEventZoneFilter(value string) (WorldEventZoneFilter, bool) { value = strings.TrimSpace(value); if value == "*" { return WorldEventZoneFilter{Any: true}, true }; out := WorldEventZoneFilter{}; for _, token := range strings.Split(value, ",") { token = strings.TrimSpace(token); if token == "" { continue }; parsed, err := strconv.ParseUint(token, 10, 16); if err != nil { return WorldEventZoneFilter{}, false }; out.Values = append(out.Values, uint16(parsed)) }; return out, true }
"#
        .to_owned(),
        fields: "\tworldEventRules []WorldEventRuleData\n\tworldEventRulesByID map[gametypes.CRC32]int\n".to_owned(),
        field_values: "\t\tworldEventRulesByID: make(map[gametypes.CRC32]int),\n".to_owned(),
        initializers: format!(
            r#"	for index := range manager.{row_field}.entries {{
		source := &manager.{row_field}.entries[index]
		key := strings.TrimSpace({key}); id := gametypes.CRC32(crc32Lowercase(key)); if key == "" || id == 0 {{ continue }}
		if _, exists := manager.worldEventRulesByID[id]; exists {{ continue }}
		maxEvents, ok := exactUint32({max_events}); if !ok {{ continue }}
		category := worldEventCRCFilter({category}); hub := worldEventCRCFilter({hub}); zone, zoneOK := worldEventZoneFilter({zone}); if !zoneOK {{ continue }}
		tags := []gametypes.CRC32(nil)
		manager.worldEventRulesByID[id] = len(manager.worldEventRules)
		manager.worldEventRules = append(manager.worldEventRules, WorldEventRuleData{{RuleID: key, RuleIDCRC: id, MaxEvents: maxEvents, MinDistance: {min_distance}, Category: category, Hub: hub, Zone: zone, Tags: tags, Enabled: !({disabled})}})
	}}
"#
        ),
        methods: format!(
            r#"func (manager *{manager_type}) WorldEventRuleByCRC32(id gametypes.CRC32) *WorldEventRuleData {{ index, ok := manager.worldEventRulesByID[id]; if !ok {{ return nil }}; return rowCopy(manager.worldEventRules[index]) }}
func (manager *{manager_type}) WorldEventRule(key string) *WorldEventRuleData {{ return manager.WorldEventRuleByCRC32(gametypes.CRC32(crc32Lowercase(key))) }}
func (manager *{manager_type}) WorldEventRules() iter.Seq[WorldEventRuleData] {{ return rowValues(manager.worldEventRules) }}
func (manager *{manager_type}) Rows() iter.Seq[WorldEventRuleData] {{ return manager.WorldEventRules() }}

"#
        ),
    }
}

fn quick_course(
    unit: &GameDataCompileUnit,
    manager: &DirectManagerSurface,
) -> GoNativeManagerAugmentation {
    let course = required_row(unit, manager, "QuickCourseData");
    let node = required_row(unit, manager, "QuickCourseNodeTypeData");
    let course_field = go_direct_row_field_name("QuickCourseData");
    let node_field = go_direct_row_field_name("QuickCourseNodeTypeData");
    let manager_type = go_method_name(&manager.manager_class_name);
    let course_id = string_expression(required_field(&course, "QuickCourseID"), "source.Row");
    let node_id = string_expression(required_field(&node, "TimedRaceNodeTypeId"), "source.Row");
    let course_path = optional_field(&course, "PathReferenceQuickCourseID")
        .map(|f| string_expression(f, "source.Row"))
        .unwrap_or_else(|| "\"\"".to_owned());
    let is_timed = optional_field(&course, "IsTimed")
        .map(|f| bool_expression(f, "source.Row"))
        .unwrap_or_else(|| "false".to_owned());
    let start = optional_field(&course, "StartingTimerSeconds")
        .map(|f| number_expression(f, "source.Row"))
        .unwrap_or_else(|| "0".to_owned());
    let accumulate = optional_field(&course, "AccumulateTime")
        .map(|f| bool_expression(f, "source.Row"))
        .unwrap_or_else(|| "false".to_owned());
    let multiplier = optional_field(&course, "NodeTimeOverrideMultiplier")
        .map(|f| number_expression(f, "source.Row"))
        .unwrap_or_else(|| "1".to_owned());
    let audio = optional_field(&course, "AudioGroup")
        .map(|f| string_expression(f, "source.Row"))
        .unwrap_or_else(|| "\"\"".to_owned());
    let radius = optional_field(&node, "DetectionRadius")
        .map(|f| number_expression(f, "source.Row"))
        .unwrap_or_else(|| "0".to_owned());
    let use_override = optional_field(&node, "UseTimeOverride")
        .map(|f| bool_expression(f, "source.Row"))
        .unwrap_or_else(|| "false".to_owned());
    let add_time = optional_field(&node, "AddTimeSeconds")
        .map(|f| number_expression(f, "source.Row"))
        .unwrap_or_else(|| "0".to_owned());
    let visual = optional_field(&node, "VisualSlicePath")
        .map(|f| string_expression(f, "source.Row"))
        .unwrap_or_else(|| "\"\"".to_owned());
    let sfx = optional_field(&node, "SFX")
        .map(|f| string_expression(f, "source.Row"))
        .unwrap_or_else(|| "\"\"".to_owned());
    GoNativeManagerAugmentation {
        declarations: r#"
type QuickCourseData struct { ID string; IDCRC gametypes.CRC32; PathReferenceID gametypes.CRC32; IsTimed bool; StartingTimerSeconds uint32; AccumulateTime bool; NodeTimeOverrideMultiplier float32; AudioGroup string }
type QuickCourseNodeTypeData struct { ID string; IDCRC gametypes.CRC32; DetectionRadius float32; UseTimeOverride bool; AddTimeSeconds float32; VisualSlicePath string; SFX string }
"#.to_owned(),
        fields: "\tquickCourses []QuickCourseData\n\tquickCoursesByID map[gametypes.CRC32]int\n\tquickCourseNodes []QuickCourseNodeTypeData\n\tquickCourseNodesByID map[gametypes.CRC32]int\n".to_owned(),
        field_values: "\t\tquickCoursesByID: make(map[gametypes.CRC32]int),\n\t\tquickCourseNodesByID: make(map[gametypes.CRC32]int),\n".to_owned(),
        initializers: format!(r#"	for index := range manager.{course_field}.entries {{ source := &manager.{course_field}.entries[index]; key := strings.TrimSpace({course_id}); id := gametypes.CRC32(crc32Lowercase(key)); if key == "" || id == 0 {{ continue }}; if _, exists := manager.quickCoursesByID[id]; exists {{ continue }}; start, ok := exactUint32({start}); if !ok {{ continue }}; factor := {multiplier}; if factor == 0 {{ factor = 1 }}; manager.quickCoursesByID[id] = len(manager.quickCourses); manager.quickCourses = append(manager.quickCourses, QuickCourseData{{ID:key, IDCRC:id, PathReferenceID:gametypes.CRC32(crc32Lowercase({course_path})), IsTimed:{is_timed}, StartingTimerSeconds:start, AccumulateTime:{accumulate}, NodeTimeOverrideMultiplier:factor, AudioGroup:strings.TrimSpace({audio})}}) }}
	for index := range manager.{node_field}.entries {{ source := &manager.{node_field}.entries[index]; key := strings.TrimSpace({node_id}); id := gametypes.CRC32(crc32Lowercase(key)); if key == "" || id == 0 {{ continue }}; if _, exists := manager.quickCourseNodesByID[id]; exists {{ continue }}; manager.quickCourseNodesByID[id] = len(manager.quickCourseNodes); manager.quickCourseNodes = append(manager.quickCourseNodes, QuickCourseNodeTypeData{{ID:key, IDCRC:id, DetectionRadius:{radius}, UseTimeOverride:{use_override}, AddTimeSeconds:{add_time}, VisualSlicePath:strings.TrimSpace({visual}), SFX:strings.TrimSpace({sfx})}}) }}
"#),
        methods: format!(r#"func (manager *{manager_type}) QuickCourseByCRC32(id gametypes.CRC32) *QuickCourseData {{ index, ok := manager.quickCoursesByID[id]; if !ok {{ return nil }}; return rowCopy(manager.quickCourses[index]) }}
func (manager *{manager_type}) QuickCourse(key string) *QuickCourseData {{ return manager.QuickCourseByCRC32(gametypes.CRC32(crc32Lowercase(key))) }}
func (manager *{manager_type}) NodeTypeByCRC32(id gametypes.CRC32) *QuickCourseNodeTypeData {{ index, ok := manager.quickCourseNodesByID[id]; if !ok {{ return nil }}; return rowCopy(manager.quickCourseNodes[index]) }}
func (manager *{manager_type}) NodeType(key string) *QuickCourseNodeTypeData {{ return manager.NodeTypeByCRC32(gametypes.CRC32(crc32Lowercase(key))) }}
func (manager *{manager_type}) QuickCourses() iter.Seq[QuickCourseData] {{ return rowValues(manager.quickCourses) }}
func (manager *{manager_type}) NodeTypes() iter.Seq[QuickCourseNodeTypeData] {{ return rowValues(manager.quickCourseNodes) }}
func (manager *{manager_type}) Rows() iter.Seq[QuickCourseData] {{ return manager.QuickCourses() }}
func (manager *{manager_type}) FirstQuickCourseID() string {{ if len(manager.quickCourses) == 0 {{ return "" }}; return manager.quickCourses[0].ID }}
func (manager *{manager_type}) FirstNodeTypeID() string {{ if len(manager.quickCourseNodes) == 0 {{ return "" }}; return manager.quickCourseNodes[0].ID }}

"#),
    }
}

fn rotational_queue(
    unit: &GameDataCompileUnit,
    manager: &DirectManagerSurface,
) -> GoNativeManagerAugmentation {
    let row = required_row(unit, manager, "RotationalQueueData");
    let row_field = go_direct_row_field_name("RotationalQueueData");
    let manager_type = go_method_name(&manager.manager_class_name);
    let string = |column: &str| string_expression(required_field(&row, column), "source.Row");
    let id = string("RotationalQueueID");
    let start_time = string("QueueStartTime");
    let end_time = string("QueueEndTime");
    let modes = string("QueueGameModes");
    let notes = string("Notes");
    let start_index = number_expression(required_field(&row, "QueueStartIndex"), "source.Row");
    let span = number_expression(required_field(&row, "GameModeTimeSpan"), "source.Row");
    GoNativeManagerAugmentation { declarations:r#"
type RotationalQueueData struct { ID gametypes.CRC32; Key string; QueueStartIndex uint32; QueueStartTime string; QueueEndTime string; QueueGameModes []gametypes.CRC32; GameModeTimeSpan float32; Notes string }
"#.to_owned(), fields:"\trotationalQueues []RotationalQueueData\n\trotationalQueuesByID map[gametypes.CRC32]int\n".to_owned(), field_values:"\t\trotationalQueuesByID: make(map[gametypes.CRC32]int),\n".to_owned(), initializers:format!(r#"	for index := range manager.{row_field}.entries {{ source := &manager.{row_field}.entries[index]; key := strings.TrimSpace({id}); id := gametypes.CRC32(crc32Lowercase(key)); if key == "" || id == 0 || {span} <= 0 {{ continue }}; gameModes := []gametypes.CRC32{{}}; for _, mode := range splitDesignerList({modes}) {{ modeID := gametypes.CRC32(crc32Lowercase(mode)); if modeID != 0 {{ gameModes = append(gameModes, modeID) }} }}; if len(gameModes) == 0 {{ continue }}; startIndex, ok := exactUint32({start_index}); if !ok {{ continue }}; if _, exists := manager.rotationalQueuesByID[id]; exists {{ continue }}; manager.rotationalQueuesByID[id] = len(manager.rotationalQueues); manager.rotationalQueues = append(manager.rotationalQueues, RotationalQueueData{{ID:id, Key:key, QueueStartIndex:startIndex, QueueStartTime:strings.TrimSpace({start_time}), QueueEndTime:strings.TrimSpace({end_time}), QueueGameModes:gameModes, GameModeTimeSpan:{span}, Notes:strings.TrimSpace({notes})}}) }}
"#), methods:format!(r#"func (manager *{manager_type}) RotationalQueueFromID(id gametypes.CRC32) *RotationalQueueData {{ index, ok := manager.rotationalQueuesByID[id]; if !ok {{ return nil }}; return rowCopy(manager.rotationalQueues[index]) }}
func (manager *{manager_type}) RotationalQueue(key string) *RotationalQueueData {{ return manager.RotationalQueueFromID(gametypes.CRC32(crc32Lowercase(key))) }}
func (manager *{manager_type}) RotationalQueues() iter.Seq[RotationalQueueData] {{ return rowValues(manager.rotationalQueues) }}
func (manager *{manager_type}) Rows() iter.Seq[RotationalQueueData] {{ return manager.RotationalQueues() }}

"#) }
}

fn progression_point(
    unit: &GameDataCompileUnit,
    manager: &DirectManagerSurface,
) -> GoNativeManagerAugmentation {
    let row = required_row(unit, manager, "ProgressionPointData");
    let row_field = go_direct_row_field_name("ProgressionPointData");
    let manager_type = go_method_name(&manager.manager_class_name);
    let s = |column: &str| {
        optional_field(&row, column)
            .map(|f| string_expression(f, "source.Row"))
            .unwrap_or_else(|| "\"\"".to_owned())
    };
    let n = |column: &str| {
        optional_field(&row, column)
            .map(|f| number_expression(f, "source.Row"))
            .unwrap_or_else(|| "0".to_owned())
    };
    let b = |column: &str| {
        optional_field(&row, column)
            .map(|f| bool_expression(f, "source.Row"))
            .unwrap_or_else(|| "false".to_owned())
    };
    let id = s("ProgressionPointID");
    let description = s("Description");
    let pool = s("PointPoolID");
    let pool_category = s("PoolCategory");
    let territory = s("TerritoryBonusCategory");
    let req_cat = s("RequiredCategoricalProgressionID");
    let req_point = s("RequiredProgressionPointID");
    let max = n("MaxLevel");
    let req_level = n("RequiredCharacterLevel");
    let req_cat_level = n("RequiredCategoricalProgressionLevel");
    let req_point_level = n("RequiredProgressionPointLevel");
    let is_ability = b("IsAbility");
    let do_not_spend = b("DoNotSpendPoint");
    GoNativeManagerAugmentation { declarations:r#"
type StaticProgressionPointData struct { SourceRow int; PointID string; PointCRC gametypes.CRC32; Description string; PointPoolID string; PointPoolCRC gametypes.CRC32; IsAbility bool; SpendsPoint bool; PoolCategory string; TerritoryBonus string; MaxLevel uint32; RequiredCharacterLevel uint32; RequiredCategoricalProgressionID string; RequiredCategoricalProgressionCRC gametypes.CRC32; RequiredCategoricalProgressionLevel uint32; RequiredProgressionPointID string; RequiredProgressionPointCRC gametypes.CRC32; RequiredProgressionPointLevel uint32 }
"#.to_owned(), fields:"\tprogressionPoints []StaticProgressionPointData\n\tprogressionPointsByID map[gametypes.CRC32]int\n\tprogressionPointsBySource map[int]int\n".to_owned(), field_values:"\t\tprogressionPointsByID: make(map[gametypes.CRC32]int),\n\t\tprogressionPointsBySource: make(map[int]int),\n".to_owned(), initializers:format!(r#"	for index := range manager.{row_field}.entries {{ source := &manager.{row_field}.entries[index]; key := strings.TrimSpace({id}); id := gametypes.CRC32(crc32Lowercase(key)); if key == "" || id == 0 {{ continue }}; maxLevel, ok := exactUint32({max}); if !ok || maxLevel == 0 {{ continue }}; requiredLevel, ok := exactUint32({req_level}); if !ok {{ continue }}; requiredCategoryLevel, ok := exactUint32({req_cat_level}); if !ok {{ continue }}; requiredPointLevel, ok := exactUint32({req_point_level}); if !ok {{ continue }}; if _, exists := manager.progressionPointsByID[id]; exists {{ continue }}; data := StaticProgressionPointData{{SourceRow:source.Slot.RowIndex(), PointID:key, PointCRC:id, Description:strings.TrimSpace({description}), PointPoolID:strings.TrimSpace({pool}), PointPoolCRC:gametypes.CRC32(crc32Lowercase({pool})), IsAbility:{is_ability}, SpendsPoint:!({do_not_spend}), PoolCategory:strings.TrimSpace({pool_category}), TerritoryBonus:strings.TrimSpace({territory}), MaxLevel:maxLevel, RequiredCharacterLevel:requiredLevel, RequiredCategoricalProgressionID:strings.TrimSpace({req_cat}), RequiredCategoricalProgressionCRC:gametypes.CRC32(crc32Lowercase({req_cat})), RequiredCategoricalProgressionLevel:requiredCategoryLevel, RequiredProgressionPointID:strings.TrimSpace({req_point}), RequiredProgressionPointCRC:gametypes.CRC32(crc32Lowercase({req_point})), RequiredProgressionPointLevel:requiredPointLevel}}; manager.progressionPointsByID[id]=len(manager.progressionPoints); manager.progressionPointsBySource[data.SourceRow]=len(manager.progressionPoints); manager.progressionPoints=append(manager.progressionPoints,data) }}
"#), methods:format!(r#"func (manager *{manager_type}) ProgressionPointFromID(id gametypes.CRC32) *StaticProgressionPointData {{ index, ok := manager.progressionPointsByID[id]; if !ok {{ return nil }}; return rowCopy(manager.progressionPoints[index]) }}
func (manager *{manager_type}) ProgressionPoint(key string) *StaticProgressionPointData {{ return manager.ProgressionPointFromID(gametypes.CRC32(crc32Lowercase(key))) }}
func (manager *{manager_type}) ProgressionPointForSourceRow(row int) *StaticProgressionPointData {{ index, ok := manager.progressionPointsBySource[row]; if !ok {{ return nil }}; return rowCopy(manager.progressionPoints[index]) }}
func (manager *{manager_type}) ProgressionPoints() iter.Seq[StaticProgressionPointData] {{ return rowValues(manager.progressionPoints) }}
func (manager *{manager_type}) Rows() iter.Seq[StaticProgressionPointData] {{ return manager.ProgressionPoints() }}
func (manager *{manager_type}) ProgressionPointIDs() iter.Seq[gametypes.CRC32] {{ return func(yield func(gametypes.CRC32) bool) {{ for index := range manager.progressionPoints {{ if !yield(manager.progressionPoints[index].PointCRC) {{ return }} }} }} }}

"#) }
}

fn pvp_balance(
    unit: &GameDataCompileUnit,
    manager: &DirectManagerSurface,
    shape: &crate::manager::NativeOneTablePvpBalanceManager,
) -> GoNativeManagerAugmentation {
    let row = required_row(unit, manager, shape.row_type_name().as_str());
    let row_field = go_direct_row_field_name(shape.row_type_name().as_str());
    let manager_type = go_method_name(&manager.manager_class_name);
    let target = string_expression(
        required_field(&row, shape.target_column().as_str()),
        "source.Row",
    );
    let category = string_expression(
        required_field(&row, shape.category_column().as_str()),
        "source.Row",
    );
    let str_adjust = |column: &str| {
        optional_field(&row, column)
            .map(|f| string_expression(f, "source.Row"))
            .unwrap_or_else(|| "\"\"".to_owned())
    };
    let optional_num_adjust = |column: &str| {
        optional_field(&row, column)
            .map(|field| match (field.column_type, field.required) {
                (ColumnType::Number, true) => {
                    format!("pvpFloat32({})", number_expression(field, "source.Row"))
                }
                (ColumnType::Number, false) => {
                    format!("pvpOptionalFloat32(source.Row.{})", field.field_name)
                }
                (ColumnType::String, _) => format!(
                    "pvpOptionalFloat32Text({})",
                    string_expression(field, "source.Row")
                ),
                (ColumnType::Boolean, _) => unreachable!("PvP adjustment cannot be boolean"),
            })
            .unwrap_or_else(|| "nil".to_owned())
    };
    let required_num_adjust = |column: &str| {
        let field = required_field(&row, column);
        if field.required {
            (String::new(), number_expression(field, "source.Row"))
        } else {
            (
                format!(
                    "if source.Row.{0} == nil {{ return nil, fmt.Errorf(\"{column} is missing from PvP balance row %d\", source.Slot.RowIndex()+1) }};",
                    field.field_name
                ),
                number_expression(field, "source.Row"),
            )
        }
    };
    let (weapon_guard, weapon) = required_num_adjust("WeaponBaseDamageAdjustment");
    let (self_heal_guard, self_heal) = required_num_adjust("SelfHealAdjustment");
    let (cooldown_guard, cooldown) = required_num_adjust("CooldownAdjustment");
    let mut methods = String::new();
    for method in shape.methods() {
        let name = go_method_name(method.name().as_str());
        match method.parameter().kind(){ NativeCrcIndexLookupParameterKind::Crc32|NativeCrcIndexLookupParameterKind::IntoCrc32=>methods.push_str(&format!("func (manager *{manager_type}) {name}(id gametypes.CRC32) *PvpBalanceData {{ index, ok := manager.pvpBalancesByID[id]; if !ok {{ return nil }}; return rowCopy(manager.pvpBalances[index]) }}\n")), NativeCrcIndexLookupParameterKind::StrRef|NativeCrcIndexLookupParameterKind::AsRefStr=>methods.push_str(&format!("func (manager *{manager_type}) {name}(key string) *PvpBalanceData {{ id := gametypes.CRC32(crc32Lowercase(key)); index, ok := manager.pvpBalancesByID[id]; if !ok {{ return nil }}; return rowCopy(manager.pvpBalances[index]) }}\n")) }
    }
    if let Some(name) = shape.balances_method() {
        methods.push_str(&format!("func (manager *{manager_type}) {}() iter.Seq[PvpBalanceData] {{ return rowValues(manager.pvpBalances) }}\n",go_method_name(name.as_str())));
    }
    methods.push_str(&format!(
        "func (manager *{manager_type}) Rows() iter.Seq[PvpBalanceData] {{ return rowValues(manager.pvpBalances) }}\n"
    ));
    GoNativeManagerAugmentation{declarations:r#"
type PvpBalanceData struct { SourceRow int; Target string; TargetCRC gametypes.CRC32; Category string; AbilityBaseDamage *string; AffixStat *string; IncomingHeal *string; ConsumableHeal *string; Potency *float32; Duration *float32; WeaponBaseDamage float32; SelfHeal float32; Cooldown float32 }
func pvpOptionalText(value string) *string { value = strings.TrimSpace(value); if value == "" { return nil }; return &value }
func pvpFloat32(value float32) *float32 { return &value }
func pvpOptionalFloat32(value *float32) *float32 { if value == nil { return nil }; out := *value; return &out }
func pvpOptionalFloat32Text(value string) *float32 { value = strings.TrimSpace(value); if value == "" { return nil }; parsed, err := strconv.ParseFloat(value, 32); if err != nil { return nil }; out := float32(parsed); return &out }
"#.to_owned(),fields:"\tpvpBalances []PvpBalanceData\n\tpvpBalancesByID map[gametypes.CRC32]int\n".to_owned(),field_values:"\t\tpvpBalancesByID: make(map[gametypes.CRC32]int),\n".to_owned(),initializers:format!(r#"	for index := range manager.{row_field}.entries {{ source:=&manager.{row_field}.entries[index]; key:=strings.TrimSpace({target}); id:=gametypes.CRC32(crc32Lowercase(key)); if key==""||id==0{{continue}}; if _,exists:=manager.pvpBalancesByID[id];exists{{continue}}; {weapon_guard}{self_heal_guard}{cooldown_guard} manager.pvpBalancesByID[id]=len(manager.pvpBalances); manager.pvpBalances=append(manager.pvpBalances,PvpBalanceData{{SourceRow:source.Slot.RowIndex(),Target:key,TargetCRC:id,Category:strings.TrimSpace({category}),AbilityBaseDamage:pvpOptionalText({}),AffixStat:pvpOptionalText({}),IncomingHeal:pvpOptionalText({}),ConsumableHeal:pvpOptionalText({}),Potency:{},Duration:{},WeaponBaseDamage:{weapon},SelfHeal:{self_heal},Cooldown:{cooldown}}}) }}
"#,str_adjust("AbilityBaseDamageAdjustment"),str_adjust("AffixStatAdjustment"),str_adjust("IncomingHealAdjustment"),str_adjust("ConsumableHealAdjustment"),optional_num_adjust("PotencyAdjustment"),optional_num_adjust("DurationAdjustment")),methods}
}

fn qualified_pvp_balance(
    unit: &GameDataCompileUnit,
    manager: &DirectManagerSurface,
    shape: &crate::manager::NativeOneTablePvpBalanceManager,
) -> GoNativeManagerAugmentation {
    let mut augmentation = pvp_balance(unit, manager, shape);
    let manager_type = go_method_name(&manager.manager_class_name);
    let data_type = manager_type
        .strip_suffix("Manager")
        .unwrap_or(&manager_type);
    let helper_prefix = go_local_name(data_type);
    for source in [
        &mut augmentation.declarations,
        &mut augmentation.fields,
        &mut augmentation.initializers,
        &mut augmentation.methods,
    ] {
        *source = source
            .replace("PvpBalanceData", data_type)
            .replace(
                "pvpOptionalFloat32",
                &format!("{helper_prefix}OptionalFloat32"),
            )
            .replace("pvpOptionalText", &format!("{helper_prefix}OptionalText"))
            .replace("pvpFloat32", &format!("{helper_prefix}Float32"));
    }
    augmentation
}

fn particle(
    unit: &GameDataCompileUnit,
    manager: &DirectManagerSurface,
) -> GoNativeManagerAugmentation {
    let row = required_row(unit, manager, "ParticleData");
    let row_field = go_direct_row_field_name("ParticleData");
    let manager_type = go_method_name(&manager.manager_class_name);
    let key = string_expression(required_field(&row, "Effect Name"), "source.Row");
    let group = string_expression(required_field(&row, "Group"), "source.Row");
    let max = number_expression(required_field(&row, "Max Number"), "source.Row");
    let priority = number_expression(required_field(&row, "Priority"), "source.Row");
    let constants = number_expression(required_field(&row, "Constants"), "source.Row");
    GoNativeManagerAugmentation{declarations:r#"
type ParticleData struct { EffectName string; EffectNameCRC gametypes.CRC32; GroupID string; Priority uint32; MaxNumber uint32 }
type ParticleGroupData struct { GroupID string; GroupIDCRC gametypes.CRC32; MaxNumber uint32 }
type ParticleLookup struct { Particle *ParticleData; Group *ParticleGroupData }
"#.to_owned(),fields:"\tparticles []ParticleData\n\tparticlesByID map[gametypes.CRC32]int\n\tparticleGroups []ParticleGroupData\n\tparticleGroupsByID map[gametypes.CRC32]int\n\tlocalPlayerFactor uint32\n\tmaxTotalNumberEmitters uint32\n\tmaxTotalGroupNumberEmitters uint32\n".to_owned(),field_values:"\t\tparticlesByID: make(map[gametypes.CRC32]int),\n\t\tparticleGroupsByID: make(map[gametypes.CRC32]int),\n".to_owned(),initializers:format!(r#"	for index:=range manager.{row_field}.entries{{source:=&manager.{row_field}.entries[index];name:=strings.TrimSpace({key});id:=gametypes.CRC32(crc32Lowercase(name));if name==""||id==0{{continue}};maxValue,ok:=exactUint32({max});if !ok{{continue}};if maxValue==0{{maxValue=math.MaxUint32}};groupID:=strings.TrimSpace({group});if groupID==""{{manager.particleGroupsByID[id]=len(manager.particleGroups);manager.particleGroups=append(manager.particleGroups,ParticleGroupData{{GroupID:name,GroupIDCRC:id,MaxNumber:maxValue}});continue}};priorityValue,ok:=exactUint32({priority});if !ok{{continue}};constantValue,ok:=exactUint32({constants});if !ok{{continue}};if constantValue!=0{{priorityValue=constantValue}};if previous,exists:=manager.particlesByID[id];exists{{manager.particles[previous]=ParticleData{{EffectName:name,EffectNameCRC:id,GroupID:groupID,Priority:priorityValue,MaxNumber:maxValue}};continue}};manager.particlesByID[id]=len(manager.particles);manager.particles=append(manager.particles,ParticleData{{EffectName:name,EffectNameCRC:id,GroupID:groupID,Priority:priorityValue,MaxNumber:maxValue}})}}
	if value:=manager.particleConstant("_LOCAL_PLAYER_FACTOR");value!=nil{{manager.localPlayerFactor=value.Priority}};if value:=manager.particleConstant("_MAX_TOTAL_NUMBER_EMITTERS");value!=nil{{manager.maxTotalNumberEmitters=value.Priority}};if value:=manager.particleConstant("_MAX_TOTAL_GROUP_NUMBER_EMITTERS");value!=nil{{manager.maxTotalGroupNumberEmitters=value.Priority}}
"#),methods:format!(r#"func(manager *{manager_type}) particleConstant(key string)*ParticleData{{index,ok:=manager.particlesByID[gametypes.CRC32(crc32Lowercase(key))];if !ok{{return nil}};return rowCopy(manager.particles[index])}}
func(manager *{manager_type}) ParticleDataFromID(id gametypes.CRC32)*ParticleLookup{{index,ok:=manager.particlesByID[id];if !ok{{return nil}};particle:=rowCopy(manager.particles[index]);lookup:=&ParticleLookup{{Particle:particle}};if groupIndex,ok:=manager.particleGroupsByID[gametypes.CRC32(crc32Lowercase(particle.GroupID))];ok{{lookup.Group=rowCopy(manager.particleGroups[groupIndex])}};return lookup}}
func(manager *{manager_type}) ParticleData(key string)*ParticleLookup{{return manager.ParticleDataFromID(gametypes.CRC32(crc32Lowercase(key)))}}
func(manager *{manager_type}) ParticleDataByKey(key string)*ParticleLookup{{return manager.ParticleData(key)}}
func(manager *{manager_type}) LocalPlayerFactor()uint32{{return manager.localPlayerFactor}}
func(manager *{manager_type}) MaxTotalNumberEmitters()uint32{{return manager.maxTotalNumberEmitters}}
func(manager *{manager_type}) MaxTotalGroupNumberEmitters()uint32{{return manager.maxTotalGroupNumberEmitters}}
func(manager *{manager_type}) Rows()iter.Seq[ParticleData]{{return rowValues(manager.particles)}}

"#)}
}
