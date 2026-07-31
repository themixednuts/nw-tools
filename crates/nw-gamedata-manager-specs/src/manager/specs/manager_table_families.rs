use std::collections::BTreeSet;

use super::*;

pub(super) fn dungeon_tile_static_data_manager_spec() -> NativeManagerSpec {
    let shape = NativeOneTableDungeonTileManager::new(
        ident("dungeon_tile_static_data"),
        game_table("DungeonTile"),
        game_row_type("DungeonTileStaticData"),
        ident("DungeonTileStaticData"),
        ident("rows"),
        ident("rows_by_tile_id"),
        ident("rows_by_source"),
        ident("rows_by_feature_connections"),
        ident("tile_key"),
        ident("tile_id"),
        column("DungeonTileId"),
        ident("dungeon_tile_id"),
        ident("feature_key"),
        ident("feature_id"),
        column("FeatureId"),
        ident("feature_id"),
        ident("connections"),
        column("Connections"),
        ident("connections"),
        ident("rotations"),
        column("Rotations"),
        ident("rotations"),
        ident("tile_size"),
        column("TileSize"),
        ident("tile_size"),
        ident("weight"),
        column("Weight"),
        ident("weight"),
        ident("variation_asset_paths"),
        column("VariationAssetPaths"),
        ident("variation_asset_paths"),
        ident("supported_room_types"),
        column("SupportedRoomTypes"),
        ident("supported_room_types"),
        ident("source_row"),
        ident("dungeon_tile_static_data_for_source_row"),
        vec![
            lookup(
                "dungeon_tile_static_data",
                "tile_id",
                NativeCrcIndexLookupParameterKind::IntoCrc32,
            ),
            lookup(
                "dungeon_tile_static_data_by_key",
                "tile_id",
                NativeCrcIndexLookupParameterKind::AsRefStr,
            ),
        ],
        ident("tile_variants"),
        ident("tile_variant_row"),
        ident("rows"),
        ident("len"),
        ident("is_empty"),
    )
    .expect("validated DungeonTile manager shape");

    manager_spec(
        "Javelin::PCG::DungeonTileStaticDataManager",
        "crate::DungeonTileStaticDataManager",
        "DungeonTile",
        "DungeonTileStaticData",
        vec![
            "Javelin::PCG::DungeonTileStaticDataManager::DungeonTileStaticDataManager",
            "Javelin::PCG::DungeonTileStaticDataManager::CacheAllDataTables",
        ],
    )
    .with_shape(NativeManagerShape::one_table_dungeon_tile(shape))
}

pub(super) fn costume_change_data_manager_spec() -> NativeManagerSpec {
    let shape = NativeOneTableCostumeChangeManager::new(
        ident("costume_change_data"),
        game_table("CostumeChanges"),
        game_row_type("CostumeChangeData"),
        ident("CostumeChangeData"),
        ident("CostumeAudioSlot"),
        ident("CostumeAudioDataOverride"),
        ident("costume_changes"),
        ident("costume_changes_by_id"),
        ident("costume_changes_by_source"),
        ident("costume_change_id"),
        ident("costume_change_key"),
        column("CostumeChangeId"),
        ident("costume_change_id"),
        ident("costume_change_mesh"),
        column("CostumeChangeMesh"),
        ident("costume_change_mesh"),
        ident("matches_player_skeleton"),
        column("MatchesPlayerSkeleton"),
        ident("matches_player_skeleton"),
        ident("mesh_render_z_pos_offset"),
        column("MeshRenderZPosOffset"),
        ident("mesh_render_z_pos_offset"),
        ident("audio_overrides"),
        ident("source_row"),
        ident("costume_change_data_for_source_row"),
        vec![
            costume_audio_slot(
                "Head",
                "Head",
                "HEAD_SLOT_Left",
                "head_slot_left",
                "HEAD_SLOT_Right",
                "head_slot_right",
            ),
            costume_audio_slot(
                "Chest",
                "Chest",
                "CHEST_SLOT_Left",
                "chest_slot_left",
                "CHEST_SLOT_Right",
                "chest_slot_right",
            ),
            costume_audio_slot(
                "Hands",
                "Hands",
                "HANDS_SLOT_Left",
                "hands_slot_left",
                "HANDS_SLOT_Right",
                "hands_slot_right",
            ),
            costume_audio_slot(
                "Legs",
                "Legs",
                "LEGS_SLOT_Left",
                "legs_slot_left",
                "LEGS_SLOT_Right",
                "legs_slot_right",
            ),
            costume_audio_slot(
                "Feet",
                "Feet",
                "FEET_SLOT_Left",
                "feet_slot_left",
                "FEET_SLOT_Right",
                "feet_slot_right",
            ),
        ],
        ident("costume_change_data_from_id"),
        ident("costume_change_data"),
        ident("costume_change_data_by_key"),
        ident("costume_audio_data_override_from_id"),
        ident("costume_audio_data_override"),
        ident("rows"),
        ident("len"),
        ident("is_empty"),
    )
    .expect("validated CostumeChange manager shape");

    manager_spec(
        "Javelin::CostumeChangeDataManager",
        "crate::CostumeChangeDataManager",
        "CostumeChanges",
        "CostumeChangeData",
        vec![
            "Javelin::CostumeChangeDataManager::CostumeChangeDataManager",
            "Javelin::CostumeChangeDataManager::CacheAllDataTables",
            "Javelin::CostumeChangeDataManager::GetCostumeAudioDataOverride",
        ],
    )
    .with_shape(NativeManagerShape::one_table_costume_change(shape))
}

pub(super) fn crest_part_data_manager_spec() -> NativeManagerSpec {
    let shape = NativeOneTableCrestPartManager::new(
        ident("crest_part_data"),
        game_table("Crests"),
        game_row_type("CrestPartData"),
        ident("CrestPartData"),
        ident("CrestPartKind"),
        ident("CrestPartFaction"),
        ident("CrestPartFactionParseError"),
        ident("CrestPartIndexes"),
        ident("crest_parts"),
    );

    manager_spec(
        "Javelin::CrestPartDataManager",
        "crate::CrestPartDataManager",
        "Crests",
        "CrestPartData",
        vec![
            "Javelin::CrestPartDataManager::CrestPartDataManager",
            "Javelin::CrestPartDataManager::CacheAllDataTables",
            "Javelin::CrestPartDataManager::FindGameSystemDataByKey",
        ],
    )
    .with_shape(NativeManagerShape::one_table_crest_part(shape))
}

pub(super) fn vitals_base_data_manager_spec() -> NativeManagerSpec {
    table_family_crc_projection(TableFamilyCrcProjectionSpec {
        module: "vitals_base_data",
        table_module: None,
        tables: vitals_base_data_tables(),
        tables_type: "VitalsBaseDataTables",
        table_type: "VitalsBaseDataTable",
        handle_type: "VitalsBaseDataHandle",
        row_alias: "VitalsBaseDataRow",
        data_type: "VitalsBaseData",
        entries_field: "base_vitals",
        index_field: "rows_by_crc",
        key_field: "vitals_id",
        crc_field: "vitals_id_crc",
        key_column: "VitalsID",
        key_getter: "vitals_id",
        skip_empty_key: true,
        trim_key: false,
        reject_zero_crc: true,
        duplicate_key_policy: NativeDuplicateKeyPolicy::FirstWins,
        source_handle_field: Some("source"),
        source_handle_method: None,
        row_filters: Vec::new(),
        fields: vec![
            projection_field(
                "creature_type",
                "CreatureType",
                "creature_type",
                NativeProjectionTransform::OptionalFirstString,
            ),
            projection_field(
                "creature_type_crc",
                "CreatureType",
                "creature_type",
                NativeProjectionTransform::OptionalFirstLowercaseCrcStringDefaultZero,
            ),
            projection_field(
                "deaths_door_time",
                "DeathsDoorTime",
                "deaths_door_time",
                NativeProjectionTransform::F32,
            ),
            projection_field(
                "deaths_door_delay",
                "DeathsDoorDelay",
                "deaths_door_delay",
                NativeProjectionTransform::F32,
            ),
        ],
        table_indexes: Vec::new(),
        field_lookup_methods: Vec::new(),
        store_key_text: true,
        lookup_methods: vec![
            lookup(
                "vitals_base_data_from_id",
                "vitals_id",
                NativeCrcIndexLookupParameterKind::Crc32,
            ),
            lookup(
                "vitals_base_data",
                "vitals_id",
                NativeCrcIndexLookupParameterKind::AsRefStr,
            ),
            lookup(
                "vitals_base_data_by_key",
                "key",
                NativeCrcIndexLookupParameterKind::AsRefStr,
            ),
        ],
        ids_method: None,
        rows_method: Some("base_vitals"),
        len_method: Some("len"),
        is_empty_method: Some("is_empty"),
        ghidra_class: "Javelin::VitalsBaseDataManager",
        rust_type: "crate::VitalsBaseDataManager",
        ghidra_functions: vec![
            "Javelin::VitalsBaseDataManager::VitalsBaseDataManager",
            "Javelin::VitalsBaseDataManager::CacheAllDataTables",
        ],
    })
}

