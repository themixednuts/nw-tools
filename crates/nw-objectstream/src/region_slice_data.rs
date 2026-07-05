//! Typed reader for New World `region.slicedata` ObjectStreams.
//!
//! Source: `RegionSliceDataLookup::Reflect` and `SliceDataEntryKey::Reflect`
//! in the New World 3-26 binary.

use thiserror::Error;
use uuid::{Uuid, uuid};

use crate::slice_meta::{
    SLICE_META_DATA_TYPE_ID, SliceMetaData, SliceMetaDataError, read_slice_meta_data_element,
};
use crate::types;
use crate::value::{DecodeAzValue, FieldCursor, ObjectStreamValueError};
use crate::{Element, ObjectStream};

pub const REGION_SLICE_DATA_LOOKUP_TYPE_ID: Uuid = uuid!("BE64A116-579D-49B1-9511-87D2CE9AC3E6");
pub const REGION_SLICE_DATA_MAP_ENTRY_TYPE_ID: Uuid = uuid!("DB52AACF-0A06-572D-8DC2-A36C84FE19C6");
pub const SLICE_DATA_ENTRY_KEY_TYPE_ID: Uuid = uuid!("E9B80917-3B79-4F83-830A-86F277069E46");
const SLICE_NAME_FIELD_CRC: u32 = 0x1698_73af;
const VARIANT_NAME_FIELD_CRC: u32 = 0x3ece_d8bf;

#[derive(Debug, Clone, PartialEq)]
pub struct RegionSliceDataLookup<'a> {
    pub entries: Vec<RegionSliceDataEntry<'a>>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct RegionSliceDataEntry<'a> {
    pub key: SliceDataEntryKey<'a>,
    pub metadata: SliceMetaData<'a>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SliceDataEntryKey<'a> {
    pub slice_name: &'a str,
    pub variant_name: &'a str,
}

#[derive(Debug, Error)]
pub enum RegionSliceDataError {
    #[error("region slice data stream has no top-level element")]
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
    #[error("slice metadata map entry {index} is missing {value}")]
    MissingMapValue { index: usize, value: &'static str },
    #[error("slice entry key field `{field}` could not be read")]
    Field {
        field: &'static str,
        #[source]
        source: ObjectStreamValueError,
    },
    #[error("slice metadata map entry {index} metadata could not be read")]
    Metadata {
        index: usize,
        #[source]
        source: SliceMetaDataError,
    },
}

pub fn read_region_slice_data(
    stream: &ObjectStream,
) -> Result<RegionSliceDataLookup<'_>, RegionSliceDataError> {
    let element = stream
        .elements()
        .first()
        .ok_or(RegionSliceDataError::EmptyStream)?;
    read_region_slice_data_element(element)
}

pub fn read_region_slice_data_element(
    element: &Element,
) -> Result<RegionSliceDataLookup<'_>, RegionSliceDataError> {
    expect_type(
        element,
        REGION_SLICE_DATA_LOOKUP_TYPE_ID,
        "RegionSliceDataLookup",
    )?;
    let mut fields = FieldCursor::from_element(element);
    let map = if let Some(map) = fields.find("sliceMetaDataMap") {
        map
    } else {
        find_slice_metadata_map(element).ok_or(RegionSliceDataError::MissingField {
            owner: "RegionSliceDataLookup",
            field: "sliceMetaDataMap",
        })?
    };

    Ok(RegionSliceDataLookup {
        entries: read_entries(map)?,
    })
}

fn find_slice_metadata_map(element: &Element) -> Option<&Element> {
    element.children().iter().find(|child| {
        child
            .children()
            .iter()
            .any(|entry| entry.id() == &REGION_SLICE_DATA_MAP_ENTRY_TYPE_ID)
    })
}

fn read_entries(map: &Element) -> Result<Vec<RegionSliceDataEntry<'_>>, RegionSliceDataError> {
    map.children()
        .iter()
        .enumerate()
        .map(|(index, element)| read_entry(index, element))
        .collect()
}

