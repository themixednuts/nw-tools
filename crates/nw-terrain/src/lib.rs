//! Parsers for New World terrain formats.

pub mod coatlicue;
pub mod heightmap;
pub mod mapsettings;
pub mod surfacemap;
pub mod terrain_material;
pub mod tractmap;
pub mod waterqt;

use std::borrow::Cow;

pub use coatlicue::{TerrainSettings, TractsDocument};
pub use heightmap::{
    HeightmapInspectionError, ParseError as HeightmapParseError, RegionHeightmap,
    RegionHeightmapFileInspectionReport, RegionHeightmapInspection,
    RegionHeightmapInspectionReport, RegionHeightmapSummary,
    SettingsError as RegionHeightmapSettingsError, WriteError as HeightmapWriteError,
    inspect_heightmap, inspect_heightmap_file, inspect_heightmap_path, summarize_heightmap,
};
pub use mapsettings::{MapSettings, ParseError as MapSettingsParseError};
pub use surfacemap::{
    Cell, Cells, NO_LAYER, ParseError as SurfaceMapParseError, SurfaceMap,
    SurfaceMapFileInspectionReport, SurfaceMapHistogramRow, SurfaceMapInspection,
    SurfaceMapInspectionError, SurfaceMapInspectionReportOptions, VERSION, inspect_surface_map,
    inspect_surface_map_file, inspect_surface_map_path,
};
pub use terrain_material::{
    TerrainMaterialError, parse_region_material_data_asset, parse_world_material_data_asset,
};
pub use tractmap::{
    TractMap, TractMapError, TractMapSummary, TractMapTags, is_tract_map_name, summarize_tract_map,
};
pub use waterqt::{
    ParseError as WaterQuadtreeParseError, WaterNodeSummary, WaterQuadtreeSummary,
    parse_water_quadtree, summarize_water_quadtree,
};

/// Region metadata parsed from a path containing `regions/r_+XX_+YY`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RegionPathMeta {
    pub level: String,
    pub x: i32,
    pub y: i32,
}

impl RegionPathMeta {
    #[must_use]
    pub fn parse(path: impl AsRef<str>) -> Option<Self> {
        let path = path.as_ref();
        let normalized = if path.contains('\\') {
            Cow::Owned(path.replace('\\', "/"))
        } else {
            Cow::Borrowed(path)
        };
        let mut previous_level: Option<&str> = None;
        let mut previous: Option<&str> = None;
        for part in normalized.split('/').filter(|part| !part.is_empty()) {
            if previous == Some("regions") {
                let (x, y) = parse_region_segment(part)?;
                return Some(Self {
                    level: previous_level?.to_owned(),
                    x,
                    y,
                });
            }
            previous_level = previous;
            previous = Some(part);
        }
        None
    }
}

fn parse_region_segment(segment: &str) -> Option<(i32, i32)> {
    let coordinates = segment.strip_prefix("r_")?;
    let mut parts = coordinates.split('_');
    Some((parts.next()?.parse().ok()?, parts.next()?.parse().ok()?))
}

#[cfg(test)]
mod tests {
    use super::RegionPathMeta;

    #[test]
    fn parses_region_path_metadata() {
        assert_eq!(
            RegionPathMeta::parse("sharedassets/coatlicue/world/regions/r_+02_-03/heightmap.dat"),
            Some(RegionPathMeta {
                level: "world".to_owned(),
                x: 2,
                y: -3,
            })
        );
    }
}
