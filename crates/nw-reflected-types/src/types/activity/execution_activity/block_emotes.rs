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
pub struct BlockEmotes {
    #[serde(rename = "BaseClass1", default)]
    pub execution_activity: ExecutionActivity,
}

impl AzRtti for BlockEmotes {
    const NAME: &'static str = "BlockEmotes";
    const TYPE_ID: AzUuid = AzUuid::from_u128(0x053D9460_30F6_4FF9_B92C_F8262232CAE0);
    const BASE_TYPE_IDS: &'static [AzUuid] =
        &[AzUuid::from_u128(0x544B9BF1_0EBF_4786_B4A6_A026628B9E7F)];
}
