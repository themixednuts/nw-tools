//! Typed dependency-resource decoding: terrain, vegetation, region, slice
//! metadata, cloth/physics-material documents, and scene-physics embedding.
//!
//! Split out of `model_asset` as a pure move; shared helpers stay in the parent.

use super::physics::parse_scene_physics;
use super::*;

pub(super) fn dependency_resource_kind(path: &str) -> Option<nw_model::CryEmbeddedResourceKind> {
    let lowercase = path.to_ascii_lowercase();
    if lowercase.ends_with("mapsettings.json") {
        return Some(nw_model::CryEmbeddedResourceKind::TerrainMapSettings);
    }
    if lowercase.ends_with("tractmap.tif") {
        return Some(nw_model::CryEmbeddedResourceKind::TerrainTractMap);
    }
    if lowercase.ends_with("terrain.json") {
        return Some(nw_model::CryEmbeddedResourceKind::TerrainSettings);
    }
    if lowercase.ends_with("tracts.json") {
        return Some(nw_model::CryEmbeddedResourceKind::TerrainTracts);
    }
    if lowercase.ends_with(".slice.meta") {
        return Some(nw_model::CryEmbeddedResourceKind::SliceMetadata);
    }
    if lowercase.contains("/regions/") && lowercase.ends_with("/region.metadata") {
        return Some(nw_model::CryEmbeddedResourceKind::RegionMetadata);
    }
    if lowercase.contains("/regions/") && lowercase.ends_with(".chunks") {
        return Some(nw_model::CryEmbeddedResourceKind::RegionChunks);
    }
    let kind = match source_extension(path).as_str() {
        "rnr" => nw_model::CryEmbeddedResourceKind::RockNRollShape,
        "cloth" => nw_model::CryEmbeddedResourceKind::NvClothFabric,
        "clothmaterial" => nw_model::CryEmbeddedResourceKind::NvClothMaterial,
        "vshapec" => nw_model::CryEmbeddedResourceKind::VertexShape,
        "collisionfilters" => nw_model::CryEmbeddedResourceKind::CollisionFilters,
        "physicsmaterialset" => nw_model::CryEmbeddedResourceKind::PhysicsMaterialSet,
        "rig" => nw_model::CryEmbeddedResourceKind::CharacterRig,
        "phys" => nw_model::CryEmbeddedResourceKind::CharacterPhysics,
        "fxl" => nw_model::CryEmbeddedResourceKind::FaceLibrary,
        // Animations (caf/i_caf/dba) are not embedded as raw dependency
        // resources: each clip ships as its glTF channel buffer at its catalog
        // path (see `gltf::append_animation`). They remain model-context assets
        // via `is_model_context_asset`'s explicit extension match.
        "heightmap" => nw_model::CryEmbeddedResourceKind::TerrainHeightmap,
        "surfacemap" => nw_model::CryEmbeddedResourceKind::TerrainSurfaceMap,
        "waterqt" => nw_model::CryEmbeddedResourceKind::TerrainWaterQuadtree,
        "regionmat" => nw_model::CryEmbeddedResourceKind::TerrainRegionMaterial,
        "worldmat" => nw_model::CryEmbeddedResourceKind::TerrainWorldMaterial,
        "terrain" => nw_model::CryEmbeddedResourceKind::TerrainSettings,
        "distribution" => nw_model::CryEmbeddedResourceKind::VegetationDistribution,
        "vegetation" => nw_model::CryEmbeddedResourceKind::VegetationRegion,
        "vegimage" => nw_model::CryEmbeddedResourceKind::VegetationImage,
        "slicedata" => nw_model::CryEmbeddedResourceKind::RegionSliceData,
        "slice" | "dynamicslice" | "entity" | "entities" | "entities_xml" | "prefab" => {
            nw_model::CryEmbeddedResourceKind::LegacyObjectStreamScene
        }
        _ => return None,
    };
    Some(kind)
}