pub(super) fn cutscene_camera_data_manager_spec() -> NativeManagerSpec {
    crc_projection(CrcProjectionSpec {
        module: "cutscene_camera_data",
        table_name: "CutsceneCameraPresets",
        row_type_name: "CutsceneCameraStaticData",
        data_type: "CutsceneCameraData",
        entries_field: "cutscene_cameras",
        index_field: "cutscene_cameras_by_id",
        key_field: "cutscene_camera_key",
        crc_field: "cutscene_camera_id",
        key_column: "CutsceneCameraId",
        key_getter: "cutscene_camera_id",
        skip_empty_key: false,
        trim_key: false,
        reject_zero_crc: true,
        duplicate_key_policy: NativeDuplicateKeyPolicy::FirstWins,
        source_row_field: Some("source_row"),
        source_row_method: Some("cutscene_camera_for_source_row"),
        row_filters: Vec::new(),
        fields: vec![
            projection_field(
                "camera_state",
                "CameraState",
                "camera_state",
                NativeProjectionTransform::String,
            ),
            projection_field(
                "camera_state_origin",
                "CameraStateOrigin",
                "camera_state_origin",
                NativeProjectionTransform::String,
            ),
            projection_field(
                "camera_state_look_at",
                "CameraStateLookAt",
                "camera_state_look_at",
                NativeProjectionTransform::OptionalString,
            ),
            projection_field(
                "enter_blend_time",
                "EnterBlendTime",
                "enter_blend_time",
                NativeProjectionTransform::F32,
            ),
            projection_field(
                "exit_blend_time",
                "ExitBlendTime",
                "exit_blend_time",
                NativeProjectionTransform::F32,
            ),
            projection_field(
                "force_instant_exit_transition",
                "ForceInstantExitTransition",
                "force_instant_exit_transition",
                NativeProjectionTransform::Bool,
            ),
            projection_field(
                "hide_ui_on_trigger",
                "HideUIOnTrigger",
                "hide_ui_on_trigger",
                NativeProjectionTransform::Bool,
            ),
            projection_field(
                "origin_enter_blend_time",
                "OriginEnterBlendTime",
                "origin_enter_blend_time",
                NativeProjectionTransform::F32,
            ),
            projection_field(
                "origin_exit_blend_time",
                "OriginExitBlendTime",
                "origin_exit_blend_time",
                NativeProjectionTransform::F32,
            ),
            projection_field(
                "npc_look_at_info",
                "NpcLookAtInfo",
                "npc_look_at_info",
                NativeProjectionTransform::OptionalString,
            ),
            projection_field(
                "player_fade_rate",
                "PlayerFadeRate",
                "player_fade_rate",
                NativeProjectionTransform::F32,
            ),
            projection_field(
                "hide_player_avatar",
                "HidePlayerAvatar",
                "hide_player_avatar",
                NativeProjectionTransform::Bool,
            ),
            projection_field(
                "hide_nearby_player_avatars",
                "HideNearbyPlayerAvatars",
                "hide_nearby_player_avatars",
                NativeProjectionTransform::OptionalBool,
            ),
            projection_field(
                "hide_nearby_ai_avatars",
                "HideNearbyAIAvatars",
                "hide_nearby_ai_avatars",
                NativeProjectionTransform::OptionalBool,
            ),
            projection_field(
                "force_instant_look_ats",
                "ForceInstantLookAts",
                "force_instant_look_ats",
                NativeProjectionTransform::OptionalBool,
            ),
            projection_field(
                "depth_of_field_override",
                "DepthOfFieldOverride",
                "depth_of_field_override",
                NativeProjectionTransform::F32ListDefaultEmpty,
            ),
            projection_field(
                "look_at_smooth_time_override",
                "LookAtSmoothTimeOverride",
                "look_at_smooth_time_override",
                NativeProjectionTransform::OptionalF32,
            ),
            projection_field(
                "dummy_preset_pos",
                "DummyPresetPos",
                "dummy_preset_pos",
                NativeProjectionTransform::OptionalString,
            ),
            projection_field(
                "dummy_preset_rot",
                "DummyPresetRot",
                "dummy_preset_rot",
                NativeProjectionTransform::OptionalString,
            ),
            projection_field(
                "dummy_is_armed",
                "DummyIsArmed",
                "dummy_is_armed",
                NativeProjectionTransform::OptionalBool,
            ),
            projection_field(
                "dummy_look_at_info",
                "DummyLookAtInfo",
                "dummy_look_at_info",
                NativeProjectionTransform::OptionalString,
            ),
            projection_field(
                "block_player_input",
                "BlockPlayerInput",
                "block_player_input",
                NativeProjectionTransform::Bool,
            ),
            projection_field(
                "cancel_inventory",
                "CancelInventory",
                "cancel_inventory",
                NativeProjectionTransform::OptionalBool,
            ),
            projection_field(
                "interrupt_in_combat",
                "InterruptInCombat",
                "interrupt_in_combat",
                NativeProjectionTransform::OptionalBool,
            ),
            projection_field(
                "interrupt_on_movement",
                "InterruptOnMovement",
                "interrupt_on_movement",
                NativeProjectionTransform::OptionalBool,
            ),
            projection_field(
                "can_skip",
                "CanSkip",
                "can_skip",
                NativeProjectionTransform::OptionalBool,
            ),
            projection_field(
                "play_fade_effect",
                "PlayFadeEffect",
                "play_fade_effect",
                NativeProjectionTransform::Bool,
            ),
            projection_field(
                "banner_title_label_text",
                "BannerTitleLabelText",
                "banner_title_label_text",
                NativeProjectionTransform::OptionalString,
            ),
            projection_field(
                "banner_title_text",
                "BannerTitleText",
                "banner_title_text",
                NativeProjectionTransform::OptionalString,
            ),
            projection_field(
                "banner_description_text",
                "BannerDescriptionText",
                "banner_description_text",
                NativeProjectionTransform::OptionalString,
            ),
            projection_field(
                "enable_client_fps_optimizer",
                "EnableClientFPSOptimizer",
                "enable_client_fps_optimizer",
                NativeProjectionTransform::OptionalBool,
            ),
            projection_field(
                "spectator_mode",
                "SpectatorMode",
                "spectator_mode",
                NativeProjectionTransform::OptionalBool,
            ),
            projection_field(
                "spectator_camera_origin_pitch",
                "SpectatorCameraOriginPitch",
                "spectator_camera_origin_pitch",
                NativeProjectionTransform::I32,
            ),
        ],
        secondary_indexes: Vec::new(),
        lookup_methods: vec![
            lookup(
                "cutscene_camera",
                "cutscene_camera_id",
                NativeCrcIndexLookupParameterKind::AsRefStr,
            ),
            lookup(
                "cutscene_camera_by_id",
                "cutscene_camera_id",
                NativeCrcIndexLookupParameterKind::Crc32,
            ),
            lookup(
                "cutscene_camera_by_key",
                "key",
                NativeCrcIndexLookupParameterKind::AsRefStr,
            ),
        ],
        ids_method: None,
        rows_method: Some("rows"),
        len_method: Some("len"),
        is_empty_method: Some("is_empty"),
        ghidra_class: "Javelin::CutsceneCameraDataManager",
        rust_type: "crate::CutsceneCameraDataManager",
        ghidra_functions: vec!["Javelin::CutsceneCameraDataManager::CutsceneCameraDataManager"],
    })
}

pub(super) fn level_disparity_data_manager_spec() -> NativeManagerSpec {
    let shape = NativeOneTableLevelDisparityManager::new(
        ident("level_disparity_data"),
        game_table("AILevelDisparity"),
        game_row_type("LevelDisparityData"),
        ident("LevelDisparityData"),
        ident("LevelDisparityRange"),
        ident("rows"),
        ident("rows_by_disparity"),
        ident("rows_by_source"),
        ident("range"),
        ident("max_vision_distance_adjustment"),
        ident("level_disparity"),
        column("LevelDisparity"),
        ident("level_disparity"),
        ident("source_row"),
        ident("level_disparity_data_for_source_row"),
        ident("vision_distance_adjustment"),
        vec![
            projection_field(
                "damage_modifier",
                "DamageModifier",
                "damage_modifier",
                NativeProjectionTransform::F32,
            ),
            projection_field(
                "physical_armor_rating_modifier",
                "PhysicalArmorRatingModifier",
                "physical_armor_rating_modifier",
                NativeProjectionTransform::F32,
            ),
            projection_field(
                "elemental_armor_rating_modifier",
                "ElementalArmorRatingModifier",
                "elemental_armor_rating_modifier",
                NativeProjectionTransform::F32,
            ),
            projection_field(
                "skip_deaths_door",
                "SkipDeathsDoor",
                "skip_deaths_door",
                NativeProjectionTransform::Bool,
            ),
            projection_field(
                "incoming_power_level_zero",
                "IncomingPowerLevelZero",
                "incoming_power_level_zero",
                NativeProjectionTransform::Bool,
            ),
            projection_field(
                "adjust_power_level",
                "AdjustPowerLevel",
                "adjust_power_level",
                NativeProjectionTransform::Bool,
            ),
            projection_field(
                "required_power_level",
                "RequiredPowerLevel",
                "required_power_level",
                NativeProjectionTransform::U32,
            ),
            projection_field(
                "adjusted_power_level",
                "AdjustedPowerLevel",
                "adjusted_power_level",
                NativeProjectionTransform::U32,
            ),
            projection_field(
                "adjusted_hit_stun",
                "AdjustedHitStun",
                "adjusted_hit_stun",
                NativeProjectionTransform::F32,
            ),
            projection_field(
                "vision_distance_adjustment",
                "VisionDistanceAdjustment",
                "vision_distance_adjustment",
                NativeProjectionTransform::F32,
            ),
            projection_field(
                "max_reward_level_delta",
                "MaxRewardLevelDelta",
                "max_reward_level_delta",
                NativeProjectionTransform::U32,
            ),
            projection_field(
                "kill_exp_modifier",
                "KillExpModifier",
                "kill_exp_modifier",
                NativeProjectionTransform::F32,
            ),
            projection_field(
                "event_exp_modifier",
                "EventExpModifier",
                "event_exp_modifier",
                NativeProjectionTransform::F32,
            ),
        ],
        ident("level_disparity_data"),
        ident("level_disparity_data_for_levels"),
        ident("clamped_level_disparity_data_for_levels"),
        ident("level_disparity_data_for_levels_with_player_level_cap"),
        ident("clamped_level_disparity_data_for_levels_with_player_level_cap"),
        ident("loaded_range"),
        ident("clamped_disparity"),
        ident("max_vision_distance_adjustment"),
        ident("rows"),
        ident("len"),
        ident("is_empty"),
    );

    manager_spec(
        "Javelin::LevelDisparityDataManager",
        "crate::LevelDisparityDataManager",
        "AILevelDisparity",
        "LevelDisparityData",
        vec!["Javelin::LevelDisparityDataManager::CacheDataTables"],
    )
    .with_shape(NativeManagerShape::one_table_level_disparity(shape))
}

pub(super) fn leaderboard_rewards_data_manager_spec() -> NativeManagerSpec {
    const LEADERBOARD_REWARD_ROTATION: &str = "gamedata::semantic::LeaderboardRotations";

    crc_projection(CrcProjectionSpec {
        module: "leaderboard_rewards_data",
        table_name: "LeaderboardRewardsDataTable",
        row_type_name: "LeaderboardRewardsData",
        data_type: "StaticLeaderboardRewardData",
        entries_field: "rewards",
        index_field: "rewards_by_id",
        key_field: "reward_key",
        crc_field: "reward_id",
        key_column: "LeaderboardRewardId",
        key_getter: "leaderboard_reward_id",
        skip_empty_key: true,
        trim_key: false,
        reject_zero_crc: false,
        duplicate_key_policy: NativeDuplicateKeyPolicy::FirstWins,
        source_row_field: Some("source_row"),
        source_row_method: Some("leaderboard_reward_for_source_row"),
        row_filters: Vec::new(),
        fields: vec![
            projection_field(
                "entitlement_reward_id",
                "EntitlementRewards",
                "entitlement_rewards",
                NativeProjectionTransform::OptionalCrc32ZeroAsNone,
            ),
            typed_projection_field(
                "rotation",
                "Rotation",
                "rotation",
                NativeProjectionTransform::U8Enum,
                LEADERBOARD_REWARD_ROTATION,
            ),
            projection_field(
                "rotation_start",
                "RotationStart",
                "rotation_start",
                NativeProjectionTransform::NonZeroU32,
            ),
            projection_field(
                "reward_id_no_rotation",
                "LeaderboardRewardIdNoRotation",
                "leaderboard_reward_id_no_rotation",
                NativeProjectionTransform::Crc32,
            ),
        ],
        secondary_indexes: Vec::new(),
        lookup_methods: vec![
            lookup(
                "leaderboard_reward_from_id",
                "id",
                NativeCrcIndexLookupParameterKind::IntoCrc32,
            ),
            lookup(
                "leaderboard_reward",
                "id",
                NativeCrcIndexLookupParameterKind::AsRefStr,
            ),
        ],
        ids_method: None,
        rows_method: Some("leaderboard_rewards"),
        len_method: Some("len"),
        is_empty_method: Some("is_empty"),
        ghidra_class: "Javelin::LeaderboardRewardsDataManager",
        rust_type: "crate::LeaderboardRewardsDataManager",
        ghidra_functions: vec![
            "Javelin::LeaderboardRewardsDataManager::LeaderboardRewardsDataManager",
            "Javelin::LeaderboardRewardsDataManager::CacheAllDataTables",
        ],
    })
}

pub(super) fn expansion_data_manager_spec() -> NativeManagerSpec {
    const NON_ZERO_U16: &str = "std::num::NonZeroU16";

    enum_projection(EnumProjectionSpec {
        module: "expansion_data",
        table_name: "Expansions",
        row_type_name: "ExpansionData",
        data_type: "ExpansionData",
        entries_field: "expansions",
        index_field: "expansions_by_id",
        key_field: "expansion_id",
        key_column: "ExpansionId",
        key_getter: "expansion_id",
        key_type: "gamedata::semantic::ExpansionId",
        key_type_alias: None,
        table_view_alias: None,
        expose_table_constructor: false,
        invalid_key_variants: Vec::new(),
        skip_empty_key: true,
        trim_key: true,
        duplicate_key_policy: NativeDuplicateKeyPolicy::FirstWins,
        source_row_field: Some("source_row"),
        source_row_method: None,
        fields: vec![
            projection_field(
                "display_name",
                "DisplayName",
                "display_name",
                NativeProjectionTransform::String,
            ),
            projection_field("icon", "Icon", "icon", NativeProjectionTransform::String),
            typed_projection_field(
                "max_display_level",
                "MaxDisplayLevel",
                "max_display_level",
                NativeProjectionTransform::TypedCell,
                NON_ZERO_U16,
            ),
            typed_projection_field(
                "max_craft_gs",
                "MaxCraftGS",
                "max_craft_gs",
                NativeProjectionTransform::TypedCell,
                NON_ZERO_U16,
            ),
            typed_projection_field(
                "max_equip_gs",
                "MaxEquipGS",
                "max_equip_gs",
                NativeProjectionTransform::TypedCell,
                NON_ZERO_U16,
            ),
            typed_projection_field(
                "max_tradeskill_level",
                "MaxTradeskillLevel",
                "max_tradeskill_level",
                NativeProjectionTransform::TypedCell,
                NON_ZERO_U16,
            ),
            projection_field(
                "entitlement_id",
                "EntitlementId",
                "entitlement_id",
                NativeProjectionTransform::OptionalCrc32,
            ),
        ],
        lookup_methods: vec![enum_lookup(
            "expansion_data_by_id",
            "expansion_id",
            NativeEnumLookupParameterKind::Enum,
        )],
        secondary_crc_index: Some(EnumProjectionCrcIndexSpec {
            index_field: "expansions_by_crc",
            crc_field: "expansion_id_crc",
            lookup_methods: vec![
                lookup(
                    "expansion_data",
                    "expansion_id",
                    NativeCrcIndexLookupParameterKind::IntoCrc32,
                ),
                lookup(
                    "expansion_data_by_key",
                    "key",
                    NativeCrcIndexLookupParameterKind::AsRefStr,
                ),
            ],
        }),
        ids_method: Some("expansion_ids"),
        rows_method: Some("expansions"),
        len_method: Some("len"),
        is_empty_method: Some("is_empty"),
        ghidra_class: "Javelin::ExpansionDataManager",
        rust_type: "crate::ExpansionDataManager",
        ghidra_functions: vec!["Javelin::ExpansionDataManager::ExpansionDataManager"],
    })
}

