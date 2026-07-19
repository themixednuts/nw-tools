use crate::az::rtti::AzRtti;
use crate::az::uuid::Uuid as AzUuid;
use bevy_reflect::{ReflectDeserialize, ReflectSerialize};

#[derive(
    Debug,
    Default,
    Clone,
    PartialEq,
    Eq,
    Hash,
    serde::Serialize,
    serde::Deserialize,
    bevy_reflect::Reflect,
)]
#[reflect(Serialize, Deserialize)]
pub struct ParticleEmitBoneLayer {
    #[serde(rename = "Joint name", default)]
    pub joint_name: String,
    #[serde(rename = "Enable Layer", default)]
    pub enable_layer: bool,
    #[serde(rename = "AffectedIndices", default)]
    pub affected_indices: Vec<u32>,
}

impl AzRtti for ParticleEmitBoneLayer {
    const NAME: &'static str = "ParticleEmitBoneLayer";
    const TYPE_ID: AzUuid = AzUuid::from_u128(0xD29E0CF9_8F02_4E61_BBDE_7BEB76D13FE5);
}
