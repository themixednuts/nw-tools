//! Standalone Rust augmentations for the manager families with bespoke semantics.

use super::*;

pub(super) fn augmentation(
    context: &AugmentationContext<'_>,
    shape: &NativeManagerShape,
) -> Option<Result<RustNativeManagerAugmentation>> {
    Some(match shape {
        NativeManagerShape::ObjectivesData(_) => objectives(context),
        NativeManagerShape::ContributionData(_) => context.contribution(),
        NativeManagerShape::BuffBucketData(_) => buff_bucket(context),
        NativeManagerShape::StructureData(_) => structure(context),
        NativeManagerShape::ReusableScoreboardData(_) => reusable_scoreboard(context),
        NativeManagerShape::MountHitVolumeData(_) => mount_hit_volume(context),
        NativeManagerShape::OneTableExperience(_) => experience(context),
        NativeManagerShape::DamageData(_) => damage(context),
        NativeManagerShape::VitalsData(_) => vitals(context),
        NativeManagerShape::ElementalMutationStaticData(_) => elemental_mutations(context),
        NativeManagerShape::PromotionMutationStaticData(_) => promotion_mutations(context),
        NativeManagerShape::MusicalRewardsData(_) => musical_rewards(context),
        NativeManagerShape::CombatProfilesData(_) => combat_profiles(context),
        NativeManagerShape::GatherableData(_) => gatherable(context),
        NativeManagerShape::SocialData(_) => social(context),
        NativeManagerShape::PlayerData(_) => player(context),
        NativeManagerShape::RecipeData(_) => recipe(context),
        NativeManagerShape::TradeskillRankData(_) => tradeskill_rank(context),
        _ => return None,
    })
}

fn objectives(context: &AugmentationContext<'_>) -> Result<RustNativeManagerAugmentation> {
    Ok(RustNativeManagerAugmentation::merged([
        context.simple_crc(
            "Objectives",
            "ObjectiveID",
            &["objective_data_from_id"],
            &["objective_data"],
            "objectives",
        )?,
        context.simple_crc(
            "ObjectiveTasks",
            "TaskID",
            &["objective_task_data_from_id"],
            &["objective_task_data"],
            "objective_tasks",
        )?,
        context.named_rows("Objectives", "objectives")?,
        context.named_rows("ObjectiveTasks", "objective_tasks")?,
    ]))
}

fn structure(context: &AugmentationContext<'_>) -> Result<RustNativeManagerAugmentation> {
    Ok(RustNativeManagerAugmentation::merged([
        context.simple_crc(
            "StructureFootprintData",
            "FootprintID",
            &["structure_footprint_data_from_id"],
            &["structure_footprint_data"],
            "structure_footprints",
        )?,
        context.simple_crc(
            "StructurePieceData",
            "StructurePieceID",
            &["structure_piece_data_from_id"],
            &["structure_piece_data"],
            "structure_pieces",
        )?,
        context.named_rows("StructureFootprintData", "footprints")?,
        context.named_rows("StructurePieceData", "pieces")?,
    ]))
}

fn reusable_scoreboard(context: &AugmentationContext<'_>) -> Result<RustNativeManagerAugmentation> {
    let mut values = Vec::new();
    if context
        .rows
        .iter()
        .any(|row| row.source_row_type == "ReusableScoreboardTabData")
    {
        values.push(context.simple_crc(
            "ReusableScoreboardTabData",
            "ReusableScoreboardTabId",
            &["reusable_scoreboard_data_from_id"],
            &["reusable_scoreboard_data"],
            "reusable_scoreboards",
        )?);
        values.push(context.named_rows("ReusableScoreboardTabData", "scoreboards")?);
    }
    if context
        .rows
        .iter()
        .any(|row| row.source_row_type == "PugActivityData")
    {
        values.push(context.simple_crc(
            "PugActivityData",
            "PugActivityId",
            &["pug_activity_data_from_id"],
            &["pug_activity_data"],
            "pug_activities",
        )?);
        values.push(context.named_rows("PugActivityData", "pug_activities")?);
    }
    if values.is_empty() {
        return context.unsupported(
            "ReusableScoreboardData",
            "neither scoreboard nor PUG activity schema rows are present",
        );
    }
    Ok(RustNativeManagerAugmentation::merged(values))
}

fn mount_hit_volume(context: &AugmentationContext<'_>) -> Result<RustNativeManagerAugmentation> {
    let row = context.find_first_row(&["MountTypeData", "MountHitVolumeData"])?;
    let row_name = row.source_row_type.as_str();
    let key_name = if field(row, "MountID").is_some() {
        "MountID"
    } else {
        "MountTypeID"
    };
    let mut value = context.simple_crc(
        row_name,
        key_name,
        &["mount_hit_volume_from_mount_type_id"],
        &["mount_hit_volume"],
        "mount_hit_volume",
    )?;
    let Some(prefab) = [
        "MountHitVolumePrefab",
        "PrefabPath",
        "Prefab",
        "MountPrefabPath",
    ]
    .into_iter()
    .find_map(|name| field(row, name)) else {
        return Ok(value);
    };
    let parts = context.row_parts(row);
    let prefab_value = string_value_expression(prefab, "source.row")?;
    value.fields.push_str(&format!(
        "    mount_hit_volumes_by_prefab: HashMap<Crc32, Vec<RowRef<{}, {}>>>,\n",
        parts.table_type, parts.row_type
    ));
    value
        .field_values
        .push_str("            mount_hit_volumes_by_prefab,\n");
    value.initializers.push_str(&format!(
        "        let mut mount_hit_volumes_by_prefab = HashMap::<Crc32, Vec<RowRef<{}, {}>>>::new();\n        for source in {}.rows() {{\n            let prefab = {}.trim();\n            if prefab.is_empty() {{ continue; }}\n            let id = Crc32::from_str_lower(prefab);\n            if id != Crc32::ZERO {{ mount_hit_volumes_by_prefab.entry(id).or_default().push(source.reference.clone()); }}\n        }}\n",
        parts.table_type, parts.row_type, parts.row_field, prefab_value
    ));
    value.methods.push_str(&format!(
        "    pub fn mount_hit_volumes_for_prefab_from_id(&self, id: Crc32) -> impl Iterator<Item = &{}> {{\n        self.mount_hit_volumes_by_prefab.get(&id).into_iter().flatten().filter_map(|reference| self.{}.get(reference))\n    }}\n\n    pub fn mount_hit_volumes_for_prefab(&self, prefab: &str) -> impl Iterator<Item = &{}> {{\n        self.mount_hit_volumes_for_prefab_from_id(Crc32::from_str_lower(prefab))\n    }}\n\n",
        parts.row_type, parts.row_field, parts.row_type
    ));
    Ok(value)
}

