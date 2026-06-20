//! Typed reader for New World `.slice.meta` ObjectStream sidecars.
//!
//! Source: `SliceMetaData::Reflect`, `SliceMetaDataMeshEntry::Reflect`, and
//! `SliceMetaDataSpawnerEntry::Reflect` in the New World 3-26 binary.

use nw_asset::AssetId;
use thiserror::Error;
use uuid::{Uuid, uuid};

use crate::value::{DecodeAzValue, FieldCursor, ObjectStreamValueError};
use crate::{Element, ObjectStream, types};

pub const SLICE_META_DATA_TYPE_ID: Uuid = uuid!("7D314916-0502-4D0C-B457-AE485E62A156");
pub const SLICE_META_DATA_MESH_ENTRY_TYPE_ID: Uuid = uuid!("0C584B89-F7CD-414E-8200-15BD5DE7E623");
pub const SLICE_META_DATA_SPAWNER_ENTRY_TYPE_ID: Uuid =
    uuid!("BEFF7D78-15DF-4F72-8F0C-40100EBDA993");

#[derive(Debug, Clone, PartialEq)]
pub struct SliceMetaData<'a> {
    pub gde_spawn_radius: f32,
    pub grid_registration_radius: f32,
    pub aoi_distance: f32,
    pub slice_physical_radius: f32,
    pub grid_category: u16,
    pub is_static_slice: bool,
    pub has_collision: bool,
    pub is_required_on_server: bool,
    pub skip_mid_range_impostors: bool,
    pub force_wait_replicated_data: bool,
    pub is_long_distance_gde: bool,
    pub mesh_options_bitset: u32,
    pub slice_tags: u32,
    pub phasing_restriction: u8,
    pub meshes: Vec<SliceMetaDataMeshEntry>,
    pub spawners: Vec<SliceMetaDataSpawnerEntry<'a>>,
    pub child_spawn_slice_ids: Vec<AssetId>,
    pub spawn_in_instances: bool,
    pub prioritize_gde_when_mounted: bool,
    pub uses_custom_defined_spawn_radius: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct SliceMetaDataMeshEntry {
    pub mesh_asset_id: AssetId,
    pub material_override_asset_id: AssetId,
    pub max_view_distance: f32,
    pub impostor_far_distance: bool,
    pub root_relative_transform: [f32; 12],
    pub root_relative_instance_readers: Vec<[f32; 12]>,
    pub mesh_options_bitset: u32,
    pub lod_ratio: u32,
}

#[derive(Debug, Clone, PartialEq)]
pub struct SliceMetaDataSpawnerEntry<'a> {
    pub world_transform: [f32; 12],
    pub slice_asset_id: AssetId,
    pub slice_name: &'a str,
    pub variation_name: &'a str,
    pub prefab_persists: bool,
    pub max_rotation_angle: f32,
}

#[derive(Debug, Error)]
pub enum SliceMetaDataError {
    #[error("slice metadata stream has no top-level element")]
    EmptyStream,
    #[error("expected {expected}, got {actual}")]
    UnexpectedType {
        expected: &'static str,
        actual: Uuid,
    },
    #[error("{owner} is missing field `{field}`")]
    MissingField {
        owner: &'static str,
        field: &'static str,
    },
    #[error("slice metadata field `{field}` could not be read")]
    Field {
        field: &'static str,
        #[source]
        source: ObjectStreamValueError,
    },
}

pub fn read_slice_meta_data(
    stream: &ObjectStream,
) -> Result<SliceMetaData<'_>, SliceMetaDataError> {
    let element = stream
        .elements()
        .first()
        .ok_or(SliceMetaDataError::EmptyStream)?;
    read_slice_meta_data_element(element)
}