pub(super) fn territory_progression_data_manager_spec() -> NativeManagerSpec {
    crc_projection(CrcProjectionSpec {
        module: "territory_progression_data",
        table_name: "TerritoryProgression",
        row_type_name: "TerritoryProgressionData",
        data_type: "TerritoryProgressionData",
        entries_field: "rows",
        index_field: "rows_by_crc",
        key_field: "project_key",
        crc_field: "project_id",
        key_column: "ProjectId",
        key_getter: "project_id",
        skip_empty_key: true,
        trim_key: false,
        reject_zero_crc: true,
        duplicate_key_policy: NativeDuplicateKeyPolicy::FirstWins,
        source_row_field: Some("source_row"),
        source_row_method: Some("territory_progression_data_for_source_row"),
        row_filters: Vec::new(),
        fields: vec![
            projection_field("cost", "Cost", "cost", NativeProjectionTransform::U32),
            projection_field(
                "level",
                "Level",
                "level",
                NativeProjectionTransform::NonZeroU32,
            ),
            projection_field("icon", "Icon", "icon", NativeProjectionTransform::String),
            projection_field("title", "Title", "title", NativeProjectionTransform::String),
            projection_field(
                "chat_notification_title",
                "ChatNotificationTitle",
                "chat_notification_title",
                NativeProjectionTransform::String,
            ),
            projection_field(
                "button_label",
                "ButtonLabel",
                "button_label",
                NativeProjectionTransform::String,
            ),
            projection_field("image", "Image", "image", NativeProjectionTransform::String),
            projection_field(
                "current_tier",
                "CurrentTier",
                "current_tier",
                NativeProjectionTransform::NonZeroU32,
            ),
            projection_field(
                "crafting_azoth_discount",
                "CraftingAzothDiscount",
                "crafting_azoth_discount",
                NativeProjectionTransform::F32,
            ),
            projection_field(
                "description",
                "Description",
                "description",
                NativeProjectionTransform::String,
            ),
            projection_field(
                "territory_progression_needed",
                "TerritoryProgressionNeeded",
                "territory_progression_needed",
                NativeProjectionTransform::U32,
            ),
            projection_field(
                "completion_time_minutes",
                "CompletionTimeMinutes",
                "completion_time_minutes",
                NativeProjectionTransform::F32,
            ),
            projection_field(
                "prev_level_project_row",
                "PrevLevelProjectId",
                "prev_level_project_id",
                NativeProjectionTransform::OptionalForeignKeyRow,
            ),
            projection_field(
                "next_level_project_row",
                "NextLevelProjectId",
                "next_level_project_id",
                NativeProjectionTransform::OptionalForeignKeyRow,
            ),
            projection_field(
                "project_type",
                "ProjectType",
                "project_type",
                NativeProjectionTransform::String,
            ),
            projection_field(
                "display_column",
                "DisplayColumn",
                "display_column",
                NativeProjectionTransform::String,
            ),
            projection_field(
                "progression_category",
                "ProgressionCategory",
                "progression_category",
                NativeProjectionTransform::String,
            ),
            projection_field(
                "progression_category_name",
                "ProgressionCategoryName",
                "progression_category_name",
                NativeProjectionTransform::String,
            ),
            projection_field(
                "progression_level",
                "ProgressionLevel",
                "progression_level",
                NativeProjectionTransform::NonZeroU32,
            ),
            projection_field(
                "lifestyle_buff_effect_id",
                "LifestyleBuffEffectId",
                "lifestyle_buff_effect_id",
                NativeProjectionTransform::OptionalString,
            ),
            projection_field(
                "lifestyle_buff_effect_duration",
                "LifestyleBuffEffectDuration",
                "lifestyle_buff_effect_duration",
                NativeProjectionTransform::F32,
            ),
        ],
        secondary_indexes: Vec::new(),
        lookup_methods: vec![
            lookup(
                "territory_progression_data_from_id",
                "project_id",
                NativeCrcIndexLookupParameterKind::Crc32,
            ),
            lookup(
                "territory_progression_data",
                "project_id",
                NativeCrcIndexLookupParameterKind::AsRefStr,
            ),
            lookup(
                "territory_progression_data_by_key",
                "key",
                NativeCrcIndexLookupParameterKind::AsRefStr,
            ),
        ],
        ids_method: None,
        rows_method: Some("rows"),
        len_method: Some("len"),
        is_empty_method: Some("is_empty"),
        ghidra_class: "Javelin::TerritoryProgressionDataManager",
        rust_type: "crate::TerritoryProgressionDataManager",
        ghidra_functions: vec![
            "Javelin::TerritoryProgressionDataManager::TerritoryProgressionDataManager",
            "Javelin::TerritoryProgressionDataManager::CacheAllDataTables",
        ],
    })
}

pub(super) fn item_skin_data_manager_spec() -> NativeManagerSpec {
    crc_projection(CrcProjectionSpec {
        module: "item_skin_data",
        table_name: "ItemSkinDataTable",
        row_type_name: "ItemSkinData",
        data_type: "ItemSkinData",
        entries_field: "skins",
        index_field: "rows_by_crc",
        key_field: "item_skin_id",
        crc_field: "item_skin_id_crc",
        key_column: "ItemSkinID",
        key_getter: "item_skin_id",
        skip_empty_key: false,
        trim_key: false,
        reject_zero_crc: false,
        duplicate_key_policy: NativeDuplicateKeyPolicy::Error,
        source_row_field: Some("source_row"),
        source_row_method: Some("source_row"),
        row_filters: Vec::new(),
        fields: vec![
            projection_field(
                "index_id",
                "IndexID",
                "index_id",
                NativeProjectionTransform::NonZeroU32,
            ),
            projection_field(
                "is_entitlement",
                "IsEntitlement",
                "is_entitlement",
                NativeProjectionTransform::Bool,
            ),
            projection_field(
                "from_item_ids",
                "FromItemIDs",
                "from_item_i_ds",
                NativeProjectionTransform::OptionalString,
            ),
            projection_field(
                "needs_one_classes",
                "NeedsOneClasses",
                "needs_one_classes",
                NativeProjectionTransform::String,
            ),
            projection_field(
                "required_classes",
                "RequiredClasses",
                "required_classes",
                NativeProjectionTransform::OptionalString,
            ),
            projection_field(
                "excluded_classes",
                "ExcludedClasses",
                "excluded_classes",
                NativeProjectionTransform::OptionalString,
            ),
            projection_field(
                "to_item_id",
                "ToItemID",
                "to_item_id",
                NativeProjectionTransform::LowercaseCrcString,
            ),
            projection_field(
                "to_item_key",
                "ToItemID",
                "to_item_id",
                NativeProjectionTransform::String,
            ),
            projection_field(
                "outfit",
                "Outfit",
                "outfit",
                NativeProjectionTransform::OptionalString,
            ),
            projection_field(
                "is_temporary_skin",
                "IsTemporarySkin",
                "is_temporary_skin",
                NativeProjectionTransform::Bool,
            ),
        ],
        secondary_indexes: vec![sparse_nonzero_u32_index(
            "index_to_row",
            "index_id",
            NativeDuplicateKeyPolicy::Error,
            vec![
                secondary_u32_lookup("item_skin_by_index", "index_id"),
                secondary_u32_string_field_lookup(
                    "item_skin_id_at_index",
                    "index_id",
                    "item_skin_id",
                ),
            ],
        )],
        lookup_methods: vec![
            lookup(
                "item_skin_from_id",
                "item_skin_id",
                NativeCrcIndexLookupParameterKind::IntoCrc32,
            ),
            lookup(
                "item_skin_by_id",
                "item_skin_id",
                NativeCrcIndexLookupParameterKind::AsRefStr,
            ),
            lookup(
                "item_skin_by_key",
                "key",
                NativeCrcIndexLookupParameterKind::AsRefStr,
            ),
        ],
        ids_method: None,
        rows_method: Some("rows"),
        len_method: Some("len"),
        is_empty_method: Some("is_empty"),
        ghidra_class: "Javelin::ItemSkinDataManager",
        rust_type: "crate::ItemSkinDataManager",
        ghidra_functions: vec![
            "Javelin::ItemSkinDataManager::GetItemSkinDataFromIndex",
            "Javelin::ItemSkinDataManager::GetItemSkinDataFromId",
            "Javelin::ItemSkinDataManager::GetItemSkinDataFromKey",
        ],
    })
}

pub(super) fn mission_data_table_specs() -> Vec<TableFamilyTableSpec> {
    inputs::MISSION_DATA_MANAGER_TABLES
        .iter()
        .copied()
        .map(|table_name| TableFamilyTableSpec {
            variant: to_upper_camel_ident(table_name, "Table"),
            table_name,
            row_type_name: "MissionData",
        })
        .collect()
}

pub(super) fn mission_data_manager_spec() -> NativeManagerSpec {
    let tables = mission_data_table_specs();
    let input_tables = tables
        .iter()
        .map(|table| table_input(table.table_name, table.row_type_name))
        .collect::<Vec<_>>();
    let shape = NativeTableFamilyCrcKeyProjectionManager::new(
        ident("mission_data"),
        tables
            .into_iter()
            .map(|table| {
                let TableFamilyTableSpec {
                    variant,
                    table_name,
                    row_type_name,
                } = table;
                NativeTableFamilyTable::new(
                    ident(variant),
                    game_table(table_name),
                    game_row_type(row_type_name),
                )
            })
            .collect(),
        ident("MissionDataTables"),
        ident("MissionDataTable"),
        ident("MissionDataHandle"),
        ident("MissionDataRow"),
        ident("MissionData"),
        ident("missions"),
        ident("missions_by_id"),
        ident("mission_key"),
        ident("mission_id"),
        column("MissionID"),
        ident("mission_id"),
        true,
        false,
        false,
        NativeDuplicateKeyPolicy::FirstWins,
        Vec::new(),
        vec![
            lookup(
                "mission_data_from_id",
                "mission_id",
                NativeCrcIndexLookupParameterKind::IntoCrc32,
            ),
            lookup(
                "mission_data",
                "mission_id",
                NativeCrcIndexLookupParameterKind::AsRefStr,
            ),
            lookup(
                "mission_data_by_key",
                "key",
                NativeCrcIndexLookupParameterKind::AsRefStr,
            ),
        ],
    )
    .expect("validated table-family CRC-key projection manager shape")
    .with_schema_validation_fields(NativeSchemaProjectionFields::all_non_key())
    .with_rows_method(ident("rows"))
    .with_len_method(ident("len"))
    .with_is_empty_method(ident("is_empty"));

    NativeManagerSpec::new(
        GhidraClassPath::new("Javelin::MissionDataManager").expect("validated Ghidra class"),
        rust_type("crate::MissionDataManager"),
        input_tables,
        vec![
            GhidraFunctionPath::new("Javelin::MissionDataManager::MissionDataManager")
                .expect("validated Ghidra function"),
            GhidraFunctionPath::new("Javelin::MissionDataManager::CacheAllDataTables")
                .expect("validated Ghidra function"),
        ],
    )
    .expect("validated native manager spec")
    .with_shape(NativeManagerShape::table_family_crc_key_projection(shape))
}

