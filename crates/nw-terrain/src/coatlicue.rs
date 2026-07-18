//! Borrowed parsers for Coatlicue `terrain.json` and `tracts.json`.

use std::borrow::Cow;

use serde::Deserialize;

#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct TerrainSettings<'a> {
    #[serde(default, borrow)]
    pub generator_type: Option<Cow<'a, str>>,
    #[serde(default)]
    pub height_crop: Option<f32>,
    #[serde(default)]
    pub mountain_roughness: Option<f32>,
    #[serde(default)]
    pub mountain_height: Option<f32>,
    #[serde(default)]
    pub snow_minimum_slope: Option<f32>,
    #[serde(default)]
    pub snow_start_height: Option<f32>,
    #[serde(default)]
    pub valley_intensity: Option<f32>,
    #[serde(default)]
    pub ocean_level: Option<f32>,
    #[serde(default, borrow)]
    pub world_material_asset_path: Option<Cow<'a, str>>,
}

impl<'a> TerrainSettings<'a> {
    pub fn parse_bytes(bytes: &'a [u8]) -> Result<Self, serde_json::Error> {
        parse_json(bytes)
    }
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct TractsDocument<'a> {
    #[serde(default)]
    pub tractmap_cell_size: Option<u32>,
    #[serde(default)]
    pub heightmap_cell_size: Option<u32>,
    #[serde(default)]
    pub region_size: Option<u32>,
    #[serde(default, borrow)]
    pub territory_master_slice_path: Option<Cow<'a, str>>,
    #[serde(default, borrow)]
    pub world: Option<WorldConfig<'a>>,
    #[serde(default, borrow)]
    pub tracts: Vec<TractDefinition<'a>>,
    #[serde(default, borrow)]
    pub regions: Vec<RegionDefinition<'a>>,
    #[serde(default, borrow)]
    pub forced_regions: Vec<ForcedRegion<'a>>,
    #[serde(default, borrow)]
    pub global_transition: Option<TractTransition<'a>>,
    #[serde(default, borrow)]
    pub transitions: Vec<TractTransition<'a>>,
    #[serde(default, borrow)]
    pub persisted_capitals_whitelist: Vec<Cow<'a, str>>,
    #[serde(default, borrow, rename = "protectCoatlicueUUIDAllowList")]
    pub protect_coatlicue_uuid_allow_list: Vec<Cow<'a, str>>,
    #[serde(default, borrow)]
    pub claim_whitelist: Vec<Cow<'a, str>>,
    #[serde(default, borrow)]
    pub azure_well_whitelist: Vec<Cow<'a, str>>,
}