pub fn read_slice_meta_data_element(
    element: &Element,
) -> Result<SliceMetaData<'_>, SliceMetaDataError> {
    expect_type(element, SLICE_META_DATA_TYPE_ID, "SliceMetaData")?;
    let mut fields = FieldCursor::from_element(element);

    Ok(SliceMetaData {
        gde_spawn_radius: read_or_default(&mut fields, "gdeSpawnRadius")?,
        grid_registration_radius: read_or_default(&mut fields, "gridRegistrationRadius")?,
        aoi_distance: read_or_default(&mut fields, "aoiDistance")?,
        slice_physical_radius: read_or_default(&mut fields, "slicePhysicalRadius")?,
        grid_category: read_or_default(&mut fields, "gridCategory")?,
        is_static_slice: read_or_default(&mut fields, "isStaticSlice")?,
        has_collision: read_or_default(&mut fields, "hasCollision")?,
        is_required_on_server: read_or_default(&mut fields, "isRequiredOnServer")?,
        skip_mid_range_impostors: read_or_default(&mut fields, "skipMidRangeImpostors")?,
        force_wait_replicated_data: read_or_default(&mut fields, "forceWaitReplicatedData")?,
        is_long_distance_gde: read_or_default(&mut fields, "isLongDistanceGDE")?,
        mesh_options_bitset: read_or_default(&mut fields, "meshOptionsBitset")?,
        slice_tags: read_or_default(&mut fields, "sliceTags")?,
        phasing_restriction: read_or_default(&mut fields, "phasingRestriction")?,
        meshes: fields
            .find("meshes")
            .map(read_meshes)
            .transpose()?
            .unwrap_or_default(),
        spawners: fields
            .find_any(&["spawners", "Spawners"])
            .map(|(_, field)| read_spawners(field))
            .transpose()?
            .unwrap_or_default(),
        child_spawn_slice_ids: fields
            .find("childSpawnSliceIds")
            .map(read_asset_id_vector)
            .transpose()?
            .unwrap_or_default(),
        spawn_in_instances: read_or_default(&mut fields, "spawnInInstances")?,
        prioritize_gde_when_mounted: read_or_default(&mut fields, "prioritizeGDEWhenMounted")?,
        uses_custom_defined_spawn_radius: read_or_default(
            &mut fields,
            "usesCustomDefinedSpawnRadius",
        )?,
    })
}

fn read_meshes(element: &Element) -> Result<Vec<SliceMetaDataMeshEntry>, SliceMetaDataError> {
    element
        .children()
        .iter()
        .filter(|child| child.id() == &SLICE_META_DATA_MESH_ENTRY_TYPE_ID)
        .map(read_mesh)
        .collect()
}

fn read_mesh(element: &Element) -> Result<SliceMetaDataMeshEntry, SliceMetaDataError> {
    expect_type(
        element,
        SLICE_META_DATA_MESH_ENTRY_TYPE_ID,
        "SliceMetaDataMeshEntry",
    )?;
    let mut fields = FieldCursor::from_element(element);
    Ok(SliceMetaDataMeshEntry {
        mesh_asset_id: read_required_asset_id(
            &mut fields,
            "SliceMetaDataMeshEntry",
            "meshAssetId",
        )?,
        material_override_asset_id: fields
            .find("materialOverrideAssetId")
            .map(read_asset_id)
            .transpose()?
            .unwrap_or_else(AssetId::nil),
        max_view_distance: read_any_or_default(
            &mut fields,
            &["maxViewDistance", "MaxViewDistance"],
        )?,
        impostor_far_distance: read_or_default(&mut fields, "m_impostorFarDistance")?,
        root_relative_transform: read_or_identity_transform(&mut fields, "rootRelativeTransform")?,
        root_relative_instance_readers: fields
            .find("rootRelativeInstanceTransforms")
            .map(read_transform_vector)
            .transpose()?
            .unwrap_or_default(),
        mesh_options_bitset: read_or_default(&mut fields, "meshOptionsBitset")?,
        lod_ratio: read_any_or_default(&mut fields, &["lodRatio", "LODRatio"])?,
    })
}

fn read_spawners(
    element: &Element,
) -> Result<Vec<SliceMetaDataSpawnerEntry<'_>>, SliceMetaDataError> {
    element
        .children()
        .iter()
        .filter(|child| child.id() == &SLICE_META_DATA_SPAWNER_ENTRY_TYPE_ID)
        .map(read_spawner)
        .collect()
}

fn read_spawner(element: &Element) -> Result<SliceMetaDataSpawnerEntry<'_>, SliceMetaDataError> {
    expect_type(
        element,
        SLICE_META_DATA_SPAWNER_ENTRY_TYPE_ID,
        "SliceMetaDataSpawnerEntry",
    )?;
    let mut fields = FieldCursor::from_element(element);
    Ok(SliceMetaDataSpawnerEntry {
        world_transform: read_or_identity_transform(&mut fields, "worldTM")?,
        slice_asset_id: read_required_asset_id(
            &mut fields,
            "SliceMetaDataSpawnerEntry",
            "sliceAssetId",
        )?,
        slice_name: read_required(&mut fields, "SliceMetaDataSpawnerEntry", "sliceName")?,
        variation_name: read_required(&mut fields, "SliceMetaDataSpawnerEntry", "variationName")?,
        prefab_persists: read_or_default(&mut fields, "prefabPersists")?,
        max_rotation_angle: read_or_default(&mut fields, "maxRotationAngle")?,
    })
}