fn buff_bucket(context: &AugmentationContext<'_>) -> Result<RustNativeManagerAugmentation> {
    let row = context.row("BuffBucketData")?;
    let parts = context.row_parts(row);
    let bucket_id = text(context, row, "BuffBucketID", "source.row")?;
    let table_type = text(context, row, "TableType", "source.row")?;
    let max_roll = number(context, row, "MaxRoll", "probability.row", "0.0")?;
    let mut slots = String::new();
    for slot in 1..=6u8 {
        let Some(buff_field) = field(row, &format!("Buff{slot}")) else {
            continue;
        };
        let Some(kind_field) = field(row, &format!("BuffType{slot}")) else {
            continue;
        };
        let Some(potency_field) = field(row, &format!("BuffPotency{slot}")) else {
            continue;
        };
        let key = string_value_expression(buff_field, "source.row")?;
        let threshold = string_value_expression(buff_field, "probability.row")?;
        let kind = string_value_expression(kind_field, "source.row")?;
        let potency = numeric_value_or_default_expression(potency_field, "source.row", "0.0")?;
        slots.push_str(&format!(
            "            {{\n                let buff_key = {key}.trim();\n                if !buff_key.is_empty() {{\n                    let Some(kind) = BuffBucketEntryKind::parse({kind}.trim()) else {{ continue; }};\n                    let Ok(roll_threshold) = {threshold}.trim().parse::<u32>() else {{ continue; }};\n                    data.entries.push(BuffBucketEntry {{ slot: {slot}, roll_threshold, buff_key: buff_key.to_owned(), buff_id: Crc32::from_str_lower(buff_key), kind, potency: {potency} }});\n                }}\n            }}\n"
        ));
    }
    Ok(RustNativeManagerAugmentation {
        declarations: format!(
            r#"#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BuffBucketTableType {{ And, Or }}

impl BuffBucketTableType {{
    fn parse(value: &str) -> Option<Self> {{
        match value {{ "AND" => Some(Self::And), "OR" => Some(Self::Or), _ => None }}
    }}
}}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BuffBucketEntryKind {{ StatusEffect, Ability, BuffBucket, Promotion }}

impl BuffBucketEntryKind {{
    fn parse(value: &str) -> Option<Self> {{
        match value {{
            "StatusEffect" => Some(Self::StatusEffect),
            "Ability" => Some(Self::Ability),
            "BuffBucket" => Some(Self::BuffBucket),
            "Promotion" => Some(Self::Promotion),
            _ => None,
        }}
    }}
}}

#[derive(Debug, Clone, PartialEq)]
pub struct BuffBucketEntry {{
    pub slot: u8,
    pub roll_threshold: u32,
    pub buff_key: String,
    pub buff_id: Crc32,
    pub kind: BuffBucketEntryKind,
    pub potency: f32,
}}

#[derive(Debug, Clone, PartialEq)]
pub struct StaticBuffBucketData {{
    pub source: RowRef<{table}, {schema}>,
    pub probability_source: RowRef<{table}, {schema}>,
    pub bucket_key: String,
    pub bucket_id: Crc32,
    pub table_type: BuffBucketTableType,
    pub max_roll: u32,
    pub entries: Vec<BuffBucketEntry>,
}}

"#,
            table = parts.table_type,
            schema = parts.row_type
        ),
        fields: "    buff_buckets: Vec<StaticBuffBucketData>,\n    buff_buckets_by_id: HashMap<Crc32, usize>,\n".to_owned(),
        field_values: "            buff_buckets,\n            buff_buckets_by_id,\n".to_owned(),
        initializers: format!(
            r#"        let mut buff_bucket_sources = HashMap::<String, _>::new();
        for source in {rows}.rows() {{
            let key = {bucket_id}.trim();
            if !key.is_empty() {{ buff_bucket_sources.entry(key.to_owned()).or_insert(source); }}
        }}
        let mut buff_buckets = Vec::new();
        let mut buff_buckets_by_id = HashMap::new();
        for source in {rows}.rows() {{
            let key = {bucket_id}.trim();
            if key.is_empty() || key.ends_with("_Probs") {{ continue; }}
            let id = Crc32::from_str_lower(key);
            if id == Crc32::ZERO || buff_buckets_by_id.contains_key(&id) {{ continue; }}
            let Some(table_type) = BuffBucketTableType::parse({table_type}.trim()) else {{ continue; }};
            let Some(probability) = buff_bucket_sources.get(&format!("{{key}}_Probs")) else {{ continue; }};
            let max_roll_value = {max_roll};
            if !max_roll_value.is_finite()
                || max_roll_value < 0.0
                || max_roll_value.fract() != 0.0
                || f64::from(max_roll_value) > f64::from(u32::MAX)
            {{
                continue;
            }}
            let mut data = StaticBuffBucketData {{ source: source.reference.clone(), probability_source: probability.reference.clone(), bucket_key: key.to_owned(), bucket_id: id, table_type, max_roll: max_roll_value as u32, entries: Vec::new() }};
{slots}
            data.entries.sort_by_key(|entry| entry.roll_threshold);
            buff_buckets_by_id.insert(id, buff_buckets.len());
            buff_buckets.push(data);
        }}
"#,
            rows = parts.row_field,
        ),
        methods: r#"    pub fn buff_bucket_data_from_id(&self, id: Crc32) -> Option<&StaticBuffBucketData> {
        self.buff_buckets.get(*self.buff_buckets_by_id.get(&id)?)
    }

    pub fn buff_bucket_data(&self, key: &str) -> Option<&StaticBuffBucketData> {
        self.buff_bucket_data_from_id(Crc32::from_str_lower(key))
    }

    pub fn visit_all_buffs_from_id(&self, id: Crc32) -> Vec<&BuffBucketEntry> {
        fn visit<'a>(manager: &'a BuffBucketDataManager, id: Crc32, active: &mut HashSet<Crc32>, output: &mut Vec<&'a BuffBucketEntry>) {
            if !active.insert(id) { return; }
            if let Some(bucket) = manager.buff_bucket_data_from_id(id) {
                for entry in &bucket.entries {
                    if entry.kind == BuffBucketEntryKind::BuffBucket { visit(manager, entry.buff_id, active, output); }
                    else { output.push(entry); }
                }
            }
            active.remove(&id);
        }
        let mut output = Vec::new();
        visit(self, id, &mut HashSet::new(), &mut output);
        output
    }

    pub fn visit_all_buffs(&self, key: &str) -> Vec<&BuffBucketEntry> {
        self.visit_all_buffs_from_id(Crc32::from_str_lower(key))
    }

    pub fn buff_buckets(&self) -> std::slice::Iter<'_, StaticBuffBucketData> { self.buff_buckets.iter() }

"#.to_owned(),
        rows_type: "StaticBuffBucketData".to_owned(),
        rows_method: "buff_buckets".to_owned(),
    })
}

fn experience(context: &AugmentationContext<'_>) -> Result<RustNativeManagerAugmentation> {
    let row = context.row("ExperienceData")?;
    let parts = context.row_parts(row);
    let level = number(context, row, "Level Number", "source.row", "0.0")?;
    let gear_score = optional_number(row, "MaxEquippableGearScore", "source.row", "0.0")?;
    let xp = optional_number(row, "XPToLevel", "source.row", "0.0")?;
    Ok(RustNativeManagerAugmentation {
        declarations: r#"fn experience_u32(value: f32) -> Option<u32> {
    (value.is_finite()
        && value >= 0.0
        && value.fract() == 0.0
        && f64::from(value) <= f64::from(u32::MAX))
        .then_some(value as u32)
}

"#
        .to_owned(),
        fields: format!(
            "    experience_by_level: HashMap<u32, RowRef<{}, {}>>,\n    gear_score_thresholds: Vec<(u32, u32)>,\n    xp_thresholds: Vec<(u32, u32)>,\n    maximum_level: Option<u32>,\n",
            parts.table_type, parts.row_type
        ),
        field_values: "            experience_by_level,\n            gear_score_thresholds,\n            xp_thresholds,\n            maximum_level,\n".to_owned(),
        initializers: format!(
            r#"        let mut experience_by_level = HashMap::new();
        let mut gear_score_thresholds = Vec::new();
        let mut xp_thresholds = Vec::new();
        let mut maximum_level = None;
        for source in {rows}.rows() {{
            let Some(level) = experience_u32({level}) else {{ continue; }};
            if experience_by_level.contains_key(&level) {{ continue; }}
            experience_by_level.insert(level, source.reference.clone());
            maximum_level = Some(maximum_level.map_or(level, |current: u32| current.max(level)));
            if let Some(threshold) = experience_u32({gear_score}).filter(|value| *value != 0) {{
                gear_score_thresholds.push((threshold, level));
            }}
            xp_thresholds.push((experience_u32({xp}).unwrap_or_default(), level));
        }}
        gear_score_thresholds.sort_unstable();
        xp_thresholds.sort_unstable();
"#,
            rows = parts.row_field,
        ),
        methods: format!(
            r#"    pub fn experience_data_from_id(&self, level: u32) -> Option<&{schema}> {{
        self.{rows}.get(self.experience_by_level.get(&level)?)
    }}

    pub fn experience_data(&self, level: u32) -> Option<&{schema}> {{ self.experience_data_from_id(level) }}

    pub fn experience_data_for_max_equippable_gear_score(&self, gear_score: u32) -> Option<&{schema}> {{
        let index = self.gear_score_thresholds.partition_point(|(threshold, _)| *threshold < gear_score);
        let (_, level) = self.gear_score_thresholds.get(index)?;
        self.experience_data_from_id(*level)
    }}

    pub fn level_for_xp(&self, xp: u64) -> u32 {{
        self.xp_thresholds.iter().take_while(|(threshold, _)| u64::from(*threshold) <= xp).map(|(_, level)| *level).max().unwrap_or_default()
    }}

    pub fn max_level(&self) -> Option<u32> {{ self.maximum_level }}
    pub fn len(&self) -> usize {{ self.experience_by_level.len() }}
    pub fn is_empty(&self) -> bool {{ self.experience_by_level.is_empty() }}

"#,
            schema = parts.row_type,
            rows = parts.row_field,
        ),
        ..RustNativeManagerAugmentation::default()
    })
}

