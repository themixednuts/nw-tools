use super::indexed::{
    number_expression, optional_field, required_field, required_row, string_expression,
};
use super::*;

pub(super) fn augmentation(
    unit: &GameDataCompileUnit,
    manager: &DirectManagerSurface,
    shape: &NativeManagerShape,
) -> GoNativeManagerAugmentation {
    match shape {
        NativeManagerShape::AbilityData(_) => ability(unit, manager),
        NativeManagerShape::VitalsData(_) => vitals(unit, manager),
        NativeManagerShape::StatusEffectData(_) => status_effect(unit, manager),
        NativeManagerShape::ItemConversionData(_) => item_conversion(unit, manager),
        NativeManagerShape::ItemTransformData(_) => item_transform(unit, manager),
        _ => panic!(
            "manager {} reached core Go native dispatch with {shape:?}",
            manager.manager_name
        ),
    }
}

fn vitals(
    unit: &GameDataCompileUnit,
    manager: &DirectManagerSurface,
) -> GoNativeManagerAugmentation {
    let row = required_row(unit, manager, "VitalsLevelVariantData");
    let row_field = go_direct_row_field_name("VitalsLevelVariantData");
    let manager_type = go_method_name(&manager.manager_class_name);
    let default = go_direct_default_row_spec(unit, manager)
        .map(|row| row.source_row_type)
        .as_deref()
        == Some("VitalsLevelVariantData");
    let table_type = go_direct_table_type_name(manager, "VitalsLevelVariantData", default);
    let key = string_expression(required_field(&row, "VitalsID"), "source.Row");
    let base = string_expression(required_field(&row, "BaseVitalsID"), "source.Row");
    GoNativeManagerAugmentation {
        declarations: format!(
            r#"
type VitalsLevelVariantDataHandle struct {{ Table {table_type}; Row int }}
type VitalsLevelVariantData struct {{ Handle VitalsLevelVariantDataHandle; Table {table_type}; RowIndex int; Key string; ID gametypes.CRC32; BaseVitalsKey string; BaseVitalsID gametypes.CRC32; Source RowRef[{table_type}, {row_type}] }}
"#,
            row_type = row.type_name
        ),
        fields: "\tvitals []VitalsLevelVariantData\n\tvitalsByID map[gametypes.CRC32]int\n\tvitalsByHandle map[VitalsLevelVariantDataHandle]int\n\tcreatureTypeIDs []gametypes.CRC32\n\tcreatureTypeIDSet map[gametypes.CRC32]struct{}\n".to_owned(),
        field_values: "\t\tvitalsByID:make(map[gametypes.CRC32]int),\n\t\tvitalsByHandle:make(map[VitalsLevelVariantDataHandle]int),\n\t\tcreatureTypeIDSet:make(map[gametypes.CRC32]struct{}),\n".to_owned(),
        initializers: format!(
            r#"	for index:=range manager.{row_field}.entries{{source:=&manager.{row_field}.entries[index];key:=strings.TrimSpace({key});id:=gametypes.CRC32(crc32Lowercase(key));baseKey:=strings.TrimSpace({base});baseID:=gametypes.CRC32(crc32Lowercase(baseKey));if key==""||id==0||baseKey==""||baseID==0{{continue}};handle:=VitalsLevelVariantDataHandle{{Table:source.Ref.Table(),Row:source.Slot.RowIndex()}};if _,exists:=manager.vitalsByHandle[handle];exists{{continue}};data:=VitalsLevelVariantData{{Handle:handle,Table:source.Ref.Table(),RowIndex:source.Slot.RowIndex(),Key:key,ID:id,BaseVitalsKey:baseKey,BaseVitalsID:baseID,Source:source.Ref}};dataIndex:=len(manager.vitals);manager.vitalsByHandle[handle]=dataIndex;if _,exists:=manager.vitalsByID[id];!exists{{manager.vitalsByID[id]=dataIndex}};manager.vitals=append(manager.vitals,data);baseData:=_vitalsBaseData.VitalsBaseDataFromID(baseID);if baseData==nil||baseData.CreatureTypeCRC==0{{continue}};if _,exists:=manager.creatureTypeIDSet[baseData.CreatureTypeCRC];!exists{{manager.creatureTypeIDSet[baseData.CreatureTypeCRC]=struct{{}}{{}};manager.creatureTypeIDs=append(manager.creatureTypeIDs,baseData.CreatureTypeCRC)}}}}
"#
        ),
        methods: format!(
            r#"func(manager *{manager_type}) Get(id gametypes.CRC32)*VitalsLevelVariantData{{index,ok:=manager.vitalsByID[id];if !ok{{return nil}};return rowCopy(manager.vitals[index])}}
func(manager *{manager_type}) ByKey(key string)*VitalsLevelVariantData{{return manager.Get(gametypes.CRC32(crc32Lowercase(key)))}}
func(manager *{manager_type}) VitalsForSource(handle VitalsLevelVariantDataHandle)*VitalsLevelVariantData{{index,ok:=manager.vitalsByHandle[handle];if !ok{{return nil}};return rowCopy(manager.vitals[index])}}
func(manager *{manager_type}) Vitals()iter.Seq[VitalsLevelVariantData]{{return rowValues(manager.vitals)}}
func(manager *{manager_type}) Rows()iter.Seq[VitalsLevelVariantData]{{return manager.Vitals()}}
func(manager *{manager_type}) CreatureTypeIDs()iter.Seq[gametypes.CRC32]{{return slices.Values(manager.creatureTypeIDs)}}

"#
        ),
    }
}

