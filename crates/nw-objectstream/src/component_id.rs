//! AZ `ComponentId` / `vector<ComponentId>` ObjectStream value helpers.

use crate::value::{ObjectStreamValueError, field_name, read_u64_scalar};
use crate::{Element, types};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
#[repr(transparent)]
pub struct ComponentId(u64);

impl ComponentId {
    pub const INVALID: Self = Self(0);

    #[must_use]
    pub const fn new(value: u64) -> Self {
        Self(value)
    }

    #[must_use]
    pub const fn value(self) -> u64 {
        self.0
    }

    #[must_use]
    pub const fn is_valid(self) -> bool {
        self.0 != Self::INVALID.0
    }
}

impl Default for ComponentId {
    fn default() -> Self {
        Self::INVALID
    }
}

/// Decode a reflected `AZ::ComponentId` (`typedef AZ::u64`).
pub fn read_component_id(element: &Element) -> Result<ComponentId, ObjectStreamValueError> {
    if !is_component_id_type(element) {
        return Err(ObjectStreamValueError::UnexpectedType {
            field: field_name(element),
            expected: "AZ::ComponentId or AZ::u64",
            actual: *element.id(),
        });
    }

    read_u64_scalar(element).map(ComponentId::new)
}

/// Decode a reflected `AZStd::vector<AZ::u64>` component-id list.
///
/// Payloads use the folded vector type UUID (`COMPONENT_ID_VECTOR`) with
/// `AZ::u64` children. Generic `AZStd::vector` wrappers with `AZ::u64`
/// children are also accepted.
pub fn read_component_id_vector(
    element: &Element,
) -> Result<Vec<ComponentId>, ObjectStreamValueError> {
    if !is_component_id_vector_type(element) {
        return Err(ObjectStreamValueError::UnexpectedType {
            field: field_name(element),
            expected: "AZStd::vector<AZ::u64> component-id list",
            actual: *element.id(),
        });
    }

    element
        .children()
        .iter()
        .filter(|child| is_component_id_type(child))
        .map(read_component_id)
        .collect()
}

#[inline]
fn is_component_id_type(element: &Element) -> bool {
    matches!(*element.id(), types::AZ_U64)
}

#[inline]
fn is_component_id_vector_type(element: &Element) -> bool {
    matches!(
        *element.id(),
        types::COMPONENT_ID_VECTOR | types::AZSTD_VECTOR
    )
}

impl TryFrom<&Element> for ComponentId {
    type Error = ObjectStreamValueError;

    fn try_from(element: &Element) -> Result<Self, Self::Error> {
        read_component_id(element)
    }
}

impl TryFrom<&Element> for Vec<ComponentId> {
    type Error = ObjectStreamValueError;

    fn try_from(element: &Element) -> Result<Self, Self::Error> {
        read_component_id_vector(element)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use uuid::Uuid;

    #[test]
    fn component_id_vector_type_matches_folded_uuid() {
        let expected = crate::type_uuid::azstd_vector(crate::type_uuid::type_ids::U64);
        assert_eq!(types::COMPONENT_ID_VECTOR, expected);
    }

    #[test]
    fn component_id_uses_az_u64_type_uuid() {
        assert_eq!(types::AZ_U64, crate::type_uuid::type_ids::U64);
    }

    #[test]
    fn decodes_component_id_scalar() {
        let element = leaf("id", types::AZ_U64, 42_u64.to_be_bytes());

        assert_eq!(read_component_id(&element).unwrap(), ComponentId::new(42));
        assert_eq!(
            ComponentId::try_from(&element).unwrap(),
            ComponentId::new(42)
        );
    }

    #[test]
    fn decodes_folded_component_id_vector() {
        let element = Element::new(types::COMPONENT_ID_VECTOR).with_children([
            leaf("Element", types::AZ_U64, 1_u64.to_be_bytes()),
            leaf("Element", types::AZ_U64, 2_u64.to_be_bytes()),
            leaf("ignored", types::UNSIGNED_INT, 9_u32.to_be_bytes()),
        ]);

        assert_eq!(
            read_component_id_vector(&element).unwrap(),
            vec![ComponentId::new(1), ComponentId::new(2)]
        );
        assert_eq!(
            Vec::<ComponentId>::try_from(&element).unwrap(),
            vec![ComponentId::new(1), ComponentId::new(2)]
        );
    }

    #[test]
    fn decodes_generic_vector_of_component_ids() {
        let element = Element::new(types::AZSTD_VECTOR).with_children([
            leaf("Element", types::AZ_U64, 7_u64.to_be_bytes()),
            leaf("Element", types::AZ_U64, 8_u64.to_be_bytes()),
        ]);

        assert_eq!(
            read_component_id_vector(&element).unwrap(),
            vec![ComponentId::new(7), ComponentId::new(8)]
        );
    }

    #[test]
    fn rejects_wrong_vector_type() {
        let element = Element::new(types::ENTITY_ID);

        assert!(matches!(
            read_component_id_vector(&element).unwrap_err(),
            ObjectStreamValueError::UnexpectedType { .. }
        ));
    }

    fn leaf(field: &str, id: Uuid, data: impl Into<Vec<u8>>) -> Element {
        Element::new(id).with_field(field).with_data(data)
    }
}
