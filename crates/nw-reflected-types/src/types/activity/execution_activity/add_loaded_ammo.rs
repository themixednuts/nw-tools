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
pub struct AddLoadedAmmo {
    #[serde(rename = "BaseClass1", default)]
    pub execution_activity: ExecutionActivity,
}

impl AzRtti for AddLoadedAmmo {
    const NAME: &'static str = "AddLoadedAmmo";
    const TYPE_ID: AzUuid = AzUuid::from_u128(0x11DE1949_071F_488A_91F2_E3A11C9BF3A5);
    const BASE_TYPE_IDS: &'static [AzUuid] =
        &[AzUuid::from_u128(0x544B9BF1_0EBF_4786_B4A6_A026628B9E7F)];
}
