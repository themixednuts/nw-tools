use crate::az::rtti::AzRtti;
use crate::az::uuid::Uuid as AzUuid;
use bevy_reflect::{ReflectDeserialize, ReflectSerialize};

#[derive(
    Debug,
    Default,
    Clone,
    PartialEq,
    Eq,
    Hash,
    serde::Serialize,
    serde::Deserialize,
    bevy_reflect::Reflect,
)]
#[reflect(Serialize, Deserialize)]
pub struct EditableCollisionFilter {
    #[serde(rename = "Name", default)]
    pub name: String,
    #[serde(rename = "Description", default)]
    pub description: String,
    #[serde(rename = "InheritsFilters", default)]
    pub inherits_filters: Vec<String>,
    #[serde(rename = "IsCategories", default)]
    pub is_categories: Vec<String>,
    #[serde(rename = "CollideWithCategories", default)]
    pub collide_with_categories: Vec<String>,
    #[serde(rename = "FilterTags", default)]
    pub filter_tags: Vec<u8>,
}

impl AzRtti for EditableCollisionFilter {
    const NAME: &'static str = "EditableCollisionFilter";
    const TYPE_ID: AzUuid = AzUuid::from_u128(0x0F8A4615_8824_4E01_BA47_A5CBF14227CA);
}
