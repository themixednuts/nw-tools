//! Standalone, schema-row-backed augmentations for indexed semantic managers.
//!
//! The emitted code owns only stable row references and semantic projections. It
//! deliberately has no engine assets, generated table crates, or implementation-
//! provenance marker names in its public or private surface.

use super::*;

pub(super) fn augmentation(
    context: &AugmentationContext<'_>,
    shape: &NativeManagerShape,
) -> Option<Result<RustNativeManagerAugmentation>> {
    let result = match shape {
        NativeManagerShape::OneTableCampSkin(_) => camp_skin(context),
        NativeManagerShape::OneTableEmote(_) => emote(context),
        NativeManagerShape::OneTableStoreCategory(_) => crc_rows(
            context,
            "StoreCategoryProperties",
            "StoreCategory",
            "store_categories",
            &["store_category_properties_from_id"],
            &[
                "store_category_properties",
                "store_category_properties_by_name",
            ],
            Some("categories"),
        ),
        NativeManagerShape::OneTableStoreProduct(_) => store_product(context),
        NativeManagerShape::OneTableRewardTrackItem(_) => reward_track_item(context),
        NativeManagerShape::OneTableWorldEventRule(shape) => world_event_rule(context, shape),
        NativeManagerShape::QuickCourseData(shape) => quick_course(context, shape),
        NativeManagerShape::RotationalQueueData(_) => crc_rows(
            context,
            "RotationalQueueData",
            "RotationalQueueID",
            "rotational_queues",
            &["rotational_queue_from_id"],
            &["rotational_queue"],
            Some("rotational_queues"),
        ),
        NativeManagerShape::DynamicDifficultyData(_) => dynamic_difficulty(context),
        NativeManagerShape::ProgressionPointData(_) => crc_rows(
            context,
            "ProgressionPointData",
            "ProgressionPointID",
            "progression_points",
            &["progression_point_from_id"],
            &["progression_point"],
            Some("progression_points"),
        ),
        NativeManagerShape::EntitlementData(_) => entitlement(context),
        NativeManagerShape::EquipmentSetData(_) => equipment_set(context),
        NativeManagerShape::OneTablePvpBalance(shape) => qualified_pvp_balance(context, shape),
        NativeManagerShape::OneTableDyeColor(shape) => dye_color(context, shape),
        NativeManagerShape::RewardTrackData(_) => reward_track(context),
        NativeManagerShape::PostSkillCapProgression(_) => crc_rows(
            context,
            "TradeSkillPostCapData",
            "TradeSkillType",
            "post_skill_cap_progression",
            &["post_skill_cap_progression_data_from_id"],
            &["post_skill_cap_progression_data"],
            Some("post_skill_cap_progression_rows"),
        ),
        NativeManagerShape::WhisperData(_) => whisper(context),
        NativeManagerShape::OneTableCrestPart(_) => numeric_rows(
            context,
            "CrestPartData",
            "Index",
            "crest_parts",
            NumericStorage::U32,
            &["crest_part_data_from_id", "crest_part_data_from_index"],
            Some("crest_parts"),
        ),
        NativeManagerShape::OneTableDungeonTile(_) => dungeon_tile(context),
        NativeManagerShape::OneTableLevelDisparity(_) => level_disparity(context),
        NativeManagerShape::OneTableEncumbrance(_) => crc_rows(
            context,
            "EncumbranceData",
            "ContainerTypeID",
            "encumbrance",
            &["encumbrance_data_from_id"],
            &["encumbrance_data", "encumbrance_data_by_key"],
            Some("encumbrance_rows"),
        ),
        NativeManagerShape::OneTableDifficultyScaling(_) => difficulty_scaling(context),
        NativeManagerShape::OneTableDarkness(_) => crc_rows(
            context,
            "DarknessData",
            "DarknessID",
            "darkness",
            &["darkness_data_by_crc32"],
            &["darkness_data"],
            Some("darkness_rows"),
        ),
        NativeManagerShape::OneTableParticleData(_) => particle(context),
        NativeManagerShape::CharacterAttributeData(_) => character_attribute(context),
        NativeManagerShape::GovernanceData(_) => governance(context),
        NativeManagerShape::LootBucketData(_) => loot_bucket(context),
        NativeManagerShape::TerritoryDefinitionsData(_) => territory(context),
        NativeManagerShape::StatModifierData(_) => stat_modifier(context),
        NativeManagerShape::StatusEffectData(_) => status_effect(context),
        NativeManagerShape::ItemConversionData(_) => item_conversion(context),
        NativeManagerShape::AbilityData(_) => ability(context),
        NativeManagerShape::ItemTransformData(_) => item_transform(context),
        _ => return None,
    };
    Some(result)
}

#[derive(Clone, Copy)]
enum NumericStorage {
    I32,
    U32,
}

fn crc_rows(
    context: &AugmentationContext<'_>,
    row_name: &str,
    key_column: &str,
    stem: &str,
    crc_methods: &[&str],
    text_methods: &[&str],
    rows_method: Option<&str>,
) -> Result<RustNativeManagerAugmentation> {
    let row = context.row(row_name)?;
    let key = context.field(row, key_column)?;
    require_string_field(context.manager, row, key)?;
    let parts = context.row_parts(row);
    let field = format!("{}_by_id", clean_ident(stem));
    let value = string_value_expression(key, "source.row")?;
    let mut methods = String::new();
    for method in crc_methods {
        methods.push_str(&format!(
            "    pub fn {method}(&self, id: Crc32) -> Option<&{row_type}> {{\n        self.{row_field}.get(self.{field}.get(&id)?)\n    }}\n\n",
            row_type = parts.row_type,
            row_field = parts.row_field,
        ));
    }
    for method in text_methods {
        methods.push_str(&format!(
            "    pub fn {method}(&self, key: &str) -> Option<&{row_type}> {{\n        self.{row_field}.get(self.{field}.get(&Crc32::from_str_lower(key.trim()))?)\n    }}\n\n",
            row_type = parts.row_type,
            row_field = parts.row_field,
        ));
    }
    if let Some(method) = rows_method {
        methods.push_str(&named_rows_method(method, &parts));
    }
    Ok(RustNativeManagerAugmentation {
        fields: format!(
            "    {field}: HashMap<Crc32, RowRef<{table}, {row_type}>>,\n",
            table = parts.table_type,
            row_type = parts.row_type,
        ),
        field_values: format!("            {field},\n"),
        initializers: format!(
            "        let mut {field} = HashMap::new();\n        for source in {row_field}.rows() {{\n            let text = {value}.trim();\n            if text.is_empty() {{ continue; }}\n            let id = Crc32::from_str_lower(text);\n            if id == Crc32::ZERO {{ continue; }}\n            {field}.entry(id).or_insert_with(|| source.reference.clone());\n        }}\n",
            row_field = parts.row_field,
        ),
        methods,
        ..RustNativeManagerAugmentation::default()
    })
}

fn numeric_rows(
    context: &AugmentationContext<'_>,
    row_name: &str,
    key_column: &str,
    stem: &str,
    storage: NumericStorage,
    lookup_methods: &[&str],
    rows_method: Option<&str>,
) -> Result<RustNativeManagerAugmentation> {
    let row = context.row(row_name)?;
    let key = context.field(row, key_column)?;
    let parts = context.row_parts(row);
    let field = format!("{}_by_number", clean_ident(stem));
    let value = numeric_value_expression(key, "source.row")?;
    let (ty, conversion) = match storage {
        NumericStorage::I32 => (
            "i32",
            "            if !raw.is_finite() || raw.fract() != 0.0 || raw < i32::MIN as f32 || raw > i32::MAX as f32 { continue; }\n            let key = raw as i32;\n",
        ),
        NumericStorage::U32 => (
            "u32",
            "            if !raw.is_finite() || raw.fract() != 0.0 || raw < 0.0 || raw > u32::MAX as f32 { continue; }\n            let key = raw as u32;\n",
        ),
    };
    let mut methods = lookup_methods
        .iter()
        .map(|method| {
            format!(
                "    pub fn {method}(&self, key: {ty}) -> Option<&{row_type}> {{\n        self.{row_field}.get(self.{field}.get(&key)?)\n    }}\n\n",
                row_type = parts.row_type,
                row_field = parts.row_field,
            )
        })
        .collect::<String>();
    if let Some(method) = rows_method {
        methods.push_str(&named_rows_method(method, &parts));
    }
    Ok(RustNativeManagerAugmentation {
        fields: format!(
            "    {field}: HashMap<{ty}, RowRef<{table}, {row_type}>>,\n",
            table = parts.table_type,
            row_type = parts.row_type,
        ),
        field_values: format!("            {field},\n"),
        initializers: format!(
            "        let mut {field} = HashMap::new();\n        for source in {row_field}.rows() {{\n            let raw = {value};\n{conversion}            {field}.entry(key).or_insert_with(|| source.reference.clone());\n        }}\n",
            row_field = parts.row_field,
        ),
        methods,
        ..RustNativeManagerAugmentation::default()
    })
}

fn camp_skin(context: &AugmentationContext<'_>) -> Result<RustNativeManagerAugmentation> {
    let mut value = crc_rows(
        context,
        "CampSkinData",
        "CampSkinID",
        "camp_skins",
        &["camp_skin_data_from_id"],
        &["camp_skin_data", "camp_skin_data_by_key"],
        Some("camp_skins"),
    )?;
    let row = context.row("CampSkinData")?;
    let parts = context.row_parts(row);
    let enabled = optional_bool(context, row, &["IsEnabled"], "source.row", "true")?;
    let item = optional_string(context, row, &["ItemID"], "source.row", "\"\"")?;
    value.initializers = value.initializers.replace(
        "            let text =",
        &format!(
            "            if !({enabled}) {{ continue; }}\n            if {item}.trim().is_empty() {{ continue; }}\n            let text ="
        ),
    );
    value.methods.push_str(&format!(
        "    pub fn enabled_camp_skins(&self) -> impl Iterator<Item = &{row}> + '_ {{\n        self.{field}.rows().map(|entry| &entry.row).filter(|row| {enabled})\n    }}\n\n",
        row = parts.row_type,
        field = parts.row_field,
        enabled = optional_bool(context, row, &["IsEnabled"], "row", "true")?,
    ));
    Ok(value)
}

fn emote(context: &AugmentationContext<'_>) -> Result<RustNativeManagerAugmentation> {
    let mut value = crc_rows(
        context,
        "EmoteData",
        "UniqueTagID",
        "emotes",
        &["emote_data_from_id"],
        &["emote_data", "emote_data_by_key"],
        Some("emotes"),
    )?;
    let row = context.row("EmoteData")?;
    let parts = context.row_parts(row);
    let display = context
        .field(row, "DisplayName")
        .or_else(|_| context.field(row, "Name"))?;
    let display_value = string_value_expression(display, "source.row")?;
    value.fields.push_str(&format!(
        "    emotes_by_display_name: HashMap<Crc32, RowRef<{}, {}>>,\n",
        parts.table_type, parts.row_type
    ));
    value
        .field_values
        .push_str("            emotes_by_display_name,\n");
    value.initializers.push_str(&format!(
        "        let mut emotes_by_display_name = HashMap::new();\n        for source in {field}.rows() {{\n            let text = {display_value}.trim();\n            if text.is_empty() {{ continue; }}\n            emotes_by_display_name.entry(Crc32::from_str_lower(text)).or_insert_with(|| source.reference.clone());\n        }}\n",
        field = parts.row_field,
    ));
    value.methods.push_str(&format!(
        "    pub fn emote_by_display_name(&self, name: &str) -> Option<&{row}> {{\n        self.{field}.get(self.emotes_by_display_name.get(&Crc32::from_str_lower(name.trim()))?)\n    }}\n\n",
        row = parts.row_type,
        field = parts.row_field,
    ));
    Ok(value)
}

fn store_product(context: &AugmentationContext<'_>) -> Result<RustNativeManagerAugmentation> {
    let mut value = crc_rows(
        context,
        "StoreProductData",
        "UniqueTagID",
        "store_products",
        &["store_product_data_from_id"],
        &["store_product_data", "store_product_data_by_tag"],
        Some("products"),
    )?;
    let row = context.row("StoreProductData")?;
    let enabled = optional_bool(context, row, &["IsEnabled"], "source.row", "true")?;
    value.initializers = value.initializers.replace(
        "            let text =",
        &format!("            if !({enabled}) {{ continue; }}\n            let text ="),
    );
    Ok(value)
}

fn reward_track_item(context: &AugmentationContext<'_>) -> Result<RustNativeManagerAugmentation> {
    let mut value = crc_rows(
        context,
        "RewardTrackItemData",
        "RewardID",
        "reward_track_items",
        &["reward_track_item_from_id"],
        &["reward_track_item", "reward_track_item_by_key"],
        Some("reward_track_items"),
    )?;
    let row = context.row("RewardTrackItemData")?;
    let entitlement = optional_string(context, row, &["Entitlement"], "source.row", "\"\"")?;
    let event = optional_string(context, row, &["GameEvent"], "source.row", "\"\"")?;
    let item = optional_string(context, row, &["Item"], "source.row", "\"\"")?;
    value.initializers = value.initializers.replace(
        "            let text =",
        &format!(
            "            if {entitlement}.trim().is_empty() && {event}.trim().is_empty() && {item}.trim().is_empty() {{ continue; }}\n            let text ="
        ),
    );
    Ok(value)
}