fn damage(context: &AugmentationContext<'_>) -> Result<RustNativeManagerAugmentation> {
    let damage = context.row("DamageData")?;
    let damage_type = context.row("DamageTypeData")?;
    let affliction = context.row("AfflictionData")?;
    let damage_parts = context.row_parts(damage);
    let type_parts = context.row_parts(damage_type);
    let affliction_parts = context.row_parts(affliction);
    let damage_id = text(context, damage, "DamageID", "source.row")?;
    let weapon_category = optional_text(damage, "WeaponCategory", "source.row")?;
    let type_id = text(context, damage_type, "TypeID", "source.row")?;
    let type_numeric = number(context, damage_type, "IntID", "source.row", "0.0")?;
    let affliction_id = text(context, affliction, "AfflictionID", "source.row")?;
    let affliction_numeric = number(context, affliction, "IntID", "source.row", "0.0")?;
    Ok(RustNativeManagerAugmentation {
        declarations: format!(
            r#"#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct DamageDataReference {{ pub table: {damage_table}, pub id: Crc32 }}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct DamageDataSlot {{ pub table: {damage_table}, pub row_index: usize }}

#[derive(Debug, Clone, PartialEq)]
pub struct StaticDamageData {{
    pub reference: DamageDataReference,
    pub slot: DamageDataSlot,
    pub key: String,
    pub id: Crc32,
    pub weapon_category: String,
    pub weapon_category_id: Crc32,
    pub source: RowRef<{damage_table}, {damage_schema}>,
}}

#[derive(Debug, Clone, PartialEq)]
pub struct StaticDamageTypeData {{
    pub key: String,
    pub id: Crc32,
    pub numeric_id: u8,
    pub source: RowRef<{type_table}, {type_schema}>,
}}

#[derive(Debug, Clone, PartialEq)]
pub struct StaticAfflictionData {{
    pub key: String,
    pub id: Crc32,
    pub numeric_id: u8,
    pub source: RowRef<{affliction_table}, {affliction_schema}>,
}}

fn damage_u8(value: f32, allow_max: bool) -> Option<u8> {{
    (value.is_finite() && value > 0.0 && value.fract() == 0.0 && value <= if allow_max {{ u8::MAX as f32 }} else {{ (u8::MAX - 1) as f32 }})
        .then_some(value as u8)
}}

"#,
            damage_table = damage_parts.table_type,
            damage_schema = damage_parts.row_type,
            type_table = type_parts.table_type,
            type_schema = type_parts.row_type,
            affliction_table = affliction_parts.table_type,
            affliction_schema = affliction_parts.row_type,
        ),
        fields: "    damage_entries: Vec<StaticDamageData>,\n    damage_by_reference: HashMap<DamageDataReference, usize>,\n    damage_by_slot: HashMap<DamageDataSlot, usize>,\n    damage_type_entries: Vec<StaticDamageTypeData>,\n    damage_types_by_id: HashMap<Crc32, usize>,\n    affliction_entries: Vec<StaticAfflictionData>,\n    afflictions_by_id: HashMap<Crc32, usize>,\n    weapon_categories: Vec<String>,\n".to_owned(),
        field_values: "            damage_entries,\n            damage_by_reference,\n            damage_by_slot,\n            damage_type_entries,\n            damage_types_by_id,\n            affliction_entries,\n            afflictions_by_id,\n            weapon_categories,\n".to_owned(),
        initializers: format!(
            r#"        let mut damage_entries = Vec::new();
        let mut damage_by_reference = HashMap::new();
        let mut damage_by_slot = HashMap::new();
        let mut weapon_categories = Vec::new();
        let mut weapon_category_ids = HashSet::new();
        for source in {damage_rows}.rows() {{
            let key = {damage_id}.trim();
            let id = Crc32::from_str_lower(key);
            if key.is_empty() || id == Crc32::ZERO || source.slot.row_index() >= u16::MAX as usize {{ continue; }}
            let reference = DamageDataReference {{ table: *source.reference.table(), id }};
            let slot = DamageDataSlot {{ table: *source.slot.table(), row_index: source.slot.row_index() }};
            if damage_by_reference.contains_key(&reference) || damage_by_slot.contains_key(&slot) {{ continue; }}
            let candidate = {weapon_category}.trim();
            let category = if candidate.is_empty() || candidate.eq_ignore_ascii_case("none") {{ "Default" }} else {{ candidate }};
            let category_id = Crc32::from_str_lower(category);
            let index = damage_entries.len();
            damage_by_reference.insert(reference, index);
            damage_by_slot.insert(slot, index);
            if category_id != Crc32::ZERO && weapon_category_ids.insert(category_id) {{ weapon_categories.push(category.to_owned()); }}
            damage_entries.push(StaticDamageData {{ reference, slot, key: key.to_owned(), id, weapon_category: category.to_owned(), weapon_category_id: category_id, source: source.reference.clone() }});
        }}
        let mut damage_type_entries = Vec::new();
        let mut damage_types_by_id = HashMap::new();
        for source in {type_rows}.rows() {{
            let key = {type_id}.trim();
            let id = Crc32::from_str_lower(key);
            let Some(numeric_id) = damage_u8({type_numeric}, true) else {{ continue; }};
            if key.is_empty() || id == Crc32::ZERO || damage_types_by_id.contains_key(&id) {{ continue; }}
            damage_types_by_id.insert(id, damage_type_entries.len());
            damage_type_entries.push(StaticDamageTypeData {{ key: key.to_owned(), id, numeric_id, source: source.reference.clone() }});
        }}
        let mut affliction_entries = Vec::new();
        let mut afflictions_by_id = HashMap::new();
        for source in {affliction_rows}.rows() {{
            let key = {affliction_id}.trim();
            let id = Crc32::from_str_lower(key);
            let Some(numeric_id) = damage_u8({affliction_numeric}, false) else {{ continue; }};
            if key.is_empty() || id == Crc32::ZERO || afflictions_by_id.contains_key(&id) {{ continue; }}
            afflictions_by_id.insert(id, affliction_entries.len());
            affliction_entries.push(StaticAfflictionData {{ key: key.to_owned(), id, numeric_id, source: source.reference.clone() }});
        }}
"#,
            damage_rows = damage_parts.row_field,
            type_rows = type_parts.row_field,
            affliction_rows = affliction_parts.row_field,
        ),
        methods: format!(
            r#"    pub fn damage(&self, reference: DamageDataReference) -> Option<&StaticDamageData> {{ self.damage_entries.get(*self.damage_by_reference.get(&reference)?) }}
    pub fn damage_by_slot(&self, slot: DamageDataSlot) -> Option<&StaticDamageData> {{ self.damage_entries.get(*self.damage_by_slot.get(&slot)?) }}
    pub fn damage_by_id(&self, table: {damage_table}, id: Crc32) -> Option<&StaticDamageData> {{ self.damage(DamageDataReference {{ table, id }}) }}
    pub fn damage_by_key(&self, table: {damage_table}, key: &str) -> Option<&StaticDamageData> {{ self.damage_by_id(table, Crc32::from_str_lower(key)) }}
    pub fn resolve(&self, reference: TableReference<'_>) -> Option<&StaticDamageData> {{
        self.damage_by_key({damage_table}::from_path(reference.path())?, reference.key())
    }}
    pub fn damage_ref_by_slot(&self, slot: DamageDataSlot) -> Option<DamageDataReference> {{ Some(self.damage_by_slot(slot)?.reference) }}
    pub fn damage_key_by_slot(&self, slot: DamageDataSlot) -> Option<&str> {{ Some(&self.damage_by_slot(slot)?.key) }}
    pub fn damage_type(&self, id: Crc32) -> Option<&StaticDamageTypeData> {{ self.damage_type_entries.get(*self.damage_types_by_id.get(&id)?) }}
    pub fn damage_type_by_key(&self, key: &str) -> Option<&StaticDamageTypeData> {{ self.damage_type(Crc32::from_str_lower(key)) }}
    pub fn affliction(&self, id: Crc32) -> Option<&StaticAfflictionData> {{ self.affliction_entries.get(*self.afflictions_by_id.get(&id)?) }}
    pub fn affliction_by_key(&self, key: &str) -> Option<&StaticAfflictionData> {{ self.affliction(Crc32::from_str_lower(key)) }}
    pub fn damage_rows(&self) -> std::slice::Iter<'_, StaticDamageData> {{ self.damage_entries.iter() }}
    pub fn damage_types(&self) -> std::slice::Iter<'_, StaticDamageTypeData> {{ self.damage_type_entries.iter() }}
    pub fn damage_type_ids(&self) -> impl Iterator<Item = Crc32> + '_ {{ self.damage_type_entries.iter().map(|value| value.id) }}
    pub fn afflictions(&self) -> std::slice::Iter<'_, StaticAfflictionData> {{ self.affliction_entries.iter() }}
    pub fn weapon_categories(&self) -> impl Iterator<Item = &str> {{ self.weapon_categories.iter().map(String::as_str) }}
    pub fn len(&self) -> usize {{ self.damage_entries.len() }}
    pub fn is_empty(&self) -> bool {{ self.damage_entries.is_empty() }}

"#,
            damage_table = damage_parts.table_type,
        ),
        rows_type: "StaticDamageData".to_owned(),
        rows_method: "damage_rows".to_owned(),
    })
}

