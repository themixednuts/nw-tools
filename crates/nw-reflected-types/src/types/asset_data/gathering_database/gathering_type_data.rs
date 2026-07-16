use crate::az::rtti::AzRtti;
use crate::az::uuid::Uuid as AzUuid;
use crate::types::SimpleAssetReferenceTextureAsset;
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
pub struct GatheringTypeData {
    #[serde(rename = "Type", default)]
    pub type_: String,
    #[serde(rename = "Ui Icon", default)]
    pub ui_icon: SimpleAssetReferenceTextureAsset,
    #[serde(rename = "Requirement Text", default)]
    pub requirement_text: String,
}

impl AzRtti for GatheringTypeData {
    const NAME: &'static str = "GatheringTypeData";
    const TYPE_ID: AzUuid = AzUuid::from_u128(0x3266A19A_6BAC_4703_B663_9F6ED48F1D76);
}
