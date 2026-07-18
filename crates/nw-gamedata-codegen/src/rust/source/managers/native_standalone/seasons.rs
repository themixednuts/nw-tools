use super::*;

pub(super) fn augmentation(
    context: &AugmentationContext<'_>,
    shape: &NativeManagerShape,
) -> Result<Option<RustNativeManagerAugmentation>> {
    let value = match shape {
        NativeManagerShape::SeasonsRewardsData(_) => rewards(context)?,
        NativeManagerShape::SeasonsTrackedStatData(_) => tracked_stats(context)?,
        NativeManagerShape::SeasonsRewardsActivitiesTasksData(_) => activity_tasks(context)?,
        NativeManagerShape::SeasonsRewardsBattlePassData(_) => battle_pass(context)?,
        NativeManagerShape::SeasonsRewardsCardTemplateData(_) => card_templates(context)?,
        NativeManagerShape::SeasonsRewardsChapterData(_) => chapters(context)?,
        NativeManagerShape::SeasonsRewardsJourneyData(_) => journeys(context)?,
        NativeManagerShape::SongBookSheetData(_) => song_sheets(context)?,
        NativeManagerShape::SongBookData(_) => songs(context)?,
        _ => return Ok(None),
    };
    Ok(Some(value))
}

fn rewards(context: &AugmentationContext<'_>) -> Result<RustNativeManagerAugmentation> {
    let row = context.row("SeasonsRewardData")?;
    let parts = context.row_parts(row);
    let id = required_string(context, row, "RewardID")?;
    let index = optional_number(context, row, "RewardIndex", "source.row")?;
    let reward_type = optional_owned_string(context, row, "RewardType", "source.row")?;
    let item_id = optional_crc(context, row, "ItemID", "source.row")?;
    let entitlements = optional_crc_list(context, row, "EntitlementIDs", "source.row")?;
    let seasons_xp = optional_number(context, row, "SeasonsXP", "source.row")?;
    let requires_premium = optional_boolish(context, row, "RequiresPremium", "source.row")?;
    let declarations = format!(
        r#"#[derive(Debug, Clone)]
pub struct SeasonReward {{
    pub id: Crc32,
    pub key: String,
    pub index: Option<u32>,
    pub reward_type: Option<String>,
    pub item_id: Option<Crc32>,
    pub entitlement_ids: Vec<Crc32>,
    pub seasons_xp: Option<f32>,
    pub requires_premium: bool,
    source: RowRef<{table}, {row_type}>,
}}

impl SeasonReward {{
    pub fn source_ref(&self) -> &RowRef<{table}, {row_type}> {{ &self.source }}
}}
"#,
        table = parts.table_type,
        row_type = parts.row_type,
    );
    let initializers = format!(
        r#"        let mut rewards = Vec::new();
        let mut rewards_by_id = HashMap::new();
        let mut rewards_by_index = HashMap::new();
        let mut rewards_by_type: HashMap<String, Vec<usize>> = HashMap::new();
        for source in {row_field}.rows() {{
            let key = {id}.trim();
            let id = Crc32::from_str_lower(key);
            if key.is_empty() || id == Crc32::ZERO || rewards_by_id.contains_key(&id) {{ continue; }}
            let index = {index}.and_then(season_exact_u32);
            let reward_type = {reward_type};
            let value = SeasonReward {{
                id,
                key: key.to_owned(),
                index,
                reward_type: reward_type.clone(),
                item_id: {item_id},
                entitlement_ids: {entitlements},
                seasons_xp: {seasons_xp},
                requires_premium: {requires_premium},
                source: source.reference.clone(),
            }};
            let slot = rewards.len();
            rewards.push(value);
            rewards_by_id.insert(id, slot);
            if let Some(index) = index {{ rewards_by_index.entry(index).or_insert(slot); }}
            if let Some(kind) = reward_type {{
                rewards_by_type.entry(normalize_lookup_key(&kind)).or_default().push(slot);
            }}
        }}
"#,
        row_field = parts.row_field,
    );
    let methods = format!(
        r#"    pub fn by_id(&self, id: Crc32) -> Option<&SeasonReward> {{ self.rewards_by_id.get(&id).and_then(|slot| self.rewards.get(*slot)) }}
    pub fn by_key(&self, key: impl AsRef<str>) -> Option<&SeasonReward> {{ self.by_id(Crc32::from_str_lower(key.as_ref())) }}
    pub fn by_index(&self, index: u32) -> Option<&SeasonReward> {{ self.rewards_by_index.get(&index).and_then(|slot| self.rewards.get(*slot)) }}
    pub fn rewards_by_type(&self, reward_type: impl AsRef<str>) -> impl Iterator<Item = &SeasonReward> {{
        self.rewards_by_type.get(&normalize_lookup_key(reward_type.as_ref())).into_iter().flatten().filter_map(|slot| self.rewards.get(*slot))
    }}
    pub fn rewards(&self) -> std::slice::Iter<'_, SeasonReward> {{ self.rewards.iter() }}
    pub fn reward_source(&self, reward: &SeasonReward) -> Option<&{row_type}> {{ self.{row_field}.get(&reward.source) }}

"#,
        row_type = parts.row_type,
        row_field = parts.row_field,
    );
    Ok(RustNativeManagerAugmentation {
        declarations: format!("{}{}", numeric_helpers(), declarations),
        fields: "    rewards: Vec<SeasonReward>,\n    rewards_by_id: HashMap<Crc32, usize>,\n    rewards_by_index: HashMap<u32, usize>,\n    rewards_by_type: HashMap<String, Vec<usize>>,\n".to_owned(),
        field_values: "            rewards,\n            rewards_by_id,\n            rewards_by_index,\n            rewards_by_type,\n".to_owned(),
        initializers,
        methods,
        rows_type: "SeasonReward".to_owned(),
        rows_method: "rewards".to_owned(),
    })
}

fn tracked_stats(context: &AugmentationContext<'_>) -> Result<RustNativeManagerAugmentation> {
    let row = context.row("SeasonsRewardsStats")?;
    let parts = context.row_parts(row);
    let id = required_string(context, row, "TrackedStatID")?;
    let stat_type = optional_owned_string(context, row, "StatType", "source.row")?;
    let target_ids = optional_crc_list(context, row, "TargetID", "source.row")?;
    let categorical_ids =
        optional_crc_list(context, row, "CategoricalProgressionIDs", "source.row")?;
    let loot_tags = optional_crc_list(context, row, "LootTags", "source.row")?;
    let mastery_ids = optional_crc_list(context, row, "WeaponMasteryIDs", "source.row")?;
    let creature_types = optional_crc_list(context, row, "CreatureTypes", "source.row")?;
    let gatherable_ids = optional_crc_list(context, row, "GatherableIDs", "source.row")?;
    let event_ids = optional_crc_list(context, row, "GameEventIDs", "source.row")?;
    let status_ids = optional_crc_list(context, row, "RequiresStatusEffect", "source.row")?;
    let declarations = format!(
        r#"{helpers}#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SeasonsTrackedStatKind {{
    Kill, Craft, Fishing, Salvage, CompleteQuest, CompleteJourneyTask, CompleteSong,
    ObtainLevel, CaptureFcp, CompleteExpedition, CompleteDuel, CompleteOutpostRush,
    CompleteArena, CompleteActivityCard, CompleteWar, GameEvent, Achievement,
    CommitResources, CategoricalProgression, Contribution, Combat, Consume, Gather,
    EquipItem,
}}

