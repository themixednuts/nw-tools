use crate::az::rtti::AzRtti;
use crate::az::uuid::Uuid as AzUuid;
use crate::types::ExecutionActivity;
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
pub struct SetActiveWeapon {
    #[serde(rename = "BaseClass1", default)]
    pub execution_activity: ExecutionActivity,
}

impl AzRtti for SetActiveWeapon {
    const NAME: &'static str = "SetActiveWeapon";
    const TYPE_ID: AzUuid = AzUuid::from_u128(0xEB228CD2_B99A_4B6A_8A07_B103F15182CA);
    const BASE_TYPE_IDS: &'static [AzUuid] =
        &[AzUuid::from_u128(0x544B9BF1_0EBF_4786_B4A6_A026628B9E7F)];
}