fn typed_dependency_document(
    kind: nw_model::CryEmbeddedResourceKind,
    bytes: &[u8],
) -> Result<Option<(nw_model::CrySourceAssetKind, serde_json::Value)>> {
    let source = match kind {
        nw_model::CryEmbeddedResourceKind::NvClothFabric => (
            nw_model::CrySourceAssetKind::NvClothFabric,
            serde_json::to_value(nv_cloth_assets::parse_cloth_fabric(bytes)?)?,
        ),
        nw_model::CryEmbeddedResourceKind::NvClothMaterial => (
            nw_model::CrySourceAssetKind::NvClothMaterial,
            serde_json::to_value(nv_cloth_assets::parse_cloth_material(bytes)?)?,
        ),
        nw_model::CryEmbeddedResourceKind::VertexShape => (
            nw_model::CrySourceAssetKind::VertexShape,
            serde_json::to_value(lmbr_central_vshape::parse_vertex_shape(bytes)?)?,
        ),
        nw_model::CryEmbeddedResourceKind::CollisionFilters => (
            nw_model::CrySourceAssetKind::CollisionFilters,
            serde_json::to_value(az_physics_assets::CollisionFiltersAsset::parse(bytes)?)?,
        ),
        nw_model::CryEmbeddedResourceKind::PhysicsMaterialSet => (
            nw_model::CrySourceAssetKind::PhysicsMaterialSet,
            serde_json::to_value(az_physics_assets::MaterialSetAsset::parse(bytes)?)?,
        ),
        nw_model::CryEmbeddedResourceKind::TerrainHeightmap => {
            let summary = nw_terrain::RegionHeightmap::parse_tiff(bytes)?.summary();
            (
                nw_model::CrySourceAssetKind::TerrainHeightmap,
                serde_json::json!({
                    "width": summary.width,
                    "height": summary.height,
                    "samples": summary.samples,
                    "square": summary.square,
                    "minSample": summary.min_sample,
                    "maxSample": summary.max_sample,
                }),
            )
        }
        nw_model::CryEmbeddedResourceKind::TerrainSurfaceMap => {
            let summary = nw_terrain::surfacemap::summarize_surface_map(bytes)?;
            (
                nw_model::CrySourceAssetKind::TerrainSurfaceMap,
                serde_json::json!({
                    "bytes": summary.bytes,
                    "version": summary.version,
                    "layerIdBits": summary.layer_id_bits,
                    "gridDimension": summary.grid_dim,
                    "materialCount": summary.material_count,
                    "splatBytes": summary.splat_bytes,
                    "trivialLayer": summary.trivial_layer,
                }),
            )
        }
        nw_model::CryEmbeddedResourceKind::TerrainMapSettings => {
            let settings = nw_terrain::MapSettings::parse_json(bytes)?;
            (
                nw_model::CrySourceAssetKind::TerrainMapSettings,
                serde_json::json!({
                    "cellResolution": settings.cell_resolution,
                    "regionSize": settings.region_size,
                    "regionType": settings.region_type,
                }),
            )
        }
        nw_model::CryEmbeddedResourceKind::TerrainWaterQuadtree => (
            nw_model::CrySourceAssetKind::TerrainWaterQuadtree,
            serde_json::to_value(nw_terrain::parse_water_quadtree(bytes)?)?,
        ),
        nw_model::CryEmbeddedResourceKind::TerrainTractMap => {
            let summary = nw_terrain::summarize_tract_map(bytes)?;
            (
                nw_model::CrySourceAssetKind::TerrainTractMap,
                serde_json::json!({
                    "width": summary.width,
                    "height": summary.height,
                    "samples": summary.samples,
                    "minTract": summary.min_tract,
                    "maxTract": summary.max_tract,
                }),
            )
        }
        nw_model::CryEmbeddedResourceKind::TerrainRegionMaterial => (
            nw_model::CrySourceAssetKind::TerrainRegionMaterial,
            serde_json::to_value(nw_terrain::parse_region_material_data_asset(bytes)?)?,
        ),
        nw_model::CryEmbeddedResourceKind::TerrainWorldMaterial => (
            nw_model::CrySourceAssetKind::TerrainWorldMaterial,
            serde_json::to_value(nw_terrain::parse_world_material_data_asset(bytes)?)?,
        ),
        nw_model::CryEmbeddedResourceKind::TerrainSettings => {
            nw_terrain::TerrainSettings::parse_bytes(bytes)?;
            (
                nw_model::CrySourceAssetKind::TerrainSettings,
                serde_json::from_slice(strip_utf8_bom(bytes))?,
            )
        }
        nw_model::CryEmbeddedResourceKind::TerrainTracts => {
            nw_terrain::TractsDocument::parse_bytes(bytes)?;
            (
                nw_model::CrySourceAssetKind::TerrainTracts,
                serde_json::from_slice(strip_utf8_bom(bytes))?,
            )
        }
        nw_model::CryEmbeddedResourceKind::VegetationDistribution => {
            let distribution = nw_vegetation::distribution::Distribution::parse(bytes)?;
            let summary = distribution.summary();
            let descriptors = distribution
                .entries()
                .iter()
                .map(|entry| {
                    serde_json::json!({
                        "slicePath": entry.dynamic_slice_source_path(),
                        "variant": entry.variant,
                        "variantMetadataPath": entry.variant_metadata_source_path(),
                    })
                })
                .collect::<Vec<_>>();
            (
                nw_model::CrySourceAssetKind::VegetationDistribution,
                serde_json::json!({
                    "layout": summary.layout.to_string(),
                    "entries": summary.entries,
                    "primaryPlacements": summary.primary_placements,
                    "pointLayerCounts": summary.point_layer_counts,
                    "hasHeightModes": summary.has_height_modes,
                    "bytes": summary.bytes,
                    "descriptors": descriptors,
                }),
            )
        }
        nw_model::CryEmbeddedResourceKind::VegetationRegion => {
            let summary = nw_vegetation::summarize_vegetation_image(bytes)?;
            (
                nw_model::CrySourceAssetKind::VegetationRegion,
                serde_json::json!({
                    "assetEntries": summary.asset_entries,
                    "emptyBlocks": summary.empty_blocks,
                    "cellBlocks": summary.cell_blocks,
                    "cellGroups": summary.cell_groups,
                    "instances": summary.instances,
                }),
            )
        }
        nw_model::CryEmbeddedResourceKind::VegetationImage => {
            let summary = nw_vegetation::summarize_vegetation_image_asset(bytes)?;
            (
                nw_model::CrySourceAssetKind::VegetationImage,
                serde_json::json!({
                    "width": summary.width,
                    "height": summary.height,
                    "format": summary.format.to_string(),
                    "dataBytes": summary.data_bytes,
                }),
            )
        }
        nw_model::CryEmbeddedResourceKind::SliceMetadata => {
            let stream =
                nw_objectstream::ObjectStream::from_bytes(bytes, Some(&OBJECTSTREAM_LOOKUP))?;
            let metadata = nw_objectstream::slice_meta::read_slice_meta_data(&stream)?;
            (
                nw_model::CrySourceAssetKind::SliceMetadata,
                slice_metadata_document(&metadata),
            )
        }
        nw_model::CryEmbeddedResourceKind::RegionSliceData => {
            let stream =
                nw_objectstream::ObjectStream::from_bytes(bytes, Some(&OBJECTSTREAM_LOOKUP))?;
            let lookup = nw_objectstream::region_slice_data::read_region_slice_data(&stream)?;
            let entries = lookup
                .entries
                .iter()
                .map(|entry| {
                    let slice_path = nw_vegetation::distribution::dynamic_slice_source_path(
                        entry.key.slice_name,
                    )
                    .map(|path| path.into_owned());
                    let variant_metadata_path = slice_path.as_deref().and_then(|path| {
                        nw_vegetation::distribution::slice_metadata_source_path(
                            path,
                            entry.key.variant_name,
                        )
                    });
                    serde_json::json!({
                        "slicePath": slice_path,
                        "variant": entry.key.variant_name,
                        "variantMetadataPath": variant_metadata_path,
                        "metadata": slice_metadata_document(&entry.metadata),
                    })
                })
                .collect::<Vec<_>>();
            (
                nw_model::CrySourceAssetKind::RegionSliceData,
                serde_json::json!({ "entries": entries }),
            )
        }
        nw_model::CryEmbeddedResourceKind::RegionMetadata => (
            nw_model::CrySourceAssetKind::RegionMetadata,
            serde_json::to_value(nw_scene::parse_region_metadata(
                bytes,
                Some(&OBJECTSTREAM_LOOKUP),
            )?)?,
        ),
        nw_model::CryEmbeddedResourceKind::RegionChunks => (
            nw_model::CrySourceAssetKind::RegionChunks,
            serde_json::to_value(nw_scene::parse_region_chunks(
                bytes,
                Some(&OBJECTSTREAM_LOOKUP),
            )?)?,
        ),
        _ => return Ok(None),
    };
    Ok(Some(source))
}

