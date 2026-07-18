//! Generated-type readers for New World region manifest satellites.

use nw_objectstream::{
    ObjectStream, ObjectStreamError,
    lookup::NameLookup,
    schema::{SchemaDeserializeError, SchemaValueCodec},
};
use nw_reflected_types::{
    az::rtti::AzRtti,
    types::{GridGenericAssetAssetData, RegionMetadataAsset},
};
use thiserror::Error;
use uuid::Uuid;

pub fn parse_region_metadata(
    bytes: &[u8],
    lookup: Option<&NameLookup>,
) -> Result<RegionMetadataAsset, RegionManifestError> {
    let stream = ObjectStream::from_bytes(bytes, lookup)?;
    read_region_metadata(&stream, lookup)
}

pub fn parse_region_chunks(
    bytes: &[u8],
    lookup: Option<&NameLookup>,
) -> Result<GridGenericAssetAssetData, RegionManifestError> {
    let stream = ObjectStream::from_bytes(bytes, lookup)?;
    read_region_chunks(&stream, lookup)
}

pub fn read_region_metadata(
    stream: &ObjectStream,
    lookup: Option<&NameLookup>,
) -> Result<RegionMetadataAsset, RegionManifestError> {
    decode_root::<RegionMetadataAsset>(stream, lookup)
}

pub fn read_region_chunks(
    stream: &ObjectStream,
    lookup: Option<&NameLookup>,
) -> Result<GridGenericAssetAssetData, RegionManifestError> {
    decode_root::<GridGenericAssetAssetData>(stream, lookup)
}

fn decode_root<T>(
    stream: &ObjectStream,
    lookup: Option<&NameLookup>,
) -> Result<T, RegionManifestError>
where
    T: serde::de::DeserializeOwned + AzRtti,
{
    let root = stream
        .elements()
        .first()
        .ok_or(RegionManifestError::MissingRoot { expected: T::NAME })?;
    let expected = *T::TYPE_ID.as_inner();
    if root.id() != &expected {
        return Err(RegionManifestError::UnexpectedRoot {
            expected_name: T::NAME,
            expected,
            actual: *root.id(),
        });
    }
    SchemaValueCodec::new(lookup)
        .deserialize(root)
        .map_err(RegionManifestError::Decode)
}

#[derive(Debug, Error)]
pub enum RegionManifestError {
    #[error("parse region manifest ObjectStream")]
    ObjectStream(#[from] ObjectStreamError),
    #[error("region manifest ObjectStream has no root; expected {expected}")]
    MissingRoot { expected: &'static str },
    #[error("expected {expected_name} root {expected}, got {actual}")]
    UnexpectedRoot {
        expected_name: &'static str,
        expected: Uuid,
        actual: Uuid,
    },
    #[error("decode region manifest generated type")]
    Decode(#[source] SchemaDeserializeError),
}

#[cfg(test)]
mod tests {
    use nw_objectstream::{Element, types};
    use nw_reflected_types::types::{ChunkEntry, FactionType, NPCData};
    use uuid::uuid;

    use super::*;

    #[test]
    fn region_metadata_uses_generated_nested_types_and_enum_storage() {
        let mut lookup = NameLookup::new();
        lookup
            .enum_underlying_types
            .insert(*FactionType::TYPE_ID.as_inner(), types::UNSIGNED_CHAR);
        let stream = ObjectStream {
            elements: vec![
                Element::new(RegionMetadataAsset::TYPE_ID.into()).with_children([Element::new(
                    types::AZSTD_VECTOR,
                )
                .with_field("NpcData")
                .with_children([Element::new(NPCData::TYPE_ID.into()).with_children([
                    Element::new(FactionType::TYPE_ID.into())
                        .with_field("FactionType")
                        .with_data([2]),
                ])])]),
            ],
            ..ObjectStream::new(3)
        };

        let metadata = read_region_metadata(&stream, Some(&lookup)).unwrap();
        assert_eq!(metadata.npc_data.len(), 1);
        assert_eq!(metadata.npc_data[0].faction_type, FactionType::Faction2);
    }

    #[test]
    fn region_chunks_use_generated_chunk_and_asset_id_types() {
        let guid = uuid!("53C24E32-9B91-5C08-90F2-FCF51832F8FA");
        let stream = ObjectStream {
            elements: vec![
                Element::new(GridGenericAssetAssetData::TYPE_ID.into()).with_children([
                    Element::new(types::AZSTD_VECTOR)
                        .with_field("Chunks")
                        .with_children([Element::new(ChunkEntry::TYPE_ID.into()).with_children([
                            Element::new(types::AZ_U64)
                                .with_field("size")
                                .with_data(64_u64.to_be_bytes()),
                            Element::new(types::ASSET_ID)
                                .with_field("assetId")
                                .with_children([
                                    Element::new(types::AZ_UUID)
                                        .with_field("guid")
                                        .with_data(guid.as_bytes()),
                                    Element::new(types::UNSIGNED_INT)
                                        .with_field("subId")
                                        .with_data(7_u32.to_be_bytes()),
                                ]),
                        ])]),
                ]),
            ],
            ..ObjectStream::new(3)
        };

        let chunks = read_region_chunks(&stream, None).unwrap();
        assert_eq!(chunks.chunks.len(), 1);
        assert_eq!(chunks.chunks[0].size, 64);
        assert_eq!(*chunks.chunks[0].asset_id.guid.as_inner(), guid);
        assert_eq!(chunks.chunks[0].asset_id.sub_id, 7);
    }
}
