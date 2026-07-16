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
pub struct InputSubComponent;

impl AzRtti for InputSubComponent {
    const NAME: &'static str = "InputSubComponent";
    const TYPE_ID: AzUuid = AzUuid::from_u128(0x3D0F14F8_AE29_4ECC_BC88_26B8F8168398);
}