fn read_asset_id_vector(element: &Element) -> Result<Vec<AssetId>, SliceMetaDataError> {
    element
        .children()
        .iter()
        .filter(|child| child.id() == &types::ASSET)
        .map(read_asset_id)
        .collect()
}

fn read_transform_vector(element: &Element) -> Result<Vec<[f32; 12]>, SliceMetaDataError> {
    element
        .children()
        .iter()
        .filter(|child| child.id() == &types::TRANSFORM)
        .map(|child| {
            <[f32; 12]>::decode_az_value(child).map_err(|source| SliceMetaDataError::Field {
                field: "rootRelativeInstanceTransforms",
                source,
            })
        })
        .collect()
}

fn read_asset_id(element: &Element) -> Result<AssetId, SliceMetaDataError> {
    expect_type(element, types::ASSET, "AZ::Data::AssetId")?;
    let mut fields = FieldCursor::from_element(element);
    let guid = read_required(&mut fields, "AZ::Data::AssetId", "guid")?;
    let sub_id = read_required(&mut fields, "AZ::Data::AssetId", "subId")?;
    Ok(AssetId::new(guid, sub_id))
}

fn read_required_asset_id(
    fields: &mut FieldCursor<'_>,
    owner: &'static str,
    field: &'static str,
) -> Result<AssetId, SliceMetaDataError> {
    fields
        .find(field)
        .ok_or(SliceMetaDataError::MissingField { owner, field })
        .and_then(read_asset_id)
}

fn read_or_identity_transform(
    fields: &mut FieldCursor<'_>,
    field: &'static str,
) -> Result<[f32; 12], SliceMetaDataError> {
    fields
        .find(field)
        .map(|element| {
            <[f32; 12]>::decode_az_value(element)
                .map_err(|source| SliceMetaDataError::Field { field, source })
        })
        .transpose()
        .map(|value| value.unwrap_or([1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0]))
}

fn read_required<'a, T>(
    fields: &mut FieldCursor<'a>,
    owner: &'static str,
    field: &'static str,
) -> Result<T, SliceMetaDataError>
where
    T: DecodeAzValue<'a>,
{
    fields
        .find(field)
        .ok_or(SliceMetaDataError::MissingField { owner, field })
        .and_then(|element| {
            T::decode_az_value(element)
                .map_err(|source| SliceMetaDataError::Field { field, source })
        })
}

fn read_or_default<'a, T>(
    fields: &mut FieldCursor<'a>,
    field: &'static str,
) -> Result<T, SliceMetaDataError>
where
    T: DecodeAzValue<'a> + Default,
{
    fields
        .find(field)
        .map(|element| {
            T::decode_az_value(element)
                .map_err(|source| SliceMetaDataError::Field { field, source })
        })
        .transpose()
        .map(Option::unwrap_or_default)
}

fn read_any_or_default<'a, T>(
    fields: &mut FieldCursor<'a>,
    names: &[&'static str],
) -> Result<T, SliceMetaDataError>
where
    T: DecodeAzValue<'a> + Default,
{
    fields
        .find_any(names)
        .map(|(_, element)| {
            T::decode_az_value(element).map_err(|source| SliceMetaDataError::Field {
                field: names[0],
                source,
            })
        })
        .transpose()
        .map(Option::unwrap_or_default)
}

