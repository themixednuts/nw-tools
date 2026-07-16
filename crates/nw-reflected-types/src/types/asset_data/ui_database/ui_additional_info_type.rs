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
pub struct UiAdditionalInfoType {
    #[serde(rename = "Additional Info Type", default)]
    pub additional_info_type: i32,
}

impl AzRtti for UiAdditionalInfoType {
    const NAME: &'static str = "UiAdditionalInfoType";
    const TYPE_ID: AzUuid = AzUuid::from_u128(0x3AABE13D_4482_4DFE_8EF5_B843F5606E7F);
}
