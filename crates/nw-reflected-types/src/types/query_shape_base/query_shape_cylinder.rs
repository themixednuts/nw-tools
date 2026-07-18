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
pub struct QueryShapeCylinder {
    #[serde(rename = "m_height", default)]
    pub height: f32,
    #[serde(rename = "m_radius", default)]
    pub radius: f32,
}

impl AzRtti for QueryShapeCylinder {
    const NAME: &'static str = "QueryShapeCylinder";
    const TYPE_ID: AzUuid = AzUuid::from_u128(0x709B11EA_FD56_4FEF_B841_7CEA549368E6);
    const BASE_TYPE_IDS: &'static [AzUuid] =
        &[AzUuid::from_u128(0xCF5EB8E9_9C03_4A19_828D_ED500C732978)];
}
