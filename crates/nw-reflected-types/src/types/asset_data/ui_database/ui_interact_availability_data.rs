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
pub struct UiInteractAvailabilityData {
    #[serde(rename = "Availability", default)]
    pub availability: i32,
}

impl AzRtti for UiInteractAvailabilityData {
    const NAME: &'static str = "UiInteractAvailabilityData";
    const TYPE_ID: AzUuid = AzUuid::from_u128(0x2E72BBBE_7A5A_4D26_B26F_3F50D648664E);
}
