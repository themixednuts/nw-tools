//! Parser for terrain `region.waterqt` ObjectStream payloads.

use std::fmt;

use nw_objectstream::{Element, ObjectStream, ObjectStreamError};
use nw_reflected_types::{
    az::rtti::AzRtti,
    types::serializable_water_quadtree::{SerializableWaterQuadtree, WaterNodeData},
};
use uuid::Uuid;

const REGION_SIZE_CRC: u32 = 3_404_125_817;
const QUADTREE_NODES_CRC: u32 = 2_218_521_579;
const HEIGHT_CRC: u32 = 4_115_522_831;
const FLOOR_HEIGHT_CRC: u32 = 2_907_992_229;
const FLAGS_CRC: u32 = 184_893_882;

/// Parse a reflected `SerializableWaterQuadtree` ObjectStream into the
/// SerializeContext-generated legacy wire types.
pub fn parse_water_quadtree(bytes: &[u8]) -> Result<SerializableWaterQuadtree, ParseError> {
    let stream = ObjectStream::from_bytes(bytes, None).map_err(ParseError::ObjectStream)?;
    let root = stream.elements().first().ok_or(ParseError::MissingRoot)?;
    let expected = *SerializableWaterQuadtree::TYPE_ID.as_inner();
    if root.id() != &expected {
        return Err(ParseError::UnexpectedRoot {
            found: *root.id(),
            expected,
        });
    }

    let region_size = required_i32(root, REGION_SIZE_CRC)?;
    let nodes = field(root, QUADTREE_NODES_CRC, "SerializableWaterQuadtree")?;
    let node_type = WaterNodeData::TYPE_ID.as_inner();
    let quadtree_nodes = nodes
        .children()
        .iter()
        .filter(|node| node.id() == node_type)
        .map(parse_node)
        .collect::<Result<Vec<_>, _>>()?;

    Ok(SerializableWaterQuadtree {
        region_size,
        quadtree_nodes,
    })
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct WaterNodeSummary {
    pub height: f32,
    pub floor_height: f32,
    pub flags: u32,
}

#[derive(Debug, Clone, PartialEq)]
pub struct WaterQuadtreeSummary {
    pub bytes: usize,
    pub region_size: i32,
    pub nodes: usize,
    pub root: Option<WaterNodeSummary>,
    pub leading_nodes: Vec<WaterNodeSummary>,
}

pub fn summarize_water_quadtree(
    bytes: &[u8],
    leading_nodes: usize,
) -> Result<WaterQuadtreeSummary, ParseError> {
    let tree = parse_water_quadtree(bytes)?;
    Ok(WaterQuadtreeSummary {
        bytes: bytes.len(),
        region_size: tree.region_size,
        nodes: tree.quadtree_nodes.len(),
        root: tree.quadtree_nodes.first().map(WaterNodeSummary::from),
        leading_nodes: tree
            .quadtree_nodes
            .iter()
            .take(leading_nodes)
            .map(WaterNodeSummary::from)
            .collect(),
    })
}

impl From<&WaterNodeData> for WaterNodeSummary {
    fn from(node: &WaterNodeData) -> Self {
        Self {
            height: node.height,
            floor_height: node.floor_height,
            flags: node.flags[0],
        }
    }
}

#[derive(Debug)]
pub enum ParseError {
    ObjectStream(ObjectStreamError),
    MissingRoot,
    UnexpectedRoot {
        found: Uuid,
        expected: Uuid,
    },
    MissingField {
        parent: &'static str,
        field_crc: u32,
    },
    MissingValue {
        parent: &'static str,
        field_crc: u32,
    },
    BadValueSize {
        parent: &'static str,
        field_crc: u32,
        expected: usize,
        found: usize,
    },
}

impl fmt::Display for ParseError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ObjectStream(error) => write!(formatter, "parse ObjectStream: {error}"),
            Self::MissingRoot => {
                write!(formatter, "water quadtree ObjectStream has no root object")
            }
            Self::UnexpectedRoot { found, expected } => {
                write!(
                    formatter,
                    "unexpected root type {found} (expected {expected})"
                )
            }
            Self::MissingField { parent, field_crc } => {
                write!(formatter, "{parent} is missing field CRC {field_crc}")
            }
            Self::MissingValue { parent, field_crc } => {
                write!(formatter, "{parent} field CRC {field_crc} has no payload")
            }
            Self::BadValueSize {
                parent,
                field_crc,
                expected,
                found,
            } => write!(
                formatter,
                "{parent} field CRC {field_crc} has {found} bytes (expected {expected})"
            ),
        }
    }
}

impl std::error::Error for ParseError {}

fn parse_node(element: &Element) -> Result<WaterNodeData, ParseError> {
    Ok(WaterNodeData {
        height: optional_f32(element, HEIGHT_CRC)?.unwrap_or_default(),
        floor_height: optional_f32(element, FLOOR_HEIGHT_CRC)?.unwrap_or_default(),
        flags: [optional_u32(element, FLAGS_CRC)?.unwrap_or_default()],
    })
}