fn expect_type(
    element: &Element,
    expected: Uuid,
    expected_name: &'static str,
) -> Result<(), SliceMetaDataError> {
    if element.id() == &expected {
        Ok(())
    } else {
        Err(SliceMetaDataError::UnexpectedType {
            expected: expected_name,
            actual: *element.id(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Element;
    use uuid::uuid;

    #[test]
    fn reads_mesh_entries_and_child_spawns() {
        let mesh_guid = uuid!("A1BB52F8-7884-5B5E-BD5B-8B608E7601D6");
        let material_guid = uuid!("42F02A20-4EBC-51C4-8F76-B04DE97FC6AF");
        let child_guid = uuid!("53C24E32-9B91-5C08-90F2-FCF51832F8FA");
        let stream = ObjectStream {
            elements: vec![
                Element::new(SLICE_META_DATA_TYPE_ID).with_children([
                    leaf("gdeSpawnRadius", types::FLOAT, 2.5_f32.to_be_bytes()),
                    leaf("gridCategory", types::UNSIGNED_SHORT, 7_u16.to_be_bytes()),
                    Element::new(types::AZSTD_VECTOR)
                        .with_field("meshes")
                        .with_children([Element::new(SLICE_META_DATA_MESH_ENTRY_TYPE_ID)
                            .with_children([
                                asset_id("meshAssetId", mesh_guid, 0),
                                asset_id("materialOverrideAssetId", material_guid, 1),
                                leaf("MaxViewDistance", types::FLOAT, 123.0_f32.to_be_bytes()),
                                transform("rootRelativeTransform", [1.0; 12]),
                                Element::new(types::AZSTD_VECTOR)
                                    .with_field("rootRelativeInstanceTransforms")
                                    .with_children([transform("", [2.0; 12])]),
                                leaf("LODRatio", types::UNSIGNED_INT, 75_u32.to_be_bytes()),
                            ])]),
                    Element::new(types::AZSTD_VECTOR)
                        .with_field("childSpawnSliceIds")
                        .with_children([asset_id("", child_guid, 2)]),
                ]),
            ],
            ..ObjectStream::new(3)
        };

        let metadata = read_slice_meta_data(&stream).unwrap();

        assert_eq!(metadata.gde_spawn_radius, 2.5);
        assert_eq!(metadata.grid_category, 7);
        assert_eq!(metadata.meshes.len(), 1);
        assert_eq!(metadata.meshes[0].mesh_asset_id, AssetId::new(mesh_guid, 0));
        assert_eq!(
            metadata.meshes[0].material_override_asset_id,
            AssetId::new(material_guid, 1)
        );
        assert_eq!(metadata.meshes[0].max_view_distance, 123.0);
        assert_eq!(metadata.meshes[0].root_relative_transform, [1.0; 12]);
        assert_eq!(
            metadata.meshes[0].root_relative_instance_readers,
            vec![[2.0; 12]]
        );
        assert_eq!(metadata.meshes[0].lod_ratio, 75);
        assert_eq!(
            metadata.child_spawn_slice_ids,
            vec![AssetId::new(child_guid, 2)]
        );
    }

    #[test]
    fn reads_spawner_entries() {
        let slice_guid = uuid!("53C24E32-9B91-5C08-90F2-FCF51832F8FA");
        let stream = ObjectStream {
            elements: vec![
                Element::new(SLICE_META_DATA_TYPE_ID).with_children([Element::new(
                    types::AZSTD_VECTOR,
                )
                .with_field("Spawners")
                .with_children([Element::new(SLICE_META_DATA_SPAWNER_ENTRY_TYPE_ID)
                    .with_children([
                        transform("worldTM", [3.0; 12]),
                        asset_id("sliceAssetId", slice_guid, 0),
                        leaf("sliceName", types::AZSTD_STRING, b"Tree"),
                        leaf("variationName", types::AZSTD_STRING, b"Fall"),
                        leaf("prefabPersists", types::BOOL, [1]),
                        leaf("maxRotationAngle", types::FLOAT, 45.0_f32.to_be_bytes()),
                    ])])]),
            ],
            ..ObjectStream::new(3)
        };

        let metadata = read_slice_meta_data(&stream).unwrap();

        assert_eq!(metadata.spawners.len(), 1);
        assert_eq!(
            metadata.spawners[0].slice_asset_id,
            AssetId::new(slice_guid, 0)
        );
        assert_eq!(metadata.spawners[0].slice_name, "Tree");
        assert_eq!(metadata.spawners[0].variation_name, "Fall");
        assert!(metadata.spawners[0].prefab_persists);
        assert_eq!(metadata.spawners[0].max_rotation_angle, 45.0);
    }

    fn leaf(field: &str, id: Uuid, data: impl Into<Vec<u8>>) -> Element {
        Element::new(id).with_field(field).with_data(data)
    }

    fn transform(field: &str, values: [f32; 12]) -> Element {
        let mut bytes = Vec::with_capacity(48);
        for value in values {
            bytes.extend_from_slice(&value.to_be_bytes());
        }
        let element = Element::new(types::TRANSFORM).with_data(bytes);
        if field.is_empty() {
            element
        } else {
            element.with_field(field)
        }
    }

    fn asset_id(field: &str, guid: Uuid, sub_id: u32) -> Element {
        let element = Element::new(types::ASSET).with_children([
            leaf("guid", types::AZ_UUID, guid.as_bytes()),
            leaf("subId", types::UNSIGNED_INT, sub_id.to_be_bytes()),
        ]);
        if field.is_empty() {
            element
        } else {
            element.with_field(field)
        }
    }
}
