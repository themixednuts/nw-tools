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
pub struct GDEID {
    #[serde(default)]
    pub id: u64,
}

impl AzRtti for GDEID {
    const NAME: &'static str = "GDEID";
    const TYPE_ID: AzUuid = AzUuid::from_u128(0x07CE17BA_C4B7_4B42_81C1_79AF6A61F9A5);
}