fn read_entry(
    index: usize,
    element: &Element,
) -> Result<RegionSliceDataEntry<'_>, RegionSliceDataError> {
    if element.id() != &REGION_SLICE_DATA_MAP_ENTRY_TYPE_ID {
        return Err(RegionSliceDataError::UnexpectedType {
            expected: "RegionSliceDataLookup map entry",
            actual: *element.id(),
        });
    }

    let key = element
        .children()
        .iter()
        .find(|child| child.id() == &SLICE_DATA_ENTRY_KEY_TYPE_ID)
        .ok_or(RegionSliceDataError::MissingMapValue {
            index,
            value: "SliceDataEntryKey",
        })?;
    let metadata = element
        .children()
        .iter()
        .find(|child| child.id() == &SLICE_META_DATA_TYPE_ID)
        .ok_or(RegionSliceDataError::MissingMapValue {
            index,
            value: "SliceMetaData",
        })?;

    Ok(RegionSliceDataEntry {
        key: read_key(key)?,
        metadata: read_slice_meta_data_element(metadata)
            .map_err(|source| RegionSliceDataError::Metadata { index, source })?,
    })
}

fn read_key(element: &Element) -> Result<SliceDataEntryKey<'_>, RegionSliceDataError> {
    expect_type(element, SLICE_DATA_ENTRY_KEY_TYPE_ID, "SliceDataEntryKey")?;
    let mut fields = FieldCursor::from_element(element);
    let field_result = (|| {
        Ok(SliceDataEntryKey {
            slice_name: read_required(&mut fields, "sliceName", SLICE_NAME_FIELD_CRC)?,
            variant_name: read_required(&mut fields, "variantName", VARIANT_NAME_FIELD_CRC)?,
        })
    })();

    match field_result {
        Ok(key) => Ok(key),
        Err(err @ RegionSliceDataError::MissingField { .. }) => {
            match read_key_by_position(element) {
                Some(result) => result,
                None => Err(err),
            }
        }
        Err(err) => Err(err),
    }
}

fn read_required<'a, T>(
    fields: &mut FieldCursor<'a>,
    field: &'static str,
    name_crc: u32,
) -> Result<T, RegionSliceDataError>
where
    T: DecodeAzValue<'a>,
{
    fields
        .find(field)
        .or_else(|| find_by_crc(fields, name_crc))
        .ok_or(RegionSliceDataError::MissingField {
            owner: "SliceDataEntryKey",
            field,
        })
        .and_then(|element| {
            T::decode_az_value(element)
                .map_err(|source| RegionSliceDataError::Field { field, source })
        })
}

fn find_by_crc<'a>(fields: &mut FieldCursor<'a>, name_crc: u32) -> Option<&'a Element> {
    let index = fields
        .remaining()
        .iter()
        .position(|child| child.name_crc() == Some(name_crc))?;
    for _ in 0..index {
        fields.next();
    }
    fields.next()
}

fn read_key_by_position(
    element: &Element,
) -> Option<Result<SliceDataEntryKey<'_>, RegionSliceDataError>> {
    let [slice_name, variant_name] = element.children() else {
        return None;
    };
    if slice_name.id() != &types::AZSTD_STRING || variant_name.id() != &types::AZSTD_STRING {
        return None;
    }
    let slice_name = match decode_key_string(slice_name, "sliceName") {
        Ok(value) => value,
        Err(err) => return Some(Err(err)),
    };
    let variant_name = match decode_key_string(variant_name, "variantName") {
        Ok(value) => value,
        Err(err) => return Some(Err(err)),
    };
    Some(Ok(SliceDataEntryKey {
        slice_name,
        variant_name,
    }))
}

fn decode_key_string<'a>(
    element: &'a Element,
    field: &'static str,
) -> Result<&'a str, RegionSliceDataError> {
    <&str>::decode_az_value(element).map_err(|source| RegionSliceDataError::Field { field, source })
}