fn ability(
    unit: &GameDataCompileUnit,
    manager: &DirectManagerSurface,
) -> GoNativeManagerAugmentation {
    let row = required_row(unit, manager, "AbilityData");
    let row_field = go_direct_row_field_name("AbilityData");
    let manager_type = go_method_name(&manager.manager_class_name);
    let default = go_direct_default_row_spec(unit, manager)
        .map(|r| r.source_row_type)
        .as_deref()
        == Some("AbilityData");
    let table_type = go_direct_table_type_name(manager, "AbilityData", default);
    let key = string_expression(required_field(&row, "AbilityID"), "source.Row");
    let tree_parse = exact_u8_parse(required_field(&row, "TreeID"), "source.Row", "tree");
    let tree_row_parse = exact_u8_parse(
        required_field(&row, "TreeRowPosition"),
        "source.Row",
        "treeRow",
    );
    GoNativeManagerAugmentation{declarations:format!(r#"
type AbilityDataHandle struct {{ Table {table_type}; Row int }}
type AbilityDataPosition struct {{ Table {table_type}; Position uint16 }}
type AbilityData struct {{ Source RowRef[{table_type}, {row_type}]; Table {table_type}; TablePosition uint16; AbilityID string; AbilityCRC gametypes.CRC32; TreeID uint8; TreeRowPosition uint8 }}
type AbilityDataTableKey struct {{ Table {table_type}; ID gametypes.CRC32 }}
"#,row_type=row.type_name),fields:"\tabilities []AbilityData\n\tabilitiesByID map[gametypes.CRC32]int\n\tabilitiesByTableAndID map[AbilityDataTableKey]int\n\tabilitiesByPosition map[AbilityDataPosition]int\n\tabilitiesBySource map[AbilityDataHandle]int\n\tabilityMaxTreeRow map[uint8]uint8\n".to_owned(),field_values:"\t\tabilitiesByID:make(map[gametypes.CRC32]int),\n\t\tabilitiesByTableAndID:make(map[AbilityDataTableKey]int),\n\t\tabilitiesByPosition:make(map[AbilityDataPosition]int),\n\t\tabilitiesBySource:make(map[AbilityDataHandle]int),\n\t\tabilityMaxTreeRow:make(map[uint8]uint8),\n".to_owned(),initializers:format!(r#"	for index:=range manager.{row_field}.entries{{source:=&manager.{row_field}.entries[index];key:=strings.TrimSpace({key});id:=gametypes.CRC32(crc32Lowercase(key));if key==""||id==0{{continue}};{tree_parse}{tree_row_parse}position:=uint16(source.Slot.RowIndex()+1);handle:=AbilityDataHandle{{Table:source.Ref.Table(),Row:source.Slot.RowIndex()}};data:=AbilityData{{Source:source.Ref,Table:source.Ref.Table(),TablePosition:position,AbilityID:key,AbilityCRC:id,TreeID:tree,TreeRowPosition:treeRow}};dataIndex:=len(manager.abilities);manager.abilities=append(manager.abilities,data);if _,exists:=manager.abilitiesByID[id];!exists{{manager.abilitiesByID[id]=dataIndex}};tableKey:=AbilityDataTableKey{{Table:data.Table,ID:id}};if _,exists:=manager.abilitiesByTableAndID[tableKey];!exists{{manager.abilitiesByTableAndID[tableKey]=dataIndex}};pos:=AbilityDataPosition{{Table:data.Table,Position:position}};if _,exists:=manager.abilitiesByPosition[pos];!exists{{manager.abilitiesByPosition[pos]=dataIndex}};manager.abilitiesBySource[handle]=dataIndex;if current,exists:=manager.abilityMaxTreeRow[data.TreeID];!exists||data.TreeRowPosition>current{{manager.abilityMaxTreeRow[data.TreeID]=data.TreeRowPosition}}}}
"#),methods:format!(r#"func(manager *{manager_type}) AbilityDataFromID(id gametypes.CRC32)*AbilityData{{index,ok:=manager.abilitiesByID[id];if !ok{{return nil}};return rowCopy(manager.abilities[index])}}
func(manager *{manager_type}) AbilityData(key string)*AbilityData{{return manager.AbilityDataFromID(gametypes.CRC32(crc32Lowercase(key)))}}
func(manager *{manager_type}) AbilityDataForTable(table {table_type},id gametypes.CRC32)*AbilityData{{index,ok:=manager.abilitiesByTableAndID[AbilityDataTableKey{{Table:table,ID:id}}];if !ok{{return nil}};return rowCopy(manager.abilities[index])}}
func(manager *{manager_type}) AbilityDataByKeyForTable(table {table_type},key string)*AbilityData{{return manager.AbilityDataForTable(table,gametypes.CRC32(crc32Lowercase(key)))}}
func(manager *{manager_type}) AbilityDataAtPosition(position AbilityDataPosition)*AbilityData{{index,ok:=manager.abilitiesByPosition[position];if !ok{{return nil}};return rowCopy(manager.abilities[index])}}
func(manager *{manager_type}) AbilityDataForSourceHandle(handle AbilityDataHandle)*AbilityData{{index,ok:=manager.abilitiesBySource[handle];if !ok{{return nil}};return rowCopy(manager.abilities[index])}}
func(manager *{manager_type}) MaxTreeRowPosition(treeID uint8)(uint8,bool){{value,ok:=manager.abilityMaxTreeRow[treeID];return value,ok}}
func(manager *{manager_type}) AbilityIDs()iter.Seq[gametypes.CRC32]{{return func(yield func(gametypes.CRC32)bool){{for index:=range manager.abilities{{if !yield(manager.abilities[index].AbilityCRC){{return}}}}}}}}
func(manager *{manager_type}) Abilities()iter.Seq[AbilityData]{{return rowValues(manager.abilities)}}
func(manager *{manager_type}) Rows()iter.Seq[AbilityData]{{return manager.Abilities()}}

"#)}
}

fn exact_u8_parse(field: &GoSchemaField, receiver: &str, variable: &str) -> String {
    match field.column_type {
        ColumnType::String => {
            let value = string_expression(field, receiver);
            format!(
                "parsed{variable},err:=strconv.ParseUint(strings.TrimSpace({value}),10,8);if err!=nil{{continue}};{variable}:=uint8(parsed{variable});"
            )
        }
        ColumnType::Number => {
            let value = number_expression(field, receiver);
            format!(
                "parsed{variable},ok:=exactUint32({value});if !ok||parsed{variable}>255{{continue}};{variable}:=uint8(parsed{variable});"
            )
        }
        ColumnType::Boolean => {
            panic!("{} cannot be decoded as a u8", field.source_name)
        }
    }
}

fn status_effect(
    unit: &GameDataCompileUnit,
    manager: &DirectManagerSurface,
) -> GoNativeManagerAugmentation {
    let row = required_row(unit, manager, "StatusEffectData");
    let row_field = go_direct_row_field_name("StatusEffectData");
    let manager_type = go_method_name(&manager.manager_class_name);
    let default = go_direct_default_row_spec(unit, manager)
        .map(|r| r.source_row_type)
        .as_deref()
        == Some("StatusEffectData");
    let table_type = go_direct_table_type_name(manager, "StatusEffectData", default);
    let key = string_expression(required_field(&row, "StatusID"), "source.Row");
    let categories = optional_field(&row, "EffectCategories")
        .or_else(|| optional_field(&row, "EffectCategory"))
        .map(|f| string_expression(f, "source.Row"))
        .unwrap_or_else(|| "\"\"".to_owned());
    let priority = optional_field(&row, "UIPriority")
        .map(|f| number_expression(f, "source.Row"))
        .unwrap_or_else(|| "0".to_owned());
    GoNativeManagerAugmentation{declarations:format!(r#"
type StatusEffectDataHandle struct {{ Table {table_type}; Row int }}
type StatusEffectDataTableKey struct {{ Table {table_type}; ID gametypes.CRC32 }}
type StatusEffectData struct {{ Handle StatusEffectDataHandle; Table {table_type}; RowIndex int; Key string; ID gametypes.CRC32; EffectCategories []string; EffectCategoryIDs []gametypes.CRC32; UIPriority int32; Source RowRef[{table_type}, {row_type}] }}
"#,row_type=row.type_name),fields:"\tstatusEffects []StatusEffectData\n\tstatusEffectsByID map[gametypes.CRC32]int\n\tstatusEffectsByTableAndID map[StatusEffectDataTableKey]int\n\tstatusEffectsByHandle map[StatusEffectDataHandle]int\n".to_owned(),field_values:"\t\tstatusEffectsByID:make(map[gametypes.CRC32]int),\n\t\tstatusEffectsByTableAndID:make(map[StatusEffectDataTableKey]int),\n\t\tstatusEffectsByHandle:make(map[StatusEffectDataHandle]int),\n".to_owned(),initializers:format!(r#"	for index:=range manager.{row_field}.entries{{source:=&manager.{row_field}.entries[index];key:=strings.TrimSpace({key});id:=gametypes.CRC32(crc32Lowercase(key));if key==""||id==0{{continue}};handle:=StatusEffectDataHandle{{Table:source.Ref.Table(),Row:source.Slot.RowIndex()}};tableKey:=StatusEffectDataTableKey{{Table:source.Ref.Table(),ID:id}};if _,exists:=manager.statusEffectsByTableAndID[tableKey];exists{{continue}};priority,ok:=exactUint32({priority});if !ok||priority>math.MaxInt32{{priority=0}};data:=StatusEffectData{{Handle:handle,Table:source.Ref.Table(),RowIndex:source.Slot.RowIndex(),Key:key,ID:id,UIPriority:int32(priority),Source:source.Ref}};for _,category:=range splitDesignerList({categories}){{categoryID:=gametypes.CRC32(crc32Lowercase(category));if categoryID!=0{{data.EffectCategories=append(data.EffectCategories,category);data.EffectCategoryIDs=append(data.EffectCategoryIDs,categoryID)}}}};dataIndex:=len(manager.statusEffects);manager.statusEffectsByTableAndID[tableKey]=dataIndex;manager.statusEffectsByHandle[handle]=dataIndex;if _,exists:=manager.statusEffectsByID[id];!exists{{manager.statusEffectsByID[id]=dataIndex}};manager.statusEffects=append(manager.statusEffects,data)}}
	sort.SliceStable(manager.statusEffects,func(left,right int)bool{{return manager.statusEffects[left].Key<manager.statusEffects[right].Key}})
"#),methods:format!(r#"func(manager *{manager_type}) StatusEffectData(handle StatusEffectDataHandle)*StatusEffectData{{index,ok:=manager.statusEffectsByHandle[handle];if !ok{{return nil}};return rowCopy(manager.statusEffects[index])}}
func(manager *{manager_type}) StatusEffectDataFromID(id gametypes.CRC32)*StatusEffectData{{index,ok:=manager.statusEffectsByID[id];if !ok{{return nil}};return rowCopy(manager.statusEffects[index])}}
func(manager *{manager_type}) StatusEffectDataByID(id gametypes.CRC32)*StatusEffectData{{return manager.StatusEffectDataFromID(id)}}
func(manager *{manager_type}) StatusEffectDataByName(key string)*StatusEffectData{{return manager.StatusEffectDataFromID(gametypes.CRC32(crc32Lowercase(key)))}}
func(manager *{manager_type}) TryStatusEffectDataByID(id gametypes.CRC32)*StatusEffectData{{return manager.StatusEffectDataFromID(id)}}
func(manager *{manager_type}) TryStatusEffectDataByName(key string)*StatusEffectData{{return manager.StatusEffectDataByName(key)}}
func(manager *{manager_type}) StatusEffectDataInTable(table {table_type},id gametypes.CRC32)*StatusEffectData{{index,ok:=manager.statusEffectsByTableAndID[StatusEffectDataTableKey{{Table:table,ID:id}}];if !ok{{return nil}};return rowCopy(manager.statusEffects[index])}}
func(manager *{manager_type}) StatusEffectIDs()iter.Seq[string]{{return func(yield func(string)bool){{for index:=range manager.statusEffects{{if !yield(manager.statusEffects[index].Key){{return}}}}}}}}
func(manager *{manager_type}) StatusEffects()iter.Seq[StatusEffectData]{{return rowValues(manager.statusEffects)}}
func(manager *{manager_type}) Rows()iter.Seq[StatusEffectData]{{return manager.StatusEffects()}}

"#)}
}

fn item_conversion(
    unit: &GameDataCompileUnit,
    manager: &DirectManagerSurface,
) -> GoNativeManagerAugmentation {
    let row = required_row(unit, manager, "ItemCurrencyConversionData");
    let row_field = go_direct_row_field_name("ItemCurrencyConversionData");
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
    let id = s("ConversionID");
    let item = s("ItemID");
    let entitlement = s("EntitlementID");
    let buy_item = s("BuyCurrencyItemName");
    let buy_progression3 = s("BuyProgression3ID");
    let perks = (1..=4)
        .map(|slot| s(&format!("Perk{slot}")))
        .collect::<Vec<_>>();
    let perk_values = perks
        .iter()
        .map(|v| format!("gametypes.CRC32(crc32Lowercase({v})),"))
        .collect::<String>();
    GoNativeManagerAugmentation{declarations:r#"
type ItemConversionFaction uint8
const (
	ItemConversionFactionNone ItemConversionFaction = iota
	ItemConversionFactionSyndicate
	ItemConversionFactionCovenant
	ItemConversionFactionMarauder
	ItemConversionFactionOther
)
type ItemConversionDataHandle struct { Row int }
type ItemCurrencyConversionData struct { Handle ItemConversionDataHandle; ConversionID string; ConversionIDCRC gametypes.CRC32; ItemID string; ItemIDCRC gametypes.CRC32; ItemQty uint32; PerkOverrides [4]gametypes.CRC32; Bought bool; Sold bool; InContracts bool; DisplayOrder uint32; EntitlementID string; EntitlementIDCRC gametypes.CRC32; RequiredRank uint32; BuyCategoricalProgressionCost uint32; BuyCurrencyCost uint32; BuyCurrencyItemName string; BuyCurrencyItemCRC gametypes.CRC32; BuyCurrencyItemCost uint32; BuyCooldownSeconds uint32; BuyProgression3ID string; BuyProgression3IDCRC gametypes.CRC32; BuyProgression3Cost uint32; SellCategoricalProgressionCost uint32; SellCurrencyCost uint32; SellAzothCost uint32 }
"#.to_owned(),fields:"\titemConversions []ItemCurrencyConversionData\n\titemConversionsByID map[gametypes.CRC32]int\n\titemConversionsByRow map[int]int\n".to_owned(),field_values:"\t\titemConversionsByID:make(map[gametypes.CRC32]int),\n\t\titemConversionsByRow:make(map[int]int),\n".to_owned(),initializers:format!(r#"	for index:=range manager.{row_field}.entries{{source:=&manager.{row_field}.entries[index];key:=strings.TrimSpace({id});id:=gametypes.CRC32(crc32Lowercase(key));if key==""||id==0{{continue}};if _,exists:=manager.itemConversionsByID[id];exists{{continue}};toUint:=func(value float32)uint32{{out,ok:=exactUint32(value);if !ok{{return 0}};return out}};itemKey:=strings.TrimSpace({item});entitlementKey:=strings.TrimSpace({entitlement});buyItem:=strings.TrimSpace({buy_item});progression3:=strings.TrimSpace({buy_progression3});data:=ItemCurrencyConversionData{{Handle:ItemConversionDataHandle{{Row:source.Slot.RowIndex()}},ConversionID:key,ConversionIDCRC:id,ItemID:itemKey,ItemIDCRC:gametypes.CRC32(crc32Lowercase(itemKey)),ItemQty:toUint({}),PerkOverrides:[4]gametypes.CRC32{{{perk_values}}},Bought:{},Sold:{},InContracts:{},DisplayOrder:toUint({}),EntitlementID:entitlementKey,EntitlementIDCRC:gametypes.CRC32(crc32Lowercase(entitlementKey)),RequiredRank:toUint({}),BuyCategoricalProgressionCost:toUint({}),BuyCurrencyCost:toUint({}),BuyCurrencyItemName:buyItem,BuyCurrencyItemCRC:gametypes.CRC32(crc32Lowercase(buyItem)),BuyCurrencyItemCost:toUint({}),BuyCooldownSeconds:toUint({}),BuyProgression3ID:progression3,BuyProgression3IDCRC:gametypes.CRC32(crc32Lowercase(progression3)),BuyProgression3Cost:toUint({}),SellCategoricalProgressionCost:toUint({}),SellCurrencyCost:toUint({}),SellAzothCost:toUint({})}};manager.itemConversionsByID[id]=len(manager.itemConversions);manager.itemConversionsByRow[data.Handle.Row]=len(manager.itemConversions);manager.itemConversions=append(manager.itemConversions,data)}}
"#,n("ItemQty"),b("Bought"),b("Sold"),b("InContracts"),n("DisplayOrder"),n("RequiredCategoricalProgressionRank"),n("BuyCategoricalProgressionCost"),n("BuyCurrencyCost"),n("BuyCurrencyItemCost"),n("BuyCooldownSeconds"),n("BuyProgression3Cost"),n("SellCategoricalProgressionCost"),n("SellCurrencyCost"),n("SellAzothCost")),methods:format!(r#"func(manager *{manager_type}) Get(id gametypes.CRC32)*ItemCurrencyConversionData{{index,ok:=manager.itemConversionsByID[id];if !ok{{return nil}};return rowCopy(manager.itemConversions[index])}}
func(manager *{manager_type}) ByID(key string)*ItemCurrencyConversionData{{return manager.Get(gametypes.CRC32(crc32Lowercase(key)))}}
func(manager *{manager_type}) ItemConversionForSourceRow(row int)*ItemCurrencyConversionData{{index,ok:=manager.itemConversionsByRow[row];if !ok{{return nil}};return rowCopy(manager.itemConversions[index])}}
func(manager *{manager_type}) ItemConversions()iter.Seq[ItemCurrencyConversionData]{{return rowValues(manager.itemConversions)}}
func(manager *{manager_type}) Rows()iter.Seq[ItemCurrencyConversionData]{{return manager.ItemConversions()}}

"#)}
}

fn item_transform(
    unit: &GameDataCompileUnit,
    manager: &DirectManagerSurface,
) -> GoNativeManagerAugmentation {
    let row = go_direct_row_specs(unit, manager)
        .into_iter()
        .find(|row| {
            row.fields
                .iter()
                .any(|field| field.source_name.eq_ignore_ascii_case("FromItemID"))
        })
        .unwrap_or_else(|| panic!("{} requires item transform rows", manager.manager_name));
    let row_field = go_direct_row_field_name(&row.source_row_type);
    let default = go_direct_default_row_spec(unit, manager)
        .map(|r| r.source_row_type)
        .as_deref()
        == Some(row.source_row_type.as_str());
    let table_type = go_direct_table_type_name(manager, &row.source_row_type, default);
    let manager_type = go_method_name(&manager.manager_class_name);
    let from = string_expression(required_field(&row, "FromItemID"), "source.Row");
    let to = string_expression(required_field(&row, "ToItemID"), "source.Row");
    let keep = optional_field(&row, "KeepPerks")
        .map(|f| bool_expression(f, "source.Row"))
        .unwrap_or_else(|| "false".to_owned());
    let feature = optional_field(&row, "FeatureID")
        .map(|f| string_expression(f, "source.Row"))
        .unwrap_or_else(|| "\"\"".to_owned());
    GoNativeManagerAugmentation{declarations:format!(r#"
type ItemTransformHandle struct {{ Table {table_type}; Row int }}
type ItemTransformKey struct {{ Table {table_type}; FromItemID gametypes.CRC32 }}
type ItemTransformData struct {{ Source ItemTransformHandle; Table {table_type}; SourceRow int; FromItemKey string; FromItemID gametypes.CRC32; ToItemKey string; ToItemID gametypes.CRC32; KeepPerks bool; FeatureID string; Feature gametypes.CRC32 }}
"#),fields:"\titemTransforms []ItemTransformData\n\titemTransformsByKey map[ItemTransformKey]int\n\titemTransformsBySource map[ItemTransformHandle]int\n\titemTransformsByTable map[INVALID][]int\n".replace("INVALID",&table_type),field_values:"\t\titemTransformsByKey:make(map[ItemTransformKey]int),\n\t\titemTransformsBySource:make(map[ItemTransformHandle]int),\n\t\titemTransformsByTable:make(map[INVALID][]int),\n".replace("INVALID",&table_type),initializers:format!(r#"	for index:=range manager.{row_field}.entries{{source:=&manager.{row_field}.entries[index];fromKey:=strings.TrimSpace({from});fromID:=gametypes.CRC32(crc32Lowercase(fromKey));toKey:=strings.TrimSpace({to});toID:=gametypes.CRC32(crc32Lowercase(toKey));if fromKey==""||fromID==0||toKey==""||toID==0{{continue}};key:=ItemTransformKey{{Table:source.Ref.Table(),FromItemID:fromID}};if _,exists:=manager.itemTransformsByKey[key];exists{{continue}};handle:=ItemTransformHandle{{Table:source.Ref.Table(),Row:source.Slot.RowIndex()}};featureID:=strings.TrimSpace({feature});data:=ItemTransformData{{Source:handle,Table:source.Ref.Table(),SourceRow:source.Slot.RowIndex(),FromItemKey:fromKey,FromItemID:fromID,ToItemKey:toKey,ToItemID:toID,KeepPerks:{keep},FeatureID:featureID,Feature:gametypes.CRC32(crc32Lowercase(featureID))}};dataIndex:=len(manager.itemTransforms);manager.itemTransformsByKey[key]=dataIndex;manager.itemTransformsBySource[handle]=dataIndex;manager.itemTransformsByTable[data.Table]=append(manager.itemTransformsByTable[data.Table],dataIndex);manager.itemTransforms=append(manager.itemTransforms,data)}}
"#),methods:format!(r#"func(manager *{manager_type}) Transform(table {table_type},fromItemID gametypes.CRC32)*ItemTransformData{{index,ok:=manager.itemTransformsByKey[ItemTransformKey{{Table:table,FromItemID:fromItemID}}];if !ok{{return nil}};return rowCopy(manager.itemTransforms[index])}}
func(manager *{manager_type}) TransformByKey(table {table_type},fromItemKey string)*ItemTransformData{{return manager.Transform(table,gametypes.CRC32(crc32Lowercase(fromItemKey)))}}
func(manager *{manager_type}) Source(handle ItemTransformHandle)*ItemTransformData{{index,ok:=manager.itemTransformsBySource[handle];if !ok{{return nil}};return rowCopy(manager.itemTransforms[index])}}
func(manager *{manager_type}) TableRows(table {table_type})iter.Seq[ItemTransformData]{{return func(yield func(ItemTransformData)bool){{for _,index:=range manager.itemTransformsByTable[table]{{if !yield(manager.itemTransforms[index]){{return}}}}}}}}
func(manager *{manager_type}) ItemTransforms()iter.Seq[ItemTransformData]{{return rowValues(manager.itemTransforms)}}
func(manager *{manager_type}) Rows()iter.Seq[ItemTransformData]{{return manager.ItemTransforms()}}

"#)}
}
