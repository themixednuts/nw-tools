use crate::az::rtti::AzRtti;
use crate::az::uuid::Uuid as AzUuid;
use bevy_reflect::{ReflectDeserialize, ReflectSerialize};

pub mod simple_asset_reference_bink_asset;

pub use self::simple_asset_reference_bink_asset::SimpleAssetReferenceBinkAsset;

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
pub struct SimpleAssetReferenceBase {
    #[serde(rename = "AssetPath", default)]
    pub asset_path: String,
}

impl AzRtti for SimpleAssetReferenceBase {
    const NAME: &'static str = "SimpleAssetReferenceBase";
    const TYPE_ID: AzUuid = AzUuid::from_u128(0xE16CA6C5_5C78_4AD9_8E9B_F8C1FB4D1DB8);
}
