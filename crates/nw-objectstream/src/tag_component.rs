//! Typed ordinary `TagComponent` inspection on serialized `AZ::Entity` values.

use thiserror::Error;
use uuid::{Uuid, uuid};

use crate::query::az_entity_elements;
use crate::value::{ObjectStreamValueError, child_by_field, read_crc32_vector, read_entity_id};
use crate::{Element, types};

/// Ordinary Lumberyard entity tag component.
///
/// New World's separate `NWTagComponent` intentionally has a different UUID and
/// is never accepted by this reader.
pub const TAG_COMPONENT_TYPE_ID: Uuid = uuid!("0f16a377-eaa0-47d2-8472-9eaaa680b169");

/// The ordinary tags directly co-owned by one serialized `AZ::Entity`.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct EntityTagComponent {
    pub entity_id: u64,
    pub tags: Vec<u32>,
}

#[derive(Debug, Error)]
pub enum TagComponentError {
    #[error("ordinary TagComponent entity has invalid Id")]
    EntityId(#[source] ObjectStreamValueError),
    #[error("ordinary TagComponent is missing direct Tags field on entity {entity_id}")]
    MissingTags { entity_id: u64 },
    #[error("ordinary TagComponent has invalid Tags on entity {entity_id}")]
    Tags {
        entity_id: u64,
        #[source]
        source: ObjectStreamValueError,
    },
}

/// Read ordinary `TagComponent` CRCs directly co-owned by `entity`.
///
/// Returns `Ok(None)` when `entity` is not an `AZ::Entity`, has no direct
/// `Components` field, or owns no ordinary `TagComponent`. Once an ordinary
/// component is recognized, malformed `Id` or `Tags` fields fail closed.
pub fn read_entity_tag_component(
    entity: &Element,
) -> Result<Option<EntityTagComponent>, TagComponentError> {
    if entity.id() != &types::AZ_ENTITY {
        return Ok(None);
    }
    let Some(components) = child_by_field(entity, "Components") else {
        return Ok(None);
    };
    let tag_components = components
        .children()
        .iter()
        .filter(|component| component.id() == &TAG_COMPONENT_TYPE_ID)
        .collect::<Vec<_>>();
    if tag_components.is_empty() {
        return Ok(None);
    }

    let entity_id = child_by_field(entity, "Id")
        .ok_or_else(|| ObjectStreamValueError::MissingField {
            field: "Id".to_owned(),
        })
        .and_then(read_entity_id)
        .map_err(TagComponentError::EntityId)?;
    let mut tags = Vec::new();
    for component in tag_components {
        let values = child_by_field(component, "Tags")
            .ok_or(TagComponentError::MissingTags { entity_id })?;
        tags.extend(
            read_crc32_vector(values)
                .map_err(|source| TagComponentError::Tags { entity_id, source })?,
        );
    }
    tags.sort_unstable();
    tags.dedup();
    Ok(Some(EntityTagComponent { entity_id, tags }))
}

/// Read every ordinary per-entity `TagComponent` under ObjectStream roots.
///
/// Results are sorted by numeric entity id and then tag bytes. Duplicate entity
/// records are retained because distinct serialized entities can reuse an id in
/// nested source data; callers that need scene identity must keep their source
/// path as additional provenance.
pub fn read_entity_tag_components(
    roots: &[Element],
) -> Result<Vec<EntityTagComponent>, TagComponentError> {
    let mut components = az_entity_elements(roots)
        .filter_map(|entity| read_entity_tag_component(entity).transpose())
        .collect::<Result<Vec<_>, _>>()?;
    components.sort();
    Ok(components)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types;

    const NW_TAG_COMPONENT_TYPE_ID: Uuid = uuid!("5b7ec8b0-530e-444f-b10f-cd2f30017188");

    #[test]
    fn reads_direct_ordinary_tags_deterministically() {
        let roots = vec![entity(
            42,
            vec![
                tag_component(&[9, 2, 9]),
                Element::new(NW_TAG_COMPONENT_TYPE_ID).with_children(vec![tags(&[1])]),
            ],
        )];

        assert_eq!(
            read_entity_tag_components(&roots).unwrap(),
            vec![EntityTagComponent {
                entity_id: 42,
                tags: vec![2, 9],
            }]
        );
    }

    #[test]
    fn ignores_nested_and_nw_tag_components() {
        let nested = Element::new(Uuid::from_u128(1)).with_children(vec![tag_component(&[3])]);
        let roots = vec![entity(
            7,
            vec![
                Element::new(NW_TAG_COMPONENT_TYPE_ID).with_children(vec![tags(&[4])]),
                nested,
            ],
        )];

        assert!(read_entity_tag_components(&roots).unwrap().is_empty());
    }

    #[test]
    fn malformed_recognized_tags_fail_closed() {
        let malformed_crc = Element::new(types::CRC32).with_children(vec![
            Element::new(types::UNSIGNED_INT)
                .with_field("value")
                .with_data([0, 1]),
        ]);
        let malformed = Element::new(TAG_COMPONENT_TYPE_ID).with_children(vec![
            Element::new(Uuid::from_u128(5))
                .with_field("Tags")
                .with_children(vec![malformed_crc]),
        ]);
        let roots = vec![entity(8, vec![malformed])];

        assert!(matches!(
            read_entity_tag_components(&roots),
            Err(TagComponentError::Tags { entity_id: 8, .. })
        ));
    }

    #[test]
    fn missing_recognized_tags_field_fails_closed() {
        let roots = vec![entity(9, vec![Element::new(TAG_COMPONENT_TYPE_ID)])];

        assert!(matches!(
            read_entity_tag_components(&roots),
            Err(TagComponentError::MissingTags { entity_id: 9 })
        ));
    }

    fn entity(id: u64, components: Vec<Element>) -> Element {
        Element::new(types::AZ_ENTITY).with_children(vec![
            entity_id(id).with_field("Id"),
            Element::new(Uuid::from_u128(10))
                .with_field("Components")
                .with_children(components),
        ])
    }

    fn entity_id(id: u64) -> Element {
        Element::new(types::ENTITY_ID).with_children(vec![
            Element::new(types::AZ_U64)
                .with_field("id")
                .with_data(id.to_be_bytes()),
        ])
    }

    fn tag_component(values: &[u32]) -> Element {
        Element::new(TAG_COMPONENT_TYPE_ID).with_children(vec![tags(values)])
    }

    fn tags(values: &[u32]) -> Element {
        Element::new(Uuid::from_u128(11))
            .with_field("Tags")
            .with_children(values.iter().copied().map(crc32).collect::<Vec<_>>())
    }

    fn crc32(value: u32) -> Element {
        Element::new(types::CRC32).with_children(vec![
            Element::new(types::UNSIGNED_INT)
                .with_field("value")
                .with_data(value.to_be_bytes()),
        ])
    }
}
