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
pub struct HitVolumeState {}

impl AzRtti for HitVolumeState {
    const NAME: &'static str = "HitVolumeState";
    const TYPE_ID: AzUuid = AzUuid::from_u128(0x95AE7E8C_4D34_43E9_94CA_1A2AB67A5BBA);
}
