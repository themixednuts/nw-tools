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
pub struct QueryShapeBox {
    #[serde(rename = "m_box", default)]
    pub box_: bevy_math::Vec3,
}

impl AzRtti for QueryShapeBox {
    const NAME: &'static str = "QueryShapeBox";
    const TYPE_ID: AzUuid = AzUuid::from_u128(0xC6651A66_23D4_4508_B4AD_180C516655A8);
    const BASE_TYPE_IDS: &'static [AzUuid] =
        &[AzUuid::from_u128(0xCF5EB8E9_9C03_4A19_828D_ED500C732978)];
}
