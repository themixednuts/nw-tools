//! Terrain region/world material ObjectStream parsers.
//!
//! The decoded values are the SerializeContext-generated wire types. Path
//! transformation belongs to callers, so asset IDs, types, and authored hints
//! remain intact here.

use bevy_color::LinearRgba;
use nw_objectstream::{
    Element, ObjectStream, ObjectStreamError,
    asset_reference::{AssetValueError, read_asset_value},
    query::child_by_field_ignore_case_or_crc,
    value::{self, DecodeAzValue, ObjectStreamValueError},
};
use nw_reflected_types::{
    az::{
        asset::{Asset, AssetId},
        crc::Crc32,
        rtti::AzRtti,
    },
    types::{
        RegionMaterialDataAsset, SerializableMacroMaterialParams, TerrainMaterialLayerData,
        TileMaterialData, WorldMaterialDataAsset,
    },
};
use thiserror::Error;
use uuid::Uuid;

pub fn parse_region_material_data_asset(
    bytes: &[u8],
) -> Result<RegionMaterialDataAsset, TerrainMaterialError> {
    let stream = ObjectStream::from_bytes(bytes, None)?;
    let root = root_element(&stream, *RegionMaterialDataAsset::TYPE_ID.as_inner())?;
    read_region_material_data_asset(root)
}

pub fn parse_world_material_data_asset(
    bytes: &[u8],
) -> Result<WorldMaterialDataAsset, TerrainMaterialError> {
    let stream = ObjectStream::from_bytes(bytes, None)?;
    let root = root_element(&stream, *WorldMaterialDataAsset::TYPE_ID.as_inner())?;
    read_world_material_data_asset(root)
}

