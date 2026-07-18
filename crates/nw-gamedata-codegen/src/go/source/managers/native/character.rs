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
        NativeManagerShape::PostSkillCapProgression(_) => post_skill_cap(unit, manager),
        NativeManagerShape::OneTableCostumeChange(shape) => costume_change(unit, manager, shape),
        NativeManagerShape::OneTableDungeonTile(_) => dungeon_tile(unit, manager),
        NativeManagerShape::OneTableEncumbrance(_) => encumbrance(unit, manager),
        NativeManagerShape::OneTableDifficultyScaling(_) => difficulty_scaling(unit, manager),
        NativeManagerShape::OneTableDarkness(_) => darkness(unit, manager),
        _ => panic!(
            "manager {} reached character Go native dispatch with {shape:?}",
            manager.manager_name
        ),
    }
}

fn post_skill_cap(
    unit: &GameDataCompileUnit,
    manager: &DirectManagerSurface,
) -> GoNativeManagerAugmentation {
    let row = required_row(unit, manager, "TradeSkillPostCapData");
    let row_field = go_direct_row_field_name("TradeSkillPostCapData");
    let manager_type = go_method_name(&manager.manager_class_name);
    let skill = string_expression(required_field(&row, "TradeSkillType"), "source.Row");
    let xp = number_expression(required_field(&row, "TradeSkillRewardXP"), "source.Row");
    let percentages = (1..=9)
        .filter_map(|slot| optional_field(&row, &format!("SubRewardPerc{slot}")))
        .map(|f| number_expression(f, "source.Row"))
        .collect::<Vec<_>>();
    let percentage_values = percentages
        .iter()
        .map(|v| format!("{v},"))
        .collect::<String>();
    let mut reward_inits = String::new();
    for level in ["01", "02"] {
        let expansion = optional_field(&row, &format!("Level{level}ExpansionId"))
            .map(|f| string_expression(f, "source.Row"))
            .unwrap_or_else(|| "\"\"".to_owned());
        let reward = optional_field(&row, &format!("Level{level}Reward"))
            .map(|f| string_expression(f, "source.Row"))
            .unwrap_or_else(|| "\"\"".to_owned());
        let event = optional_field(&row, &format!("Level{level}GameEvent"))
            .map(|f| string_expression(f, "source.Row"))
            .unwrap_or_else(|| "\"\"".to_owned());
        let subs = (1..=9)
            .filter_map(|slot| optional_field(&row, &format!("Level{level}SubReward{slot}")))
            .map(|f| string_expression(f, "source.Row"))
            .collect::<Vec<_>>();
        let events = (1..=2)
            .filter_map(|slot| optional_field(&row, &format!("Level{level}GameEvent{slot}")))
            .map(|f| string_expression(f, "source.Row"))
            .collect::<Vec<_>>();
        for sub in subs {
            reward_inits.push_str(&format!("\t\tif key := strings.TrimSpace({sub}); key != \"\" {{ levelRewards.SubRewardIDs = append(levelRewards.SubRewardIDs, gametypes.CRC32(crc32Lowercase(key))) }}\n"));
        }
        for sub in events {
            reward_inits.push_str(&format!("\t\tif key := strings.TrimSpace({sub}); key != \"\" {{ levelRewards.SubGameEventIDs = append(levelRewards.SubGameEventIDs, gametypes.CRC32(crc32Lowercase(key))) }}\n"));
        }
        reward_inits.push_str(&format!("\t\tlevelRewards.ExpansionID = strings.TrimSpace({expansion}); levelRewards.RewardID = gametypes.CRC32(crc32Lowercase({reward})); levelRewards.GameEventID = gametypes.CRC32(crc32Lowercase({event})); data.LevelRewards = append(data.LevelRewards, levelRewards)\n"));
    }
    GoNativeManagerAugmentation{declarations:r#"
type PostSkillCapLevelRewards struct { ExpansionID string; SubRewardIDs []gametypes.CRC32; SubGameEventIDs []gametypes.CRC32; RewardID gametypes.CRC32; GameEventID gametypes.CRC32 }
type PostSkillCapProgressionData struct { TradeSkillType gametypes.CRC32; TradeSkillKey string; TradeSkillRewardXP uint32; SubRewardPercentages []float32; LevelRewards []PostSkillCapLevelRewards }
"#.to_owned(),fields:"\tpostSkillCap []PostSkillCapProgressionData\n\tpostSkillCapByID map[gametypes.CRC32]int\n".to_owned(),field_values:"\t\tpostSkillCapByID: make(map[gametypes.CRC32]int),\n".to_owned(),initializers:format!(r#"	for index:=range manager.{row_field}.entries{{source:=&manager.{row_field}.entries[index];key:=strings.TrimSpace({skill});id:=gametypes.CRC32(crc32Lowercase(key));if key==""||id==0{{continue}};xp,ok:=exactUint32({xp});if !ok||xp==0{{continue}};if _,exists:=manager.postSkillCapByID[id];exists{{continue}};data:=PostSkillCapProgressionData{{TradeSkillType:id,TradeSkillKey:key,TradeSkillRewardXP:xp,SubRewardPercentages:[]float32{{{percentage_values}}}}}
		levelRewards:=PostSkillCapLevelRewards{{}}
{reward_inits}		manager.postSkillCapByID[id]=len(manager.postSkillCap);manager.postSkillCap=append(manager.postSkillCap,data)}}
"#),methods:format!(r#"func(manager *{manager_type}) PostSkillCapProgressionDataFromID(id gametypes.CRC32)*PostSkillCapProgressionData{{index,ok:=manager.postSkillCapByID[id];if !ok{{return nil}};return rowCopy(manager.postSkillCap[index])}}
func(manager *{manager_type}) PostSkillCapProgressionData(key string)*PostSkillCapProgressionData{{return manager.PostSkillCapProgressionDataFromID(gametypes.CRC32(crc32Lowercase(key)))}}
func(manager *{manager_type}) PostSkillCapRows()iter.Seq[PostSkillCapProgressionData]{{return rowValues(manager.postSkillCap)}}
func(manager *{manager_type}) Rows()iter.Seq[PostSkillCapProgressionData]{{return manager.PostSkillCapRows()}}

"#)}
}

fn costume_change(
    unit: &GameDataCompileUnit,
    manager: &DirectManagerSurface,
    shape: &crate::manager::NativeOneTableCostumeChangeManager,
) -> GoNativeManagerAugmentation {
    let row = required_row(unit, manager, "CostumeChangeData");
    let row_field = go_direct_row_field_name("CostumeChangeData");
    let manager_type = go_method_name(&manager.manager_class_name);
    let key = string_expression(
        required_field(&row, shape.key_column().as_str()),
        "source.Row",
    );
    let mesh = string_expression(
        required_field(&row, shape.mesh_column().as_str()),
        "source.Row",
    );
    let skeleton = bool_expression(
        required_field(&row, shape.matches_skeleton_column().as_str()),
        "source.Row",
    );
    let offset = number_expression(
        required_field(&row, shape.z_offset_column().as_str()),
        "source.Row",
    );
    let mut slots = String::new();
    for slot in shape.slots() {
        let left = optional_field(&row, slot.left_column().as_str())
            .map(|f| string_expression(f, "source.Row"))
            .unwrap_or_else(|| "\"\"".to_owned());
        let right = optional_field(&row, slot.right_column().as_str())
            .map(|f| string_expression(f, "source.Row"))
            .unwrap_or_else(|| "\"\"".to_owned());
        slots.push_str(&format!("\t\tdata.AudioOverrides[CostumeAudioSlot({:?})] = CostumeAudioOverride{{Left:gametypes.CRC32(crc32Lowercase({left})),Right:gametypes.CRC32(crc32Lowercase({right}))}}\n",slot.display().as_str()));
    }
    GoNativeManagerAugmentation{declarations:r#"
type CostumeAudioSlot string
type CostumeAudioOverride struct { Left gametypes.CRC32; Right gametypes.CRC32 }
type CostumeChangeData struct { SourceRow int; ID gametypes.CRC32; Key string; Mesh string; MatchesPlayerSkeleton bool; MeshRenderZPosOffset float32; AudioOverrides map[CostumeAudioSlot]CostumeAudioOverride }
"#.to_owned(),fields:"\tcostumeChanges []CostumeChangeData\n\tcostumeChangesByID map[gametypes.CRC32]int\n\tcostumeChangesBySource map[int]int\n".to_owned(),field_values:"\t\tcostumeChangesByID: make(map[gametypes.CRC32]int),\n\t\tcostumeChangesBySource: make(map[int]int),\n".to_owned(),initializers:format!(r#"	for index:=range manager.{row_field}.entries{{source:=&manager.{row_field}.entries[index];key:=strings.TrimSpace({key});id:=gametypes.CRC32(crc32Lowercase(key));if key==""||id==0{{continue}};if _,exists:=manager.costumeChangesByID[id];exists{{continue}};data:=CostumeChangeData{{SourceRow:source.Slot.RowIndex(),ID:id,Key:key,Mesh:strings.TrimSpace({mesh}),MatchesPlayerSkeleton:{skeleton},MeshRenderZPosOffset:{offset},AudioOverrides:make(map[CostumeAudioSlot]CostumeAudioOverride)}}
{slots}		manager.costumeChangesByID[id]=len(manager.costumeChanges);manager.costumeChangesBySource[data.SourceRow]=len(manager.costumeChanges);manager.costumeChanges=append(manager.costumeChanges,data)}}
"#),methods:format!(r#"func(manager *{manager_type}) CostumeChangeDataFromID(id gametypes.CRC32)*CostumeChangeData{{index,ok:=manager.costumeChangesByID[id];if !ok{{return nil}};return rowCopy(manager.costumeChanges[index])}}
func(manager *{manager_type}) CostumeChangeData(key string)*CostumeChangeData{{return manager.CostumeChangeDataFromID(gametypes.CRC32(crc32Lowercase(key)))}}
func(manager *{manager_type}) CostumeChangeDataByKey(key string)*CostumeChangeData{{return manager.CostumeChangeData(key)}}
func(manager *{manager_type}) CostumeAudioOverrideFromID(id gametypes.CRC32,slot CostumeAudioSlot)(CostumeAudioOverride,bool){{data:=manager.CostumeChangeDataFromID(id);if data==nil{{return CostumeAudioOverride{{}},false}};value,ok:=data.AudioOverrides[slot];return value,ok}}
func(manager *{manager_type}) CostumeAudioOverride(key string,slot CostumeAudioSlot)(CostumeAudioOverride,bool){{return manager.CostumeAudioOverrideFromID(gametypes.CRC32(crc32Lowercase(key)),slot)}}
func(manager *{manager_type}) CostumeChanges()iter.Seq[CostumeChangeData]{{return rowValues(manager.costumeChanges)}}
func(manager *{manager_type}) Rows()iter.Seq[CostumeChangeData]{{return manager.CostumeChanges()}}

"#)}
}

fn dungeon_tile(
    unit: &GameDataCompileUnit,
    manager: &DirectManagerSurface,
) -> GoNativeManagerAugmentation {
    let row = required_row(unit, manager, "DungeonTileStaticData");
    let row_field = go_direct_row_field_name("DungeonTileStaticData");
    let manager_type = go_method_name(&manager.manager_class_name);
    let s = |column: &str| string_expression(required_field(&row, column), "source.Row");
    let n = |column: &str| number_expression(required_field(&row, column), "source.Row");
    let key = s("DungeonTileId");
    let feature = s("FeatureId");
    let connections = s("Connections");
    let variations = s("VariationAssetPaths");
    let rooms = s("SupportedRoomTypes");
    let rotations = n("Rotations");
    let size = n("TileSize");
    let weight = optional_field(&row, "Weight")
        .map(|f| number_expression(f, "source.Row"))
        .unwrap_or_else(|| "0".to_owned());
    GoNativeManagerAugmentation{declarations:r#"
type DungeonTileConnections uint8
const (
	DungeonTileNorth DungeonTileConnections = 1 << iota
	DungeonTileEast
	DungeonTileSouth
	DungeonTileWest
)
func dungeonTileConnections(value string) DungeonTileConnections { var out DungeonTileConnections; for _, token := range splitDesignerList(value) { switch strings.ToUpper(token) { case "N","NORTH":out|=DungeonTileNorth;case "E","EAST":out|=DungeonTileEast;case "S","SOUTH":out|=DungeonTileSouth;case "W","WEST":out|=DungeonTileWest } }; return out }
func (value DungeonTileConnections) Rotate(rotation uint8) DungeonTileConnections { rotation%=4; for ;rotation>0;rotation-- { value=((value<<1)&15)|((value>>3)&1) }; return value }
type DungeonTileLookupKey struct { FeatureID gametypes.CRC32; Connections DungeonTileConnections }
type DungeonTileVariant struct { SourceRow int; Rotation uint8; Connections DungeonTileConnections }
type DungeonTileStaticData struct { SourceRow int; Key string; ID gametypes.CRC32; FeatureKey string; FeatureID gametypes.CRC32; Connections DungeonTileConnections; Rotations uint8; TileSize uint8; Weight uint32; VariationAssetPaths []string; SupportedRoomTypes []gametypes.CRC32 }
"#.to_owned(),fields:"\tdungeonTiles []DungeonTileStaticData\n\tdungeonTilesByID map[gametypes.CRC32]int\n\tdungeonTilesBySource map[int]int\n\tdungeonTileVariants map[DungeonTileLookupKey][]DungeonTileVariant\n".to_owned(),field_values:"\t\tdungeonTilesByID: make(map[gametypes.CRC32]int),\n\t\tdungeonTilesBySource: make(map[int]int),\n\t\tdungeonTileVariants: make(map[DungeonTileLookupKey][]DungeonTileVariant),\n".to_owned(),initializers:format!(r#"	for index:=range manager.{row_field}.entries{{source:=&manager.{row_field}.entries[index];key:=strings.TrimSpace({key});id:=gametypes.CRC32(crc32Lowercase(key));featureKey:=strings.TrimSpace({feature});featureID:=gametypes.CRC32(crc32Lowercase(featureKey));if key==""||id==0||featureID==0{{continue}};rotationCount,ok:=exactUint32({rotations});if !ok||rotationCount>4{{continue}};tileSize,ok:=exactUint32({size});if !ok||tileSize==0||tileSize>255{{continue}};tileWeight,ok:=exactUint32({weight});if !ok{{continue}};if _,exists:=manager.dungeonTilesByID[id];exists{{continue}};base:=dungeonTileConnections({connections});data:=DungeonTileStaticData{{SourceRow:source.Slot.RowIndex(),Key:key,ID:id,FeatureKey:featureKey,FeatureID:featureID,Connections:base,Rotations:uint8(rotationCount),TileSize:uint8(tileSize),Weight:tileWeight,VariationAssetPaths:splitDesignerList({variations})}};for _,room:=range splitDesignerList({rooms}){{roomID:=gametypes.CRC32(crc32Lowercase(room));if roomID!=0{{data.SupportedRoomTypes=append(data.SupportedRoomTypes,roomID)}}}};manager.dungeonTilesByID[id]=len(manager.dungeonTiles);manager.dungeonTilesBySource[data.SourceRow]=len(manager.dungeonTiles);manager.dungeonTiles=append(manager.dungeonTiles,data);for rotation:=uint8(0);rotation<uint8(rotationCount);rotation++{{variant:=DungeonTileVariant{{SourceRow:data.SourceRow,Rotation:rotation,Connections:base.Rotate(rotation)}};lookup:=DungeonTileLookupKey{{FeatureID:featureID,Connections:variant.Connections}};manager.dungeonTileVariants[lookup]=append(manager.dungeonTileVariants[lookup],variant)}}}}
"#),methods:format!(r#"func(manager *{manager_type}) DungeonTileStaticData(id gametypes.CRC32)*DungeonTileStaticData{{index,ok:=manager.dungeonTilesByID[id];if !ok{{return nil}};return rowCopy(manager.dungeonTiles[index])}}
func(manager *{manager_type}) DungeonTileStaticDataByKey(key string)*DungeonTileStaticData{{return manager.DungeonTileStaticData(gametypes.CRC32(crc32Lowercase(key)))}}
func(manager *{manager_type}) TileVariants(key DungeonTileLookupKey)iter.Seq[DungeonTileVariant]{{return slices.Values(manager.dungeonTileVariants[key])}}
func(manager *{manager_type}) TileVariantRow(variant DungeonTileVariant)*DungeonTileStaticData{{index,ok:=manager.dungeonTilesBySource[variant.SourceRow];if !ok{{return nil}};return rowCopy(manager.dungeonTiles[index])}}
func(manager *{manager_type}) DungeonTiles()iter.Seq[DungeonTileStaticData]{{return rowValues(manager.dungeonTiles)}}
func(manager *{manager_type}) Rows()iter.Seq[DungeonTileStaticData]{{return manager.DungeonTiles()}}

"#)}
}

fn encumbrance(
    unit: &GameDataCompileUnit,
    manager: &DirectManagerSurface,
) -> GoNativeManagerAugmentation {
    let row = required_row(unit, manager, "EncumbranceData");
    let row_field = go_direct_row_field_name("EncumbranceData");
    let manager_type = go_method_name(&manager.manager_class_name);
    let key = string_expression(required_field(&row, "ContainerTypeID"), "source.Row");
    let n = |column: &str| {
        optional_field(&row, column)
            .map(|f| number_expression(f, "source.Row"))
            .unwrap_or_else(|| "0".to_owned())
    };
    let categories = optional_field(&row, "EquipLoadCCStatusEffectCategories")
        .map(|f| string_expression(f, "source.Row"))
        .unwrap_or_else(|| "\"\"".to_owned());
    let load = |prefix: &str| {
        format!(
            "EncumbranceLoadValues{{Fast:{},Normal:{},Slow:{},Overburdened:{}}}",
            n(&format!("{prefix}Fast")),
            n(&format!("{prefix}Normal")),
            n(&format!("{prefix}Slow")),
            n(&format!("{prefix}Overburdened"))
        )
    };
    let ratio = load("EquipLoadRatio");
    let stamina = load("EquipLoadStaminaCostMult");
    let regen = load("EquipLoadStaminaRegenMult");
    let move_speed = load("EquipLoadMoveSpeedMult");
    let grit = load("EquipLoadGritResistMult");
    let defense = load("EquipLoadDefenseMult");
    let block = load("EquipLoadBlockStabilityMult");
    let damage = load("EquipLoadDamageMult");
    let heal = load("EquipLoadHealMult");
    let cc = load("EquipLoadCCStatusEffectDurationMult");
    GoNativeManagerAugmentation{declarations:r#"
type EncumbranceLoadState uint8
const (
	EncumbranceFast EncumbranceLoadState = iota
	EncumbranceNormal
	EncumbranceSlow
	EncumbranceOverburdened
)
type EncumbranceLoadValues struct { Fast float32; Normal float32; Slow float32; Overburdened float32 }
func(value EncumbranceLoadValues)Get(state EncumbranceLoadState)float32{switch state{case EncumbranceFast:return value.Fast;case EncumbranceNormal:return value.Normal;case EncumbranceSlow:return value.Slow;default:return value.Overburdened}}
type EncumbranceData struct { ContainerTypeID string; ContainerTypeCRC gametypes.CRC32; StaminaCostMult EncumbranceLoadValues; StaminaRegenMult EncumbranceLoadValues; MoveSpeedMult EncumbranceLoadValues; GritResistMult EncumbranceLoadValues; DefenseMult EncumbranceLoadValues; BlockStabilityMult EncumbranceLoadValues; DamageMult EncumbranceLoadValues; HealMult EncumbranceLoadValues; CCStatusEffectDurationMult EncumbranceLoadValues; CCStatusEffectCategories []string; EncumbranceBaseMax int32; EncumbranceWarningPercent float32; EquipLoadBaseMax int32; EquipLoadWarningPercent float32; EquipLoadRatio EncumbranceLoadValues; FullWhenEncumbered float32 }
"#.to_owned(),fields:"\tencumbrance []EncumbranceData\n\tencumbranceByID map[gametypes.CRC32]int\n".to_owned(),field_values:"\t\tencumbranceByID:make(map[gametypes.CRC32]int),\n".to_owned(),initializers:format!(r#"	for index:=range manager.{row_field}.entries{{source:=&manager.{row_field}.entries[index];key:=strings.TrimSpace({key});id:=gametypes.CRC32(crc32Lowercase(key));if key==""||id==0{{continue}};if _,exists:=manager.encumbranceByID[id];exists{{continue}};base,_:=exactUint32({});equipBase,_:=exactUint32({});data:=EncumbranceData{{ContainerTypeID:key,ContainerTypeCRC:id,StaminaCostMult:{stamina},StaminaRegenMult:{regen},MoveSpeedMult:{move_speed},GritResistMult:{grit},DefenseMult:{defense},BlockStabilityMult:{block},DamageMult:{damage},HealMult:{heal},CCStatusEffectDurationMult:{cc},CCStatusEffectCategories:splitDesignerList({categories}),EncumbranceBaseMax:int32(base),EncumbranceWarningPercent:{},EquipLoadBaseMax:int32(equipBase),EquipLoadWarningPercent:{},EquipLoadRatio:{ratio},FullWhenEncumbered:{}}};manager.encumbranceByID[id]=len(manager.encumbrance);manager.encumbrance=append(manager.encumbrance,data)}}
"#,n("EncumbranceBaseMax"),n("EquipLoadBaseMax"),n("EncumbranceWarningPercent"),n("EquipLoadWarningPercent"),n("FullWhenEncumbered")),methods:format!(r#"func(manager *{manager_type}) EncumbranceDataFromID(id gametypes.CRC32)*EncumbranceData{{index,ok:=manager.encumbranceByID[id];if !ok{{return nil}};return rowCopy(manager.encumbrance[index])}}
func(manager *{manager_type}) EncumbranceData(key string)*EncumbranceData{{return manager.EncumbranceDataFromID(gametypes.CRC32(crc32Lowercase(key)))}}
func(manager *{manager_type}) EncumbranceDataByKey(key string)*EncumbranceData{{return manager.EncumbranceData(key)}}
func(manager *{manager_type}) Rows()iter.Seq[EncumbranceData]{{return rowValues(manager.encumbrance)}}

"#)}
}

fn difficulty_scaling(
    unit: &GameDataCompileUnit,
    manager: &DirectManagerSurface,
) -> GoNativeManagerAugmentation {
    let row = required_row(unit, manager, "DifficultyScalingData");
    let row_field = go_direct_row_field_name("DifficultyScalingData");
    let manager_type = go_method_name(&manager.manager_class_name);
    let key = string_expression(required_field(&row, "WorldEncounterID"), "source.Row");
    let participants = number_expression(
        required_field(&row, "ExpectedParticipantCount"),
        "source.Row",
    );
    let creatures = string_expression(required_field(&row, "AffectedCreatureTypes"), "source.Row");
    let min = number_expression(required_field(&row, "ScalingFactorMin"), "source.Row");
    let max = number_expression(required_field(&row, "ScalingFactorMax"), "source.Row");
    let coefficient = number_expression(required_field(&row, "FunctionCoefficient"), "source.Row");
    let health = string_expression(required_field(&row, "MaxHealthMod"), "source.Row");
    GoNativeManagerAugmentation{declarations:r#"
type DifficultyHealthModifier string
const (
	DifficultyHealthModifierNone DifficultyHealthModifier = ""
	DifficultyHealthModifierAdd DifficultyHealthModifier = "Add"
	DifficultyHealthModifierMultiply DifficultyHealthModifier = "Multiply"
	DifficultyHealthModifierOverride DifficultyHealthModifier = "Override"
)
type AffectedCreatureTypes struct { All bool; IDs []gametypes.CRC32 }
type DifficultyScalingData struct { SourceRow int; Key string; ID gametypes.CRC32; ExpectedParticipantCount uint32; ScalingFactorMin float32; ScalingFactorMax float32; AffectedCreatureTypes AffectedCreatureTypes; HealthModifier DifficultyHealthModifier; FunctionCoefficient float32 }
"#.to_owned(),fields:"\tdifficultyScaling []DifficultyScalingData\n\tdifficultyScalingByID map[gametypes.CRC32]int\n".to_owned(),field_values:"\t\tdifficultyScalingByID:make(map[gametypes.CRC32]int),\n".to_owned(),initializers:format!(r#"	for index:=range manager.{row_field}.entries{{source:=&manager.{row_field}.entries[index];key:=strings.TrimSpace({key});id:=gametypes.CRC32(crc32Lowercase(key));if key==""||id==0{{continue}};participants,ok:=exactUint32({participants});if !ok||participants==0{{continue}};affectedText:=strings.TrimSpace({creatures});affected:=AffectedCreatureTypes{{All:affectedText==""||strings.EqualFold(affectedText,"all")}};if !affected.All{{for _,creature:=range splitDesignerList(affectedText){{creatureID:=gametypes.CRC32(crc32Lowercase(creature));if creatureID!=0{{affected.IDs=append(affected.IDs,creatureID)}}}}}};modifier:=DifficultyHealthModifier(strings.TrimSpace({health}));switch strings.ToLower(string(modifier)){{case"add","additive":modifier=DifficultyHealthModifierAdd;case"multiply","multiplicative":modifier=DifficultyHealthModifierMultiply;case"override","set":modifier=DifficultyHealthModifierOverride;default:modifier=DifficultyHealthModifierNone}};if _,exists:=manager.difficultyScalingByID[id];exists{{continue}};manager.difficultyScalingByID[id]=len(manager.difficultyScaling);manager.difficultyScaling=append(manager.difficultyScaling,DifficultyScalingData{{SourceRow:source.Slot.RowIndex(),Key:key,ID:id,ExpectedParticipantCount:participants,ScalingFactorMin:{min},ScalingFactorMax:{max},AffectedCreatureTypes:affected,HealthModifier:modifier,FunctionCoefficient:{coefficient}}})}}
"#),methods:format!(r#"func(manager *{manager_type}) DifficultyScalingDataFromID(id gametypes.CRC32)*DifficultyScalingData{{index,ok:=manager.difficultyScalingByID[id];if !ok{{return nil}};return rowCopy(manager.difficultyScaling[index])}}
func(manager *{manager_type}) DifficultyScalingData(key string)*DifficultyScalingData{{return manager.DifficultyScalingDataFromID(gametypes.CRC32(crc32Lowercase(key)))}}
func(manager *{manager_type}) DifficultyScalingDataByKey(key string)*DifficultyScalingData{{return manager.DifficultyScalingData(key)}}
func(manager *{manager_type}) Rows()iter.Seq[DifficultyScalingData]{{return rowValues(manager.difficultyScaling)}}

"#)}
}

fn darkness(
    unit: &GameDataCompileUnit,
    manager: &DirectManagerSurface,
) -> GoNativeManagerAugmentation {
    let row = required_row(unit, manager, "DarknessData");
    let row_field = go_direct_row_field_name("DarknessData");
    let manager_type = go_method_name(&manager.manager_class_name);
    let s = |column: &str| string_expression(required_field(&row, column), "source.Row");
    let n = |column: &str| number_expression(required_field(&row, column), "source.Row");
    let key = s("DarknessID");
    let levels = s("DarknessLevels");
    let activation = s("DarknessActivationSpec");
    let groups = s("DarknessGroupSpec");
    let scaling_group = s("DifficultyScalingGroup");
    let scaling_table = s("DifficultyScalingTable");
    let duration = n("DarknessDuration");
    let territory = n("TerritoryType");
    let enabled = bool_expression(required_field(&row, "UpdateEnabled"), "source.Row");
    GoNativeManagerAugmentation{declarations:r#"
type DarknessLevel struct { Threshold string; Level uint32 }
type DarknessActivationSpec struct { StartHour uint32; EndHour uint32 }
type DarknessGroupSpec struct { Percentage uint32; Group uint32 }
type DarknessData struct { SourceRow int; Key string; ID gametypes.CRC32; Levels []DarknessLevel; Duration float32; ActivationSpecs []DarknessActivationSpec; GroupSpecs []DarknessGroupSpec; TerritoryType uint32; DifficultyScalingGroup string; DifficultyScalingTable string; UpdateEnabled bool }
func darknessPairs(value string)[][2]string{var out [][2]string;for _,token:=range splitDesignerList(value){parts:=strings.FieldsFunc(token,func(r rune)bool{return r==':'||r=='='||r=='-' });if len(parts)>=2{out=append(out,[2]string{strings.TrimSpace(parts[0]),strings.TrimSpace(parts[1])})}};return out}
"#.to_owned(),fields:"\tdarkness []DarknessData\n\tdarknessByID map[gametypes.CRC32]int\n\tdarknessBySource map[int]int\n".to_owned(),field_values:"\t\tdarknessByID:make(map[gametypes.CRC32]int),\n\t\tdarknessBySource:make(map[int]int),\n".to_owned(),initializers:format!(r#"	for index:=range manager.{row_field}.entries{{source:=&manager.{row_field}.entries[index];key:=strings.TrimSpace({key});id:=gametypes.CRC32(crc32Lowercase(key));if key==""||id==0{{continue}};territoryType,ok:=exactUint32({territory});if !ok||territoryType==0{{continue}};data:=DarknessData{{SourceRow:source.Slot.RowIndex(),Key:key,ID:id,Duration:{duration},TerritoryType:territoryType,DifficultyScalingGroup:strings.TrimSpace({scaling_group}),DifficultyScalingTable:strings.TrimSpace({scaling_table}),UpdateEnabled:{enabled}}};for _,pair:=range darknessPairs({levels}){{value,err:=strconv.ParseUint(pair[1],10,32);if err==nil{{data.Levels=append(data.Levels,DarknessLevel{{Threshold:pair[0],Level:uint32(value)}})}}}};for _,pair:=range darknessPairs({activation}){{start,e1:=strconv.ParseUint(pair[0],10,32);end,e2:=strconv.ParseUint(pair[1],10,32);if e1==nil&&e2==nil{{data.ActivationSpecs=append(data.ActivationSpecs,DarknessActivationSpec{{StartHour:uint32(start),EndHour:uint32(end)}})}}}};for _,pair:=range darknessPairs({groups}){{percentage,e1:=strconv.ParseUint(pair[0],10,32);group,e2:=strconv.ParseUint(pair[1],10,32);if e1==nil&&e2==nil{{data.GroupSpecs=append(data.GroupSpecs,DarknessGroupSpec{{Percentage:uint32(percentage),Group:uint32(group)}})}}}};if _,exists:=manager.darknessByID[id];exists{{continue}};manager.darknessByID[id]=len(manager.darkness);manager.darknessBySource[data.SourceRow]=len(manager.darkness);manager.darkness=append(manager.darkness,data)}}
"#),methods:format!(r#"func(manager *{manager_type}) DarknessDataByCRC32(id gametypes.CRC32)*DarknessData{{index,ok:=manager.darknessByID[id];if !ok{{return nil}};return rowCopy(manager.darkness[index])}}
func(manager *{manager_type}) DarknessData(key string)*DarknessData{{return manager.DarknessDataByCRC32(gametypes.CRC32(crc32Lowercase(key)))}}
func(manager *{manager_type}) DarknessForSourceRow(row int)*DarknessData{{index,ok:=manager.darknessBySource[row];if !ok{{return nil}};return rowCopy(manager.darkness[index])}}
func(manager *{manager_type}) DarknessRows()iter.Seq[DarknessData]{{return rowValues(manager.darkness)}}
func(manager *{manager_type}) Rows()iter.Seq[DarknessData]{{return manager.DarknessRows()}}

"#)}
}
