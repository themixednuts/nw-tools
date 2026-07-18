use crate::az::rtti::AzRtti;
use crate::az::uuid::Uuid as AzUuid;
use bevy_reflect::{ReflectDeserialize, ReflectSerialize};

#[derive(
    Debug,
    Default,
    Clone,
    Copy,
    PartialEq,
    serde::Serialize,
    serde::Deserialize,
    bevy_reflect::Reflect,
)]
#[reflect(Serialize, Deserialize)]
pub struct QueryShapeAabb {
    #[serde(rename = "m_aabb", default)]
    pub aabb: bevy_math::Vec3,
}

impl AzRtti for QueryShapeAabb {
    const NAME: &'static str = "QueryShapeAabb";
    const TYPE_ID: AzUuid = AzUuid::from_u128(0x27462017_FE0F_4B81_96E9_8875B750EC69);
    const BASE_TYPE_IDS: &'static [AzUuid] =
        &[AzUuid::from_u128(0xCF5EB8E9_9C03_4A19_828D_ED500C732978)];
}