#[derive(Debug, Error)]
pub enum TerrainMaterialError {
    #[error("parse terrain material ObjectStream")]
    ObjectStream(#[from] ObjectStreamError),
    #[error("terrain material ObjectStream has no root element")]
    MissingRoot,
    #[error("expected root type {expected}, got {actual}")]
    UnexpectedRoot { expected: Uuid, actual: Uuid },
    #[error("terrain material field `{field}` could not be read")]
    Field {
        field: &'static str,
        #[source]
        source: ObjectStreamValueError,
    },
    #[error("terrain material asset `{field}` could not be read")]
    Asset {
        field: &'static str,
        #[source]
        source: AssetValueError,
    },
}

fn root_element(stream: &ObjectStream, expected: Uuid) -> Result<&Element, TerrainMaterialError> {
    let root = stream
        .elements()
        .first()
        .ok_or(TerrainMaterialError::MissingRoot)?;
    if root.id() != &expected {
        return Err(TerrainMaterialError::UnexpectedRoot {
            expected,
            actual: *root.id(),
        });
    }
    Ok(root)
}

fn read_region_material_data_asset(
    element: &Element,
) -> Result<RegionMaterialDataAsset, TerrainMaterialError> {
    let layers = child(element, "Layers")
        .map(|layers| {
            layers
                .children()
                .iter()
                .filter(|entry| {
                    entry.id() == TerrainMaterialLayerData::TYPE_ID.as_inner()
                        && field_eq(entry, "element")
                })
                .map(read_terrain_material_layer_data)
                .collect()
        })
        .transpose()?
        .unwrap_or_default();

    Ok(RegionMaterialDataAsset {
        layers,
        default_material: read_asset(element, "Default Material")?.unwrap_or_default(),
        macro_color_map: read_asset(element, "Macro ColorMap")?.unwrap_or_default(),
        macro_gloss_map: read_asset(element, "Macro GlossMap")?.unwrap_or_default(),
        macro_normal_map: read_asset(element, "Macro NormalMap")?.unwrap_or_default(),
        pertinent_layers_mip_chain: child(element, "PertinentLayersMipChain")
            .map(read_u64_vector)
            .transpose()?
            .unwrap_or_default(),
        enable_custom_background_params: field_value(element, "EnableCustomBackgroundParams")?
            .unwrap_or(false),
        macro_material_params: child(element, "MacroMaterialParams")
            .map(read_macro_material_params)
            .transpose()?
            .unwrap_or_default(),
        enable_custom_foreground_params: field_value(element, "EnableCustomForegroundParams")?
            .unwrap_or(false),
        custom_macro_material_compositing_params: child(
            element,
            "CustomMacroMaterialCompositingParams",
        )
        .map(read_macro_material_params)
        .transpose()?
        .unwrap_or_default(),
    })
}

fn read_world_material_data_asset(
    element: &Element,
) -> Result<WorldMaterialDataAsset, TerrainMaterialError> {
    let regions = child(element, "Regions")
        .map(|regions| {
            regions
                .children()
                .iter()
                .filter(|entry| {
                    entry.id() == TileMaterialData::TYPE_ID.as_inner() && field_eq(entry, "element")
                })
                .map(read_tile_material_data)
                .collect()
        })
        .transpose()?
        .unwrap_or_default();

    Ok(WorldMaterialDataAsset {
        regions,
        background_macro_material_params: child(element, "BackgroundMacroMaterialParams")
            .map(read_macro_material_params)
            .transpose()?
            .unwrap_or_default(),
        foreground_macro_material_params: child(element, "ForegroundMacroMaterialParams")
            .map(read_macro_material_params)
            .transpose()?
            .unwrap_or_default(),
        pom_height_bias: field_value(element, "POMHeightBias")?.unwrap_or(0.0),
        pom_displacement: field_value(element, "POMDisplacement")?.unwrap_or(0.0),
        pom_self_shadow_strength: field_value(element, "POMSelfShadowStrength")?.unwrap_or(0.0),
    })
}

fn read_terrain_material_layer_data(
    element: &Element,
) -> Result<TerrainMaterialLayerData, TerrainMaterialError> {
    Ok(TerrainMaterialLayerData {
        material: read_asset(element, "Material")?.unwrap_or_default(),
        splat_map: read_asset(element, "SplatMap")?.unwrap_or_default(),
        affected_tiles: field_value(element, "AffectedTiles")?.unwrap_or_default(),
        priority: field_value(element, "Priority")?.unwrap_or_default(),
    })
}

fn read_tile_material_data(element: &Element) -> Result<TileMaterialData, TerrainMaterialError> {
    Ok(TileMaterialData {
        tile_x: field_value(element, "Tile X")?.unwrap_or_default(),
        tile_y: field_value(element, "Tile Y")?.unwrap_or_default(),
        layers: read_asset(element, "Layers")?.unwrap_or_default(),
    })
}

fn read_macro_material_params(
    element: &Element,
) -> Result<SerializableMacroMaterialParams, TerrainMaterialError> {
    Ok(SerializableMacroMaterialParams {
        macro_color_scale: field_value(element, "MacroColorScale")?.unwrap_or(1.0),
        macro_color: read_color(element, "MacroColor")?.unwrap_or(LinearRgba::WHITE),
        macro_gloss_scale: field_value(element, "MacroGlossScale")?.unwrap_or(1.0),
        macro_normal_scale: field_value(element, "MacroNormalScale")?.unwrap_or(1.0),
        macro_specular_reflectance: field_value(element, "MacroSpecularReflectance")?
            .unwrap_or(0.03),
    })
}

fn read_asset(
    element: &Element,
    field: &'static str,
) -> Result<Option<Asset>, TerrainMaterialError> {
    child(element, field)
        .map(|element| {
            let asset = read_asset_value(element)
                .map_err(|source| TerrainMaterialError::Asset { field, source })?;
            Ok(Asset::new(
                AssetId::new(asset.guid().into(), asset.sub_id()),
                asset.asset_type().into(),
                (!asset.hint().trim().is_empty()).then(|| asset.hint().to_owned()),
            ))
        })
        .transpose()
}

fn field_value<'a, T>(
    element: &'a Element,
    field: &'static str,
) -> Result<Option<T>, TerrainMaterialError>
where
    T: DecodeAzValue<'a>,
{
    child(element, field)
        .map(T::decode_az_value)
        .transpose()
        .map_err(|source| TerrainMaterialError::Field { field, source })
}

fn read_color(
    element: &Element,
    field: &'static str,
) -> Result<Option<LinearRgba>, TerrainMaterialError> {
    child(element, field)
        .map(value::read_color)
        .transpose()
        .map(|color| color.map(|[r, g, b, a]| LinearRgba::new(r, g, b, a)))
        .map_err(|source| TerrainMaterialError::Field { field, source })
}