pub(super) fn recipe_data_manager_spec() -> NativeManagerSpec {
    let product = product_asset_resource(
        "sharedassets/genericassets/craftingstations.craftstationdb",
        NativeManagerProductKind::CraftingStationDatabase,
        "crafting_station_database",
        "database",
        "crafting_station_database",
    );
    let shape = NativeRecipeDataManager::new(
        ident("recipe_data"),
        inputs::manager_table_family_specs("crate::RecipeDataManager")
            .into_iter()
            .map(|table| {
                NativeTableFamilyTable::new(
                    ident(table.variant),
                    game_table(table.table_name),
                    game_row_type(table.row_type_name),
                )
            })
            .collect(),
        ident("RecipeDataTable"),
        ident("RecipeDataHandle"),
        ident("RecipeData"),
        NativeProductAssetResource::new(
            rust_type(product.product.rust_type),
            rust_type(product.product.rust_type),
            ident(product.handle_getter),
            ident(product.asset_getter),
            ident(product.manager_getter),
        ),
    )
    .expect("validated recipe data manager shape");

    manager_spec_with_inputs(
        "Javelin::RecipeDataManager",
        "crate::RecipeDataManager",
        vec![NativeManagerInput::object_stream_product(
            asset_path(product.product.asset_path),
            rust_type(product.product.rust_type),
        )],
        vec![
            "Javelin::RecipeDataManager::RecipeDataManager",
            "Javelin::RecipeDataManager::CacheAllRecipeDataTables",
        ],
    )
    .with_shape(NativeManagerShape::recipe_data(shape))
}

pub(super) fn item_data_manager_spec() -> NativeManagerSpec {
    let mut seen_tables = BTreeSet::new();
    let tables = inputs::manager_table_family_specs("crate::ItemDataManager")
        .into_iter()
        .filter(|table| table.row_type_name == "MasterItemDefinitions")
        .filter(|table| seen_tables.insert((table.table_name, table.row_type_name)))
        .map(|table| {
            NativeTableFamilyTable::new(
                ident(table.variant),
                game_table(table.table_name),
                game_row_type(table.row_type_name),
            )
        })
        .collect::<Vec<_>>();
    let shape = NativeItemDataManager::new(
        ident("item_data"),
        tables,
        ident("ItemDataTable"),
        ident("ItemDataHandle"),
        ident("StaticItemData"),
    )
    .expect("validated item data manager shape");

    manager_spec_with_inputs(
        "Javelin::ItemDataManager",
        "crate::ItemDataManager",
        vec![
            NativeManagerInput::manager(rust_type("crate::DyeItemDataManager")),
            NativeManagerInput::manager(rust_type("crate::MountDyeItemDataManager")),
            NativeManagerInput::manager(rust_type("crate::DyeColorDataManager")),
        ],
        vec![
            "Javelin::ItemDataManager::ItemDataManager",
            "Javelin::ItemDataManager::CacheAllItemDataTables",
            "Javelin::ItemDataManager::CacheItemDefinitionTables",
            "Javelin::ItemDataManager::CacheMasterItemDefinitionsTable",
            "Javelin::ItemDataManager::GetStaticItemDataById",
            "Javelin::ItemDataManager::GetStaticItemDataByIndex",
            "Javelin::ItemDataManager::BuildPerkItemIndexes",
        ],
    )
    .with_shape(NativeManagerShape::item_data(shape))
}

pub(super) fn item_conversion_data_manager_spec() -> NativeManagerSpec {
    let shape = NativeItemConversionDataManager::new(
        ident("item_conversion_data"),
        game_table("ItemCurrencyConversions"),
        game_row_type("ItemCurrencyConversionData"),
        ident("ItemConversionDataHandle"),
        ident("ItemConversionData"),
    )
    .expect("validated item conversion data manager shape");

    manager_spec_with_inputs(
        "Javelin::ItemConversionDataManager",
        "crate::ItemConversionDataManager",
        vec![
            NativeManagerInput::manager(rust_type("crate::ItemDataManager")),
            NativeManagerInput::manager(rust_type("crate::CategoricalProgressionDataManager")),
            NativeManagerInput::manager(rust_type("crate::AchievementDataManager")),
        ],
        vec![
            "Javelin::ItemConversionDataManager::ItemConversionDataManager",
            "Javelin::ItemConversionDataManager::CacheAllDataTables",
        ],
    )
    .with_shape(NativeManagerShape::item_conversion_data(shape))
}

pub(super) fn variation_data_manager_spec() -> NativeManagerSpec {
    let tables = inputs::manager_table_family_specs("crate::VariationDataManager");
    let input_tables = tables
        .iter()
        .map(|table| table_input(table.table_name, table.row_type_name))
        .collect::<Vec<_>>();
    let shape = NativeTableFamilyFallbackCrcKeyProjectionManager::new(
        ident("variation_data"),
        tables
            .into_iter()
            .map(|table| {
                let TableFamilyTableSpec {
                    variant,
                    table_name,
                    row_type_name,
                } = table;
                NativeTableFamilyTable::new(
                    ident(variant),
                    game_table(table_name),
                    game_row_type(row_type_name),
                )
            })
            .collect(),
        ident("VariationDataTables"),
        ident("VariationData"),
        ident("variations"),
        ident("rows_by_crc"),
        ident("key_kind"),
        ident("VariationDataKeyKind"),
        ident("VariantId"),
        ident("HouseItemId"),
        ident("key"),
        ident("key_id"),
        column("VariantID"),
        ident("variant_id"),
        column("HouseItemID"),
        ident("house_item_id"),
        true,
        NativeDuplicateKeyPolicy::FirstWins,
        vec![
            lookup(
                "variation_data_from_id",
                "id",
                NativeCrcIndexLookupParameterKind::IntoCrc32,
            ),
            lookup(
                "variation_data",
                "key",
                NativeCrcIndexLookupParameterKind::AsRefStr,
            ),
            lookup(
                "variation_data_by_key",
                "key",
                NativeCrcIndexLookupParameterKind::AsRefStr,
            ),
        ],
    )
    .expect("validated table-family fallback CRC-key projection manager shape")
    .with_rows_method(ident("rows"))
    .with_len_method(ident("len"))
    .with_is_empty_method(ident("is_empty"));

    NativeManagerSpec::new(
        GhidraClassPath::new("Javelin::VariationDataManager").expect("validated Ghidra class"),
        rust_type("crate::VariationDataManager"),
        input_tables,
        vec![
            GhidraFunctionPath::new("Javelin::VariationDataManager::VariationDataManager")
                .expect("validated Ghidra function"),
            GhidraFunctionPath::new("Javelin::VariationDataManager::CacheAllDataTables")
                .expect("validated Ghidra function"),
        ],
    )
    .expect("validated native manager spec")
    .with_shape(NativeManagerShape::table_family_fallback_crc_key_projection(shape))
}

pub(super) fn spell_data_table_specs() -> Vec<TableFamilyTableSpec> {
    inputs::SPELL_DATA_MANAGER_TABLES
        .iter()
        .copied()
        .map(|(variant, table_name)| TableFamilyTableSpec {
            variant: variant.to_owned(),
            table_name,
            row_type_name: "SpellData",
        })
        .collect()
}

pub(super) fn spell_data_manager_spec() -> NativeManagerSpec {
    table_family_crc_projection(TableFamilyCrcProjectionSpec {
        module: "spell_data",
        table_module: None,
        tables: spell_data_table_specs(),
        tables_type: "SpellDataTables",
        table_type: "SpellDataTable",
        handle_type: "SpellDataHandle",
        row_alias: "SpellDataRow",
        data_type: "SpellData",
        entries_field: "spells",
        index_field: "rows_by_spell_id",
        key_field: "spell_id",
        crc_field: "spell_id_crc",
        key_column: "SpellID",
        key_getter: "spell_id",
        skip_empty_key: true,
        trim_key: false,
        reject_zero_crc: true,
        duplicate_key_policy: NativeDuplicateKeyPolicy::FirstWins,
        source_handle_field: Some("source"),
        source_handle_method: None,
        row_filters: Vec::new(),
        fields: vec![
            projection_field(
                "damage_table_row",
                "DamageTableRow",
                "damage_table_row",
                NativeProjectionTransform::OptionalString,
            ),
            projection_field(
                "damage_table_row_crc",
                "DamageTableRow",
                "damage_table_row",
                NativeProjectionTransform::OptionalTrimmedLowercaseCrcString,
            ),
            projection_field(
                "ranged_attack_name",
                "RangedAttackName",
                "ranged_attack_name",
                NativeProjectionTransform::OptionalString,
            ),
            projection_field(
                "ranged_attack_profile",
                "RangedAttackProfile",
                "ranged_attack_profile",
                NativeProjectionTransform::OptionalString,
            ),
        ],
        table_indexes: Vec::new(),
        field_lookup_methods: Vec::new(),
        store_key_text: true,
        lookup_methods: vec![
            lookup(
                "spell_data_from_id",
                "spell_id",
                NativeCrcIndexLookupParameterKind::Crc32,
            ),
            lookup(
                "spell_data",
                "spell_id",
                NativeCrcIndexLookupParameterKind::AsRefStr,
            ),
            lookup(
                "spell_data_by_key",
                "key",
                NativeCrcIndexLookupParameterKind::AsRefStr,
            ),
        ],
        ids_method: Some("spell_ids"),
        rows_method: None,
        len_method: Some("len"),
        is_empty_method: Some("is_empty"),
        ghidra_class: "Javelin::SpellDataManager",
        rust_type: "crate::SpellDataManager",
        ghidra_functions: vec![
            "Javelin::SpellDataManager::SpellDataManager",
            "Javelin::SpellDataManager::CacheAllSpellDataTables",
            "Javelin::SpellDataManager::GetSpellData",
        ],
    })
}

pub(super) fn lore_data_manager_spec() -> NativeManagerSpec {
    crc_projection(CrcProjectionSpec {
        module: "lore_data",
        table_name: "Lore",
        row_type_name: "LoreData",
        data_type: "LoreData",
        entries_field: "rows",
        index_field: "rows_by_id",
        key_field: "lore_key",
        crc_field: "lore_id",
        key_column: "LoreID",
        key_getter: "lore_id",
        skip_empty_key: true,
        trim_key: false,
        reject_zero_crc: false,
        duplicate_key_policy: NativeDuplicateKeyPolicy::FirstWins,
        source_row_field: None,
        source_row_method: None,
        row_filters: Vec::new(),
        fields: vec![
            projection_field(
                "type_id",
                "Type",
                "type_",
                NativeProjectionTransform::LowercaseCrcString,
            ),
            projection_field(
                "type_key",
                "Type",
                "type_",
                NativeProjectionTransform::String,
            ),
            projection_field("title", "Title", "title", NativeProjectionTransform::String),
            projection_field(
                "subtitle",
                "Subtitle",
                "subtitle",
                NativeProjectionTransform::OptionalString,
            ),
            projection_field(
                "body",
                "Body",
                "body",
                NativeProjectionTransform::OptionalString,
            ),
            projection_field(
                "achievement_id",
                "AchievementId",
                "achievement_id",
                NativeProjectionTransform::OptionalLowercaseCrcString,
            ),
            projection_field(
                "achievement_key",
                "AchievementId",
                "achievement_id",
                NativeProjectionTransform::OptionalString,
            ),
            projection_field(
                "parent_id",
                "ParentID",
                "parent_id",
                NativeProjectionTransform::OptionalLowercaseCrcString,
            ),
            projection_field(
                "parent_key",
                "ParentID",
                "parent_id",
                NativeProjectionTransform::OptionalString,
            ),
            projection_field("order", "Order", "order", NativeProjectionTransform::U32),
            projection_field(
                "image_path",
                "ImagePath",
                "image_path",
                NativeProjectionTransform::OptionalString,
            ),
            projection_field(
                "location_name",
                "LocationName",
                "location_name",
                NativeProjectionTransform::OptionalString,
            ),
            projection_field(
                "location_xy",
                "LocationXY",
                "location_xy",
                NativeProjectionTransform::F32ListDefaultEmpty,
            ),
            projection_field(
                "associated_quests",
                "AssociatedQuest",
                "associated_quest",
                NativeProjectionTransform::StringList,
            ),
            projection_field(
                "writer",
                "Writer",
                "writer",
                NativeProjectionTransform::OptionalString,
            ),
            projection_field(
                "loc_notes",
                "LocNotes",
                "loc_notes",
                NativeProjectionTransform::OptionalString,
            ),
            projection_field(
                "recording_status",
                "RecordingStatus",
                "recording_status",
                NativeProjectionTransform::OptionalString,
            ),
            projection_field(
                "lore_notes_locations",
                "LoreNotesLocation",
                "lore_notes_location",
                NativeProjectionTransform::StringList,
            ),
        ],
        secondary_indexes: Vec::new(),
        lookup_methods: vec![
            lookup(
                "lore_data",
                "lore_id",
                NativeCrcIndexLookupParameterKind::AsRefStr,
            ),
            lookup(
                "lore_data_from_id",
                "lore_id",
                NativeCrcIndexLookupParameterKind::IntoCrc32,
            ),
            lookup(
                "lore_data_by_crc32",
                "lore_id",
                NativeCrcIndexLookupParameterKind::IntoCrc32,
            ),
        ],
        ids_method: None,
        rows_method: Some("rows"),
        len_method: Some("len"),
        is_empty_method: Some("is_empty"),
        ghidra_class: "Javelin::LoreDataManager",
        rust_type: "crate::LoreDataManager",
        ghidra_functions: vec![
            "Javelin::LoreDataManager::LoreDataManager",
            "Javelin::LoreDataManager::CacheAllLoreDataTables",
            "Javelin::LoreDataManager::GetLoreDataById",
        ],
    })
}