impl SeasonsTrackedStatKind {{
    pub fn parse(value: &str) -> Option<Self> {{
        Some(match normalize_lookup_key(value).as_str() {{
            "kill" => Self::Kill, "craft" => Self::Craft, "fishing" => Self::Fishing,
            "salvage" => Self::Salvage, "completequest" => Self::CompleteQuest,
            "completejourneytask" => Self::CompleteJourneyTask,
            "completesong" => Self::CompleteSong, "obtainlevel" => Self::ObtainLevel,
            "capturefcp" => Self::CaptureFcp, "completeexpedition" => Self::CompleteExpedition,
            "completeduel" => Self::CompleteDuel, "completeoutpostrush" => Self::CompleteOutpostRush,
            "completearena" => Self::CompleteArena, "completeactivitycard" => Self::CompleteActivityCard,
            "completewar" => Self::CompleteWar, "gameevent" => Self::GameEvent,
            "achievement" => Self::Achievement, "commitresources" => Self::CommitResources,
            "categoricalprogression" => Self::CategoricalProgression,
            "contribution" => Self::Contribution, "combat" => Self::Combat,
            "consume" => Self::Consume, "gather" => Self::Gather, "equipitem" => Self::EquipItem,
            _ => return None,
        }})
    }}
}}

#[derive(Debug, Clone)]
pub struct SeasonsTrackedStat {{
    pub id: Crc32,
    pub key: String,
    pub stat_type: SeasonsTrackedStatKind,
    pub target_ids: Vec<Crc32>,
    pub categorical_progression_ids: Vec<Crc32>,
    pub loot_tags: Vec<Crc32>,
    pub weapon_mastery_ids: Vec<Crc32>,
    pub creature_types: Vec<Crc32>,
    pub gatherable_ids: Vec<Crc32>,
    pub game_event_ids: Vec<Crc32>,
    pub required_status_effect_ids: Vec<Crc32>,
    source: RowRef<{table}, {row_type}>,
}}

impl SeasonsTrackedStat {{
    pub fn source_ref(&self) -> &RowRef<{table}, {row_type}> {{ &self.source }}
}}
"#,
        helpers = numeric_helpers(),
        table = parts.table_type,
        row_type = parts.row_type,
    );
    let initializers = format!(
        r#"        let mut tracked_stats = Vec::new();
        let mut tracked_stats_by_id = HashMap::new();
        let mut tracked_stats_by_table_and_id = HashMap::new();
        let mut tracked_stats_by_type: HashMap<SeasonsTrackedStatKind, Vec<usize>> = HashMap::new();
        for source in {row_field}.rows() {{
            let key = {id}.trim();
            let id = Crc32::from_str_lower(key);
            let stat_type_text = {stat_type};
            let Some(stat_type) = stat_type_text.as_deref().and_then(SeasonsTrackedStatKind::parse) else {{ continue; }};
            if key.is_empty() || id == Crc32::ZERO {{ continue; }}
            let table_key = (*source.reference.table(), id);
            if tracked_stats_by_table_and_id.contains_key(&table_key) {{ continue; }}
            let value = SeasonsTrackedStat {{
                id,
                key: key.to_owned(),
                stat_type,
                target_ids: {target_ids},
                categorical_progression_ids: {categorical_ids},
                loot_tags: {loot_tags},
                weapon_mastery_ids: {mastery_ids},
                creature_types: {creature_types},
                gatherable_ids: {gatherable_ids},
                game_event_ids: {event_ids},
                required_status_effect_ids: {status_ids},
                source: source.reference.clone(),
            }};
            let slot = tracked_stats.len();
            tracked_stats.push(value);
            tracked_stats_by_table_and_id.insert(table_key, slot);
            tracked_stats_by_id.entry(id).or_insert(slot);
            tracked_stats_by_type.entry(stat_type).or_default().push(slot);
        }}
"#,
        row_field = parts.row_field,
    );
    let methods = format!(
        r#"    pub fn tracked_stat_from_id(&self, id: Crc32) -> Option<&SeasonsTrackedStat> {{ self.tracked_stats_by_id.get(&id).and_then(|slot| self.tracked_stats.get(*slot)) }}
    pub fn tracked_stat(&self, key: impl AsRef<str>) -> Option<&SeasonsTrackedStat> {{ self.tracked_stat_from_id(Crc32::from_str_lower(key.as_ref())) }}
    pub fn tracked_stat_in(&self, table: {table}, id: Crc32) -> Option<&SeasonsTrackedStat> {{ self.tracked_stats_by_table_and_id.get(&(table, id)).and_then(|slot| self.tracked_stats.get(*slot)) }}
    pub fn tracked_stats_by_type(&self, stat_type: SeasonsTrackedStatKind) -> impl Iterator<Item = &SeasonsTrackedStat> {{
        self.tracked_stats_by_type.get(&stat_type).into_iter().flatten().filter_map(|slot| self.tracked_stats.get(*slot))
    }}
    pub fn tracked_stats_by_type_key(&self, stat_type: impl AsRef<str>) -> impl Iterator<Item = &SeasonsTrackedStat> {{ SeasonsTrackedStatKind::parse(stat_type.as_ref()).into_iter().flat_map(|kind| self.tracked_stats_by_type(kind)) }}
    pub fn tracked_stats(&self) -> std::slice::Iter<'_, SeasonsTrackedStat> {{ self.tracked_stats.iter() }}
    pub fn tracked_stat_source(&self, stat: &SeasonsTrackedStat) -> Option<&{row_type}> {{ self.{row_field}.get(&stat.source) }}

"#,
        table = parts.table_type,
        row_type = parts.row_type,
        row_field = parts.row_field,
    );
    Ok(RustNativeManagerAugmentation {
        declarations,
        fields: format!(
            "    tracked_stats: Vec<SeasonsTrackedStat>,\n    tracked_stats_by_id: HashMap<Crc32, usize>,\n    tracked_stats_by_table_and_id: HashMap<({}, Crc32), usize>,\n    tracked_stats_by_type: HashMap<SeasonsTrackedStatKind, Vec<usize>>,\n",
            parts.table_type
        ),
        field_values: "            tracked_stats,\n            tracked_stats_by_id,\n            tracked_stats_by_table_and_id,\n            tracked_stats_by_type,\n".to_owned(),
        initializers,
        methods,
        rows_type: "SeasonsTrackedStat".to_owned(),
        rows_method: "tracked_stats".to_owned(),
    })
}

