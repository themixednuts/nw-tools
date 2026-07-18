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
pub struct QueryShapeSphere {
    #[serde(rename = "m_radius", default)]
    pub radius: f32,
}

impl AzRtti for QueryShapeSphere {
    const NAME: &'static str = "QueryShapeSphere";
    const TYPE_ID: AzUuid = AzUuid::from_u128(0x7F2EF312_4089_4582_89C5_5D4156DAA7FB);
    const BASE_TYPE_IDS: &'static [AzUuid] =
        &[AzUuid::from_u128(0xCF5EB8E9_9C03_4A19_828D_ED500C732978)];
}