fn read_u64_vector(element: &Element) -> Result<Vec<u64>, TerrainMaterialError> {
    element
        .children()
        .iter()
        .map(value::read_u64)
        .collect::<Result<_, _>>()
        .map_err(|source| TerrainMaterialError::Field {
            field: "PertinentLayersMipChain",
            source,
        })
}

fn child<'a>(element: &'a Element, field: &str) -> Option<&'a Element> {
    child_by_field_ignore_case_or_crc(element, field, field_crc(field))
}

fn field_eq(element: &Element, field: &str) -> bool {
    element
        .field()
        .is_some_and(|actual| actual.eq_ignore_ascii_case(field))
        || element.name_crc() == Some(field_crc(field))
}

fn field_crc(field: &str) -> u32 {
    Crc32::from_str_lower(field).value()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_region_material_into_generated_types() {
        let bytes = br#"<ObjectStream version="3">
    <Class name="RegionMaterialDataAsset" version="3" type="{9A623978-DFB6-4CC1-A649-1F172637E52A}">
        <Class name="AZStd::vector" field="Layers" type="{64EC4A5C-8FC7-57FF-8C3F-8FC53807B1DB}">
            <Class name="TerrainMaterialLayerData" field="element" version="5" type="{180454CF-AD7E-440B-91F9-A071574422F4}">
                <Class name="Asset" field="Material" value="id={1E9A1948-F2A6-5500-B918-964558497331}:0,type={F46985B5-F7FF-4FCB-8E8C-DC240D701841},hint={materials/terrain/rock.mtl}" version="1" type="{77A19D40-8731-4D3C-9041-1B43047366A4}"/>
                <Class name="Asset" field="SplatMap" value="id={682044C2-982D-5580-9469-2EA56D488F80}:0,type={59D5E20B-34DB-4D8E-B867-D33CC2556355},hint={materials/terrain/rock_splat.dds}" version="1" type="{77A19D40-8731-4D3C-9041-1B43047366A4}"/>
                <Class name="AZ::u64" field="AffectedTiles" value="42" type="{D6597933-47CD-4FC8-B911-63F3E2B0993A}"/>
                <Class name="unsigned char" field="Priority" value="14" type="{72B9409A-7D1A-4831-9CFE-FCB3FADD3426}"/>
            </Class>
        </Class>
    </Class>
</ObjectStream>"#;

        let asset = parse_region_material_data_asset(bytes).unwrap();
        assert_eq!(asset.layers.len(), 1);
        assert_eq!(
            asset.layers[0].material.hint(),
            Some("materials/terrain/rock.mtl")
        );
        assert_eq!(
            asset.layers[0].splat_map.hint(),
            Some("materials/terrain/rock_splat.dds")
        );
        assert_eq!(asset.layers[0].affected_tiles, 42);
        assert_eq!(asset.layers[0].priority, 14);
    }

    #[test]
    fn parses_world_material_variants_and_region_asset() {
        let bytes = br#"<ObjectStream version="3">
    <Class name="WorldMaterialDataAsset" version="1" type="{0C5DEBF7-4320-42AB-B77B-B7270D04206A}">
        <Class name="AZStd::vector" field="Regions" type="{64EC4A5C-8FC7-57FF-8C3F-8FC53807B1DB}">
            <Class name="TileMaterialData" field="element" type="{7C65441F-6B36-444F-A722-BE103F85BFAE}">
                <Class name="int" field="Tile X" value="-2" type="{72039442-EB38-4D42-A1AD-CB68F7E0EEF6}"/>
                <Class name="int" field="Tile Y" value="3" type="{72039442-EB38-4D42-A1AD-CB68F7E0EEF6}"/>
                <Class name="Asset" field="Layers" value="id={1E9A1948-F2A6-5500-B918-964558497331}:0,type={9A623978-DFB6-4CC1-A649-1F172637E52A},hint={materials/terrain/world/r_-02_+03.regionmat}" version="1" type="{77A19D40-8731-4D3C-9041-1B43047366A4}"/>
            </Class>
        </Class>
    </Class>
</ObjectStream>"#;

        let asset = parse_world_material_data_asset(bytes).unwrap();
        assert_eq!(asset.regions.len(), 1);
        assert_eq!((asset.regions[0].tile_x, asset.regions[0].tile_y), (-2, 3));
        assert_eq!(
            asset.regions[0].layers.hint(),
            Some("materials/terrain/world/r_-02_+03.regionmat")
        );
    }
}