fn strip_utf8_bom(bytes: &[u8]) -> &[u8] {
    bytes.strip_prefix(b"\xEF\xBB\xBF").unwrap_or(bytes)
}

fn slice_metadata_document(
    metadata: &nw_objectstream::slice_meta::SliceMetaData<'_>,
) -> serde_json::Value {
    let meshes = metadata
        .meshes
        .iter()
        .map(|mesh| {
            serde_json::json!({
                "meshAssetId": mesh.mesh_asset_id.to_string(),
                "materialOverrideAssetId": (!mesh.material_override_asset_id.is_nil())
                    .then(|| mesh.material_override_asset_id.to_string()),
                "maxViewDistance": mesh.max_view_distance,
                "impostorFarDistance": mesh.impostor_far_distance,
                "rootRelativeTransform": mesh.root_relative_transform,
                "rootRelativeInstanceTransforms": mesh.root_relative_instance_readers,
                "meshOptionsBitset": mesh.mesh_options_bitset,
                "lodRatio": mesh.lod_ratio,
            })
        })
        .collect::<Vec<_>>();
    let spawners = metadata
        .spawners
        .iter()
        .map(|spawner| {
            let slice_path =
                nw_vegetation::distribution::dynamic_slice_source_path(spawner.slice_name)
                    .map(|path| path.into_owned());
            let variant_metadata_path = slice_path.as_deref().and_then(|path| {
                nw_vegetation::distribution::slice_metadata_source_path(
                    path,
                    spawner.variation_name,
                )
            });
            serde_json::json!({
                "sliceAssetId": spawner.slice_asset_id.to_string(),
                "slicePath": slice_path,
                "variation": spawner.variation_name,
                "variantMetadataPath": variant_metadata_path,
                "worldTransform": spawner.world_transform,
                "prefabPersists": spawner.prefab_persists,
                "maxRotationAngle": spawner.max_rotation_angle,
            })
        })
        .collect::<Vec<_>>();
    serde_json::json!({
        "gdeSpawnRadius": metadata.gde_spawn_radius,
        "gridRegistrationRadius": metadata.grid_registration_radius,
        "aoiDistance": metadata.aoi_distance,
        "slicePhysicalRadius": metadata.slice_physical_radius,
        "gridCategory": metadata.grid_category,
        "isStaticSlice": metadata.is_static_slice,
        "hasCollision": metadata.has_collision,
        "isRequiredOnServer": metadata.is_required_on_server,
        "skipMidRangeImpostors": metadata.skip_mid_range_impostors,
        "forceWaitReplicatedData": metadata.force_wait_replicated_data,
        "isLongDistanceGde": metadata.is_long_distance_gde,
        "meshOptionsBitset": metadata.mesh_options_bitset,
        "sliceTags": metadata.slice_tags,
        "phasingRestriction": metadata.phasing_restriction,
        "meshes": meshes,
        "spawners": spawners,
        "childSpawnSliceIds": metadata.child_spawn_slice_ids.iter().map(ToString::to_string).collect::<Vec<_>>(),
        "spawnInInstances": metadata.spawn_in_instances,
        "prioritizeGdeWhenMounted": metadata.prioritize_gde_when_mounted,
        "usesCustomDefinedSpawnRadius": metadata.uses_custom_defined_spawn_radius,
    })
}