fn activity_tasks(context: &AugmentationContext<'_>) -> Result<RustNativeManagerAugmentation> {
    let row = context.row("SeasonsRewardsActivitiesTasksData")?;
    let parts = context.row_parts(row);
    let id = required_string(context, row, "ActivitiesTaskID")?;
    let task = optional_owned_string(context, row, "Task", "source.row")?;
    let task_type = optional_owned_string(context, row, "Type", "source.row")?;
    let description = optional_owned_string(context, row, "Description", "source.row")?;
    let disabled = optional_boolish(context, row, "Disabled", "source.row")?;
    let declarations = format!(
        r#"{helpers}#[derive(Debug, Clone)]
pub struct SeasonActivityTask {{
    pub id: Crc32,
    pub key: String,
    pub task: Option<String>,
    pub task_type: Option<String>,
    pub description: Option<String>,
    pub disabled: bool,
    source: RowRef<{table}, {row_type}>,
}}

impl SeasonActivityTask {{
    pub fn table(&self) -> {table} {{ *self.source.table() }}
    pub fn source_ref(&self) -> &RowRef<{table}, {row_type}> {{ &self.source }}
}}
"#,
        helpers = numeric_helpers(),
        table = parts.table_type,
        row_type = parts.row_type,
    );
    let initializers = format!(
        r#"        let mut activity_tasks = Vec::new();
        let mut activity_tasks_by_table_and_id = HashMap::new();
        let mut activity_tasks_by_table: HashMap<{table}, Vec<usize>> = HashMap::new();
        for source in {row_field}.rows() {{
            let key = {id}.trim();
            let id = Crc32::from_str_lower(key);
            if key.is_empty() || id == Crc32::ZERO {{ continue; }}
            let table = *source.reference.table();
            if activity_tasks_by_table_and_id.contains_key(&(table, id)) {{ continue; }}
            let slot = activity_tasks.len();
            activity_tasks.push(SeasonActivityTask {{ id, key: key.to_owned(), task: {task}, task_type: {task_type}, description: {description}, disabled: {disabled}, source: source.reference.clone() }});
            activity_tasks_by_table_and_id.insert((table, id), slot);
            activity_tasks_by_table.entry(table).or_default().push(slot);
        }}
"#,
        table = parts.table_type,
        row_field = parts.row_field,
    );
    let methods = format!(
        r#"    pub fn activity_task(&self, table: {table}, id: Crc32) -> Option<&SeasonActivityTask> {{ self.activity_tasks_by_table_and_id.get(&(table, id)).and_then(|slot| self.activity_tasks.get(*slot)) }}
    pub fn activity_task_by_key(&self, table: {table}, key: impl AsRef<str>) -> Option<&SeasonActivityTask> {{ self.activity_task(table, Crc32::from_str_lower(key.as_ref())) }}
    pub fn activity_tasks_for_table(&self, table: {table}) -> impl Iterator<Item = &SeasonActivityTask> {{ self.activity_tasks_by_table.get(&table).into_iter().flatten().filter_map(|slot| self.activity_tasks.get(*slot)) }}
    pub fn activity_tasks(&self) -> std::slice::Iter<'_, SeasonActivityTask> {{ self.activity_tasks.iter() }}
    pub fn activity_task_source(&self, value: &SeasonActivityTask) -> Option<&{row_type}> {{ self.{row_field}.get(&value.source) }}

"#,
        table = parts.table_type,
        row_type = parts.row_type,
        row_field = parts.row_field,
    );
    Ok(RustNativeManagerAugmentation {
        declarations,
        fields: format!("    activity_tasks: Vec<SeasonActivityTask>,\n    activity_tasks_by_table_and_id: HashMap<({}, Crc32), usize>,\n    activity_tasks_by_table: HashMap<{}, Vec<usize>>,\n", parts.table_type, parts.table_type),
        field_values: "            activity_tasks,\n            activity_tasks_by_table_and_id,\n            activity_tasks_by_table,\n".to_owned(),
        initializers,
        methods,
        rows_type: "SeasonActivityTask".to_owned(),
        rows_method: "activity_tasks".to_owned(),
    })
}

fn battle_pass(context: &AugmentationContext<'_>) -> Result<RustNativeManagerAugmentation> {
    let row = context.row("SeasonPassRankData")?;
    let parts = context.row_parts(row);
    let level = numeric_value_expression(context.field(row, "Level")?, "source.row")?;
    let maximum = optional_number(context, row, "MaximumInfluence", "source.row")?;
    let influence_cost = optional_number(context, row, "InfluenceCost", "source.row")?;
    let free_reward = optional_crc(context, row, "FreeRewardID", "source.row")?;
    let premium_reward = optional_crc(context, row, "PremiumRewardID", "source.row")?;
    let declarations = format!(
        r#"{helpers}#[derive(Debug, Clone)]
pub struct SeasonPassRank {{
    pub season_id: Crc32,
    pub level: u32,
    pub maximum_influence: Option<u32>,
    pub influence_cost: Option<u32>,
    pub free_reward_id: Option<Crc32>,
    pub premium_reward_id: Option<Crc32>,
    source: RowRef<{table}, {row_type}>,
}}

impl SeasonPassRank {{
    pub fn source_ref(&self) -> &RowRef<{table}, {row_type}> {{ &self.source }}
}}
"#,
        helpers = season_table_helpers(),
        table = parts.table_type,
        row_type = parts.row_type,
    );
    let initializers = format!(
        r#"        let mut ranks = Vec::new();
        let mut ranks_by_season_and_level = HashMap::new();
        let mut ranks_by_season: HashMap<Crc32, Vec<usize>> = HashMap::new();
        let mut max_rank_by_season = HashMap::new();
        let mut maximum_influence_by_season = HashMap::new();
        for source in {row_field}.rows() {{
            let season_id = season_id_from_table_name(source.reference.table().catalog_name());
            let Some(level) = season_exact_u32({level}) else {{ continue; }};
            if season_id == Crc32::ZERO || ranks_by_season_and_level.contains_key(&(season_id, level)) {{ continue; }}
            let maximum_influence = {maximum}.and_then(season_exact_u32);
            let slot = ranks.len();
            ranks.push(SeasonPassRank {{ season_id, level, maximum_influence, influence_cost: {influence_cost}.and_then(season_exact_u32), free_reward_id: {free_reward}, premium_reward_id: {premium_reward}, source: source.reference.clone() }});
            ranks_by_season_and_level.insert((season_id, level), slot);
            ranks_by_season.entry(season_id).or_default().push(slot);
            max_rank_by_season.entry(season_id).and_modify(|value: &mut u32| *value = (*value).max(level)).or_insert(level);
            if let Some(value) = maximum_influence {{ maximum_influence_by_season.entry(season_id).and_modify(|current: &mut u32| *current = (*current).max(value)).or_insert(value); }}
        }}
        for slots in ranks_by_season.values_mut() {{ slots.sort_by_key(|slot| ranks[*slot].level); }}
"#,
        row_field = parts.row_field,
    );
    let methods = format!(
        r#"    pub fn rank(&self, season_id: Crc32, level: u32) -> Option<&SeasonPassRank> {{ self.ranks_by_season_and_level.get(&(season_id, level)).and_then(|slot| self.ranks.get(*slot)) }}
    pub fn rank_by_season_key(&self, season_key: impl AsRef<str>, level: u32) -> Option<&SeasonPassRank> {{ self.rank(Crc32::from_str_lower(season_key.as_ref()), level) }}
    pub fn ranks_for_season(&self, season_id: Crc32) -> impl Iterator<Item = &SeasonPassRank> {{ self.ranks_by_season.get(&season_id).into_iter().flatten().filter_map(|slot| self.ranks.get(*slot)) }}
    pub fn max_rank_level(&self, season_id: Crc32) -> Option<u32> {{ self.max_rank_by_season.get(&season_id).copied() }}
    pub fn maximum_influence(&self, season_id: Crc32) -> Option<u32> {{ self.maximum_influence_by_season.get(&season_id).copied() }}
    pub fn ranks(&self) -> std::slice::Iter<'_, SeasonPassRank> {{ self.ranks.iter() }}
    pub fn rank_source(&self, value: &SeasonPassRank) -> Option<&{row_type}> {{ self.{row_field}.get(&value.source) }}

"#,
        row_type = parts.row_type,
        row_field = parts.row_field,
    );
    Ok(RustNativeManagerAugmentation {
        declarations,
        fields: "    ranks: Vec<SeasonPassRank>,\n    ranks_by_season_and_level: HashMap<(Crc32, u32), usize>,\n    ranks_by_season: HashMap<Crc32, Vec<usize>>,\n    max_rank_by_season: HashMap<Crc32, u32>,\n    maximum_influence_by_season: HashMap<Crc32, u32>,\n".to_owned(),
        field_values: "            ranks,\n            ranks_by_season_and_level,\n            ranks_by_season,\n            max_rank_by_season,\n            maximum_influence_by_season,\n".to_owned(),
        initializers,
        methods,
        rows_type: "SeasonPassRank".to_owned(),
        rows_method: "ranks".to_owned(),
    })
}

