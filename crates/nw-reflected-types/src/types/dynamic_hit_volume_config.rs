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
pub struct DynamicHitVolumeConfig {}

impl AzRtti for DynamicHitVolumeConfig {
    const NAME: &'static str = "DynamicHitVolumeConfig";
    const TYPE_ID: AzUuid = AzUuid::from_u128(0xC4D808A6_BC5E_4DA2_889D_D5497F8E2A36);
}
