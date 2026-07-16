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
pub struct HandToWeaponIK {
    #[serde(rename = "m_animDrivenIKHandle", default)]
    pub anim_driven_ik_handle: i8,
    #[serde(rename = "m_targetAttachmentName", default)]
    pub target_attachment_name: i8,
}

impl AzRtti for HandToWeaponIK {
    const NAME: &'static str = "HandToWeaponIK";
    const TYPE_ID: AzUuid = AzUuid::from_u128(0x05DE5783_F19A_47C7_BC25_5DE7CFAC7119);
    const BASE_TYPE_IDS: &'static [AzUuid] =
        &[AzUuid::from_u128(0x9B454E3B_282D_4089_90BE_DF25317205E7)];
}
