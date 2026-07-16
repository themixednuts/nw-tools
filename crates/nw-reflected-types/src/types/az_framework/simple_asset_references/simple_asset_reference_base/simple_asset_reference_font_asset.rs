use crate::az::rtti::AzRtti;
use crate::az::uuid::Uuid as AzUuid;
use crate::types::SimpleAssetReferenceBase;
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
pub struct SimpleAssetReferenceFontAsset {
    #[serde(rename = "BaseClass1", default)]
    pub simple_asset_reference_base: SimpleAssetReferenceBase,
}

impl AzRtti for SimpleAssetReferenceFontAsset {
    const NAME: &'static str = "AzFramework::SimpleAssetReference<LyShine::FontAsset>";
    const TYPE_ID: AzUuid = AzUuid::from_u128(0xD6342379_A5FA_4B18_B890_702C2FE99A5A);
    const BASE_TYPE_IDS: &'static [AzUuid] =
        &[AzUuid::from_u128(0xE16CA6C5_5C78_4AD9_8E9B_F8C1FB4D1DB8)];
}