fn vitals(context: &AugmentationContext<'_>) -> Result<RustNativeManagerAugmentation> {
    let row = context.row("VitalsLevelVariantData")?;
    let parts = context.row_parts(row);
    let id = text(context, row, "VitalsID", "source.row")?;
    let base = optional_text(row, "BaseVitalsID", "source.row")?;
    Ok(RustNativeManagerAugmentation {
        declarations: format!(
            "#[derive(Debug, Clone, PartialEq)]\npub struct StaticVitalsLevelVariantData {{\n    pub key: String,\n    pub id: Crc32,\n    pub base_vitals_key: String,\n    pub base_vitals_id: Crc32,\n    pub source: RowRef<{}, {}>,\n}}\n\n",
            parts.table_type, parts.row_type
        ),
        fields: "    vitals_entries: Vec<StaticVitalsLevelVariantData>,\n    vitals_by_id: HashMap<Crc32, usize>,\n    creature_type_ids: Vec<Crc32>,\n".to_owned(),
        field_values: "            vitals_entries,\n            vitals_by_id,\n            creature_type_ids,\n".to_owned(),
        initializers: format!(
            r#"        let mut vitals_entries = Vec::new();
        let mut vitals_by_id = HashMap::new();
        let mut creature_type_ids = Vec::new();
        let mut creature_type_id_set = HashSet::new();
        for source in {rows}.rows() {{
            let key = {id}.trim();
            let id = Crc32::from_str_lower(key);
            if key.is_empty() || id == Crc32::ZERO || vitals_by_id.contains_key(&id) {{ continue; }}
            let base_vitals_key = {base}.trim();
            let base_vitals_id = Crc32::from_str_lower(base_vitals_key);
            if let Some(base) = _vitals_base_data.vitals_base_data_from_id(base_vitals_id) {{
                if base.creature_type_crc != Crc32::ZERO && creature_type_id_set.insert(base.creature_type_crc) {{ creature_type_ids.push(base.creature_type_crc); }}
            }}
            vitals_by_id.insert(id, vitals_entries.len());
            vitals_entries.push(StaticVitalsLevelVariantData {{ key: key.to_owned(), id, base_vitals_key: base_vitals_key.to_owned(), base_vitals_id, source: source.reference.clone() }});
        }}
"#,
            rows = parts.row_field,
        ),
        methods: r#"    pub fn get_by_id(&self, id: Crc32) -> Option<&StaticVitalsLevelVariantData> { self.vitals_entries.get(*self.vitals_by_id.get(&id)?) }
    pub fn by_key(&self, key: &str) -> Option<&StaticVitalsLevelVariantData> { self.get_by_id(Crc32::from_str_lower(key)) }
    pub fn vitals(&self) -> std::slice::Iter<'_, StaticVitalsLevelVariantData> { self.vitals_entries.iter() }
    pub fn creature_type_ids(&self) -> impl Iterator<Item = Crc32> + '_ { self.creature_type_ids.iter().copied() }
    pub fn len(&self) -> usize { self.vitals_entries.len() }
    pub fn is_empty(&self) -> bool { self.vitals_entries.is_empty() }

"#.to_owned(),
        rows_type: "StaticVitalsLevelVariantData".to_owned(),
        rows_method: "vitals".to_owned(),
    })
}

fn elemental_mutations(context: &AugmentationContext<'_>) -> Result<RustNativeManagerAugmentation> {
    let row = context.row("ElementalMutationStaticData")?;
    let parts = context.row_parts(row);
    let id = text(context, row, "ElementalMutationID", "source.row")?;
    let mut bucket_columns = Vec::new();
    let bucket_candidates: &[(&str, &[&str])] = &[
        ("Dungeon-", &["Dungeon1", "Dungeon"]),
        ("Dungeon", &["Dungeon2"]),
        ("Dungeon+", &["Dungeon3"]),
        ("DungeonMiniBoss", &["DungeonMiniBoss"]),
        ("DungeonBoss", &["DungeonBoss"]),
    ];
    for &(creature, candidates) in bucket_candidates {
        if let Some(column) = candidates.iter().find_map(|name| field(row, name)) {
            bucket_columns.push((creature, string_value_expression(column, "source.row")?));
        }
    }
    let bucket_builders = bucket_columns
        .into_iter()
        .map(|(creature, expression)| {
            format!(
                r#"            {{
                let bucket_key = {expression}.trim();
                let bucket_id = Crc32::from_str_lower(bucket_key);
                if bucket_id != Crc32::ZERO {{
                    data.buckets.push(ElementalMutationBucket {{ creature_type_id: Crc32::from_str_lower({creature:?}), bucket_key: bucket_key.to_owned(), bucket_id }});
                    for entry in _buff_bucket_data.visit_all_buffs_from_id(bucket_id) {{
                        if entry.kind != BuffBucketEntryKind::StatusEffect {{ continue; }}
                        let Some(status) = _status_effect_data.status_effect_data_from_id(entry.buff_id) else {{ continue; }};
                        let Some(priority) = mutation_status_priority(status.ui_priority.as_deref()) else {{ continue; }};
                        let Some((group, tier)) = mutation_status_group_and_tier(&entry.buff_key) else {{ continue; }};
                        match selected.get(&group) {{
                            Some((current, _, _)) if *current >= tier => {{}},
                            _ => {{ selected.insert(group, (tier, priority, entry.buff_id)); }},
                        }}
                    }}
                }}
            }}
"#
            )
        })
        .collect::<String>();
    Ok(RustNativeManagerAugmentation {
        declarations: format!(
            r#"#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ElementalMutationBucket {{
    pub creature_type_id: Crc32,
    pub bucket_key: String,
    pub bucket_id: Crc32,
}}

#[derive(Debug, Clone, PartialEq)]
pub struct StaticElementalMutationData {{
    pub source: RowRef<{table}, {schema}>,
    pub key: String,
    pub id: Crc32,
    pub buckets: Vec<ElementalMutationBucket>,
    pub possible_status_effect_ids: Vec<Crc32>,
}}

{mutation_helpers}
"#,
            table = parts.table_type,
            schema = parts.row_type,
            mutation_helpers = mutation_helpers(),
        ),
        fields: "    elemental_mutations: Vec<StaticElementalMutationData>,\n    elemental_mutations_by_id: HashMap<Crc32, usize>,\n".to_owned(),
        field_values: "            elemental_mutations,\n            elemental_mutations_by_id,\n".to_owned(),
        initializers: format!(
            r#"        let mut elemental_mutations = Vec::new();
        let mut elemental_mutations_by_id = HashMap::new();
        for source in {rows}.rows() {{
            let key = {id}.trim();
            let id = Crc32::from_str_lower(key);
            if key.is_empty() || id == Crc32::ZERO || elemental_mutations_by_id.contains_key(&id) {{ continue; }}
            let mut data = StaticElementalMutationData {{ source: source.reference.clone(), key: key.to_owned(), id, buckets: Vec::new(), possible_status_effect_ids: Vec::new() }};
            let mut selected = HashMap::<String, (u32, i32, Crc32)>::new();
{bucket_builders}            let mut selected = selected.into_values().collect::<Vec<_>>();
            selected.sort_unstable_by(|left, right| right.1.cmp(&left.1).then_with(|| right.0.cmp(&left.0)).then_with(|| left.2.value().cmp(&right.2.value())));
            data.possible_status_effect_ids.extend(selected.into_iter().map(|(_, _, id)| id));
            elemental_mutations_by_id.insert(id, elemental_mutations.len());
            elemental_mutations.push(data);
        }}
"#,
            rows = parts.row_field,
        ),
        methods: r#"    pub fn elemental_mutation_static_data_from_id(&self, id: Crc32) -> Option<&StaticElementalMutationData> { self.elemental_mutations.get(*self.elemental_mutations_by_id.get(&id)?) }
    pub fn elemental_mutation_static_data(&self, key: &str) -> Option<&StaticElementalMutationData> { self.elemental_mutation_static_data_from_id(Crc32::from_str_lower(key)) }
    pub fn possible_elemental_status_effects(&self, id: Crc32) -> impl Iterator<Item = Crc32> + '_ { self.elemental_mutation_static_data_from_id(id).into_iter().flat_map(|value| value.possible_status_effect_ids.iter().copied()) }
    pub fn possible_elemental_status_effects_by_key(&self, key: &str) -> impl Iterator<Item = Crc32> + '_ { self.possible_elemental_status_effects(Crc32::from_str_lower(key)) }
    pub fn elemental_mutations(&self) -> std::slice::Iter<'_, StaticElementalMutationData> { self.elemental_mutations.iter() }