fn card_templates(context: &AugmentationContext<'_>) -> Result<RustNativeManagerAugmentation> {
    let row = context.row("SeasonsRewardsCardTemplates")?;
    let parts = context.row_parts(row);
    let id = required_string(context, row, "CardAndRowID")?;
    let columns = (1..=4)
        .map(|column| {
            Ok(format!(
                "SeasonCardColumn {{ kind: {}, xp: {}, special: {} }}",
                optional_owned_string(context, row, &format!("Column{column}Type"), "source.row")?,
                optional_number(context, row, &format!("Column{column}XP"), "source.row")?,
                optional_owned_string(
                    context,
                    row,
                    &format!("Column{column}Special"),
                    "source.row"
                )?,
            ))
        })
        .collect::<Result<Vec<_>>>()?
        .join(", ");
    let declarations = format!(
        r#"{helpers}#[derive(Debug, Clone, PartialEq)]
pub struct SeasonCardColumn {{ pub kind: Option<String>, pub xp: Option<f32>, pub special: Option<String> }}

#[derive(Debug, Clone)]
pub struct SeasonCardTemplate {{
    pub id: Crc32,
    pub key: String,
    pub columns: [SeasonCardColumn; 4],
    source: RowRef<{table}, {row_type}>,
}}

impl SeasonCardTemplate {{
    pub fn table(&self) -> {table} {{ *self.source.table() }}
    pub fn source_ref(&self) -> &RowRef<{table}, {row_type}> {{ &self.source }}
}}
"#,
        helpers = numeric_helpers(),
        table = parts.table_type,
        row_type = parts.row_type,
    );
    let initializers = format!(
        r#"        let mut card_templates = Vec::new();
        let mut card_templates_by_table_and_id = HashMap::new();
        let mut card_templates_by_table: HashMap<{table}, Vec<usize>> = HashMap::new();
        for source in {row_field}.rows() {{
            let key = {id}.trim();
            let id = Crc32::from_str_lower(key);
            if key.is_empty() || id == Crc32::ZERO {{ continue; }}
            let table = *source.reference.table();
            if card_templates_by_table_and_id.contains_key(&(table, id)) {{ continue; }}
            let slot = card_templates.len();
            card_templates.push(SeasonCardTemplate {{ id, key: key.to_owned(), columns: [{columns}], source: source.reference.clone() }});
            card_templates_by_table_and_id.insert((table, id), slot);
            card_templates_by_table.entry(table).or_default().push(slot);
        }}
"#,
        table = parts.table_type,
        row_field = parts.row_field,
    );
    let methods = format!(
        r#"    pub fn card_template(&self, table: {table}, id: Crc32) -> Option<&SeasonCardTemplate> {{ self.card_templates_by_table_and_id.get(&(table, id)).and_then(|slot| self.card_templates.get(*slot)) }}
    pub fn card_template_by_key(&self, table: {table}, key: impl AsRef<str>) -> Option<&SeasonCardTemplate> {{ self.card_template(table, Crc32::from_str_lower(key.as_ref())) }}
    pub fn card_templates_for_table(&self, table: {table}) -> impl Iterator<Item = &SeasonCardTemplate> {{ self.card_templates_by_table.get(&table).into_iter().flatten().filter_map(|slot| self.card_templates.get(*slot)) }}
    pub fn card_templates(&self) -> std::slice::Iter<'_, SeasonCardTemplate> {{ self.card_templates.iter() }}
    pub fn card_template_source(&self, value: &SeasonCardTemplate) -> Option<&{row_type}> {{ self.{row_field}.get(&value.source) }}

"#,
        table = parts.table_type,
        row_type = parts.row_type,
        row_field = parts.row_field,
    );
    Ok(RustNativeManagerAugmentation {
        declarations,
        fields: format!("    card_templates: Vec<SeasonCardTemplate>,\n    card_templates_by_table_and_id: HashMap<({}, Crc32), usize>,\n    card_templates_by_table: HashMap<{}, Vec<usize>>,\n", parts.table_type, parts.table_type),
        field_values: "            card_templates,\n            card_templates_by_table_and_id,\n            card_templates_by_table,\n".to_owned(),
        initializers,
        methods,
        rows_type: "SeasonCardTemplate".to_owned(),
        rows_method: "card_templates".to_owned(),
    })
}

