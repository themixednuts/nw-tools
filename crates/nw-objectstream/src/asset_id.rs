//! AZ `Data::AssetId` ObjectStream value helpers.

use nw_asset::AssetId;

use crate::value::{
    ElementFields, FieldAccess, ObjectStreamValueError, field_name, read_u32, read_uuid,
};
use crate::{Element, types};

/// Decode an `AZ::Data::AssetId` ObjectStream object.
///
/// The reflected shape is an `AssetId` element with `guid` and `subId`
/// children. Child lookup is non-consuming, so XML/binary dumps with
/// equivalent reflected fields decode the same even if child order changes.
pub fn read_asset_id(element: &Element) -> Result<AssetId, ObjectStreamValueError> {
    if !is_asset_id_type(element) {
        return Err(ObjectStreamValueError::UnexpectedType {
            field: field_name(element),
            expected: "AZ::Data::AssetId",
            actual: *element.id(),
        });
    }

    let mut fields = ElementFields::new(element);
    let guid = fields.required_element("guid").and_then(read_uuid)?;
    let sub_id = fields.required_element("subId").and_then(read_u32)?;
    Ok(AssetId::new(guid, sub_id))
}

/// Decode an `AZ::Data::AssetId` and treat the nil sentinel as absent.
pub fn read_non_nil_asset_id(element: &Element) -> Result<Option<AssetId>, ObjectStreamValueError> {
    read_asset_id(element).map(|asset_id| (!asset_id.is_nil()).then_some(asset_id))
}

/// Decode direct `AZ::Data::AssetId` children from a reflected vector.
pub fn read_asset_id_vector(element: &Element) -> Result<Vec<AssetId>, ObjectStreamValueError> {
    element
        .children()
        .iter()
        .filter(|child| is_asset_id_type(child))
        .map(read_asset_id)
        .collect()
}

#[inline]
fn is_asset_id_type(element: &Element) -> bool {
    matches!(*element.id(), types::ASSET_ID | types::ASSET)
}

#[cfg(test)]
mod tests {
    use super::*;
    use uuid::Uuid;

    #[test]
    fn decodes_asset_id_fields() {
        let guid = Uuid::from_u128(0x11111111_2222_3333_4444_555555555555);
        let element = asset_id_element(guid, 7);

        assert_eq!(read_asset_id(&element).unwrap(), AssetId::new(guid, 7));
    }

    #[test]
    fn decodes_runtime_asset_id_type() {
        let guid = Uuid::from_u128(0x11111111_2222_3333_4444_555555555555);
        let element = Element::new(types::ASSET_ID).with_children([
            leaf("guid", types::AZ_UUID, guid.as_bytes().to_vec()),
            leaf("subId", types::UNSIGNED_INT, 7_u32.to_be_bytes()),
        ]);

        assert_eq!(read_asset_id(&element).unwrap(), AssetId::new(guid, 7));
    }

    #[test]
    fn decodes_asset_id_fields_without_order_dependency() {
        let guid = Uuid::from_u128(0x11111111_2222_3333_4444_555555555555);
        let element = Element::new(types::ASSET).with_children([
            leaf("subId", types::UNSIGNED_INT, 7_u32.to_be_bytes()),
            leaf("guid", types::AZ_UUID, guid.as_bytes().to_vec()),
        ]);

        assert_eq!(read_asset_id(&element).unwrap(), AssetId::new(guid, 7));
    }

    #[test]
    fn nil_asset_id_is_optional_absent() {
        let element = asset_id_element(Uuid::nil(), 0);

        assert_eq!(read_non_nil_asset_id(&element).unwrap(), None);
    }

    #[test]
    fn reads_asset_id_vector_children() {
        let first = Uuid::from_u128(1);
        let second = Uuid::from_u128(2);
        let element = Element::new(types::AZSTD_VECTOR).with_children([
            asset_id_element(first, 0),
            Element::new(types::ASSET_ID).with_children([
                leaf("guid", types::AZ_UUID, second.as_bytes().to_vec()),
                leaf("subId", types::UNSIGNED_INT, 3_u32.to_be_bytes()),
            ]),
            leaf("notAsset", types::UNSIGNED_INT, 12_u32.to_be_bytes()),
        ]);

        assert_eq!(
            read_asset_id_vector(&element).unwrap(),
            vec![AssetId::new(first, 0), AssetId::new(second, 3)]
        );
    }

    #[test]
    fn rejects_wrong_type_and_missing_fields() {
        assert!(matches!(
            read_asset_id(&leaf("Asset", types::UNSIGNED_INT, 7_u32.to_be_bytes())).unwrap_err(),
            ObjectStreamValueError::UnexpectedType { .. }
        ));
        assert!(matches!(
            read_asset_id(&Element::new(types::ASSET)).unwrap_err(),
            ObjectStreamValueError::MissingField { field } if field == "guid"
        ));
    }

    fn asset_id_element(guid: Uuid, sub_id: u32) -> Element {
        Element::new(types::ASSET).with_children([
            leaf("guid", types::AZ_UUID, guid.as_bytes().to_vec()),
            leaf("subId", types::UNSIGNED_INT, sub_id.to_be_bytes()),
        ])
    }

    fn leaf(field: &str, id: Uuid, data: impl Into<Vec<u8>>) -> Element {
        Element::new(id).with_field(field).with_data(data)
    }
}