pub(super) fn add_dependency_resources(
    runner: &nw_jobs::JobRunner,
    source: &dyn AssetSource,
    paths: &[String],
    resolved: &mut ResolvedAsset,
) -> Result<()> {
    let mut candidates = paths
        .iter()
        .filter_map(|path| dependency_resource_kind(path).map(|kind| (normalize_path(path), kind)))
        .filter(|(path, kind)| {
            let already_embedded = resolved.extras.resource_payloads.iter().any(|resource| {
                resource.kind == *kind && resource.source_path.eq_ignore_ascii_case(path)
            });
            let already_loaded_shape = *kind == nw_model::CryEmbeddedResourceKind::RockNRollShape
                && resolved
                    .physics
                    .shape_assets
                    .iter()
                    .any(|asset| asset.source_path.eq_ignore_ascii_case(path));
            !already_embedded && !already_loaded_shape
        })
        .collect::<Vec<_>>();
    candidates.sort_by(|left, right| {
        left.0
            .to_ascii_lowercase()
            .cmp(&right.0.to_ascii_lowercase())
    });
    candidates.dedup_by(|left, right| left.0.eq_ignore_ascii_case(&right.0) && left.1 == right.1);

    let resources = runner.try_map(&candidates, |(path, kind)| {
        let bytes = read_required(source, path)?;
        let document = typed_dependency_document(*kind, &bytes)
            .with_context(|| format!("decode typed dependency resource {path}"))?;
        let physics = (*kind == nw_model::CryEmbeddedResourceKind::LegacyObjectStreamScene)
            .then(|| parse_scene_physics(path, &bytes))
            .transpose()
            .with_context(|| format!("decode scene physics {path}"))?;
        Ok::<_, anyhow::Error>((path.clone(), *kind, bytes, document, physics))
    })?;
    for (path, kind, bytes, document, physics) in resources {
        if let Some((source_kind, document)) = document
            && !has_source_asset(&resolved.extras, &path)
        {
            resolved
                .extras
                .source_assets
                .push(nw_model::CrySourceAsset {
                    path: path.clone(),
                    kind: source_kind,
                    document,
                });
        }
        if let Some(mut physics) = physics {
            resolved
                .physics
                .hit_volumes
                .append(&mut physics.hit_volumes);
            resolved
                .physics
                .rigid_bodies
                .append(&mut physics.rigid_bodies);
        }
        add_resource(&mut resolved.extras, &path, kind, bytes);
    }
    Ok(())
}
