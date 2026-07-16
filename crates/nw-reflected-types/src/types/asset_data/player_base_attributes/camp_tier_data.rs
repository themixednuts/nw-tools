use crate::az::rtti::AzRtti;
use crate::az::uuid::Uuid as AzUuid;
use crate::types::EditCrc;
use bevy_reflect::{ReflectDeserialize, ReflectSerialize};

#[derive(
    Debug,
    Default,
    Clone,
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
pub struct CampTierData {
    #[serde(rename = "Blueprint Id", default)]
    pub blueprint_id: EditCrc,
    #[serde(rename = "Effect Id", default)]
    pub effect_id: EditCrc,
}

impl AzRtti for CampTierData {
    const NAME: &'static str = "CampTierData";
    const TYPE_ID: AzUuid = AzUuid::from_u128(0xBDAE3278_3CDA_4359_B4BF_BFDA6A818753);
}
