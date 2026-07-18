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
pub struct Any {}

impl AzRtti for Any {
    const NAME: &'static str = "any";
    const TYPE_ID: AzUuid = AzUuid::from_u128(0x03924488_C7F4_4D6D_948B_ABC2D1AE2FD3);
}