pub(super) fn timeline_registry_manager_spec() -> NativeManagerSpec {
    table_family_crc_projection(TableFamilyCrcProjectionSpec {
        module: "timeline_registry_entry_data",
        table_module: None,
        tables: vec![
            TableFamilyTableSpec {
                variant: "GenericTimelineRegistryEntry".to_owned(),
                table_name: "GenericTimelineRegistryEntry",
                row_type_name: "TimelineRegistryEntryData",
            },
            TableFamilyTableSpec {
                variant: "TimelineRegistryEntry".to_owned(),
                table_name: "TimelineRegistryEntry",
                row_type_name: "TimelineRegistryEntryData",
            },
            TableFamilyTableSpec {
                variant: "WhisperTimelineRegistryEntry".to_owned(),
                table_name: "WhisperTimelineRegistryEntry",
                row_type_name: "TimelineRegistryEntryData",
            },
        ],
        tables_type: "TimelineRegistryEntryDataTables",
        table_type: "TimelineRegistryEntryDataTable",
        handle_type: "TimelineRegistryEntryDataHandle",
        row_alias: "TimelineRegistryEntryDataRow",
        data_type: "TimelineRegistryEntryData",
        entries_field: "entries",
        index_field: "entries_by_name",
        key_field: "entry_name_key",
        crc_field: "entry_name",
        key_column: "TimelineEntryName",
        key_getter: "timeline_entry_name",
        skip_empty_key: true,
        trim_key: false,
        reject_zero_crc: false,
        duplicate_key_policy: NativeDuplicateKeyPolicy::FirstWins,
        source_handle_field: Some("source"),
        source_handle_method: None,
        row_filters: Vec::new(),
        fields: vec![projection_field(
            "timeline_asset_path",
            "TimelineAssetPath",
            "timeline_asset_path",
            NativeProjectionTransform::LowercaseCrcString,
        )],
        table_indexes: vec![
            TableFamilyCrcTableIndexSpec {
                index_field: "generic_entries_by_name",
                table_variant: "GenericTimelineRegistryEntry",
                duplicate_key_policy: NativeDuplicateKeyPolicy::FirstWins,
                lookup_methods: vec![lookup(
                    "generic_timeline_registry_entry",
                    "entry_name",
                    NativeCrcIndexLookupParameterKind::IntoCrc32,
                )],
            },
            TableFamilyCrcTableIndexSpec {
                index_field: "whisper_entries_by_name",
                table_variant: "WhisperTimelineRegistryEntry",
                duplicate_key_policy: NativeDuplicateKeyPolicy::FirstWins,
                lookup_methods: vec![lookup(
                    "whisper_timeline_registry_entry",
                    "entry_name",
                    NativeCrcIndexLookupParameterKind::IntoCrc32,
                )],
            },
        ],
        field_lookup_methods: Vec::new(),
        store_key_text: false,
        lookup_methods: vec![lookup(
            "timeline_registry_entry",
            "entry_name",
            NativeCrcIndexLookupParameterKind::IntoCrc32,
        )],
        ids_method: None,
        rows_method: Some("entries"),
        len_method: Some("len"),
        is_empty_method: Some("is_empty"),
        ghidra_class: "Javelin::TimelineRegistryManager",
        rust_type: "crate::TimelineRegistryManager",
        ghidra_functions: vec![
            "Javelin::TimelineRegistryManager::TimelineRegistryManager",
            "Javelin::TimelineRegistryManager::CacheAllDataTables",
        ],
    })
}

pub(super) fn seasons_rewards_activities_config_data_manager_spec() -> NativeManagerSpec {
    table_family_crc_projection(TableFamilyCrcProjectionSpec {
        module: "seasons_rewards_activities_config",
        table_module: None,
        tables: inputs::manager_table_family_specs(
            "crate::SeasonsRewardsActivitiesConfigDataManager",
        ),
        tables_type: "SeasonsRewardsActivitiesConfigTables",
        table_type: "SeasonsRewardsActivitiesConfigTable",
        handle_type: "SeasonsRewardsActivitiesConfigHandle",
        row_alias: "SeasonsRewardsActivitiesConfigRow",
        data_type: "SeasonsRewardsActivitiesConfigData",
        entries_field: "configs",
        index_field: "rows_by_config_id",
        key_field: "config_key",
        crc_field: "config_id",
        key_column: "ConfigId",
        key_getter: "config_id",
        skip_empty_key: true,
        trim_key: false,
        reject_zero_crc: false,
        duplicate_key_policy: NativeDuplicateKeyPolicy::FirstWins,
        source_handle_field: None,
        source_handle_method: None,
        row_filters: Vec::new(),
        fields: vec![projection_field(
            "config_value",
            "ConfigValue",
            "config_value",
            NativeProjectionTransform::NonZeroU32,
        )],
        table_indexes: Vec::new(),
        field_lookup_methods: Vec::new(),
        store_key_text: true,
        lookup_methods: vec![
            lookup(
                "seasons_rewards_activities_config_data",
                "config_id",
                NativeCrcIndexLookupParameterKind::IntoCrc32,
            ),
            lookup(
                "seasons_rewards_activities_config_data_by_key",
                "config_key",
                NativeCrcIndexLookupParameterKind::AsRefStr,
            ),
        ],
        ids_method: None,
        rows_method: Some("configs"),
        len_method: Some("len"),
        is_empty_method: Some("is_empty"),
        ghidra_class: "Javelin::SeasonsRewardsActivitiesConfigDataManager",
        rust_type: "crate::SeasonsRewardsActivitiesConfigDataManager",
        ghidra_functions: vec![
            "Javelin::SeasonsRewardsActivitiesConfigDataManager::SeasonsRewardsActivitiesConfigDataManager",
            "Javelin::SeasonsRewardsActivitiesConfigDataManager::CacheAllDataTables",
            "Javelin::SeasonsRewardsActivitiesConfigDataManager::CacheDataTable",
            "Javelin::SeasonsRewardsActivitiesConfigDataManager::CacheTableRows",
        ],
    })
}

pub(super) fn seasons_rewards_season_data_manager_spec() -> NativeManagerSpec {
    crc_projection(CrcProjectionSpec {
        module: "seasons_rewards_season_data",
        table_name: "SeasonsRewardsSeasonDataTable",
        row_type_name: "SeasonsRewardsSeasonData",
        data_type: "SeasonsRewardsSeasonData",
        entries_field: "seasons",
        index_field: "seasons_by_id",
        key_field: "season_key",
        crc_field: "season_id",
        key_column: "SeasonId",
        key_getter: "season_id",
        skip_empty_key: true,
        trim_key: false,
        reject_zero_crc: false,
        duplicate_key_policy: NativeDuplicateKeyPolicy::FirstWins,
        source_row_field: None,
        source_row_method: None,
        row_filters: vec![lowercase_crc_string_nonzero_filter(
            "PremiumEntitlementId",
            "premium_entitlement_id",
        )],
        fields: vec![
            projection_field(
                "season_index",
                "SeasonIndex",
                "season_index",
                NativeProjectionTransform::NonZeroU32,
            ),
            projection_field("name", "Name", "name", NativeProjectionTransform::String),
            projection_field(
                "display_name",
                "DisplayName",
                "display_name",
                NativeProjectionTransform::String,
            ),
            projection_field(
                "description",
                "Description",
                "description",
                NativeProjectionTransform::String,
            ),
            projection_field(
                "premium_entitlement_id",
                "PremiumEntitlementId",
                "premium_entitlement_id",
                NativeProjectionTransform::LowercaseCrcString,
            ),
            projection_field(
                "premium_entitlement_key",
                "PremiumEntitlementId",
                "premium_entitlement_id",
                NativeProjectionTransform::String,
            ),
            projection_field(
                "purchased_levels_entitlement_id",
                "PurchasedLevelsEntitlementId",
                "purchased_levels_entitlement_id",
                NativeProjectionTransform::LowercaseCrcString,
            ),
            projection_field(
                "purchased_levels_entitlement_key",
                "PurchasedLevelsEntitlementId",
                "purchased_levels_entitlement_id",
                NativeProjectionTransform::String,
            ),
            projection_field(
                "fresh_start_world_gen",
                "FreshStartWorldGen",
                "fresh_start_world_gen",
                NativeProjectionTransform::NonZeroU32,
            ),
        ],
        secondary_indexes: vec![secondary_nonzero_u32_index(
            "seasons_by_index",
            "season_index",
            NativeDuplicateKeyPolicy::FirstWins,
            vec![secondary_nonzero_u32_lookup(
                "seasons_rewards_season_data_by_index",
                "season_index",
            )],
        )],
        lookup_methods: vec![
            lookup(
                "seasons_rewards_season_data",
                "season_id",
                NativeCrcIndexLookupParameterKind::IntoCrc32,
            ),
            lookup(
                "seasons_rewards_season_data_by_key",
                "season_key",
                NativeCrcIndexLookupParameterKind::AsRefStr,
            ),
        ],
        ids_method: None,
        rows_method: Some("seasons"),
        len_method: Some("len"),
        is_empty_method: Some("is_empty"),
        ghidra_class: "Javelin::SeasonsRewardsSeasonDataManager",
        rust_type: "crate::SeasonsRewardsSeasonDataManager",
        ghidra_functions: vec![
            "Javelin::SeasonsRewardsSeasonDataManager::SeasonsRewardsSeasonDataManager",
            "Javelin::SeasonsRewardsSeasonDataManager::CacheAllDataTables",
            "Javelin::SeasonsRewardsSeasonDataManager::CacheDataTable",
            "Javelin::SeasonsRewardsSeasonDataManager::CacheTableRows",
            "Javelin::SeasonsRewardsSeasonDataManager::DecodeRow",
        ],
    })
}

pub(super) fn seasons_rewards_task_data_manager_spec() -> NativeManagerSpec {
    crc_projection(CrcProjectionSpec {
        module: "seasons_rewards_task_data",
        table_name: "SeasonsRewardsTasks",
        row_type_name: "SeasonsRewardsTasks",
        data_type: "SeasonsRewardsTaskData",
        entries_field: "tasks",
        index_field: "tasks_by_id",
        key_field: "task_key",
        crc_field: "task_id",
        key_column: "SeasonsTaskID",
        key_getter: "seasons_task_id",
        skip_empty_key: true,
        trim_key: false,
        reject_zero_crc: false,
        duplicate_key_policy: NativeDuplicateKeyPolicy::FirstWins,
        source_row_field: Some("source_row"),
        source_row_method: Some("source_row"),
        row_filters: vec![lowercase_crc_string_nonzero_filter(
            "SeasonsTrackedStatID",
            "seasons_tracked_stat_id",
        )],
        fields: vec![
            projection_field(
                "seasons_tracked_stat_id",
                "SeasonsTrackedStatID",
                "seasons_tracked_stat_id",
                NativeProjectionTransform::LowercaseCrcString,
            ),
            projection_field(
                "seasons_tracked_stat_key",
                "SeasonsTrackedStatID",
                "seasons_tracked_stat_id",
                NativeProjectionTransform::String,
            ),
            projection_field(
                "task_max_value",
                "TaskMaxValue",
                "task_max_value",
                NativeProjectionTransform::NonZeroU32,
            ),
            projection_field("name", "Name", "name", NativeProjectionTransform::String),
            projection_field(
                "description",
                "Description",
                "description",
                NativeProjectionTransform::String,
            ),
        ],
        secondary_indexes: Vec::new(),
        lookup_methods: vec![
            lookup(
                "seasons_rewards_task_data",
                "task_id",
                NativeCrcIndexLookupParameterKind::IntoCrc32,
            ),
            lookup(
                "seasons_rewards_task_data_by_key",
                "task_key",
                NativeCrcIndexLookupParameterKind::AsRefStr,
            ),
        ],
        ids_method: None,
        rows_method: Some("tasks"),
        len_method: Some("len"),
        is_empty_method: Some("is_empty"),
        ghidra_class: "Javelin::SeasonsRewardsTaskDataManager",
        rust_type: "crate::SeasonsRewardsTaskDataManager",
        ghidra_functions: vec![
            "Javelin::SeasonsRewardsTaskDataManager::SeasonsRewardsTaskDataManager",
            "Javelin::SeasonsRewardsTaskDataManager::CacheAllDataTables",
            "Javelin::SeasonsRewardsTaskDataManager::CacheDataTable",
            "Javelin::SeasonsRewardsTaskDataManager::CacheTableRows",
        ],
    })
}