"#.to_owned(),
        rows_type: "StaticElementalMutationData".to_owned(),
        rows_method: "elemental_mutations".to_owned(),
    })
}

fn promotion_mutations(context: &AugmentationContext<'_>) -> Result<RustNativeManagerAugmentation> {
    let row = context.row("PromotionMutationStaticData")?;
    let parts = context.row_parts(row);
    let id = text(context, row, "PromotionMutationID", "source.row")?;
    let promotions = (1..=3u8)
        .filter_map(|slot| {
            field(row, &format!("Promotion{slot}"))
                .map(|column| (slot, string_value_expression(column, "source.row")))
        })
        .map(|(slot, expression)| {
            let expression = expression?;
            Ok(format!(
                "            {{ let key = {expression}.trim(); let id = Crc32::from_str_lower(key); if id != Crc32::ZERO {{ data.promotions.push(PromotionStatusEffect {{ slot: {slot}, slot_tag_id: Crc32::from_str_lower(\"Promotion{slot}\"), status_effect_key: key.to_owned(), status_effect_id: id }}); }} }}\n"
            ))
        })
        .collect::<Result<String>>()?;
    Ok(RustNativeManagerAugmentation {
        declarations: format!(
            r#"#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PromotionStatusEffect {{ pub slot: u8, pub slot_tag_id: Crc32, pub status_effect_key: String, pub status_effect_id: Crc32 }}

#[derive(Debug, Clone, PartialEq)]
pub struct StaticPromotionMutationData {{
    pub source: RowRef<{table}, {schema}>,
    pub key: String,
    pub id: Crc32,
    pub promotions: Vec<PromotionStatusEffect>,
    pub possible_status_effect_ids_by_element: HashMap<Crc32, Vec<Crc32>>,
}}

{mutation_helpers}
"#,
            table = parts.table_type,
            schema = parts.row_type,
            mutation_helpers = mutation_helpers(),
        ),
        fields: "    promotion_mutations: Vec<StaticPromotionMutationData>,\n    promotion_mutations_by_id: HashMap<Crc32, usize>,\n".to_owned(),
        field_values: "            promotion_mutations,\n            promotion_mutations_by_id,\n".to_owned(),
        initializers: format!(
            r#"        let mut promotion_mutations = Vec::new();
        let mut promotion_mutations_by_id = HashMap::new();
        for source in {rows}.rows() {{
            let key = {id}.trim();
            let id = Crc32::from_str_lower(key);
            if key.is_empty() || id == Crc32::ZERO || promotion_mutations_by_id.contains_key(&id) {{ continue; }}
            let mut data = StaticPromotionMutationData {{ source: source.reference.clone(), key: key.to_owned(), id, promotions: Vec::new(), possible_status_effect_ids_by_element: HashMap::new() }};
{promotions}            let promotions_by_slot = data.promotions.iter().map(|value| (value.slot_tag_id, value)).collect::<HashMap<_, _>>();
            for elemental in _elemental_mutation_static_data.elemental_mutations() {{
                let mut selected = HashMap::<String, (u32, i32, Crc32)>::new();
                for bucket in &elemental.buckets {{
                    for entry in _buff_bucket_data.visit_all_buffs_from_id(bucket.bucket_id) {{
                        if entry.kind != BuffBucketEntryKind::Promotion {{ continue; }}
                        let Some(promotion) = promotions_by_slot.get(&entry.buff_id) else {{ continue; }};
                        let Some(status) = _status_effect_data.status_effect_data_from_id(promotion.status_effect_id) else {{ continue; }};
                        let Some(priority) = mutation_status_priority(status.ui_priority.as_deref()) else {{ continue; }};
                        let Some((group, tier)) = mutation_status_group_and_tier(&promotion.status_effect_key) else {{ continue; }};
                        match selected.get(&group) {{ Some((current, _, _)) if *current >= tier => {{}}, _ => {{ selected.insert(group, (tier, priority, promotion.status_effect_id)); }} }}
                    }}
                }}
                let mut values = selected.into_values().collect::<Vec<_>>();
                values.sort_unstable_by(|left, right| right.1.cmp(&left.1).then_with(|| right.0.cmp(&left.0)).then_with(|| left.2.value().cmp(&right.2.value())));
                data.possible_status_effect_ids_by_element.insert(elemental.id, values.into_iter().map(|(_, _, id)| id).collect());
            }}
            promotion_mutations_by_id.insert(id, promotion_mutations.len());
            promotion_mutations.push(data);
        }}
"#,
            rows = parts.row_field,
        ),
        methods: r#"    pub fn promotion_mutation_static_data_from_id(&self, id: Crc32) -> Option<&StaticPromotionMutationData> { self.promotion_mutations.get(*self.promotion_mutations_by_id.get(&id)?) }
    pub fn promotion_mutation_static_data(&self, key: &str) -> Option<&StaticPromotionMutationData> { self.promotion_mutation_static_data_from_id(Crc32::from_str_lower(key)) }
    pub fn possible_promotional_status_effects_for_element(&self, promotion_id: Crc32, elemental_id: Crc32) -> impl Iterator<Item = Crc32> + '_ { self.promotion_mutation_static_data_from_id(promotion_id).and_then(|value| value.possible_status_effect_ids_by_element.get(&elemental_id)).into_iter().flatten().copied() }
    pub fn possible_promotional_status_effects_for_element_by_key(&self, promotion: &str, elemental: &str) -> impl Iterator<Item = Crc32> + '_ { self.possible_promotional_status_effects_for_element(Crc32::from_str_lower(promotion), Crc32::from_str_lower(elemental)) }
    pub fn promotion_mutations(&self) -> std::slice::Iter<'_, StaticPromotionMutationData> { self.promotion_mutations.iter() }

"#.to_owned(),
        rows_type: "StaticPromotionMutationData".to_owned(),
        rows_method: "promotion_mutations".to_owned(),
    })
}

fn musical_rewards(context: &AugmentationContext<'_>) -> Result<RustNativeManagerAugmentation> {
    let row = context.row("MusicalPerformanceRewards")?;
    let parts = context.row_parts(row);
    let mut value = context.simple_crc(
        "MusicalPerformanceRewards",
        "RewardID",
        &["reward_from_id"],
        &["reward"],
        "musical_rewards",
    )?;
    let events = [
        "GameEventId_Rank_Amazing",
        "GameEventId_Rank_Great",
        "GameEventId_Rank_Okay",
        "GameEventId_Rank_Bad",
    ]
    .into_iter()
    .filter_map(|name| field(row, name))
    .map(|column| string_value_expression(column, "source.row"))
    .collect::<Result<Vec<_>>>()?;
    let event_insertions = events
        .into_iter()
        .map(|expression| format!("            {{ let id = Crc32::from_str_lower({expression}.trim()); if id != Crc32::ZERO {{ musical_rewards_by_game_event.entry(id).or_insert_with(|| source.reference.clone()); }} }}\n"))
        .collect::<String>();
    value.fields.push_str(&format!(
        "    musical_rewards_by_game_event: HashMap<Crc32, RowRef<{}, {}>>,\n",
        parts.table_type, parts.row_type
    ));
    value
        .field_values
        .push_str("            musical_rewards_by_game_event,\n");
    value.initializers.push_str(&format!("        let mut musical_rewards_by_game_event = HashMap::new();\n        for source in {}.rows() {{\n{}        }}\n", parts.row_field, event_insertions));
    value.methods.push_str(&format!(
        "    pub fn reward_for_game_event(&self, id: Crc32) -> Option<&{}> {{ self.{}.get(self.musical_rewards_by_game_event.get(&id)?) }}\n    pub fn reward_for_game_event_key(&self, key: &str) -> Option<&{}> {{ self.reward_for_game_event(Crc32::from_str_lower(key)) }}\n\n",
        parts.row_type, parts.row_field, parts.row_type
    ));
    Ok(value)
}

