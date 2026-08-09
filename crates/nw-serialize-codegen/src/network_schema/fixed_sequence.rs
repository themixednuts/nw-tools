use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

use super::{
    NetworkNativeTypeInfoEvidence, NetworkWireScalarShape, NetworkWireShape, hex_or_decimal_u64,
    native_type_info_evidence, parse_network_wire_scalar_shape, parse_network_wire_shape, string,
    string_ref,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum NetworkFixedSequenceStorageKind {
    InlineFixed,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NetworkFixedSequenceWireShape {
    pub element: Box<NetworkWireShape>,
    pub capacity: u16,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NetworkFixedSequenceShape {
    pub storage_kind: NetworkFixedSequenceStorageKind,
    pub capacity: u16,
    pub element_stride: u64,
    pub data_offset: u64,
    pub end_offset: u64,
    pub element_wire_shape: NetworkWireScalarShape,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub element_wire_shape_source: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub element_type_info: Option<NetworkNativeTypeInfoEvidence>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub count_callsite: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub loop_header: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source: Option<String>,
}

impl NetworkFixedSequenceShape {
    pub fn wire_shape(&self) -> NetworkFixedSequenceWireShape {
        NetworkFixedSequenceWireShape {
            element: Box::new(self.element_wire_shape.into()),
            capacity: self.capacity,
        }
    }
}

pub(super) fn parse_fixed_sequence_shape(
    object: &Map<String, Value>,
) -> Option<NetworkFixedSequenceShape> {
    let storage_kind = match string_ref(object, "storageKind")? {
        "inline-fixed" => NetworkFixedSequenceStorageKind::InlineFixed,
        _ => return None,
    };
    let capacity = u16::try_from(object.get("capacity")?.as_u64()?).ok()?;
    let element_stride = hex_or_decimal_u64(object, "elementStride")?;
    let data_offset = hex_or_decimal_u64(object, "dataOffset")?;
    let end_offset = hex_or_decimal_u64(object, "endOffset")?;
    let inline_span = element_stride.checked_mul(u64::from(capacity))?;
    if capacity == 0 || element_stride == 0 || end_offset.checked_sub(data_offset)? != inline_span {
        return None;
    }
    let element_wire_shape = string_ref(object, "elementWireShape")
        .or_else(|| string_ref(object, "elementWireLayout"))
        .and_then(parse_network_wire_scalar_shape)?;
    let element_type_info = object
        .get("elementTypeInfo")
        .and_then(Value::as_object)
        .map(native_type_info_evidence);
    if element_type_info
        .as_ref()
        .and_then(|evidence| evidence.native_size)
        .is_some_and(|native_size| native_size != element_stride)
    {
        return None;
    }
    Some(NetworkFixedSequenceShape {
        storage_kind,
        capacity,
        element_stride,
        data_offset,
        end_offset,
        element_wire_shape,
        element_wire_shape_source: string(object, "elementWireShapeSource")
            .or_else(|| string(object, "elementWireLayoutSource")),
        element_type_info,
        count_callsite: string(object, "countCallsite"),
        loop_header: string(object, "loopHeader"),
        source: string(object, "source"),
    })
}

pub(super) fn parse_fixed_sequence_wire_shape(
    value: &str,
) -> Option<NetworkFixedSequenceWireShape> {
    let inner = value.strip_prefix("fixed-vector<")?.strip_suffix('>')?;
    let (element, capacity) = inner.rsplit_once(',')?;
    let capacity = capacity.trim().parse::<u16>().ok()?;
    (capacity > 0).then_some(NetworkFixedSequenceWireShape {
        element: Box::new(parse_network_wire_shape(element.trim())?),
        capacity,
    })
}

#[cfg(test)]
mod tests {
    use serde_json::json;
    use uuid::Uuid;

    use super::*;

    #[test]
    fn parses_proven_inline_fixed_sequence() {
        let value = json!({
            "storageKind": "inline-fixed",
            "capacity": 10,
            "elementStride": "0x20",
            "dataOffset": "0x10",
            "endOffset": "0x150",
            "elementWireLayout": "fixed-bytes-16",
            "elementTypeInfo": {
                "name": "GroupId",
                "typeId": "A40B891F-7B5F-434A-9E79-F7456844E5F3",
                "nativeSize": "0x20"
            }
        });
        let shape = parse_fixed_sequence_shape(value.as_object().unwrap()).unwrap();

        assert_eq!(shape.capacity, 10);
        assert_eq!(shape.element_stride, 0x20);
        assert_eq!(
            shape.element_wire_shape,
            NetworkWireScalarShape::FixedBytes(16)
        );
        assert_eq!(
            shape.element_type_info.unwrap().type_id,
            Some(Uuid::parse_str("A40B891F-7B5F-434A-9E79-F7456844E5F3").unwrap())
        );
    }

    #[test]
    fn rejects_capacity_that_does_not_match_inline_span() {
        let value = json!({
            "storageKind": "inline-fixed",
            "capacity": 9,
            "elementStride": "0x20",
            "dataOffset": "0x10",
            "endOffset": "0x150",
            "elementWireLayout": "fixed-bytes-16"
        });

        assert!(parse_fixed_sequence_shape(value.as_object().unwrap()).is_none());
    }

    #[test]
    fn parses_nested_fixed_sequence_wire_shape() {
        let shape = parse_fixed_sequence_wire_shape(
            "fixed-vector<fixed-vector<composite<u32,string>,5>,20>",
        )
        .unwrap();

        assert_eq!(shape.capacity, 20);
        assert_eq!(
            shape.element.as_ref(),
            &NetworkWireShape::FixedSequence(NetworkFixedSequenceWireShape {
                element: Box::new(NetworkWireShape::Composite(vec![
                    NetworkWireShape::U32,
                    NetworkWireShape::String,
                ])),
                capacity: 5,
            })
        );
    }
}
