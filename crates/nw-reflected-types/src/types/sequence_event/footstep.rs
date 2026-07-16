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
pub struct Footstep {
    #[serde(rename = "m_name", default)]
    pub name: i8,
    #[serde(rename = "m_jointName", default)]
    pub joint_name: i8,
}

impl AzRtti for Footstep {
    const NAME: &'static str = "Footstep";
    const TYPE_ID: AzUuid = AzUuid::from_u128(0x597B5C81_CFE5_47C9_870B_BD37C885AA9D);
    const BASE_TYPE_IDS: &'static [AzUuid] =
        &[AzUuid::from_u128(0x9B454E3B_282D_4089_90BE_DF25317205E7)];
}