fn combat_profiles(context: &AugmentationContext<'_>) -> Result<RustNativeManagerAugmentation> {
    let row = context.row("CombatProfilesData")?;
    let parts = context.row_parts(row);
    let profile_type = text(context, row, "ProfileType", "source.row")?;
    let weapon = text(context, row, "WeaponName", "source.row")?;
    let mut value = context.simple_crc(
        "CombatProfilesData",
        "WeaponName",
        &["profile"],
        &["profile_by_key"],
        "combat_profiles",
    )?;
    let reference_type = format!("RowRef<{}, {}>", parts.table_type, parts.row_type);
    value.fields.push_str(&format!(
        "    combat_unarmed: Option<{reference_type}>,\n    combat_heartgem: Option<{reference_type}>,\n    combat_siege: HashMap<String, {reference_type}>,\n    combat_active_ability: HashMap<Crc32, {reference_type}>,\n    combat_ability_aim_pose: HashMap<Crc32, {reference_type}>,\n    combat_item_class: HashMap<String, {reference_type}>,\n"
    ));
    value.field_values.push_str("            combat_unarmed,\n            combat_heartgem,\n            combat_siege,\n            combat_active_ability,\n            combat_ability_aim_pose,\n            combat_item_class,\n");
    value.initializers.push_str(&format!(
        r#"        let mut combat_unarmed = None;
        let mut combat_heartgem = None;
        let mut combat_siege = HashMap::new();
        let mut combat_active_ability = HashMap::new();
        let mut combat_ability_aim_pose = HashMap::new();
        let mut combat_item_class = HashMap::new();
        for source in {rows}.rows() {{
            let profile_type = normalize_lookup_key({profile_type});
            let weapon = {weapon}.trim();
            let id = Crc32::from_str_lower(weapon);
            match profile_type.as_str() {{
                "weapon" => {{ let class = normalize_lookup_key(weapon); combat_item_class.entry(class.clone()).or_insert_with(|| source.reference.clone()); if class == "heartgem" && combat_heartgem.is_none() {{ combat_heartgem = Some(source.reference.clone()); }} }},
                "unarmed" if combat_unarmed.is_none() => combat_unarmed = Some(source.reference.clone()),
                "siegeweapon" => {{ combat_siege.entry(normalize_lookup_key(weapon)).or_insert_with(|| source.reference.clone()); }},
                "activeability" => {{ combat_active_ability.entry(id).or_insert_with(|| source.reference.clone()); }},
                "abilityaimpose" => {{ combat_ability_aim_pose.entry(id).or_insert_with(|| source.reference.clone()); }},
                _ => {{}},
            }}
        }}
"#,
            rows = parts.row_field,
        ));
    value.methods.push_str(
        &context
            .named_rows("CombatProfilesData", "profiles")?
            .methods,
    );
    value.methods.push_str(&format!(
        r#"    pub fn heartgem_profile(&self) -> Option<&{schema}> {{ self.{rows}.get(self.combat_heartgem.as_ref()?) }}
    pub fn unarmed_profile(&self) -> Option<&{schema}> {{ self.{rows}.get(self.combat_unarmed.as_ref()?) }}
    pub fn siege_weapon_profile(&self, kind: &str) -> Option<&{schema}> {{ self.{rows}.get(self.combat_siege.get(&normalize_lookup_key(kind))?) }}
    pub fn active_ability_profile(&self, id: Crc32) -> Option<&{schema}> {{ self.{rows}.get(self.combat_active_ability.get(&id)?) }}
    pub fn active_ability_profile_by_key(&self, key: &str) -> Option<&{schema}> {{ self.active_ability_profile(Crc32::from_str_lower(key)) }}
    pub fn ability_aim_pose_profile(&self, id: Crc32) -> Option<&{schema}> {{ self.{rows}.get(self.combat_ability_aim_pose.get(&id)?) }}
    pub fn ability_aim_pose_profile_by_key(&self, key: &str) -> Option<&{schema}> {{ self.ability_aim_pose_profile(Crc32::from_str_lower(key)) }}
    pub fn item_class_profile(&self, class: &str) -> Option<&{schema}> {{ self.{rows}.get(self.combat_item_class.get(&normalize_lookup_key(class))?) }}

"#,
        schema = parts.row_type,
        rows = parts.row_field,
    ));
    Ok(value)
}

fn gatherable(context: &AugmentationContext<'_>) -> Result<RustNativeManagerAugmentation> {
    require_products(context, &["GatheringDatabase", "GatheringActionDatabase"])?;
    let row = context.find_first_row(&["GatherableData", "GatherableDataRow"])?;
    let parts = context.row_parts(row);
    let id = text(context, row, "GatherableID", "source.row")?;
    let action = optional_text(row, "GatheringAction", "source.row")?;
    let kind = optional_text(row, "GatheringType", "source.row")?;
    Ok(RustNativeManagerAugmentation {
        declarations: format!(
            r#"#[derive(Debug, Clone, PartialEq)]
pub struct StaticGatherableData {{
    pub source: RowRef<{table}, {schema}>,
    pub key: String,
    pub id: Crc32,
    pub gathering_action_key: String,
    pub gathering_action_id: Crc32,
    pub gathering_type_key: String,
    pub gathering_type_id: Crc32,
}}

"#,
            table = parts.table_type,
            schema = parts.row_type,
        ),
        fields: format!(
            "    gatherable_entries: Vec<StaticGatherableData>,\n    gatherables_by_id: HashMap<Crc32, usize>,\n    gatherables_by_table: HashMap<{}, Vec<usize>>,\n    gathering_types_by_id: HashMap<Crc32, usize>,\n    gathering_actions_by_id: HashMap<Crc32, usize>,\n",
            parts.table_type
        ),
        field_values: "            gatherable_entries,\n            gatherables_by_id,\n            gatherables_by_table,\n            gathering_types_by_id,\n            gathering_actions_by_id,\n".to_owned(),
        initializers: format!(
            r#"        let mut gatherable_entries = Vec::new();
        let mut gatherables_by_id = HashMap::new();
        let mut gatherables_by_table = HashMap::<{table}, Vec<usize>>::new();
        for source in {rows}.rows() {{
            let key = {id}.trim();
            let id = Crc32::from_str_lower(key);
            if key.is_empty() || id == Crc32::ZERO || gatherables_by_id.contains_key(&id) {{ continue; }}
            let action_key = {action}.trim();
            let type_key = {kind}.trim();
            let index = gatherable_entries.len();
            gatherables_by_id.insert(id, index);
            gatherables_by_table.entry(*source.reference.table()).or_default().push(index);
            gatherable_entries.push(StaticGatherableData {{ source: source.reference.clone(), key: key.to_owned(), id, gathering_action_key: action_key.to_owned(), gathering_action_id: Crc32::from_str_lower(action_key), gathering_type_key: type_key.to_owned(), gathering_type_id: Crc32::from_str_lower(type_key) }});
        }}
        let mut gathering_types_by_id = HashMap::new();
        for (index, value) in gathering_database.gathering_data.gathering_types.iter().enumerate() {{
            let id = Crc32::from_str_lower(&value.gathering_type);
            if id != Crc32::ZERO {{ gathering_types_by_id.entry(id).or_insert(index); }}
        }}
        let mut gathering_actions_by_id = HashMap::new();
        for (index, value) in gathering_action_database.gathering_actions.iter().enumerate() {{
            let id = Crc32::from_str_lower(&value.name);
            if id != Crc32::ZERO {{ gathering_actions_by_id.entry(id).or_insert(index); }}
        }}
"#,
            table = parts.table_type,
            rows = parts.row_field,
        ),
        methods: format!(
            r#"    pub fn gatherable_data(&self, id: Crc32) -> Option<&StaticGatherableData> {{ self.gatherable_entries.get(*self.gatherables_by_id.get(&id)?) }}
    pub fn gatherable_data_from_id(&self, key: &str) -> Option<&StaticGatherableData> {{ self.gatherable_data(Crc32::from_str_lower(key)) }}
    pub fn gatherables(&self, table: {table}) -> impl Iterator<Item = &StaticGatherableData> {{ self.gatherables_by_table.get(&table).into_iter().flatten().filter_map(|index| self.gatherable_entries.get(*index)) }}
    pub fn gatherable_rows(&self) -> std::slice::Iter<'_, StaticGatherableData> {{ self.gatherable_entries.iter() }}
    pub fn gathering_type(&self, id: Crc32) -> Option<&GatheringTypeData> {{ self.gathering_database.gathering_data.gathering_types.get(*self.gathering_types_by_id.get(&id)?) }}
    pub fn gathering_type_by_key(&self, key: &str) -> Option<&GatheringTypeData> {{ self.gathering_type(Crc32::from_str_lower(key)) }}
    pub fn gathering_action(&self, id: Crc32) -> Option<&GatheringActionData> {{ self.gathering_action_database.gathering_actions.get(*self.gathering_actions_by_id.get(&id)?) }}
    pub fn gathering_action_by_key(&self, key: &str) -> Option<&GatheringActionData> {{ self.gathering_action(Crc32::from_str_lower(key)) }}

"#,
            table = parts.table_type,
        ),
        rows_type: "StaticGatherableData".to_owned(),
        rows_method: "gatherable_rows".to_owned(),
    })
}

