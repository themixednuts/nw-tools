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
pub struct CommittedInteraction {
    #[serde(rename = "BaseClass1", default)]
    pub execution_activity: ExecutionActivity,
}

impl AzRtti for CommittedInteraction {
    const NAME: &'static str = "CommittedInteraction";
    const TYPE_ID: AzUuid = AzUuid::from_u128(0xEE347F76_FA31_4352_94D3_524800C95F53);
    const BASE_TYPE_IDS: &'static [AzUuid] =
        &[AzUuid::from_u128(0x544B9BF1_0EBF_4786_B4A6_A026628B9E7F)];
}