fn expect_type(
    element: &Element,
    expected: Uuid,
    expected_name: &'static str,
) -> Result<(), RegionSliceDataError> {
    if element.id() == &expected {
        Ok(())
    } else {
        Err(RegionSliceDataError::UnexpectedType {
            expected: expected_name,
            actual: *element.id(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ObjectStream;

    #[test]
    fn reads_region_slice_data_lookup() {
        let stream = ObjectStream {
            elements: vec![
                Element::new(REGION_SLICE_DATA_LOOKUP_TYPE_ID).with_children([Element::new(
                    types::AZSTD_MAP,
                )
                .with_field("sliceMetaDataMap")
                .with_children([Element::new(REGION_SLICE_DATA_MAP_ENTRY_TYPE_ID)
                    .with_children([
                        Element::new(SLICE_DATA_ENTRY_KEY_TYPE_ID).with_children([
                            leaf("sliceName", types::AZSTD_STRING, b"gatherables/master_tree"),
                            leaf("variantName", types::AZSTD_STRING, b"OakA"),
                        ]),
                        Element::new(SLICE_META_DATA_TYPE_ID).with_children([
                            leaf("gdeSpawnRadius", types::FLOAT, 32.0_f32.to_be_bytes()),
                            leaf("aoiDistance", types::FLOAT, 64.0_f32.to_be_bytes()),
                            leaf("gridCategory", types::UNSIGNED_SHORT, 5_u16.to_be_bytes()),
                        ]),
                    ])])]),
            ],
            ..ObjectStream::new(3)
        };

        let lookup = read_region_slice_data(&stream).unwrap();

        assert_eq!(lookup.entries.len(), 1);
        assert_eq!(lookup.entries[0].key.slice_name, "gatherables/master_tree");
        assert_eq!(lookup.entries[0].key.variant_name, "OakA");
        assert_eq!(lookup.entries[0].metadata.gde_spawn_radius, 32.0);
        assert_eq!(lookup.entries[0].metadata.aoi_distance, 64.0);
        assert_eq!(lookup.entries[0].metadata.grid_category, 5);
    }

    #[test]
    fn reads_region_slice_data_lookup_binary_crc_key_fields() {
        let stream = ObjectStream {
            elements: vec![
                Element::new(REGION_SLICE_DATA_LOOKUP_TYPE_ID).with_children([Element::new(
                    types::AZSTD_MAP,
                )
                .with_children([Element::new(REGION_SLICE_DATA_MAP_ENTRY_TYPE_ID)
                    .with_children([
                        Element::new(SLICE_DATA_ENTRY_KEY_TYPE_ID).with_children([
                            leaf_crc(SLICE_NAME_FIELD_CRC, b"gatherables/master_log"),
                            leaf_crc(VARIANT_NAME_FIELD_CRC, b"rottenlog_d"),
                        ]),
                        Element::new(SLICE_META_DATA_TYPE_ID).with_children([
                            leaf("gdeSpawnRadius", types::FLOAT, 32.0_f32.to_be_bytes()),
                            leaf("aoiDistance", types::FLOAT, 64.0_f32.to_be_bytes()),
                            leaf("gridCategory", types::UNSIGNED_SHORT, 5_u16.to_be_bytes()),
                        ]),
                    ])])]),
            ],
            ..ObjectStream::new(3)
        };

        let lookup = read_region_slice_data(&stream).unwrap();

        assert_eq!(lookup.entries[0].key.slice_name, "gatherables/master_log");
        assert_eq!(lookup.entries[0].key.variant_name, "rottenlog_d");
    }

    #[test]
    fn reads_region_slice_data_lookup_positional_key_fields() {
        let stream = ObjectStream {
            elements: vec![
                Element::new(REGION_SLICE_DATA_LOOKUP_TYPE_ID).with_children([Element::new(
                    types::AZSTD_MAP,
                )
                .with_children([Element::new(REGION_SLICE_DATA_MAP_ENTRY_TYPE_ID)
                    .with_children([
                        Element::new(SLICE_DATA_ENTRY_KEY_TYPE_ID).with_children([
                            Element::new(types::AZSTD_STRING).with_data(b"gatherables/master_log"),
                            Element::new(types::AZSTD_STRING).with_data(b"rottenlog_d"),
                        ]),
                        Element::new(SLICE_META_DATA_TYPE_ID).with_children([
                            leaf("gdeSpawnRadius", types::FLOAT, 32.0_f32.to_be_bytes()),
                            leaf("aoiDistance", types::FLOAT, 64.0_f32.to_be_bytes()),
                            leaf("gridCategory", types::UNSIGNED_SHORT, 5_u16.to_be_bytes()),
                        ]),
                    ])])]),
            ],
            ..ObjectStream::new(3)
        };

        let lookup = read_region_slice_data(&stream).unwrap();

        assert_eq!(lookup.entries[0].key.slice_name, "gatherables/master_log");
        assert_eq!(lookup.entries[0].key.variant_name, "rottenlog_d");
    }

    fn leaf(field: &str, id: Uuid, data: impl Into<Vec<u8>>) -> Element {
        Element::new(id).with_field(field).with_data(data)
    }

    fn leaf_crc(name_crc: u32, data: impl Into<Vec<u8>>) -> Element {
        Element {
            name_crc: Some(name_crc),
            ..Element::new(types::AZSTD_STRING).with_data(data)
        }
    }
}
