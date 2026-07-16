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
pub struct ChunkEntityTrace {
    #[serde(rename = "SliceIndex", default)]
    pub slice_index: u16,
    #[serde(rename = "ChunkIndex", default)]
    pub chunk_index: u16,
}

impl AzRtti for ChunkEntityTrace {
    const NAME: &'static str = "ChunkEntityTrace";
    const TYPE_ID: AzUuid = AzUuid::from_u128(0xA277172A_44F9_4365_9077_A40FB3D2A84F);
}