impl<'a> TractsDocument<'a> {
    pub fn parse_bytes(bytes: &'a [u8]) -> Result<Self, serde_json::Error> {
        parse_json(bytes)
    }
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct WorldConfig<'a> {
    #[serde(default, borrow)]
    pub r#type: Option<Cow<'a, str>>,
    #[serde(default)]
    pub world_origin: Option<WorldLocation>,
    #[serde(default)]
    pub width: Option<u32>,
    #[serde(default)]
    pub height: Option<u32>,
    #[serde(default)]
    pub shore_longitude: Option<u32>,
    #[serde(default)]
    pub shore_longitude_variance: Option<u32>,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct TractDefinition<'a> {
    #[serde(default, borrow)]
    pub r#type: Option<Cow<'a, str>>,
    #[serde(default)]
    pub weight: Option<u32>,
    #[serde(default, borrow)]
    pub name: Option<Cow<'a, str>>,
    #[serde(default)]
    pub display_color: Option<Rgba8>,
    #[serde(default, borrow)]
    pub map_category: Option<Cow<'a, str>>,
    #[serde(default, borrow)]
    pub ground_cover_name: Option<Cow<'a, str>>,
    #[serde(default, borrow)]
    pub time_of_day_name: Option<Cow<'a, str>>,
    #[serde(default)]
    pub height: Option<f32>,
    #[serde(default)]
    pub slope: Option<f32>,
    #[serde(default)]
    pub offset: Option<f32>,
    #[serde(default)]
    pub shape_integrity: Option<f32>,
    #[serde(default, borrow)]
    pub capitals: Vec<CapitalDefinition<'a>>,
    #[serde(default, borrow)]
    pub outposts: Vec<OutpostDefinition<'a>>,
    #[serde(default)]
    pub buildable_fraction: Option<f32>,
    #[serde(default)]
    pub plots: Vec<PlotDefinition>,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RegionDefinition<'a> {
    #[serde(default, borrow)]
    pub name: Option<Cow<'a, str>>,
    #[serde(default)]
    pub weight: Option<u32>,
    #[serde(default, borrow)]
    pub spawn_manifests: Vec<Cow<'a, str>>,
    #[serde(default, borrow)]
    pub tracts: Vec<TractDefinition<'a>>,
    #[serde(default)]
    pub type_ratios: Option<TypeRatios>,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ForcedRegion<'a> {
    #[serde(default)]
    pub location: Option<WorldLocation>,
    #[serde(default, borrow)]
    pub region_name: Option<Cow<'a, str>>,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct TractTransition<'a> {
    #[serde(default)]
    pub enabled: Option<bool>,
    #[serde(default, borrow)]
    pub buffer: Option<Cow<'a, str>>,
    #[serde(default)]
    pub buffer_relative_height: Option<f32>,
    #[serde(default, borrow)]
    pub sides: Vec<TractTransitionSide<'a>>,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct TractTransitionSide<'a> {
    #[serde(default, borrow)]
    pub tract: Option<Cow<'a, str>>,
    #[serde(default)]
    pub wall_height: Option<f32>,
    #[serde(default)]
    pub buffer_width: Option<f32>,
    #[serde(default)]
    pub time_of_day_blend_depth: Option<f32>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WorldLocation {
    pub x: i32,
    pub y: i32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Rgba8 {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    #[serde(default)]
    pub a: Option<u8>,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Deserialize)]
#[serde(rename_all = "PascalCase", deny_unknown_fields)]
pub struct TypeRatios {
    #[serde(default)]
    pub mountain: Option<f32>,
    #[serde(default)]
    pub hills: Option<f32>,
    #[serde(default)]
    pub plains: Option<f32>,
    #[serde(default)]
    pub lake: Option<f32>,
    #[serde(default)]
    pub ocean: Option<f32>,
    #[serde(default)]
    pub error: Option<f32>,
    #[serde(default)]
    pub unknown: Option<f32>,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CapitalDefinition<'a> {
    #[serde(default, borrow)]
    pub asset_id: Option<Cow<'a, str>>,
    #[serde(default, borrow)]
    pub variant_id: Option<Cow<'a, str>>,
    #[serde(default)]
    pub weight: Option<u32>,
    #[serde(default)]
    pub footprint: Option<Footprint>,
    #[serde(default)]
    pub padding: Option<f32>,
    #[serde(default)]
    pub wall_padding: Option<f32>,
    #[serde(default)]
    pub elevation_offset: Option<f32>,
    #[serde(default)]
    pub cookie: Option<Cookie>,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct OutpostDefinition<'a> {
    #[serde(default, borrow)]
    pub asset_id: Option<Cow<'a, str>>,
    #[serde(default, borrow)]
    pub variant_id: Option<Cow<'a, str>>,
    #[serde(default)]
    pub count: Option<u32>,
    #[serde(default)]
    pub footprint: Option<Footprint>,
    #[serde(default)]
    pub padding: Option<f32>,
    #[serde(default)]
    pub elevation_offset: Option<f32>,
    #[serde(default)]
    pub cookie: Option<Cookie>,
}

#[derive(Debug, Clone, Copy, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PlotDefinition {
    #[serde(default)]
    pub weight: Option<u32>,
    #[serde(default)]
    pub radius: Option<f32>,
    #[serde(default)]
    pub padding: Option<f32>,
    #[serde(default)]
    pub elevation_offset: Option<f32>,
    #[serde(default)]
    pub huddle: Option<f32>,
    #[serde(default)]
    pub cookie: Option<Cookie>,
}

#[derive(Debug, Clone, Copy, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Cookie {
    #[serde(default)]
    pub inner_radius: Option<f32>,
    #[serde(default)]
    pub outer_radius: Option<f32>,
    #[serde(default)]
    pub max_height_blend: Option<f32>,
    #[serde(default)]
    pub edge_noise: Option<f32>,
    #[serde(default)]
    pub edge_noise_feature_size: Option<f32>,
    #[serde(default)]
    pub ground_noise_scale: Option<f32>,
}

#[derive(Debug, Clone, Copy, PartialEq, Deserialize)]
#[serde(untagged)]
pub enum Footprint {
    Radius(f32),
    Rect { x: f32, y: f32 },
}

fn parse_json<'a, T>(bytes: &'a [u8]) -> Result<T, serde_json::Error>
where
    T: Deserialize<'a>,
{
    serde_json::from_slice(bytes.strip_prefix(b"\xEF\xBB\xBF").unwrap_or(bytes))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_terrain_settings_and_world_material_dependency() {
        let settings = TerrainSettings::parse_bytes(
            br#"{"generatorType":"Heightmap","oceanLevel":78.0,"worldMaterialAssetPath":"Materials/terrain/Frontend/frontend.worldmat"}"#,
        )
        .unwrap();
        assert_eq!(settings.generator_type.as_deref(), Some("Heightmap"));
        assert_eq!(
            settings.world_material_asset_path.as_deref(),
            Some("Materials/terrain/Frontend/frontend.worldmat")
        );
    }

    #[test]
    fn parses_tract_variants_and_slice_dependencies() {
        let document = TractsDocument::parse_bytes(
            br#"{
                "territoryMasterSlicePath":"slices/territories/master.dynamicslice",
                "tracts":[{"name":"starter","capitals":[{"assetId":"slices/capital.slice","variantId":"Fall/A"}]}],
                "regions":[{"name":"greenZone","spawnManifests":["greenZone"]}]
            }"#,
        )
        .unwrap();
        assert_eq!(
            document.territory_master_slice_path.as_deref(),
            Some("slices/territories/master.dynamicslice")
        );
        assert_eq!(
            document.tracts[0].capitals[0].variant_id.as_deref(),
            Some("Fall/A")
        );
    }
}