pub(super) fn seasons_rewards_card_data_manager_spec() -> NativeManagerSpec {
    table_family_partitioned_crc_projection(TableFamilyPartitionedCrcProjectionSpec {
        module: "seasons_rewards_card_data",
        tables: inputs::manager_table_family_specs("crate::SeasonsRewardsCardDataManager"),
        tables_type: "SeasonsRewardsCardDataTables",
        table_type: "SeasonsRewardsCardDataTable",
        data_type: "SeasonsRewardsCardData",
        entries_field: "cards",
        key_field: "card_key",
        crc_field: "card_id",
        key_column: "CardId",
        key_getter: "card_id",
        skip_empty_key: true,
        trim_key: true,
        reject_zero_crc: false,
        global_index: Some(partitioned_global_index(
            "cards_by_card_id",
            NativeDuplicateKeyPolicy::FirstWins,
            vec![
                lookup(
                    "seasons_rewards_card_data",
                    "card_id",
                    NativeCrcIndexLookupParameterKind::IntoCrc32,
                ),
                lookup(
                    "seasons_rewards_card_data_by_key",
                    "card_key",
                    NativeCrcIndexLookupParameterKind::AsRefStr,
                ),
            ],
        )),
        table_indexes: vec![
            private_table_index(
                "season1_cards_by_card_id",
                "SeasonsRewardsCardDataSeason1",
                NativeDuplicateKeyPolicy::FirstWins,
            ),
            private_table_index(
                "season10_cards_by_card_id",
                "SeasonsRewardsCardDataSeason10",
                NativeDuplicateKeyPolicy::FirstWins,
            ),
            private_table_index(
                "season2_cards_by_card_id",
                "SeasonsRewardsCardDataSeason2",
                NativeDuplicateKeyPolicy::FirstWins,
            ),
            private_table_index(
                "season3_cards_by_card_id",
                "SeasonsRewardsCardDataSeason3",
                NativeDuplicateKeyPolicy::FirstWins,
            ),
            private_table_index(
                "season4_cards_by_card_id",
                "SeasonsRewardsCardDataSeason4",
                NativeDuplicateKeyPolicy::FirstWins,
            ),
            private_table_index(
                "season5_cards_by_card_id",
                "SeasonsRewardsCardDataSeason5",
                NativeDuplicateKeyPolicy::FirstWins,
            ),
            private_table_index(
                "season6_cards_by_card_id",
                "SeasonsRewardsCardDataSeason6",
                NativeDuplicateKeyPolicy::FirstWins,
            ),
            private_table_index(
                "season7_cards_by_card_id",
                "SeasonsRewardsCardDataSeason7",
                NativeDuplicateKeyPolicy::FirstWins,
            ),
            private_table_index(
                "season8_cards_by_card_id",
                "SeasonsRewardsCardDataSeason8",
                NativeDuplicateKeyPolicy::FirstWins,
            ),
            private_table_index(
                "season9_cards_by_card_id",
                "SeasonsRewardsCardDataSeason9",
                NativeDuplicateKeyPolicy::FirstWins,
            ),
        ],
        fields: vec![
            projection_field(
                "stamps_to_complete",
                "StampsToComplete",
                "stamps_to_complete",
                NativeProjectionTransform::NonZeroU32,
            ),
            projection_field(
                "line_bonus_xp",
                "LineBonusXp",
                "line_bonus_xp",
                NativeProjectionTransform::NonZeroU32,
            ),
            projection_field(
                "pattern_bonus_xp",
                "PatternBonusXp",
                "pattern_bonus_xp",
                NativeProjectionTransform::NonZeroU32,
            ),
            projection_field(
                "card_bonus_xp",
                "CardBonusXp",
                "card_bonus_xp",
                NativeProjectionTransform::NonZeroU32,
            ),
        ],
        vec3_fields: Vec::new(),
        store_key_text: true,
        rows_method: Some("cards"),
        len_method: Some("len"),
        is_empty_method: Some("is_empty"),
        ghidra_class: "Javelin::SeasonsRewardsCardDataManager",
        rust_type: "crate::SeasonsRewardsCardDataManager",
        ghidra_functions: vec![
            "Javelin::SeasonsRewardsCardDataManager::SeasonsRewardsCardDataManager",
            "Javelin::SeasonsRewardsCardDataManager::CacheAllDataTables",
            "Javelin::SeasonsRewardsCardDataManager::CacheDataTable",
            "Javelin::SeasonsRewardsCardDataManager::CacheTableRows",
            "Javelin::SeasonsRewardsCardDataManager::DecodeRow",
        ],
    })
}

pub(super) fn npc_data_manager_spec() -> NativeManagerSpec {
    table_family_crc_projection(TableFamilyCrcProjectionSpec {
        module: "npc_data",
        table_module: None,
        tables: inputs::manager_table_family_specs("crate::NPCDataManager"),
        tables_type: "NpcDataTables",
        table_type: "NpcDataTable",
        handle_type: "NpcDataHandle",
        row_alias: "NpcDataRow",
        data_type: "NpcData",
        entries_field: "npcs",
        index_field: "npcs_by_crc",
        key_field: "npc_id",
        crc_field: "npc_id_crc",
        key_column: "NpcId",
        key_getter: "npc_id",
        skip_empty_key: true,
        trim_key: false,
        reject_zero_crc: false,
        duplicate_key_policy: NativeDuplicateKeyPolicy::FirstWins,
        source_handle_field: None,
        source_handle_method: None,
        row_filters: Vec::new(),
        fields: vec![
            projection_field(
                "shop_id_key",
                "ShopId",
                "shop_id",
                NativeProjectionTransform::OptionalString,
            ),
            projection_field(
                "shop_id_crc",
                "ShopId",
                "shop_id",
                NativeProjectionTransform::OptionalLowercaseCrcString,
            ),
        ],
        table_indexes: Vec::new(),
        field_lookup_methods: vec![field_lookup(
            "shop_id",
            "npc_id",
            NativeCrcIndexLookupParameterKind::Crc32,
            "shop_id_crc",
            "Crc32",
            true,
        )],
        store_key_text: true,
        lookup_methods: vec![
            lookup(
                "npc_data",
                "npc_id",
                NativeCrcIndexLookupParameterKind::Crc32,
            ),
            lookup(
                "npc_data_by_key",
                "npc_id",
                NativeCrcIndexLookupParameterKind::AsRefStr,
            ),
        ],
        ids_method: None,
        rows_method: Some("npcs"),
        len_method: Some("len"),
        is_empty_method: Some("is_empty"),
        ghidra_class: "Javelin::NPCDataManager",
        rust_type: "crate::NPCDataManager",
        ghidra_functions: vec![
            "Javelin::NPCDataManager::NPCDataManager",
            "Javelin::NPCDataManager::CacheAllDataTables",
        ],
    })
}

pub(super) fn simple_tree_category_data_manager_spec() -> NativeManagerSpec {
    table_family_crc_projection(TableFamilyCrcProjectionSpec {
        module: "simple_tree_category_data",
        table_module: None,
        tables: inputs::manager_table_family_specs("crate::SimpleTreeCategoryDataManager"),
        tables_type: "SimpleTreeCategoryDataTables",
        table_type: "SimpleTreeCategoryDataTable",
        handle_type: "SimpleTreeCategoryDataHandle",
        row_alias: "SimpleTreeCategoryDataRow",
        data_type: "SimpleTreeCategoryData",
        entries_field: "rows",
        index_field: "rows_by_category_crc",
        key_field: "category_id",
        crc_field: "category_crc",
        key_column: "MetaAchievementCategoryId",
        key_getter: "meta_achievement_category_id",
        skip_empty_key: true,
        trim_key: false,
        reject_zero_crc: false,
        duplicate_key_policy: NativeDuplicateKeyPolicy::FirstWins,
        source_handle_field: None,
        source_handle_method: None,
        row_filters: Vec::new(),
        fields: vec![
            projection_field(
                "parent_category_crc",
                "Parent Category",
                "parent_category",
                NativeProjectionTransform::OptionalLowercaseCrcStringDefaultZero,
            ),
            projection_field(
                "index",
                "Index",
                "index",
                NativeProjectionTransform::NonZeroU32,
            ),
            projection_field("title", "Title", "title", NativeProjectionTransform::String),
            projection_field(
                "icon_color_background",
                "Icon Color Background",
                "icon_color_background",
                NativeProjectionTransform::OptionalString,
            ),
            projection_field(
                "hide_from_ui",
                "HideFromUI",
                "hide_from_ui",
                NativeProjectionTransform::OptionalBoolDefaultFalse,
            ),
        ],
        table_indexes: Vec::new(),
        field_lookup_methods: Vec::new(),
        store_key_text: true,
        lookup_methods: vec![
            lookup(
                "category_data",
                "category_id",
                NativeCrcIndexLookupParameterKind::IntoCrc32,
            ),
            lookup(
                "category_data_by_key",
                "key",
                NativeCrcIndexLookupParameterKind::AsRefStr,
            ),
        ],
        ids_method: None,
        rows_method: Some("rows"),
        len_method: Some("len"),
        is_empty_method: Some("is_empty"),
        ghidra_class: "Javelin::SimpleTreeCategoryDataManager",
        rust_type: "crate::SimpleTreeCategoryDataManager",
        ghidra_functions: vec![
            "Javelin::SimpleTreeCategoryDataManager::SimpleTreeCategoryDataManager",
            "Javelin::SimpleTreeCategoryDataManager::CacheAllDataTables",
        ],
    })
}

pub(super) fn weapon_appearance_data_manager_spec() -> NativeManagerSpec {
    table_family_crc_projection(TableFamilyCrcProjectionSpec {
        module: "weapon_appearance_data",
        table_module: None,
        tables: vec![
            TableFamilyTableSpec {
                variant: "InstrumentsAppearanceDefinitions".to_owned(),
                table_name: "InstrumentsAppearanceDefinitions",
                row_type_name: "WeaponAppearanceDefinitions",
            },
            TableFamilyTableSpec {
                variant: "WeaponAppearanceDefinitions".to_owned(),
                table_name: "WeaponAppearanceDefinitions",
                row_type_name: "WeaponAppearanceDefinitions",
            },
            TableFamilyTableSpec {
                variant: "MountAttachments".to_owned(),
                table_name: "WeaponAppearanceDefinitions_MountAttachments",
                row_type_name: "WeaponAppearanceDefinitions",
            },
        ],
        tables_type: "WeaponAppearanceDefinitionsTables",
        table_type: "WeaponAppearanceDefinitionsTable",
        handle_type: "WeaponAppearanceDefinitionsHandle",
        row_alias: "WeaponAppearanceDefinitionsRow",
        data_type: "WeaponAppearanceData",
        entries_field: "appearances",
        index_field: "appearances_by_id",
        key_field: "weapon_appearance_id",
        crc_field: "weapon_appearance_id_crc",
        key_column: "WeaponAppearanceID",
        key_getter: "weapon_appearance_id",
        skip_empty_key: true,
        trim_key: false,
        reject_zero_crc: false,
        duplicate_key_policy: NativeDuplicateKeyPolicy::Error,
        source_handle_field: None,
        source_handle_method: None,
        row_filters: Vec::new(),
        fields: Vec::new(),
        table_indexes: Vec::new(),
        field_lookup_methods: Vec::new(),
        store_key_text: true,
        lookup_methods: vec![
            lookup(
                "weapon_appearance",
                "appearance_id",
                NativeCrcIndexLookupParameterKind::AsRefStr,
            ),
            lookup(
                "weapon_appearance_by_key",
                "key",
                NativeCrcIndexLookupParameterKind::AsRefStr,
            ),
            lookup(
                "weapon_appearance_from_id",
                "appearance_id",
                NativeCrcIndexLookupParameterKind::IntoCrc32,
            ),
        ],
        ids_method: None,
        rows_method: Some("rows"),
        len_method: Some("len"),
        is_empty_method: Some("is_empty"),
        ghidra_class: "Javelin::WeaponAppearanceDataManager",
        rust_type: "crate::WeaponAppearanceDataManager",
        ghidra_functions: vec![
            "Javelin::WeaponAppearanceDataManager::WeaponAppearanceDataManager",
            "Javelin::WeaponAppearanceDataManager::CacheAllDataTables",
        ],
    })
}

