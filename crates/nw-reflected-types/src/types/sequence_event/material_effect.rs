use crate::az::rtti::AzRtti;
use crate::az::uuid::Uuid as AzUuid;
use bevy_reflect::{ReflectDeserialize, ReflectSerialize};

#[derive(
    Debug,
    Default,
    Clone,
    Copy,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Hash,
    serde::Serialize,
    serde::Deserialize,
    bevy_reflect::Reflect,
)]
#[reflect(Serialize, Deserialize)]
pub struct MaterialEffect {
    #[serde(rename = "m_libName", default)]
    pub lib_name: i8,
    #[serde(rename = "m_effectName", default)]
    pub effect_name: i8,
    #[serde(rename = "m_jointName", default)]
    pub joint_name: i8,
}

impl AzRtti for MaterialEffect {
    const NAME: &'static str = "MaterialEffect";
    const TYPE_ID: AzUuid = AzUuid::from_u128(0xE4D903EB_AF52_4085_872D_55A941F810DD);
    const BASE_TYPE_IDS: &'static [AzUuid] =
        &[AzUuid::from_u128(0x9B454E3B_282D_4089_90BE_DF25317205E7)];
}
