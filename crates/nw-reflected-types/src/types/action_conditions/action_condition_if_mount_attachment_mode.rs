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
pub struct ActionConditionIfMountAttachmentMode;

impl AzRtti for ActionConditionIfMountAttachmentMode {
    const NAME: &'static str = "ActionConditionIfMountAttachmentMode";
    const TYPE_ID: AzUuid = AzUuid::from_u128(0xE0EB8F2E_7D1D_488C_AD7A_B2DE2005E72C);
    const BASE_TYPE_IDS: &'static [AzUuid] =
        &[AzUuid::from_u128(0x401EA5B5_DDE2_4848_BE17_FD45660FF8C5)];
}