fn world_event_rule(
    context: &AugmentationContext<'_>,
    shape: &crate::manager::NativeOneTableWorldEventRuleManager,
) -> Result<RustNativeManagerAugmentation> {
    let row = context.row("WorldEventRuleData")?;
    let parts = context.row_parts(row);
    let rule_id = string_value_expression(context.field(row, "RuleID")?, "source.row")?;
    let disabled = optional_bool(context, row, &["Disabled"], "source.row", "false")?;
    let category = optional_string(
        context,
        row,
        &["Category", "EventCategory"],
        "source.row",
        "\"*\"",
    )?;
    let hubs = optional_string(
        context,
        row,
        &["Hub", "HubIDs", "HubId", "Hubs"],
        "source.row",
        "\"\"",
    )?;
    let zones = optional_string(
        context,
        row,
        &["Zone", "ZoneIDs", "ZoneId", "Zones"],
        "source.row",
        "\"\"",
    )?;
    let tags = optional_string(context, row, &["Tags", "EventTags"], "source.row", "\"\"")?;
    let max_events = optional_number(context, row, &["MaxEvents"], "source.row", "0.0")?;
    let min_distance = optional_number(context, row, &["MinDistance"], "source.row", "0.0")?;
    let lookup = clean_ident(shape.lookup_method().as_str());
    let lookup_by_crc = clean_ident(shape.lookup_by_crc_method().as_str());
    let rows = clean_ident(shape.rows_method().as_str());
    let len = clean_ident(shape.len_method().as_str());
    let is_empty = clean_ident(shape.is_empty_method().as_str());
    Ok(RustNativeManagerAugmentation {
        declarations: "#[derive(Debug, Clone, PartialEq, Eq)]\npub enum WorldEventRuleCrcFilter { Any, Specific(Vec<Crc32>) }\n\n#[derive(Debug, Clone, PartialEq, Eq)]\npub enum WorldEventRuleZoneFilter { Any, Zones(Vec<u16>) }\n\n#[derive(Debug, Clone, PartialEq)]\npub struct WorldEventRuleData {\n    pub rule_id: String,\n    pub rule_id_crc: Crc32,\n    pub max_events: u32,\n    pub min_distance: f32,\n    pub category: WorldEventRuleCrcFilter,\n    pub hub: WorldEventRuleCrcFilter,\n    pub zone: WorldEventRuleZoneFilter,\n    pub tags: Vec<Crc32>,\n    pub enabled: bool,\n}\n\nfn world_event_crc_filter(value: &str) -> WorldEventRuleCrcFilter {\n    let value = value.trim();\n    if value == \"*\" { return WorldEventRuleCrcFilter::Any; }\n    WorldEventRuleCrcFilter::Specific(value.split(',').map(str::trim).filter(|part| !part.is_empty()).map(Crc32::from_str_lower).filter(|id| *id != Crc32::ZERO).collect())\n}\n\nfn world_event_zone_filter(value: &str, rule_id: &str) -> Result<WorldEventRuleZoneFilter> {\n    let value = value.trim();\n    if value == \"*\" { return Ok(WorldEventRuleZoneFilter::Any); }\n    let mut zones = Vec::new();\n    for token in value.split(',').map(str::trim).filter(|part| !part.is_empty()) {\n        zones.push(token.parse::<u16>().with_context(|| format!(\"WorldEventRuleData `{rule_id}` has invalid Zone `{token}`\"))?);\n    }\n    Ok(WorldEventRuleZoneFilter::Zones(zones))\n}\n\n".to_owned(),
        fields: "    world_event_rules: Vec<WorldEventRuleData>,\n    world_event_rules_by_id: HashMap<Crc32, usize>,\n".to_owned(),
        field_values: "            world_event_rules,\n            world_event_rules_by_id,\n".to_owned(),
        initializers: format!(r#"        let mut world_event_rules = Vec::new();
        let mut world_event_rules_by_id = HashMap::new();
        for source in {field}.rows() {{
            let rule_id = {rule_id}.trim();
            let id = Crc32::from_str_lower(rule_id);
            if rule_id.is_empty() || id == Crc32::ZERO || world_event_rules_by_id.contains_key(&id) {{ continue; }}
            let max_events_value = {max_events};
            if !max_events_value.is_finite() || max_events_value.fract() != 0.0 || max_events_value < 0.0 || max_events_value >= 4_294_967_296.0 {{
                bail!("WorldEventRuleData `{{rule_id}}` has invalid MaxEvents `{{max_events_value}}`");
            }}
            let min_distance = {min_distance};
            if !min_distance.is_finite() {{ bail!("WorldEventRuleData `{{rule_id}}` has non-finite MinDistance"); }}
            let category = world_event_crc_filter({category});
            let hub = world_event_crc_filter({hubs});
            let zone = world_event_zone_filter({zones}, rule_id)?;
            let tags = {tags}.split(',').map(str::trim).filter(|part| !part.is_empty()).map(Crc32::from_str_lower).filter(|id| *id != Crc32::ZERO).collect();
            world_event_rules_by_id.insert(id, world_event_rules.len());
            world_event_rules.push(WorldEventRuleData {{ rule_id: rule_id.to_owned(), rule_id_crc: id, max_events: max_events_value as u32, min_distance, category, hub, zone, tags, enabled: !({disabled}) }});
        }}
"#,
        field = parts.row_field,
        ),
        methods: format!("    pub fn {lookup_by_crc}(&self, id: Crc32) -> Option<&WorldEventRuleData> {{ self.world_event_rules.get(*self.world_event_rules_by_id.get(&id)?) }}\n\n    pub fn {lookup}(&self, key: &str) -> Option<&WorldEventRuleData> {{ self.{lookup_by_crc}(Crc32::from_str_lower(key.trim())) }}\n\n    pub fn {rows}(&self) -> impl ExactSizeIterator<Item = &WorldEventRuleData> {{ self.world_event_rules.iter() }}\n\n    pub fn {len}(&self) -> usize {{ self.world_event_rules.len() }}\n\n    pub fn {is_empty}(&self) -> bool {{ self.world_event_rules.is_empty() }}\n\n"),
        rows_type: "WorldEventRuleData".to_owned(),
        rows_method: rows,
    })
}

