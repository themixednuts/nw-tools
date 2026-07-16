use crate::az::asset::Asset as AzAsset;
use crate::az::rtti::AzRtti;
use crate::az::uuid::Uuid as AzUuid;
use crate::types::{SimpleAssetReferenceMaterialDataAsset, TerrainValidationData};
use bevy_reflect::{ReflectDeserialize, ReflectSerialize};

#[derive(
    Debug, Default, Clone, PartialEq, serde::Serialize, serde::Deserialize, bevy_reflect::Reflect,
)]
#[reflect(Serialize, Deserialize)]
pub struct StructurePlacementData {
    #[serde(rename = "Grid Box Size", default)]
    pub grid_box_size: f32,
    #[serde(rename = "Grid Box Height", default)]
    pub grid_box_height: f32,
    #[serde(rename = "Is Placing Sticky Factor", default)]
    pub is_placing_sticky_factor: f32,
    #[serde(rename = "Is Placed Sticky Factor", default)]
    pub is_placed_sticky_factor: f32,
    #[serde(rename = "Min Build Distance From Player", default)]
    pub min_build_distance_from_player: f32,
    #[serde(rename = "Max Build Distance From Player", default)]
    pub max_build_distance_from_player: f32,
    #[serde(rename = "Min Build Pitch Percent", default)]
    pub min_build_pitch_percent: f32,
    #[serde(rename = "Max Build Pitch Percent", default)]
    pub max_build_pitch_percent: f32,
    #[serde(rename = "Max Build Vertical Distance From Player", default)]
    pub max_build_vertical_distance_from_player: f32,
    #[serde(
        rename = "Placement Obstruction Update Frequency Time In Secs",
        default
    )]
    pub placement_obstruction_update_frequency_time_in_secs: f32,
    #[serde(rename = "Placing Settings", default)]
    pub placing_settings: TerrainValidationData,
    #[serde(rename = "Snapped To Settings", default)]
    pub snapped_to_settings: TerrainValidationData,
    #[serde(rename = "Snap Point Mesh File Name", default)]
    pub snap_point_mesh_file_name: AzAsset,
    #[serde(rename = "Grid Footprint Mesh File Name", default)]
    pub grid_footprint_mesh_file_name: AzAsset,
    #[serde(rename = "Valid Placement Material File Name", default)]
    pub valid_placement_material_file_name: SimpleAssetReferenceMaterialDataAsset,
    #[serde(rename = "Invalid Placement Material File Name", default)]
    pub invalid_placement_material_file_name: SimpleAssetReferenceMaterialDataAsset,
    #[serde(rename = "Completion Blocked Material File Name", default)]
    pub completion_blocked_material_file_name: SimpleAssetReferenceMaterialDataAsset,
    #[serde(rename = "Snap Valid Placement Material File Name", default)]
    pub snap_valid_placement_material_file_name: SimpleAssetReferenceMaterialDataAsset,
    #[serde(rename = "Snap Invalid Placement Material File Name", default)]
    pub snap_invalid_placement_material_file_name: SimpleAssetReferenceMaterialDataAsset,
    #[serde(rename = "Snap Completion Blocked Material File Name", default)]
    pub snap_completion_blocked_material_file_name: SimpleAssetReferenceMaterialDataAsset,
    #[serde(rename = "Placement Obstruction Filter", default)]
    pub placement_obstruction_filter: String,
    #[serde(rename = "Completion Obstruction Filter", default)]
    pub completion_obstruction_filter: String,
    #[serde(rename = "LOS Obstruction Filter", default)]
    pub los_obstruction_filter: String,
    #[serde(rename = "LOS Box Height", default)]
    pub los_box_height: f32,
    #[serde(rename = "LOS Box Width", default)]
    pub los_box_width: f32,
}

impl AzRtti for StructurePlacementData {
    const NAME: &'static str = "StructurePlacementData";
    const TYPE_ID: AzUuid = AzUuid::from_u128(0x0AAC0D48_9308_4ADB_B6CA_B1DCAAC61AA5);
}
