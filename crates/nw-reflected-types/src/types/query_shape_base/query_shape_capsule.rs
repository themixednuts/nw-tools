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
pub struct QueryShapeCapsule {
    #[serde(rename = "m_height", default)]
    pub height: f32,
    #[serde(rename = "m_radius", default)]
    pub radius: f32,
    #[serde(rename = "m_axis", default)]
    pub axis: bevy_math::Vec3,
}

impl AzRtti for QueryShapeCapsule {
    const NAME: &'static str = "QueryShapeCapsule";
    const TYPE_ID: AzUuid = AzUuid::from_u128(0x7495C65C_9193_4193_BBB2_DE3343B9EB03);
    const BASE_TYPE_IDS: &'static [AzUuid] =
        &[AzUuid::from_u128(0xCF5EB8E9_9C03_4A19_828D_ED500C732978)];
}