pub(super) fn weapon_ref_data_manager_spec() -> NativeManagerSpec {
    table_family_crc_projection(TableFamilyCrcProjectionSpec {
        module: "weapon_item_definitions",
        table_module: None,
        tables: inputs::manager_table_family_specs("crate::WeaponRefDataManager"),
        tables_type: "WeaponItemDefinitionsTables",
        table_type: "WeaponItemDefinitionsTable",
        handle_type: "WeaponItemDefinitionsHandle",
        row_alias: "WeaponItemDefinitionsRow",
        data_type: "WeaponRefData",
        entries_field: "weapon_refs",
        index_field: "weapon_refs_by_id",
        key_field: "weapon_key",
        crc_field: "weapon_id",
        key_column: "WeaponID",
        key_getter: "weapon_id",
        skip_empty_key: true,
        trim_key: false,
        reject_zero_crc: true,
        duplicate_key_policy: NativeDuplicateKeyPolicy::FirstWins,
        source_handle_field: None,
        source_handle_method: None,
        row_filters: vec![f32_any_greater_than_zero_filter(
            "ScalingStrength",
            "scaling_strength",
            &["scaling_dexterity", "scaling_intelligence", "scaling_focus"],
        )],
        fields: vec![
            projection_field(
                "scaling_strength",
                "ScalingStrength",
                "scaling_strength",
                NativeProjectionTransform::F32,
            ),
            projection_field(
                "scaling_dexterity",
                "ScalingDexterity",
                "scaling_dexterity",
                NativeProjectionTransform::F32,
            ),
            projection_field(
                "scaling_intelligence",
                "ScalingIntelligence",
                "scaling_intelligence",
                NativeProjectionTransform::F32,
            ),
            projection_field(
                "scaling_focus",
                "ScalingFocus",
                "scaling_focus",
                NativeProjectionTransform::F32,
            ),
        ],
        table_indexes: Vec::new(),
        field_lookup_methods: Vec::new(),
        store_key_text: true,
        lookup_methods: vec![
            lookup(
                "weapon_ref_from_id",
                "weapon_id",
                NativeCrcIndexLookupParameterKind::IntoCrc32,
            ),
            lookup(
                "weapon_ref",
                "weapon_id",
                NativeCrcIndexLookupParameterKind::AsRefStr,
            ),
        ],
        ids_method: None,
        rows_method: Some("weapon_refs"),
        len_method: Some("len"),
        is_empty_method: Some("is_empty"),
        ghidra_class: "Javelin::WeaponRefDataManager",
        rust_type: "crate::WeaponRefDataManager",
        ghidra_functions: vec![
            "Javelin::WeaponRefDataManager::WeaponRefDataManager",
            "Javelin::WeaponRefDataManager::CacheAllDataTables",
        ],
    })
}

pub(super) fn weapon_item_data_manager_spec() -> NativeManagerSpec {
    table_family_crc_projection(TableFamilyCrcProjectionSpec {
        module: "weapon_item_data",
        table_module: Some("weapon_item_definitions"),
        tables: inputs::manager_table_family_specs("crate::WeaponItemDataManager"),
        tables_type: "WeaponItemDefinitionsTables",
        table_type: "WeaponItemDefinitionsTable",
        handle_type: "WeaponItemDefinitionsHandle",
        row_alias: "WeaponItemDefinitionsRow",
        data_type: "WeaponItemData",
        entries_field: "weapon_items",
        index_field: "weapon_items_by_id",
        key_field: "weapon_key",
        crc_field: "weapon_id",
        key_column: "WeaponID",
        key_getter: "weapon_id",
        skip_empty_key: true,
        trim_key: false,
        reject_zero_crc: false,
        duplicate_key_policy: NativeDuplicateKeyPolicy::Error,
        source_handle_field: None,
        source_handle_method: None,
        row_filters: Vec::new(),
        fields: vec![
            projection_field(
                "base_weapon_id",
                "BaseWeaponID",
                "base_weapon_id",
                NativeProjectionTransform::OptionalString,
            ),
            projection_field(
                "primary_use",
                "PrimaryUse",
                "primary_use",
                NativeProjectionTransform::OptionalString,
            ),
            projection_field(
                "icon_path",
                "IconPath",
                "icon_path",
                NativeProjectionTransform::OptionalString,
            ),
            projection_field(
                "max_stack_size",
                "MaxStackSize",
                "max_stack_size",
                NativeProjectionTransform::U32,
            ),
            projection_field(
                "equip_type",
                "EquipType",
                "equip_type",
                NativeProjectionTransform::OptionalString,
            ),
            projection_field(
                "damage_stat_multiplier",
                "DamageStatMultiplier",
                "damage_stat_multiplier",
                NativeProjectionTransform::OptionalF32,
            ),
            projection_field(
                "weapon_mastery_category_id",
                "WeaponMasteryCategoryId",
                "weapon_mastery_category_id",
                NativeProjectionTransform::OptionalString,
            ),
            projection_field(
                "tier_number",
                "TierNumber",
                "tier_number",
                NativeProjectionTransform::U32,
            ),
            projection_field(
                "base_damage",
                "BaseDamage",
                "base_damage",
                NativeProjectionTransform::U32,
            ),
            projection_field(
                "crit_chance",
                "CritChance",
                "crit_chance",
                NativeProjectionTransform::F32,
            ),
            projection_field(
                "crit_damage_multiplier",
                "CritDamageMultiplier",
                "crit_damage_multiplier",
                NativeProjectionTransform::F32,
            ),
            projection_field(
                "base_stagger_damage",
                "BaseStaggerDamage",
                "base_stagger_damage",
                NativeProjectionTransform::U32,
            ),
            projection_field(
                "crit_stagger_damage_multiplier",
                "CritStaggerDamageMultiplier",
                "crit_stagger_damage_multiplier",
                NativeProjectionTransform::F32,
            ),
            projection_field(
                "reticle_name",
                "ReticleName",
                "reticle_name",
                NativeProjectionTransform::OptionalString,
            ),
            projection_field(
                "reticle_target_name",
                "ReticleTargetName",
                "reticle_target_name",
                NativeProjectionTransform::OptionalString,
            ),
            projection_field(
                "reticle_ray_cast_distance",
                "ReticleRayCastDistance",
                "reticle_ray_cast_distance",
                NativeProjectionTransform::F32,
            ),
            projection_field(
                "ammo_type",
                "AmmoType",
                "ammo_type",
                NativeProjectionTransform::OptionalString,
            ),
            projection_field(
                "max_loaded_ammo",
                "MaxLoadedAmmo",
                "max_loaded_ammo",
                NativeProjectionTransform::OptionalU32,
            ),
            projection_field(
                "auto_reload_ammo_seconds",
                "AutoReloadAmmoSeconds",
                "auto_reload_ammo_seconds",
                NativeProjectionTransform::OptionalF32,
            ),
            projection_field(
                "ammo_mesh",
                "AmmoMesh",
                "ammo_mesh",
                NativeProjectionTransform::OptionalString,
            ),
            projection_field(
                "mannequin_tag",
                "MannequinTag",
                "mannequin_tag",
                NativeProjectionTransform::OptionalStringList,
            ),
            projection_field(
                "off_hand_mannequin_tag",
                "OffHandMannequinTag",
                "off_hand_mannequin_tag",
                NativeProjectionTransform::OptionalStringList,
            ),
            projection_field(
                "mesh_override",
                "MeshOverride",
                "mesh_override",
                NativeProjectionTransform::OptionalStringList,
            ),
            projection_field(
                "skin_override1",
                "SkinOverride1",
                "skin_override1",
                NativeProjectionTransform::OptionalString,
            ),
            projection_field(
                "material_override1",
                "MaterialOverride1",
                "material_override1",
                NativeProjectionTransform::OptionalString,
            ),
            projection_field(
                "skin_override2",
                "SkinOverride2",
                "skin_override2",
                NativeProjectionTransform::OptionalString,
            ),
            projection_field(
                "material_override2",
                "MaterialOverride2",
                "material_override2",
                NativeProjectionTransform::OptionalString,
            ),
            projection_field(
                "skin_override3",
                "SkinOverride3",
                "skin_override3",
                NativeProjectionTransform::OptionalString,
            ),
            projection_field(
                "material_override3",
                "MaterialOverride3",
                "material_override3",
                NativeProjectionTransform::OptionalString,
            ),
            projection_field(
                "skin_override4",
                "SkinOverride4",
                "skin_override4",
                NativeProjectionTransform::OptionalString,
            ),
            projection_field(
                "material_override4",
                "MaterialOverride4",
                "material_override4",
                NativeProjectionTransform::OptionalString,
            ),
            projection_field(
                "fire_joint",
                "FireJoint",
                "fire_joint",
                NativeProjectionTransform::OptionalString,
            ),
            projection_field(
                "damage_table_row",
                "DamageTableRow",
                "damage_table_row",
                NativeProjectionTransform::OptionalString,
            ),
            projection_field(
                "damage_table_row_id",
                "DamageTableRow",
                "damage_table_row",
                NativeProjectionTransform::OptionalLowercaseCrcStringDefaultZero,
            ),
            projection_field(
                "tooltip_primary_attack_data",
                "TooltipPrimaryAttackData",
                "tooltip_primary_attack_data",
                NativeProjectionTransform::OptionalString,
            ),
            projection_field(
                "tooltip_alternate_attack_data",
                "TooltipAlternateAttackData",
                "tooltip_alternate_attack_data",
                NativeProjectionTransform::OptionalString,
            ),
            projection_field(
                "anim_db_path",
                "AnimDbPath",
                "anim_db_path",
                NativeProjectionTransform::OptionalString,
            ),
            projection_field(
                "gathering_types",
                "GatheringTypes",
                "gathering_types",
                NativeProjectionTransform::OptionalStringList,
            ),
            projection_field(
                "gathering_multiplier",
                "GatheringMultiplier",
                "gathering_multiplier",
                NativeProjectionTransform::F32,
            ),
            projection_field(
                "gathering_efficiency",
                "GatheringEfficiency",
                "gathering_efficiency",
                NativeProjectionTransform::F32,
            ),
            projection_field(
                "min_gather_eff",
                "MinGatherEFF",
                "min_gather_eff",
                NativeProjectionTransform::OptionalF32,
            ),
            projection_field(
                "max_gather_eff",
                "MaxGatherEFF",
                "max_gather_eff",
                NativeProjectionTransform::OptionalF32,
            ),
            projection_field(
                "audio_pickup",
                "AudioPickup",
                "audio_pickup",
                NativeProjectionTransform::OptionalString,
            ),
            projection_field(
                "audio_place",
                "AudioPlace",
                "audio_place",
                NativeProjectionTransform::OptionalString,
            ),
            projection_field(
                "primary_hand",
                "Primary Hand",
                "primary_hand",
                NativeProjectionTransform::OptionalString,
            ),
            projection_field(
                "equipment_categories",
                "EquipmentCategories",
                "equipment_categories",
                NativeProjectionTransform::OptionalString,
            ),
            projection_field(
                "required_strength",
                "RequiredStrength",
                "required_strength",
                NativeProjectionTransform::U32,
            ),
            projection_field(
                "required_dexterity",
                "RequiredDexterity",
                "required_dexterity",
                NativeProjectionTransform::U32,
            ),
            projection_field(
                "required_intelligence",
                "RequiredIntelligence",
                "required_intelligence",
                NativeProjectionTransform::U32,
            ),
            projection_field(
                "required_focus",
                "RequiredFocus",
                "required_focus",
                NativeProjectionTransform::U32,
            ),
            projection_field(
                "scaling_strength",
                "ScalingStrength",
                "scaling_strength",
                NativeProjectionTransform::F32,
            ),
            projection_field(
                "scaling_dexterity",
                "ScalingDexterity",
                "scaling_dexterity",
                NativeProjectionTransform::F32,
            ),
            projection_field(
                "scaling_intelligence",
                "ScalingIntelligence",
                "scaling_intelligence",
                NativeProjectionTransform::F32,
            ),
            projection_field(
                "scaling_focus",
                "ScalingFocus",
                "scaling_focus",
                NativeProjectionTransform::F32,
            ),
            projection_field(
                "resistances",
                "Resistances",
                "resistances",
                NativeProjectionTransform::OptionalString,
            ),
            projection_field(
                "weaknesses",
                "Weaknesses",
                "weaknesses",
                NativeProjectionTransform::OptionalString,
            ),
            typed_projection_field(
                "stat_bonuses",
                "StatBonuses",
                "stat_bonuses",
                NativeProjectionTransform::OptionalTypedCell,
                "Vec<(az_core::crc::Crc32, f32)>",
            ),
            typed_projection_field(
                "stat_multipliers",
                "StatMultipliers",
                "stat_multipliers",
                NativeProjectionTransform::OptionalTypedCell,
                "Vec<(u8, f32)>",
            ),
            projection_field(
                "equipment_category_multiplier",
                "EquipmentCategoryMultiplier",
                "equipment_category_multiplier",
                NativeProjectionTransform::OptionalF32,
            ),
            projection_field(
                "attack_game_event_id",
                "AttackGameEventID",
                "attack_game_event_id",
                NativeProjectionTransform::OptionalString,
            ),
            projection_field(
                "physical_armor_set_scale_factor",
                "PhysicalArmorSetScaleFactor",
                "physical_armor_set_scale_factor",
                NativeProjectionTransform::OptionalF32,
            ),
            projection_field(
                "elemental_armor_set_scale_factor",
                "ElementalArmorSetScaleFactor",
                "elemental_armor_set_scale_factor",
                NativeProjectionTransform::OptionalF32,
            ),
            projection_field(
                "armor_rating_scale_factor",
                "ArmorRatingScaleFactor",
                "armor_rating_scale_factor",
                NativeProjectionTransform::OptionalF32,
            ),
            projection_field(
                "weight_override",
                "WeightOverride",
                "weight_override",
                NativeProjectionTransform::OptionalF32,
            ),
            projection_field(
                "block_stamina_damage",
                "BlockStaminaDamage",
                "block_stamina_damage",
                NativeProjectionTransform::U32,
            ),
            projection_field(
                "block_stability",
                "BlockStability",
                "block_stability",
                NativeProjectionTransform::U32,
            ),
            projection_field(
                "deflection_rating",
                "DeflectionRating",
                "deflection_rating",
                NativeProjectionTransform::U32,
            ),
            projection_field(
                "bla_standard",
                "BLAStandard",
                "bla_standard",
                NativeProjectionTransform::OptionalF32,
            ),
            projection_field(
                "bla_siege",
                "BLASiege",
                "bla_siege",
                NativeProjectionTransform::F32,
            ),
            projection_field(
                "bla_strike",
                "BLAStrike",
                "bla_strike",
                NativeProjectionTransform::F32,
            ),
            projection_field(
                "bla_slash",
                "BLASlash",
                "bla_slash",
                NativeProjectionTransform::F32,
            ),
            projection_field(
                "bla_thrust",
                "BLAThrust",
                "bla_thrust",
                NativeProjectionTransform::F32,
            ),
            projection_field(
                "bla_arcane",
                "BLAArcane",
                "bla_arcane",
                NativeProjectionTransform::F32,
            ),
            projection_field(
                "bla_fire",
                "BLAFire",
                "bla_fire",
                NativeProjectionTransform::F32,
            ),
            projection_field(
                "bla_ice",
                "BLAIce",
                "bla_ice",
                NativeProjectionTransform::F32,
            ),
            projection_field(
                "bla_nature",
                "BLANature",
                "bla_nature",
                NativeProjectionTransform::F32,
            ),
            projection_field(
                "bla_lightning",
                "BLALightning",
                "bla_lightning",
                NativeProjectionTransform::F32,
            ),
            projection_field(
                "bla_corruption",
                "BLACorruption",
                "bla_corruption",
                NativeProjectionTransform::F32,
            ),
            projection_field(
                "aba_poison",
                "ABAPoison",
                "aba_poison",
                NativeProjectionTransform::F32,
            ),
            projection_field(
                "aba_disease",
                "ABADisease",
                "aba_disease",
                NativeProjectionTransform::F32,
            ),
            projection_field(
                "aba_bleed",
                "ABABleed",
                "aba_bleed",
                NativeProjectionTransform::F32,
            ),
            projection_field(
                "aba_frostbite",
                "ABAFrostbite",
                "aba_frostbite",
                NativeProjectionTransform::F32,
            ),
            projection_field(
                "aba_curse",
                "ABACurse",
                "aba_curse",
                NativeProjectionTransform::F32,
            ),
            projection_field(
                "ranged_attack_profile",
                "RangedAttackProfile",
                "ranged_attack_profile",
                NativeProjectionTransform::OptionalString,
            ),
            projection_field(
                "attached_spell_data",
                "AttachedSpellData",
                "attached_spell_data",
                NativeProjectionTransform::OptionalString,
            ),
            projection_field(
                "appearance",
                "Appearance",
                "appearance",
                NativeProjectionTransform::OptionalString,
            ),
            projection_field(
                "female_appearance",
                "FemaleAppearance",
                "female_appearance",
                NativeProjectionTransform::OptionalString,
            ),
            projection_field(
                "can_block_ranged",
                "CanBlockRanged",
                "can_block_ranged",
                NativeProjectionTransform::OptionalBoolDefaultFalse,
            ),
            projection_field(
                "ranged_block_health_damage_scaling",
                "RangedBlockHealthDamageScaling",
                "ranged_block_health_damage_scaling",
                NativeProjectionTransform::OptionalF32,
            ),
            projection_field(
                "ranged_block_stamina_damage_scaling",
                "RangedBlockStaminaDamageScaling",
                "ranged_block_stamina_damage_scaling",
                NativeProjectionTransform::OptionalF32,
            ),
            projection_field(
                "mana_cost_id",
                "ManaCostId",
                "mana_cost_id",
                NativeProjectionTransform::OptionalString,
            ),
            projection_field(
                "weapon_effect_id",
                "WeaponEffectId",
                "weapon_effect_id",
                NativeProjectionTransform::OptionalString,
            ),
            projection_field(
                "base_accuracy",
                "BaseAccuracy",
                "base_accuracy",
                NativeProjectionTransform::U32,
            ),
            projection_field(
                "sound_table_id",
                "SoundTableID",
                "sound_table_id",
                NativeProjectionTransform::OptionalForeignKeyRow,
            ),
            projection_field(
                "is_shield_compatible",
                "IsShieldCompatible",
                "is_shield_compatible",
                NativeProjectionTransform::OptionalBoolDefaultFalse,
            ),
            projection_field(
                "hide_main_weapon_mesh_while_sheathed",
                "HideMainWeaponMeshWhileSheathed",
                "hide_main_weapon_mesh_while_sheathed",
                NativeProjectionTransform::OptionalBoolDefaultFalse,
            ),
            projection_field(
                "weapon_base_damage_compound_increase",
                "WeaponBaseDamageCompoundIncrease",
                "weapon_base_damage_compound_increase",
                NativeProjectionTransform::OptionalString,
            ),
            projection_field(
                "equip_ability",
                "EquipAbility",
                "equip_ability",
                NativeProjectionTransform::OptionalString,
            ),
            projection_field(
                "num_weapon_visuals",
                "NumWeaponVisuals",
                "num_weapon_visuals",
                NativeProjectionTransform::OptionalU32,
            ),
            projection_field(
                "max_num_combo_points",
                "MaxNumComboPoints",
                "max_num_combo_points",
                NativeProjectionTransform::OptionalU32,
            ),
            projection_field(
                "combo_points_clear_duration",
                "ComboPointsClearDuration",
                "combo_points_clear_duration",
                NativeProjectionTransform::OptionalF32,
            ),
        ],
        table_indexes: Vec::new(),
        field_lookup_methods: Vec::new(),
        store_key_text: true,
        lookup_methods: vec![
            lookup(
                "weapon_item_from_id",
                "weapon_id",
                NativeCrcIndexLookupParameterKind::IntoCrc32,
            ),
            lookup(
                "weapon_item",
                "weapon_id",
                NativeCrcIndexLookupParameterKind::AsRefStr,
            ),
            lookup(
                "weapon_item_by_key",
                "key",
                NativeCrcIndexLookupParameterKind::AsRefStr,
            ),
        ],
        ids_method: None,
        rows_method: Some("weapon_items"),
        len_method: Some("len"),
        is_empty_method: Some("is_empty"),
        ghidra_class: "Javelin::WeaponItemDataManager",
        rust_type: "crate::WeaponItemDataManager",
        ghidra_functions: vec![
            "Javelin::WeaponItemDataManager::WeaponItemDataManager",
            "Javelin::WeaponItemDataManager::CacheAllDataTables",
        ],
    })
}