fn quick_course(
    context: &AugmentationContext<'_>,
    shape: &crate::manager::NativeQuickCourseDataManager,
) -> Result<RustNativeManagerAugmentation> {
    let course = context.find_first_row(&["QuickCourseData", "TimedRaceCourseData"])?;
    let node = context.find_first_row(&["QuickCourseNodeTypeData", "TimedRaceNodeTypeData"])?;
    let course_parts = context.row_parts(course);
    let node_parts = context.row_parts(node);
    let course_id = optional_string(
        context,
        course,
        &["QuickCourseID", "TimedRaceCourseId"],
        "source.row",
        "\"\"",
    )?;
    let path = optional_string(
        context,
        course,
        &["PathReferenceQuickCourseID"],
        "source.row",
        "\"\"",
    )?;
    let is_timed = optional_bool(context, course, &["IsTimed"], "source.row", "false")?;
    let starting = optional_number(
        context,
        course,
        &["StartingTimerSeconds"],
        "source.row",
        "0.0",
    )?;
    let accumulate = optional_bool(context, course, &["AccumulateTime"], "source.row", "false")?;
    let multiplier = optional_number(
        context,
        course,
        &["NodeTimeOverrideMultiplier"],
        "source.row",
        "1.0",
    )?;
    let audio = optional_string(context, course, &["AudioGroup"], "source.row", "\"\"")?;
    let node_id = optional_string(
        context,
        node,
        &["TimedRaceNodeTypeId", "NodeTypeID"],
        "source.row",
        "\"\"",
    )?;
    let radius = optional_number(context, node, &["DetectionRadius"], "source.row", "0.0")?;
    let use_override = optional_bool(context, node, &["UseTimeOverride"], "source.row", "false")?;
    let add_time = optional_number(context, node, &["AddTimeSeconds"], "source.row", "0.0")?;
    let visual = optional_string(context, node, &["VisualSlicePath"], "source.row", "\"\"")?;
    let sfx = optional_string(context, node, &["SFX"], "source.row", "\"\"")?;
    let quick_course_lookup = clean_ident(shape.quick_course_lookup_method().as_str());
    let quick_course_lookup_by_crc =
        clean_ident(shape.quick_course_lookup_by_crc_method().as_str());
    let quick_courses = clean_ident(shape.quick_courses_method().as_str());
    let quick_course_ids = clean_ident(shape.quick_course_ids_method().as_str());
    let first_quick_course_id = clean_ident(shape.first_quick_course_id_method().as_str());
    let node_type_lookup = clean_ident(shape.node_type_lookup_method().as_str());
    let node_type_lookup_by_crc = clean_ident(shape.node_type_lookup_by_crc_method().as_str());
    let node_types = clean_ident(shape.node_types_method().as_str());
    let node_type_ids = clean_ident(shape.node_type_ids_method().as_str());
    let first_node_type_id = clean_ident(shape.first_node_type_id_method().as_str());
    let quick_course_len = clean_ident(shape.quick_course_len_method().as_str());
    let node_type_len = clean_ident(shape.node_type_len_method().as_str());
    let is_empty = clean_ident(shape.is_empty_method().as_str());
    Ok(RustNativeManagerAugmentation {
        declarations: "#[derive(Debug, Clone, PartialEq)]\npub struct QuickCourseData {\n    pub id: String,\n    pub id_crc: Crc32,\n    pub path_reference_id: Option<Crc32>,\n    pub is_timed: bool,\n    pub starting_timer_seconds: u32,\n    pub accumulate_time: bool,\n    pub node_time_override_multiplier: f32,\n    pub audio_group: String,\n}\n\n#[derive(Debug, Clone, PartialEq)]\npub struct QuickCourseNodeTypeData {\n    pub id: String,\n    pub id_crc: Crc32,\n    pub detection_radius: f32,\n    pub use_time_override: bool,\n    pub add_time_seconds: f32,\n    pub visual_slice_path: String,\n    pub sfx: String,\n}\n\n".to_owned(),
        fields: "    quick_courses: Vec<QuickCourseData>,\n    quick_courses_by_id: HashMap<Crc32, usize>,\n    quick_course_ids: Vec<String>,\n    quick_course_node_types: Vec<QuickCourseNodeTypeData>,\n    quick_course_node_types_by_id: HashMap<Crc32, usize>,\n    quick_course_node_type_ids: Vec<String>,\n".to_owned(),
        field_values: "            quick_courses,\n            quick_courses_by_id,\n            quick_course_ids,\n            quick_course_node_types,\n            quick_course_node_types_by_id,\n            quick_course_node_type_ids,\n".to_owned(),
        initializers: format!(r#"        let mut quick_courses = Vec::new();
        let mut quick_courses_by_id = HashMap::new();
        let mut quick_course_ids = Vec::new();
        for source in {course_field}.rows() {{
            let key = {course_id}.trim();
            let id = Crc32::from_str_lower(key);
            if key.is_empty() || id == Crc32::ZERO {{ continue; }}
            let starting = {starting};
            if !starting.is_finite() || starting.fract() != 0.0 || starting < 0.0 || starting >= 4_294_967_296.0 {{ bail!("QuickCourseData `{{key}}` has invalid StartingTimerSeconds `{{starting}}`"); }}
            let mut factor = {multiplier};
            if !factor.is_finite() {{ bail!("QuickCourseData `{{key}}` has non-finite NodeTimeOverrideMultiplier"); }}
            if factor == 0.0 {{ factor = 1.0; }}
            let path = {path}.trim();
            let data = QuickCourseData {{ id: key.to_owned(), id_crc: id, path_reference_id: (!path.is_empty()).then(|| Crc32::from_str_lower(path)), is_timed: {is_timed}, starting_timer_seconds: starting as u32, accumulate_time: {accumulate}, node_time_override_multiplier: factor, audio_group: {audio}.trim().to_owned() }};
            quick_course_ids.push(key.to_owned());
            if let Some(index) = quick_courses_by_id.get(&id).copied() {{ quick_courses[index] = data; }} else {{ quick_courses_by_id.insert(id, quick_courses.len()); quick_courses.push(data); }}
        }}
        let mut quick_course_node_types = Vec::new();
        let mut quick_course_node_types_by_id = HashMap::new();
        let mut quick_course_node_type_ids = Vec::new();
        for source in {node_field}.rows() {{
            let key = {node_id}.trim();
            let id = Crc32::from_str_lower(key);
            if key.is_empty() || id == Crc32::ZERO {{ continue; }}
            let detection_radius = {radius};
            let add_time_seconds = {add_time};
            if !detection_radius.is_finite() || !add_time_seconds.is_finite() {{ bail!("QuickCourseNodeTypeData `{{key}}` has non-finite numeric data"); }}
            let data = QuickCourseNodeTypeData {{ id: key.to_owned(), id_crc: id, detection_radius, use_time_override: {use_override}, add_time_seconds, visual_slice_path: {visual}.trim().to_ascii_lowercase(), sfx: {sfx}.trim().to_owned() }};
            quick_course_node_type_ids.push(key.to_owned());
            if let Some(index) = quick_course_node_types_by_id.get(&id).copied() {{ quick_course_node_types[index] = data; }} else {{ quick_course_node_types_by_id.insert(id, quick_course_node_types.len()); quick_course_node_types.push(data); }}
        }}
"#, course_field=course_parts.row_field, node_field=node_parts.row_field),
        methods: format!("    pub fn {quick_course_lookup_by_crc}(&self, id: Crc32) -> Option<&QuickCourseData> {{ self.quick_courses.get(*self.quick_courses_by_id.get(&id)?) }}\n\n    pub fn {quick_course_lookup}(&self, key: &str) -> Option<&QuickCourseData> {{ self.{quick_course_lookup_by_crc}(Crc32::from_str_lower(key.trim())) }}\n\n    pub fn {quick_courses}(&self) -> impl ExactSizeIterator<Item = &QuickCourseData> {{ self.quick_courses.iter() }}\n\n    pub fn {quick_course_ids}(&self) -> impl ExactSizeIterator<Item = &str> {{ self.quick_course_ids.iter().map(String::as_str) }}\n\n    pub fn {first_quick_course_id}(&self) -> Option<&str> {{ self.quick_course_ids.first().map(String::as_str) }}\n\n    pub fn {node_type_lookup_by_crc}(&self, id: Crc32) -> Option<&QuickCourseNodeTypeData> {{ self.quick_course_node_types.get(*self.quick_course_node_types_by_id.get(&id)?) }}\n\n    pub fn {node_type_lookup}(&self, key: &str) -> Option<&QuickCourseNodeTypeData> {{ self.{node_type_lookup_by_crc}(Crc32::from_str_lower(key.trim())) }}\n\n    pub fn {node_types}(&self) -> impl ExactSizeIterator<Item = &QuickCourseNodeTypeData> {{ self.quick_course_node_types.iter() }}\n\n    pub fn {node_type_ids}(&self) -> impl ExactSizeIterator<Item = &str> {{ self.quick_course_node_type_ids.iter().map(String::as_str) }}\n\n    pub fn {first_node_type_id}(&self) -> Option<&str> {{ self.quick_course_node_type_ids.first().map(String::as_str) }}\n\n    pub fn {quick_course_len}(&self) -> usize {{ self.quick_courses.len() }}\n\n    pub fn {node_type_len}(&self) -> usize {{ self.quick_course_node_types.len() }}\n\n    pub fn {is_empty}(&self) -> bool {{ self.quick_courses.is_empty() && self.quick_course_node_types.is_empty() }}\n\n"),
        rows_type: "QuickCourseData".to_owned(),
        rows_method: quick_courses,
    })
}

fn dynamic_difficulty(context: &AugmentationContext<'_>) -> Result<RustNativeManagerAugmentation> {
    let mut value = crc_rows(
        context,
        "DynamicDifficultyStaticData",
        "DynamicDifficultyID",
        "dynamic_difficulties",
        &["dynamic_difficulty_data_from_id"],
        &["dynamic_difficulty_data", "dynamic_difficulty_data_by_key"],
        Some("dynamic_difficulties"),
    )?;
    let row = context.row("DynamicDifficultyStaticData")?;
    let parts = context.row_parts(row);
    let modes = optional_string(context, row, &["GameModeIds"], "source.row", "\"\"")?;
    value
        .fields
        .push_str("    dynamic_difficulty_modes: HashMap<Crc32, Vec<Crc32>>,\n");
    value
        .field_values
        .push_str("            dynamic_difficulty_modes,\n");
    value.initializers.push_str(&format!(
        "        let mut dynamic_difficulty_modes = HashMap::new();\n        for source in {field}.rows() {{\n            let Some((id, _)) = dynamic_difficulties_by_id.iter().find(|(_, reference)| *reference == &source.reference) else {{ continue; }};\n            let modes = split_designer_list({modes}).into_iter().map(Crc32::from_str_lower).filter(|id| *id != Crc32::ZERO).collect();\n            dynamic_difficulty_modes.insert(*id, modes);\n        }}\n",
        field = parts.row_field,
    ));
    value.methods.push_str("    pub fn dynamic_difficulty_game_modes(&self, id: Crc32) -> &[Crc32] { self.dynamic_difficulty_modes.get(&id).map(Vec::as_slice).unwrap_or_default() }\n\n");
    add_designer_list_helper(&mut value);
    Ok(value)
}

fn entitlement(context: &AugmentationContext<'_>) -> Result<RustNativeManagerAugmentation> {
    let mut value = crc_rows(
        context,
        "EntitlementData",
        "UniqueTagID",
        "entitlements",
        &["by_id"],
        &["by_key"],
        Some("entitlements"),
    )?;
    let row = context.row("EntitlementData")?;
    let parts = context.row_parts(row);
    let index = context.field(row, "EntitlementIndex")?;
    let index_value = numeric_value_expression(index, "source.row")?;
    let rewards = optional_string(context, row, &["Reward(s)"], "source.row", "\"\"")?;
    value.fields.push_str(&format!("    entitlements_by_index: HashMap<u32, RowRef<{}, {}>>,\n    entitlements_by_reward: HashMap<Crc32, Vec<RowRef<{}, {}>>>,\n", parts.table_type, parts.row_type, parts.table_type, parts.row_type));
    value
        .field_values
        .push_str("            entitlements_by_index,\n            entitlements_by_reward,\n");
    value.initializers.push_str(&format!(r#"        let mut entitlements_by_index = HashMap::new();
        let mut entitlements_by_reward: HashMap<Crc32, Vec<RowRef<{table}, {row}>>> = HashMap::new();
        for source in {field}.rows() {{
            let raw = {index_value};
            if raw.is_finite() && raw.fract() == 0.0 && raw >= 0.0 && raw <= u32::MAX as f32 {{ entitlements_by_index.insert(raw as u32, source.reference.clone()); }}
            for reward in split_designer_list({rewards}) {{ let id = Crc32::from_str_lower(reward); if id != Crc32::ZERO {{ entitlements_by_reward.entry(id).or_default().push(source.reference.clone()); }} }}
        }}
"#, table=parts.table_type, row=parts.row_type, field=parts.row_field));
    value.methods.push_str(&format!("    pub fn by_index(&self, index: u32) -> Option<&{row}> {{ self.{field}.get(self.entitlements_by_index.get(&index)?) }}\n\n    pub fn entitlements_for_reward(&self, reward: Crc32) -> impl Iterator<Item = &{row}> + '_ {{ self.entitlements_by_reward.get(&reward).into_iter().flatten().filter_map(|reference| self.{field}.get(reference)) }}\n\n", row=parts.row_type, field=parts.row_field));
    add_designer_list_helper(&mut value);
    Ok(value)
}

fn equipment_set(context: &AugmentationContext<'_>) -> Result<RustNativeManagerAugmentation> {
    let mut value = crc_rows(
        context,
        "EquipmentSetData",
        "EquipmentSetID",
        "equipment_sets",
        &["by_id"],
        &["by_key"],
        Some("sets"),
    )?;
    let row = context.row("EquipmentSetData")?;
    let parts = context.row_parts(row);
    let items = optional_string(context, row, &["ItemIds", "ItemIDs"], "source.row", "\"\"")?;
    let perks = row
        .fields
        .iter()
        .filter(|field| {
            field.source_name.starts_with("Perk") && !field.source_name.ends_with("Threshold")
        })
        .map(|field| string_value_expression(field, "source.row"))
        .collect::<Result<Vec<_>>>()?;
    let perk_insert = perks.iter().map(|perk| format!("            {{ let id = Crc32::from_str_lower({perk}.trim()); if id != Crc32::ZERO {{ equipment_sets_by_perk.entry(id).or_default().push(source.reference.clone()); }} }}\n")).collect::<String>();
    value.fields.push_str(&format!("    equipment_sets_by_item: HashMap<Crc32, Vec<RowRef<{}, {}>>>,\n    equipment_sets_by_perk: HashMap<Crc32, Vec<RowRef<{}, {}>>>,\n",parts.table_type,parts.row_type,parts.table_type,parts.row_type));
    value
        .field_values
        .push_str("            equipment_sets_by_item,\n            equipment_sets_by_perk,\n");
    value.initializers.push_str(&format!(r#"        let mut equipment_sets_by_item: HashMap<Crc32, Vec<RowRef<{table}, {row}>>> = HashMap::new();
        let mut equipment_sets_by_perk: HashMap<Crc32, Vec<RowRef<{table}, {row}>>> = HashMap::new();
        for source in {field}.rows() {{
            for item in split_designer_list({items}) {{ let id = Crc32::from_str_lower(item); if id != Crc32::ZERO {{ equipment_sets_by_item.entry(id).or_default().push(source.reference.clone()); }} }}
{perk_insert}        }}
"#,table=parts.table_type,row=parts.row_type,field=parts.row_field));
    value.methods.push_str(&format!("    pub fn sets_for_item(&self, item: Crc32) -> impl Iterator<Item = &{row}> + '_ {{ self.equipment_sets_by_item.get(&item).into_iter().flatten().filter_map(|reference| self.{field}.get(reference)) }}\n\n    pub fn sets_for_perk(&self, perk: Crc32) -> impl Iterator<Item = &{row}> + '_ {{ self.equipment_sets_by_perk.get(&perk).into_iter().flatten().filter_map(|reference| self.{field}.get(reference)) }}\n\n",row=parts.row_type,field=parts.row_field));
    add_designer_list_helper(&mut value);
    Ok(value)
}

fn pvp_balance(
    context: &AugmentationContext<'_>,
    shape: &crate::manager::NativeOneTablePvpBalanceManager,
) -> Result<RustNativeManagerAugmentation> {
    let row_name = shape.row_type_name().as_str();
    let row = context.row(row_name)?;
    let parts = context.row_parts(row);
    let target = string_value_expression(
        context.field(row, shape.target_column().as_str())?,
        "source.row",
    )?;
    let category = optional_string(
        context,
        row,
        &[shape.category_column().as_str()],
        "source.row",
        "\"\"",
    )?;
    let ability = optional_balance_text(
        context,
        row,
        &["AbilityBaseDamageAdjustment"],
        "source.row",
        "AbilityBaseDamageAdjustment",
    )?;
    let affix = optional_balance_text(
        context,
        row,
        &["AffixStatAdjustment"],
        "source.row",
        "AffixStatAdjustment",
    )?;
    let incoming = optional_balance_text(
        context,
        row,
        &["IncomingHealAdjustment"],
        "source.row",
        "IncomingHealAdjustment",
    )?;
    let consumable = optional_balance_text(
        context,
        row,
        &["ConsumableHealAdjustment"],
        "source.row",
        "ConsumableHealAdjustment",
    )?;
    let potency = optional_f32_option(
        context,
        row,
        &["PotencyAdjustment"],
        "source.row",
        "PotencyAdjustment",
    )?;
    let duration = optional_f32_option(
        context,
        row,
        &["DurationAdjustment"],
        "source.row",
        "DurationAdjustment",
    )?;
    let weapon = optional_f32_option(
        context,
        row,
        &["WeaponBaseDamageAdjustment"],
        "source.row",
        "WeaponBaseDamageAdjustment",
    )?;
    let self_heal = optional_f32_option(
        context,
        row,
        &["SelfHealAdjustment"],
        "source.row",
        "SelfHealAdjustment",
    )?;
    let cooldown = optional_f32_option(
        context,
        row,
        &["CooldownAdjustment"],
        "source.row",
        "CooldownAdjustment",
    )?;
    let mut methods = String::new();
    for method in shape.methods() {
        let method_name = clean_ident(method.name().as_str());
        match method.parameter().kind() {
            NativeCrcIndexLookupParameterKind::Crc32
            | NativeCrcIndexLookupParameterKind::IntoCrc32 => {
                methods.push_str(&format!(
                    "    pub fn {method_name}(&self, id: Crc32) -> Option<&PvpBalanceData> {{ self.pvp_balances.get(*self.pvp_balances_by_id.get(&id)?) }}\n\n"
                ));
            }
            NativeCrcIndexLookupParameterKind::StrRef
            | NativeCrcIndexLookupParameterKind::AsRefStr => {
                methods.push_str(&format!(
                    "    pub fn {method_name}(&self, key: &str) -> Option<&PvpBalanceData> {{ self.pvp_balances.get(*self.pvp_balances_by_id.get(&Crc32::from_str_lower(key.trim()))?) }}\n\n"
                ));
            }
        }
    }
    let rows_method = shape
        .balances_method()
        .map(|name| clean_ident(name.as_str()))
        .unwrap_or_else(|| "balances".to_owned());
    methods.push_str(&format!(
        "    pub fn {rows_method}(&self) -> impl ExactSizeIterator<Item = &PvpBalanceData> {{ self.pvp_balances.iter() }}\n\n"
    ));
    if let Some(name) = shape.len_method() {
        let name = clean_ident(name.as_str());
        methods.push_str(&format!(
            "    pub fn {name}(&self) -> usize {{ self.pvp_balances.len() }}\n\n"
        ));
    }
    if let Some(name) = shape.is_empty_method() {
        let name = clean_ident(name.as_str());
        methods.push_str(&format!(
            "    pub fn {name}(&self) -> bool {{ self.pvp_balances.is_empty() }}\n\n"
        ));
    }
    methods.push_str("    pub fn balances_for_category(&self, category: Crc32) -> impl Iterator<Item = &PvpBalanceData> { self.pvp_balance_by_category.get(&category).into_iter().flatten().filter_map(|index| self.pvp_balances.get(*index)) }\n\n");
    Ok(RustNativeManagerAugmentation {
        declarations: "#[derive(Debug, Clone, PartialEq)]\npub struct PvpBalanceData {\n    pub source_row: usize,\n    pub target: String,\n    pub target_crc: Crc32,\n    pub category: String,\n    pub ability_base_damage: Option<String>,\n    pub affix_stat: Option<String>,\n    pub incoming_heal: Option<String>,\n    pub consumable_heal: Option<String>,\n    pub potency: Option<f32>,\n    pub duration: Option<f32>,\n    pub weapon_base_damage: f32,\n    pub self_heal: f32,\n    pub cooldown: f32,\n}\n\nfn pvp_balance_text(value: &str) -> Option<String> { let value = value.trim(); (!value.is_empty()).then(|| value.to_owned()) }\n\nfn pvp_balance_f32(value: Option<f32>, field: &str, target: &str) -> Result<Option<f32>> {\n    if value.is_some_and(|value| !value.is_finite()) { bail!(\"PvpBalanceData `{target}` has non-finite {field}\"); }\n    Ok(value)\n}\n\nfn pvp_balance_number_text(value: Option<f32>, field: &str, target: &str) -> Result<Option<String>> {\n    Ok(pvp_balance_f32(value, field, target)?.map(|value| value.to_string()))\n}\n\nfn pvp_balance_number(value: Option<&str>, field: &str, target: &str) -> Result<Option<f32>> {\n    let Some(value) = value.map(str::trim).filter(|value| !value.is_empty()) else { return Ok(None); };\n    let parsed = value.parse::<f32>().with_context(|| format!(\"PvpBalanceData `{target}` has invalid {field} `{value}`\"))?;\n    pvp_balance_f32(Some(parsed), field, target)\n}\n\n".to_owned(),
        fields: "    pvp_balances: Vec<PvpBalanceData>,\n    pvp_balances_by_id: HashMap<Crc32, usize>,\n    pvp_balance_by_category: HashMap<Crc32, Vec<usize>>,\n".to_owned(),
        field_values: "            pvp_balances,\n            pvp_balances_by_id,\n            pvp_balance_by_category,\n".to_owned(),
        initializers: format!(r#"        let mut pvp_balances = Vec::new();
        let mut pvp_balances_by_id = HashMap::new();
        let mut pvp_balance_by_category: HashMap<Crc32, Vec<usize>> = HashMap::new();
        for source in {row_field}.rows() {{
            let target = {target}.trim();
            let id = Crc32::from_str_lower(target);
            if target.is_empty() || id == Crc32::ZERO || pvp_balances_by_id.contains_key(&id) {{ continue; }}
            let category = {category}.trim();
            let potency = {potency};
            let duration = {duration};
            let weapon_base_damage = {weapon}.unwrap_or(0.0);
            let self_heal = {self_heal}.unwrap_or(0.0);
            let cooldown = {cooldown}.unwrap_or(0.0);
            let data = PvpBalanceData {{ source_row: source.slot.row_index(), target: target.to_owned(), target_crc: id, category: category.to_owned(), ability_base_damage: {ability}, affix_stat: {affix}, incoming_heal: {incoming}, consumable_heal: {consumable}, potency, duration, weapon_base_damage, self_heal, cooldown }};
            let index = pvp_balances.len();
            pvp_balances_by_id.insert(id, index);
            let category_id = Crc32::from_str_lower(category);
            if category_id != Crc32::ZERO {{ pvp_balance_by_category.entry(category_id).or_default().push(index); }}
            pvp_balances.push(data);
        }}
"#, row_field=parts.row_field),
        methods,
        rows_type: "PvpBalanceData".to_owned(),
        rows_method,
    })
}

fn qualified_pvp_balance(
    context: &AugmentationContext<'_>,
    shape: &crate::manager::NativeOneTablePvpBalanceManager,
) -> Result<RustNativeManagerAugmentation> {
    let mut augmentation = pvp_balance(context, shape)?;
    let data_type = context
        .manager
        .manager_class_name
        .strip_suffix("Manager")
        .unwrap_or(&context.manager.manager_class_name);
    for source in [
        &mut augmentation.declarations,
        &mut augmentation.fields,
        &mut augmentation.initializers,
        &mut augmentation.methods,
        &mut augmentation.rows_type,
    ] {
        *source = source.replace("PvpBalanceData", data_type);
    }
    Ok(augmentation)
}

fn dye_color(
    context: &AugmentationContext<'_>,
    shape: &crate::manager::NativeOneTableDyeColorManager,
) -> Result<RustNativeManagerAugmentation> {
    let row = context.row("DyeColorData")?;
    let parts = context.row_parts(row);
    let index = numeric_value_expression(context.field(row, "Index")?, "source.row")?;
    let name = optional_string(context, row, &["Name"], "source.row", "\"\"")?;
    let color = optional_string(context, row, &["Color"], "source.row", "\"\"")?;
    let category = optional_string(context, row, &["Category"], "source.row", "\"\"")?;
    let entitlement = optional_boolish(context, row, &["IsEntitlement"], "source.row", "false")?;
    let color_amount = optional_number(context, row, &["ColorAmount"], "source.row", "0.0")?;
    let color_override = optional_number(context, row, &["ColorOverride"], "source.row", "0.0")?;
    let spec_color = optional_string(context, row, &["SpecColor"], "source.row", "\"\"")?;
    let spec_amount = optional_number(context, row, &["SpecAmount"], "source.row", "0.0")?;
    let mask_gloss_shift = optional_number(context, row, &["MaskGlossShift"], "source.row", "0.0")?;
    let lookup = clean_ident(shape.lookup_method().as_str());
    let lookup_from_index = clean_ident(shape.lookup_from_index_method().as_str());
    let lookup_by_key = clean_ident(shape.lookup_by_key_method().as_str());
    let rows = clean_ident(shape.rows_method().as_str());
    let entitlement_indexes = clean_ident(shape.entitlement_indexes_method().as_str());
    let len = clean_ident(shape.len_method().as_str());
    let is_empty = clean_ident(shape.is_empty_method().as_str());
    Ok(RustNativeManagerAugmentation {
        declarations: "#[derive(Debug, Clone, Copy, PartialEq)]\npub struct DyeColorRgba { pub red: f32, pub green: f32, pub blue: f32, pub alpha: f32 }\n\n#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]\npub struct DyeColorIndex(core::num::NonZeroU8);\n\nimpl DyeColorIndex { pub const fn new(value: core::num::NonZeroU8) -> Self { Self(value) } pub fn from_u8(value: u8) -> Option<Self> { core::num::NonZeroU8::new(value).map(Self) } pub const fn get(self) -> core::num::NonZeroU8 { self.0 } }\n\n#[derive(Debug, Clone, PartialEq)]\npub struct DyeColorData {\n    pub index: DyeColorIndex,\n    pub name: String,\n    pub color: DyeColorRgba,\n    pub category: String,\n    pub is_entitlement: bool,\n    pub color_amount: f32,\n    pub color_override: f32,\n    pub spec_color: DyeColorRgba,\n    pub spec_amount: f32,\n    pub mask_gloss_shift: f32,\n}\n\nfn dye_color_rgba(value: &str, index: DyeColorIndex, field: &str) -> Result<DyeColorRgba> {\n    let value = value.trim().strip_prefix('#').unwrap_or(value.trim());\n    if value.is_empty() || value.len() > 8 { bail!(\"DyeColorData index {} has invalid {field} `{value}`\", index.get()); }\n    let mut padded = value.to_owned();\n    while padded.len() < 8 { padded.push('F'); }\n    let raw = u32::from_str_radix(&padded, 16).with_context(|| format!(\"DyeColorData index {} has invalid {field} `{value}`\", index.get()))?;\n    let channel = |shift: u32| ((raw >> shift) & 0xffu32) as f32 / 255.0;\n    Ok(DyeColorRgba { red: channel(24), green: channel(16), blue: channel(8), alpha: channel(0) })\n}\n\n".to_owned(),
        fields: "    dye_colors: Vec<DyeColorData>,\n    dye_colors_by_index: HashMap<DyeColorIndex, usize>,\n    dye_color_entitlement_indexes: Vec<DyeColorIndex>,\n".to_owned(),
        field_values: "            dye_colors,\n            dye_colors_by_index,\n            dye_color_entitlement_indexes,\n".to_owned(),
        initializers: format!(r#"        let mut dye_colors = Vec::new();
        let mut dye_colors_by_index = HashMap::new();
        for source in {row_field}.rows() {{
            let raw_index = {index};
            if !raw_index.is_finite() || raw_index.fract() != 0.0 || !(1.0..=u8::MAX as f32).contains(&raw_index) {{ bail!("DyeColorData has invalid Index `{{raw_index}}`"); }}
            let index = DyeColorIndex::from_u8(raw_index as u8).expect("validated non-zero dye index");
            if dye_colors_by_index.contains_key(&index) {{ continue; }}
            let color_text = {color}.trim();
            if color_text.is_empty() {{ continue; }}
            let color = dye_color_rgba(color_text, index, "Color")?;
            let spec_text = {spec_color}.trim();
            let spec_color = if spec_text.is_empty() {{ color }} else {{ dye_color_rgba(spec_text, index, "SpecColor")? }};
            let color_amount = {color_amount};
            let color_override = {color_override};
            let spec_amount = {spec_amount};
            let mask_gloss_shift = {mask_gloss_shift};
            for (field, value) in [("ColorAmount", color_amount), ("ColorOverride", color_override), ("SpecAmount", spec_amount), ("MaskGlossShift", mask_gloss_shift)] {{ if !value.is_finite() {{ bail!("DyeColorData index {{}} has non-finite {{field}}", index.get()); }} }}
            dye_colors_by_index.insert(index, dye_colors.len());
            dye_colors.push(DyeColorData {{ index, name: {name}.trim().to_owned(), color, category: {category}.trim().to_owned(), is_entitlement: {entitlement}, color_amount, color_override, spec_color, spec_amount, mask_gloss_shift }});
        }}
        let mut dye_color_entitlement_indexes = dye_colors.iter().filter(|color| color.is_entitlement).map(|color| color.index).collect::<Vec<_>>();
        dye_color_entitlement_indexes.sort_unstable();
"#, row_field=parts.row_field),
        methods: format!("    pub fn {lookup}(&self, index: DyeColorIndex) -> Option<&DyeColorData> {{ self.dye_colors.get(*self.dye_colors_by_index.get(&index)?) }}\n\n    pub fn {lookup_from_index}(&self, index: core::num::NonZeroU8) -> Option<&DyeColorData> {{ self.{lookup}(DyeColorIndex::new(index)) }}\n\n    pub fn {lookup_by_key}(&self, index: DyeColorIndex) -> Option<&DyeColorData> {{ self.{lookup}(index) }}\n\n    pub fn {rows}(&self) -> impl ExactSizeIterator<Item = &DyeColorData> {{ self.dye_colors.iter() }}\n\n    pub fn {entitlement_indexes}(&self) -> &[DyeColorIndex] {{ &self.dye_color_entitlement_indexes }}\n\n    pub fn {len}(&self) -> usize {{ self.dye_colors.len() }}\n\n    pub fn {is_empty}(&self) -> bool {{ self.dye_colors.is_empty() }}\n\n"),
        rows_type: "DyeColorData".to_owned(),
        rows_method: rows,
    })
}

#[derive(Default)]
struct RewardTrackSlotFields<'a> {
    bucket: Option<&'a RustStandaloneSchemaField>,
    reward: Option<&'a RustStandaloneSchemaField>,
    tag: Option<&'a RustStandaloneSchemaField>,
    match_one: Option<&'a RustStandaloneSchemaField>,
    random_weight: Option<&'a RustStandaloneSchemaField>,
    budget_contribution: Option<&'a RustStandaloneSchemaField>,
    reward_type: Option<&'a RustStandaloneSchemaField>,
    stage_exclusion: Option<&'a RustStandaloneSchemaField>,
    shop_exclusion: Option<&'a RustStandaloneSchemaField>,
}

fn reward_track(context: &AugmentationContext<'_>) -> Result<RustNativeManagerAugmentation> {
    let row = context.row("PvPStoreData")?;
    let parts = context.row_parts(row);
    let mut slots = std::collections::BTreeMap::<u16, RewardTrackSlotFields<'_>>::new();
    for field in &row.fields {
        if let Some(slot) = numbered_suffix(&field.source_name, "Bucket") {
            slots.entry(slot).or_default().bucket = Some(field);
        }
        if let Some(slot) = numbered_suffix(&field.source_name, "RewardID") {
            slots.entry(slot).or_default().reward = Some(field);
        }
        if let Some(slot) = numbered_suffix(&field.source_name, "Tag") {
            slots.entry(slot).or_default().tag = Some(field);
        }
        if let Some(slot) = numbered_suffix(&field.source_name, "MatchOne") {
            slots.entry(slot).or_default().match_one = Some(field);
        }
        if let Some(slot) = numbered_suffix(&field.source_name, "RandomWeights") {
            slots.entry(slot).or_default().random_weight = Some(field);
        }
        if let Some(slot) = numbered_suffix(&field.source_name, "BudgetContribution") {
            slots.entry(slot).or_default().budget_contribution = Some(field);
        }
        if let Some(slot) = numbered_suffix(&field.source_name, "Type") {
            slots.entry(slot).or_default().reward_type = Some(field);
        }
        if let Some(slot) = numbered_suffix(&field.source_name, "ExcludeTypeStage") {
            slots.entry(slot).or_default().stage_exclusion = Some(field);
        }
        if let Some(slot) = numbered_suffix(&field.source_name, "ExcludeTypeShop") {
            slots.entry(slot).or_default().shop_exclusion = Some(field);
        }
    }
    let mut init = String::new();
    for (slot, fields) in slots {
        let (Some(bucket), Some(reward)) = (fields.bucket, fields.reward) else {
            continue;
        };
        let bucket = string_value_expression(bucket, "source.row")?;
        let reward = string_value_expression(reward, "source.row")?;
        let tag = optional_field_string(fields.tag, "source.row", "\"\"")?;
        let match_one = optional_field_string(fields.match_one, "source.row", "\"\"")?;
        let random_weight = optional_field_number(fields.random_weight, "source.row", "0.0")?;
        let budget = optional_field_number(fields.budget_contribution, "source.row", "0.0")?;
        let reward_type = optional_field_string(fields.reward_type, "source.row", "\"\"")?;
        let stage = optional_field_string(fields.stage_exclusion, "source.row", "\"\"")?;
        let shop = optional_field_string(fields.shop_exclusion, "source.row", "\"\"")?;
        init.push_str(&format!(r#"            {{
                let slot_key = RewardTrackSlot {{ table: *source.reference.table(), slot: {slot} }};
                if source.slot.row_index() == 0 && !reward_tracks_by_slot.contains_key(&slot_key) {{
                    let key = {bucket}.trim();
                    let id = Crc32::from_str_lower(key);
                    if !key.is_empty() && id != Crc32::ZERO {{
                        let index = reward_tracks.len();
                        reward_tracks.push(RewardTrackData {{ table: *source.reference.table(), slot: {slot}, key: key.to_owned(), id, entries: Vec::new() }});
                        reward_tracks_by_slot.insert(slot_key, index);
                        reward_tracks_by_id.entry(id).or_insert(index);
                    }}
                }}
                let Some(track_index) = reward_tracks_by_slot.get(&slot_key).copied() else {{ continue; }};
                let reward_key = {reward}.trim();
                let reward_id = Crc32::from_str_lower(reward_key);
                if reward_key.is_empty() || reward_id == Crc32::ZERO {{ continue; }}
                let random_weight = reward_track_u32({random_weight}, "RandomWeights", reward_key)?;
                let budget_contribution = reward_track_u32({budget}, "BudgetContribution", reward_key)?;
                reward_tracks[track_index].entries.push(RewardTrackEntry {{
                    source_slot: {slot},
                    source_row: source.slot.row_index(),
                    reward_key: reward_key.to_owned(),
                    reward_id,
                    reward_type: reward_track_crc({reward_type}),
                    tag_constraints: reward_track_tags({tag}),
                    match_one: reward_track_bool({match_one}),
                    select_once: true,
                    random_weight,
                    budget_contribution,
                    stage_exclusion: reward_track_crc({stage}),
                    shop_exclusion: reward_track_crc({shop}),
                }});
            }}
"#));
    }
    Ok(RustNativeManagerAugmentation {
        declarations: format!(r#"#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct RewardTrackSlot {{ pub table: {table}, pub slot: u16 }}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RewardTrackTagConstraint {{ pub tag: Crc32, pub range: core::ops::RangeInclusive<u16> }}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RewardTrackEntry {{
    pub source_slot: u16,
    pub source_row: usize,
    pub reward_key: String,
    pub reward_id: Crc32,
    pub reward_type: Option<Crc32>,
    pub tag_constraints: Vec<RewardTrackTagConstraint>,
    pub match_one: bool,
    pub select_once: bool,
    pub random_weight: u32,
    pub budget_contribution: u32,
    pub stage_exclusion: Option<Crc32>,
    pub shop_exclusion: Option<Crc32>,
}}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RewardTrackData {{ pub table: {table}, pub slot: u16, pub key: String, pub id: Crc32, pub entries: Vec<RewardTrackEntry> }}

fn reward_track_crc(value: &str) -> Option<Crc32> {{ let value = value.trim(); let id = Crc32::from_str_lower(value); (!value.is_empty() && id != Crc32::ZERO).then_some(id) }}
fn reward_track_bool(value: &str) -> bool {{ matches!(value.trim().to_ascii_lowercase().as_str(), "true" | "1" | "yes") }}
fn reward_track_u32(value: f32, field: &str, reward: &str) -> Result<u32> {{
    if !value.is_finite() || value.fract() != 0.0 || value < 0.0 || value >= 4_294_967_296.0 {{ bail!("RewardTrackData reward `{{reward}}` has invalid {{field}} `{{value}}`"); }}
    Ok(value as u32)
}}
fn reward_track_tags(value: &str) -> Vec<RewardTrackTagConstraint> {{
    value.split(',').filter_map(|token| {{
        let token = token.trim();
        if token.is_empty() {{ return None; }}
        let (tag, range) = token.split_once(':').map_or((token, ""), |(tag, range)| (tag.trim(), range.trim()));
        let id = Crc32::from_str_lower(tag);
        if tag.is_empty() || id == Crc32::ZERO {{ return None; }}
        let mut parts = range.split('-').map(str::trim);
        let start = reward_track_u16(parts.next().unwrap_or_default());
        let end = parts.next().map(reward_track_u16).unwrap_or(10_000);
        Some(RewardTrackTagConstraint {{ tag: id, range: start.min(end)..=start.max(end) }})
    }}).collect()
}}
fn reward_track_u16(value: &str) -> u16 {{ value.parse::<i64>().ok().and_then(|value| u16::try_from(value).ok()).or_else(|| value.parse::<f32>().ok().filter(|value| value.is_finite() && *value >= 0.0 && *value <= u16::MAX as f32).map(|value| value.trunc() as u16)).unwrap_or(0) }}

"#, table=parts.table_type),
        fields: "    reward_tracks: Vec<RewardTrackData>,\n    reward_tracks_by_id: HashMap<Crc32, usize>,\n    reward_tracks_by_slot: HashMap<RewardTrackSlot, usize>,\n".to_owned(),
        field_values: "            reward_tracks,\n            reward_tracks_by_id,\n            reward_tracks_by_slot,\n".to_owned(),
        initializers: format!("        let mut reward_tracks = Vec::new();\n        let mut reward_tracks_by_id = HashMap::new();\n        let mut reward_tracks_by_slot = HashMap::new();\n        for source in {}.rows() {{\n{init}        }}\n", parts.row_field),
        methods: "    pub fn reward_track_data_from_id(&self, id: Crc32) -> Option<&RewardTrackData> { self.reward_tracks.get(*self.reward_tracks_by_id.get(&id)?) }\n\n    pub fn reward_track_data(&self, key: &str) -> Option<&RewardTrackData> { self.reward_track_data_from_id(Crc32::from_str_lower(key.trim())) }\n\n    pub fn reward_track_data_by_key(&self, key: &str) -> Option<&RewardTrackData> { self.reward_track_data(key) }\n\n    pub fn reward_track_for_slot(&self, slot: RewardTrackSlot) -> Option<&RewardTrackData> { self.reward_tracks.get(*self.reward_tracks_by_slot.get(&slot)?) }\n\n    pub fn reward_tracks(&self) -> impl ExactSizeIterator<Item = &RewardTrackData> { self.reward_tracks.iter() }\n\n".to_owned(),
        rows_type: "RewardTrackData".to_owned(),
        rows_method: "reward_tracks".to_owned(),
    })
}

fn whisper(context: &AugmentationContext<'_>) -> Result<RustNativeManagerAugmentation> {
    Ok(RustNativeManagerAugmentation::merged([
        crc_rows(
            context,
            "WhisperData",
            "WhisperId",
            "whispers",
            &["whisper_data_from_id"],
            &["whisper_data", "whisper_data_by_key"],
            Some("whispers"),
        )?,
        crc_rows(
            context,
            "WhisperVfxData",
            "WhisperVfxId",
            "whisper_vfx",
            &["whisper_vfx_from_id"],
            &["whisper_vfx"],
            Some("whisper_vfx_rows"),
        )?,
    ]))
}

fn dungeon_tile(context: &AugmentationContext<'_>) -> Result<RustNativeManagerAugmentation> {
    let mut value = crc_rows(
        context,
        "DungeonTileStaticData",
        "DungeonTileId",
        "dungeon_tiles",
        &["dungeon_tile_static_data"],
        &["dungeon_tile_static_data_by_key"],
        Some("dungeon_tiles"),
    )?;
    let row = context.row("DungeonTileStaticData")?;
    let parts = context.row_parts(row);
    let feature = optional_string(context, row, &["FeatureId"], "source.row", "\"\"")?;
    value.fields.push_str(&format!(
        "    dungeon_tiles_by_feature: HashMap<Crc32, Vec<RowRef<{}, {}>>>,\n",
        parts.table_type, parts.row_type
    ));
    value
        .field_values
        .push_str("            dungeon_tiles_by_feature,\n");
    value.initializers.push_str(&format!("        let mut dungeon_tiles_by_feature:HashMap<Crc32,Vec<RowRef<{table},{row}>>>=HashMap::new();\n        for source in {field}.rows(){{let id=Crc32::from_str_lower({feature}.trim());if id!=Crc32::ZERO{{dungeon_tiles_by_feature.entry(id).or_default().push(source.reference.clone());}}}}\n",table=parts.table_type,row=parts.row_type,field=parts.row_field));
    value.methods.push_str(&format!("    pub fn tiles_for_feature(&self,id:Crc32)->impl Iterator<Item=&{row}>+'_{{self.dungeon_tiles_by_feature.get(&id).into_iter().flatten().filter_map(|reference|self.{field}.get(reference))}}\n\n",row=parts.row_type,field=parts.row_field));
    Ok(value)
}

fn level_disparity(context: &AugmentationContext<'_>) -> Result<RustNativeManagerAugmentation> {
    let mut value = numeric_rows(
        context,
        "LevelDisparityData",
        "LevelDisparity",
        "level_disparity",
        NumericStorage::I32,
        &["level_disparity_data"],
        Some("level_disparity_rows"),
    )?;
    value
        .fields
        .push_str("    level_disparity_range: Option<(i32,i32)>,\n");
    value
        .field_values
        .push_str("            level_disparity_range,\n");
    value.initializers.push_str("        let level_disparity_range = level_disparity_by_number.keys().copied().min().zip(level_disparity_by_number.keys().copied().max());\n");
    let row_type = context.row("LevelDisparityData")?.type_name.as_str();
    value.methods.push_str(&format!("    pub fn level_disparity_data_for_levels(&self,player_level:i32,target_level:i32)->Option<&{row_type}>{{self.level_disparity_data(target_level-player_level)}}\n    pub fn clamped_disparity(&self,value:i32)->Option<i32>{{let(min,max)=self.level_disparity_range?;Some(value.clamp(min,max))}}\n    pub fn clamped_level_disparity_data_for_levels(&self,player_level:i32,target_level:i32)->Option<&{row_type}>{{self.level_disparity_data(self.clamped_disparity(target_level-player_level)?)}}\n    pub fn loaded_range(&self)->Option<(i32,i32)>{{self.level_disparity_range}}\n\n"));
    Ok(value)
}

fn difficulty_scaling(context: &AugmentationContext<'_>) -> Result<RustNativeManagerAugmentation> {
    let mut value = crc_rows(
        context,
        "DifficultyScalingData",
        "WorldEncounterID",
        "difficulty_scaling",
        &["difficulty_scaling_data_from_id"],
        &["difficulty_scaling_data", "difficulty_scaling_data_by_key"],
        Some("difficulty_scaling_rows"),
    )?;
    let row = context.row("DifficultyScalingData")?;
    let parts = context.row_parts(row);
    let creatures = optional_string(
        context,
        row,
        &["AffectedCreatureTypes"],
        "source.row",
        "\"all\"",
    )?;
    value
        .fields
        .push_str("    difficulty_scaling_creatures: HashMap<Crc32, Vec<Crc32>>,\n");
    value
        .field_values
        .push_str("            difficulty_scaling_creatures,\n");
    value.initializers.push_str(&format!("        let mut difficulty_scaling_creatures=HashMap::new();\n        for source in {field}.rows(){{let Some((id,_))=difficulty_scaling_by_id.iter().find(|(_,reference)|*reference==&source.reference)else{{continue;}};let values=split_designer_list({creatures}).into_iter().filter(|value|!value.eq_ignore_ascii_case(\"all\")).map(Crc32::from_str_lower).filter(|id|*id!=Crc32::ZERO).collect();difficulty_scaling_creatures.insert(*id,values);}}\n",field=parts.row_field));
    value.methods.push_str("    pub fn affected_creature_types(&self,id:Crc32)->&[Crc32]{self.difficulty_scaling_creatures.get(&id).map(Vec::as_slice).unwrap_or_default()}\n\n");
    add_designer_list_helper(&mut value);
    Ok(value)
}

fn particle(context: &AugmentationContext<'_>) -> Result<RustNativeManagerAugmentation> {
    let mut value = crc_rows(
        context,
        "ParticleData",
        "Effect Name",
        "particles",
        &["particle_data_from_id"],
        &["particle_data", "particle_data_by_key"],
        Some("particles"),
    )?;
    let row = context.row("ParticleData")?;
    let parts = context.row_parts(row);
    let enabled = optional_bool(
        context,
        row,
        &["IsEnabled", "Enabled"],
        "source.row",
        "true",
    )?;
    value.initializers = value.initializers.replace(
        "            let text =",
        &format!("            if !({enabled}){{continue;}}\n            let text ="),
    );
    let row_enabled = optional_bool(context, row, &["IsEnabled", "Enabled"], "row", "true")?;
    let enabled_rows = if row_enabled == "true" {
        format!("self.{}.rows().map(|source| &source.row)", parts.row_field)
    } else {
        format!(
            "self.{}.rows().map(|source| &source.row).filter(|row| {row_enabled})",
            parts.row_field
        )
    };
    value.methods.push_str(&format!(
        "    pub fn enabled_particles(&self) -> impl Iterator<Item = &{}> + '_ {{ {enabled_rows} }}\n\n",
        parts.row_type
    ));
    Ok(value)
}

fn character_attribute(context: &AugmentationContext<'_>) -> Result<RustNativeManagerAugmentation> {
    let row = context.row("AttributeDefinition")?;
    let parts = context.row_parts(row);
    let level = context.field(row, "Level")?;
    let level_value = numeric_value_expression(level, "source.row")?;
    let key_type = format!("{}AttributeKey", context.manager.manager_class_name);
    Ok(RustNativeManagerAugmentation {
        declarations: format!(
            "#[derive(Debug,Clone,Copy,PartialEq,Eq,Hash)]\nstruct {key_type}{{table:{},level:u32}}\n\n",
            parts.table_type
        ),
        fields: format!(
            "    attributes_by_level:HashMap<{key_type},RowRef<{},{}>>,\n    attribute_levels:HashMap<{},Vec<u32>>,\n",
            parts.table_type, parts.row_type, parts.table_type
        ),
        field_values: "            attributes_by_level,\n            attribute_levels,\n"
            .to_owned(),
        initializers: format!(
            "        let mut attributes_by_level=HashMap::new();let mut attribute_levels:HashMap<{},Vec<u32>>=HashMap::new();\n        for source in {}.rows(){{let raw={level_value};if !raw.is_finite()||raw.fract()!=0.0||raw<0.0||raw>u32::MAX as f32{{continue;}}let level=raw as u32;let table=*source.reference.table();attributes_by_level.entry({key_type}{{table,level}}).or_insert_with(||source.reference.clone());attribute_levels.entry(table).or_default().push(level);}}\n        for levels in attribute_levels.values_mut(){{levels.sort_unstable();levels.dedup();}}\n",
            parts.table_type, parts.row_field
        ),
        methods: format!(
            "    pub fn attribute_data(&self,table:{table},level:u32)->Option<&{row}>{{self.{field}.get(self.attributes_by_level.get(&{key_type}{{table,level}})?)}}\n    pub fn clamped_level(&self,table:{table},level:u32)->Option<u32>{{let levels=self.attribute_levels.get(&table)?;match levels.binary_search(&level){{Ok(index)=>Some(levels[index]),Err(0)=>levels.first().copied(),Err(index)=>levels.get(index-1).copied()}}}}\n    pub fn clamped_attribute_data(&self,table:{table},level:u32)->Option<&{row}>{{self.attribute_data(table,self.clamped_level(table,level)?)}}\n\n",
            table = parts.table_type,
            row = parts.row_type,
            field = parts.row_field
        ),
        ..RustNativeManagerAugmentation::default()
    })
}

fn governance(context: &AugmentationContext<'_>) -> Result<RustNativeManagerAugmentation> {
    let mut value = numeric_rows(
        context,
        "TerritoryUpkeepDefinition",
        "Level",
        "governance",
        NumericStorage::U32,
        &["governance"],
        Some("governance_rows"),
    )?;
    let row = context.row("TerritoryUpkeepDefinition")?;
    let parts = context.row_parts(row);
    let distributions = row
        .fields
        .iter()
        .filter_map(|field| {
            field
                .source_name
                .strip_prefix("EarningsDistributionTID")?
                .parse::<u32>()
                .ok()
                .map(|id| (id, field))
        })
        .map(|(id, field)| {
            numeric_value_or_default_expression(field, "source.row", "0.0").map(|expr| (id, expr))
        })
        .collect::<Result<Vec<_>>>()?;
    let inserts=distributions.iter().map(|(id,expr)|format!("            governance_distribution.entry(level).or_default().push(TerritoryEarningsDistribution{{territory_id:{id},share:{expr}}});\n")).collect::<String>();
    let max = distributions.iter().map(|(id, _)| *id).max().unwrap_or(0);
    value.declarations.push_str("#[derive(Debug,Clone,Copy,PartialEq)]\npub struct TerritoryEarningsDistribution{pub territory_id:u32,pub share:f32}\n\n");
    value.fields.push_str("    governance_distribution:HashMap<u32,Vec<TerritoryEarningsDistribution>>,\n    max_territory_id:u32,\n");
    value
        .field_values
        .push_str("            governance_distribution,\n            max_territory_id,\n");
    value.initializers.push_str(&format!("        let mut governance_distribution:HashMap<u32,Vec<TerritoryEarningsDistribution>>=HashMap::new();\n        for source in {}.rows(){{let raw=source.row.level;if !raw.is_finite()||raw.fract()!=0.0||raw<0.0{{continue;}}let level=raw as u32;\n{inserts}        }}\n        let max_territory_id={max};\n",parts.row_field));
    value.methods.push_str("    pub fn territory_earnings_distribution(&self,level:u32)->impl Iterator<Item=TerritoryEarningsDistribution>+'_ {self.governance_distribution.get(&level).into_iter().flatten().copied()}\n    pub fn max_territory_id(&self)->u32{self.max_territory_id}\n\n");
    Ok(value)
}

fn loot_bucket(context: &AugmentationContext<'_>) -> Result<RustNativeManagerAugmentation> {
    let row = context.row("LootBucketData")?;
    let parts = context.row_parts(row);
    Ok(RustNativeManagerAugmentation {
        declarations: format!(
            r#"#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct LootBucketSlot {{ pub table: {table}, pub slot: u16 }}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LootBucketTag {{ pub key: String, pub id: Crc32, pub range: Option<core::ops::RangeInclusive<u16>> }}

#[derive(Debug, Clone, PartialEq)]
pub struct LootBucketEntry {{
    pub source_row: usize,
    pub item_key: String,
    pub item_id: Crc32,
    pub tags: Vec<LootBucketTag>,
    pub match_one: bool,
    pub quantity: core::ops::RangeInclusive<u16>,
    pub odds: f32,
}}

#[derive(Debug, Clone, PartialEq)]
pub struct LootBucketData {{
    pub table: {table},
    pub slot: u16,
    pub key: String,
    pub id: Crc32,
    pub loot_biasing_disabled: bool,
    pub entries: Vec<LootBucketEntry>,
}}

fn loot_bucket_bool(value: Option<&str>) -> bool {{ matches!(value.unwrap_or_default().trim().to_ascii_lowercase().as_str(), "true" | "1" | "yes") }}
fn loot_bucket_odds(value: Option<&str>) -> f32 {{ value.and_then(|value| value.trim().parse::<f32>().ok()).filter(|value| value.is_finite()).unwrap_or(1.0) }}
fn loot_bucket_quantity(value: Option<&str>) -> core::ops::RangeInclusive<u16> {{
    let Some(value) = value.map(str::trim).filter(|value| !value.is_empty()) else {{ return 0..=0; }};
    let mut parts = value.split('-').map(str::trim);
    let start = loot_bucket_u16(parts.next().unwrap_or_default());
    let end = parts.next().map(loot_bucket_u16).unwrap_or(start);
    start.min(end)..=start.max(end)
}}
fn loot_bucket_tags(value: Option<&str>) -> Vec<LootBucketTag> {{
    value.unwrap_or_default().split(',').filter_map(|token| {{
        let token = token.trim();
        if token.is_empty() {{ return None; }}
        let (key, range) = match token.split_once(':') {{ Some((key, range)) => (key.trim(), Some(loot_bucket_tag_range(range))), None => (token, None) }};
        let id = Crc32::from_str_lower(key);
        (!key.is_empty() && id != Crc32::ZERO).then(|| LootBucketTag {{ key: key.to_owned(), id, range }})
    }}).collect()
}}
fn loot_bucket_tag_range(value: &str) -> core::ops::RangeInclusive<u16> {{
    let mut parts = value.split('-').map(str::trim);
    let start = loot_bucket_u16(parts.next().unwrap_or_default());
    let end = parts.next().map(loot_bucket_u16).unwrap_or(10_000);
    start.min(end)..=start.max(end)
}}
fn loot_bucket_u16(value: &str) -> u16 {{ value.parse::<i64>().ok().and_then(|value| u16::try_from(value).ok()).or_else(|| value.parse::<f32>().ok().filter(|value| value.is_finite() && *value >= 0.0 && *value <= u16::MAX as f32).map(|value| value.trunc() as u16)).unwrap_or(0) }}

"#,
            table = parts.table_type
        ),
        fields: "    loot_buckets: Vec<LootBucketData>,\n    loot_buckets_by_id: HashMap<Crc32, usize>,\n    loot_buckets_by_slot: HashMap<LootBucketSlot, usize>,\n".to_owned(),
        field_values: "            loot_buckets,\n            loot_buckets_by_id,\n            loot_buckets_by_slot,\n".to_owned(),
        initializers: format!(r#"        let mut loot_buckets: Vec<LootBucketData> = Vec::new();
        let mut loot_buckets_by_id: HashMap<Crc32, usize> = HashMap::new();
        let mut loot_buckets_by_slot: HashMap<LootBucketSlot, usize> = HashMap::new();
        for source in {field}.rows() {{
            for entry in &source.row.entries {{
                let slot = LootBucketSlot {{ table: *source.reference.table(), slot: entry.slot }};
                if source.slot.row_index() == 0 {{
                    if let Some(key) = entry.loot_bucket.as_deref().map(str::trim).filter(|key| !key.is_empty()) {{
                        let id = Crc32::from_str_lower(key);
                        if id != Crc32::ZERO {{
                            let loot_biasing_disabled = source.row.loot_biasing_disabled.iter().find(|value| value.slot == entry.slot).is_some_and(|value| value.disabled);
                            let data = LootBucketData {{ table: *source.reference.table(), slot: entry.slot, key: key.to_owned(), id, loot_biasing_disabled, entries: Vec::new() }};
                            let index = if let Some(index) = loot_buckets_by_id.get(&id).copied() {{
                                let previous = LootBucketSlot {{ table: loot_buckets[index].table, slot: loot_buckets[index].slot }};
                                loot_buckets_by_slot.remove(&previous);
                                loot_buckets[index] = data;
                                index
                            }} else {{
                                let index = loot_buckets.len();
                                loot_buckets.push(data);
                                loot_buckets_by_id.insert(id, index);
                                index
                            }};
                            loot_buckets_by_slot.insert(slot, index);
                        }}
                    }}
                }}
                let Some(bucket_index) = loot_buckets_by_slot.get(&slot).copied() else {{ continue; }};
                let Some(items) = entry.item.as_deref() else {{ continue; }};
                for item_key in items.split([',', '+']).map(str::trim).filter(|item| !item.is_empty()) {{
                    let item_id = Crc32::from_str_lower(item_key);
                    if item_id == Crc32::ZERO {{ continue; }}
                    loot_buckets[bucket_index].entries.push(LootBucketEntry {{ source_row: source.slot.row_index(), item_key: item_key.to_owned(), item_id, tags: loot_bucket_tags(entry.tags.as_deref()), match_one: loot_bucket_bool(entry.match_one.as_deref()), quantity: loot_bucket_quantity(entry.quantity.as_deref()), odds: loot_bucket_odds(entry.odds.as_deref()) }});
                }}
            }}
        }}
"#,field=parts.row_field),
        methods: "    pub fn loot_bucket_data_from_id(&self, id: Crc32) -> Option<&LootBucketData> { self.loot_buckets.get(*self.loot_buckets_by_id.get(&id)?) }\n\n    pub fn loot_bucket_data(&self, key: &str) -> Option<&LootBucketData> { self.loot_bucket_data_from_id(Crc32::from_str_lower(key.trim())) }\n\n    pub fn loot_bucket_data_by_key(&self, key: &str) -> Option<&LootBucketData> { self.loot_bucket_data(key) }\n\n    pub fn loot_bucket_for_slot(&self, slot: LootBucketSlot) -> Option<&LootBucketData> { self.loot_buckets.get(*self.loot_buckets_by_slot.get(&slot)?) }\n\n    pub fn buckets(&self) -> impl ExactSizeIterator<Item = &LootBucketData> { self.loot_buckets.iter() }\n\n".to_owned(),
        rows_type: "LootBucketData".to_owned(),
        rows_method: "buckets".to_owned(),
    })
}

fn territory(context: &AugmentationContext<'_>) -> Result<RustNativeManagerAugmentation> {
    let row = context.row("TerritoryDefinition")?;
    let parts = context.row_parts(row);
    let achievements = [
        "DiscoveredAchievement",
        "ChartedAchievement",
        "POIObjectiveAchievementId",
    ]
    .into_iter()
    .map(|name| optional_string(context, row, &[name], "source.row", "\"\""))
    .collect::<Result<Vec<_>>>()?;
    let tags = ["POITag", "POITags", "LootTags"]
        .into_iter()
        .map(|name| optional_string(context, row, &[name], "source.row", "\"\""))
        .collect::<Result<Vec<_>>>()?;
    let achievement_insert = achievements.iter().map(|achievement|format!("            for key in split_designer_list({achievement}){{let id=Crc32::from_str_lower(key);if id!=Crc32::ZERO{{territories_by_achievement.entry(id).or_insert(territory_id);}}}}\n")).collect::<String>();
    let tag_insert=tags.iter().map(|tags|format!("            for tag in split_designer_list({tags}){{let id=Crc32::from_str_lower(tag);if id!=Crc32::ZERO{{territories_by_tag.entry(id).or_default().push(source.reference.clone());}}}}\n")).collect::<String>();
    let mut value=RustNativeManagerAugmentation{
        fields:format!("    territories_by_id:HashMap<u16,RowRef<{table},{row}>>,\n    territories_by_label:HashMap<Crc32,u16>,\n    territories_by_achievement:HashMap<Crc32,u16>,\n    territories_by_tag:HashMap<Crc32,Vec<RowRef<{table},{row}>>>,\n",table=parts.table_type,row=parts.row_type),
        field_values:"            territories_by_id,\n            territories_by_label,\n            territories_by_achievement,\n            territories_by_tag,\n".to_owned(),
        initializers:format!("        let mut territories_by_id=HashMap::new();let mut territories_by_label=HashMap::new();let mut territories_by_achievement=HashMap::new();let mut territories_by_tag:HashMap<Crc32,Vec<RowRef<{table},{row}>>>=HashMap::new();\n        for source in {field}.rows(){{let Ok(raw_id)=source.reference.key().trim().parse::<i64>() else{{continue;}};let territory_id=raw_id as u16;if territory_id==0{{continue;}};territories_by_id.insert(territory_id,source.reference.clone());territories_by_label.insert(Crc32::from_str_lower(&format!(\"Territory_{{territory_id}}\")),territory_id);\n{achievement_insert}{tag_insert}        }}\n",table=parts.table_type,row=parts.row_type,field=parts.row_field),
        methods:format!("    pub fn by_id(&self,id:u16)->Option<&{row}>{{self.{field}.get(self.territories_by_id.get(&id)?)}}\n    pub fn by_label(&self,label:&str)->Option<&{row}>{{self.by_id(*self.territories_by_label.get(&Crc32::from_str_lower(label))?)}}\n    pub fn territory_id_for_achievement(&self,id:Crc32)->Option<u16>{{self.territories_by_achievement.get(&id).copied()}}\n    pub fn territory_for_achievement(&self,id:Crc32)->Option<&{row}>{{self.by_id(self.territory_id_for_achievement(id)?)}}\n    pub fn territories_with_tag(&self,id:Crc32)->impl Iterator<Item=&{row}>+'_{{self.territories_by_tag.get(&id).into_iter().flatten().filter_map(|reference|self.{field}.get(reference))}}\n    pub fn territories(&self)->impl Iterator<Item=&{row}>+'_{{self.{field}.rows().map(|source|&source.row)}}\n\n",row=parts.row_type,field=parts.row_field),
        ..RustNativeManagerAugmentation::default()
    };
    add_designer_list_helper(&mut value);
    Ok(value)
}

fn stat_modifier(context: &AugmentationContext<'_>) -> Result<RustNativeManagerAugmentation> {
    let mut values = Vec::new();
    for row in context.rows {
        if let Some(key) = [
            "StatusID",
            "ItemID",
            "WeaponID",
            "ConsumableID",
            "VitalsID",
            "Id",
        ]
        .into_iter()
        .find(|candidate| {
            row.fields
                .iter()
                .any(|field| field.source_name.eq_ignore_ascii_case(candidate))
        }) {
            let stem = clean_ident(row.source_row_type.trim_end_matches("Data"));
            let by_id = format!("{stem}_from_id");
            let by_key = format!("{stem}_by_key");
            let rows = format!("{stem}_rows");
            values.push(crc_rows(
                context,
                &row.source_row_type,
                key,
                &stem,
                &[&by_id],
                &[&by_key],
                Some(&rows),
            )?);
        }
    }
    Ok(RustNativeManagerAugmentation::merged(values))
}

fn status_effect(context: &AugmentationContext<'_>) -> Result<RustNativeManagerAugmentation> {
    let mut value = crc_rows(
        context,
        "StatusEffectData",
        "StatusID",
        "status_effects",
        &[
            "status_effect_data_from_id",
            "status_effect_data_by_id",
            "try_status_effect_data_by_id",
        ],
        &[
            "status_effect_data_by_name",
            "try_status_effect_data_by_name",
        ],
        Some("status_effects"),
    )?;
    let row = context.row("StatusEffectData")?;
    let parts = context.row_parts(row);
    let categories = optional_string(
        context,
        row,
        &["EffectCategories", "EffectCategory"],
        "source.row",
        "\"\"",
    )?;
    value
        .fields
        .push_str("    status_effect_categories:HashMap<Crc32,Vec<Crc32>>,\n");
    value
        .field_values
        .push_str("            status_effect_categories,\n");
    value.initializers.push_str(&format!("        let mut status_effect_categories=HashMap::new();\n        for source in {field}.rows(){{let Some((id,_))=status_effects_by_id.iter().find(|(_,reference)|*reference==&source.reference)else{{continue;}};let values=split_designer_list({categories}).into_iter().map(Crc32::from_str_lower).filter(|id|*id!=Crc32::ZERO).collect();status_effect_categories.insert(*id,values);}}\n",field=parts.row_field));
    value.methods.push_str("    pub fn status_effect_category_ids(&self,id:Crc32)->&[Crc32]{self.status_effect_categories.get(&id).map(Vec::as_slice).unwrap_or_default()}\n\n");
    add_designer_list_helper(&mut value);
    Ok(value)
}

fn item_conversion(context: &AugmentationContext<'_>) -> Result<RustNativeManagerAugmentation> {
    let mut value = crc_rows(
        context,
        "ItemCurrencyConversionData",
        "ConversionID",
        "item_conversions",
        &["get"],
        &["by_id"],
        Some("item_conversions"),
    )?;
    let row = context.row("ItemCurrencyConversionData")?;
    let parts = context.row_parts(row);
    let item = optional_string(context, row, &["ItemID"], "source.row", "\"\"")?;
    value.fields.push_str(&format!(
        "    item_conversions_by_item:HashMap<Crc32,Vec<RowRef<{},{}>>>,\n",
        parts.table_type, parts.row_type
    ));
    value
        .field_values
        .push_str("            item_conversions_by_item,\n");
    value.initializers.push_str(&format!("        let mut item_conversions_by_item:HashMap<Crc32,Vec<RowRef<{table},{row}>>>=HashMap::new();\n        for source in {field}.rows(){{let id=Crc32::from_str_lower({item}.trim());if id!=Crc32::ZERO{{item_conversions_by_item.entry(id).or_default().push(source.reference.clone());}}}}\n",table=parts.table_type,row=parts.row_type,field=parts.row_field));
    value.methods.push_str(&format!("    pub fn conversions_for_item(&self,id:Crc32)->impl Iterator<Item=&{row}>+'_{{self.item_conversions_by_item.get(&id).into_iter().flatten().filter_map(|reference|self.{field}.get(reference))}}\n\n",row=parts.row_type,field=parts.row_field));
    Ok(value)
}

fn ability(context: &AugmentationContext<'_>) -> Result<RustNativeManagerAugmentation> {
    let row = context.row("AbilityData")?;
    let parts = context.row_parts(row);
    let id = string_value_expression(context.field(row, "AbilityID")?, "source.row")?;
    let tree = string_value_expression(context.field(row, "TreeID")?, "source.row")?;
    let tree_row = string_value_expression(context.field(row, "TreeRowPosition")?, "source.row")?;
    let key_type = format!("{}AbilityTableKey", context.manager.manager_class_name);
    let position_type = format!("{}AbilityPosition", context.manager.manager_class_name);
    Ok(RustNativeManagerAugmentation {
        declarations: format!(
            "#[derive(Debug,Clone,Copy,PartialEq,Eq,Hash)]\nstruct {key_type}{{table:{table},id:Crc32}}\n#[derive(Debug,Clone,Copy,PartialEq,Eq,Hash)]\npub struct {position_type}{{pub table:{table},pub position:u16}}\n#[derive(Debug,Clone,PartialEq,Eq)]\npub struct Ability{{pub source:RowRef<{table},{row}>,pub table_ordinal:u8,pub table_position:u16,pub id:Crc32,pub key:String,pub tree_id:u8,pub tree_row_position:u8}}\n\n",
            table = parts.table_type,
            row = parts.row_type,
        ),
        fields: format!(
            "    abilities:Vec<Ability>,\n    abilities_by_id:HashMap<Crc32,usize>,\n    abilities_by_table:HashMap<{key_type},usize>,\n    abilities_by_position:HashMap<{position_type},usize>,\n    ability_tables:Vec<{table}>,\n    ability_max_tree_row:HashMap<u8,u8>,\n",
            table = parts.table_type,
        ),
        field_values: "            abilities,\n            abilities_by_id,\n            abilities_by_table,\n            abilities_by_position,\n            ability_tables,\n            ability_max_tree_row,\n".to_owned(),
        initializers: format!(r#"        let mut abilities=Vec::new();
        let mut abilities_by_id=HashMap::new();
        let mut abilities_by_table=HashMap::new();
        let mut abilities_by_position=HashMap::new();
        let mut ability_tables=Vec::new();
        let mut ability_table_ordinals=HashMap::new();
        let mut ability_table_positions=HashMap::new();
        let mut ability_max_tree_row=HashMap::new();
        for source in {field}.rows(){{
            let key={id}.trim();
            let id=Crc32::from_str_lower(key);
            let Ok(tree_id)={tree}.trim().parse::<u8>() else{{continue;}};
            let Ok(tree_row_position)={tree_row}.trim().parse::<u8>() else{{continue;}};
            if key.is_empty()||id==Crc32::ZERO{{continue;}}
            let table=*source.reference.table();
            let table_ordinal=*ability_table_ordinals.entry(table).or_insert_with(||{{let ordinal=ability_tables.len() as u8;ability_tables.push(table);ordinal}});
            let table_position=ability_table_positions.entry(table).or_insert(0u16);
            if *table_position>0x3ff{{continue;}}
            let position={position_type}{{table,position:*table_position}};
            *table_position+=1;
            let data=Ability{{source:source.reference.clone(),table_ordinal,table_position:position.position,id,key:key.to_owned(),tree_id,tree_row_position}};
            let index=abilities.len();
            abilities_by_id.entry(id).or_insert(index);
            abilities_by_table.entry({key_type}{{table,id}}).or_insert(index);
            abilities_by_position.entry(position).or_insert(index);
            ability_max_tree_row.entry(data.tree_id).and_modify(|value:&mut u8|*value=(*value).max(data.tree_row_position)).or_insert(data.tree_row_position);
            abilities.push(data);
        }}
"#,field=parts.row_field),
        methods: format!("    pub fn ability_data_from_id(&self,id:Crc32)->Option<&Ability>{{self.abilities_by_id.get(&id).and_then(|index|self.abilities.get(*index))}}\n    pub fn ability_data(&self,key:&str)->Option<&Ability>{{self.ability_data_from_id(Crc32::from_str_lower(key))}}\n    pub fn ability_data_for_table(&self,table:{table},id:Crc32)->Option<&Ability>{{self.abilities_by_table.get(&{key_type}{{table,id}}).and_then(|index|self.abilities.get(*index))}}\n    pub fn ability_data_at_position(&self,position:{position_type})->Option<&Ability>{{self.abilities_by_position.get(&position).and_then(|index|self.abilities.get(*index))}}\n    pub fn ability_data_for_table_slot(&self,ordinal:u8,position:u16)->Option<&Ability>{{let table=*self.ability_tables.get(usize::from(ordinal))?;self.ability_data_at_position({position_type}{{table,position}})}}\n    pub fn max_tree_row_position(&self,tree_id:u8)->Option<u8>{{self.ability_max_tree_row.get(&tree_id).copied()}}\n    pub fn ability_ids(&self)->impl Iterator<Item=Crc32>+'_{{self.abilities.iter().map(|ability|ability.id)}}\n    pub fn abilities(&self)->impl Iterator<Item=&Ability>{{self.abilities.iter()}}\n\n",table=parts.table_type),
        rows_type: "Ability".to_owned(),
        rows_method: "abilities".to_owned(),
    })
}

fn item_transform(context: &AugmentationContext<'_>) -> Result<RustNativeManagerAugmentation> {
    let row = context
        .rows
        .iter()
        .find(|row| {
            row.fields
                .iter()
                .any(|field| field.source_name.eq_ignore_ascii_case("FromItemID"))
        })
        .ok_or_else(|| {
            anyhow!(
                "{} requires an item-transform schema row",
                context.manager.manager_name
            )
        })?;
    let parts = context.row_parts(row);
    let from = string_value_expression(context.field(row, "FromItemID")?, "source.row")?;
    let to = string_value_expression(context.field(row, "ToItemID")?, "source.row")?;
    let keep = optional_bool(context, row, &["KeepPerks"], "source.row", "false")?;
    let feature = optional_string(context, row, &["FeatureID"], "source.row", "\"\"")?;
    let key_type = format!("{}ItemTransformKey", context.manager.manager_class_name);
    Ok(RustNativeManagerAugmentation {
        declarations: format!(
            "#[derive(Debug,Clone,Copy,PartialEq,Eq,Hash)]\nstruct {key_type}{{table:{},from_item_id:Crc32}}\n#[derive(Debug,Clone,PartialEq,Eq)]\npub struct ItemTransform{{pub source:RowRef<{},{}>,pub from_item_key:String,pub from_item_id:Crc32,pub to_item_key:String,pub to_item_id:Crc32,pub keep_perks:bool,pub feature_id:Crc32}}\n\n",
            parts.table_type, parts.table_type, parts.row_type
        ),
        fields: format!(
            "    item_transforms:Vec<ItemTransform>,\n    item_transforms_by_key:HashMap<{key_type},usize>,\n"
        ),
        field_values: "            item_transforms,\n            item_transforms_by_key,\n"
            .to_owned(),
        initializers: format!(
            "        let mut item_transforms=Vec::new();let mut item_transforms_by_key=HashMap::new();\n        for source in {field}.rows(){{let from_key={from}.trim();let to_key={to}.trim();let from_item_id=Crc32::from_str_lower(from_key);let to_item_id=Crc32::from_str_lower(to_key);if from_item_id==Crc32::ZERO||to_item_id==Crc32::ZERO{{continue;}}let key={key_type}{{table:*source.reference.table(),from_item_id}};if item_transforms_by_key.contains_key(&key){{continue;}}let feature_id=Crc32::from_str_lower({feature}.trim());let index=item_transforms.len();item_transforms.push(ItemTransform{{source:source.reference.clone(),from_item_key:from_key.to_owned(),from_item_id,to_item_key:to_key.to_owned(),to_item_id,keep_perks:{keep},feature_id}});item_transforms_by_key.insert(key,index);}}\n",
            field = parts.row_field
        ),
        methods: format!(
            "    pub fn transform(&self,table:{table},from_item_id:Crc32)->Option<&ItemTransform>{{self.item_transforms_by_key.get(&{key_type}{{table,from_item_id}}).and_then(|index|self.item_transforms.get(*index))}}\n    pub fn transform_by_key(&self,table:{table},key:&str)->Option<&ItemTransform>{{self.transform(table,Crc32::from_str_lower(key))}}\n    pub fn transforms(&self)->impl Iterator<Item=&ItemTransform>{{self.item_transforms.iter()}}\n\n",
            table = parts.table_type
        ),
        rows_type: "ItemTransform".to_owned(),
        rows_method: "transforms".to_owned(),
    })
}

fn optional_string(
    context: &AugmentationContext<'_>,
    row: &RustStandaloneSchemaRow,
    names: &[&str],
    receiver: &str,
    default: &str,
) -> Result<String> {
    for name in names {
        if let Ok(field) = context.field(row, name) {
            return string_value_expression(field, receiver);
        }
    }
    Ok(default.to_owned())
}
fn optional_bool(
    context: &AugmentationContext<'_>,
    row: &RustStandaloneSchemaRow,
    names: &[&str],
    receiver: &str,
    default: &str,
) -> Result<String> {
    optional_boolish(context, row, names, receiver, default)
}
fn optional_number(
    context: &AugmentationContext<'_>,
    row: &RustStandaloneSchemaRow,
    names: &[&str],
    receiver: &str,
    default: &str,
) -> Result<String> {
    for name in names {
        if let Ok(field) = context.field(row, name) {
            return numeric_value_or_default_expression(field, receiver, default);
        }
    }
    Ok(default.to_owned())
}

fn optional_boolish(
    context: &AugmentationContext<'_>,
    row: &RustStandaloneSchemaRow,
    names: &[&str],
    receiver: &str,
    default: &str,
) -> Result<String> {
    for name in names {
        let Ok(field) = context.field(row, name) else {
            continue;
        };
        return match (field.column_type, field.required) {
            (ColumnType::Boolean, _) => boolean_value_expression(field, receiver),
            (ColumnType::Number, true) => Ok(format!("{receiver}.{} != 0.0", field.field_name)),
            (ColumnType::Number, false) => Ok(format!(
                "{receiver}.{}.is_some_and(|value| value != 0.0)",
                field.field_name
            )),
            (ColumnType::String, true) => Ok(format!(
                "match {receiver}.{}.trim().to_ascii_lowercase().as_str() {{ \"true\" | \"1\" | \"yes\" => true, \"false\" | \"0\" | \"no\" => false, _ => {default} }}",
                field.field_name
            )),
            (ColumnType::String, false) => Ok(format!(
                "match {receiver}.{}.as_deref().map(str::trim).map(str::to_ascii_lowercase).as_deref() {{ Some(\"true\" | \"1\" | \"yes\") => true, Some(\"false\" | \"0\" | \"no\") => false, _ => {default} }}",
                field.field_name
            )),
        };
    }
    Ok(default.to_owned())
}

fn optional_f32_option(
    context: &AugmentationContext<'_>,
    row: &RustStandaloneSchemaRow,
    names: &[&str],
    receiver: &str,
    diagnostic_name: &str,
) -> Result<String> {
    for name in names {
        let Ok(field) = context.field(row, name) else {
            continue;
        };
        return match (field.column_type, field.required) {
            (ColumnType::Number, true) => Ok(format!(
                "pvp_balance_f32(Some({receiver}.{}), \"{diagnostic_name}\", target)?",
                field.field_name
            )),
            (ColumnType::Number, false) => Ok(format!(
                "pvp_balance_f32({receiver}.{}, \"{diagnostic_name}\", target)?",
                field.field_name
            )),
            (ColumnType::String, _) => {
                let value = string_value_expression(field, receiver)?;
                Ok(format!(
                    "pvp_balance_number(Some({value}), \"{diagnostic_name}\", target)?"
                ))
            }
            (ColumnType::Boolean, _) => bail!(
                "standalone Rust manager `{}` requires numeric or string column `{}` on `{}`",
                context.manager.manager_name,
                field.source_name,
                row.source_row_type
            ),
        };
    }
    Ok("None".to_owned())
}

fn optional_balance_text(
    context: &AugmentationContext<'_>,
    row: &RustStandaloneSchemaRow,
    names: &[&str],
    receiver: &str,
    diagnostic_name: &str,
) -> Result<String> {
    for name in names {
        let Ok(field) = context.field(row, name) else {
            continue;
        };
        return match (field.column_type, field.required) {
            (ColumnType::String, _) => {
                let value = string_value_expression(field, receiver)?;
                Ok(format!("pvp_balance_text({value})"))
            }
            (ColumnType::Number, true) => Ok(format!(
                "pvp_balance_number_text(Some({receiver}.{}), \"{diagnostic_name}\", target)?",
                field.field_name
            )),
            (ColumnType::Number, false) => Ok(format!(
                "pvp_balance_number_text({receiver}.{}, \"{diagnostic_name}\", target)?",
                field.field_name
            )),
            (ColumnType::Boolean, _) => bail!(
                "standalone Rust manager `{}` requires string or numeric column `{}` on `{}`",
                context.manager.manager_name,
                field.source_name,
                row.source_row_type
            ),
        };
    }
    Ok("None".to_owned())
}

fn optional_field_string(
    field: Option<&RustStandaloneSchemaField>,
    receiver: &str,
    default: &str,
) -> Result<String> {
    field
        .map(|field| string_value_expression(field, receiver))
        .transpose()
        .map(|value| value.unwrap_or_else(|| default.to_owned()))
}

fn optional_field_number(
    field: Option<&RustStandaloneSchemaField>,
    receiver: &str,
    default: &str,
) -> Result<String> {
    field
        .map(|field| numeric_value_or_default_expression(field, receiver, default))
        .transpose()
        .map(|value| value.unwrap_or_else(|| default.to_owned()))
}

fn numbered_suffix(value: &str, prefix: &str) -> Option<u16> {
    let suffix = value.get(prefix.len()..)?;
    value
        .get(..prefix.len())?
        .eq_ignore_ascii_case(prefix)
        .then(|| suffix.parse().ok())
        .flatten()
}

fn add_designer_list_helper(value: &mut RustNativeManagerAugmentation) {
    value.declarations.push_str(
        "fn split_designer_list(value: &str) -> impl Iterator<Item = &str> {\n    value.split([',', '+']).map(str::trim).filter(|part| !part.is_empty())\n}\n\n",
    );
}

fn clean_ident(value: &str) -> String {
    rust_fragment_ident(value)
}

#[cfg(test)]
mod indexed_tests {
    use std::collections::BTreeMap;

    use nw_datasheet::{ColumnType, game_system::Crc32};

    use super::super::super::rust_effective_native_manager_surface;
    use super::super::augment_native_manager;
    use crate::game_system_schema::{
        GameSystemColumnSchema, GameSystemColumnValueShape, GameSystemDataTablesSchemaReport,
        GameSystemNumberShape, GameSystemTableSchema,
    };
    use crate::manager::{NativeManagerShape, validated_native_manager_specs};
    use crate::manager_records::{ManagerSurface, manager_surfaces_from_managers};

    #[test]
    fn audited_indexed_managers_emit_semantic_rust_contracts() {
        let surfaces = manager_surfaces_from_managers(&validated_native_manager_specs())
            .expect("manager surfaces");
        let selected = surfaces
            .iter()
            .filter_map(|surface| match surface {
                ManagerSurface::Native { manager, shape, .. }
                    if matches!(
                        shape,
                        NativeManagerShape::OneTableDyeColor(_)
                            | NativeManagerShape::QuickCourseData(_)
                            | NativeManagerShape::OneTableWorldEventRule(_)
                            | NativeManagerShape::OneTablePvpBalance(_)
                            | NativeManagerShape::RewardTrackData(_)
                            | NativeManagerShape::LootBucketData(_)
                    ) =>
                {
                    Some((manager, shape))
                }
                _ => None,
            })
            .collect::<Vec<_>>();
        assert!(selected.len() >= 13, "expected all PvP balance variants");

        for (manager, shape) in selected {
            let effective = rust_effective_native_manager_surface(manager, shape);
            let schema = schema_for(&effective);
            let augmentation = augment_native_manager(&effective, shape, &schema)
                .unwrap_or_else(|error| panic!("{}: {error:#}", manager.manager_name));
            let emitted = format!(
                "{}{}{}{}{}",
                augmentation.declarations,
                augmentation.fields,
                augmentation.field_values,
                augmentation.initializers,
                augmentation.methods
            );
            assert!(!emitted.contains("RowRef<"), "{}", manager.manager_name);
            assert!(
                !augmentation.rows_type.is_empty(),
                "{}",
                manager.manager_name
            );
            assert!(
                augmentation
                    .methods
                    .contains(&format!("fn {}(", augmentation.rows_method)),
                "{}",
                manager.manager_name
            );
            let source = format!(
                "{}\nstruct Contract {{ {} }}\nimpl Contract {{ fn build() -> Result<Self> {{ {} Ok(Self {{ {} }}) }} {} }}",
                augmentation.declarations,
                augmentation.fields,
                augmentation.initializers,
                augmentation.field_values,
                augmentation.methods
            );
            syn::parse_file(&source).unwrap_or_else(|error| {
                panic!(
                    "{} emitted invalid Rust: {error}\n{source}",
                    manager.manager_name
                )
            });
        }
    }

    fn schema_for(
        manager: &crate::manager_records::DirectManagerSurface,
    ) -> GameSystemDataTablesSchemaReport {
        let tables = manager
            .tables
            .iter()
            .map(|table| GameSystemTableSchema {
                table_name: table.table_name.clone(),
                table_name_crc: Crc32::from_str_lower(&table.table_name).value(),
                row_type_name: table.row_type_name.clone(),
                row_type_crc: Crc32::from_str_lower(&table.row_type_name).value(),
                row_count: 1,
                sources: vec![format!("{}.datasheet", table.table_name)],
                columns: contract_columns(),
            })
            .collect();
        GameSystemDataTablesSchemaReport {
            tables,
            diagnostics: Vec::new(),
            type_affinities: Vec::new(),
        }
    }

    fn contract_columns() -> Vec<GameSystemColumnSchema> {
        let mut columns = BTreeMap::new();
        for (names, column_type) in [
            (
                &[
                    "RuleID",
                    "Category",
                    "Hub",
                    "Zone",
                    "Tags",
                    "QuickCourseID",
                    "PathReferenceQuickCourseID",
                    "AudioGroup",
                    "TimedRaceNodeTypeId",
                    "VisualSlicePath",
                    "SFX",
                    "BalanceTarget",
                    "BalanceCategory",
                    "AbilityBaseDamageAdjustment",
                    "AffixStatAdjustment",
                    "IncomingHealAdjustment",
                    "ConsumableHealAdjustment",
                    "PotencyAdjustment",
                    "DurationAdjustment",
                    "Bucket1",
                    "RewardID1",
                    "Tag1",
                    "MatchOne1",
                    "Type1",
                    "ExcludeTypeStage1",
                    "ExcludeTypeShop1",
                    "RowPlaceholders",
                    "RewardID",
                    "Name",
                    "Color",
                    "SpecColor",
                ][..],
                ColumnType::String,
            ),
            (
                &[
                    "MaxEvents",
                    "MinDistance",
                    "StartingTimerSeconds",
                    "NodeTimeOverrideMultiplier",
                    "DetectionRadius",
                    "AddTimeSeconds",
                    "WeaponBaseDamageAdjustment",
                    "SelfHealAdjustment",
                    "CooldownAdjustment",
                    "RandomWeights1",
                    "BudgetContribution1",
                    "Index",
                    "IsEntitlement",
                    "ColorAmount",
                    "ColorOverride",
                    "SpecAmount",
                    "MaskGlossShift",
                ][..],
                ColumnType::Number,
            ),
            (
                &["Disabled", "IsTimed", "AccumulateTime", "UseTimeOverride"][..],
                ColumnType::Boolean,
            ),
        ] {
            for name in names {
                columns.insert(
                    name.to_ascii_lowercase(),
                    contract_column(name, column_type),
                );
            }
        }
        columns.into_values().collect()
    }

    fn contract_column(name: &str, declared_type: ColumnType) -> GameSystemColumnSchema {
        let value_shape = match declared_type {
            ColumnType::Boolean => GameSystemColumnValueShape::Boolean,
            ColumnType::Number => GameSystemColumnValueShape::Number {
                number_shape: GameSystemNumberShape::Float,
            },
            ColumnType::String => GameSystemColumnValueShape::String {
                identifier_like: true,
                localized_key_like: false,
                asset_path_like: false,
                expression_like: false,
                qualified_reference_like: false,
                list: None,
                foreign_keys: Vec::new(),
            },
        };
        GameSystemColumnSchema {
            name: name.to_owned(),
            crc: Crc32::from_str_lower(name).value(),
            declared_type,
            row_key: false,
            required: false,
            non_empty_rows: 1,
            empty_rows: 1,
            distinct_values: 1,
            value_shape,
        }
    }
}