fn social(context: &AugmentationContext<'_>) -> Result<RustNativeManagerAugmentation> {
    require_products(context, &["SocialRankDatabase"])?;
    Ok(RustNativeManagerAugmentation {
        fields: "    social_ranks_by_name: HashMap<String, usize>,\n    social_ranks_by_security_level: HashMap<u32, usize>,\n".to_owned(),
        field_values: "            social_ranks_by_name,\n            social_ranks_by_security_level,\n".to_owned(),
        initializers: r#"        let mut social_ranks_by_name = HashMap::new();
        let mut social_ranks_by_security_level = HashMap::new();
        for (index, value) in social_rank_database.ranks.iter().enumerate() {
            let rank = &value.guild_rank_data;
            let name = normalize_lookup_key(&rank.name);
            if !name.is_empty() { social_ranks_by_name.entry(name).or_insert(index); }
            social_ranks_by_security_level.entry(rank.security_level).or_insert(index);
        }
"#.to_owned(),
        methods: r#"    pub fn rank_by_name(&self, name: &str) -> Option<&SocialRankData> { self.social_rank_database.ranks.get(*self.social_ranks_by_name.get(&normalize_lookup_key(name))?) }
    pub fn rank_by_security_level(&self, level: u32) -> Option<&SocialRankData> { self.social_rank_database.ranks.get(*self.social_ranks_by_security_level.get(&level)?) }

"#.to_owned(),
        ..RustNativeManagerAugmentation::default()
    })
}

fn player(context: &AugmentationContext<'_>) -> Result<RustNativeManagerAugmentation> {
    require_products(
        context,
        &["PlayerBaseAttributes", "SettlementProgressionData"],
    )?;
    Ok(RustNativeManagerAugmentation {
        methods: r#"    pub fn has_player_base_attributes(&self) -> bool { !self.player_base_attributes.player_attribute_data.item_rarity_data.is_empty() }
    pub fn has_settlement_progression_data(&self) -> bool { !self.settlement_progression_data.settlement_progression_categories.is_empty() }
    pub fn player_products_are_loaded(&self) -> bool {
        self.has_player_base_attributes() || self.has_settlement_progression_data()
    }

"#.to_owned(),
        ..RustNativeManagerAugmentation::default()
    })
}

fn recipe(context: &AugmentationContext<'_>) -> Result<RustNativeManagerAugmentation> {
    require_products(context, &["CraftingStationDatabase"])?;
    let row = context.find_first_row(&["CraftingRecipeData", "RecipeData"])?;
    let parts = context.row_parts(row);
    let id = text(context, row, "RecipeID", "source.row")?;
    let category = optional_text(row, "CraftingCategory", "source.row")?;
    let item = optional_text(row, "ItemID", "source.row")?;
    let output = optional_number(row, "OutputQty", "source.row", "1.0")?;
    let ingredients = (1..=7u8)
        .filter_map(|slot| field(row, &format!("Ingredient{slot}")).map(|value| (slot, value)))
        .map(|(slot, ingredient)| {
            let ingredient = string_value_expression(ingredient, "source.row")?;
            let quantity = optional_number(row, &format!("Qty{slot}"), "source.row", "1.0")?;
            Ok(format!("            {{ let id = Crc32::from_str_lower({ingredient}.trim()); let quantity = recipe_quantity({quantity}, 1); if id != Crc32::ZERO && quantity != 0 {{ data.ingredients.push(RecipeIngredient {{ ingredient_id: id, quantity }}); }} }}\n"))
        })
        .collect::<Result<String>>()?;
    Ok(RustNativeManagerAugmentation {
        declarations: format!(
            r#"#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RecipeIngredient {{ pub ingredient_id: Crc32, pub quantity: u32 }}

#[derive(Debug, Clone, PartialEq)]
pub struct StaticRecipeData {{
    pub source: RowRef<{table}, {schema}>,
    pub recipe_key: String,
    pub recipe_id: Crc32,
    pub crafting_category: Crc32,
    pub item_id: Crc32,
    pub output_quantity: u32,
    pub ingredients: Vec<RecipeIngredient>,
}}

fn recipe_quantity(value: f32, default: u32) -> u32 {{
    if value.is_finite()
        && value >= 0.0
        && value.fract() == 0.0
        && f64::from(value) <= f64::from(u32::MAX)
    {{
        value as u32
    }} else {{
        default
    }}
}}

"#,
            table = parts.table_type,
            schema = parts.row_type,
        ),
        fields: "    recipe_entries: Vec<StaticRecipeData>,\n    recipes_by_id: HashMap<Crc32, usize>,\n    recipes_by_result: HashMap<Crc32, Vec<usize>>,\n    recipes_by_category: HashMap<Crc32, Vec<usize>>,\n    crafting_stations_by_name: HashMap<String, usize>,\n    crafting_stations_by_type: HashMap<String, Vec<usize>>,\n".to_owned(),
        field_values: "            recipe_entries,\n            recipes_by_id,\n            recipes_by_result,\n            recipes_by_category,\n            crafting_stations_by_name,\n            crafting_stations_by_type,\n".to_owned(),
        initializers: format!(
            r#"        let mut recipe_entries = Vec::new();
        let mut recipes_by_id = HashMap::new();
        let mut recipes_by_result = HashMap::<Crc32, Vec<usize>>::new();
        let mut recipes_by_category = HashMap::<Crc32, Vec<usize>>::new();
        for source in {rows}.rows() {{
            let recipe_key = {id}.trim();
            let recipe_id = Crc32::from_str_lower(recipe_key);
            if recipe_key.is_empty() || recipe_id == Crc32::ZERO || recipes_by_id.contains_key(&recipe_id) {{ continue; }}
            let mut data = StaticRecipeData {{ source: source.reference.clone(), recipe_key: recipe_key.to_owned(), recipe_id, crafting_category: Crc32::from_str_lower({category}.trim()), item_id: Crc32::from_str_lower({item}.trim()), output_quantity: recipe_quantity({output}, 1).max(1), ingredients: Vec::new() }};
{ingredients}            let index = recipe_entries.len();
            recipes_by_id.insert(recipe_id, index);
            if data.item_id != Crc32::ZERO {{ recipes_by_result.entry(data.item_id).or_default().push(index); }}
            if data.crafting_category != Crc32::ZERO {{ recipes_by_category.entry(data.crafting_category).or_default().push(index); }}
            recipe_entries.push(data);
        }}
        let mut crafting_stations_by_name = HashMap::new();
        let mut crafting_stations_by_type = HashMap::<String, Vec<usize>>::new();
        for (index, station) in crafting_station_database.crafting_stations.iter().enumerate() {{
            let name = normalize_lookup_key(&station.name);
            if !name.is_empty() {{ crafting_stations_by_name.entry(name).or_insert(index); }}
            for kind in &station.crafting_types {{ crafting_stations_by_type.entry(normalize_lookup_key(kind)).or_default().push(index); }}
        }}
"#,
            rows = parts.row_field,
        ),
        methods: r#"    pub fn crafting_recipe_data(&self, key: &str) -> Option<&StaticRecipeData> { self.crafting_recipe_data_from_id(Crc32::from_str_lower(key)) }
    pub fn crafting_recipe_data_from_id(&self, id: Crc32) -> Option<&StaticRecipeData> { self.recipe_entries.get(*self.recipes_by_id.get(&id)?) }
    pub fn crafting_recipe_data_by_result(&self, item_id: Crc32) -> impl Iterator<Item = &StaticRecipeData> { self.recipes_by_result.get(&item_id).into_iter().flatten().filter_map(|index| self.recipe_entries.get(*index)) }
    pub fn recipes_by_category(&self, category: &str) -> impl Iterator<Item = &StaticRecipeData> { self.recipes_by_category_id(Crc32::from_str_lower(category)) }
    pub fn recipes_by_category_id(&self, id: Crc32) -> impl Iterator<Item = &StaticRecipeData> { self.recipes_by_category.get(&id).into_iter().flatten().filter_map(|index| self.recipe_entries.get(*index)) }
    pub fn recipes(&self) -> std::slice::Iter<'_, StaticRecipeData> { self.recipe_entries.iter() }
    pub fn recipes_by_ingredients<'a>(&'a self, available: &'a HashMap<Crc32, u32>) -> impl Iterator<Item = &'a StaticRecipeData> { self.recipe_entries.iter().filter(|recipe| recipe.ingredients.iter().all(|ingredient| available.get(&ingredient.ingredient_id).copied().unwrap_or_default() >= ingredient.quantity)) }
    pub fn crafting_station(&self, name: &str) -> Option<&CraftingStationData> { self.crafting_station_database.crafting_stations.get(*self.crafting_stations_by_name.get(&normalize_lookup_key(name))?) }
    pub fn crafting_stations_for_type(&self, kind: &str) -> impl Iterator<Item = &CraftingStationData> { self.crafting_stations_by_type.get(&normalize_lookup_key(kind)).into_iter().flatten().filter_map(|index| self.crafting_station_database.crafting_stations.get(*index)) }

"#.to_owned(),
        rows_type: "StaticRecipeData".to_owned(),
        rows_method: "recipes".to_owned(),
    })
}

