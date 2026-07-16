use crate::az::rtti::AzRtti;
use crate::az::uuid::Uuid as AzUuid;
use crate::types::AssetData;
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
pub struct VegetationImageAsset {
    #[serde(rename = "BaseClass1", default)]
    pub asset_data: AssetData,
    #[serde(rename = "Width", default)]
    pub width: u32,
    #[serde(rename = "Height", default)]
    pub height: u32,
    #[serde(rename = "Format", default)]
    pub format: u32,
    #[serde(rename = "Data", default)]
    pub data: Vec<u8>,
}

impl AzRtti for VegetationImageAsset {
    const NAME: &'static str = "VegetationImageAsset";
    const TYPE_ID: AzUuid = AzUuid::from_u128(0xE0F05299_DB68_4158_A207_1FD8E1ADC280);
    const BASE_TYPE_IDS: &'static [AzUuid] =
        &[AzUuid::from_u128(0xAF3F7D32_1536_422A_89F3_A11E1F5B5A9C)];
}