fn required_i32(parent: &Element, field_crc: u32) -> Result<i32, ParseError> {
    let bytes = required_bytes(parent, field_crc, "SerializableWaterQuadtree", 4)?;
    Ok(i32::from_be_bytes(
        bytes.try_into().expect("validated length"),
    ))
}

fn optional_f32(parent: &Element, field_crc: u32) -> Result<Option<f32>, ParseError> {
    optional_bytes(parent, field_crc, "WaterNodeData", 4).map(|bytes| {
        bytes.map(|bytes| f32::from_be_bytes(bytes.try_into().expect("validated length")))
    })
}

fn optional_u32(parent: &Element, field_crc: u32) -> Result<Option<u32>, ParseError> {
    optional_bytes(parent, field_crc, "WaterNodeData", 4).map(|bytes| {
        bytes.map(|bytes| u32::from_be_bytes(bytes.try_into().expect("validated length")))
    })
}

fn required_bytes<'a>(
    parent: &'a Element,
    field_crc: u32,
    parent_name: &'static str,
    expected: usize,
) -> Result<&'a [u8], ParseError> {
    let field = field(parent, field_crc, parent_name)?;
    value_bytes(field, field_crc, parent_name, expected)
}

fn optional_bytes<'a>(
    parent: &'a Element,
    field_crc: u32,
    parent_name: &'static str,
    expected: usize,
) -> Result<Option<&'a [u8]>, ParseError> {
    parent
        .children()
        .iter()
        .find(|element| element.name_crc() == Some(field_crc))
        .map(|field| value_bytes(field, field_crc, parent_name, expected))
        .transpose()
}

fn value_bytes<'a>(
    field: &'a Element,
    field_crc: u32,
    parent_name: &'static str,
    expected: usize,
) -> Result<&'a [u8], ParseError> {
    let bytes = field.data().ok_or(ParseError::MissingValue {
        parent: parent_name,
        field_crc,
    })?;
    if bytes.len() != expected {
        return Err(ParseError::BadValueSize {
            parent: parent_name,
            field_crc,
            expected,
            found: bytes.len(),
        });
    }
    Ok(bytes)
}

fn field<'a>(
    parent: &'a Element,
    field_crc: u32,
    parent_name: &'static str,
) -> Result<&'a Element, ParseError> {
    parent
        .children()
        .iter()
        .find(|element| element.name_crc() == Some(field_crc))
        .ok_or(ParseError::MissingField {
            parent: parent_name,
            field_crc,
        })
}

#[cfg(test)]
mod tests {
    use nw_objectstream::{
        Element, ObjectStream, ST_BINARYFLAG_ELEMENT_HEADER, ST_BINARYFLAG_HAS_NAME,
        ST_BINARYFLAG_HAS_VALUE, StreamTag,
    };

    use super::*;

    #[test]
    fn parses_generated_water_quadtree_types() {
        let stream = ObjectStream {
            tag: StreamTag::BINARY,
            version: 3,
            elements: vec![Element {
                flags: ST_BINARYFLAG_ELEMENT_HEADER,
                id: SerializableWaterQuadtree::TYPE_ID.into(),
                elements: vec![
                    value_element(
                        nw_objectstream::types::INT,
                        REGION_SIZE_CRC,
                        2048_i32.to_be_bytes(),
                    ),
                    Element {
                        flags: ST_BINARYFLAG_ELEMENT_HEADER | ST_BINARYFLAG_HAS_NAME,
                        name_crc: Some(QUADTREE_NODES_CRC),
                        elements: vec![Element {
                            flags: ST_BINARYFLAG_ELEMENT_HEADER,
                            id: WaterNodeData::TYPE_ID.into(),
                            elements: vec![
                                value_element(
                                    nw_objectstream::types::FLOAT,
                                    HEIGHT_CRC,
                                    12.5_f32.to_be_bytes(),
                                ),
                                value_element(
                                    nw_objectstream::types::FLOAT,
                                    FLOOR_HEIGHT_CRC,
                                    3.25_f32.to_be_bytes(),
                                ),
                                value_element(Uuid::nil(), FLAGS_CRC, 2_u32.to_be_bytes()),
                            ],
                            ..Default::default()
                        }],
                        ..Default::default()
                    },
                ],
                ..Default::default()
            }],
        };

        let bytes = stream.to_bytes();
        let tree = parse_water_quadtree(&bytes).unwrap();
        assert_eq!(tree.region_size, 2048);
        assert_eq!(tree.quadtree_nodes.len(), 1);
        assert_eq!(tree.quadtree_nodes[0].height, 12.5);
        assert_eq!(tree.quadtree_nodes[0].floor_height, 3.25);
        assert_eq!(tree.quadtree_nodes[0].flags, [2]);
    }

    fn value_element<const N: usize>(id: Uuid, field_crc: u32, data: [u8; N]) -> Element {
        Element {
            flags: ST_BINARYFLAG_ELEMENT_HEADER
                | ST_BINARYFLAG_HAS_NAME
                | ST_BINARYFLAG_HAS_VALUE
                | 4,
            name_crc: Some(field_crc),
            id,
            data_size: Some(data.len()),
            data: Some(data.to_vec()),
            ..Default::default()
        }
    }
}