fn tradeskill_rank(context: &AugmentationContext<'_>) -> Result<RustNativeManagerAugmentation> {
    let experience = context.row("ExperienceData")?;
    let rank = context.row("TradeskillRankData")?;
    let experience_parts = context.row_parts(experience);
    let rank_parts = context.row_parts(rank);
    let level = number(context, experience, "Level Number", "source.row", "0.0")?;
    let rank_level = number(context, rank, "Level", "source.row", "0.0")?;
    let display = optional_text(rank, "DisplayName", "source.row")?;
    Ok(RustNativeManagerAugmentation {
        declarations: format!(
            r#"#[derive(Debug, Clone, PartialEq)]
pub struct PlayerLevelRankData {{ pub rank: TradeskillRank, pub source: RowRef<{experience_table}, {experience_schema}> }}

#[derive(Debug, Clone, PartialEq)]
pub struct StaticTradeskillRankData {{ pub table: {rank_table}, pub rank: TradeskillRank, pub display_name: Option<String>, pub display_name_id: Crc32, pub source: RowRef<{rank_table}, {rank_schema}> }}

fn tradeskill_rank_value(value: f32) -> Option<TradeskillRank> {{
    (value.is_finite() && value >= 0.0 && value.fract() == 0.0 && value <= u16::MAX as f32)
        .then(|| TradeskillRank::new(value as u16))
}}

"#,
            experience_table = experience_parts.table_type,
            experience_schema = experience_parts.row_type,
            rank_table = rank_parts.table_type,
            rank_schema = rank_parts.row_type,
        ),
        fields: format!(
            "    player_levels: Vec<PlayerLevelRankData>,\n    player_levels_by_rank: HashMap<TradeskillRank, usize>,\n    tradeskill_ranks: Vec<StaticTradeskillRankData>,\n    tradeskill_ranks_by_key: HashMap<({}, TradeskillRank), usize>,\n",
            rank_parts.table_type
        ),
        field_values: "            player_levels,\n            player_levels_by_rank,\n            tradeskill_ranks,\n            tradeskill_ranks_by_key,\n".to_owned(),
        initializers: format!(
            r#"        let mut player_levels = Vec::new();
        let mut player_levels_by_rank = HashMap::new();
        for source in {experience_rows}.rows() {{
            let Some(rank) = tradeskill_rank_value({level}) else {{ continue; }};
            if player_levels_by_rank.contains_key(&rank) {{ continue; }}
            player_levels_by_rank.insert(rank, player_levels.len());
            player_levels.push(PlayerLevelRankData {{ rank, source: source.reference.clone() }});
        }}
        let mut tradeskill_ranks = Vec::new();
        let mut tradeskill_ranks_by_key = HashMap::new();
        for source in {rank_rows}.rows() {{
            let Some(rank) = tradeskill_rank_value({rank_level}) else {{ continue; }};
            let table = *source.reference.table();
            if tradeskill_ranks_by_key.contains_key(&(table, rank)) {{ continue; }}
            let display_name = match {display}.trim() {{ "" => None, value => Some(value.to_owned()) }};
            let display_name_id = display_name.as_deref().map(Crc32::from_str_lower).unwrap_or(Crc32::ZERO);
            tradeskill_ranks_by_key.insert((table, rank), tradeskill_ranks.len());
            tradeskill_ranks.push(StaticTradeskillRankData {{ table, rank, display_name, display_name_id, source: source.reference.clone() }});
        }}
"#,
            experience_rows = experience_parts.row_field,
            rank_rows = rank_parts.row_field,
        ),
        methods: format!(
            r#"    pub fn player_level_row(&self, rank: TradeskillRank) -> Option<&PlayerLevelRankData> {{ self.player_levels.get(*self.player_levels_by_rank.get(&rank)?) }}
    pub fn tradeskill_rank(&self, table: {rank_table}, rank: TradeskillRank) -> Option<&StaticTradeskillRankData> {{ self.tradeskill_ranks.get(*self.tradeskill_ranks_by_key.get(&(table, rank))?) }}
    pub fn player_levels(&self) -> std::slice::Iter<'_, PlayerLevelRankData> {{ self.player_levels.iter() }}
    pub fn tradeskill_ranks(&self) -> std::slice::Iter<'_, StaticTradeskillRankData> {{ self.tradeskill_ranks.iter() }}
    pub fn len(&self) -> usize {{ self.player_levels.len() + self.tradeskill_ranks.len() }}
    pub fn is_empty(&self) -> bool {{ self.player_levels.is_empty() && self.tradeskill_ranks.is_empty() }}

"#,
            rank_table = rank_parts.table_type,
        ),
        rows_type: "StaticTradeskillRankData".to_owned(),
        rows_method: "tradeskill_ranks".to_owned(),
    })
}

fn mutation_helpers() -> &'static str {
    r#"fn mutation_status_priority(value: Option<&str>) -> Option<i32> {
    let value = value?.trim().parse::<f64>().ok()?;
    (value.is_finite()
        && value.fract() == 0.0
        && value >= f64::from(i32::MIN)
        && value <= f64::from(i32::MAX))
        .then_some(value as i32)
}

fn mutation_status_group_and_tier(key: &str) -> Option<(String, u32)> {
    let key = key.trim();
    let group = key
        .chars()
        .filter(|value| !value.is_ascii_digit())
        .collect::<String>();
    if group.is_empty() {
        return None;
    }
    let tier = key
        .chars()
        .filter(char::is_ascii_digit)
        .collect::<String>();
    let tier = if tier.is_empty() { 0 } else { tier.parse().ok()? };
    Some((group, tier))
}

"#
}

fn require_products(context: &AugmentationContext<'_>, expected: &[&str]) -> Result<()> {
    for expected in expected {
        if !context
            .manager
            .products
            .iter()
            .any(|product| product.value_type.rsplit("::").next() == Some(*expected))
        {
            bail!(
                "standalone Rust manager `{}` requires decoded product `{expected}`",
                context.manager.manager_name
            );
        }
    }
    Ok(())
}

fn field<'a>(
    row: &'a RustStandaloneSchemaRow,
    name: &str,
) -> Option<&'a RustStandaloneSchemaField> {
    row.fields
        .iter()
        .find(|candidate| candidate.source_name.eq_ignore_ascii_case(name))
}

fn text(
    context: &AugmentationContext<'_>,
    row: &RustStandaloneSchemaRow,
    name: &str,
    receiver: &str,
) -> Result<String> {
    string_value_expression(context.field(row, name)?, receiver)
}

fn optional_text(row: &RustStandaloneSchemaRow, name: &str, receiver: &str) -> Result<String> {
    field(row, name)
        .map(|value| string_value_expression(value, receiver))
        .transpose()
        .map(|value| value.unwrap_or_else(|| "\"\"".to_owned()))
}

fn number(
    context: &AugmentationContext<'_>,
    row: &RustStandaloneSchemaRow,
    name: &str,
    receiver: &str,
    default: &str,
) -> Result<String> {
    numeric_value_or_default_expression(context.field(row, name)?, receiver, default)
}

fn optional_number(
    row: &RustStandaloneSchemaRow,
    name: &str,
    receiver: &str,
    default: &str,
) -> Result<String> {
    field(row, name)
        .map(|value| numeric_value_or_default_expression(value, receiver, default))
        .transpose()
        .map(|value| value.unwrap_or_else(|| default.to_owned()))
}
