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
pub struct MaterialHandle;

impl AzRtti for MaterialHandle {
    const NAME: &'static str = "MaterialHandle";
    const TYPE_ID: AzUuid = AzUuid::from_u128(0xBF659DC6_ACDD_4062_A52E_4EC053286F4F);
}
