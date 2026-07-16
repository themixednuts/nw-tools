use crate::az::rtti::AzRtti;
use crate::az::uuid::Uuid as AzUuid;
use bevy_reflect::{ReflectDeserialize, ReflectSerialize};

pub mod collision_filter_color;
pub mod editable_collision_filter;

pub use self::collision_filter_color::CollisionFilterColor;
pub use self::editable_collision_filter::EditableCollisionFilter;

#[derive(
    Debug, Default, Clone, PartialEq, serde::Serialize, serde::Deserialize, bevy_reflect::Reflect,
)]
#[reflect(Serialize, Deserialize)]
pub struct CollisionFiltersAsset {
    #[serde(rename = "Categories", default)]
    pub categories: Vec<String>,
    #[serde(rename = "Filters", default)]
    pub filters: Vec<EditableCollisionFilter>,
    #[serde(rename = "CharacterFilterColor", default)]
    pub character_filter_color: bevy_color::LinearRgba,
    #[serde(rename = "GhostFilterColor", default)]
    pub ghost_filter_color: bevy_color::LinearRgba,
    #[serde(rename = "SleepingBodyColor", default)]
    pub sleeping_body_color: bevy_color::LinearRgba,
    #[serde(rename = "CustomFilterColors", default)]
    pub custom_filter_colors: Vec<CollisionFilterColor>,
}

impl AzRtti for CollisionFiltersAsset {
    const NAME: &'static str = "CollisionFiltersAsset";
    const TYPE_ID: AzUuid = AzUuid::from_u128(0x3F5634A1_8683_4783_8ACB_07478CB686FE);
    const BASE_TYPE_IDS: &'static [AzUuid] =
        &[AzUuid::from_u128(0xAF3F7D32_1536_422A_89F3_A11E1F5B5A9C)];
}
