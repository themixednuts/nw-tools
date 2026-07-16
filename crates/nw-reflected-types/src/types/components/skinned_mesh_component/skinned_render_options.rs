use crate::az::rtti::AzRtti;
use crate::az::uuid::Uuid as AzUuid;
use bevy_reflect::{ReflectDeserialize, ReflectSerialize};

#[derive(
    Debug, Default, Clone, PartialEq, serde::Serialize, serde::Deserialize, bevy_reflect::Reflect,
)]
#[reflect(Serialize, Deserialize)]
pub struct SkinnedRenderOptions {
    #[serde(rename = "Opacity", default)]
    pub opacity: f32,
    #[serde(rename = "MaxViewDistance", default)]
    pub max_view_distance: f32,
    #[serde(rename = "ViewDistanceMultiplier", default)]
    pub view_distance_multiplier: f32,
    #[serde(rename = "LODRatio", default)]
    pub lod_ratio: u32,
    #[serde(rename = "CastDynamicShadows", default)]
    pub cast_dynamic_shadows: bool,
    #[serde(rename = "UseVisAreas", default)]
    pub use_vis_areas: bool,
    #[serde(rename = "RainOccluder", default)]
    pub rain_occluder: bool,
    #[serde(rename = "AcceptDecals", default)]
    pub accept_decals: bool,
    #[serde(rename = "AcceptSnow", default)]
    pub accept_snow: bool,
    #[serde(rename = "AlwaysRender", default)]
    pub always_render: bool,
    #[serde(rename = "Lod_MinScreenPct", default)]
    pub lod_min_screen_pct: Vec<f32>,
    #[serde(rename = "SortType", default)]
    pub sort_type: u8,
    #[serde(rename = "AcceptSilhouette", default)]
    pub accept_silhouette: bool,
    #[serde(rename = "MirrorPlane", default)]
    pub mirror_plane: u64,
    #[serde(rename = "DrawCallStatsFormat", default)]
    pub draw_call_stats_format: String,
    #[serde(rename = "CurrentTotalDrawCallStats", default)]
    pub current_total_draw_call_stats: String,
    #[serde(rename = "CurrentIndividualDrawCallStats", default)]
    pub current_individual_draw_call_stats: std::collections::BTreeMap<String, String>,
    #[serde(rename = "EditorRefreshButton", default)]
    pub editor_refresh_button: bool,
}

impl AzRtti for SkinnedRenderOptions {
    const NAME: &'static str = "SkinnedRenderOptions";
    const TYPE_ID: AzUuid = AzUuid::from_u128(0x33E69F1C_518F_4DD2_88D1_DF6D12ECA54E);
}