fn chapters(context: &AugmentationContext<'_>) -> Result<RustNativeManagerAugmentation> {
    let row = context.row("SeasonsRewardsChapterData")?;
    let parts = context.row_parts(row);
    let id = required_string(context, row, "ChapterID")?;
    let kind = optional_owned_string(context, row, "ChapterType", "source.row")?;
    let chapter_index = optional_number(context, row, "ChapterIndex", "source.row")?;
    let reward = optional_crc(context, row, "ChapterRewardID", "source.row")?;
    let declarations = format!(
        r#"{helpers}#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SeasonChapterKind {{ Chapter, Challenge, SeasonalServerChapter, SeasonalServerChallenge }}

impl SeasonChapterKind {{
    pub fn parse(value: &str) -> Option<Self> {{
        Some(match normalize_lookup_key(value).as_str() {{
            "chapter" => Self::Chapter, "challenge" => Self::Challenge,
            "seasonalserverchapter" => Self::SeasonalServerChapter,
            "seasonalserverchallenge" => Self::SeasonalServerChallenge,
            _ => return None,
        }})
    }}
}}

#[derive(Debug, Clone)]
pub struct SeasonChapter {{
    pub season_id: Crc32,
    pub id: Crc32,
    pub key: String,
    pub kind: SeasonChapterKind,
    pub index: Option<u32>,
    pub reward_id: Option<Crc32>,
    source: RowRef<{table}, {row_type}>,
}}

impl SeasonChapter {{ pub fn source_ref(&self) -> &RowRef<{table}, {row_type}> {{ &self.source }} }}
"#,
        helpers = season_table_helpers(),
        table = parts.table_type,
        row_type = parts.row_type,
    );
    let initializers = format!(
        r#"        let mut chapters = Vec::new();
        let mut chapters_by_id = HashMap::new();
        let mut chapters_by_reward = HashMap::new();
        let mut chapters_by_kind_index = HashMap::new();
        let mut chapters_by_season: HashMap<Crc32, Vec<usize>> = HashMap::new();
        for source in {row_field}.rows() {{
            let season_id = season_id_from_table_name(source.reference.table().catalog_name());
            let key = {id}.trim();
            let id = Crc32::from_str_lower(key);
            if season_id == Crc32::ZERO || key.is_empty() || id == Crc32::ZERO || chapters_by_id.contains_key(&(season_id, id)) {{ continue; }}
            let kind_text = {kind};
            let Some(kind) = kind_text.as_deref().and_then(SeasonChapterKind::parse) else {{ continue; }};
            let index = {chapter_index}.and_then(season_exact_u32);
            let reward_id = {reward};
            let slot = chapters.len();
            chapters.push(SeasonChapter {{ season_id, id, key: key.to_owned(), kind, index, reward_id, source: source.reference.clone() }});
            chapters_by_id.insert((season_id, id), slot);
            chapters_by_season.entry(season_id).or_default().push(slot);
            if let Some(reward_id) = reward_id {{ chapters_by_reward.entry((season_id, reward_id)).or_insert(slot); }}
            if let Some(index) = index {{ chapters_by_kind_index.entry((season_id, kind, index)).or_insert(slot); }}
        }}
        for slots in chapters_by_season.values_mut() {{ slots.sort_by_key(|slot| chapters[*slot].index.unwrap_or(u32::MAX)); }}
"#,
        row_field = parts.row_field,
    );
    let methods = format!(
        r#"    pub fn chapter(&self, season_id: Crc32, chapter_id: Crc32) -> Option<&SeasonChapter> {{ self.chapters_by_id.get(&(season_id, chapter_id)).and_then(|slot| self.chapters.get(*slot)) }}
    pub fn chapter_by_key(&self, season_key: impl AsRef<str>, chapter_key: impl AsRef<str>) -> Option<&SeasonChapter> {{ self.chapter(Crc32::from_str_lower(season_key.as_ref()), Crc32::from_str_lower(chapter_key.as_ref())) }}
    pub fn chapter_by_reward(&self, season_id: Crc32, reward_id: Crc32) -> Option<&SeasonChapter> {{ self.chapters_by_reward.get(&(season_id, reward_id)).and_then(|slot| self.chapters.get(*slot)) }}
    pub fn chapter_by_kind_index(&self, season_id: Crc32, kind: SeasonChapterKind, index: u32) -> Option<&SeasonChapter> {{ self.chapters_by_kind_index.get(&(season_id, kind, index)).and_then(|slot| self.chapters.get(*slot)) }}
    pub fn chapter_by_kind_key_index(&self, season_id: Crc32, kind: impl AsRef<str>, index: u32) -> Option<&SeasonChapter> {{ self.chapter_by_kind_index(season_id, SeasonChapterKind::parse(kind.as_ref())?, index) }}
    pub fn chapters_for_season(&self, season_id: Crc32) -> impl Iterator<Item = &SeasonChapter> {{ self.chapters_by_season.get(&season_id).into_iter().flatten().filter_map(|slot| self.chapters.get(*slot)) }}
    pub fn chapters(&self) -> std::slice::Iter<'_, SeasonChapter> {{ self.chapters.iter() }}
    pub fn chapter_source(&self, value: &SeasonChapter) -> Option<&{row_type}> {{ self.{row_field}.get(&value.source) }}

"#,
        row_type = parts.row_type,
        row_field = parts.row_field,
    );
    Ok(RustNativeManagerAugmentation {
        declarations,
        fields: "    chapters: Vec<SeasonChapter>,\n    chapters_by_id: HashMap<(Crc32, Crc32), usize>,\n    chapters_by_reward: HashMap<(Crc32, Crc32), usize>,\n    chapters_by_kind_index: HashMap<(Crc32, SeasonChapterKind, u32), usize>,\n    chapters_by_season: HashMap<Crc32, Vec<usize>>,\n".to_owned(),
        field_values: "            chapters,\n            chapters_by_id,\n            chapters_by_reward,\n            chapters_by_kind_index,\n            chapters_by_season,\n".to_owned(),
        initializers,
        methods,
        rows_type: "SeasonChapter".to_owned(),
        rows_method: "chapters".to_owned(),
    })
}

fn journeys(context: &AugmentationContext<'_>) -> Result<RustNativeManagerAugmentation> {
    let row = context.row("SeasonsRewardsJourneyData")?;
    let parts = context.row_parts(row);
    let id = required_string(context, row, "JourneyTaskID")?;
    let chapter = optional_crc(context, row, "Chapter", "source.row")?;
    let reward = optional_crc(context, row, "RewardID", "source.row")?;
    let task = optional_owned_string(context, row, "Task", "source.row")?;
    let sort_order = optional_number(context, row, "SortOrder", "source.row")?;
    let declarations = format!(
        r#"{helpers}#[derive(Debug, Clone)]
pub struct SeasonJourneyTask {{
    pub season_id: Crc32,
    pub id: Crc32,
    pub key: String,
    pub chapter_id: Crc32,
    pub reward_id: Crc32,
    pub task: Option<String>,
    pub sort_order: u32,
    source: RowRef<{table}, {row_type}>,
}}

impl SeasonJourneyTask {{ pub fn source_ref(&self) -> &RowRef<{table}, {row_type}> {{ &self.source }} }}
"#,
        helpers = season_table_helpers(),
        table = parts.table_type,
        row_type = parts.row_type,
    );
    let initializers = format!(
        r#"        let mut journey_tasks = Vec::new();
        let mut journey_tasks_by_id = HashMap::new();
        let mut journey_tasks_by_reward = HashMap::new();
        let mut journey_tasks_by_chapter: HashMap<(Crc32, Crc32), Vec<usize>> = HashMap::new();
        let mut journey_tasks_by_season: HashMap<Crc32, Vec<usize>> = HashMap::new();
        for source in {row_field}.rows() {{
            let season_id = season_id_from_table_name(source.reference.table().catalog_name());
            let key = {id}.trim();
            let id = Crc32::from_str_lower(key);
            let Some(chapter_id) = {chapter} else {{ continue; }};
            let Some(reward_id) = {reward} else {{ continue; }};
            if season_id == Crc32::ZERO || key.is_empty() || id == Crc32::ZERO || journey_tasks_by_id.contains_key(&(season_id, id)) {{ continue; }}
            let slot = journey_tasks.len();
            journey_tasks.push(SeasonJourneyTask {{ season_id, id, key: key.to_owned(), chapter_id, reward_id, task: {task}, sort_order: {sort_order}.and_then(season_exact_u32).unwrap_or_default(), source: source.reference.clone() }});
            journey_tasks_by_id.insert((season_id, id), slot);
            journey_tasks_by_reward.entry((season_id, reward_id)).or_insert(slot);
            journey_tasks_by_chapter.entry((season_id, chapter_id)).or_default().push(slot);
            journey_tasks_by_season.entry(season_id).or_default().push(slot);
        }}
        for slots in journey_tasks_by_season.values_mut() {{ slots.sort_by_key(|slot| journey_tasks[*slot].sort_order); }}
        for slots in journey_tasks_by_chapter.values_mut() {{ slots.sort_by_key(|slot| journey_tasks[*slot].sort_order); }}
"#,
        row_field = parts.row_field,
    );
    let methods = format!(
        r#"    pub fn journey_task(&self, season_id: Crc32, task_id: Crc32) -> Option<&SeasonJourneyTask> {{ self.journey_tasks_by_id.get(&(season_id, task_id)).and_then(|slot| self.journey_tasks.get(*slot)) }}
    pub fn journey_task_by_key(&self, season_key: impl AsRef<str>, task_key: impl AsRef<str>) -> Option<&SeasonJourneyTask> {{ self.journey_task(Crc32::from_str_lower(season_key.as_ref()), Crc32::from_str_lower(task_key.as_ref())) }}
    pub fn journey_task_by_reward(&self, season_id: Crc32, reward_id: Crc32) -> Option<&SeasonJourneyTask> {{ self.journey_tasks_by_reward.get(&(season_id, reward_id)).and_then(|slot| self.journey_tasks.get(*slot)) }}
    pub fn journeys_for_season(&self, season_id: Crc32) -> impl Iterator<Item = &SeasonJourneyTask> {{ self.journey_tasks_by_season.get(&season_id).into_iter().flatten().filter_map(|slot| self.journey_tasks.get(*slot)) }}
    pub fn journeys_for_chapter(&self, season_id: Crc32, chapter_id: Crc32) -> impl Iterator<Item = &SeasonJourneyTask> {{ self.journey_tasks_by_chapter.get(&(season_id, chapter_id)).into_iter().flatten().filter_map(|slot| self.journey_tasks.get(*slot)) }}
    pub fn journey_tasks(&self) -> std::slice::Iter<'_, SeasonJourneyTask> {{ self.journey_tasks.iter() }}
    pub fn journey_task_source(&self, value: &SeasonJourneyTask) -> Option<&{row_type}> {{ self.{row_field}.get(&value.source) }}

"#,
        row_type = parts.row_type,
        row_field = parts.row_field,
    );
    Ok(RustNativeManagerAugmentation {
        declarations,
        fields: "    journey_tasks: Vec<SeasonJourneyTask>,\n    journey_tasks_by_id: HashMap<(Crc32, Crc32), usize>,\n    journey_tasks_by_reward: HashMap<(Crc32, Crc32), usize>,\n    journey_tasks_by_chapter: HashMap<(Crc32, Crc32), Vec<usize>>,\n    journey_tasks_by_season: HashMap<Crc32, Vec<usize>>,\n".to_owned(),
        field_values: "            journey_tasks,\n            journey_tasks_by_id,\n            journey_tasks_by_reward,\n            journey_tasks_by_chapter,\n            journey_tasks_by_season,\n".to_owned(),
        initializers,
        methods,
        rows_type: "SeasonJourneyTask".to_owned(),
        rows_method: "journey_tasks".to_owned(),
    })
}

