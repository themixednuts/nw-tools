use crate::az::crc::Crc32 as AzCrc32;
use crate::az::rtti::AzRtti;
use crate::az::uuid::Uuid as AzUuid;
use crate::types::GDEID;
use bevy_reflect::{ReflectDeserialize, ReflectSerialize};

#[derive(
    Debug,
    Default,
    Clone,
    Copy,
    PartialEq,
    serde::Serialize,
    serde::Deserialize,
    bevy_reflect::Reflect,
)]
#[reflect(Serialize, Deserialize)]
pub struct InstancedSlayerScriptPart {
    #[serde(rename = "TagId", default)]
    pub tag_id: AzCrc32,
    #[serde(rename = "WorldPosition", default)]
    pub world_position: bevy_math::Vec3,
    #[serde(rename = "GDEID", default)]
    pub gdeid: GDEID,
    #[serde(rename = "SpawnId", default)]
    pub spawn_id: AzCrc32,
}

impl AzRtti for InstancedSlayerScriptPart {
    const NAME: &'static str = "InstancedSlayerScriptPart";
    const TYPE_ID: AzUuid = AzUuid::from_u128(0x41584DD1_E69B_4EC2_B9BE_45BC5ECDBBA9);
}