pub(super) fn encumbrance_data_manager_spec() -> NativeManagerSpec {
    let shape = NativeOneTableEncumbranceManager::new(
        ident("encumbrance_data"),
        game_table("EncumbranceLimits"),
        game_row_type("EncumbranceData"),
        ident("EncumbranceData"),
        ident("EncumbranceLoadState"),
        ident("EncumbranceLoadValues"),
        ident("EncumbranceIndexes"),
        ident("entries"),
        column("ContainerTypeID"),
        ident("container_type_id"),
        ident("encumbrance_data_from_id"),
        ident("encumbrance_data"),
        ident("encumbrance_data_by_key"),
        ident("len"),
        ident("is_empty"),
    );

    manager_spec(
        "Javelin::EncumbranceDataManager",
        "crate::EncumbranceDataManager",
        "EncumbranceLimits",
        "EncumbranceData",
        vec!["Javelin::EncumbranceDataManager::GetEncumbranceData"],
    )
    .with_shape(NativeManagerShape::one_table_encumbrance(shape))
}

pub(super) fn darkness_data_manager_spec() -> NativeManagerSpec {
    let shape = NativeOneTableDarknessManager::new(
        ident("darkness_data"),
        game_table("DarknessDataTable"),
        game_row_type("DarknessData"),
        ident("DarknessData"),
        ident("DarknessThreshold"),
        ident("DarknessLevel"),
        ident("DarknessActivationSpec"),
        ident("DarknessGroupSpec"),
        ident("entries"),
        ident("by_crc32"),
        ident("by_source"),
        ident("source_row"),
        ident("darkness_id"),
        ident("darkness_crc32"),
        column("DarknessId"),
        ident("darkness_id"),
        ident("darkness_data_by_crc32"),
        ident("darkness_data"),
        ident("source_row"),
        ident("rows"),
        ident("len"),
        ident("is_empty"),
    );

    manager_spec(
        "Javelin::DarknessDataManager",
        "crate::DarknessDataManager",
        "DarknessDataTable",
        "DarknessData",
        vec!["Javelin::DarknessDataManager::DarknessDataManager"],
    )
    .with_shape(NativeManagerShape::one_table_darkness(shape))
}

pub(super) fn difficulty_scaling_data_manager_spec() -> NativeManagerSpec {
    let shape = NativeOneTableDifficultyScalingManager::new(
        ident("difficulty_scaling_data"),
        game_table("DifficultyScaling_WorldEncounter_Participants"),
        game_row_type("DifficultyScalingData"),
        ident("DifficultyScalingData"),
        ident("DifficultyScalingAffectedCreatureTypes"),
        ident("DifficultyScalingHealthModifier"),
        ident("entries"),
        ident("by_crc"),
        ident("source_row"),
        ident("world_encounter_id"),
        column("WorldEncounterID"),
        ident("world_encounter_id"),
        ident("difficulty_scaling_data_from_id"),
        ident("difficulty_scaling_data"),
        ident("difficulty_scaling_data_by_key"),
        ident("len"),
        ident("is_empty"),
    );

    manager_spec(
        "Javelin::DifficultyScalingDataManager",
        "crate::DifficultyScalingDataManager",
        "DifficultyScaling_WorldEncounter_Participants",
        "DifficultyScalingData",
        vec![
            "Javelin::DifficultyScalingDataManager::DifficultyScalingDataManager",
            "Javelin::DifficultyScalingDataManager::CacheAllDataTables",
        ],
    )
    .with_shape(NativeManagerShape::one_table_difficulty_scaling(shape))
}