fn song_sheets(context: &AugmentationContext<'_>) -> Result<RustNativeManagerAugmentation> {
    let row = context.row("SongBookSheets")?;
    let parts = context.row_parts(row);
    let id = required_string(context, row, "SheetID")?;
    let instrument = optional_owned_string(context, row, "Instrument", "source.row")?;
    let name = optional_owned_string(context, row, "SheetName", "source.row")?;
    let pages = optional_crc_list(context, row, "Pages", "source.row")?;
    let declarations = format!(
        r#"{helpers}#[derive(Debug, Clone)]
pub struct SongBookSheet {{
    pub id: Crc32,
    pub key: String,
    pub instrument: String,
    pub name: Option<String>,
    pub page_ids: Vec<Crc32>,
    source: RowRef<{table}, {row_type}>,
}}

impl SongBookSheet {{ pub fn source_ref(&self) -> &RowRef<{table}, {row_type}> {{ &self.source }} }}
"#,
        helpers = numeric_helpers(),
        table = parts.table_type,
        row_type = parts.row_type,
    );
    let initializers = format!(
        r#"        let mut sheets = Vec::new();
        let mut sheets_by_id = HashMap::new();
        let mut sheets_by_instrument: HashMap<String, Vec<usize>> = HashMap::new();
        let mut sheet_ids_by_page: HashMap<Crc32, Vec<Crc32>> = HashMap::new();
        for source in {row_field}.rows() {{
            let key = {id}.trim();
            let id = Crc32::from_str_lower(key);
            if key.is_empty() || id == Crc32::ZERO || sheets_by_id.contains_key(&id) {{ continue; }}
            let Some(instrument) = {instrument} else {{ continue; }};
            let Some(instrument_data) = _musical_instrument_slot_data.musical_instrument_slot(&instrument) else {{ continue; }};
            let page_ids = {pages};
            let slot = sheets.len();
            let name = {name}.or_else(|| Some(instrument_data.name.clone()));
            sheets.push(SongBookSheet {{ id, key: key.to_owned(), instrument: instrument.clone(), name, page_ids: page_ids.clone(), source: source.reference.clone() }});
            sheets_by_id.insert(id, slot);
            if !instrument.is_empty() {{ sheets_by_instrument.entry(normalize_lookup_key(&instrument)).or_default().push(slot); }}
            for page_id in page_ids {{ let values = sheet_ids_by_page.entry(page_id).or_default(); if !values.contains(&id) {{ values.push(id); }} }}
        }}
"#,
        row_field = parts.row_field,
    );
    let methods = format!(
        r#"    pub fn sheet_from_id(&self, id: Crc32) -> Option<&SongBookSheet> {{ self.sheets_by_id.get(&id).and_then(|slot| self.sheets.get(*slot)) }}
    pub fn sheet(&self, key: impl AsRef<str>) -> Option<&SongBookSheet> {{ self.sheet_from_id(Crc32::from_str_lower(key.as_ref())) }}
    pub fn sheets_for_instrument(&self, instrument: impl AsRef<str>) -> impl Iterator<Item = &SongBookSheet> {{ self.sheets_by_instrument.get(&normalize_lookup_key(instrument.as_ref())).into_iter().flatten().filter_map(|slot| self.sheets.get(*slot)) }}
    pub fn sheet_ids_for_page(&self, page_id: Crc32) -> impl Iterator<Item = Crc32> + '_ {{ self.sheet_ids_by_page.get(&page_id).into_iter().flatten().copied() }}
    pub fn instrument_for_sheet(&self, sheet_id: Crc32) -> Option<&str> {{ self.sheet_from_id(sheet_id).map(|sheet| sheet.instrument.as_str()) }}
    pub fn page_ids_for_sheet(&self, sheet_id: Crc32) -> impl Iterator<Item = Crc32> + '_ {{ self.sheet_from_id(sheet_id).into_iter().flat_map(|sheet| sheet.page_ids.iter().copied()) }}
    pub fn sheets(&self) -> std::slice::Iter<'_, SongBookSheet> {{ self.sheets.iter() }}
    pub fn sheet_source(&self, value: &SongBookSheet) -> Option<&{row_type}> {{ self.{row_field}.get(&value.source) }}

"#,
        row_type = parts.row_type,
        row_field = parts.row_field,
    );
    Ok(RustNativeManagerAugmentation {
        declarations,
        fields: "    sheets: Vec<SongBookSheet>,\n    sheets_by_id: HashMap<Crc32, usize>,\n    sheets_by_instrument: HashMap<String, Vec<usize>>,\n    sheet_ids_by_page: HashMap<Crc32, Vec<Crc32>>,\n".to_owned(),
        field_values: "            sheets,\n            sheets_by_id,\n            sheets_by_instrument,\n            sheet_ids_by_page,\n".to_owned(),
        initializers,
        methods,
        rows_type: "SongBookSheet".to_owned(),
        rows_method: "sheets".to_owned(),
    })
}

