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
pub struct SequenceMarker {
    #[serde(rename = "m_name", default)]
    pub name: i32,
}

impl AzRtti for SequenceMarker {
    const NAME: &'static str = "SequenceMarker";
    const TYPE_ID: AzUuid = AzUuid::from_u128(0xF11A96D4_6200_4D38_9FCF_03AA8DBB1558);
    const BASE_TYPE_IDS: &'static [AzUuid] =
        &[AzUuid::from_u128(0x9B454E3B_282D_4089_90BE_DF25317205E7)];
}