fn songs(context: &AugmentationContext<'_>) -> Result<RustNativeManagerAugmentation> {
    let row = context.row("SongBookData")?;
    let parts = context.row_parts(row);
    let id = required_string(context, row, "SongID")?;
    let title = optional_owned_string(context, row, "MusicTitle", "source.row")?;
    let performance = optional_owned_string(context, row, "Performance", "source.row")?;
    let pattern = optional_owned_string(context, row, "NotePattern", "source.row")?;
    let sound_bank = optional_owned_string(context, row, "SoundBank", "source.row")?;
    let crowd = optional_owned_string(context, row, "Crowd", "source.row")?;
    let dance_timeline = optional_owned_string(context, row, "DanceTimeline", "source.row")?;
    let tempo = optional_number(context, row, "Tempo", "source.row")?;
    let scroll_speed = optional_number(context, row, "ScrollSpeed", "source.row")?;
    let duration = optional_number(context, row, "Duration", "source.row")?;
    let hit_zone = optional_number(context, row, "HitZone", "source.row")?;
    let perfect_zone = optional_number(context, row, "PerfectZone", "source.row")?;
    let tipping_duration = optional_number(context, row, "TippingDurationMs", "source.row")?;
    let filter_type_key = optional_owned_string(context, row, "FilterType", "source.row")?;
    let completion_event_key =
        optional_owned_string(context, row, "CompletionGameEvent", "source.row")?;
    let highest_rank_achievement_key =
        optional_owned_string(context, row, "HighestRankAchievementId", "source.row")?;
    let slots = (1..=5)
        .filter_map(|slot| {
            let column = format!("Slot{slot:02}");
            row.fields
                .iter()
                .find(|field| field.source_name.eq_ignore_ascii_case(&column))
                .map(|field| string_value_expression(field, "source.row"))
        })
        .collect::<Result<Vec<_>>>()?;
    let slot_initializers = slots
        .iter()
        .map(|value| {
            format!(
                "            let sheet_id = Crc32::from_str_lower({value}.trim());\n            if sheet_id != Crc32::ZERO && _song_book_sheet_data.sheet_from_id(sheet_id).is_some() && !sheet_ids.contains(&sheet_id) {{ sheet_ids.push(sheet_id); }}\n"
            )
        })
        .collect::<String>();
    let declarations = format!(
        r#"{helpers}#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SongPerformance {{ Unknown, Easy, Medium, Hard }}

impl SongPerformance {{
    pub fn parse(value: &str) -> Option<Self> {{
        Some(match normalize_lookup_key(value).as_str() {{
            "unknown" => Self::Unknown, "easy" => Self::Easy, "medium" => Self::Medium, "hard" => Self::Hard,
            _ => return None,
        }})
    }}
}}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SongPattern {{ Novice, Skilled, Expert }}

impl SongPattern {{
    pub fn parse(value: &str) -> Option<Self> {{
        Some(match normalize_lookup_key(value).as_str() {{
            "novice" => Self::Novice, "skilled" => Self::Skilled, "expert" => Self::Expert,
            _ => return None,
        }})
    }}
}}

#[derive(Debug, Clone)]
pub struct SongBookSong {{
    pub id: Crc32,
    pub key: String,
    pub title: String,
    pub performance: SongPerformance,
    pub note_pattern: SongPattern,
    pub sound_bank: String,
    pub crowd: String,
    pub dance_timeline: String,
    pub tempo: std::num::NonZeroU32,
    pub scroll_speed: f32,
    pub duration: f32,
    pub hit_zone: f32,
    pub perfect_zone: f32,
    pub tipping_duration_ms: f32,
    pub filter_type_key: String,
    pub filter_type: Crc32,
    pub completion_game_event_key: String,
    pub completion_game_event: Crc32,
    pub highest_rank_achievement_key: Option<String>,
    pub highest_rank_achievement_row: Option<u32>,
    pub sheet_ids: Vec<Crc32>,
    pub page_ids: Vec<Crc32>,
    source: RowRef<{table}, {row_type}>,
}}

impl SongBookSong {{ pub fn source_ref(&self) -> &RowRef<{table}, {row_type}> {{ &self.source }} }}
"#,
        helpers = numeric_helpers(),
        table = parts.table_type,
        row_type = parts.row_type,
    );
    let initializers = format!(
        r#"        let mut songs = Vec::new();
        let mut songs_by_id = HashMap::new();
        let mut song_sheet_ids = Vec::new();
        let mut song_page_ids = Vec::new();
        let mut sheet_ids_by_page: HashMap<Crc32, Vec<Crc32>> = HashMap::new();
        let mut sheet_ids_by_instrument: HashMap<String, Vec<Crc32>> = HashMap::new();
        for source in {row_field}.rows() {{
            let key = {id}.trim();
            let id = Crc32::from_str_lower(key);
            if key.is_empty() || id == Crc32::ZERO || songs_by_id.contains_key(&id) {{ continue; }}
            let performance_text = {performance};
            let Some(performance) = performance_text.as_deref().and_then(SongPerformance::parse) else {{ continue; }};
            let pattern_text = {pattern};
            let Some(note_pattern) = pattern_text.as_deref().and_then(SongPattern::parse) else {{ continue; }};
            let Some(tempo) = {tempo}.and_then(season_exact_nonzero_u32) else {{ continue; }};
            let Some(scroll_speed) = {scroll_speed}.and_then(season_finite_f32) else {{ continue; }};
            let Some(duration) = {duration}.and_then(season_finite_f32) else {{ continue; }};
            let Some(hit_zone) = {hit_zone}.and_then(season_finite_f32) else {{ continue; }};
            let Some(perfect_zone) = {perfect_zone}.and_then(season_finite_f32) else {{ continue; }};
            let Some(tipping_duration_ms) = {tipping_duration}.and_then(season_finite_f32) else {{ continue; }};
            let filter_type_key = {filter_type_key}.unwrap_or_default();
            let filter_type = season_crc(&filter_type_key).unwrap_or(Crc32::ZERO);
            let completion_game_event_key = {completion_event_key}.unwrap_or_default();
            let completion_game_event = season_crc(&completion_game_event_key).unwrap_or(Crc32::ZERO);
            let highest_rank_achievement_key = {highest_rank_achievement_key};
            let highest_rank_achievement_row = highest_rank_achievement_key.as_deref().and_then(season_row_index);
            let mut sheet_ids = Vec::new();
{slot_initializers}            let mut page_ids = Vec::new();
            for sheet_id in &sheet_ids {{
                if !song_sheet_ids.contains(sheet_id) {{ song_sheet_ids.push(*sheet_id); }}
                if let Some(instrument) = _song_book_sheet_data.instrument_for_sheet(*sheet_id) {{ let values = sheet_ids_by_instrument.entry(normalize_lookup_key(instrument)).or_default(); if !values.contains(sheet_id) {{ values.push(*sheet_id); }} }}
                for page_id in _song_book_sheet_data.page_ids_for_sheet(*sheet_id) {{
                    if !page_ids.contains(&page_id) {{ page_ids.push(page_id); }}
                    if !song_page_ids.contains(&page_id) {{ song_page_ids.push(page_id); }}
                    let values = sheet_ids_by_page.entry(page_id).or_default(); if !values.contains(sheet_id) {{ values.push(*sheet_id); }}
                }}
            }}
            let slot = songs.len();
            songs.push(SongBookSong {{
                id,
                key: key.to_owned(),
                title: {title}.unwrap_or_default(),
                performance,
                note_pattern,
                sound_bank: {sound_bank}.unwrap_or_default(),
                crowd: {crowd}.unwrap_or_default(),
                dance_timeline: {dance_timeline}.unwrap_or_default(),
                tempo,
                scroll_speed,
                duration,
                hit_zone,
                perfect_zone,
                tipping_duration_ms,
                filter_type_key,
                filter_type,
                completion_game_event_key,
                completion_game_event,
                highest_rank_achievement_key,
                highest_rank_achievement_row,
                sheet_ids,
                page_ids,
                source: source.reference.clone(),
            }});
            songs_by_id.insert(id, slot);
        }}
"#,
        row_field = parts.row_field,
    );
    let methods = format!(
        r#"    pub fn song_from_id(&self, id: Crc32) -> Option<&SongBookSong> {{ self.songs_by_id.get(&id).and_then(|slot| self.songs.get(*slot)) }}
    pub fn song(&self, key: impl AsRef<str>) -> Option<&SongBookSong> {{ self.song_from_id(Crc32::from_str_lower(key.as_ref())) }}
    pub fn sheet_ids(&self) -> impl Iterator<Item = Crc32> + '_ {{ self.song_sheet_ids.iter().copied() }}
    pub fn page_ids(&self) -> impl Iterator<Item = Crc32> + '_ {{ self.song_page_ids.iter().copied() }}
    pub fn sheet_ids_for_page(&self, page_id: Crc32) -> impl Iterator<Item = Crc32> + '_ {{ self.sheet_ids_by_page.get(&page_id).into_iter().flatten().copied() }}
    pub fn sheet_ids_for_instrument(&self, instrument: impl AsRef<str>) -> impl Iterator<Item = Crc32> + '_ {{ self.sheet_ids_by_instrument.get(&normalize_lookup_key(instrument.as_ref())).into_iter().flatten().copied() }}
    pub fn songs(&self) -> std::slice::Iter<'_, SongBookSong> {{ self.songs.iter() }}
    pub fn song_source(&self, value: &SongBookSong) -> Option<&{row_type}> {{ self.{row_field}.get(&value.source) }}

"#,
        row_type = parts.row_type,
        row_field = parts.row_field,
    );
    Ok(RustNativeManagerAugmentation {
        declarations,
        fields: "    songs: Vec<SongBookSong>,\n    songs_by_id: HashMap<Crc32, usize>,\n    song_sheet_ids: Vec<Crc32>,\n    song_page_ids: Vec<Crc32>,\n    sheet_ids_by_page: HashMap<Crc32, Vec<Crc32>>,\n    sheet_ids_by_instrument: HashMap<String, Vec<Crc32>>,\n".to_owned(),
        field_values: "            songs,\n            songs_by_id,\n            song_sheet_ids,\n            song_page_ids,\n            sheet_ids_by_page,\n            sheet_ids_by_instrument,\n".to_owned(),
        initializers,
        methods,
        rows_type: "SongBookSong".to_owned(),
        rows_method: "songs".to_owned(),
    })
}

fn required_string(
    context: &AugmentationContext<'_>,
    row: &RustStandaloneSchemaRow,
    column: &str,
) -> Result<String> {
    let field = context.field(row, column)?;
    require_string_field(context.manager, row, field)?;
    string_value_expression(field, "source.row")
}

fn optional_owned_string(
    context: &AugmentationContext<'_>,
    row: &RustStandaloneSchemaRow,
    column: &str,
    receiver: &str,
) -> Result<String> {
    let Some(field) = field_if(row, column) else {
        return Ok("None".to_owned());
    };
    require_string_field(context.manager, row, field)?;
    Ok(if field.required {
        format!("season_owned_text(&{receiver}.{})", field.field_name)
    } else {
        format!(
            "{receiver}.{}.as_deref().and_then(season_owned_text)",
            field.field_name
        )
    })
}

fn optional_crc(
    context: &AugmentationContext<'_>,
    row: &RustStandaloneSchemaRow,
    column: &str,
    receiver: &str,
) -> Result<String> {
    let Some(field) = field_if(row, column) else {
        return Ok("None".to_owned());
    };
    require_string_field(context.manager, row, field)?;
    Ok(if field.required {
        format!("season_crc(&{receiver}.{})", field.field_name)
    } else {
        format!(
            "{receiver}.{}.as_deref().and_then(season_crc)",
            field.field_name
        )
    })
}

fn optional_crc_list(
    context: &AugmentationContext<'_>,
    row: &RustStandaloneSchemaRow,
    column: &str,
    receiver: &str,
) -> Result<String> {
    let Some(field) = field_if(row, column) else {
        return Ok("Vec::new()".to_owned());
    };
    require_string_field(context.manager, row, field)?;
    Ok(if field.required {
        format!("season_crc_list(&{receiver}.{})", field.field_name)
    } else {
        format!(
            "{receiver}.{}.as_deref().map(season_crc_list).unwrap_or_default()",
            field.field_name
        )
    })
}

fn optional_number(
    context: &AugmentationContext<'_>,
    row: &RustStandaloneSchemaRow,
    column: &str,
    receiver: &str,
) -> Result<String> {
    let Some(field) = field_if(row, column) else {
        return Ok("None".to_owned());
    };
    if field.column_type != ColumnType::Number {
        bail!(
            "standalone Rust season manager `{}` requires numeric column `{column}`",
            context.manager.manager_name
        );
    }
    Ok(if field.required {
        format!("Some({receiver}.{})", field.field_name)
    } else {
        format!("{receiver}.{}", field.field_name)
    })
}

fn optional_boolish(
    context: &AugmentationContext<'_>,
    row: &RustStandaloneSchemaRow,
    column: &str,
    receiver: &str,
) -> Result<String> {
    let Some(field) = field_if(row, column) else {
        return Ok("false".to_owned());
    };
    match (field.column_type, field.required) {
        (ColumnType::Boolean, true) => Ok(format!("{receiver}.{}", field.field_name)),
        (ColumnType::Boolean, false) => {
            Ok(format!("{receiver}.{}.unwrap_or(false)", field.field_name))
        }
        (ColumnType::String, true) => Ok(format!("season_bool(&{receiver}.{})", field.field_name)),
        (ColumnType::String, false) => Ok(format!(
            "{receiver}.{}.as_deref().is_some_and(season_bool)",
            field.field_name
        )),
        _ => bail!(
            "standalone Rust season manager `{}` requires boolean-like column `{column}`",
            context.manager.manager_name
        ),
    }
}

fn field_if<'a>(
    row: &'a RustStandaloneSchemaRow,
    column: &str,
) -> Option<&'a RustStandaloneSchemaField> {
    row.fields
        .iter()
        .find(|field| field.source_name.eq_ignore_ascii_case(column))
}

fn numeric_helpers() -> &'static str {
    r#"fn season_exact_u32(value: f32) -> Option<u32> {
    let value = f64::from(value);
    (value.is_finite() && value >= 0.0 && value.fract() == 0.0 && value <= f64::from(u32::MAX)).then_some(value as u32)
}

fn season_exact_nonzero_u32(value: f32) -> Option<std::num::NonZeroU32> {
    std::num::NonZeroU32::new(season_exact_u32(value)?)
}

fn season_finite_f32(value: f32) -> Option<f32> {
    value.is_finite().then_some(value)
}

fn season_row_index(value: &str) -> Option<u32> {
    let value = value.trim().parse::<u32>().ok()?;
    (value != 0).then_some(value)
}

fn season_owned_text(value: &str) -> Option<String> {
    let value = value.trim();
    (!value.is_empty()).then(|| value.to_owned())
}

fn season_crc(value: &str) -> Option<Crc32> {
    let value = value.trim();
    let id = Crc32::from_str_lower(value);
    (!value.is_empty() && id != Crc32::ZERO).then_some(id)
}

fn season_crc_list(value: &str) -> Vec<Crc32> {
    let mut values = Vec::new();
    for item in split_designer_list(value) {
        if let Some(id) = season_crc(item) { if !values.contains(&id) { values.push(id); } }
    }
    values
}

fn season_bool(value: &str) -> bool {
    matches!(value.trim().to_ascii_lowercase().as_str(), "1" | "true" | "yes")
}

"#
}

fn season_table_helpers() -> String {
    format!(
        "{}{}",
        numeric_helpers(),
        r#"fn season_id_from_table_name(name: &str) -> Crc32 {
    name.rsplit_once('_').and_then(|(_, suffix)| season_crc(suffix)).unwrap_or(Crc32::ZERO)
}

"#
    )
}
